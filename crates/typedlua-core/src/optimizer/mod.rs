//! AST optimizer for TypedLua
//!
//! Optimization passes work on `MutableProgram<'arena>` which provides a mutable
//! `Vec<Statement<'arena>>` at the top level. Sub-expressions within statements use
//! immutable `&'arena` references, so passes use a clone-and-rebuild pattern:
//! clone sub-expressions to owned values, mutate, then allocate back into the arena.

use crate::config::OptimizationLevel;
use crate::diagnostics::DiagnosticHandler;
use crate::MutableProgram;

use bumpalo::Bump;
use std::sync::Arc;
use tracing::{debug, info};
use typedlua_parser::ast::expression::{Expression, ExpressionKind};
use typedlua_parser::ast::statement::{Block, ForStatement, Statement};
use typedlua_parser::string_interner::StringInterner;

use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct AstFeatures: u32 {
        const HAS_LOOPS = 0b00000001;
        const HAS_CLASSES = 0b00000010;
        const HAS_METHODS = 0b00000100;
        const HAS_FUNCTIONS = 0b00001000;
        const HAS_ARROWS = 0b00010000;
        const HAS_INTERFACES = 0b00100000;
        const HAS_ARRAYS = 0b01000000;
        const HAS_OBJECTS = 0b10000000;
        const HAS_ENUMS = 0b100000000;
        const EMPTY = 0b00000000;
    }
}

pub struct AstFeatureDetector {
    features: AstFeatures,
}

impl AstFeatureDetector {
    pub fn new() -> Self {
        Self {
            features: AstFeatures::EMPTY,
        }
    }

    pub fn detect(program: &MutableProgram<'_>) -> AstFeatures {
        let mut detector = Self::new();
        for stmt in &program.statements {
            detector.visit_statement(stmt);
        }
        detector.features
    }

    fn visit_statement(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::For(for_stmt) => {
                self.features |= AstFeatures::HAS_LOOPS;
                match &**for_stmt {
                    ForStatement::Numeric(for_num) => {
                        self.visit_expression(&for_num.start);
                        self.visit_expression(&for_num.end);
                        if let Some(step) = &for_num.step {
                            self.visit_expression(step);
                        }
                        for stmt in for_num.body.statements.iter() {
                            self.visit_statement(stmt);
                        }
                    }
                    ForStatement::Generic(for_gen) => {
                        for expr in for_gen.iterators.iter() {
                            self.visit_expression(expr);
                        }
                        for stmt in for_gen.body.statements.iter() {
                            self.visit_statement(stmt);
                        }
                    }
                }
            }
            Statement::While(while_stmt) => {
                self.features |= AstFeatures::HAS_LOOPS;
                self.visit_expression(&while_stmt.condition);
                for stmt in while_stmt.body.statements.iter() {
                    self.visit_statement(stmt);
                }
            }
            Statement::Repeat(repeat_stmt) => {
                self.features |= AstFeatures::HAS_LOOPS;
                self.visit_expression(&repeat_stmt.until);
                for stmt in repeat_stmt.body.statements.iter() {
                    self.visit_statement(stmt);
                }
            }
            Statement::Class(_) => {
                self.features |= AstFeatures::HAS_CLASSES;
            }
            Statement::Interface(_) => {
                self.features |= AstFeatures::HAS_INTERFACES;
            }
            Statement::Enum(_) => {
                self.features |= AstFeatures::HAS_ENUMS;
            }
            Statement::Function(func) => {
                self.features |= AstFeatures::HAS_FUNCTIONS;
                for stmt in func.body.statements.iter() {
                    self.visit_statement(stmt);
                }
            }
            Statement::Expression(expr) => {
                self.visit_expression(expr);
            }
            Statement::Block(block) => {
                for stmt in block.statements.iter() {
                    self.visit_statement(stmt);
                }
            }
            Statement::If(if_stmt) => {
                self.visit_expression(&if_stmt.condition);
                for stmt in if_stmt.then_block.statements.iter() {
                    self.visit_statement(stmt);
                }
                for else_if in if_stmt.else_ifs.iter() {
                    self.visit_expression(&else_if.condition);
                    for stmt in else_if.block.statements.iter() {
                        self.visit_statement(stmt);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for stmt in else_block.statements.iter() {
                        self.visit_statement(stmt);
                    }
                }
            }
            Statement::Return(ret) => {
                for expr in ret.values.iter() {
                    self.visit_expression(expr);
                }
            }
            Statement::Variable(var) => {
                self.visit_expression(&var.initializer);
            }
            _ => {}
        }
    }

    fn visit_expression(&mut self, expr: &Expression<'_>) {
        match &expr.kind {
            ExpressionKind::Arrow(_) => {
                self.features |= AstFeatures::HAS_ARROWS;
            }
            ExpressionKind::Object(_) => {
                self.features |= AstFeatures::HAS_OBJECTS;
            }
            ExpressionKind::Array(_) => {
                self.features |= AstFeatures::HAS_ARRAYS;
            }
            _ => {}
        }
    }
}

// Pass modules — enabled incrementally during arena migration
mod passes;
use passes::*;

