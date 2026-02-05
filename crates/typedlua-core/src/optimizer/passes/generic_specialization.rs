// =============================================================================
// O3: Generic Specialization Pass
// =============================================================================

use crate::config::OptimizationLevel;
use crate::optimizer::WholeProgramPass;
use crate::{build_substitutions, instantiate_function_declaration};
use rustc_hash::FxHashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use typedlua_parser::ast::expression::{ArrayElement, Expression, ExpressionKind, ObjectProperty};
use typedlua_parser::ast::statement::{ForStatement, FunctionDeclaration, Statement};
use typedlua_parser::ast::types::Type;
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::{StringId, StringInterner};

/// Computes a hash of type arguments for caching specialized functions
fn hash_type_args(type_args: &[Type]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for t in type_args {
        // Hash the debug representation - simple but effective
        format!("{:?}", t.kind).hash(&mut hasher);
    }
    hasher.finish()
}

/// Generic specialization pass
/// Creates specialized versions of generic functions for known types
#[derive(Default)]
pub struct GenericSpecializationPass {
    interner: Option<Rc<StringInterner>>,
    /// Maps (function_name, type_args_hash) -> specialized_function_name
    specializations: FxHashMap<(StringId, u64), StringId>,
    /// Counter for generating unique specialization IDs
    next_spec_id: usize,
    /// Collected generic function declarations
    generic_functions: FxHashMap<StringId, FunctionDeclaration>,
    /// New specialized function declarations to add to program
    new_functions: Vec<Statement>,
}

impl GenericSpecializationPass {
    pub fn new(interner: Rc<StringInterner>) -> Self {
        Self {
            interner: Some(interner),
            specializations: FxHashMap::default(),
            next_spec_id: 0,
            generic_functions: FxHashMap::default(),
            new_functions: Vec::new(),
        }
    }

    /// Collects all generic function declarations from the program
    fn collect_generic_functions(&mut self, program: &Program) {
        for stmt in &program.statements {
            if let Statement::Function(func) = stmt {
                if func.type_parameters.is_some() {
                    self.generic_functions.insert(func.name.node, func.clone());
                }
            }
        }
    }

    /// Creates a specialized version of a generic function with concrete type arguments
    fn specialize_function(
        &mut self,
        func: &FunctionDeclaration,
        type_args: &[Type],
    ) -> Option<StringId> {
        let interner = self.interner.as_ref()?;
        let type_params = func.type_parameters.as_ref()?;

        // Build type substitution map
        let substitutions = match build_substitutions(type_params, type_args) {
            Ok(s) => s,
            Err(_) => return None,
        };

        // Check cache first
        let type_args_hash = hash_type_args(type_args);
        let cache_key = (func.name.node, type_args_hash);
        if let Some(&specialized_name) = self.specializations.get(&cache_key) {
            return Some(specialized_name);
        }

        // Generate specialized function name: funcName__spec{id}
        let orig_name = interner.resolve(func.name.node);
        let specialized_name_str = format!("{}__spec{}", orig_name, self.next_spec_id);
        self.next_spec_id += 1;

        // Intern the new name
        let specialized_name = interner.get_or_intern(&specialized_name_str);

        // Create specialized function by instantiating with type substitutions
        let mut specialized_func = instantiate_function_declaration(func, &substitutions);
        specialized_func.name =
            typedlua_parser::ast::Spanned::new(specialized_name, func.name.span);

        // Add to cache and to list of new functions
        self.specializations.insert(cache_key, specialized_name);
        self.new_functions
            .push(Statement::Function(specialized_func));

        Some(specialized_name)
    }

