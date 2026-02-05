// =============================================================================
// O2: Loop Optimization Pass
// =============================================================================

use crate::config::OptimizationLevel;
use crate::optimizer::WholeProgramPass;
use std::collections::HashSet;
use typedlua_parser::ast::expression::{
    ArrayElement, BinaryOp, Expression, ExpressionKind, Literal, UnaryOp,
};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{Block, ForNumeric, ForStatement, Statement};
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::StringId;

/// Loop optimization pass
/// 1. Hoists loop-invariant local variable declarations
/// 2. Removes dead loops (while false, zero-iteration for, repeat until true)
/// 3. Handles all loop types including repeat...until
pub struct LoopOptimizationPass;

impl LoopOptimizationPass {
    pub fn new() -> Self {
        Self
    }
}

impl WholeProgramPass for LoopOptimizationPass {
    fn name(&self) -> &'static str {
        "loop-optimization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        let mut changed = false;
        let mut i = 0;

        while i < program.statements.len() {
            let (hoisted, stmt_changed) =
                self.optimize_loops_in_statement(&mut program.statements[i]);
            if !hoisted.is_empty() {
                program.statements.splice(i..i, hoisted);
                i += 1;
                changed = true;
            }
            if stmt_changed {
                changed = true;
            }
            i += 1;
        }

        Ok(changed)
    }
}

