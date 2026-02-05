//! O3 Operator Inlining Pass
//!
//! Converts operator overload calls to direct function calls, enabling
//! subsequent inlining by FunctionInliningPass (O2).
//!
//! Conversion Criteria:
//! 1. Operator body contains 5 or fewer statements
//! 2. Operator has no side effects (no external state mutation)
//! 3. Operator is called frequently (heuristic: 3+ call sites)

use crate::config::OptimizationLevel;

use crate::optimizer::{ExprVisitor, PreAnalysisPass, WholeProgramPass};
use rustc_hash::FxHashMap;
use std::sync::Arc;
use typedlua_parser::ast::expression::{BinaryOp, Expression, ExpressionKind, UnaryOp};
use typedlua_parser::ast::statement::{Block, ClassMember, Statement};
use typedlua_parser::ast::types::{Type, TypeKind};
use typedlua_parser::ast::Program;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::{StringId, StringInterner};

const MAX_INLINE_STATEMENTS: usize = 5;
const MIN_CALL_FREQUENCY: usize = 3;

#[derive(Debug, Clone)]
struct OperatorInfo {
    statement_count: usize,
    has_side_effects: bool,
    call_count: usize,
}

pub struct OperatorInliningPass {
    operator_catalog:
        FxHashMap<(StringId, typedlua_parser::ast::statement::OperatorKind), OperatorInfo>,
    interner: Arc<StringInterner>,
}

impl OperatorInliningPass {
    pub fn new(interner: Arc<StringInterner>) -> Self {
        Self {
            operator_catalog: FxHashMap::default(),
            interner,
        }
    }

    fn build_operator_catalog(&mut self, program: &Program) {
        for stmt in &program.statements {
            self.catalog_statement(stmt);
        }
    }