    /// Processes a statement looking for call sites to specialize
    fn specialize_calls_in_statement(&mut self, stmt: &mut Statement) -> bool {
        let mut changed = false;

        match stmt {
            Statement::Variable(var_decl) => {
                if self.specialize_calls_in_expression(&mut var_decl.initializer) {
                    changed = true;
                }
            }
            Statement::Expression(expr) => {
                if self.specialize_calls_in_expression(expr) {
                    changed = true;
                }
            }
            Statement::Return(ret) => {
                for value in &mut ret.values {
                    if self.specialize_calls_in_expression(value) {
                        changed = true;
                    }
                }
            }
            Statement::If(if_stmt) => {
                if self.specialize_calls_in_expression(&mut if_stmt.condition) {
                    changed = true;
                }
                for stmt in &mut if_stmt.then_block.statements {
                    if self.specialize_calls_in_statement(stmt) {
                        changed = true;
                    }
                }
                for else_if in &mut if_stmt.else_ifs {
                    if self.specialize_calls_in_expression(&mut else_if.condition) {
                        changed = true;
                    }
                    for stmt in &mut else_if.block.statements {
                        if self.specialize_calls_in_statement(stmt) {
                            changed = true;
                        }
                    }
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    for stmt in &mut else_block.statements {
                        if self.specialize_calls_in_statement(stmt) {
                            changed = true;
                        }
                    }
                }
            }
            Statement::While(while_stmt) => {
                if self.specialize_calls_in_expression(&mut while_stmt.condition) {
                    changed = true;
                }
                for stmt in &mut while_stmt.body.statements {
                    if self.specialize_calls_in_statement(stmt) {
                        changed = true;
                    }
                }
            }
            Statement::For(for_stmt) => match for_stmt.as_mut() {
                ForStatement::Numeric(num) => {
                    if self.specialize_calls_in_expression(&mut num.start) {
                        changed = true;
                    }
                    if self.specialize_calls_in_expression(&mut num.end) {
                        changed = true;
                    }
                    if let Some(step) = &mut num.step {
                        if self.specialize_calls_in_expression(step) {
                            changed = true;
                        }
                    }
                    for stmt in &mut num.body.statements {
                        if self.specialize_calls_in_statement(stmt) {
                            changed = true;
                        }
                    }
                }
                ForStatement::Generic(gen) => {
                    for iter in &mut gen.iterators {
                        if self.specialize_calls_in_expression(iter) {
                            changed = true;
                        }
                    }
                    for stmt in &mut gen.body.statements {
                        if self.specialize_calls_in_statement(stmt) {
                            changed = true;
                        }
                    }
                }
            },
            Statement::Function(func) => {
                for stmt in &mut func.body.statements {
                    if self.specialize_calls_in_statement(stmt) {
                        changed = true;
                    }
                }
            }
            Statement::Block(block) => {
                for stmt in &mut block.statements {
                    if self.specialize_calls_in_statement(stmt) {
                        changed = true;
                    }
                }
            }
            Statement::Repeat(repeat) => {
                for stmt in &mut repeat.body.statements {
                    if self.specialize_calls_in_statement(stmt) {
                        changed = true;
                    }
                }
                if self.specialize_calls_in_expression(&mut repeat.until) {
                    changed = true;
                }
            }
            Statement::Throw(throw) => {
                if self.specialize_calls_in_expression(&mut throw.expression) {
                    changed = true;
                }
            }
            // Other statements don't contain call expressions we care about
            _ => {}
        }

        changed
    }

