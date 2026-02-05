use crate::config::OptimizationLevel;
use crate::diagnostics::DiagnosticHandler;

use std::rc::Rc;
use std::sync::Arc;
use typedlua_parser::ast::expression::Expression;
use typedlua_parser::ast::statement::{Block, ForStatement, Statement};
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::StringInterner;

// Pass modules
mod passes;
use passes::*;

mod rich_enum_optimization;
use rich_enum_optimization::RichEnumOptimizationPass;

mod method_to_function_conversion;
use method_to_function_conversion::MethodToFunctionConversionPass;

mod devirtualization;
use devirtualization::DevirtualizationPass;

mod operator_inlining;
use operator_inlining::OperatorInliningPass;

mod interface_inlining;
use interface_inlining::InterfaceMethodInliningPass;

mod aggressive_inlining;
use aggressive_inlining::AggressiveInliningPass;

// =============================================================================
// Visitor Traits - Core of the pass merging architecture
// =============================================================================

/// Visitor for expression-level transformations
///
/// Implement this trait to participate in composite expression passes.
/// The composite dispatcher handles AST traversal, calling all visitors
/// at each expression node in a single pass.
pub trait ExprVisitor {
    /// Transform an expression in-place
    ///
    /// # Arguments
    /// * `expr` - The expression to potentially transform
    ///
    /// # Returns
    /// `true` if the expression was modified, `false` otherwise
    fn visit_expr(&mut self, expr: &mut Expression) -> bool;
}

/// Visitor for statement-level transformations
///
/// Implement this trait to participate in composite statement passes.
/// The composite dispatcher handles AST traversal, calling all visitors
/// at each statement node in a single pass.
pub trait StmtVisitor {
    /// Transform a statement in-place
    ///
    /// # Arguments
    /// * `stmt` - The statement to potentially transform
    ///
    /// # Returns
    /// `true` if the statement was modified, `false` otherwise
    fn visit_stmt(&mut self, stmt: &mut Statement) -> bool;
}

/// Pass that requires pre-analysis before transformation
///
/// Passes that need to scan the entire program before making
/// transformations should implement this trait. The analysis
/// phase runs before any transformations.
pub trait PreAnalysisPass {
    /// Run analysis phase before transformations
    ///
    /// # Arguments
    /// * `program` - The program to analyze (read-only)
    fn analyze(&mut self, program: &Program);
}

/// Pass that operates on the whole program
///
/// Passes that need whole-program context and cannot be merged
/// into composite traversals should implement this trait.
pub trait WholeProgramPass {
    /// Get the name of this pass
    fn name(&self) -> &'static str;

    /// Get the minimum optimization level required
    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    /// Run the pass on the entire program
    ///
    /// # Arguments
    /// * `program` - The program to transform
    ///
    /// # Returns
    /// `Ok(true)` if changes were made, `Ok(false)` if no changes,
    /// `Err` if an error occurred
    fn run(&mut self, program: &mut Program) -> Result<bool, String>;
}

// =============================================================================
// Composite Passes - Merge multiple visitors into single traversals
// =============================================================================

/// Composite pass that runs multiple expression visitors in one traversal
pub struct ExpressionCompositePass {
    visitors: Vec<Box<dyn ExprVisitor>>,
    #[allow(dead_code)]
    name: &'static str,
}

impl ExpressionCompositePass {
    pub fn new(name: &'static str) -> Self {
        Self {
            visitors: Vec::new(),
            name,
        }
    }

    pub fn add_visitor(&mut self, visitor: Box<dyn ExprVisitor>) {
        self.visitors.push(visitor);
    }

