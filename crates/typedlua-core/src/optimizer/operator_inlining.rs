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
use crate::MutableProgram;

use crate::optimizer::{ExprVisitor, PreAnalysisPass, WholeProgramPass};
use bumpalo::Bump;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use typedlua_parser::ast::expression::{BinaryOp, Expression, ExpressionKind, UnaryOp};
use typedlua_parser::ast::statement::{Block, ClassMember, ForStatement, Statement};
use typedlua_parser::ast::types::{Type, TypeKind};
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

    fn build_operator_catalog<'arena>(&mut self, program: &MutableProgram<'arena>) {
        for stmt in &program.statements {
            self.catalog_statement(stmt);
        }
    }

    fn catalog_statement<'arena>(&mut self, stmt: &Statement<'arena>) {
        match stmt {
            Statement::Class(class) => {
                let class_name = class.name.node;
                for member in class.members.iter() {
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
                for else_if in if_stmt.else_ifs.iter() {
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
                        for expr in for_gen.iterators.iter() {
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
                for expr in return_stmt.values.iter() {
                    self.catalog_expression(expr);
                }
            }
            Statement::Block(block) => self.catalog_block(block),
            Statement::Try(try_stmt) => {
                self.catalog_block(&try_stmt.try_block);
                for clause in try_stmt.catch_clauses.iter() {
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

    fn catalog_block<'arena>(&mut self, block: &Block<'arena>) {
        for stmt in block.statements.iter() {
            self.catalog_statement(stmt);
        }
    }

    fn catalog_expression<'arena>(&mut self, expr: &Expression<'arena>) {
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
                for arg in args.iter() {
                    self.catalog_expression(&arg.value);
                }
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.catalog_expression(obj);
                for arg in args.iter() {
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
                for arm in match_expr.arms.iter() {
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
                for param in arrow.parameters.iter() {
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
                for arg in args.iter() {
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
                for arg in args.iter() {
                    self.catalog_expression(&arg.value);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.catalog_expression(obj);
                for arg in args.iter() {
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
                for elem in elements.iter() {
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
                for prop in props.iter() {
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

    fn count_operator_call<'arena>(
        &mut self,
        op: &BinaryOp,
        left: &Expression<'arena>,
        _right: &Expression<'arena>,
    ) {
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

    fn count_unary_operator_call<'arena>(
        &mut self,
        op: &UnaryOp,
        operand: &Expression<'arena>,
    ) {
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

    fn convert_operator_call<'arena>(
        &self,
        op: &BinaryOp,
        left: &Expression<'arena>,
        right: &Expression<'arena>,
        span: Span,
        arena: &'arena Bump,
    ) -> Option<ExpressionKind<'arena>> {
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
                arena.alloc(Expression {
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

        let args = arena.alloc_slice_clone(&[
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
        ]);

        Some(ExpressionKind::Call(arena.alloc(func_expr), args, None))
    }
}

impl<'arena> ExprVisitor<'arena> for OperatorInliningPass {
    fn visit_expr(&mut self, expr: &mut Expression<'arena>, arena: &'arena Bump) -> bool {
        // Only process binary operations (the main transformation target)
        if let ExpressionKind::Binary(op, left, right) = &expr.kind {
            let op = *op;
            let left_clone = (*left).clone();
            let right_clone = (*right).clone();
            if let Some(new_kind) =
                self.convert_operator_call(&op, &left_clone, &right_clone, expr.span, arena)
            {
                expr.kind = new_kind;
                return true;
            }
        }
        false
    }
}

impl<'arena> PreAnalysisPass<'arena> for OperatorInliningPass {
    fn analyze(&mut self, program: &MutableProgram<'arena>) {
        self.build_operator_catalog(program);
    }
}

impl<'arena> WholeProgramPass<'arena> for OperatorInliningPass {
    fn name(&self) -> &'static str {
        "operator-inlining"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(
        &mut self,
        program: &mut MutableProgram<'arena>,
        arena: &'arena Bump,
    ) -> Result<bool, String> {
        // Run analysis phase
        self.analyze(program);

        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.process_statement(stmt, arena);
        }

        Ok(changed)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl OperatorInliningPass {
    fn process_statement<'arena>(
        &mut self,
        stmt: &mut Statement<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        let mut changed = false;

        match stmt {
            Statement::Function(func) => {
                changed |= self.process_block(&mut func.body, arena);
            }
            Statement::If(if_stmt) => {
                changed |= self.visit_expr(&mut if_stmt.condition, arena);
                changed |= self.process_block(&mut if_stmt.then_block, arena);
                let mut new_else_ifs: Vec<_> = if_stmt.else_ifs.to_vec();
                let mut eic = false;
                for else_if in &mut new_else_ifs {
                    eic |= self.visit_expr(&mut else_if.condition, arena);
                    eic |= self.process_block(&mut else_if.block, arena);
                }
                if eic {
                    if_stmt.else_ifs = arena.alloc_slice_clone(&new_else_ifs);
                    changed = true;
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.process_block(else_block, arena);
                }
            }
            Statement::While(while_stmt) => {
                changed |= self.visit_expr(&mut while_stmt.condition, arena);
                changed |= self.process_block(&mut while_stmt.body, arena);
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
                        fc |= self.process_block(&mut new_num.body, arena);
                        if fc {
                            *stmt = Statement::For(
                                arena.alloc(ForStatement::Numeric(arena.alloc(new_num))),
                            );
                            changed = true;
                        }
                    }
                    ForStatement::Generic(for_gen_ref) => {
                        let mut new_gen = for_gen_ref.clone();
                        let mut fc = false;
                        let mut new_iters: Vec<_> = new_gen.iterators.to_vec();
                        for expr in &mut new_iters {
                            fc |= self.process_expression(expr, arena);
                        }
                        if fc {
                            new_gen.iterators = arena.alloc_slice_clone(&new_iters);
                        }
                        fc |= self.process_block(&mut new_gen.body, arena);
                        if fc {
                            *stmt = Statement::For(
                                arena.alloc(ForStatement::Generic(new_gen)),
                            );
                            changed = true;
                        }
                    }
                }
            }
            Statement::Repeat(repeat_stmt) => {
                changed |= self.visit_expr(&mut repeat_stmt.until, arena);
                changed |= self.process_block(&mut repeat_stmt.body, arena);
            }
            Statement::Return(return_stmt) => {
                let mut values: Vec<Expression<'arena>> = return_stmt.values.to_vec();
                let mut ret_changed = false;
                for value in &mut values {
                    ret_changed |= self.visit_expr(value, arena);
                }
                if ret_changed {
                    return_stmt.values = arena.alloc_slice_clone(&values);
                    changed = true;
                }
            }
            Statement::Expression(expr) => {
                changed |= self.process_expression(expr, arena);
            }
            Statement::Block(block) => {
                changed |= self.process_block(block, arena);
            }
            Statement::Try(try_stmt) => {
                changed |= self.process_block(&mut try_stmt.try_block, arena);
                let mut new_clauses: Vec<_> = try_stmt.catch_clauses.to_vec();
                let mut cc = false;
                for clause in &mut new_clauses {
                    cc |= self.process_block(&mut clause.body, arena);
                }
                if cc {
                    try_stmt.catch_clauses = arena.alloc_slice_clone(&new_clauses);
                    changed = true;
                }
                if let Some(finally) = &mut try_stmt.finally_block {
                    changed |= self.process_block(finally, arena);
                }
            }
            _ => {}
        }

        changed
    }

    fn process_block<'arena>(
        &mut self,
        block: &mut Block<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        let mut stmts: Vec<Statement<'arena>> = block.statements.to_vec();
        let mut changed = false;
        for stmt in &mut stmts {
            changed |= self.process_statement(stmt, arena);
        }
        if changed {
            block.statements = arena.alloc_slice_clone(&stmts);
        }
        changed
    }

    fn process_expression<'arena>(
        &mut self,
        expr: &mut Expression<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        let mut changed = false;

        // Visit children first (clone-and-rebuild for &'arena refs)
        match &expr.kind {
            ExpressionKind::Binary(op, left, right) => {
                let op = *op;
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                let lc = self.process_expression(&mut new_left, arena);
                let rc = self.process_expression(&mut new_right, arena);
                if lc || rc {
                    expr.kind =
                        ExpressionKind::Binary(op, arena.alloc(new_left), arena.alloc(new_right));
                    changed = true;
                }
            }
            ExpressionKind::Call(func, args, type_args) => {
                let type_args = *type_args;
                let mut new_func = (**func).clone();
                let fc = self.process_expression(&mut new_func, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.process_expression(&mut arg.value, arena);
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
                let oc = self.process_expression(&mut new_obj, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.process_expression(&mut arg.value, arena);
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
            ExpressionKind::Unary(op, operand) => {
                let op = *op;
                let mut new_operand = (**operand).clone();
                if self.process_expression(&mut new_operand, arena) {
                    expr.kind = ExpressionKind::Unary(op, arena.alloc(new_operand));
                    changed = true;
                }
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut new_cond = (**cond).clone();
                let mut new_then = (**then_expr).clone();
                let mut new_else = (**else_expr).clone();
                let cc = self.process_expression(&mut new_cond, arena);
                let tc = self.process_expression(&mut new_then, arena);
                let ec = self.process_expression(&mut new_else, arena);
                if cc || tc || ec {
                    expr.kind = ExpressionKind::Conditional(
                        arena.alloc(new_cond),
                        arena.alloc(new_then),
                        arena.alloc(new_else),
                    );
                    changed = true;
                }
            }
            ExpressionKind::Pipe(left, right) => {
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                let lc = self.process_expression(&mut new_left, arena);
                let rc = self.process_expression(&mut new_right, arena);
                if lc || rc {
                    expr.kind =
                        ExpressionKind::Pipe(arena.alloc(new_left), arena.alloc(new_right));
                    changed = true;
                }
            }
            ExpressionKind::Match(match_expr) => {
                let mut new_match = match_expr.clone();
                let mut new_value = (*match_expr.value).clone();
                let vc = self.process_expression(&mut new_value, arena);
                if vc {
                    new_match.value = arena.alloc(new_value);
                }
                let mut new_arms: Vec<_> = match_expr.arms.to_vec();
                let mut ac = false;
                for arm in &mut new_arms {
                    match &mut arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(e) => {
                            let mut new_e = (**e).clone();
                            if self.process_expression(&mut new_e, arena) {
                                arm.body = typedlua_parser::ast::expression::MatchArmBody::Expression(arena.alloc(new_e));
                                ac = true;
                            }
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            let mut new_block = block.clone();
                            if self.process_block(&mut new_block, arena) {
                                arm.body = typedlua_parser::ast::expression::MatchArmBody::Block(new_block);
                                ac = true;
                            }
                        }
                    }
                }
                if vc || ac {
                    new_match.arms = arena.alloc_slice_clone(&new_arms);
                    expr.kind = ExpressionKind::Match(new_match);
                    changed = true;
                }
            }
            ExpressionKind::Arrow(arrow) => {
                let mut new_arrow = arrow.clone();
                let mut new_params: Vec<_> = arrow.parameters.to_vec();
                let mut pc = false;
                for param in &mut new_params {
                    if let Some(default) = &mut param.default {
                        pc |= self.process_expression(default, arena);
                    }
                }
                if pc {
                    new_arrow.parameters = arena.alloc_slice_clone(&new_params);
                }
                let mut bc = false;
                match &mut new_arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(e) => {
                        let mut new_e = (**e).clone();
                        if self.process_expression(&mut new_e, arena) {
                            new_arrow.body = typedlua_parser::ast::expression::ArrowBody::Expression(arena.alloc(new_e));
                            bc = true;
                        }
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        bc |= self.process_block(block, arena);
                    }
                }
                if pc || bc {
                    expr.kind = ExpressionKind::Arrow(new_arrow);
                    changed = true;
                }
            }
            ExpressionKind::New(callee, args, type_args) => {
                let type_args = *type_args;
                let mut new_callee = (**callee).clone();
                let cc = self.process_expression(&mut new_callee, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.process_expression(&mut arg.value, arena);
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
            ExpressionKind::Try(try_expr) => {
                let mut new_expr = (*try_expr.expression).clone();
                let mut new_catch = (*try_expr.catch_expression).clone();
                let ec = self.process_expression(&mut new_expr, arena);
                let cc = self.process_expression(&mut new_catch, arena);
                if ec || cc {
                    expr.kind = ExpressionKind::Try(typedlua_parser::ast::expression::TryExpression {
                        expression: arena.alloc(new_expr),
                        catch_variable: try_expr.catch_variable.clone(),
                        catch_expression: arena.alloc(new_catch),
                        span: try_expr.span,
                    });
                    changed = true;
                }
            }
            ExpressionKind::ErrorChain(left, right) => {
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                let lc = self.process_expression(&mut new_left, arena);
                let rc = self.process_expression(&mut new_right, arena);
                if lc || rc {
                    expr.kind =
                        ExpressionKind::ErrorChain(arena.alloc(new_left), arena.alloc(new_right));
                    changed = true;
                }
            }
            ExpressionKind::OptionalMember(obj, member) => {
                let member = member.clone();
                let mut new_obj = (**obj).clone();
                if self.process_expression(&mut new_obj, arena) {
                    expr.kind = ExpressionKind::OptionalMember(arena.alloc(new_obj), member);
                    changed = true;
                }
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                let mut new_obj = (**obj).clone();
                let mut new_index = (**index).clone();
                let oc = self.process_expression(&mut new_obj, arena);
                let ic = self.process_expression(&mut new_index, arena);
                if oc || ic {
                    expr.kind =
                        ExpressionKind::OptionalIndex(arena.alloc(new_obj), arena.alloc(new_index));
                    changed = true;
                }
            }
            ExpressionKind::OptionalCall(obj, args, type_args) => {
                let type_args = *type_args;
                let mut new_obj = (**obj).clone();
                let oc = self.process_expression(&mut new_obj, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.process_expression(&mut arg.value, arena);
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
                let oc = self.process_expression(&mut new_obj, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.process_expression(&mut arg.value, arena);
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
            ExpressionKind::Assignment(left, op, right) => {
                let op = *op;
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                let lc = self.process_expression(&mut new_left, arena);
                let rc = self.process_expression(&mut new_right, arena);
                if lc || rc {
                    expr.kind = ExpressionKind::Assignment(
                        arena.alloc(new_left),
                        op,
                        arena.alloc(new_right),
                    );
                    changed = true;
                }
            }
            ExpressionKind::Member(obj, member) => {
                let member = member.clone();
                let mut new_obj = (**obj).clone();
                if self.process_expression(&mut new_obj, arena) {
                    expr.kind = ExpressionKind::Member(arena.alloc(new_obj), member);
                    changed = true;
                }
            }
            ExpressionKind::Index(obj, index) => {
                let mut new_obj = (**obj).clone();
                let mut new_index = (**index).clone();
                let oc = self.process_expression(&mut new_obj, arena);
                let ic = self.process_expression(&mut new_index, arena);
                if oc || ic {
                    expr.kind =
                        ExpressionKind::Index(arena.alloc(new_obj), arena.alloc(new_index));
                    changed = true;
                }
            }
            ExpressionKind::Array(elements) => {
                let mut new_elements: Vec<_> = elements.to_vec();
                let mut ec = false;
                for elem in &mut new_elements {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(e) => {
                            ec |= self.process_expression(e, arena);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(e) => {
                            ec |= self.process_expression(e, arena);
                        }
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
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            key,
                            value,
                            span,
                        } => {
                            let mut new_val = (**value).clone();
                            if self.process_expression(&mut new_val, arena) {
                                *prop = typedlua_parser::ast::expression::ObjectProperty::Property {
                                    key: key.clone(),
                                    value: arena.alloc(new_val),
                                    span: *span,
                                };
                                pc = true;
                            }
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            span,
                        } => {
                            let mut new_key = (**key).clone();
                            let mut new_val = (**value).clone();
                            let kc = self.process_expression(&mut new_key, arena);
                            let vc = self.process_expression(&mut new_val, arena);
                            if kc || vc {
                                *prop = typedlua_parser::ast::expression::ObjectProperty::Computed {
                                    key: arena.alloc(new_key),
                                    value: arena.alloc(new_val),
                                    span: *span,
                                };
                                pc = true;
                            }
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value,
                            span,
                        } => {
                            let mut new_val = (**value).clone();
                            if self.process_expression(&mut new_val, arena) {
                                *prop = typedlua_parser::ast::expression::ObjectProperty::Spread {
                                    value: arena.alloc(new_val),
                                    span: *span,
                                };
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
            ExpressionKind::Parenthesized(inner) => {
                let mut new_inner = (**inner).clone();
                if self.process_expression(&mut new_inner, arena) {
                    expr.kind = ExpressionKind::Parenthesized(arena.alloc(new_inner));
                    changed = true;
                }
            }
            ExpressionKind::TypeAssertion(inner, ty) => {
                let ty = ty.clone();
                let mut new_inner = (**inner).clone();
                if self.process_expression(&mut new_inner, arena) {
                    expr.kind = ExpressionKind::TypeAssertion(arena.alloc(new_inner), ty);
                    changed = true;
                }
            }
            _ => {}
        }

        // Then apply visitor transformation
        changed |= self.visit_expr(expr, arena);

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

fn get_class_from_type(t: &Type<'_>) -> Option<StringId> {
    match &t.kind {
        TypeKind::Reference(type_ref) => Some(type_ref.name.node),
        _ => None,
    }
}

fn count_statements(block: &Block<'_>) -> usize {
    block.statements.len()
}

fn has_side_effects(block: &Block<'_>) -> bool {
    for stmt in block.statements.iter() {
        if statement_has_side_effects(stmt) {
            return true;
        }
    }
    false
}

fn statement_has_side_effects(stmt: &Statement<'_>) -> bool {
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

fn block_has_side_effects(block: &Block<'_>) -> bool {
    block.statements.iter().any(statement_has_side_effects)
}

fn expression_has_side_effects(expr: &Expression<'_>) -> bool {
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
    use bumpalo::Bump;
    use typedlua_parser::ast::types::{PrimitiveType, Type, TypeKind};

    #[test]
    fn test_operator_catalog_build() {
        let arena = Bump::new();
        let interner = Arc::new(StringInterner::new());
        let mut pass = OperatorInliningPass::new(interner);

        let program = MutableProgram {
            statements: vec![],
            span: Span::dummy(),
        };
        pass.build_operator_catalog(&program);

        assert!(pass.operator_catalog.is_empty());
        let _ = &arena; // keep arena alive
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
        let arena = Bump::new();
        let block = Block {
            statements: arena.alloc_slice_clone(&[
                Statement::Return(typedlua_parser::ast::statement::ReturnStatement {
                    values: &[],
                    span: Span::dummy(),
                }),
                Statement::Return(typedlua_parser::ast::statement::ReturnStatement {
                    values: &[],
                    span: Span::dummy(),
                }),
            ]),
            span: Span::dummy(),
        };

        assert_eq!(count_statements(&block), 2);
    }

    #[test]
    fn test_has_side_effects() {
        let arena = Bump::new();
        let block = Block {
            statements: arena.alloc_slice_clone(&[Statement::Expression(Expression::new(
                ExpressionKind::Identifier(StringId::from_u32(1)),
                Span::dummy(),
            ))]),
            span: Span::dummy(),
        };

        assert!(!has_side_effects(&block));
    }
}