mod rich_enum_optimization;
use rich_enum_optimization::RichEnumOptimizationPass;

mod method_to_function_conversion;
use method_to_function_conversion::MethodToFunctionConversionPass;

mod devirtualization;
pub use devirtualization::ClassHierarchy;
use devirtualization::DevirtualizationPass;

mod whole_program_analysis;
pub use whole_program_analysis::WholeProgramAnalysis;

mod operator_inlining;
use operator_inlining::OperatorInliningPass;

mod interface_inlining;
use interface_inlining::InterfaceMethodInliningPass;

mod aggressive_inlining;
use aggressive_inlining::AggressiveInliningPass;

// =============================================================================
// Visitor Traits - Core of the pass merging architecture
// =============================================================================

/// Visitor for expression-level transformations.
///
/// Passes use the clone-and-rebuild pattern for sub-expression mutation:
/// clone `&'arena` sub-expressions to owned, mutate, allocate back via arena.
pub trait ExprVisitor<'arena> {
    fn visit_expr(&mut self, expr: &mut Expression<'arena>, arena: &'arena Bump) -> bool;

    fn required_features(&self) -> AstFeatures {
        AstFeatures::EMPTY
    }
}

/// Visitor for statement-level transformations.
pub trait StmtVisitor<'arena> {
    fn visit_stmt(&mut self, stmt: &mut Statement<'arena>, arena: &'arena Bump) -> bool;

    fn required_features(&self) -> AstFeatures {
        AstFeatures::EMPTY
    }
}

/// Pass that requires pre-analysis before transformation.
pub trait PreAnalysisPass<'arena> {
    fn analyze(&mut self, program: &MutableProgram<'arena>);

    fn required_features(&self) -> AstFeatures {
        AstFeatures::EMPTY
    }
}

/// Pass that operates on the whole program.
pub trait WholeProgramPass<'arena> {
    fn name(&self) -> &'static str;

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    fn required_features(&self) -> AstFeatures {
        AstFeatures::EMPTY
    }

    fn run(
        &mut self,
        program: &mut MutableProgram<'arena>,
        arena: &'arena Bump,
    ) -> Result<bool, String>;

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

// =============================================================================
// Composite Passes - Merge multiple visitors into single traversals
// =============================================================================

/// Composite pass that runs multiple expression visitors in one traversal.
///
/// Uses clone-and-rebuild for sub-expression mutation: inner `&'arena` references
/// are cloned to owned values, mutated, then allocated back into the arena.
pub struct ExpressionCompositePass<'arena> {
    visitors: Vec<Box<dyn ExprVisitor<'arena> + 'arena>>,
    _name: &'static str,
}

impl<'arena> ExpressionCompositePass<'arena> {
    pub fn new(name: &'static str) -> Self {
        Self {
            visitors: Vec::new(),
            _name: name,
        }
    }

    pub fn add_visitor(&mut self, visitor: Box<dyn ExprVisitor<'arena> + 'arena>) {
        self.visitors.push(visitor);
    }

    pub fn visitor_count(&self) -> usize {
        self.visitors.len()
    }

    pub fn required_features(&self) -> AstFeatures {
        let mut combined = AstFeatures::EMPTY;
        for visitor in &self.visitors {
            combined |= visitor.required_features();
        }
        combined
    }