    pub fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.visit_stmt(stmt);
        }
        Ok(changed)
    }

    fn visit_stmt(&mut self, stmt: &mut Statement) -> bool {
        let mut changed = false;

        // Visit expressions within the statement
        match stmt {
            Statement::Variable(decl) => {
                changed |= self.visit_expr(&mut decl.initializer);
            }
            Statement::Expression(expr) => {
                changed |= self.visit_expr(expr);
            }
            Statement::If(if_stmt) => {
                changed |= self.visit_expr(&mut if_stmt.condition);
                changed |= self.visit_block(&mut if_stmt.then_block);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.visit_expr(&mut else_if.condition);
                    changed |= self.visit_block(&mut else_if.block);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.visit_block(else_block);
                }
            }
            Statement::While(while_stmt) => {
                changed |= self.visit_expr(&mut while_stmt.condition);
                changed |= self.visit_block(&mut while_stmt.body);
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Numeric(for_num) => {
                    changed |= self.visit_expr(&mut for_num.start);
                    changed |= self.visit_expr(&mut for_num.end);
                    if let Some(step) = &mut for_num.step {
                        changed |= self.visit_expr(step);
                    }
                    changed |= self.visit_block(&mut for_num.body);
                }
                ForStatement::Generic(for_gen) => {
                    for expr in &mut for_gen.iterators {
                        changed |= self.visit_expr(expr);
                    }
                    changed |= self.visit_block(&mut for_gen.body);
                }
            },
            Statement::Repeat(repeat_stmt) => {
                changed |= self.visit_expr(&mut repeat_stmt.until);
                changed |= self.visit_block(&mut repeat_stmt.body);
            }
            Statement::Return(ret_stmt) => {
                for expr in &mut ret_stmt.values {
                    changed |= self.visit_expr(expr);
                }
            }
            Statement::Function(func) => {
                changed |= self.visit_block(&mut func.body);
            }
            Statement::Block(block) => {
                changed |= self.visit_block(block);
            }
            _ => {}
        }

        changed
    }

    fn visit_block(&mut self, block: &mut Block) -> bool {
        let mut changed = false;
        for stmt in &mut block.statements {
            changed |= self.visit_stmt(stmt);
        }
        changed
    }

    fn visit_expr(&mut self, expr: &mut Expression) -> bool {
        let mut changed = false;

        // First, visit children recursively
        changed |= self.visit_expr_children(expr);

        // Then, apply all visitors to this expression
        for visitor in &mut self.visitors {
            changed |= visitor.visit_expr(expr);
        }

        changed
    }

    fn visit_expr_children(&mut self, expr: &mut Expression) -> bool {
        use typedlua_parser::ast::expression::{ArrayElement, MatchArmBody, ObjectProperty};

        let mut changed = false;

        match &mut expr.kind {
            typedlua_parser::ast::expression::ExpressionKind::Binary(_, left, right) => {
                changed |= self.visit_expr(left);
                changed |= self.visit_expr(right);
            }
            typedlua_parser::ast::expression::ExpressionKind::Unary(_, operand) => {
                changed |= self.visit_expr(operand);
            }
            typedlua_parser::ast::expression::ExpressionKind::Call(func, args, _) => {
                changed |= self.visit_expr(func);
                for arg in args {
                    changed |= self.visit_expr(&mut arg.value);
                }
            }
            typedlua_parser::ast::expression::ExpressionKind::MethodCall(obj, _, args, _) => {
                changed |= self.visit_expr(obj);
                for arg in args {
                    changed |= self.visit_expr(&mut arg.value);
                }
            }
            typedlua_parser::ast::expression::ExpressionKind::Member(obj, _) => {
                changed |= self.visit_expr(obj);
            }
            typedlua_parser::ast::expression::ExpressionKind::Index(obj, index) => {
                changed |= self.visit_expr(obj);
                changed |= self.visit_expr(index);
            }
            typedlua_parser::ast::expression::ExpressionKind::Conditional(
                cond,
                then_expr,
                else_expr,
            ) => {
                changed |= self.visit_expr(cond);
                changed |= self.visit_expr(then_expr);
                changed |= self.visit_expr(else_expr);
            }
            typedlua_parser::ast::expression::ExpressionKind::Array(elements) => {
                for elem in elements {
                    match elem {
                        ArrayElement::Expression(e) => changed |= self.visit_expr(e),
                        ArrayElement::Spread(e) => changed |= self.visit_expr(e),
                    }
                }
            }
            typedlua_parser::ast::expression::ExpressionKind::Object(props) => {
                for prop in props {
                    match prop {
                        ObjectProperty::Property { value, .. } => {
                            changed |= self.visit_expr(value);
                        }
                        ObjectProperty::Computed { key, value, .. } => {
                            changed |= self.visit_expr(key);
                            changed |= self.visit_expr(value);
                        }
                        ObjectProperty::Spread { value, .. } => {
                            changed |= self.visit_expr(value);
                        }
                    }
                }
            }
            typedlua_parser::ast::expression::ExpressionKind::Arrow(arrow) => {
                for param in &mut arrow.parameters {
                    if let Some(default) = &mut param.default {
                        changed |= self.visit_expr(default);
                    }
                }
                match &mut arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(e) => {
                        changed |= self.visit_expr(e);
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        changed |= self.visit_block(block);
                    }
                }
            }
            typedlua_parser::ast::expression::ExpressionKind::Match(match_expr) => {
                changed |= self.visit_expr(&mut match_expr.value);
                for arm in &mut match_expr.arms {
                    match &mut arm.body {
                        MatchArmBody::Expression(e) => changed |= self.visit_expr(e),
                        MatchArmBody::Block(block) => changed |= self.visit_block(block),
                    }
                }
            }
            typedlua_parser::ast::expression::ExpressionKind::New(callee, args, _) => {
                changed |= self.visit_expr(callee);
                for arg in args {
                    changed |= self.visit_expr(&mut arg.value);
                }
            }
            typedlua_parser::ast::expression::ExpressionKind::Pipe(left, right) => {
                changed |= self.visit_expr(left);
                changed |= self.visit_expr(right);
            }
            typedlua_parser::ast::expression::ExpressionKind::Try(try_expr) => {
                changed |= self.visit_expr(&mut try_expr.expression);
                changed |= self.visit_expr(&mut try_expr.catch_expression);
            }
            typedlua_parser::ast::expression::ExpressionKind::ErrorChain(left, right) => {
                changed |= self.visit_expr(left);
                changed |= self.visit_expr(right);
            }
            typedlua_parser::ast::expression::ExpressionKind::OptionalMember(obj, _) => {
                changed |= self.visit_expr(obj);
            }
            typedlua_parser::ast::expression::ExpressionKind::OptionalIndex(obj, index) => {
                changed |= self.visit_expr(obj);
                changed |= self.visit_expr(index);
            }
            typedlua_parser::ast::expression::ExpressionKind::OptionalCall(obj, args, _) => {
                changed |= self.visit_expr(obj);
                for arg in args {
                    changed |= self.visit_expr(&mut arg.value);
                }
            }
            typedlua_parser::ast::expression::ExpressionKind::OptionalMethodCall(
                obj,
                _,
                args,
                _,
            ) => {
                changed |= self.visit_expr(obj);
                for arg in args {
                    changed |= self.visit_expr(&mut arg.value);
                }
            }
            typedlua_parser::ast::expression::ExpressionKind::TypeAssertion(expr, _) => {
                changed |= self.visit_expr(expr);
            }
            typedlua_parser::ast::expression::ExpressionKind::Parenthesized(expr) => {
                changed |= self.visit_expr(expr);
            }
            _ => {}
        }

        changed
    }
}

