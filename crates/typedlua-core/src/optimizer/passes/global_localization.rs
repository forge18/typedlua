use bumpalo::Bump;
use crate::config::OptimizationLevel;
use crate::optimizer::WholeProgramPass;
use crate::MutableProgram;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use typedlua_parser::ast::expression::{Expression, ExpressionKind};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{
    Block, ForStatement, Statement, VariableDeclaration, VariableKind,
};
use typedlua_parser::ast::Spanned;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::StringInterner;

pub struct GlobalLocalizationPass {
    interner: Arc<StringInterner>,
}

impl GlobalLocalizationPass {
    /// Create a new pass with the given string interner
    pub fn new(interner: Arc<StringInterner>) -> Self {
        GlobalLocalizationPass { interner }
    }
}

impl<'arena> WholeProgramPass<'arena> for GlobalLocalizationPass {
    fn name(&self) -> &'static str {
        "global-localization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    fn run(
        &mut self,
        program: &mut MutableProgram<'arena>,
        arena: &'arena Bump,
    ) -> Result<bool, String> {
        // Apply global localization to the top-level program statements.
        // MutableProgram.statements is Vec<Statement<'arena>>, so we can work with it directly.
        let changed = self.localize_in_program(program, arena);
        Ok(changed)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl GlobalLocalizationPass {
    /// Collect all declared local names in a block (variables and functions)
    fn collect_declared_locals<'arena>(
        &self,
        statements: &[Statement<'arena>],
        locals: &mut HashSet<typedlua_parser::string_interner::StringId>,
    ) {
        for stmt in statements {
            match stmt {
                Statement::Variable(decl) => {
                    // Extract name(s) from pattern
                    self.collect_pattern_names(&decl.pattern, locals);
                }
                Statement::Function(func) => {
                    // Function name is a local declaration
                    locals.insert(func.name.node);
                }
                _ => {}
            }
        }
    }

    /// Extract all identifier names from a pattern
    fn collect_pattern_names<'arena>(
        &self,
        pattern: &Pattern<'arena>,
        locals: &mut HashSet<typedlua_parser::string_interner::StringId>,
    ) {
        match pattern {
            Pattern::Identifier(ident) => {
                locals.insert(ident.node);
            }
            Pattern::Array(arr) => {
                for elem in arr.elements {
                    match elem {
                        typedlua_parser::ast::pattern::ArrayPatternElement::Pattern(typedlua_parser::ast::pattern::PatternWithDefault { pattern: p, .. }) => {
                            self.collect_pattern_names(p, locals);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Rest(ident) => {
                            locals.insert(ident.node);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Hole => {}
                    }
                }
            }
            Pattern::Object(obj) => {
                for prop in obj.properties {
                    if let Some(value_pattern) = &prop.value {
                        self.collect_pattern_names(value_pattern, locals);
                    } else {
                        // If no value pattern, the key itself is the binding
                        locals.insert(prop.key.node);
                    }
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_, _) => {}
            Pattern::Or(or_pattern) => {
                // All alternatives bind the same variables (guaranteed by type checker)
                // So we can collect from just the first alternative
                if let Some(first) = or_pattern.alternatives.first() {
                    self.collect_pattern_names(first, locals);
                }
            }
        }
    }

    /// Check if a name looks like it was created by this pass (starts with underscore)
    fn is_localized_name(&self, name: typedlua_parser::string_interner::StringId) -> bool {
        let resolved = self.interner.resolve(name);
        resolved.starts_with('_')
    }

    /// Localize globals in the top-level program (MutableProgram has Vec<Statement>).
    fn localize_in_program<'arena>(
        &self,
        program: &mut MutableProgram<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        let mut changed = false;
        let mut declared_locals = HashSet::new();

        // First, collect all locally declared names (variables and functions)
        self.collect_declared_locals(&program.statements, &mut declared_locals);

        let mut global_usage: HashMap<typedlua_parser::string_interner::StringId, usize> =
            HashMap::default();

        for stmt in &program.statements {
            self.collect_global_usage_optimized(stmt, &mut global_usage, &declared_locals);
        }

        let frequently_used: Vec<_> = global_usage
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .filter(|(name, _)| !declared_locals.contains(name))
            // Skip names that already start with underscore to prevent cascading localization
            .filter(|(name, _)| !self.is_localized_name(*name))
            .collect();

        let mut new_statements = Vec::new();

        for (name, count) in &frequently_used {
            if *count > 1 {
                let local_name_id = self.create_local_name(*name);
                let span = Span::dummy();
                let var_decl = self.create_local_declaration(*name, local_name_id, span);

                new_statements.push(Statement::Variable(var_decl));
                declared_locals.insert(local_name_id);
                changed = true;
            }
        }

        for stmt in &mut program.statements {
            self.replace_global_usages(stmt, &frequently_used, &declared_locals, arena);
        }

        new_statements.extend(program.statements.drain(..));
        program.statements = new_statements;

        changed
    }

    /// Localize globals in an immutable Block (clone-to-Vec pattern).
    fn localize_in_block<'arena>(
        &self,
        block: &mut Block<'arena>,
        mut declared_locals: HashSet<typedlua_parser::string_interner::StringId>,
        arena: &'arena Bump,
    ) -> bool {
        let mut changed = false;

        // First, collect all locally declared names (variables and functions)
        self.collect_declared_locals(block.statements, &mut declared_locals);

        let mut global_usage: HashMap<typedlua_parser::string_interner::StringId, usize> =
            HashMap::default();

        for stmt in block.statements {
            self.collect_global_usage_optimized(stmt, &mut global_usage, &declared_locals);
        }

        let frequently_used: Vec<_> = global_usage
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .filter(|(name, _)| !declared_locals.contains(name))
            // Skip names that already start with underscore to prevent cascading localization
            .filter(|(name, _)| !self.is_localized_name(*name))
            .collect();

        let mut new_statements: Vec<Statement<'arena>> = Vec::new();

        for (name, count) in &frequently_used {
            if *count > 1 {
                let local_name_id = self.create_local_name(*name);
                let span = Span::dummy();
                let var_decl = self.create_local_declaration(*name, local_name_id, span);

                new_statements.push(Statement::Variable(var_decl));
                declared_locals.insert(local_name_id);
                changed = true;
            }
        }

        // Clone block statements to a mutable Vec, apply replacements, then combine
        let mut stmts: Vec<Statement<'arena>> = block.statements.to_vec();
        for stmt in &mut stmts {
            self.replace_global_usages(stmt, &frequently_used, &declared_locals, arena);
        }

        new_statements.extend(stmts);
        block.statements = arena.alloc_slice_clone(&new_statements);

        changed
    }

    fn create_local_declaration<'arena>(
        &self,
        global_name: typedlua_parser::string_interner::StringId,
        local_name: typedlua_parser::string_interner::StringId,
        span: Span,
    ) -> VariableDeclaration<'arena> {
        let local_ident = Spanned::new(local_name, span);
        let initializer = Expression::new(ExpressionKind::Identifier(global_name), span);

        VariableDeclaration {
            kind: VariableKind::Local,
            pattern: Pattern::Identifier(local_ident),
            type_annotation: None,
            initializer,
            span,
        }
    }

    fn create_local_name(
        &self,
        original: typedlua_parser::string_interner::StringId,
    ) -> typedlua_parser::string_interner::StringId {
        let name = self.interner.resolve(original);
        let local_name = format!("_{}", name);
        self.interner.get_or_intern(&local_name)
    }

    fn collect_global_usage_optimized<'arena>(
        &self,
        stmt: &Statement<'arena>,
        usage: &mut HashMap<typedlua_parser::string_interner::StringId, usize>,
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
    ) {
        match stmt {
            Statement::Variable(decl) => {
                self.collect_from_expression_optimized(&decl.initializer, usage, declared_locals);
            }
            Statement::Expression(expr) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            Statement::If(if_stmt) => {
                self.collect_from_expression_optimized(&if_stmt.condition, usage, declared_locals);
                for s in if_stmt.then_block.statements {
                    self.collect_global_usage_optimized(s, usage, declared_locals);
                }
                for else_if in if_stmt.else_ifs {
                    self.collect_from_expression_optimized(
                        &else_if.condition,
                        usage,
                        declared_locals,
                    );
                    for s in else_if.block.statements {
                        self.collect_global_usage_optimized(s, usage, declared_locals);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for s in else_block.statements {
                        self.collect_global_usage_optimized(s, usage, declared_locals);
                    }
                }
            }
            Statement::Function(func) => {
                // Function parameters are locals within the function body
                let mut func_locals = declared_locals.clone();
                for param in func.parameters {
                    self.collect_pattern_names(&param.pattern, &mut func_locals);
                }
                for s in func.body.statements {
                    self.collect_global_usage_optimized(s, usage, &func_locals);
                }
            }
            Statement::Return(ret) => {
                for expr in ret.values {
                    self.collect_from_expression_optimized(expr, usage, declared_locals);
                }
            }
            Statement::For(for_stmt) => {
                let mut locals = declared_locals.clone();
                match &**for_stmt {
                    ForStatement::Numeric(for_num) => {
                        locals.insert(for_num.variable.node);
                    }
                    ForStatement::Generic(for_gen) => {
                        for var in for_gen.variables {
                            locals.insert(var.node);
                        }
                    }
                }
                match &**for_stmt {
                    ForStatement::Numeric(for_num) => {
                        for s in for_num.body.statements {
                            self.collect_global_usage_optimized(s, usage, &locals);
                        }
                    }
                    ForStatement::Generic(for_gen) => {
                        for s in for_gen.body.statements {
                            self.collect_global_usage_optimized(s, usage, &locals);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_from_expression_optimized<'arena>(
        &self,
        expr: &Expression<'arena>,
        usage: &mut HashMap<typedlua_parser::string_interner::StringId, usize>,
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
    ) {
        match &expr.kind {
            ExpressionKind::Identifier(name) => {
                if !declared_locals.contains(name) {
                    *usage.entry(*name).or_insert(0) += 1;
                }
            }
            ExpressionKind::Binary(_, left, right) => {
                self.collect_from_expression_optimized(left, usage, declared_locals);
                self.collect_from_expression_optimized(right, usage, declared_locals);
            }
            ExpressionKind::Unary(_, operand) => {
                self.collect_from_expression_optimized(operand, usage, declared_locals);
            }
            ExpressionKind::Call(func, args, _) => {
                self.collect_from_expression_optimized(func, usage, declared_locals);
                for arg in *args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::Member(obj, _) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
                for arg in *args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::Index(obj, index) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
                self.collect_from_expression_optimized(index, usage, declared_locals);
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.collect_from_expression_optimized(cond, usage, declared_locals);
                self.collect_from_expression_optimized(then_expr, usage, declared_locals);
                self.collect_from_expression_optimized(else_expr, usage, declared_locals);
            }
            ExpressionKind::Pipe(left, right) => {
                self.collect_from_expression_optimized(left, usage, declared_locals);
                self.collect_from_expression_optimized(right, usage, declared_locals);
            }
            ExpressionKind::Arrow(arrow) => {
                for param in arrow.parameters {
                    if let Some(default) = &param.default {
                        self.collect_from_expression_optimized(default, usage, declared_locals);
                    }
                }
                self.collect_from_arrow_body(&arrow.body, usage, declared_locals);
            }
            ExpressionKind::Match(match_expr) => {
                self.collect_from_expression_optimized(&match_expr.value, usage, declared_locals);
                for arm in match_expr.arms {
                    self.collect_from_match_arm_body(&arm.body, usage, declared_locals);
                }
            }
            ExpressionKind::New(callee, args, _) => {
                self.collect_from_expression_optimized(callee, usage, declared_locals);
                for arg in *args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.collect_from_expression_optimized(
                    &try_expr.expression,
                    usage,
                    declared_locals,
                );
                self.collect_from_expression_optimized(
                    &try_expr.catch_expression,
                    usage,
                    declared_locals,
                );
            }
            ExpressionKind::ErrorChain(_, expr) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            ExpressionKind::OptionalMember(obj, _) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
                self.collect_from_expression_optimized(index, usage, declared_locals);
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
                for arg in *args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
                for arg in *args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::TypeAssertion(expr, _) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            _ => {}
        }
    }

    fn collect_from_arrow_body<'arena>(
        &self,
        body: &typedlua_parser::ast::expression::ArrowBody<'arena>,
        usage: &mut HashMap<typedlua_parser::string_interner::StringId, usize>,
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
    ) {
        match body {
            typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                for stmt in block.statements {
                    self.collect_global_usage_optimized(stmt, usage, declared_locals);
                }
            }
        }
    }

    fn collect_from_match_arm_body<'arena>(
        &self,
        body: &typedlua_parser::ast::expression::MatchArmBody<'arena>,
        usage: &mut HashMap<typedlua_parser::string_interner::StringId, usize>,
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
    ) {
        match body {
            typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                for stmt in block.statements {
                    self.collect_global_usage_optimized(stmt, usage, declared_locals);
                }
            }
        }
    }

    fn replace_global_usages<'arena>(
        &self,
        stmt: &mut Statement<'arena>,
        frequently_used: &[(typedlua_parser::string_interner::StringId, usize)],
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
        arena: &'arena Bump,
    ) {
        match stmt {
            Statement::Variable(decl) => {
                self.replace_in_expression(&mut decl.initializer, frequently_used, declared_locals, arena);
            }
            Statement::Expression(expr) => {
                self.replace_in_expression(expr, frequently_used, declared_locals, arena);
            }
            Statement::If(if_stmt) => {
                self.replace_in_expression(
                    &mut if_stmt.condition,
                    frequently_used,
                    declared_locals,
                    arena,
                );
                // then_block has immutable statements slice — clone-to-Vec pattern
                let mut then_stmts: Vec<_> = if_stmt.then_block.statements.to_vec();
                for s in &mut then_stmts {
                    self.replace_global_usages(s, frequently_used, declared_locals, arena);
                }
                if_stmt.then_block.statements = arena.alloc_slice_clone(&then_stmts);

                // else_ifs is &'arena [ElseIf] — clone to Vec
                let mut new_else_ifs: Vec<_> = if_stmt.else_ifs.to_vec();
                for else_if in &mut new_else_ifs {
                    self.replace_in_expression(
                        &mut else_if.condition,
                        frequently_used,
                        declared_locals,
                        arena,
                    );
                    let mut else_if_stmts: Vec<_> = else_if.block.statements.to_vec();
                    for s in &mut else_if_stmts {
                        self.replace_global_usages(s, frequently_used, declared_locals, arena);
                    }
                    else_if.block.statements = arena.alloc_slice_clone(&else_if_stmts);
                }
                if_stmt.else_ifs = arena.alloc_slice_clone(&new_else_ifs);

                if let Some(else_block) = &mut if_stmt.else_block {
                    let mut else_stmts: Vec<_> = else_block.statements.to_vec();
                    for s in &mut else_stmts {
                        self.replace_global_usages(s, frequently_used, declared_locals, arena);
                    }
                    else_block.statements = arena.alloc_slice_clone(&else_stmts);
                }
            }
            Statement::While(while_stmt) => {
                self.replace_in_expression(
                    &mut while_stmt.condition,
                    frequently_used,
                    declared_locals,
                    arena,
                );
                let mut body_stmts: Vec<_> = while_stmt.body.statements.to_vec();
                for s in &mut body_stmts {
                    self.replace_global_usages(s, frequently_used, declared_locals, arena);
                }
                while_stmt.body.statements = arena.alloc_slice_clone(&body_stmts);
            }
            Statement::For(for_stmt) => {
                let mut new_locals = declared_locals.clone();
                match &**for_stmt {
                    ForStatement::Numeric(for_num) => {
                        new_locals.insert(for_num.variable.node);
                    }
                    ForStatement::Generic(for_gen) => {
                        for var in for_gen.variables {
                            new_locals.insert(var.node);
                        }
                    }
                }
                match &**for_stmt {
                    ForStatement::Numeric(for_num_ref) => {
                        let mut new_num = (**for_num_ref).clone();
                        let mut body_stmts: Vec<_> = new_num.body.statements.to_vec();
                        for s in &mut body_stmts {
                            self.replace_global_usages(s, frequently_used, &new_locals, arena);
                        }
                        new_num.body.statements = arena.alloc_slice_clone(&body_stmts);
                        *stmt = Statement::For(
                            arena.alloc(ForStatement::Numeric(arena.alloc(new_num))),
                        );
                    }
                    ForStatement::Generic(for_gen_ref) => {
                        let mut new_gen = for_gen_ref.clone();
                        let mut body_stmts: Vec<_> = new_gen.body.statements.to_vec();
                        for s in &mut body_stmts {
                            self.replace_global_usages(s, frequently_used, &new_locals, arena);
                        }
                        new_gen.body.statements = arena.alloc_slice_clone(&body_stmts);
                        *stmt = Statement::For(
                            arena.alloc(ForStatement::Generic(new_gen)),
                        );
                    }
                }
            }
            Statement::Return(ret) => {
                let mut vals: Vec<_> = ret.values.to_vec();
                for expr in &mut vals {
                    self.replace_in_expression(expr, frequently_used, declared_locals, arena);
                }
                ret.values = arena.alloc_slice_clone(&vals);
            }
            Statement::Function(func) => {
                let new_locals = declared_locals.clone();
                let mut body_stmts: Vec<_> = func.body.statements.to_vec();
                for s in &mut body_stmts {
                    self.replace_global_usages(s, frequently_used, &new_locals, arena);
                }
                func.body.statements = arena.alloc_slice_clone(&body_stmts);
            }
            _ => {}
        }
    }

    fn replace_in_expression<'arena>(
        &self,
        expr: &mut Expression<'arena>,
        frequently_used: &[(typedlua_parser::string_interner::StringId, usize)],
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
        arena: &'arena Bump,
    ) {
        match &expr.kind {
            ExpressionKind::Identifier(name) => {
                let name = *name;
                if !declared_locals.contains(&name) {
                    for (global_name, count) in frequently_used {
                        if name == *global_name && *count > 1 {
                            let local_name_id = self.create_local_name(name);
                            expr.kind = ExpressionKind::Identifier(local_name_id);
                            break;
                        }
                    }
                }
            }
            ExpressionKind::Binary(op, left, right) => {
                let op = *op;
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                self.replace_in_expression(&mut new_left, frequently_used, declared_locals, arena);
                self.replace_in_expression(&mut new_right, frequently_used, declared_locals, arena);
                expr.kind = ExpressionKind::Binary(
                    op,
                    arena.alloc(new_left),
                    arena.alloc(new_right),
                );
            }
            ExpressionKind::Unary(op, operand) => {
                let op = *op;
                let mut new_operand = (**operand).clone();
                self.replace_in_expression(&mut new_operand, frequently_used, declared_locals, arena);
                expr.kind = ExpressionKind::Unary(op, arena.alloc(new_operand));
            }
            ExpressionKind::Call(func, args, type_args) => {
                let mut new_func = (**func).clone();
                let type_args = *type_args;
                self.replace_in_expression(&mut new_func, frequently_used, declared_locals, arena);
                let mut new_args: Vec<_> = args.to_vec();
                for arg in &mut new_args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals, arena);
                }
                expr.kind = ExpressionKind::Call(
                    arena.alloc(new_func),
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::Member(obj, member) => {
                let member = member.clone();
                let mut new_obj = (**obj).clone();
                self.replace_in_expression(&mut new_obj, frequently_used, declared_locals, arena);
                expr.kind = ExpressionKind::Member(arena.alloc(new_obj), member);
            }
            ExpressionKind::MethodCall(obj, method, args, type_args) => {
                let method = method.clone();
                let type_args = *type_args;
                let mut new_obj = (**obj).clone();
                self.replace_in_expression(&mut new_obj, frequently_used, declared_locals, arena);
                let mut new_args: Vec<_> = args.to_vec();
                for arg in &mut new_args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals, arena);
                }
                expr.kind = ExpressionKind::MethodCall(
                    arena.alloc(new_obj),
                    method,
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::Index(obj, index) => {
                let mut new_obj = (**obj).clone();
                let mut new_index = (**index).clone();
                self.replace_in_expression(&mut new_obj, frequently_used, declared_locals, arena);
                self.replace_in_expression(&mut new_index, frequently_used, declared_locals, arena);
                expr.kind = ExpressionKind::Index(
                    arena.alloc(new_obj),
                    arena.alloc(new_index),
                );
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut new_cond = (**cond).clone();
                let mut new_then = (**then_expr).clone();
                let mut new_else = (**else_expr).clone();
                self.replace_in_expression(&mut new_cond, frequently_used, declared_locals, arena);
                self.replace_in_expression(&mut new_then, frequently_used, declared_locals, arena);
                self.replace_in_expression(&mut new_else, frequently_used, declared_locals, arena);
                expr.kind = ExpressionKind::Conditional(
                    arena.alloc(new_cond),
                    arena.alloc(new_then),
                    arena.alloc(new_else),
                );
            }
            ExpressionKind::Pipe(left, right) => {
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                self.replace_in_expression(&mut new_left, frequently_used, declared_locals, arena);
                self.replace_in_expression(&mut new_right, frequently_used, declared_locals, arena);
                expr.kind = ExpressionKind::Pipe(
                    arena.alloc(new_left),
                    arena.alloc(new_right),
                );
            }
            ExpressionKind::Match(match_expr) => {
                let mut new_value = (*match_expr.value).clone();
                self.replace_in_expression(&mut new_value, frequently_used, declared_locals, arena);
                let mut new_arms: Vec<_> = match_expr.arms.to_vec();
                for arm in &mut new_arms {
                    self.replace_in_match_arm_body(&mut arm.body, frequently_used, declared_locals, arena);
                }
                expr.kind = ExpressionKind::Match(typedlua_parser::ast::expression::MatchExpression {
                    value: arena.alloc(new_value),
                    arms: arena.alloc_slice_clone(&new_arms),
                    span: match_expr.span,
                });
            }
            ExpressionKind::Arrow(arrow) => {
                let mut new_arrow = arrow.clone();
                let mut new_params: Vec<_> = new_arrow.parameters.to_vec();
                for param in &mut new_params {
                    if let Some(default) = &mut param.default {
                        self.replace_in_expression(default, frequently_used, declared_locals, arena);
                    }
                }
                new_arrow.parameters = arena.alloc_slice_clone(&new_params);
                self.replace_in_arrow_body(&mut new_arrow.body, frequently_used, declared_locals, arena);
                expr.kind = ExpressionKind::Arrow(new_arrow);
            }
            ExpressionKind::New(callee, args, type_args) => {
                let type_args = *type_args;
                let mut new_callee = (**callee).clone();
                self.replace_in_expression(&mut new_callee, frequently_used, declared_locals, arena);
                let mut new_args: Vec<_> = args.to_vec();
                for arg in &mut new_args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals, arena);
                }
                expr.kind = ExpressionKind::New(
                    arena.alloc(new_callee),
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::Try(try_expr) => {
                let mut new_expression = (*try_expr.expression).clone();
                let mut new_catch = (*try_expr.catch_expression).clone();
                self.replace_in_expression(
                    &mut new_expression,
                    frequently_used,
                    declared_locals,
                    arena,
                );
                self.replace_in_expression(
                    &mut new_catch,
                    frequently_used,
                    declared_locals,
                    arena,
                );
                expr.kind = ExpressionKind::Try(typedlua_parser::ast::expression::TryExpression {
                    expression: arena.alloc(new_expression),
                    catch_variable: try_expr.catch_variable.clone(),
                    catch_expression: arena.alloc(new_catch),
                    span: try_expr.span,
                });
            }
            ExpressionKind::ErrorChain(error_expr, handler) => {
                let mut new_error = (**error_expr).clone();
                let mut new_handler = (**handler).clone();
                self.replace_in_expression(&mut new_error, frequently_used, declared_locals, arena);
                self.replace_in_expression(&mut new_handler, frequently_used, declared_locals, arena);
                expr.kind = ExpressionKind::ErrorChain(
                    arena.alloc(new_error),
                    arena.alloc(new_handler),
                );
            }
            ExpressionKind::OptionalMember(obj, member) => {
                let member = member.clone();
                let mut new_obj = (**obj).clone();
                self.replace_in_expression(&mut new_obj, frequently_used, declared_locals, arena);
                expr.kind = ExpressionKind::OptionalMember(arena.alloc(new_obj), member);
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                let mut new_obj = (**obj).clone();
                let mut new_index = (**index).clone();
                self.replace_in_expression(&mut new_obj, frequently_used, declared_locals, arena);
                self.replace_in_expression(&mut new_index, frequently_used, declared_locals, arena);
                expr.kind = ExpressionKind::OptionalIndex(
                    arena.alloc(new_obj),
                    arena.alloc(new_index),
                );
            }
            ExpressionKind::OptionalCall(obj, args, type_args) => {
                let type_args = *type_args;
                let mut new_obj = (**obj).clone();
                self.replace_in_expression(&mut new_obj, frequently_used, declared_locals, arena);
                let mut new_args: Vec<_> = args.to_vec();
                for arg in &mut new_args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals, arena);
                }
                expr.kind = ExpressionKind::OptionalCall(
                    arena.alloc(new_obj),
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::OptionalMethodCall(obj, method, args, type_args) => {
                let method = method.clone();
                let type_args = *type_args;
                let mut new_obj = (**obj).clone();
                self.replace_in_expression(&mut new_obj, frequently_used, declared_locals, arena);
                let mut new_args: Vec<_> = args.to_vec();
                for arg in &mut new_args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals, arena);
                }
                expr.kind = ExpressionKind::OptionalMethodCall(
                    arena.alloc(new_obj),
                    method,
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::TypeAssertion(inner, ty) => {
                let ty = ty.clone();
                let mut new_inner = (**inner).clone();
                self.replace_in_expression(&mut new_inner, frequently_used, declared_locals, arena);
                expr.kind = ExpressionKind::TypeAssertion(arena.alloc(new_inner), ty);
            }
            _ => {}
        }
    }

    fn replace_in_arrow_body<'arena>(
        &self,
        body: &mut typedlua_parser::ast::expression::ArrowBody<'arena>,
        frequently_used: &[(typedlua_parser::string_interner::StringId, usize)],
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
        arena: &'arena Bump,
    ) {
        match body {
            typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                let mut new_expr = (**expr).clone();
                self.replace_in_expression(&mut new_expr, frequently_used, declared_locals, arena);
                *expr = arena.alloc(new_expr);
            }
            typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                let mut stmts: Vec<_> = block.statements.to_vec();
                for stmt in &mut stmts {
                    self.replace_global_usages(stmt, frequently_used, declared_locals, arena);
                }
                block.statements = arena.alloc_slice_clone(&stmts);
            }
        }
    }

    fn replace_in_match_arm_body<'arena>(
        &self,
        body: &mut typedlua_parser::ast::expression::MatchArmBody<'arena>,
        frequently_used: &[(typedlua_parser::string_interner::StringId, usize)],
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
        arena: &'arena Bump,
    ) {
        match body {
            typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                let mut new_expr = (**expr).clone();
                self.replace_in_expression(&mut new_expr, frequently_used, declared_locals, arena);
                *expr = arena.alloc(new_expr);
            }
            typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                let mut stmts: Vec<_> = block.statements.to_vec();
                for stmt in &mut stmts {
                    self.replace_global_usages(stmt, frequently_used, declared_locals, arena);
                }
                block.statements = arena.alloc_slice_clone(&stmts);
            }
        }
    }
}