    pub fn run(
        &mut self,
        program: &mut MutableProgram<'arena>,
        arena: &'arena Bump,
    ) -> Result<bool, String> {
        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.visit_stmt(stmt, arena);
        }
        Ok(changed)
    }

    fn visit_stmt(&mut self, stmt: &mut Statement<'arena>, arena: &'arena Bump) -> bool {
        let mut changed = false;

        match stmt {
            Statement::Variable(decl) => {
                changed |= self.visit_expr(&mut decl.initializer, arena);
            }
            Statement::Expression(expr) => {
                changed |= self.visit_expr(expr, arena);
            }
            Statement::If(if_stmt) => {
                changed |= self.visit_expr(&mut if_stmt.condition, arena);
                changed |= self.visit_block(&mut if_stmt.then_block, arena);
                let mut new_else_ifs: Vec<_> = if_stmt.else_ifs.to_vec();
                let mut eic = false;
                for else_if in &mut new_else_ifs {
                    eic |= self.visit_expr(&mut else_if.condition, arena);
                    eic |= self.visit_block(&mut else_if.block, arena);
                }
                if eic {
                    if_stmt.else_ifs = arena.alloc_slice_clone(&new_else_ifs);
                    changed = true;
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.visit_block(else_block, arena);
                }
            }
            Statement::While(while_stmt) => {
                changed |= self.visit_expr(&mut while_stmt.condition, arena);
                changed |= self.visit_block(&mut while_stmt.body, arena);
            }
            Statement::For(for_stmt) => {
                match &**for_stmt {
                    ForStatement::Numeric(for_num_ref) => {
                        let mut new_num = (**for_num_ref).clone();
                        let mut fc = false;
                        fc |= self.visit_expr(&mut new_num.start, arena);
                        fc |= self.visit_expr(&mut new_num.end, arena);
                        if let Some(step) = &mut new_num.step {
                            fc |= self.visit_expr(step, arena);
                        }
                        fc |= self.visit_block(&mut new_num.body, arena);
                        if fc {
                            *stmt = Statement::For(arena.alloc(ForStatement::Numeric(arena.alloc(new_num))));
                            changed = true;
                        }
                    }
                    ForStatement::Generic(for_gen_ref) => {
                        let mut new_gen = for_gen_ref.clone();
                        let mut fc = false;
                        let mut new_iters: Vec<_> = new_gen.iterators.to_vec();
                        for expr in &mut new_iters {
                            fc |= self.visit_expr(expr, arena);
                        }
                        if fc {
                            new_gen.iterators = arena.alloc_slice_clone(&new_iters);
                        }
                        fc |= self.visit_block(&mut new_gen.body, arena);
                        if fc {
                            *stmt = Statement::For(arena.alloc(ForStatement::Generic(new_gen)));
                            changed = true;
                        }
                    }
                }
            }
            Statement::Repeat(repeat_stmt) => {
                changed |= self.visit_expr(&mut repeat_stmt.until, arena);
                changed |= self.visit_block(&mut repeat_stmt.body, arena);
            }
            Statement::Return(ret_stmt) => {
                let mut values: Vec<Expression<'arena>> = ret_stmt.values.to_vec();
                let mut ret_changed = false;
                for expr in &mut values {
                    ret_changed |= self.visit_expr(expr, arena);
                }
                if ret_changed {
                    ret_stmt.values = arena.alloc_slice_clone(&values);
                    changed = true;
                }
            }
            Statement::Function(func) => {
                changed |= self.visit_block(&mut func.body, arena);
            }
            Statement::Block(block) => {
                changed |= self.visit_block(block, arena);
            }
            _ => {}
        }

        changed
    }

    fn visit_block(&mut self, block: &mut Block<'arena>, arena: &'arena Bump) -> bool {
        let mut stmts: Vec<Statement<'arena>> = block.statements.to_vec();
        let mut changed = false;
        for stmt in &mut stmts {
            changed |= self.visit_stmt(stmt, arena);
        }
        if changed {
            block.statements = arena.alloc_slice_clone(&stmts);
        }
        changed
    }

    fn visit_expr(&mut self, expr: &mut Expression<'arena>, arena: &'arena Bump) -> bool {
        let mut changed = false;

        // First, visit children recursively (clone-and-rebuild for &'arena refs)
        changed |= visit_expr_children(expr, arena, &mut |e, a| self.visit_expr(e, a));

        // Then, apply all visitors to this expression
        for visitor in &mut self.visitors {
            changed |= visitor.visit_expr(expr, arena);
        }

        changed
    }
}

/// Composite pass that runs multiple statement visitors in one traversal.
pub struct StatementCompositePass<'arena> {
    visitors: Vec<Box<dyn StmtVisitor<'arena> + 'arena>>,
    _name: &'static str,
}

impl<'arena> StatementCompositePass<'arena> {
    pub fn new(name: &'static str) -> Self {
        Self {
            visitors: Vec::new(),
            _name: name,
        }
    }

    pub fn add_visitor(&mut self, visitor: Box<dyn StmtVisitor<'arena> + 'arena>) {
        self.visitors.push(visitor);
    }

    pub fn visitor_count(&self) -> usize {
        self.visitors.len()
    }

    pub fn required_features(&self) -> AstFeatures {
        let mut combined = AstFeatures::EMPTY;
        for visitor in &self.visitors {
            combined |= visitor.required_features();
        }
        combined
    }

    pub fn run(
        &mut self,
        program: &mut MutableProgram<'arena>,
        arena: &'arena Bump,
    ) -> Result<bool, String> {
        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.visit_stmt(stmt, arena);
        }
        Ok(changed)
    }

    fn visit_stmt(&mut self, stmt: &mut Statement<'arena>, arena: &'arena Bump) -> bool {
        let mut changed = false;

        changed |= self.visit_stmt_children(stmt, arena);

        for visitor in &mut self.visitors {
            changed |= visitor.visit_stmt(stmt, arena);
        }

        changed
    }

    fn visit_stmt_children(&mut self, stmt: &mut Statement<'arena>, arena: &'arena Bump) -> bool {
        let mut changed = false;

        match stmt {
            Statement::Function(func) => {
                changed |= self.visit_block(&mut func.body, arena);
            }
            Statement::If(if_stmt) => {
                changed |= self.visit_block(&mut if_stmt.then_block, arena);
                let mut new_else_ifs: Vec<_> = if_stmt.else_ifs.to_vec();
                let mut eic = false;
                for else_if in &mut new_else_ifs {
                    eic |= self.visit_block(&mut else_if.block, arena);
                }
                if eic {
                    if_stmt.else_ifs = arena.alloc_slice_clone(&new_else_ifs);
                    changed = true;
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.visit_block(else_block, arena);
                }
            }
            Statement::While(while_stmt) => {
                changed |= self.visit_block(&mut while_stmt.body, arena);
            }
            Statement::For(for_stmt) => {
                match &**for_stmt {
                    ForStatement::Numeric(for_num_ref) => {
                        let mut new_num = (**for_num_ref).clone();
                        let fc = self.visit_block(&mut new_num.body, arena);
                        if fc {
                            *stmt = Statement::For(arena.alloc(ForStatement::Numeric(arena.alloc(new_num))));
                            changed = true;
                        }
                    }
                    ForStatement::Generic(for_gen_ref) => {
                        let mut new_gen = for_gen_ref.clone();
                        let fc = self.visit_block(&mut new_gen.body, arena);
                        if fc {
                            *stmt = Statement::For(arena.alloc(ForStatement::Generic(new_gen)));
                            changed = true;
                        }
                    }
                }
            }
            Statement::Repeat(repeat_stmt) => {
                changed |= self.visit_block(&mut repeat_stmt.body, arena);
            }
            Statement::Block(block) => {
                changed |= self.visit_block(block, arena);
            }
            _ => {}
        }

        changed
    }

    fn visit_block(&mut self, block: &mut Block<'arena>, arena: &'arena Bump) -> bool {
        let mut stmts: Vec<Statement<'arena>> = block.statements.to_vec();
        let mut changed = false;
        for stmt in &mut stmts {
            changed |= self.visit_stmt(stmt, arena);
        }
        if changed {
            block.statements = arena.alloc_slice_clone(&stmts);
        }
        changed
    }
}

