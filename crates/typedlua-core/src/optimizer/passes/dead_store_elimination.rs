// =============================================================================
// O2: Dead Store Elimination Pass
// =============================================================================

use crate::config::OptimizationLevel;
use crate::optimizer::{StmtVisitor, WholeProgramPass};
use std::collections::HashSet;
use typedlua_parser::ast::expression::{BinaryOp, Expression, ExpressionKind};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{Block, ForStatement, Statement};
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::StringId;

/// Dead store elimination pass
/// Removes assignments to variables that are never read
pub struct DeadStoreEliminationPass;

impl DeadStoreEliminationPass {
    pub fn new() -> Self {
        Self
    }
}

impl StmtVisitor for DeadStoreEliminationPass {
    fn visit_stmt(&mut self, stmt: &mut Statement) -> bool {
        self.eliminate_dead_stores_in_statement(stmt)
    }
}

impl WholeProgramPass for DeadStoreEliminationPass {
    fn name(&self) -> &'static str {
        "dead-store-elimination"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        let mut program_block = Block {
            statements: std::mem::take(&mut program.statements),
            span: program.span,
        };
        let changed = self.eliminate_dead_stores_in_block(&mut program_block);
        program.statements = program_block.statements;
        program.span = program_block.span;

        Ok(changed)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl DeadStoreEliminationPass {
    fn eliminate_dead_stores_in_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::Function(func) => self.eliminate_dead_stores_in_block(&mut func.body),
            Statement::Block(block) => self.eliminate_dead_stores_in_block(block),
            Statement::Variable(decl) => {
                self.eliminate_dead_stores_in_expression(&mut decl.initializer)
            }
            Statement::Expression(expr) => self.eliminate_dead_stores_in_expression(expr),
            Statement::If(if_stmt) => {
                let mut changed = self.eliminate_dead_stores_in_block(&mut if_stmt.then_block);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.eliminate_dead_stores_in_block(&mut else_if.block);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.eliminate_dead_stores_in_block(else_block);
                }
                changed
            }
            Statement::While(while_stmt) => {
                self.eliminate_dead_stores_in_block(&mut while_stmt.body)
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Numeric(for_num) => {
                    self.eliminate_dead_stores_in_block(&mut for_num.body)
                }
                ForStatement::Generic(for_gen) => {
                    self.eliminate_dead_stores_in_block(&mut for_gen.body)
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.eliminate_dead_stores_in_block(&mut repeat_stmt.body)
            }
            Statement::Return(ret) => {
                let mut changed = false;
                for expr in &mut ret.values {
                    changed |= self.eliminate_dead_stores_in_expression(expr);
                }
                changed
            }
            _ => false,
        }
    }

    fn eliminate_dead_stores_in_expression(&mut self, expr: &mut Expression) -> bool {
        match &mut expr.kind {
            ExpressionKind::Function(func) => self.eliminate_dead_stores_in_block(&mut func.body),
            ExpressionKind::Arrow(arrow) => match &mut arrow.body {
                typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                    self.eliminate_dead_stores_in_block(block)
                }
                typedlua_parser::ast::expression::ArrowBody::Expression(inner) => {
                    self.eliminate_dead_stores_in_expression(inner)
                }
            },
            ExpressionKind::Call(func, args, _) => {
                let mut changed = self.eliminate_dead_stores_in_expression(func);
                for arg in args {
                    changed |= self.eliminate_dead_stores_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                let mut changed = self.eliminate_dead_stores_in_expression(obj);
                for arg in args {
                    changed |= self.eliminate_dead_stores_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Binary(_, left, right) => {
                let mut changed = self.eliminate_dead_stores_in_expression(left);
                changed |= self.eliminate_dead_stores_in_expression(right);
                changed
            }
            ExpressionKind::Unary(_, operand) => self.eliminate_dead_stores_in_expression(operand),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut changed = self.eliminate_dead_stores_in_expression(cond);
                changed |= self.eliminate_dead_stores_in_expression(then_expr);
                changed |= self.eliminate_dead_stores_in_expression(else_expr);
                changed
            }
            ExpressionKind::Array(elements) => {
                let mut changed = false;
                for elem in elements {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(e) => {
                            changed |= self.eliminate_dead_stores_in_expression(e);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(e) => {
                            changed |= self.eliminate_dead_stores_in_expression(e);
                        }
                    }
                }
                changed
            }
            ExpressionKind::Object(properties) => {
                let mut changed = false;
                for prop in properties {
                    match prop {
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            value,
                            ..
                        } => {
                            changed |= self.eliminate_dead_stores_in_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            ..
                        } => {
                            changed |= self.eliminate_dead_stores_in_expression(key);
                            changed |= self.eliminate_dead_stores_in_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value, ..
                        } => {
                            changed |= self.eliminate_dead_stores_in_expression(value);
                        }
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn eliminate_dead_stores_in_block(&mut self, block: &mut Block) -> bool {
        if block.statements.is_empty() {
            return false;
        }

        let captured = self.collect_captured_variables(block);

        let mut new_statements: Vec<Statement> = Vec::new();
        let mut changed = false;
        let mut current_live_vars: HashSet<StringId> = HashSet::new();

        for stmt in block.statements.iter().rev() {
            let names = self.names_from_pattern(stmt);
            let has_side_effects = self.statement_has_side_effects(stmt);
            let stmt_reads = self.collect_statement_reads(stmt);

            let mut is_dead = names.is_empty();
            for name in &names {
                if captured.contains(name) || has_side_effects || current_live_vars.contains(name) {
                    is_dead = false;
                    break;
                }
            }

            if !is_dead {
                let mut stmt_clone = stmt.clone();
                changed |= self.eliminate_dead_stores_in_statement(&mut stmt_clone);
                new_statements.push(stmt_clone);
                for name in &names {
                    if stmt_reads.contains(name) {
                        current_live_vars.insert(*name);
                    }
                }
            } else {
                changed = true;
            }
        }

        if changed {
            new_statements.reverse();
            block.statements = new_statements;
        }

        changed
    }

    fn statement_has_side_effects(&self, stmt: &Statement) -> bool {
        match stmt {
            Statement::Variable(decl) => self.expression_has_side_effects(&decl.initializer),
            Statement::Expression(expr) => {
                if let ExpressionKind::Assignment(_, _, _) = &expr.kind {
                    true
                } else {
                    self.expression_has_side_effects(expr)
                }
            }
            Statement::If(if_stmt) => {
                self.expression_has_side_effects(&if_stmt.condition)
                    || self.block_has_side_effects(&if_stmt.then_block)
                    || if_stmt
                        .else_ifs
                        .iter()
                        .any(|ei| self.block_has_side_effects(&ei.block))
                    || if_stmt
                        .else_block
                        .as_ref()
                        .is_some_and(|eb| self.block_has_side_effects(eb))
            }
            Statement::While(while_stmt) => {
                self.expression_has_side_effects(&while_stmt.condition)
                    || self.block_has_side_effects(&while_stmt.body)
            }
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => {
                    self.expression_has_side_effects(&for_num.start)
                        || self.expression_has_side_effects(&for_num.end)
                        || for_num
                            .step
                            .as_ref()
                            .is_some_and(|s| self.expression_has_side_effects(s))
                        || self.block_has_side_effects(&for_num.body)
                }
                ForStatement::Generic(for_gen) => {
                    for_expr_has_side_effects(&for_gen.iterators)
                        || self.block_has_side_effects(&for_gen.body)
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.expression_has_side_effects(&repeat_stmt.until)
                    || self.block_has_side_effects(&repeat_stmt.body)
            }
            Statement::Return(ret) => ret
                .values
                .iter()
                .any(|e| self.expression_has_side_effects(e)),
            Statement::Try(try_stmt) => {
                self.block_has_side_effects(&try_stmt.try_block)
                    || try_stmt
                        .catch_clauses
                        .iter()
                        .any(|c| self.block_has_side_effects(&c.body))
                    || try_stmt
                        .finally_block
                        .as_ref()
                        .is_some_and(|fb| self.block_has_side_effects(fb))
            }
            Statement::Function(func) => self.block_has_side_effects(&func.body),
            Statement::Block(block) => self.block_has_side_effects(block),
            _ => false,
        }
    }

    fn block_has_side_effects(&self, block: &Block) -> bool {
        block
            .statements
            .iter()
            .any(|s| self.statement_has_side_effects(s))
    }

    fn names_from_pattern(&self, stmt: &Statement) -> Vec<StringId> {
        let mut names = Vec::new();
        if let Statement::Variable(decl) = stmt {
            self.collect_names_from_pattern(&decl.pattern, &mut names);
        }
        names
    }

    fn collect_names_from_pattern(&self, pattern: &Pattern, names: &mut Vec<StringId>) {
        match pattern {
            Pattern::Identifier(ident) => {
                names.push(ident.node);
            }
            Pattern::Array(arr) => {
                for elem in &arr.elements {
                    match elem {
                        typedlua_parser::ast::pattern::ArrayPatternElement::Pattern(p) => {
                            self.collect_names_from_pattern(p, names);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Rest(ident) => {
                            names.push(ident.node);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Hole => {}
                    }
                }
            }
            Pattern::Object(obj) => {
                for prop in &obj.properties {
                    if let Some(value_pattern) = &prop.value {
                        self.collect_names_from_pattern(value_pattern, names);
                    } else {
                        names.push(prop.key.node);
                    }
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_, _) => {}
            Pattern::Or(or_pattern) => {
                if let Some(first) = or_pattern.alternatives.first() {
                    self.collect_names_from_pattern(first, names);
                }
            }
        }
    }

    fn expression_has_side_effects(&self, expr: &Expression) -> bool {
        match &expr.kind {
            ExpressionKind::Call(_, _, _) => true,
            ExpressionKind::MethodCall(_, _, _, _) => true,
            ExpressionKind::Assignment(_, _, _) => true,
            ExpressionKind::Binary(BinaryOp::And, left, right) => {
                self.expression_has_side_effects(left) || self.expression_has_side_effects(right)
            }
            ExpressionKind::Binary(BinaryOp::Or, left, right) => {
                self.expression_has_side_effects(left) || self.expression_has_side_effects(right)
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.expression_has_side_effects(cond)
                    || self.expression_has_side_effects(then_expr)
                    || self.expression_has_side_effects(else_expr)
            }
            _ => false,
        }
    }

    fn collect_statement_reads(&self, stmt: &Statement) -> HashSet<StringId> {
        let mut reads = HashSet::new();
        self.collect_statement_reads_into(stmt, &mut reads);
        reads
    }

    fn collect_statement_reads_into(&self, stmt: &Statement, reads: &mut HashSet<StringId>) {
        match stmt {
            Statement::Variable(decl) => {
                self.collect_expression_reads_into(&decl.initializer, reads);
            }
            Statement::Expression(expr) => {
                self.collect_expression_reads_into(expr, reads);
            }
            Statement::Return(ret) => {
                for expr in &ret.values {
                    self.collect_expression_reads_into(expr, reads);
                }
            }
            Statement::If(if_stmt) => {
                self.collect_expression_reads_into(&if_stmt.condition, reads);
                self.collect_block_reads_into(&if_stmt.then_block, reads);
                for else_if in &if_stmt.else_ifs {
                    self.collect_expression_reads_into(&else_if.condition, reads);
                    self.collect_block_reads_into(&else_if.block, reads);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.collect_block_reads_into(else_block, reads);
                }
            }
            Statement::While(while_stmt) => {
                self.collect_expression_reads_into(&while_stmt.condition, reads);
                self.collect_block_reads_into(&while_stmt.body, reads);
            }
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => {
                    self.collect_expression_reads_into(&for_num.start, reads);
                    self.collect_expression_reads_into(&for_num.end, reads);
                    if let Some(step) = &for_num.step {
                        self.collect_expression_reads_into(step, reads);
                    }
                    self.collect_block_reads_into(&for_num.body, reads);
                }
                ForStatement::Generic(for_gen) => {
                    for_expr_has_side_effects(&for_gen.iterators);
                    self.collect_block_reads_into(&for_gen.body, reads);
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.collect_block_reads_into(&repeat_stmt.body, reads);
                self.collect_expression_reads_into(&repeat_stmt.until, reads);
            }
            Statement::Function(func) => {
                self.collect_block_reads_into(&func.body, reads);
            }
            Statement::Block(block) => {
                self.collect_block_reads_into(block, reads);
            }
            Statement::Try(try_stmt) => {
                self.collect_block_reads_into(&try_stmt.try_block, reads);
                for catch in &try_stmt.catch_clauses {
                    self.collect_block_reads_into(&catch.body, reads);
                }
                if let Some(finally_block) = &try_stmt.finally_block {
                    self.collect_block_reads_into(finally_block, reads);
                }
            }
            _ => {}
        }
    }

    fn collect_block_reads_into(&self, block: &Block, reads: &mut HashSet<StringId>) {
        for stmt in &block.statements {
            self.collect_statement_reads_into(stmt, reads);
        }
    }

    fn collect_expression_reads_into(&self, expr: &Expression, reads: &mut HashSet<StringId>) {
        match &expr.kind {
            ExpressionKind::Identifier(name) => {
                reads.insert(*name);
            }
            ExpressionKind::Binary(_, left, right) => {
                self.collect_expression_reads_into(left, reads);
                self.collect_expression_reads_into(right, reads);
            }
            ExpressionKind::Unary(_, operand) => {
                self.collect_expression_reads_into(operand, reads);
            }
            ExpressionKind::Call(func, args, _) => {
                self.collect_expression_reads_into(func, reads);
                for arg in args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.collect_expression_reads_into(obj, reads);
                for arg in args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::Member(obj, _) => {
                self.collect_expression_reads_into(obj, reads);
            }
            ExpressionKind::Index(obj, index) => {
                self.collect_expression_reads_into(obj, reads);
                self.collect_expression_reads_into(index, reads);
            }
            ExpressionKind::Assignment(lhs, _, rhs) => {
                self.collect_expression_reads_into(lhs, reads);
                self.collect_expression_reads_into(rhs, reads);
            }
            ExpressionKind::Array(elements) => {
                for elem in elements {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(e) => {
                            self.collect_expression_reads_into(e, reads);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(e) => {
                            self.collect_expression_reads_into(e, reads);
                        }
                    }
                }
            }
            ExpressionKind::Object(properties) => {
                for prop in properties {
                    match prop {
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            value,
                            ..
                        } => {
                            self.collect_expression_reads_into(value, reads);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            ..
                        } => {
                            self.collect_expression_reads_into(key, reads);
                            self.collect_expression_reads_into(value, reads);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value, ..
                        } => {
                            self.collect_expression_reads_into(value, reads);
                        }
                    }
                }
            }
            ExpressionKind::Function(func) => {
                self.collect_block_reads_into(&func.body, reads);
            }
            ExpressionKind::Arrow(arrow) => match &arrow.body {
                typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                    self.collect_expression_reads_into(expr, reads);
                }
                typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                    self.collect_block_reads_into(block, reads);
                }
            },
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.collect_expression_reads_into(cond, reads);
                self.collect_expression_reads_into(then_expr, reads);
                self.collect_expression_reads_into(else_expr, reads);
            }
            ExpressionKind::Pipe(left, right) => {
                self.collect_expression_reads_into(left, reads);
                self.collect_expression_reads_into(right, reads);
            }
            ExpressionKind::Match(match_expr) => {
                self.collect_expression_reads_into(&match_expr.value, reads);
                for arm in &match_expr.arms {
                    match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                            self.collect_expression_reads_into(expr, reads);
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            self.collect_block_reads_into(block, reads);
                        }
                    }
                }
            }
            ExpressionKind::Template(template) => {
                for part in &template.parts {
                    if let typedlua_parser::ast::expression::TemplatePart::Expression(expr) = part {
                        self.collect_expression_reads_into(expr, reads);
                    }
                }
            }
            ExpressionKind::Parenthesized(expr) => {
                self.collect_expression_reads_into(expr, reads);
            }
            ExpressionKind::TypeAssertion(expr, _) => {
                self.collect_expression_reads_into(expr, reads);
            }
            ExpressionKind::New(expr, args, _) => {
                self.collect_expression_reads_into(expr, reads);
                for arg in args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::OptionalMember(obj, _) => {
                self.collect_expression_reads_into(obj, reads);
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                self.collect_expression_reads_into(obj, reads);
                self.collect_expression_reads_into(index, reads);
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.collect_expression_reads_into(obj, reads);
                for arg in args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.collect_expression_reads_into(obj, reads);
                for arg in args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.collect_expression_reads_into(&try_expr.expression, reads);
                self.collect_expression_reads_into(&try_expr.catch_expression, reads);
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.collect_expression_reads_into(left, reads);
                self.collect_expression_reads_into(right, reads);
            }
            ExpressionKind::Literal(_)
            | ExpressionKind::SelfKeyword
            | ExpressionKind::SuperKeyword => {}
        }
    }

    fn collect_captured_variables(&self, block: &Block) -> HashSet<StringId> {
        let mut captured = HashSet::new();
        self.collect_captures_in_block(block, &mut captured);
        captured
    }

    fn collect_captures_in_block(&self, block: &Block, captured: &mut HashSet<StringId>) {
        for stmt in &block.statements {
            self.collect_captures_in_statement(stmt, captured);
        }
    }

    fn collect_captures_in_statement(&self, stmt: &Statement, captured: &mut HashSet<StringId>) {
        match stmt {
            Statement::Variable(decl) => {
                if self.expression_captures_variables(&decl.initializer) {
                    let mut names = Vec::new();
                    self.collect_names_from_pattern(&decl.pattern, &mut names);
                    for name in names {
                        captured.insert(name);
                    }
                }
            }
            Statement::Expression(expr) => {
                self.expression_captures_variables(expr);
            }
            Statement::Function(func) => {
                self.collect_captures_in_block(&func.body, captured);
            }
            Statement::Block(block) => {
                self.collect_captures_in_block(block, captured);
            }
            Statement::If(if_stmt) => {
                self.collect_captures_in_block(&if_stmt.then_block, captured);
                for else_if in &if_stmt.else_ifs {
                    self.collect_captures_in_block(&else_if.block, captured);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.collect_captures_in_block(else_block, captured);
                }
            }
            Statement::While(while_stmt) => {
                self.collect_captures_in_block(&while_stmt.body, captured);
            }
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => {
                    self.collect_captures_in_block(&for_num.body, captured);
                }
                ForStatement::Generic(for_gen) => {
                    self.collect_captures_in_block(&for_gen.body, captured);
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.collect_captures_in_block(&repeat_stmt.body, captured);
            }
            Statement::Try(try_stmt) => {
                self.collect_captures_in_block(&try_stmt.try_block, captured);
                for catch in &try_stmt.catch_clauses {
                    self.collect_captures_in_block(&catch.body, captured);
                }
                if let Some(finally_block) = &try_stmt.finally_block {
                    self.collect_captures_in_block(finally_block, captured);
                }
            }
            _ => {}
        }
    }

    fn expression_captures_variables(&self, expr: &Expression) -> bool {
        match &expr.kind {
            ExpressionKind::Function(_) => true,
            ExpressionKind::Arrow(_) => true,
            ExpressionKind::Call(func, args, _) => {
                self.expression_captures_variables(func)
                    || args
                        .iter()
                        .any(|arg| self.expression_captures_variables(&arg.value))
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.expression_captures_variables(obj)
                    || args
                        .iter()
                        .any(|arg| self.expression_captures_variables(&arg.value))
            }
            ExpressionKind::Binary(_, left, right) => {
                self.expression_captures_variables(left)
                    || self.expression_captures_variables(right)
            }
            ExpressionKind::Unary(_, operand) => self.expression_captures_variables(operand),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.expression_captures_variables(cond)
                    || self.expression_captures_variables(then_expr)
                    || self.expression_captures_variables(else_expr)
            }
            ExpressionKind::Pipe(left, right) => {
                self.expression_captures_variables(left)
                    || self.expression_captures_variables(right)
            }
            ExpressionKind::Match(match_expr) => {
                self.expression_captures_variables(&match_expr.value)
                    || match_expr.arms.iter().any(|arm| match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                            self.expression_captures_variables(expr)
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            self.block_captures_variables(block)
                        }
                    })
            }
            ExpressionKind::New(expr, args, _) => {
                self.expression_captures_variables(expr)
                    || args
                        .iter()
                        .any(|arg| self.expression_captures_variables(&arg.value))
            }
            ExpressionKind::Try(try_expr) => {
                self.expression_captures_variables(&try_expr.expression)
                    || self.expression_captures_variables(&try_expr.catch_expression)
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.expression_captures_variables(left)
                    || self.expression_captures_variables(right)
            }
            ExpressionKind::OptionalMember(obj, _) => self.expression_captures_variables(obj),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.expression_captures_variables(obj) || self.expression_captures_variables(index)
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.expression_captures_variables(obj)
                    || args
                        .iter()
                        .any(|arg| self.expression_captures_variables(&arg.value))
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.expression_captures_variables(obj)
                    || args
                        .iter()
                        .any(|arg| self.expression_captures_variables(&arg.value))
            }
            ExpressionKind::TypeAssertion(expr, _) => self.expression_captures_variables(expr),
            ExpressionKind::Member(obj, _) => self.expression_captures_variables(obj),
            ExpressionKind::Index(obj, index) => {
                self.expression_captures_variables(obj) || self.expression_captures_variables(index)
            }
            _ => false,
        }
    }

    fn block_captures_variables(&self, block: &Block) -> bool {
        for stmt in &block.statements {
            if self.statement_captures_variables(stmt) {
                return true;
            }
        }
        false
    }

    fn statement_captures_variables(&self, stmt: &Statement) -> bool {
        match stmt {
            Statement::Variable(decl) => self.expression_captures_variables(&decl.initializer),
            Statement::Expression(expr) => self.expression_captures_variables(expr),
            Statement::Function(func) => self.block_captures_variables(&func.body),
            Statement::Block(block) => self.block_captures_variables(block),
            Statement::If(if_stmt) => {
                self.block_captures_variables(&if_stmt.then_block)
                    || if_stmt
                        .else_ifs
                        .iter()
                        .any(|ei| self.block_captures_variables(&ei.block))
                    || if_stmt
                        .else_block
                        .as_ref()
                        .is_some_and(|eb| self.block_captures_variables(eb))
            }
            Statement::While(while_stmt) => self.block_captures_variables(&while_stmt.body),
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => self.block_captures_variables(&for_num.body),
                ForStatement::Generic(for_gen) => self.block_captures_variables(&for_gen.body),
            },
            Statement::Repeat(repeat_stmt) => self.block_captures_variables(&repeat_stmt.body),
            Statement::Try(try_stmt) => {
                self.block_captures_variables(&try_stmt.try_block)
                    || try_stmt
                        .catch_clauses
                        .iter()
                        .any(|c| self.block_captures_variables(&c.body))
                    || try_stmt
                        .finally_block
                        .as_ref()
                        .is_some_and(|fb| self.block_captures_variables(fb))
            }
            _ => false,
        }
    }
}

impl Default for DeadStoreEliminationPass {
    fn default() -> Self {
        Self::new()
    }
}

fn for_expr_has_side_effects(exprs: &[typedlua_parser::ast::expression::Expression]) -> bool {
    exprs.iter().any(|e| match &e.kind {
        ExpressionKind::Call(_, _, _) => true,
        ExpressionKind::MethodCall(_, _, _, _) => true,
        ExpressionKind::Assignment(_, _, _) => true,
        _ => false,
    })
}