impl LoopOptimizationPass {
    fn optimize_loops_in_statement(&mut self, stmt: &mut Statement) -> (Vec<Statement>, bool) {
        match stmt {
            Statement::For(for_stmt) => self.optimize_for_loop(for_stmt),
            Statement::While(while_stmt) => self.optimize_while_loop(while_stmt),
            Statement::Repeat(repeat_stmt) => self.optimize_repeat_loop(repeat_stmt),
            Statement::Variable(_) | Statement::Expression(_) => (Vec::new(), false),
            Statement::Return(_)
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Rethrow(_)
            | Statement::Throw(_) => (Vec::new(), false),
            Statement::Block(block) => (Vec::new(), self.optimize_block(&mut block.statements)),
            Statement::Class(_)
            | Statement::Interface(_)
            | Statement::Enum(_)
            | Statement::TypeAlias(_) => (Vec::new(), false),
            Statement::Import(_) | Statement::Export(_) => (Vec::new(), false),
            Statement::Namespace(_)
            | Statement::DeclareFunction(_)
            | Statement::DeclareNamespace(_)
            | Statement::DeclareType(_)
            | Statement::DeclareInterface(_)
            | Statement::DeclareConst(_)
            | Statement::Label(_)
            | Statement::Goto(_) => (Vec::new(), false),
            Statement::Function(func) => {
                (Vec::new(), self.optimize_block(&mut func.body.statements))
            }
            Statement::If(if_stmt) => {
                let mut changed = self.optimize_block(&mut if_stmt.then_block.statements);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.optimize_block(&mut else_if.block.statements);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.optimize_block(&mut else_block.statements);
                }
                (Vec::new(), changed)
            }
            _ => (Vec::new(), false),
        }
    }

    fn optimize_for_loop(&mut self, for_stmt: &mut Box<ForStatement>) -> (Vec<Statement>, bool) {
        match &mut **for_stmt {
            ForStatement::Generic(for_gen) => {
                let modified_vars = self.collect_modified_variables(&for_gen.body);
                let (hoisted, new_body) =
                    self.hoist_invariants_simple(&for_gen.body, &modified_vars);
                for_gen.body = new_body;
                let changed =
                    !hoisted.is_empty() || self.optimize_block(&mut for_gen.body.statements);
                (hoisted, changed)
            }
            ForStatement::Numeric(for_num) => {
                if let Some((start, end, step)) = self.evaluate_numeric_bounds(for_num) {
                    if self.has_zero_iterations(start, end, step) {
                        for_num.body.statements.clear();
                        return (Vec::new(), true);
                    }
                }
                let modified_vars = self.collect_modified_variables(&for_num.body);
                let (hoisted, new_body) =
                    self.hoist_invariants_simple(&for_num.body, &modified_vars);
                for_num.body = new_body;
                let changed =
                    !hoisted.is_empty() || self.optimize_block(&mut for_num.body.statements);
                (hoisted, changed)
            }
        }
    }

    fn optimize_while_loop(
        &mut self,
        while_stmt: &mut typedlua_parser::ast::statement::WhileStatement,
    ) -> (Vec<Statement>, bool) {
        if let ExpressionKind::Literal(Literal::Boolean(false)) = &while_stmt.condition.kind {
            while_stmt.body.statements.clear();
            return (Vec::new(), true);
        }
        let modified_vars = self.collect_modified_variables(&while_stmt.body);
        let (hoisted, new_body) = self.hoist_invariants_simple(&while_stmt.body, &modified_vars);
        while_stmt.body = new_body;
        let changed = !hoisted.is_empty() || self.optimize_block(&mut while_stmt.body.statements);
        (hoisted, changed)
    }

    fn optimize_repeat_loop(
        &mut self,
        repeat_stmt: &mut typedlua_parser::ast::statement::RepeatStatement,
    ) -> (Vec<Statement>, bool) {
        if let ExpressionKind::Literal(Literal::Boolean(true)) = &repeat_stmt.until.kind {
            repeat_stmt.body.statements.clear();
            return (Vec::new(), true);
        }
        let modified_vars = self.collect_modified_variables(&repeat_stmt.body);
        let (hoisted, new_body) = self.hoist_invariants_simple(&repeat_stmt.body, &modified_vars);
        repeat_stmt.body = new_body;
        let changed = !hoisted.is_empty() || self.optimize_block(&mut repeat_stmt.body.statements);
        (hoisted, changed)
    }

    fn optimize_block(&mut self, stmts: &mut Vec<Statement>) -> bool {
        let mut changed = false;
        let mut i = 0;
        while i < stmts.len() {
            let (hoisted, stmt_changed) = self.optimize_loops_in_statement(&mut stmts[i]);
            if !hoisted.is_empty() {
                stmts.splice(i..i, hoisted);
                i += 1;
                changed = true;
            }
            if stmt_changed {
                changed = true;
            }
            i += 1;
        }
        changed
    }

    fn collect_modified_variables(&self, block: &Block) -> HashSet<StringId> {
        let mut modified = HashSet::new();
        self.collect_modified_in_block(block, &mut modified);
        modified
    }

    fn collect_modified_in_block(&self, block: &Block, modified: &mut HashSet<StringId>) {
        for stmt in &block.statements {
            self.collect_modified_in_statement(stmt, modified);
        }
    }

    fn collect_modified_in_statement(&self, stmt: &Statement, modified: &mut HashSet<StringId>) {
        match stmt {
            Statement::Variable(decl) => {
                self.collect_modified_in_pattern(&decl.pattern, modified);
            }
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => {
                    modified.insert(for_num.variable.node);
                    self.collect_modified_in_block(&for_num.body, modified);
                }
                ForStatement::Generic(for_gen) => {
                    for var in &for_gen.variables {
                        modified.insert(var.node);
                    }
                    self.collect_modified_in_block(&for_gen.body, modified);
                }
            },
            Statement::While(while_stmt) => {
                self.collect_modified_in_expression(&while_stmt.condition, modified);
                self.collect_modified_in_block(&while_stmt.body, modified);
            }
            Statement::Repeat(repeat_stmt) => {
                self.collect_modified_in_block(&repeat_stmt.body, modified);
                self.collect_modified_in_expression(&repeat_stmt.until, modified);
            }
            Statement::Function(func) => {
                self.collect_modified_in_block(&func.body, modified);
            }
            Statement::If(if_stmt) => {
                self.collect_modified_in_expression(&if_stmt.condition, modified);
                self.collect_modified_in_block(&if_stmt.then_block, modified);
                for else_if in &if_stmt.else_ifs {
                    self.collect_modified_in_expression(&else_if.condition, modified);
                    self.collect_modified_in_block(&else_if.block, modified);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.collect_modified_in_block(else_block, modified);
                }
            }
            Statement::Expression(expr) => {
                self.collect_modified_in_expression(expr, modified);
            }
            Statement::Return(ret_stmt) => {
                for expr in &ret_stmt.values {
                    self.collect_modified_in_expression(expr, modified);
                }
            }
            Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Rethrow(_)
            | Statement::Throw(_) => {}
            Statement::Class(_)
            | Statement::Interface(_)
            | Statement::Enum(_)
            | Statement::TypeAlias(_) => {}
            Statement::Import(_) | Statement::Export(_) => {}
            Statement::Block(block) => {
                self.collect_modified_in_block(block, modified);
            }
            Statement::Try(try_stmt) => {
                self.collect_modified_in_block(&try_stmt.try_block, modified);
                for catch in &try_stmt.catch_clauses {
                    match &catch.pattern {
                        typedlua_parser::ast::statement::CatchPattern::Typed {
                            variable, ..
                        } => {
                            modified.insert(variable.node);
                        }
                        typedlua_parser::ast::statement::CatchPattern::MultiTyped {
                            variable,
                            ..
                        } => {
                            modified.insert(variable.node);
                        }
                        typedlua_parser::ast::statement::CatchPattern::Untyped {
                            variable, ..
                        } => {
                            modified.insert(variable.node);
                        }
                    }
                    self.collect_modified_in_block(&catch.body, modified);
                }
                if let Some(finally_block) = &try_stmt.finally_block {
                    self.collect_modified_in_block(finally_block, modified);
                }
            }
            Statement::Namespace(_)
            | Statement::DeclareFunction(_)
            | Statement::DeclareNamespace(_)
            | Statement::DeclareType(_)
            | Statement::DeclareInterface(_)
            | Statement::DeclareConst(_)
            | Statement::Label(_)
            | Statement::Goto(_) => {}
        }
    }

    fn collect_modified_in_pattern(&self, pattern: &Pattern, modified: &mut HashSet<StringId>) {
        match pattern {
            Pattern::Identifier(ident) => {
                modified.insert(ident.node);
            }
            Pattern::Array(array_pattern) => {
                for elem in &array_pattern.elements {
                    match elem {
                        typedlua_parser::ast::pattern::ArrayPatternElement::Pattern(p) => {
                            self.collect_modified_in_pattern(p, modified);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Rest(id) => {
                            modified.insert(id.node);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Hole => {}
                    }
                }
            }
            Pattern::Object(obj_pattern) => {
                for prop in &obj_pattern.properties {
                    if let Some(p) = &prop.value {
                        self.collect_modified_in_pattern(p, modified);
                    }
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_, _) => {}
            Pattern::Or(or_pattern) => {
                // All alternatives bind the same variables (guaranteed by type checker)
                // So we can collect from just the first alternative
                if let Some(first) = or_pattern.alternatives.first() {
                    self.collect_modified_in_pattern(first, modified);
                }
            }
        }
    }

    fn collect_modified_in_expression(&self, expr: &Expression, modified: &mut HashSet<StringId>) {
        match &expr.kind {
            ExpressionKind::Identifier(id) => {
                modified.insert(*id);
            }
            ExpressionKind::Binary(_, left, right) => {
                self.collect_modified_in_expression(left, modified);
                self.collect_modified_in_expression(right, modified);
            }
            ExpressionKind::Unary(_, operand) => {
                self.collect_modified_in_expression(operand, modified);
            }
            ExpressionKind::Call(func, args, _) => {
                self.collect_modified_in_expression(func, modified);
                for arg in args {
                    self.collect_modified_in_expression(&arg.value, modified);
                }
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.collect_modified_in_expression(obj, modified);
                for arg in args {
                    self.collect_modified_in_expression(&arg.value, modified);
                }
            }
            ExpressionKind::Member(obj, _) => {
                self.collect_modified_in_expression(obj, modified);
            }
            ExpressionKind::Index(obj, index) => {
                self.collect_modified_in_expression(obj, modified);
                self.collect_modified_in_expression(index, modified);
            }
            ExpressionKind::Assignment(lhs, _, rhs) => {
                self.collect_modified_in_expression(lhs, modified);
                self.collect_modified_in_expression(rhs, modified);
            }
            ExpressionKind::Array(elements) => {
                for elem in elements {
                    match elem {
                        ArrayElement::Expression(expr) => {
                            self.collect_modified_in_expression(expr, modified)
                        }
                        ArrayElement::Spread(expr) => {
                            self.collect_modified_in_expression(expr, modified)
                        }
                    }
                }
            }
            ExpressionKind::Object(properties) => {
                for prop in properties {
                    match prop {
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            key: _,
                            value,
                            span: _,
                        } => {
                            self.collect_modified_in_expression(value, modified);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            span: _,
                        } => {
                            self.collect_modified_in_expression(key, modified);
                            self.collect_modified_in_expression(value, modified);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value,
                            span: _,
                        } => {
                            self.collect_modified_in_expression(value, modified);
                        }
                    }
                }
            }
            ExpressionKind::Function(func) => {
                self.collect_modified_in_block(&func.body, modified);
            }
            ExpressionKind::Arrow(arrow) => {
                for param in &arrow.parameters {
                    self.collect_modified_in_pattern(&param.pattern, modified);
                }
                match &arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                        self.collect_modified_in_expression(expr, modified);
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        self.collect_modified_in_block(block, modified);
                    }
                }
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.collect_modified_in_expression(cond, modified);
                self.collect_modified_in_expression(then_expr, modified);
                self.collect_modified_in_expression(else_expr, modified);
            }
            ExpressionKind::Pipe(left, right) => {
                self.collect_modified_in_expression(left, modified);
                self.collect_modified_in_expression(right, modified);
            }
            ExpressionKind::Match(match_expr) => {
                self.collect_modified_in_expression(&match_expr.value, modified);
                for arm in &match_expr.arms {
                    self.collect_modified_in_pattern(&arm.pattern, modified);
                    if let Some(guard) = &arm.guard {
                        self.collect_modified_in_expression(guard, modified);
                    }
                    match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                            self.collect_modified_in_expression(expr, modified);
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            self.collect_modified_in_block(block, modified);
                        }
                    }
                }
            }
            ExpressionKind::Template(template) => {
                for part in &template.parts {
                    match part {
                        typedlua_parser::ast::expression::TemplatePart::String(_) => {}
                        typedlua_parser::ast::expression::TemplatePart::Expression(expr) => {
                            self.collect_modified_in_expression(expr, modified);
                        }
                    }
                }
            }
            ExpressionKind::Parenthesized(expr) => {
                self.collect_modified_in_expression(expr, modified);
            }
            ExpressionKind::TypeAssertion(expr, _) => {
                self.collect_modified_in_expression(expr, modified);
            }
            ExpressionKind::New(expr, args, _) => {
                self.collect_modified_in_expression(expr, modified);
                for arg in args {
                    self.collect_modified_in_expression(&arg.value, modified);
                }
            }
            ExpressionKind::OptionalMember(obj, _) => {
                self.collect_modified_in_expression(obj, modified);
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                self.collect_modified_in_expression(obj, modified);
                self.collect_modified_in_expression(index, modified);
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.collect_modified_in_expression(obj, modified);
                for arg in args {
                    self.collect_modified_in_expression(&arg.value, modified);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.collect_modified_in_expression(obj, modified);
                for arg in args {
                    self.collect_modified_in_expression(&arg.value, modified);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.collect_modified_in_expression(&try_expr.expression, modified);
                modified.insert(try_expr.catch_variable.node);
                self.collect_modified_in_expression(&try_expr.catch_expression, modified);
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.collect_modified_in_expression(left, modified);
                self.collect_modified_in_expression(right, modified);
            }
            ExpressionKind::Literal(_)
            | ExpressionKind::SelfKeyword
            | ExpressionKind::SuperKeyword => {}
        }
    }

    fn hoist_invariants_simple(
        &self,
        block: &Block,
        loop_vars: &HashSet<StringId>,
    ) -> (Vec<Statement>, Block) {
        let mut hoisted = Vec::new();
        let mut new_statements = Vec::new();

        for stmt in &block.statements {
            match stmt {
                Statement::Variable(decl) => {
                    if self.is_invariant_expression(&decl.initializer, loop_vars) {
                        hoisted.push(stmt.clone());
                    } else {
                        new_statements.push(stmt.clone());
                    }
                }
                _ => new_statements.push(stmt.clone()),
            }
        }

        (
            hoisted,
            Block {
                statements: new_statements,
                span: block.span,
            },
        )
    }

    fn is_invariant_expression(&self, expr: &Expression, loop_vars: &HashSet<StringId>) -> bool {
        match &expr.kind {
            ExpressionKind::Literal(_) => true,
            ExpressionKind::Identifier(id) => !loop_vars.contains(id),
            ExpressionKind::Binary(_, left, right) => {
                self.is_invariant_expression(left, loop_vars)
                    && self.is_invariant_expression(right, loop_vars)
            }
            ExpressionKind::Unary(_, operand) => self.is_invariant_expression(operand, loop_vars),
            ExpressionKind::Call(func, args, _) => {
                let func_invariant = match &func.kind {
                    ExpressionKind::Identifier(id) => !loop_vars.contains(id),
                    _ => false,
                };
                func_invariant
                    && args
                        .iter()
                        .all(|arg| self.is_invariant_expression(&arg.value, loop_vars))
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.is_invariant_expression(obj, loop_vars)
                    && args
                        .iter()
                        .all(|arg| self.is_invariant_expression(&arg.value, loop_vars))
            }
            ExpressionKind::Member(obj, _) => self.is_invariant_expression(obj, loop_vars),
            ExpressionKind::Index(obj, index) => {
                self.is_invariant_expression(obj, loop_vars)
                    && self.is_invariant_expression(index, loop_vars)
            }
            ExpressionKind::Array(elements) => elements.iter().all(|elem| match elem {
                ArrayElement::Expression(e) => self.is_invariant_expression(e, loop_vars),
                ArrayElement::Spread(e) => self.is_invariant_expression(e, loop_vars),
            }),
            ExpressionKind::Object(properties) => properties.iter().all(|prop| match prop {
                typedlua_parser::ast::expression::ObjectProperty::Property {
                    key: _,
                    value,
                    span: _,
                } => self.is_invariant_expression(value, loop_vars),
                typedlua_parser::ast::expression::ObjectProperty::Computed {
                    key,
                    value,
                    span: _,
                } => {
                    self.is_invariant_expression(key, loop_vars)
                        && self.is_invariant_expression(value, loop_vars)
                }
                typedlua_parser::ast::expression::ObjectProperty::Spread { value, span: _ } => {
                    self.is_invariant_expression(value, loop_vars)
                }
            }),
            ExpressionKind::Function(_) => true,
            ExpressionKind::Arrow(arrow) => {
                let body_invariant = match &arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                        self.is_invariant_expression(expr, loop_vars)
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        block.statements.iter().all(|s| match s {
                            Statement::Variable(decl) => {
                                self.is_invariant_expression(&decl.initializer, loop_vars)
                            }
                            _ => false,
                        })
                    }
                };
                body_invariant
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.is_invariant_expression(cond, loop_vars)
                    && self.is_invariant_expression(then_expr, loop_vars)
                    && self.is_invariant_expression(else_expr, loop_vars)
            }
            ExpressionKind::Pipe(left, right) => {
                self.is_invariant_expression(left, loop_vars)
                    && self.is_invariant_expression(right, loop_vars)
            }
            ExpressionKind::Match(match_expr) => {
                self.is_invariant_expression(&match_expr.value, loop_vars)
                    && match_expr.arms.iter().all(|arm| {
                        let body_invariant = match &arm.body {
                            typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                                self.is_invariant_expression(expr, loop_vars)
                            }
                            typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                                block.statements.iter().all(|s| match s {
                                    Statement::Variable(decl) => {
                                        self.is_invariant_expression(&decl.initializer, loop_vars)
                                    }
                                    _ => false,
                                })
                            }
                        };
                        body_invariant
                    })
            }
            ExpressionKind::Template(template) => template.parts.iter().all(|part| match part {
                typedlua_parser::ast::expression::TemplatePart::String(_) => true,
                typedlua_parser::ast::expression::TemplatePart::Expression(expr) => {
                    self.is_invariant_expression(expr, loop_vars)
                }
            }),
            ExpressionKind::Parenthesized(expr) => self.is_invariant_expression(expr, loop_vars),
            ExpressionKind::TypeAssertion(expr, _) => self.is_invariant_expression(expr, loop_vars),
            ExpressionKind::New(expr, args, _) => {
                self.is_invariant_expression(expr, loop_vars)
                    && args
                        .iter()
                        .all(|arg| self.is_invariant_expression(&arg.value, loop_vars))
            }
            ExpressionKind::OptionalMember(obj, _) => self.is_invariant_expression(obj, loop_vars),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.is_invariant_expression(obj, loop_vars)
                    && self.is_invariant_expression(index, loop_vars)
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.is_invariant_expression(obj, loop_vars)
                    && args
                        .iter()
                        .all(|arg| self.is_invariant_expression(&arg.value, loop_vars))
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.is_invariant_expression(obj, loop_vars)
                    && args
                        .iter()
                        .all(|arg| self.is_invariant_expression(&arg.value, loop_vars))
            }
            ExpressionKind::Try(try_expr) => {
                self.is_invariant_expression(&try_expr.expression, loop_vars)
                    && self.is_invariant_expression(&try_expr.catch_expression, loop_vars)
            }
            ExpressionKind::Assignment(_, _, rhs) => self.is_invariant_expression(rhs, loop_vars),
            ExpressionKind::ErrorChain(left, right) => {
                self.is_invariant_expression(left, loop_vars)
                    && self.is_invariant_expression(right, loop_vars)
            }
            ExpressionKind::SelfKeyword | ExpressionKind::SuperKeyword => true,
        }
    }

    fn evaluate_numeric_bounds(&self, for_num: &ForNumeric) -> Option<(f64, f64, f64)> {
        let start = self.evaluate_constant_f64(&for_num.start)?;
        let end = self.evaluate_constant_f64(&for_num.end)?;
        let step = for_num
            .step
            .as_ref()
            .map(|s| self.evaluate_constant_f64(s))
            .unwrap_or(Some(1.0))?;
        Some((start, end, step))
    }

    fn evaluate_constant_f64(&self, expr: &Expression) -> Option<f64> {
        match &expr.kind {
            ExpressionKind::Literal(Literal::Number(n)) => Some(*n),
            ExpressionKind::Literal(Literal::Integer(n)) => Some(*n as f64),
            ExpressionKind::Unary(UnaryOp::Negate, operand) => {
                self.evaluate_constant_f64(operand).map(|n| -n)
            }
            ExpressionKind::Binary(BinaryOp::Add, left, right) => {
                let l = self.evaluate_constant_f64(left)?;
                let r = self.evaluate_constant_f64(right)?;
                Some(l + r)
            }
            ExpressionKind::Binary(BinaryOp::Subtract, left, right) => {
                let l = self.evaluate_constant_f64(left)?;
                let r = self.evaluate_constant_f64(right)?;
                Some(l - r)
            }
            ExpressionKind::Binary(BinaryOp::Multiply, left, right) => {
                let l = self.evaluate_constant_f64(left)?;
                let r = self.evaluate_constant_f64(right)?;
                Some(l * r)
            }
            ExpressionKind::Binary(BinaryOp::Divide, left, right) => {
                let l = self.evaluate_constant_f64(left)?;
                let r = self.evaluate_constant_f64(right)?;
                if r.abs() > 1e-10 {
                    Some(l / r)
                } else {
                    None
                }
            }
            ExpressionKind::Parenthesized(expr) => self.evaluate_constant_f64(expr),
            _ => None,
        }
    }

    fn has_zero_iterations(&self, start: f64, end: f64, step: f64) -> bool {
        if step.abs() < 1e-10 {
            return false;
        }
        if step > 0.0 {
            start > end
        } else {
            start < end
        }
    }
}

impl Default for LoopOptimizationPass {
    fn default() -> Self {
        Self::new()
    }
}
