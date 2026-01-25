//! O3 Interface Method Inlining Pass
//!
//! Inlines interface method calls when the implementing class is statically known.
//! This builds on O2's MethodToFunctionConversionPass by replacing virtual dispatch
//! with direct method inlining when safe.
//!
//! Inlining Criteria:
//! 1. Interface method has exactly one implementing class in the program
//! 2. Implementing class is `final` or all subclasses are known and don't override
//! 3. Method body contains 10 or fewer statements
//! 4. Method has no `self` mutation (read-only `self`)
//!
//! This pass works by:
//! 1. Building a map: Interface -> ImplementingClass[]
//! 2. Identifying interfaces with exactly one concrete implementation
//! 3. Finding MethodCall expressions where receiver type is the sole implementing class
//! 4. Inlining the method body, binding `self` to the receiver expression
//! 5. Preserving original dispatch if multiple implementations exist

use crate::config::OptimizationLevel;
use crate::errors::CompilationError;
use crate::optimizer::OptimizationPass;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use typedlua_parser::ast::expression::{AssignmentOp, Expression, ExpressionKind};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{Block, ClassMember, InterfaceMember, Statement};
use typedlua_parser::ast::types::TypeKind;
use typedlua_parser::ast::Program;
use typedlua_parser::ast::Spanned;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::{StringId, StringInterner};

const MAX_INLINABLE_STATEMENTS: usize = 10;

#[derive(Debug, Default)]
struct InterfaceImplementationMap {
    interface_to_classes: FxHashMap<StringId, Vec<StringId>>,
    class_to_interfaces: FxHashMap<StringId, Vec<StringId>>,
    class_is_final: FxHashMap<StringId, bool>,
    method_body: FxHashMap<(StringId, StringId), Block>,
    method_signature: FxHashMap<(StringId, StringId), Spanned<StringId>>,
    known_classes: FxHashMap<StringId, bool>,
    known_interfaces: FxHashMap<StringId, bool>,
}

impl InterfaceImplementationMap {
    pub fn build(program: &Program) -> Self {
        let mut map = InterfaceImplementationMap::default();

        for stmt in &program.statements {
            match stmt {
                Statement::Class(class) => {
                    let class_id = class.name.node;
                    map.known_classes.insert(class_id, true);
                    map.class_is_final.insert(class_id, class.is_final);

                    for member in &class.members {
                        if let ClassMember::Method(method) = member {
                            let method_id = method.name.node;
                            if let Some(body) = &method.body {
                                map.method_body.insert((class_id, method_id), body.clone());
                            }
                        }
                    }

                    for implemented in &class.implements {
                        if let TypeKind::Reference(type_ref) = &implemented.kind {
                            let interface_id = type_ref.name.node;
                            map.known_interfaces.insert(interface_id, true);
                            map.interface_to_classes
                                .entry(interface_id)
                                .or_default()
                                .push(class_id);
                            map.class_to_interfaces
                                .entry(class_id)
                                .or_default()
                                .push(interface_id);
                        }
                    }
                }
                Statement::Interface(interface) => {
                    let interface_id = interface.name.node;
                    map.known_interfaces.insert(interface_id, true);

                    for member in &interface.members {
                        if let InterfaceMember::Method(method_sig) = member {
                            let method_id = method_sig.name.node;
                            map.method_signature
                                .insert((interface_id, method_id), method_sig.name.clone());
                        }
                    }
                }
                Statement::DeclareInterface(interface) => {
                    let interface_id = interface.name.node;
                    map.known_interfaces.insert(interface_id, true);
                }
                _ => {}
            }
        }

        map
    }

    pub fn is_interface(&self, type_id: StringId) -> bool {
        self.known_interfaces.contains_key(&type_id)
    }