/// Composite pass that runs multiple statement visitors in one traversal
pub struct StatementCompositePass {
    visitors: Vec<Box<dyn StmtVisitor>>,
    #[allow(dead_code)]
    name: &'static str,
}

impl StatementCompositePass {
    pub fn new(name: &'static str) -> Self {
        Self {
            visitors: Vec::new(),
            name,
        }
    }

    pub fn add_visitor(&mut self, visitor: Box<dyn StmtVisitor>) {
        self.visitors.push(visitor);
    }

    pub fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.visit_stmt(stmt);
        }
        Ok(changed)
    }

    fn visit_stmt(&mut self, stmt: &mut Statement) -> bool {
        let mut changed = false;

        // Visit children first
        changed |= self.visit_stmt_children(stmt);

        // Then apply all visitors to this statement
        for visitor in &mut self.visitors {
            changed |= visitor.visit_stmt(stmt);
        }

        changed
    }

    fn visit_stmt_children(&mut self, stmt: &mut Statement) -> bool {
        let mut changed = false;

        match stmt {
            Statement::Function(func) => {
                for s in &mut func.body.statements {
                    changed |= self.visit_stmt(s);
                }
            }
            Statement::If(if_stmt) => {
                for s in &mut if_stmt.then_block.statements {
                    changed |= self.visit_stmt(s);
                }
                for else_if in &mut if_stmt.else_ifs {
                    for s in &mut else_if.block.statements {
                        changed |= self.visit_stmt(s);
                    }
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    for s in &mut else_block.statements {
                        changed |= self.visit_stmt(s);
                    }
                }
            }
            Statement::While(while_stmt) => {
                for s in &mut while_stmt.body.statements {
                    changed |= self.visit_stmt(s);
                }
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Numeric(for_num) => {
                    for s in &mut for_num.body.statements {
                        changed |= self.visit_stmt(s);
                    }
                }
                ForStatement::Generic(for_gen) => {
                    for s in &mut for_gen.body.statements {
                        changed |= self.visit_stmt(s);
                    }
                }
            },
            Statement::Repeat(repeat_stmt) => {
                for s in &mut repeat_stmt.body.statements {
                    changed |= self.visit_stmt(s);
                }
            }
            Statement::Block(block) => {
                for s in &mut block.statements {
                    changed |= self.visit_stmt(s);
                }
            }
            _ => {}
        }

        changed
    }
}