    /// Processes an expression looking for call sites to specialize
    fn specialize_calls_in_expression(&mut self, expr: &mut Expression) -> bool {
        let mut changed = false;

        match &mut expr.kind {
            ExpressionKind::Call(callee, args, type_args) => {
                // First process nested expressions
                if self.specialize_calls_in_expression(callee) {
                    changed = true;
                }
                for arg in args.iter_mut() {
                    if self.specialize_calls_in_expression(&mut arg.value) {
                        changed = true;
                    }
                }

                // Check if this is a call to a generic function with concrete type args
                if let Some(type_args) = type_args {
                    if !type_args.is_empty() {
                        // Check if callee is a direct identifier reference to a generic function
                        if let ExpressionKind::Identifier(func_name) = &callee.kind {
                            if let Some(func) = self.generic_functions.get(func_name).cloned() {
                                // Specialize this call
                                if let Some(specialized_name) =
                                    self.specialize_function(&func, type_args)
                                {
                                    // Replace callee with specialized function name
                                    callee.kind = ExpressionKind::Identifier(specialized_name);
                                    // Clear type arguments since the function is now monomorphic
                                    *type_args = Vec::new();
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }

            ExpressionKind::Binary(_, left, right) => {
                if self.specialize_calls_in_expression(left) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(right) {
                    changed = true;
                }
            }

            ExpressionKind::Unary(_, operand) => {
                if self.specialize_calls_in_expression(operand) {
                    changed = true;
                }
            }

            ExpressionKind::Assignment(target, _, value) => {
                if self.specialize_calls_in_expression(target) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(value) {
                    changed = true;
                }
            }

            ExpressionKind::MethodCall(obj, _, args, _) => {
                if self.specialize_calls_in_expression(obj) {
                    changed = true;
                }
                for arg in args.iter_mut() {
                    if self.specialize_calls_in_expression(&mut arg.value) {
                        changed = true;
                    }
                }
                // Method specialization is more complex - skip for now
            }

            ExpressionKind::Member(obj, _) => {
                if self.specialize_calls_in_expression(obj) {
                    changed = true;
                }
            }

            ExpressionKind::Index(obj, index) => {
                if self.specialize_calls_in_expression(obj) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(index) {
                    changed = true;
                }
            }

            ExpressionKind::Array(elements) => {
                for elem in elements.iter_mut() {
                    match elem {
                        ArrayElement::Expression(e) | ArrayElement::Spread(e) => {
                            if self.specialize_calls_in_expression(e) {
                                changed = true;
                            }
                        }
                    }
                }
            }

            ExpressionKind::Object(props) => {
                for prop in props.iter_mut() {
                    match prop {
                        ObjectProperty::Property { value, .. } => {
                            if self.specialize_calls_in_expression(value) {
                                changed = true;
                            }
                        }
                        ObjectProperty::Computed { key, value, .. } => {
                            if self.specialize_calls_in_expression(key) {
                                changed = true;
                            }
                            if self.specialize_calls_in_expression(value) {
                                changed = true;
                            }
                        }
                        ObjectProperty::Spread { value, .. } => {
                            if self.specialize_calls_in_expression(value) {
                                changed = true;
                            }
                        }
                    }
                }
            }

            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                if self.specialize_calls_in_expression(cond) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(then_expr) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(else_expr) {
                    changed = true;
                }
            }

            ExpressionKind::Pipe(left, right) => {
                if self.specialize_calls_in_expression(left) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(right) {
                    changed = true;
                }
            }

            ExpressionKind::Parenthesized(inner) => {
                if self.specialize_calls_in_expression(inner) {
                    changed = true;
                }
            }

            ExpressionKind::TypeAssertion(inner, _) => {
                if self.specialize_calls_in_expression(inner) {
                    changed = true;
                }
            }

            ExpressionKind::OptionalCall(callee, args, _)
            | ExpressionKind::OptionalMethodCall(callee, _, args, _) => {
                if self.specialize_calls_in_expression(callee) {
                    changed = true;
                }
                for arg in args.iter_mut() {
                    if self.specialize_calls_in_expression(&mut arg.value) {
                        changed = true;
                    }
                }
            }

            ExpressionKind::OptionalMember(obj, _) | ExpressionKind::OptionalIndex(obj, _) => {
                if self.specialize_calls_in_expression(obj) {
                    changed = true;
                }
            }

            ExpressionKind::New(callee, args, _) => {
                if self.specialize_calls_in_expression(callee) {
                    changed = true;
                }
                for arg in args.iter_mut() {
                    if self.specialize_calls_in_expression(&mut arg.value) {
                        changed = true;
                    }
                }
            }

            ExpressionKind::ErrorChain(left, right) => {
                if self.specialize_calls_in_expression(left) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(right) {
                    changed = true;
                }
            }

            // Literals, identifiers, self, super - no calls to specialize
            _ => {}
        }

        changed
    }
}

impl WholeProgramPass for GenericSpecializationPass {
    fn name(&self) -> &'static str {
        "generic-specialization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        // Reset state for fresh run
        self.specializations.clear();
        self.generic_functions.clear();
        self.new_functions.clear();
        self.next_spec_id = 0;

        // Phase 1: Collect all generic function declarations
        self.collect_generic_functions(program);

        if self.generic_functions.is_empty() {
            return Ok(false);
        }

        // Phase 2: Find and specialize call sites
        let mut changed = false;
        for stmt in &mut program.statements {
            if self.specialize_calls_in_statement(stmt) {
                changed = true;
            }
        }

        // Phase 3: Add specialized functions to the program
        // Insert them after the original function declarations, not at the end
        // (to avoid being removed by dead code elimination after return statements)
        if !self.new_functions.is_empty() {
            // Find the last function statement index
            let mut insert_idx = 0;
            for (i, stmt) in program.statements.iter().enumerate() {
                if matches!(stmt, Statement::Function(_)) {
                    insert_idx = i + 1;
                }
            }
            // Insert new functions at that position
            for (i, func) in self.new_functions.drain(..).enumerate() {
                program.statements.insert(insert_idx + i, func);
            }
            changed = true;
        }

        Ok(changed)
    }
}