    fn catalog_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Class(class) => {
                let class_name = class.name.node;
                for member in &class.members {
                    if let ClassMember::Operator(op) = member {
                        let statement_count = count_statements(&op.body);
                        let has_side_effects = has_side_effects(&op.body);

                        let info = OperatorInfo {
                            statement_count,
                            has_side_effects,
                            call_count: 0,
                        };

                        self.operator_catalog
                            .insert((class_name, op.operator), info);
                    }
                }
            }
            Statement::Function(func) => {
                self.catalog_block(&func.body);
            }
            Statement::If(if_stmt) => {
                self.catalog_expression(&if_stmt.condition);
                self.catalog_block(&if_stmt.then_block);
                for else_if in &if_stmt.else_ifs {
                    self.catalog_expression(&else_if.condition);
                    self.catalog_block(&else_if.block);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.catalog_block(else_block);
                }
            }
            Statement::While(while_stmt) => {
                self.catalog_expression(&while_stmt.condition);
                self.catalog_block(&while_stmt.body);
            }
            Statement::For(for_stmt) => {
                use typedlua_parser::ast::statement::ForStatement;
                match &**for_stmt {
                    ForStatement::Numeric(for_num) => {
                        self.catalog_expression(&for_num.start);
                        self.catalog_expression(&for_num.end);
                        if let Some(step) = &for_num.step {
                            self.catalog_expression(step);
                        }
                        self.catalog_block(&for_num.body);
                    }
                    ForStatement::Generic(for_gen) => {
                        for expr in &for_gen.iterators {
                            self.catalog_expression(expr);
                        }
                        self.catalog_block(&for_gen.body);
                    }
                }
            }
            Statement::Repeat(repeat_stmt) => {
                self.catalog_expression(&repeat_stmt.until);
                self.catalog_block(&repeat_stmt.body);
            }
            Statement::Return(return_stmt) => {
                for expr in &return_stmt.values {
                    self.catalog_expression(expr);
                }
            }
            Statement::Block(block) => self.catalog_block(block),
            Statement::Try(try_stmt) => {
                self.catalog_block(&try_stmt.try_block);
                for clause in &try_stmt.catch_clauses {
                    self.catalog_block(&clause.body);
                }
                if let Some(finally) = &try_stmt.finally_block {
                    self.catalog_block(finally);
                }
            }
            Statement::Expression(expr) => {
                self.catalog_expression(expr);
            }
            _ => {}
        }
    }

    fn catalog_block(&mut self, block: &Block) {
        for stmt in &block.statements {
            self.catalog_statement(stmt);
        }
    }

    fn catalog_expression(&mut self, expr: &Expression) {
        match &expr.kind {
            ExpressionKind::Binary(op, left, right) => {
                self.catalog_expression(left);
                self.catalog_expression(right);
                self.count_operator_call(op, left, right);
            }
            ExpressionKind::Unary(op, operand) => {
                self.catalog_expression(operand);
                self.count_unary_operator_call(op, operand);
            }
            ExpressionKind::Call(func, args, _) => {
                self.catalog_expression(func);
                for arg in args {
                    self.catalog_expression(&arg.value);
                }
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.catalog_expression(obj);
                for arg in args {
                    self.catalog_expression(&arg.value);
                }
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.catalog_expression(cond);
                self.catalog_expression(then_expr);
                self.catalog_expression(else_expr);
            }
            ExpressionKind::Pipe(left, right) => {
                self.catalog_expression(left);
                self.catalog_expression(right);
            }
            ExpressionKind::Match(match_expr) => {
                self.catalog_expression(&match_expr.value);
                for arm in &match_expr.arms {
                    match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(e) => {
                            self.catalog_expression(e);
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            self.catalog_block(block);
                        }
                    }
                }
            }
            ExpressionKind::Arrow(arrow) => {
                for param in &arrow.parameters {
                    if let Some(default) = &param.default {
                        self.catalog_expression(default);
                    }
                }
                match &arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(e) => {
                        self.catalog_expression(e);
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        self.catalog_block(block);
                    }
                }
            }
            ExpressionKind::New(callee, args, _) => {
                self.catalog_expression(callee);
                for arg in args {
                    self.catalog_expression(&arg.value);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.catalog_expression(&try_expr.expression);
                self.catalog_expression(&try_expr.catch_expression);
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.catalog_expression(left);
                self.catalog_expression(right);
            }
            ExpressionKind::OptionalMember(obj, _) => self.catalog_expression(obj),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.catalog_expression(obj);
                self.catalog_expression(index);
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.catalog_expression(obj);
                for arg in args {
                    self.catalog_expression(&arg.value);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.catalog_expression(obj);
                for arg in args {
                    self.catalog_expression(&arg.value);
                }
            }
            ExpressionKind::Assignment(left, _, right) => {
                self.catalog_expression(left);
                self.catalog_expression(right);
            }
            ExpressionKind::Member(obj, _) => self.catalog_expression(obj),
            ExpressionKind::Index(obj, index) => {
                self.catalog_expression(obj);
                self.catalog_expression(index);
            }
            ExpressionKind::Array(elements) => {
                for elem in elements {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(e) => {
                            self.catalog_expression(e);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(e) => {
                            self.catalog_expression(e);
                        }
                    }
                }
            }
            ExpressionKind::Object(props) => {
                for prop in props {
                    match prop {
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            value,
                            ..
                        } => {
                            self.catalog_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            ..
                        } => {
                            self.catalog_expression(key);
                            self.catalog_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value, ..
                        } => {
                            self.catalog_expression(value);
                        }
                    }
                }
            }
            ExpressionKind::Parenthesized(inner) => self.catalog_expression(inner),
            ExpressionKind::TypeAssertion(expr, _) => self.catalog_expression(expr),
            ExpressionKind::Identifier(_)
            | ExpressionKind::Literal(_)
            | ExpressionKind::SelfKeyword
            | ExpressionKind::SuperKeyword
            | ExpressionKind::Template(_)
            | ExpressionKind::Function(_) => {}
        }
    }

    fn count_operator_call(&mut self, op: &BinaryOp, left: &Expression, _right: &Expression) {
        let operator_kind = binary_op_to_operator_kind(op);
        if operator_kind.is_none() {
            return;
        }
        let operator_kind = operator_kind.unwrap();

        if let Some(left_type) = &left.annotated_type {
            if let Some(class_id) = get_class_from_type(left_type) {
                if let Some(info) = self.operator_catalog.get_mut(&(class_id, operator_kind)) {
                    info.call_count += 1;
                }
            }
        }
    }

    fn count_unary_operator_call(&mut self, op: &UnaryOp, operand: &Expression) {
        let operator_kind = unary_op_to_operator_kind(op);
        if operator_kind.is_none() {
            return;
        }
        let operator_kind = operator_kind.unwrap();

        if let Some(operand_type) = &operand.annotated_type {
            if let Some(class_id) = get_class_from_type(operand_type) {
                if let Some(info) = self.operator_catalog.get_mut(&(class_id, operator_kind)) {
                    info.call_count += 1;
                }
            }
        }
    }

    fn can_inline(&self, info: &OperatorInfo) -> bool {
        if info.statement_count > MAX_INLINE_STATEMENTS {
            return false;
        }
        if info.has_side_effects {
            return false;
        }
        if info.call_count < MIN_CALL_FREQUENCY {
            return false;
        }
        true
    }

    fn convert_operator_call(
        &self,
        op: &BinaryOp,
        left: &Expression,
        right: &Expression,
        span: Span,
    ) -> Option<ExpressionKind> {
        let operator_kind = binary_op_to_operator_kind(op)?;
        let class_id = get_class_from_type(left.annotated_type.as_ref()?)?;
        let info = self.operator_catalog.get(&(class_id, operator_kind))?;
        if !self.can_inline(info) {
            return None;
        }

        let class_name_str = self.interner.resolve(class_id);
        let class_ident_id = self.interner.get_or_intern(&class_name_str);
        let metamethod_name = operator_kind_to_metamethod_name(operator_kind);
        let method_ident_id = self.interner.get_or_intern(&metamethod_name);

        let func_expr = Expression {
            kind: ExpressionKind::Member(
                Box::new(Expression {
                    kind: ExpressionKind::Identifier(class_ident_id),
                    span,
                    annotated_type: None,
                    receiver_class: None,
                }),
                typedlua_parser::ast::Spanned::new(method_ident_id, span),
            ),
            span,
            annotated_type: None,
            receiver_class: None,
        };

        let args = vec![
            typedlua_parser::ast::expression::Argument {
                value: left.clone(),
                is_spread: false,
                span,
            },
            typedlua_parser::ast::expression::Argument {
                value: right.clone(),
                is_spread: false,
                span,
            },
        ];

        Some(ExpressionKind::Call(Box::new(func_expr), args, None))
    }
}