/// Composite pass that runs multiple visitors with pre-analysis.
pub struct AnalysisCompositePass<'arena> {
    pre_analyzers: Vec<Box<dyn PreAnalysisPass<'arena> + 'arena>>,
    visitors: Vec<Box<dyn StmtVisitor<'arena> + 'arena>>,
    _name: &'static str,
}

impl<'arena> AnalysisCompositePass<'arena> {
    pub fn new(name: &'static str) -> Self {
        Self {
            pre_analyzers: Vec::new(),
            visitors: Vec::new(),
            _name: name,
        }
    }

    pub fn add_pre_analyzer(&mut self, analyzer: Box<dyn PreAnalysisPass<'arena> + 'arena>) {
        self.pre_analyzers.push(analyzer);
    }

    pub fn add_visitor(&mut self, visitor: Box<dyn StmtVisitor<'arena> + 'arena>) {
        self.visitors.push(visitor);
    }

    pub fn visitor_count(&self) -> usize {
        self.visitors.len()
    }

    pub fn required_features(&self) -> AstFeatures {
        let mut combined = AstFeatures::EMPTY;
        for analyzer in &self.pre_analyzers {
            combined |= analyzer.required_features();
        }
        for visitor in &self.visitors {
            combined |= visitor.required_features();
        }
        combined
    }

    pub fn run(
        &mut self,
        program: &mut MutableProgram<'arena>,
        arena: &'arena Bump,
    ) -> Result<bool, String> {
        for analyzer in &mut self.pre_analyzers {
            analyzer.analyze(program);
        }

        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.visit_stmt(stmt, arena);
        }
        Ok(changed)
    }

    fn visit_stmt(&mut self, stmt: &mut Statement<'arena>, arena: &'arena Bump) -> bool {
        let mut changed = false;

        changed |= self.visit_stmt_children(stmt, arena);

        for visitor in &mut self.visitors {
            changed |= visitor.visit_stmt(stmt, arena);
        }

        changed
    }

    fn visit_stmt_children(&mut self, stmt: &mut Statement<'arena>, arena: &'arena Bump) -> bool {
        let mut changed = false;

        match stmt {
            Statement::Function(func) => {
                changed |= self.visit_block(&mut func.body, arena);
            }
            Statement::If(if_stmt) => {
                changed |= self.visit_block(&mut if_stmt.then_block, arena);
                let mut new_else_ifs: Vec<_> = if_stmt.else_ifs.to_vec();
                let mut eic = false;
                for else_if in &mut new_else_ifs {
                    eic |= self.visit_block(&mut else_if.block, arena);
                }
                if eic {
                    if_stmt.else_ifs = arena.alloc_slice_clone(&new_else_ifs);
                    changed = true;
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.visit_block(else_block, arena);
                }
            }
            Statement::While(while_stmt) => {
                changed |= self.visit_block(&mut while_stmt.body, arena);
            }
            Statement::For(for_stmt) => {
                match &**for_stmt {
                    ForStatement::Numeric(for_num_ref) => {
                        let mut new_num = (**for_num_ref).clone();
                        let fc = self.visit_block(&mut new_num.body, arena);
                        if fc {
                            *stmt = Statement::For(arena.alloc(ForStatement::Numeric(arena.alloc(new_num))));
                            changed = true;
                        }
                    }
                    ForStatement::Generic(for_gen_ref) => {
                        let mut new_gen = for_gen_ref.clone();
                        let fc = self.visit_block(&mut new_gen.body, arena);
                        if fc {
                            *stmt = Statement::For(arena.alloc(ForStatement::Generic(new_gen)));
                            changed = true;
                        }
                    }
                }
            }
            Statement::Repeat(repeat_stmt) => {
                changed |= self.visit_block(&mut repeat_stmt.body, arena);
            }
            Statement::Block(block) => {
                changed |= self.visit_block(block, arena);
            }
            _ => {}
        }

        changed
    }

    fn visit_block(&mut self, block: &mut Block<'arena>, arena: &'arena Bump) -> bool {
        let mut stmts: Vec<Statement<'arena>> = block.statements.to_vec();
        let mut changed = false;
        for stmt in &mut stmts {
            changed |= self.visit_stmt(stmt, arena);
        }
        if changed {
            block.statements = arena.alloc_slice_clone(&stmts);
        }
        changed
    }
}