/// Composite pass that runs multiple expression visitors with pre-analysis
pub struct AnalysisCompositePass {
    pre_analyzers: Vec<Box<dyn PreAnalysisPass>>,
    visitors: Vec<Box<dyn StmtVisitor>>,
    #[allow(dead_code)]
    name: &'static str,
}

impl AnalysisCompositePass {
    pub fn new(name: &'static str) -> Self {
        Self {
            pre_analyzers: Vec::new(),
            visitors: Vec::new(),
            name,
        }
    }

    pub fn add_pre_analyzer(&mut self, analyzer: Box<dyn PreAnalysisPass>) {
        self.pre_analyzers.push(analyzer);
    }

    pub fn add_visitor(&mut self, visitor: Box<dyn StmtVisitor>) {
        self.visitors.push(visitor);
    }

    pub fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        // Run pre-analysis phase first
        for analyzer in &mut self.pre_analyzers {
            analyzer.analyze(program);
        }

        // Run transformation phase
        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.visit_stmt(stmt);
        }
        Ok(changed)
    }

    fn visit_stmt(&mut self, stmt: &mut Statement) -> bool {
        let mut changed = false;

        // Visit children first
        changed |= self.visit_stmt_children(stmt);

        // Then apply all visitors to this statement
        for visitor in &mut self.visitors {
            changed |= visitor.visit_stmt(stmt);
        }

        changed
    }

    fn visit_stmt_children(&mut self, stmt: &mut Statement) -> bool {
        let mut changed = false;

        match stmt {
            Statement::Function(func) => {
                for s in &mut func.body.statements {
                    changed |= self.visit_stmt(s);
                }
            }
            Statement::If(if_stmt) => {
                for s in &mut if_stmt.then_block.statements {
                    changed |= self.visit_stmt(s);
                }
                for else_if in &mut if_stmt.else_ifs {
                    for s in &mut else_if.block.statements {
                        changed |= self.visit_stmt(s);
                    }
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    for s in &mut else_block.statements {
                        changed |= self.visit_stmt(s);
                    }
                }
            }
            Statement::While(while_stmt) => {
                for s in &mut while_stmt.body.statements {
                    changed |= self.visit_stmt(s);
                }
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Numeric(for_num) => {
                    for s in &mut for_num.body.statements {
                        changed |= self.visit_stmt(s);
                    }
                }
                ForStatement::Generic(for_gen) => {
                    for s in &mut for_gen.body.statements {
                        changed |= self.visit_stmt(s);
                    }
                }
            },
            Statement::Repeat(repeat_stmt) => {
                for s in &mut repeat_stmt.body.statements {
                    changed |= self.visit_stmt(s);
                }
            }
            Statement::Block(block) => {
                for s in &mut block.statements {
                    changed |= self.visit_stmt(s);
                }
            }
            _ => {}
        }

        changed
    }
}