impl ExprVisitor for OperatorInliningPass {
    fn visit_expr(&mut self, expr: &mut Expression) -> bool {
        // Only process binary operations (the main transformation target)
        if let ExpressionKind::Binary(op, left, right) = &mut expr.kind {
            if let Some(new_kind) = self.convert_operator_call(op, left, right, expr.span) {
                expr.kind = new_kind;
                return true;
            }
        }
        false
    }
}

impl PreAnalysisPass for OperatorInliningPass {
    fn analyze(&mut self, program: &Program) {
        self.build_operator_catalog(program);
    }
}

impl WholeProgramPass for OperatorInliningPass {
    fn name(&self) -> &'static str {
        "operator-inlining"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        // Run analysis phase
        self.analyze(program);

        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.process_statement(stmt);
        }

        Ok(changed)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl OperatorInliningPass {
    fn process_statement(&mut self, stmt: &mut Statement) -> bool {
        let mut changed = false;

        match stmt {
            Statement::Function(func) => {
                for s in &mut func.body.statements {
                    changed |= self.process_statement(s);
                }
            }
            Statement::If(if_stmt) => {
                changed |= self.visit_expr(&mut if_stmt.condition);
                for s in &mut if_stmt.then_block.statements {
                    changed |= self.process_statement(s);
                }
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.visit_expr(&mut else_if.condition);
                    for s in &mut else_if.block.statements {
                        changed |= self.process_statement(s);
                    }
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    for s in &mut else_block.statements {
                        changed |= self.process_statement(s);
                    }
                }
            }
            Statement::While(while_stmt) => {
                changed |= self.visit_expr(&mut while_stmt.condition);
                for s in &mut while_stmt.body.statements {
                    changed |= self.process_statement(s);
                }
            }
            Statement::For(for_stmt) => {
                use typedlua_parser::ast::statement::ForStatement;
                let body = match &mut **for_stmt {
                    ForStatement::Numeric(for_num) => &mut for_num.body,
                    ForStatement::Generic(for_gen) => &mut for_gen.body,
                };
                for s in &mut body.statements {
                    changed |= self.process_statement(s);
                }
            }
            Statement::Repeat(repeat_stmt) => {
                changed |= self.visit_expr(&mut repeat_stmt.until);
                for s in &mut repeat_stmt.body.statements {
                    changed |= self.process_statement(s);
                }
            }
            Statement::Return(return_stmt) => {
                for value in &mut return_stmt.values {
                    changed |= self.visit_expr(value);
                }
            }
            Statement::Expression(expr) => {
                changed |= self.process_expression(expr);
            }
            Statement::Block(block) => {
                for s in &mut block.statements {
                    changed |= self.process_statement(s);
                }
            }
            Statement::Try(try_stmt) => {
                for s in &mut try_stmt.try_block.statements {
                    changed |= self.process_statement(s);
                }
                for clause in &mut try_stmt.catch_clauses {
                    for s in &mut clause.body.statements {
                        changed |= self.process_statement(s);
                    }
                }
                if let Some(finally) = &mut try_stmt.finally_block {
                    for s in &mut finally.statements {
                        changed |= self.process_statement(s);
                    }
                }
            }
            _ => {}
        }

        changed
    }

    fn process_expression(&mut self, expr: &mut Expression) -> bool {
        let mut changed = false;

        // Visit children first
        match &mut expr.kind {
            ExpressionKind::Binary(_, left, right) => {
                changed |= self.process_expression(left);
                changed |= self.process_expression(right);
            }
            ExpressionKind::Call(func, args, _) => {
                changed |= self.process_expression(func);
                for arg in args.iter_mut() {
                    changed |= self.process_expression(&mut arg.value);
                }
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                changed |= self.process_expression(obj);
                for arg in args.iter_mut() {
                    changed |= self.process_expression(&mut arg.value);
                }
            }
            ExpressionKind::Unary(_, operand) => {
                changed |= self.process_expression(operand);
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                changed |= self.process_expression(cond);
                changed |= self.process_expression(then_expr);
                changed |= self.process_expression(else_expr);
            }
            ExpressionKind::Pipe(left, right) => {
                changed |= self.process_expression(left);
                changed |= self.process_expression(right);
            }
            ExpressionKind::Match(match_expr) => {
                changed |= self.process_expression(&mut match_expr.value);
                for arm in &mut match_expr.arms {
                    match &mut arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(e) => {
                            changed |= self.process_expression(e);
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            for s in &mut block.statements {
                                changed |= self.process_statement(s);
                            }
                        }
                    }
                }
            }
            ExpressionKind::Arrow(arrow) => {
                for param in &mut arrow.parameters {
                    if let Some(default) = &mut param.default {
                        changed |= self.process_expression(default);
                    }
                }
                match &mut arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(e) => {
                        changed |= self.process_expression(e);
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        for s in &mut block.statements {
                            changed |= self.process_statement(s);
                        }
                    }
                }
            }
            ExpressionKind::New(callee, args, _) => {
                changed |= self.process_expression(callee);
                for arg in args {
                    changed |= self.process_expression(&mut arg.value);
                }
            }
            ExpressionKind::Try(try_expr) => {
                changed |= self.process_expression(&mut try_expr.expression);
                changed |= self.process_expression(&mut try_expr.catch_expression);
            }
            ExpressionKind::ErrorChain(left, right) => {
                changed |= self.process_expression(left);
                changed |= self.process_expression(right);
            }
            ExpressionKind::OptionalMember(obj, _) => {
                changed |= self.process_expression(obj);
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                changed |= self.process_expression(obj);
                changed |= self.process_expression(index);
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                changed |= self.process_expression(obj);
                for arg in args {
                    changed |= self.process_expression(&mut arg.value);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                changed |= self.process_expression(obj);
                for arg in args {
                    changed |= self.process_expression(&mut arg.value);
                }
            }
            ExpressionKind::Assignment(left, _, right) => {
                changed |= self.process_expression(left);
                changed |= self.process_expression(right);
            }
            ExpressionKind::Member(obj, _) => {
                changed |= self.process_expression(obj);
            }
            ExpressionKind::Index(obj, index) => {
                changed |= self.process_expression(obj);
                changed |= self.process_expression(index);
            }
            ExpressionKind::Array(elements) => {
                for elem in elements {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(e) => {
                            changed |= self.process_expression(e);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(e) => {
                            changed |= self.process_expression(e);
                        }
                    }
                }
            }
            ExpressionKind::Object(props) => {
                for prop in props {
                    match prop {
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            value,
                            ..
                        } => {
                            changed |= self.process_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            ..
                        } => {
                            changed |= self.process_expression(key);
                            changed |= self.process_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value, ..
                        } => {
                            changed |= self.process_expression(value);
                        }
                    }
                }
            }
            ExpressionKind::Parenthesized(inner) => {
                changed |= self.process_expression(inner);
            }
            ExpressionKind::TypeAssertion(expr, _) => {
                changed |= self.process_expression(expr);
            }
            _ => {}
        }

        // Then apply visitor transformation
        changed |= self.visit_expr(expr);

        changed
    }
}