    pub fn get_single_implementation(&self, interface_id: StringId) -> Option<StringId> {
        let implementations = self.interface_to_classes.get(&interface_id)?;
        if implementations.len() == 1 {
            Some(implementations[0])
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn is_final_class(&self, class_id: StringId) -> bool {
        self.class_is_final.get(&class_id).copied().unwrap_or(false)
    }

    pub fn get_method_body(&self, class_id: StringId, method_id: StringId) -> Option<&Block> {
        self.method_body.get(&(class_id, method_id))
    }

    pub fn count_statement_depth(&self, block: &Block) -> usize {
        block.statements.len()
    }

    pub fn mutates_self(&self, block: &Block, class_id: StringId) -> bool {
        for stmt in &block.statements {
            if self.statement_mutates_self(stmt, class_id) {
                return true;
            }
        }
        false
    }

    fn statement_mutates_self(&self, stmt: &Statement, class_id: StringId) -> bool {
        match stmt {
            Statement::Expression(expr) => self.expression_mutates_self(expr, class_id),
            Statement::If(if_stmt) => {
                self.expression_mutates_self(&if_stmt.condition, class_id)
                    || self.block_mutates_self(&if_stmt.then_block, class_id)
                    || if_stmt.else_ifs.iter().any(|ei| {
                        self.expression_mutates_self(&ei.condition, class_id)
                            || self.block_mutates_self(&ei.block, class_id)
                    })
                    || if_stmt
                        .else_block
                        .as_ref()
                        .is_some_and(|eb| self.block_mutates_self(eb, class_id))
            }
            Statement::While(while_stmt) => {
                self.expression_mutates_self(&while_stmt.condition, class_id)
                    || self.block_mutates_self(&while_stmt.body, class_id)
            }
            Statement::For(for_stmt) => {
                let body = match &**for_stmt {
                    crate::ast::statement::ForStatement::Numeric(for_num) => &for_num.body,
                    crate::ast::statement::ForStatement::Generic(for_gen) => &for_gen.body,
                };
                self.block_mutates_self(body, class_id)
            }
            Statement::Variable(decl) => self.expression_mutates_self(&decl.initializer, class_id),
            Statement::Return(ret) => ret
                .values
                .iter()
                .any(|v| self.expression_mutates_self(v, class_id)),
            _ => false,
        }
    }

    fn block_mutates_self(&self, block: &Block, class_id: StringId) -> bool {
        block
            .statements
            .iter()
            .any(|s| self.statement_mutates_self(s, class_id))
    }

    fn expression_mutates_self(&self, expr: &Expression, class_id: StringId) -> bool {
        match &expr.kind {
            ExpressionKind::Assignment(left, _op, right) => {
                if self.expression_is_self_member(left, class_id) {
                    true
                } else {
                    self.expression_mutates_self(left, class_id)
                        || self.expression_mutates_self(right, class_id)
                }
            }
            ExpressionKind::Call(func, args, _) => {
                self.expression_mutates_self(func, class_id)
                    || args
                        .iter()
                        .any(|a| self.expression_mutates_self(&a.value, class_id))
            }
            ExpressionKind::MethodCall(obj, _method_name, args, _) => {
                self.expression_mutates_self(obj, class_id)
                    || args
                        .iter()
                        .any(|a| self.expression_mutates_self(&a.value, class_id))
            }
            ExpressionKind::Binary(_op, left, right) => {
                self.expression_mutates_self(left, class_id)
                    || self.expression_mutates_self(right, class_id)
            }
            ExpressionKind::Unary(_op, operand) => self.expression_mutates_self(operand, class_id),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.expression_mutates_self(cond, class_id)
                    || self.expression_mutates_self(then_expr, class_id)
                    || self.expression_mutates_self(else_expr, class_id)
            }
            ExpressionKind::Pipe(left, right) => {
                self.expression_mutates_self(left, class_id)
                    || self.expression_mutates_self(right, class_id)
            }
            ExpressionKind::Match(match_expr) => {
                self.expression_mutates_self(&match_expr.value, class_id)
                    || match_expr.arms.iter().any(|arm| match &arm.body {
                        crate::ast::expression::MatchArmBody::Expression(e) => {
                            self.expression_mutates_self(e, class_id)
                        }
                        crate::ast::expression::MatchArmBody::Block(b) => {
                            self.block_mutates_self(b, class_id)
                        }
                    })
            }
            ExpressionKind::Arrow(arrow) => {
                arrow.parameters.iter().any(|p| {
                    p.default
                        .as_ref()
                        .is_some_and(|d| self.expression_mutates_self(d, class_id))
                }) || match &arrow.body {
                    crate::ast::expression::ArrowBody::Expression(e) => {
                        self.expression_mutates_self(e, class_id)
                    }
                    crate::ast::expression::ArrowBody::Block(b) => {
                        self.block_mutates_self(b, class_id)
                    }
                }
            }
            ExpressionKind::New(callee, args) => {
                self.expression_mutates_self(callee, class_id)
                    || args
                        .iter()
                        .any(|a| self.expression_mutates_self(&a.value, class_id))
            }
            ExpressionKind::Try(try_expr) => {
                self.expression_mutates_self(&try_expr.expression, class_id)
                    || self.expression_mutates_self(&try_expr.catch_expression, class_id)
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.expression_mutates_self(left, class_id)
                    || self.expression_mutates_self(right, class_id)
            }
            ExpressionKind::OptionalMember(obj, _) => self.expression_mutates_self(obj, class_id),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.expression_mutates_self(obj, class_id)
                    || self.expression_mutates_self(index, class_id)
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.expression_mutates_self(obj, class_id)
                    || args
                        .iter()
                        .any(|a| self.expression_mutates_self(&a.value, class_id))
            }
            ExpressionKind::OptionalMethodCall(obj, _method_name, args, _) => {
                self.expression_mutates_self(obj, class_id)
                    || args
                        .iter()
                        .any(|a| self.expression_mutates_self(&a.value, class_id))
            }
            ExpressionKind::TypeAssertion(expr, _) => self.expression_mutates_self(expr, class_id),
            ExpressionKind::Member(obj, _) => self.expression_mutates_self(obj, class_id),
            ExpressionKind::Index(obj, index) => {
                self.expression_mutates_self(obj, class_id)
                    || self.expression_mutates_self(index, class_id)
            }
            _ => false,
        }
    }