// =============================================================================
// Optimizer - Orchestrates all passes
// =============================================================================

/// Optimizer for AST transformations
///
/// This struct manages optimization passes and runs them until a fixed point
/// is reached (no more changes). Passes are organized into composite groups
/// to minimize AST traversals.
pub struct Optimizer {
    level: OptimizationLevel,
    #[allow(dead_code)]
    handler: Arc<dyn DiagnosticHandler>,
    interner: Rc<StringInterner>,

    // Composite passes (merged traversals)
    expr_pass: Option<ExpressionCompositePass>,
    elim_pass: Option<StatementCompositePass>,
    func_pass: Option<AnalysisCompositePass>,
    data_pass: Option<ExpressionCompositePass>,

    // Standalone passes (whole-program analysis)
    standalone_passes: Vec<Box<dyn WholeProgramPass>>,
}

impl Optimizer {
    /// Create a new optimizer with the given optimization level
    pub fn new(
        level: OptimizationLevel,
        handler: Arc<dyn DiagnosticHandler>,
        interner: Rc<StringInterner>,
    ) -> Self {
        let mut optimizer = Self {
            level,
            handler,
            interner,
            expr_pass: None,
            elim_pass: None,
            func_pass: None,
            data_pass: None,
            standalone_passes: Vec::new(),
        };

        // Register optimization passes based on level
        optimizer.register_passes();
        optimizer
    }

    /// Register optimization passes based on the optimization level
    fn register_passes(&mut self) {
        let interner = self.interner.clone();
        let level = self.level;

        // O1 passes - Expression transformations
        if level >= OptimizationLevel::O1 {
            let mut expr_pass = ExpressionCompositePass::new("expression-transforms");
            expr_pass.add_visitor(Box::new(ConstantFoldingPass::new()));
            expr_pass.add_visitor(Box::new(AlgebraicSimplificationPass::new()));
            self.expr_pass = Some(expr_pass);
        }

        // O2 passes - Elimination and data structure transforms
        if level >= OptimizationLevel::O2 {
            // Elimination group
            let mut elim_pass = StatementCompositePass::new("elimination-transforms");
            elim_pass.add_visitor(Box::new(DeadCodeEliminationPass::new()));
            elim_pass.add_visitor(Box::new(DeadStoreEliminationPass::new()));
            self.elim_pass = Some(elim_pass);

            // Data structure group
            let mut data_pass = ExpressionCompositePass::new("data-structure-transforms");
            data_pass.add_visitor(Box::new(TablePreallocationPass::new()));
            data_pass.add_visitor(Box::new(StringConcatOptimizationPass::new(
                interner.clone(),
            )));
            self.data_pass = Some(data_pass);

            // Function group (requires pre-analysis)
            let mut func_pass = AnalysisCompositePass::new("function-transforms");
            func_pass.add_pre_analyzer(Box::new(FunctionInliningPass::new(interner.clone())));
            func_pass.add_visitor(Box::new(FunctionInliningPass::new(interner.clone())));
            func_pass.add_visitor(Box::new(TailCallOptimizationPass::new()));
            func_pass.add_visitor(Box::new(MethodToFunctionConversionPass::new(
                interner.clone(),
            )));
            self.func_pass = Some(func_pass);

            // Standalone passes
            self.standalone_passes
                .push(Box::new(LoopOptimizationPass::new()));
            self.standalone_passes
                .push(Box::new(RichEnumOptimizationPass::new()));
        }

        // O3 passes - Aggressive optimizations
        if level >= OptimizationLevel::O3 {
            // Add O3 visitors to existing composite passes
            if let Some(ref mut expr_pass) = self.expr_pass {
                expr_pass.add_visitor(Box::new(OperatorInliningPass::new(interner.clone())));
            }

            if let Some(ref mut func_pass) = self.func_pass {
                func_pass.add_visitor(Box::new(AggressiveInliningPass::new(interner.clone())));
                func_pass.add_visitor(Box::new(InterfaceMethodInliningPass::new(interner.clone())));
            }

            // O3 standalone passes
            self.standalone_passes
                .push(Box::new(DevirtualizationPass::new(interner.clone())));
            self.standalone_passes
                .push(Box::new(GenericSpecializationPass::new(interner.clone())));
        }

        // Global localization runs at all optimization levels (but needs whole-program context)
        self.standalone_passes
            .push(Box::new(GlobalLocalizationPass::new(interner.clone())));
    }