impl Default for OperatorInliningPass {
    #[allow(clippy::arc_with_non_send_sync)]
    fn default() -> Self {
        Self::new(Arc::new(StringInterner::new()))
    }
}

fn binary_op_to_operator_kind(
    op: &BinaryOp,
) -> Option<typedlua_parser::ast::statement::OperatorKind> {
    match op {
        BinaryOp::Add => Some(typedlua_parser::ast::statement::OperatorKind::Add),
        BinaryOp::Subtract => Some(typedlua_parser::ast::statement::OperatorKind::Subtract),
        BinaryOp::Multiply => Some(typedlua_parser::ast::statement::OperatorKind::Multiply),
        BinaryOp::Divide => Some(typedlua_parser::ast::statement::OperatorKind::Divide),
        BinaryOp::Modulo => Some(typedlua_parser::ast::statement::OperatorKind::Modulo),
        BinaryOp::Power => Some(typedlua_parser::ast::statement::OperatorKind::Power),
        BinaryOp::Concatenate => Some(typedlua_parser::ast::statement::OperatorKind::Concatenate),
        BinaryOp::Equal => Some(typedlua_parser::ast::statement::OperatorKind::Equal),
        BinaryOp::NotEqual => Some(typedlua_parser::ast::statement::OperatorKind::NotEqual),
        BinaryOp::LessThan => Some(typedlua_parser::ast::statement::OperatorKind::LessThan),
        BinaryOp::LessThanOrEqual => {
            Some(typedlua_parser::ast::statement::OperatorKind::LessThanOrEqual)
        }
        BinaryOp::GreaterThan => Some(typedlua_parser::ast::statement::OperatorKind::GreaterThan),
        BinaryOp::GreaterThanOrEqual => {
            Some(typedlua_parser::ast::statement::OperatorKind::GreaterThanOrEqual)
        }
        BinaryOp::BitwiseAnd => Some(typedlua_parser::ast::statement::OperatorKind::BitwiseAnd),
        BinaryOp::BitwiseOr => Some(typedlua_parser::ast::statement::OperatorKind::BitwiseOr),
        BinaryOp::BitwiseXor => Some(typedlua_parser::ast::statement::OperatorKind::BitwiseXor),
        BinaryOp::ShiftLeft => Some(typedlua_parser::ast::statement::OperatorKind::ShiftLeft),
        BinaryOp::ShiftRight => Some(typedlua_parser::ast::statement::OperatorKind::ShiftRight),
        BinaryOp::IntegerDivide => Some(typedlua_parser::ast::statement::OperatorKind::FloorDivide),
        _ => None,
    }
}