// =============================================================================
// Shared expression child visitor — clone-and-rebuild pattern
// =============================================================================

/// Visit children of an expression using the clone-and-rebuild pattern.
/// The `visit_fn` closure handles recursive visitation.
fn visit_expr_children<'arena>(
    expr: &mut Expression<'arena>,
    arena: &'arena Bump,
    visit_fn: &mut dyn FnMut(&mut Expression<'arena>, &'arena Bump) -> bool,
) -> bool {
    use typedlua_parser::ast::expression::{ArrayElement, ObjectProperty};

    let mut changed = false;

    match &expr.kind {
        ExpressionKind::Binary(op, left, right) => {
            let op = *op;
            let mut new_left = (**left).clone();
            let mut new_right = (**right).clone();
            let lc = visit_fn(&mut new_left, arena);
            let rc = visit_fn(&mut new_right, arena);
            if lc || rc {
                expr.kind =
                    ExpressionKind::Binary(op, arena.alloc(new_left), arena.alloc(new_right));
                changed = true;
            }
        }
        ExpressionKind::Unary(op, operand) => {
            let op = *op;
            let mut new_operand = (**operand).clone();
            if visit_fn(&mut new_operand, arena) {
                expr.kind = ExpressionKind::Unary(op, arena.alloc(new_operand));
                changed = true;
            }
        }
        ExpressionKind::Call(func, args, type_args) => {
            let type_args = *type_args;
            let mut new_func = (**func).clone();
            let fc = visit_fn(&mut new_func, arena);
            let mut new_args: Vec<_> = args.to_vec();
            let mut ac = false;
            for arg in &mut new_args {
                ac |= visit_fn(&mut arg.value, arena);
            }
            if fc || ac {
                expr.kind = ExpressionKind::Call(
                    arena.alloc(new_func),
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
                changed = true;
            }
        }
        ExpressionKind::MethodCall(obj, method, args, type_args) => {
            let method = method.clone();
            let type_args = *type_args;
            let mut new_obj = (**obj).clone();
            let oc = visit_fn(&mut new_obj, arena);
            let mut new_args: Vec<_> = args.to_vec();
            let mut ac = false;
            for arg in &mut new_args {
                ac |= visit_fn(&mut arg.value, arena);
            }
            if oc || ac {
                expr.kind = ExpressionKind::MethodCall(
                    arena.alloc(new_obj),
                    method,
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
                changed = true;
            }
        }
        ExpressionKind::Member(obj, member) => {
            let member = member.clone();
            let mut new_obj = (**obj).clone();
            if visit_fn(&mut new_obj, arena) {
                expr.kind = ExpressionKind::Member(arena.alloc(new_obj), member);
                changed = true;
            }
        }
        ExpressionKind::Index(obj, index) => {
            let mut new_obj = (**obj).clone();
            let mut new_index = (**index).clone();
            let oc = visit_fn(&mut new_obj, arena);
            let ic = visit_fn(&mut new_index, arena);
            if oc || ic {
                expr.kind = ExpressionKind::Index(arena.alloc(new_obj), arena.alloc(new_index));
                changed = true;
            }
        }
        ExpressionKind::Conditional(cond, then_expr, else_expr) => {
            let mut new_cond = (**cond).clone();
            let mut new_then = (**then_expr).clone();
            let mut new_else = (**else_expr).clone();
            let cc = visit_fn(&mut new_cond, arena);
            let tc = visit_fn(&mut new_then, arena);
            let ec = visit_fn(&mut new_else, arena);
            if cc || tc || ec {
                expr.kind = ExpressionKind::Conditional(
                    arena.alloc(new_cond),
                    arena.alloc(new_then),
                    arena.alloc(new_else),
                );
                changed = true;
            }
        }
        ExpressionKind::Array(elements) => {
            let mut new_elements: Vec<_> = elements.to_vec();
            let mut ec = false;
            for elem in &mut new_elements {
                match elem {
                    ArrayElement::Expression(e) => ec |= visit_fn(e, arena),
                    ArrayElement::Spread(e) => ec |= visit_fn(e, arena),
                }
            }
            if ec {
                expr.kind = ExpressionKind::Array(arena.alloc_slice_clone(&new_elements));
                changed = true;
            }
        }
        ExpressionKind::Object(props) => {
            let mut new_props: Vec<_> = props.to_vec();
            let mut pc = false;
            for prop in &mut new_props {
                match prop {
                    ObjectProperty::Property { key, value, span } => {
                        let mut new_val = (**value).clone();
                        if visit_fn(&mut new_val, arena) {
                            *prop = ObjectProperty::Property { key: key.clone(), value: arena.alloc(new_val), span: *span };
                            pc = true;
                        }
                    }
                    ObjectProperty::Computed { key, value, span } => {
                        let mut new_key = (**key).clone();
                        let mut new_val = (**value).clone();
                        let kc = visit_fn(&mut new_key, arena);
                        let vc = visit_fn(&mut new_val, arena);
                        if kc || vc {
                            *prop = ObjectProperty::Computed {
                                key: arena.alloc(new_key),
                                value: arena.alloc(new_val),
                                span: *span,
                            };
                            pc = true;
                        }
                    }
                    ObjectProperty::Spread { value, span } => {
                        let mut new_val = (**value).clone();
                        if visit_fn(&mut new_val, arena) {
                            *prop = ObjectProperty::Spread { value: arena.alloc(new_val), span: *span };
                            pc = true;
                        }
                    }
                }
            }
            if pc {
                expr.kind = ExpressionKind::Object(arena.alloc_slice_clone(&new_props));
                changed = true;
            }
        }
        ExpressionKind::Pipe(left, right) => {
            let mut new_left = (**left).clone();
            let mut new_right = (**right).clone();
            let lc = visit_fn(&mut new_left, arena);
            let rc = visit_fn(&mut new_right, arena);
            if lc || rc {
                expr.kind =
                    ExpressionKind::Pipe(arena.alloc(new_left), arena.alloc(new_right));
                changed = true;
            }
        }
        ExpressionKind::ErrorChain(left, right) => {
            let mut new_left = (**left).clone();
            let mut new_right = (**right).clone();
            let lc = visit_fn(&mut new_left, arena);
            let rc = visit_fn(&mut new_right, arena);
            if lc || rc {
                expr.kind =
                    ExpressionKind::ErrorChain(arena.alloc(new_left), arena.alloc(new_right));
                changed = true;
            }
        }
        ExpressionKind::New(callee, args, type_args) => {
            let type_args = *type_args;
            let mut new_callee = (**callee).clone();
            let cc = visit_fn(&mut new_callee, arena);
            let mut new_args: Vec<_> = args.to_vec();
            let mut ac = false;
            for arg in &mut new_args {
                ac |= visit_fn(&mut arg.value, arena);
            }
            if cc || ac {
                expr.kind = ExpressionKind::New(
                    arena.alloc(new_callee),
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
                changed = true;
            }
        }
        ExpressionKind::OptionalMember(obj, member) => {
            let member = member.clone();
            let mut new_obj = (**obj).clone();
            if visit_fn(&mut new_obj, arena) {
                expr.kind = ExpressionKind::OptionalMember(arena.alloc(new_obj), member);
                changed = true;
            }
        }
        ExpressionKind::OptionalIndex(obj, index) => {
            let mut new_obj = (**obj).clone();
            let mut new_index = (**index).clone();
            let oc = visit_fn(&mut new_obj, arena);
            let ic = visit_fn(&mut new_index, arena);
            if oc || ic {
                expr.kind =
                    ExpressionKind::OptionalIndex(arena.alloc(new_obj), arena.alloc(new_index));
                changed = true;
            }
        }
        ExpressionKind::OptionalCall(obj, args, type_args) => {
            let type_args = *type_args;
            let mut new_obj = (**obj).clone();
            let oc = visit_fn(&mut new_obj, arena);
            let mut new_args: Vec<_> = args.to_vec();
            let mut ac = false;
            for arg in &mut new_args {
                ac |= visit_fn(&mut arg.value, arena);
            }
            if oc || ac {
                expr.kind = ExpressionKind::OptionalCall(
                    arena.alloc(new_obj),
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
                changed = true;
            }
        }
        ExpressionKind::OptionalMethodCall(obj, method, args, type_args) => {
            let method = method.clone();
            let type_args = *type_args;
            let mut new_obj = (**obj).clone();
            let oc = visit_fn(&mut new_obj, arena);
            let mut new_args: Vec<_> = args.to_vec();
            let mut ac = false;
            for arg in &mut new_args {
                ac |= visit_fn(&mut arg.value, arena);
            }
            if oc || ac {
                expr.kind = ExpressionKind::OptionalMethodCall(
                    arena.alloc(new_obj),
                    method,
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
                changed = true;
            }
        }
        ExpressionKind::TypeAssertion(inner, ty) => {
            let ty = ty.clone();
            let mut new_inner = (**inner).clone();
            if visit_fn(&mut new_inner, arena) {
                expr.kind = ExpressionKind::TypeAssertion(arena.alloc(new_inner), ty);
                changed = true;
            }
        }
        ExpressionKind::Parenthesized(inner) => {
            let mut new_inner = (**inner).clone();
            if visit_fn(&mut new_inner, arena) {
                expr.kind = ExpressionKind::Parenthesized(arena.alloc(new_inner));
                changed = true;
            }
        }
        ExpressionKind::Assignment(target, op, value) => {
            let op = *op;
            let mut new_target = (**target).clone();
            let mut new_value = (**value).clone();
            let tc = visit_fn(&mut new_target, arena);
            let vc = visit_fn(&mut new_value, arena);
            if tc || vc {
                expr.kind = ExpressionKind::Assignment(
                    arena.alloc(new_target),
                    op,
                    arena.alloc(new_value),
                );
                changed = true;
            }
        }
        // Leaf nodes — no children to visit
        _ => {}
    }

    changed
}

// =============================================================================
// Optimizer - Orchestrates all passes
// =============================================================================

/// Optimizer for AST transformations.
///
/// Manages optimization passes and runs them until a fixed point is reached.
/// Passes are organized into composite groups to minimize AST traversals.
pub struct Optimizer<'arena> {
    level: OptimizationLevel,
    #[allow(dead_code)]
    handler: Arc<dyn DiagnosticHandler>,
    interner: Arc<StringInterner>,

    // Composite passes (merged traversals)
    expr_pass: Option<ExpressionCompositePass<'arena>>,
    elim_pass: Option<StatementCompositePass<'arena>>,
    func_pass: Option<AnalysisCompositePass<'arena>>,
    data_pass: Option<ExpressionCompositePass<'arena>>,

    // Standalone passes (whole-program analysis)
    standalone_passes: Vec<Box<dyn WholeProgramPass<'arena>>>,

    // Whole-program analysis results (for O3+ cross-module optimizations)
    whole_program_analysis: Option<WholeProgramAnalysis>,
}

impl<'arena> Optimizer<'arena> {
    /// Create a new optimizer with the given optimization level
    pub fn new(
        level: OptimizationLevel,
        handler: Arc<dyn DiagnosticHandler>,
        interner: Arc<StringInterner>,
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
            whole_program_analysis: None,
        };

        optimizer.register_passes();
        optimizer
    }

    /// Set whole-program analysis results for cross-module optimizations
    pub fn set_whole_program_analysis(&mut self, analysis: WholeProgramAnalysis) {
        self.whole_program_analysis = Some(analysis.clone());

        for pass in &mut self.standalone_passes {
            if let Some(devirt) = pass.as_any_mut().downcast_mut::<DevirtualizationPass>() {
                devirt.set_class_hierarchy((*analysis.class_hierarchy).clone());
            }
        }
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
            let mut elim_pass = StatementCompositePass::new("elimination-transforms");
            elim_pass.add_visitor(Box::new(DeadCodeEliminationPass::new()));
            elim_pass.add_visitor(Box::new(DeadStoreEliminationPass::new()));
            self.elim_pass = Some(elim_pass);

            let mut data_pass = ExpressionCompositePass::new("data-structure-transforms");
            data_pass.add_visitor(Box::new(TablePreallocationPass::new()));
            data_pass.add_visitor(Box::new(StringConcatOptimizationPass::new(
                interner.clone(),
            )));
            self.data_pass = Some(data_pass);

            let mut func_pass = AnalysisCompositePass::new("function-transforms");
            func_pass.add_pre_analyzer(Box::new(FunctionInliningPass::new(interner.clone())));
            func_pass.add_visitor(Box::new(FunctionInliningPass::new(interner.clone())));
            func_pass.add_visitor(Box::new(TailCallOptimizationPass::new()));
            func_pass.add_visitor(Box::new(MethodToFunctionConversionPass::new(
                interner.clone(),
            )));
            self.func_pass = Some(func_pass);

            self.standalone_passes
                .push(Box::new(LoopOptimizationPass::new()));
            self.standalone_passes
                .push(Box::new(RichEnumOptimizationPass::new()));
        }

        // O3 passes - Aggressive optimizations
        if level >= OptimizationLevel::O3 {
            if let Some(ref mut expr_pass) = self.expr_pass {
                expr_pass.add_visitor(Box::new(OperatorInliningPass::new(interner.clone())));
            }

            if let Some(ref mut func_pass) = self.func_pass {
                func_pass.add_visitor(Box::new(AggressiveInliningPass::new(interner.clone())));
                func_pass
                    .add_visitor(Box::new(InterfaceMethodInliningPass::new(interner.clone())));
            }

            self.standalone_passes
                .push(Box::new(DevirtualizationPass::new(interner.clone())));
            self.standalone_passes
                .push(Box::new(GenericSpecializationPass::new(interner.clone())));
        }

        // Global localization runs at all optimization levels
        self.standalone_passes
            .push(Box::new(GlobalLocalizationPass::new(interner.clone())));
    }

    /// Returns the number of registered passes (counting individual visitors within composites)
    pub fn pass_count(&self) -> usize {
        let mut count = 0;

        if let Some(ref expr_pass) = self.expr_pass {
            count += expr_pass.visitor_count();
        }
        if let Some(ref elim_pass) = self.elim_pass {
            count += elim_pass.visitor_count();
        }
        if let Some(ref func_pass) = self.func_pass {
            count += func_pass.visitor_count();
        }
        if let Some(ref data_pass) = self.data_pass {
            count += data_pass.visitor_count();
        }

        count += self.standalone_passes.len();

        count
    }

    /// Returns the names of all registered passes
    pub fn pass_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();

        // expr_pass contains: constant-folding, algebraic-simplification, [operator-inlining at O3]
        if let Some(ref ep) = self.expr_pass {
            // O1 base passes
            if ep.visitor_count() >= 1 { names.push("constant-folding"); }
            if ep.visitor_count() >= 2 { names.push("algebraic-simplification"); }
            // O3 adds operator-inlining
            if ep.visitor_count() >= 3 { names.push("operator-inlining"); }
        }

        // elim_pass contains: dead-code-elimination, dead-store-elimination
        if let Some(ref elp) = self.elim_pass {
            if elp.visitor_count() >= 1 { names.push("dead-code-elimination"); }
            if elp.visitor_count() >= 2 { names.push("dead-store-elimination"); }
        }

        // data_pass contains: table-preallocation, string-concat-optimization
        if let Some(ref dp) = self.data_pass {
            if dp.visitor_count() >= 1 { names.push("table-preallocation"); }
            if dp.visitor_count() >= 2 { names.push("string-concat-optimization"); }
        }

        // func_pass contains: function-inlining, tail-call-optimization, method-to-function-conversion
        //   [O3 adds: aggressive-inlining, interface-method-inlining]
        if let Some(ref fp) = self.func_pass {
            if fp.visitor_count() >= 1 { names.push("function-inlining"); }
            if fp.visitor_count() >= 2 { names.push("tail-call-optimization"); }
            if fp.visitor_count() >= 3 { names.push("method-to-function-conversion"); }
            if fp.visitor_count() >= 4 { names.push("aggressive-inlining"); }
            if fp.visitor_count() >= 5 { names.push("interface-method-inlining"); }
        }

        for pass in &self.standalone_passes {
            names.push(pass.name());
        }

        names
    }

    /// Optimize the program AST.
    /// Runs all registered optimization passes until no more changes are made.
    pub fn optimize(
        &mut self,
        program: &mut MutableProgram<'arena>,
        arena: &'arena Bump,
    ) -> Result<(), String> {
        use std::time::Instant;

        let effective_level = self.level.effective();

        if effective_level == OptimizationLevel::O0 {
            return Ok(());
        }

        let start_total = Instant::now();

        let features = AstFeatureDetector::detect(program);
        debug!("Detected AST features: {:?}", features);

        let mut iteration = 0;
        let max_iterations = 10;

        loop {
            let mut changed = false;
            iteration += 1;

            if iteration > max_iterations {
                break;
            }

            if let Some(ref mut pass) = self.expr_pass {
                if effective_level >= OptimizationLevel::O1 {
                    let required = pass.required_features();
                    if required.is_empty() || features.contains(required) {
                        let start = Instant::now();
                        let pass_changed = pass.run(program, arena)?;
                        let elapsed = start.elapsed();
                        debug!(
                            "  [Iter {}] ExpressionCompositePass: {:?} (changed: {})",
                            iteration, elapsed, pass_changed
                        );
                        changed |= pass_changed;
                    }
                }
            }

            if let Some(ref mut pass) = self.elim_pass {
                if effective_level >= OptimizationLevel::O2 {
                    let required = pass.required_features();
                    if required.is_empty() || features.contains(required) {
                        let start = Instant::now();
                        let pass_changed = pass.run(program, arena)?;
                        let elapsed = start.elapsed();
                        debug!(
                            "  [Iter {}] EliminationCompositePass: {:?} (changed: {})",
                            iteration, elapsed, pass_changed
                        );
                        changed |= pass_changed;
                    }
                }
            }

            if let Some(ref mut pass) = self.func_pass {
                if effective_level >= OptimizationLevel::O2 {
                    let required = pass.required_features();
                    if required.is_empty() || features.contains(required) {
                        let start = Instant::now();
                        let pass_changed = pass.run(program, arena)?;
                        let elapsed = start.elapsed();
                        debug!(
                            "  [Iter {}] FunctionCompositePass: {:?} (changed: {})",
                            iteration, elapsed, pass_changed
                        );
                        changed |= pass_changed;
                    }
                }
            }

            if let Some(ref mut pass) = self.data_pass {
                if effective_level >= OptimizationLevel::O2 {
                    let required = pass.required_features();
                    if required.is_empty() || features.contains(required) {
                        let start = Instant::now();
                        let pass_changed = pass.run(program, arena)?;
                        let elapsed = start.elapsed();
                        debug!(
                            "  [Iter {}] DataStructureCompositePass: {:?} (changed: {})",
                            iteration, elapsed, pass_changed
                        );
                        changed |= pass_changed;
                    }
                }
            }

            for pass in &mut self.standalone_passes {
                if pass.min_level() <= effective_level {
                    let required = pass.required_features();
                    if required.is_empty() || features.contains(required) {
                        let start = Instant::now();
                        let pass_changed = pass.run(program, arena)?;
                        let elapsed = start.elapsed();
                        debug!(
                            "  [Iter {}] {}: {:?} (changed: {})",
                            iteration,
                            pass.name(),
                            elapsed,
                            pass_changed
                        );
                        changed |= pass_changed;
                    }
                }
            }

            if !changed {
                break;
            }
        }

        let total_elapsed = start_total.elapsed();
        info!(
            "Optimization complete: {} iterations, {:?} total",
            iteration, total_elapsed
        );

        Ok(())
    }
}