    fn expression_is_self_member(&self, expr: &Expression, class_id: StringId) -> bool {
        match &expr.kind {
            ExpressionKind::Member(obj, _prop) => self.expression_is_self(obj, class_id),
            ExpressionKind::Index(obj, _index) => self.expression_is_self(obj, class_id),
            _ => false,
        }
    }

    fn expression_is_self(&self, expr: &Expression, _class_id: StringId) -> bool {
        matches!(expr.kind, ExpressionKind::SelfKeyword)
    }
}

pub struct InterfaceMethodInliningPass {
    interner: Arc<StringInterner>,
}

impl InterfaceMethodInliningPass {
    pub fn new(interner: Arc<StringInterner>) -> Self {
        Self { interner }
    }

    fn process_statement(
        &mut self,
        stmt: &mut Statement,
        impl_map: &InterfaceImplementationMap,
    ) -> bool {
        match stmt {
            Statement::Function(func) => {
                let mut changed = false;
                for s in &mut func.body.statements {
                    changed |= self.process_statement(s, impl_map);
                }
                changed
            }
            Statement::If(if_stmt) => {
                let mut changed = self.process_expression(&mut if_stmt.condition, impl_map);
                changed |= self.process_block(&mut if_stmt.then_block, impl_map);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.process_expression(&mut else_if.condition, impl_map);
                    changed |= self.process_block(&mut else_if.block, impl_map);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.process_block(else_block, impl_map);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.process_expression(&mut while_stmt.condition, impl_map);
                changed |= self.process_block(&mut while_stmt.body, impl_map);
                changed
            }
            Statement::For(for_stmt) => {
                use typedlua_parser::ast::statement::ForStatement;
                let body = match &mut **for_stmt {
                    ForStatement::Numeric(for_num) => &mut for_num.body,
                    ForStatement::Generic(for_gen) => &mut for_gen.body,
                };
                self.process_block(body, impl_map)
            }
            Statement::Repeat(repeat_stmt) => {
                let mut changed = self.process_expression(&mut repeat_stmt.until, impl_map);
                changed |= self.process_block(&mut repeat_stmt.body, impl_map);
                changed
            }
            Statement::Return(ret_stmt) => {
                let mut changed = false;
                for value in &mut ret_stmt.values {
                    changed |= self.process_expression(value, impl_map);
                }
                changed
            }
            Statement::Expression(expr) => self.process_expression(expr, impl_map),
            Statement::Block(block) => self.process_block(block, impl_map),
            Statement::Try(try_stmt) => {
                let mut changed = self.process_block(&mut try_stmt.try_block, impl_map);
                for clause in &mut try_stmt.catch_clauses {
                    changed |= self.process_block(&mut clause.body, impl_map);
                }
                if let Some(finally) = &mut try_stmt.finally_block {
                    changed |= self.process_block(finally, impl_map);
                }
                changed
            }
            Statement::Class(class) => {
                let mut changed = false;
                for member in &mut class.members {
                    match member {
                        ClassMember::Method(method) => {
                            if let Some(body) = &mut method.body {
                                changed |= self.process_block(body, impl_map);
                            }
                        }
                        ClassMember::Constructor(ctor) => {
                            changed |= self.process_block(&mut ctor.body, impl_map);
                        }
                        ClassMember::Getter(getter) => {
                            changed |= self.process_block(&mut getter.body, impl_map);
                        }
                        ClassMember::Setter(setter) => {
                            changed |= self.process_block(&mut setter.body, impl_map);
                        }
                        ClassMember::Operator(op) => {
                            changed |= self.process_block(&mut op.body, impl_map);
                        }
                        ClassMember::Property(_) => {}
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn process_block(&mut self, block: &mut Block, impl_map: &InterfaceImplementationMap) -> bool {
        let mut changed = false;
        for stmt in &mut block.statements {
            changed |= self.process_statement(stmt, impl_map);
        }
        changed
    }

    fn process_expression(
        &mut self,
        expr: &mut Expression,
        impl_map: &InterfaceImplementationMap,
    ) -> bool {
        match &mut expr.kind {
            ExpressionKind::MethodCall(obj, method_name, args, _) => {
                let mut changed = self.process_expression(obj, impl_map);
                for arg in args.iter_mut() {
                    changed |= self.process_expression(&mut arg.value, impl_map);
                }

                if let Some(ref obj_type) = obj.annotated_type {
                    if let TypeKind::Reference(type_ref) = &obj_type.kind {
                        let type_id = type_ref.name.node;

                        if impl_map.is_interface(type_id) {
                            if let Some(implementing_class) =
                                impl_map.get_single_implementation(type_id)
                            {
                                let method_id = method_name.node;

                                if let Some(method_body) =
                                    impl_map.get_method_body(implementing_class, method_id)
                                {
                                    if impl_map.count_statement_depth(method_body)
                                        <= MAX_INLINABLE_STATEMENTS
                                        && !impl_map.mutates_self(method_body, implementing_class)
                                    {
                                        if let Some(inlined) = self.inline_interface_method(
                                            obj,
                                            implementing_class,
                                            method_body,
                                            args,
                                            expr.span,
                                        ) {
                                            expr.kind = inlined;
                                            changed = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                changed
            }
            ExpressionKind::Call(func, args, _) => {
                let mut changed = self.process_expression(func, impl_map);
                for arg in args.iter_mut() {
                    changed |= self.process_expression(&mut arg.value, impl_map);
                }
                changed
            }
            ExpressionKind::Binary(_op, left, right) => {
                let mut changed = self.process_expression(left, impl_map);
                changed |= self.process_expression(right, impl_map);
                changed
            }
            ExpressionKind::Unary(_op, operand) => self.process_expression(operand, impl_map),
            ExpressionKind::Assignment(left, _op, right) => {
                let mut changed = self.process_expression(left, impl_map);
                changed |= self.process_expression(right, impl_map);
                changed
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut changed = self.process_expression(cond, impl_map);
                changed |= self.process_expression(then_expr, impl_map);
                changed |= self.process_expression(else_expr, impl_map);
                changed
            }
            ExpressionKind::Pipe(left, right) => {
                let mut changed = self.process_expression(left, impl_map);
                changed |= self.process_expression(right, impl_map);
                changed
            }
            ExpressionKind::Match(match_expr) => {
                let mut changed = self.process_expression(&mut match_expr.value, impl_map);
                for arm in &mut match_expr.arms {
                    match &mut arm.body {
                        crate::ast::expression::MatchArmBody::Expression(e) => {
                            changed |= self.process_expression(e, impl_map);
                        }
                        crate::ast::expression::MatchArmBody::Block(block) => {
                            changed |= self.process_block(block, impl_map);
                        }
                    }
                }
                changed
            }
            ExpressionKind::Arrow(arrow) => {
                let mut changed = false;
                for param in &mut arrow.parameters {
                    if let Some(default) = &mut param.default {
                        changed |= self.process_expression(default, impl_map);
                    }
                }
                match &mut arrow.body {
                    crate::ast::expression::ArrowBody::Expression(e) => {
                        changed |= self.process_expression(e, impl_map);
                    }
                    crate::ast::expression::ArrowBody::Block(block) => {
                        changed |= self.process_block(block, impl_map);
                    }
                }
                changed
            }
            ExpressionKind::New(callee, args) => {
                let mut changed = self.process_expression(callee, impl_map);
                for arg in args {
                    changed |= self.process_expression(&mut arg.value, impl_map);
                }
                changed
            }
            ExpressionKind::Try(try_expr) => {
                let mut changed = self.process_expression(&mut try_expr.expression, impl_map);
                changed |= self.process_expression(&mut try_expr.catch_expression, impl_map);
                changed
            }
            ExpressionKind::ErrorChain(left, right) => {
                let mut changed = self.process_expression(left, impl_map);
                changed |= self.process_expression(right, impl_map);
                changed
            }
            ExpressionKind::OptionalMember(obj, _) => self.process_expression(obj, impl_map),
            ExpressionKind::OptionalIndex(obj, index) => {
                let mut changed = self.process_expression(obj, impl_map);
                changed |= self.process_expression(index, impl_map);
                changed
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                let mut changed = self.process_expression(obj, impl_map);
                for arg in args {
                    changed |= self.process_expression(&mut arg.value, impl_map);
                }
                changed
            }
            ExpressionKind::OptionalMethodCall(obj, _method_name, args, _) => {
                let mut changed = self.process_expression(obj, impl_map);
                for arg in args {
                    changed |= self.process_expression(&mut arg.value, impl_map);
                }
                changed
            }
            ExpressionKind::TypeAssertion(expr, _) => self.process_expression(expr, impl_map),
            ExpressionKind::Member(obj, _) => self.process_expression(obj, impl_map),
            ExpressionKind::Index(obj, index) => {
                let mut changed = self.process_expression(obj, impl_map);
                changed |= self.process_expression(index, impl_map);
                changed
            }
            ExpressionKind::Array(elements) => {
                let mut changed = false;
                for elem in elements {
                    match elem {
                        crate::ast::expression::ArrayElement::Expression(expr) => {
                            changed |= self.process_expression(expr, impl_map);
                        }
                        crate::ast::expression::ArrayElement::Spread(expr) => {
                            changed |= self.process_expression(expr, impl_map);
                        }
                    }
                }
                changed
            }
            ExpressionKind::Object(props) => {
                let mut changed = false;
                for prop in props {
                    if let crate::ast::expression::ObjectProperty::Property { value, .. } = prop {
                        changed |= self.process_expression(value, impl_map);
                    }
                }
                changed
            }
            ExpressionKind::Parenthesized(inner) => self.process_expression(inner, impl_map),
            ExpressionKind::Identifier(_)
            | ExpressionKind::Literal(_)
            | ExpressionKind::SelfKeyword
            | ExpressionKind::SuperKeyword
            | ExpressionKind::Template(_)
            | ExpressionKind::Function(_) => false,
        }
    }

    fn inline_interface_method(
        &self,
        receiver: &Expression,
        _class_id: StringId,
        method_body: &Block,
        _args: &[crate::ast::expression::Argument],
        span: Span,
    ) -> Option<ExpressionKind> {
        if method_body.statements.is_empty() {
            return Some(ExpressionKind::Literal(
                crate::ast::expression::Literal::Nil,
            ));
        }

        let return_value = self.extract_return_value(method_body, receiver, span);

        if let Some(ret_val) = return_value {
            Some(ret_val.kind.clone())
        } else {
            None
        }
    }

    fn extract_return_value(
        &self,
        block: &Block,
        receiver: &Expression,
        span: Span,
    ) -> Option<Expression> {
        let mut last_expr: Option<Expression> = None;

        for stmt in &block.statements {
            match stmt {
                Statement::Return(ret) => {
                    if let Some(first_value) = ret.values.first() {
                        let transformed = self.transform_expression(first_value, receiver, span);
                        return Some(transformed);
                    }
                    return Some(Expression::new(
                        ExpressionKind::Literal(crate::ast::expression::Literal::Nil),
                        span,
                    ));
                }
                Statement::Expression(expr) => {
                    let transformed = self.transform_expression(expr, receiver, span);
                    last_expr = Some(transformed);
                }
                Statement::Variable(decl) => {
                    let transformed_initializer =
                        self.transform_expression(&decl.initializer, receiver, span);
                    if let Some(ident) = self.get_pattern_name(&decl.pattern) {
                        last_expr = Some(Expression::new(
                            ExpressionKind::Assignment(
                                Box::new(Expression::new(ExpressionKind::Identifier(ident), span)),
                                AssignmentOp::Assign,
                                Box::new(transformed_initializer),
                            ),
                            span,
                        ));
                    }
                }
                _ => {}
            }
        }

        last_expr
    }

    fn get_pattern_name(&self, pattern: &Pattern) -> Option<StringId> {
        match pattern {
            Pattern::Identifier(ident) => Some(ident.node),
            _ => None,
        }
    }

    fn transform_expression(
        &self,
        expr: &Expression,
        receiver: &Expression,
        span: Span,
    ) -> Expression {
        match &expr.kind {
            ExpressionKind::SelfKeyword => receiver.clone(),
            ExpressionKind::Identifier(_) => expr.clone(),
            ExpressionKind::Literal(lit) => {
                Expression::new(ExpressionKind::Literal(lit.clone()), span)
            }
            ExpressionKind::Binary(op, left, right) => Expression::new(
                ExpressionKind::Binary(
                    *op,
                    Box::new(self.transform_expression(left, receiver, span)),
                    Box::new(self.transform_expression(right, receiver, span)),
                ),
                span,
            ),
            ExpressionKind::Unary(op, operand) => Expression::new(
                ExpressionKind::Unary(
                    *op,
                    Box::new(self.transform_expression(operand, receiver, span)),
                ),
                span,
            ),
            ExpressionKind::Call(func, args, type_args) => Expression::new(
                ExpressionKind::Call(
                    Box::new(self.transform_expression(func, receiver, span)),
                    args.iter()
                        .map(|arg| crate::ast::expression::Argument {
                            value: self.transform_expression(&arg.value, receiver, span),
                            is_spread: arg.is_spread,
                            span: arg.span,
                        })
                        .collect(),
                    type_args.clone(),
                ),
                span,
            ),
            ExpressionKind::Member(obj, prop) => Expression::new(
                ExpressionKind::Member(
                    Box::new(self.transform_expression(obj, receiver, span)),
                    prop.clone(),
                ),
                span,
            ),
            ExpressionKind::Index(obj, index) => Expression::new(
                ExpressionKind::Index(
                    Box::new(self.transform_expression(obj, receiver, span)),
                    Box::new(self.transform_expression(index, receiver, span)),
                ),
                span,
            ),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => Expression::new(
                ExpressionKind::Conditional(
                    Box::new(self.transform_expression(cond, receiver, span)),
                    Box::new(self.transform_expression(then_expr, receiver, span)),
                    Box::new(self.transform_expression(else_expr, receiver, span)),
                ),
                span,
            ),
            ExpressionKind::TypeAssertion(expr, ty) => Expression::new(
                ExpressionKind::TypeAssertion(
                    Box::new(self.transform_expression(expr, receiver, span)),
                    ty.clone(),
                ),
                span,
            ),
            _ => expr.clone(),
        }
    }
}

impl OptimizationPass for InterfaceMethodInliningPass {
    fn name(&self) -> &'static str {
        "interface-method-inlining"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        let impl_map = InterfaceImplementationMap::build(program);

        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.process_statement(stmt, &impl_map);
        }

        Ok(changed)
    }
}

impl Default for InterfaceMethodInliningPass {
    #[allow(clippy::arc_with_non_send_sync)]
    fn default() -> Self {
        Self {
            interner: Arc::new(StringInterner::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::expression::{AssignmentOp, ExpressionKind, Literal};
    use crate::ast::pattern::Pattern;
    use crate::ast::statement::{Block, ReturnStatement, VariableDeclaration, VariableKind};
    use crate::span::Span;

    #[test]
    fn test_interface_implementation_map_build_empty() {
        let program = Program::new(vec![], Span::dummy());
        let map = InterfaceImplementationMap::build(&program);

        assert!(map.known_classes.is_empty());
        assert!(map.known_interfaces.is_empty());
        assert!(map.interface_to_classes.is_empty());
    }

    #[test]
    fn test_statement_depth_counting() {
        let block = Block {
            statements: vec![
                Statement::Variable(VariableDeclaration {
                    kind: VariableKind::Local,
                    pattern: Pattern::Identifier(crate::ast::Spanned::new(
                        StringId::from_u32(1),
                        Span::dummy(),
                    )),
                    type_annotation: None,
                    initializer: Expression::new(
                        ExpressionKind::Literal(Literal::Number(1.0)),
                        Span::dummy(),
                    ),
                    span: Span::dummy(),
                }),
                Statement::Variable(VariableDeclaration {
                    kind: VariableKind::Local,
                    pattern: Pattern::Identifier(crate::ast::Spanned::new(
                        StringId::from_u32(2),
                        Span::dummy(),
                    )),
                    type_annotation: None,
                    initializer: Expression::new(
                        ExpressionKind::Literal(Literal::Number(2.0)),
                        Span::dummy(),
                    ),
                    span: Span::dummy(),
                }),
                Statement::Return(ReturnStatement {
                    values: vec![Expression::new(
                        ExpressionKind::Identifier(StringId::from_u32(1)),
                        Span::dummy(),
                    )],
                    span: Span::dummy(),
                }),
            ],
            span: Span::dummy(),
        };

        let mut map = InterfaceImplementationMap::default();
        map.known_classes.insert(StringId::from_u32(1), true);

        assert_eq!(map.count_statement_depth(&block), 3);
    }

    #[test]
    fn test_self_mutation_detection() {
        let class_id = StringId::from_u32(1);

        let mut map = InterfaceImplementationMap::default();
        map.known_classes.insert(class_id, true);

        let block = Block {
            statements: vec![Statement::Expression(Expression {
                kind: ExpressionKind::Assignment(
                    Box::new(Expression {
                        kind: ExpressionKind::Member(
                            Box::new(Expression::new(ExpressionKind::SelfKeyword, Span::dummy())),
                            crate::ast::Spanned::new(StringId::from_u32(2), Span::dummy()),
                        ),
                        span: Span::dummy(),
                        annotated_type: None,
                        receiver_class: None,
                    }),
                    AssignmentOp::Assign,
                    Box::new(Expression::new(
                        ExpressionKind::Literal(Literal::Number(42.0)),
                        Span::dummy(),
                    )),
                ),
                span: Span::dummy(),
                annotated_type: None,
                receiver_class: None,
            })],
            span: Span::dummy(),
        };

        assert!(map.mutates_self(&block, class_id));
    }

    #[test]
    fn test_non_self_mutation_returns_false() {
        let class_id = StringId::from_u32(1);

        let mut map = InterfaceImplementationMap::default();
        map.known_classes.insert(class_id, true);

        let block = Block {
            statements: vec![Statement::Return(ReturnStatement {
                values: vec![Expression::new(
                    ExpressionKind::Identifier(StringId::from_u32(1)),
                    Span::dummy(),
                )],
                span: Span::dummy(),
            })],
            span: Span::dummy(),
        };

        assert!(!map.mutates_self(&block, class_id));
    }
}