fn unary_op_to_operator_kind(
    op: &UnaryOp,
) -> Option<typedlua_parser::ast::statement::OperatorKind> {
    match op {
        UnaryOp::Negate => Some(typedlua_parser::ast::statement::OperatorKind::UnaryMinus),
        UnaryOp::Length => Some(typedlua_parser::ast::statement::OperatorKind::Length),
        _ => None,
    }
}

fn operator_kind_to_metamethod_name(op: typedlua_parser::ast::statement::OperatorKind) -> String {
    match op {
        typedlua_parser::ast::statement::OperatorKind::Add => "__add",
        typedlua_parser::ast::statement::OperatorKind::Subtract => "__sub",
        typedlua_parser::ast::statement::OperatorKind::Multiply => "__mul",
        typedlua_parser::ast::statement::OperatorKind::Divide => "__div",
        typedlua_parser::ast::statement::OperatorKind::Modulo => "__mod",
        typedlua_parser::ast::statement::OperatorKind::Power => "__pow",
        typedlua_parser::ast::statement::OperatorKind::Concatenate => "__concat",
        typedlua_parser::ast::statement::OperatorKind::FloorDivide => "__idiv",
        typedlua_parser::ast::statement::OperatorKind::Equal => "__eq",
        typedlua_parser::ast::statement::OperatorKind::NotEqual => "__eq",
        typedlua_parser::ast::statement::OperatorKind::LessThan => "__lt",
        typedlua_parser::ast::statement::OperatorKind::LessThanOrEqual => "__le",
        typedlua_parser::ast::statement::OperatorKind::GreaterThan => "__lt",
        typedlua_parser::ast::statement::OperatorKind::GreaterThanOrEqual => "__le",
        typedlua_parser::ast::statement::OperatorKind::BitwiseAnd => "__band",
        typedlua_parser::ast::statement::OperatorKind::BitwiseOr => "__bor",
        typedlua_parser::ast::statement::OperatorKind::BitwiseXor => "__bxor",
        typedlua_parser::ast::statement::OperatorKind::ShiftLeft => "__shl",
        typedlua_parser::ast::statement::OperatorKind::ShiftRight => "__shr",
        typedlua_parser::ast::statement::OperatorKind::Index => "__index",
        typedlua_parser::ast::statement::OperatorKind::NewIndex => "__newindex",
        typedlua_parser::ast::statement::OperatorKind::Call => "__call",
        typedlua_parser::ast::statement::OperatorKind::UnaryMinus => "__unm",
        typedlua_parser::ast::statement::OperatorKind::Length => "__len",
    }
    .to_string()
}