    /// Returns the number of registered passes
    pub fn pass_count(&self) -> usize {
        let mut count = 0;

        // Count composite passes
        if self.expr_pass.is_some() {
            count += 1;
        }
        if self.elim_pass.is_some() {
            count += 1;
        }
        if self.func_pass.is_some() {
            count += 1;
        }
        if self.data_pass.is_some() {
            count += 1;
        }

        // Count standalone passes
        count += self.standalone_passes.len();

        count
    }

    /// Returns the names of all registered passes
    pub fn pass_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();

        if self.expr_pass.is_some() {
            names.push("expression-transforms");
        }
        if self.elim_pass.is_some() {
            names.push("elimination-transforms");
        }
        if self.func_pass.is_some() {
            names.push("function-transforms");
        }
        if self.data_pass.is_some() {
            names.push("data-structure-transforms");
        }

        for pass in &self.standalone_passes {
            names.push(pass.name());
        }

        names
    }

    /// Optimize the program AST
    /// Runs all registered optimization passes until no more changes are made
    pub fn optimize(&mut self, program: &mut Program) -> Result<(), String> {
        // Resolve Auto to actual optimization level based on build profile
        let effective_level = self.level.effective();

        if effective_level == OptimizationLevel::O0 {
            // No optimizations at O0
            return Ok(());
        }

        // Run passes in a loop until no changes are made (fixed-point iteration)
        let mut iteration = 0;
        let max_iterations = 10; // Prevent infinite loops

        loop {
            let mut changed = false;
            iteration += 1;

            if iteration > max_iterations {
                // Safety limit reached - stop optimizing
                break;
            }

            // Run composite expression pass
            if let Some(ref mut pass) = self.expr_pass {
                if effective_level >= OptimizationLevel::O1 {
                    changed |= pass.run(program)?;
                }
            }

            // Run elimination composite pass
            if let Some(ref mut pass) = self.elim_pass {
                if effective_level >= OptimizationLevel::O2 {
                    changed |= pass.run(program)?;
                }
            }

            // Run function composite pass
            if let Some(ref mut pass) = self.func_pass {
                if effective_level >= OptimizationLevel::O2 {
                    changed |= pass.run(program)?;
                }
            }

            // Run data structure composite pass
            if let Some(ref mut pass) = self.data_pass {
                if effective_level >= OptimizationLevel::O2 {
                    changed |= pass.run(program)?;
                }
            }

            // Run standalone passes
            for pass in &mut self.standalone_passes {
                if pass.min_level() <= effective_level {
                    changed |= pass.run(program)?;
                }
            }

            // If no pass made changes, we've reached a fixed point
            if !changed {
                break;
            }
        }

        Ok(())
    }
}