fn get_class_from_type(t: &Type) -> Option<StringId> {
    match &t.kind {
        TypeKind::Reference(type_ref) => Some(type_ref.name.node),
        _ => None,
    }
}

fn count_statements(block: &Block) -> usize {
    block.statements.len()
}

fn has_side_effects(block: &Block) -> bool {
    for stmt in &block.statements {
        if statement_has_side_effects(stmt) {
            return true;
        }
    }
    false
}

fn statement_has_side_effects(stmt: &Statement) -> bool {
    match stmt {
        Statement::Expression(expr) => expression_has_side_effects(expr),
        Statement::Variable(decl) => expression_has_side_effects(&decl.initializer),
        Statement::Return(return_stmt) => {
            return_stmt.values.iter().any(expression_has_side_effects)
        }
        Statement::If(if_stmt) => {
            expression_has_side_effects(&if_stmt.condition)
                || block_has_side_effects(&if_stmt.then_block)
                || if_stmt.else_ifs.iter().any(|ei| {
                    expression_has_side_effects(&ei.condition) || block_has_side_effects(&ei.block)
                })
                || if_stmt
                    .else_block
                    .as_ref()
                    .map(block_has_side_effects)
                    .unwrap_or(false)
        }
        Statement::While(while_stmt) => {
            expression_has_side_effects(&while_stmt.condition)
                || block_has_side_effects(&while_stmt.body)
        }
        _ => true,
    }
}

fn block_has_side_effects(block: &Block) -> bool {
    block.statements.iter().any(statement_has_side_effects)
}

fn expression_has_side_effects(expr: &Expression) -> bool {
    match &expr.kind {
        ExpressionKind::Call(_, args, _) => args
            .iter()
            .any(|arg| expression_has_side_effects(&arg.value)),
        ExpressionKind::MethodCall(_, _, args, _) => args
            .iter()
            .any(|arg| expression_has_side_effects(&arg.value)),
        ExpressionKind::Assignment(left, _, right) => {
            expression_has_side_effects(left) || expression_has_side_effects(right)
        }
        ExpressionKind::Binary(_, left, right) => {
            expression_has_side_effects(left) || expression_has_side_effects(right)
        }
        ExpressionKind::Unary(_, operand) => expression_has_side_effects(operand),
        ExpressionKind::Conditional(cond, then_expr, else_expr) => {
            expression_has_side_effects(cond)
                || expression_has_side_effects(then_expr)
                || expression_has_side_effects(else_expr)
        }
        ExpressionKind::Pipe(left, right) => {
            expression_has_side_effects(left) || expression_has_side_effects(right)
        }
        ExpressionKind::Match(match_expr) => {
            expression_has_side_effects(&match_expr.value)
                || match_expr.arms.iter().any(|arm| match &arm.body {
                    typedlua_parser::ast::expression::MatchArmBody::Expression(e) => {
                        expression_has_side_effects(e)
                    }
                    typedlua_parser::ast::expression::MatchArmBody::Block(b) => {
                        block_has_side_effects(b)
                    }
                })
        }
        ExpressionKind::Arrow(arrow) => {
            arrow.parameters.iter().any(|p| {
                p.default
                    .as_ref()
                    .map(expression_has_side_effects)
                    .unwrap_or(false)
            }) || match &arrow.body {
                typedlua_parser::ast::expression::ArrowBody::Expression(e) => {
                    expression_has_side_effects(e)
                }
                typedlua_parser::ast::expression::ArrowBody::Block(b) => block_has_side_effects(b),
            }
        }
        ExpressionKind::New(callee, args, _) => {
            expression_has_side_effects(callee)
                || args
                    .iter()
                    .any(|arg| expression_has_side_effects(&arg.value))
        }
        ExpressionKind::Try(try_expr) => {
            expression_has_side_effects(&try_expr.expression)
                || expression_has_side_effects(&try_expr.catch_expression)
        }
        ExpressionKind::ErrorChain(left, right) => {
            expression_has_side_effects(left) || expression_has_side_effects(right)
        }
        ExpressionKind::OptionalCall(obj, args, _) => {
            expression_has_side_effects(obj)
                || args
                    .iter()
                    .any(|arg| expression_has_side_effects(&arg.value))
        }
        ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
            expression_has_side_effects(obj)
                || args
                    .iter()
                    .any(|arg| expression_has_side_effects(&arg.value))
        }
        ExpressionKind::Function(_) => false,
        ExpressionKind::Literal(_)
        | ExpressionKind::Identifier(_)
        | ExpressionKind::SelfKeyword => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use typedlua_parser::ast::types::{PrimitiveType, Type, TypeKind};

    #[test]
    fn test_operator_catalog_build() {
        let interner = Arc::new(StringInterner::new());
        let mut pass = OperatorInliningPass::new(interner);

        let program = Program::new(vec![], Span::dummy());
        pass.build_operator_catalog(&program);

        assert!(pass.operator_catalog.is_empty());
    }

    #[test]
    fn test_binary_op_to_operator_kind() {
        assert_eq!(
            binary_op_to_operator_kind(&BinaryOp::Add),
            Some(typedlua_parser::ast::statement::OperatorKind::Add)
        );
        assert_eq!(
            binary_op_to_operator_kind(&BinaryOp::Subtract),
            Some(typedlua_parser::ast::statement::OperatorKind::Subtract)
        );
        assert_eq!(
            binary_op_to_operator_kind(&BinaryOp::Multiply),
            Some(typedlua_parser::ast::statement::OperatorKind::Multiply)
        );
        assert_eq!(binary_op_to_operator_kind(&BinaryOp::And), None);
    }

    #[test]
    fn test_operator_kind_to_metamethod_name() {
        assert_eq!(
            operator_kind_to_metamethod_name(typedlua_parser::ast::statement::OperatorKind::Add),
            "__add"
        );
        assert_eq!(
            operator_kind_to_metamethod_name(
                typedlua_parser::ast::statement::OperatorKind::Multiply
            ),
            "__mul"
        );
    }

    #[test]
    fn test_get_class_from_type() {
        let type_id = StringId::from_u32(1);
        let ref_type = Type::new(
            TypeKind::Reference(typedlua_parser::ast::types::TypeReference {
                name: typedlua_parser::ast::Spanned::new(type_id, Span::dummy()),
                type_arguments: None,
                span: Span::dummy(),
            }),
            Span::dummy(),
        );

        let result = get_class_from_type(&ref_type);
        assert_eq!(result, Some(type_id));

        let primitive_type = Type::new(TypeKind::Primitive(PrimitiveType::Number), Span::dummy());
        let result = get_class_from_type(&primitive_type);
        assert!(result.is_none());
    }

    #[test]
    fn test_count_statements() {
        let block = Block {
            statements: vec![
                Statement::Return(typedlua_parser::ast::statement::ReturnStatement {
                    values: vec![],
                    span: Span::dummy(),
                }),
                Statement::Return(typedlua_parser::ast::statement::ReturnStatement {
                    values: vec![],
                    span: Span::dummy(),
                }),
            ],
            span: Span::dummy(),
        };

        assert_eq!(count_statements(&block), 2);
    }

    #[test]
    fn test_has_side_effects() {
        let block = Block {
            statements: vec![Statement::Expression(Expression::new(
                ExpressionKind::Identifier(StringId::from_u32(1)),
                Span::dummy(),
            ))],
            span: Span::dummy(),
        };

        assert!(!has_side_effects(&block));
    }
}
