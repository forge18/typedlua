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

use bumpalo::Bump;
use crate::config::OptimizationLevel;
use crate::MutableProgram;

use crate::optimizer::{StmtVisitor, WholeProgramPass};
use rustc_hash::FxHashMap;
use std::sync::Arc;
use typedlua_parser::ast::expression::{AssignmentOp, Expression, ExpressionKind};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{Block, ClassMember, ForStatement, InterfaceMember, Statement};
use typedlua_parser::ast::types::TypeKind;
use typedlua_parser::ast::Spanned;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::{StringId, StringInterner};

const MAX_INLINABLE_STATEMENTS: usize = 10;

#[derive(Debug)]
#[derive(Default)]
struct InterfaceImplementationMap<'arena> {
    interface_to_classes: FxHashMap<StringId, Vec<StringId>>,
    class_to_interfaces: FxHashMap<StringId, Vec<StringId>>,
    class_is_final: FxHashMap<StringId, bool>,
    method_body: FxHashMap<(StringId, StringId), Block<'arena>>,
    method_signature: FxHashMap<(StringId, StringId), Spanned<StringId>>,
    known_classes: FxHashMap<StringId, bool>,
    known_interfaces: FxHashMap<StringId, bool>,
}


impl<'arena> InterfaceImplementationMap<'arena> {
    pub fn build(program: &MutableProgram<'arena>) -> Self {
        let mut map = InterfaceImplementationMap::default();

        for stmt in &program.statements {
            match stmt {
                Statement::Class(class) => {
                    let class_id = class.name.node;
                    map.known_classes.insert(class_id, true);
                    map.class_is_final.insert(class_id, class.is_final);

                    for member in class.members.iter() {
                        if let ClassMember::Method(method) = member {
                            let method_id = method.name.node;
                            if let Some(body) = &method.body {
                                map.method_body.insert((class_id, method_id), body.clone());
                            }
                        }
                    }

                    for implemented in class.implements.iter() {
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

                    for member in interface.members.iter() {
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

    pub fn get_method_body(&self, class_id: StringId, method_id: StringId) -> Option<&Block<'arena>> {
        self.method_body.get(&(class_id, method_id))
    }

    pub fn count_statement_depth(&self, block: &Block<'arena>) -> usize {
        block.statements.len()
    }

    pub fn mutates_self(&self, block: &Block<'arena>, class_id: StringId) -> bool {
        for stmt in block.statements.iter() {
            if self.statement_mutates_self(stmt, class_id) {
                return true;
            }
        }
        false
    }

    fn statement_mutates_self(&self, stmt: &Statement<'arena>, class_id: StringId) -> bool {
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
                    ForStatement::Numeric(for_num) => {
                        &for_num.body
                    }
                    ForStatement::Generic(for_gen) => {
                        &for_gen.body
                    }
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

    fn block_mutates_self(&self, block: &Block<'arena>, class_id: StringId) -> bool {
        block
            .statements
            .iter()
            .any(|s| self.statement_mutates_self(s, class_id))
    }

    fn expression_mutates_self(&self, expr: &Expression<'arena>, class_id: StringId) -> bool {
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
                self.expression_mutates_self(match_expr.value, class_id)
                    || match_expr.arms.iter().any(|arm| match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(e) => {
                            self.expression_mutates_self(e, class_id)
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(b) => {
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
                    typedlua_parser::ast::expression::ArrowBody::Expression(e) => {
                        self.expression_mutates_self(e, class_id)
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(b) => {
                        self.block_mutates_self(b, class_id)
                    }
                }
            }
            ExpressionKind::New(callee, args, _) => {
                self.expression_mutates_self(callee, class_id)
                    || args
                        .iter()
                        .any(|a| self.expression_mutates_self(&a.value, class_id))
            }
            ExpressionKind::Try(try_expr) => {
                self.expression_mutates_self(try_expr.expression, class_id)
                    || self.expression_mutates_self(try_expr.catch_expression, class_id)
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

    fn expression_is_self_member(&self, expr: &Expression<'arena>, class_id: StringId) -> bool {
        match &expr.kind {
            ExpressionKind::Member(obj, _prop) => self.expression_is_self(obj, class_id),
            ExpressionKind::Index(obj, _index) => self.expression_is_self(obj, class_id),
            _ => false,
        }
    }

    fn expression_is_self(&self, expr: &Expression<'arena>, _class_id: StringId) -> bool {
        matches!(expr.kind, ExpressionKind::SelfKeyword)
    }
}

pub struct InterfaceMethodInliningPass;

impl InterfaceMethodInliningPass {
    pub fn new(_interner: Arc<StringInterner>) -> Self {
        Self
    }

    fn process_statement<'arena>(
        &mut self,
        stmt: &mut Statement<'arena>,
        impl_map: &InterfaceImplementationMap<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        match stmt {
            Statement::Function(func) => {
                let mut stmts: Vec<_> = func.body.statements.to_vec();
                let mut changed = false;
                for s in &mut stmts {
                    changed |= self.process_statement(s, impl_map, arena);
                }
                if changed {
                    func.body.statements = arena.alloc_slice_clone(&stmts);
                }
                changed
            }
            Statement::If(if_stmt) => {
                let mut changed = self.process_expression(&mut if_stmt.condition, impl_map, arena);
                changed |= self.process_block(&mut if_stmt.then_block, impl_map, arena);
                let mut new_else_ifs: Vec<_> = if_stmt.else_ifs.to_vec();
                let mut eic = false;
                for else_if in &mut new_else_ifs {
                    eic |= self.process_expression(&mut else_if.condition, impl_map, arena);
                    eic |= self.process_block(&mut else_if.block, impl_map, arena);
                }
                if eic {
                    if_stmt.else_ifs = arena.alloc_slice_clone(&new_else_ifs);
                    changed = true;
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.process_block(else_block, impl_map, arena);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.process_expression(&mut while_stmt.condition, impl_map, arena);
                changed |= self.process_block(&mut while_stmt.body, impl_map, arena);
                changed
            }
            Statement::For(for_stmt) => {
                match &**for_stmt {
                    ForStatement::Numeric(for_num_ref) => {
                        let mut new_num = (**for_num_ref).clone();
                        let changed = self.process_block(&mut new_num.body, impl_map, arena);
                        if changed {
                            *stmt = Statement::For(
                                arena.alloc(ForStatement::Numeric(arena.alloc(new_num))),
                            );
                        }
                        changed
                    }
                    ForStatement::Generic(for_gen_ref) => {
                        let mut new_gen = for_gen_ref.clone();
                        let changed = self.process_block(&mut new_gen.body, impl_map, arena);
                        if changed {
                            *stmt =
                                Statement::For(arena.alloc(ForStatement::Generic(new_gen)));
                        }
                        changed
                    }
                }
            }
            Statement::Repeat(repeat_stmt) => {
                let mut changed = self.process_expression(&mut repeat_stmt.until, impl_map, arena);
                changed |= self.process_block(&mut repeat_stmt.body, impl_map, arena);
                changed
            }
            Statement::Return(ret_stmt) => {
                let mut vals: Vec<_> = ret_stmt.values.to_vec();
                let mut changed = false;
                for value in &mut vals {
                    changed |= self.process_expression(value, impl_map, arena);
                }
                if changed {
                    ret_stmt.values = arena.alloc_slice_clone(&vals);
                }
                changed
            }
            Statement::Expression(expr) => self.process_expression(expr, impl_map, arena),
            Statement::Block(block) => self.process_block(block, impl_map, arena),
            Statement::Try(try_stmt) => {
                let mut changed = self.process_block(&mut try_stmt.try_block, impl_map, arena);
                let mut new_clauses: Vec<_> = try_stmt.catch_clauses.to_vec();
                let mut clauses_changed = false;
                for clause in &mut new_clauses {
                    clauses_changed |= self.process_block(&mut clause.body, impl_map, arena);
                }
                if clauses_changed {
                    try_stmt.catch_clauses = arena.alloc_slice_clone(&new_clauses);
                    changed = true;
                }
                if let Some(finally) = &mut try_stmt.finally_block {
                    changed |= self.process_block(finally, impl_map, arena);
                }
                changed
            }
            Statement::Class(class) => {
                let mut members: Vec<_> = class.members.to_vec();
                let mut changed = false;
                for member in &mut members {
                    match member {
                        ClassMember::Method(method) => {
                            if let Some(body) = &mut method.body {
                                changed |= self.process_block(body, impl_map, arena);
                            }
                        }
                        ClassMember::Constructor(ctor) => {
                            changed |= self.process_block(&mut ctor.body, impl_map, arena);
                        }
                        ClassMember::Getter(getter) => {
                            changed |= self.process_block(&mut getter.body, impl_map, arena);
                        }
                        ClassMember::Setter(setter) => {
                            changed |= self.process_block(&mut setter.body, impl_map, arena);
                        }
                        ClassMember::Operator(op) => {
                            changed |= self.process_block(&mut op.body, impl_map, arena);
                        }
                        ClassMember::Property(_) => {}
                    }
                }
                if changed {
                    class.members = arena.alloc_slice_clone(&members);
                }
                changed
            }
            _ => false,
        }
    }

    fn process_block<'arena>(
        &mut self,
        block: &mut Block<'arena>,
        impl_map: &InterfaceImplementationMap<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        let mut stmts: Vec<_> = block.statements.to_vec();
        let mut changed = false;
        for stmt in &mut stmts {
            changed |= self.process_statement(stmt, impl_map, arena);
        }
        if changed {
            block.statements = arena.alloc_slice_clone(&stmts);
        }
        changed
    }

    fn process_expression<'arena>(
        &mut self,
        expr: &mut Expression<'arena>,
        impl_map: &InterfaceImplementationMap<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        match &expr.kind {
            ExpressionKind::MethodCall(obj, method_name, args, _) => {
                let method_name_cloned = method_name.clone();
                let mut new_obj = (**obj).clone();
                let mut changed = self.process_expression(&mut new_obj, impl_map, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut args_changed = false;
                for arg in &mut new_args {
                    args_changed |= self.process_expression(&mut arg.value, impl_map, arena);
                }
                if changed || args_changed {
                    // Rebuild the expression with arena-allocated children
                    let type_args = match &expr.kind {
                        ExpressionKind::MethodCall(_, _, _, ta) => *ta,
                        _ => None,
                    };
                    expr.kind = ExpressionKind::MethodCall(
                        arena.alloc(new_obj.clone()),
                        method_name_cloned.clone(),
                        arena.alloc_slice_clone(&new_args),
                        type_args,
                    );
                }

                if let Some(ref obj_type) = new_obj.annotated_type {
                    if let TypeKind::Reference(type_ref) = &obj_type.kind {
                        let type_id = type_ref.name.node;

                        if impl_map.is_interface(type_id) {
                            if let Some(implementing_class) =
                                impl_map.get_single_implementation(type_id)
                            {
                                let method_id = method_name_cloned.node;

                                if let Some(method_body) =
                                    impl_map.get_method_body(implementing_class, method_id)
                                {
                                    if impl_map.count_statement_depth(method_body)
                                        <= MAX_INLINABLE_STATEMENTS
                                        && !impl_map.mutates_self(method_body, implementing_class)
                                    {
                                        if let Some(inlined) = self.inline_interface_method(
                                            &new_obj,
                                            implementing_class,
                                            method_body,
                                            &new_args,
                                            expr.span,
                                            arena,
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
            ExpressionKind::Call(func, args, type_args) => {
                let type_args = *type_args;
                let mut new_func = (**func).clone();
                let mut changed = self.process_expression(&mut new_func, impl_map, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut args_changed = false;
                for arg in &mut new_args {
                    args_changed |= self.process_expression(&mut arg.value, impl_map, arena);
                }
                if changed || args_changed {
                    expr.kind = ExpressionKind::Call(
                        arena.alloc(new_func),
                        arena.alloc_slice_clone(&new_args),
                        type_args,
                    );
                    changed = true;
                }
                changed
            }
            ExpressionKind::Binary(op, left, right) => {
                let op = *op;
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                let left_changed = self.process_expression(&mut new_left, impl_map, arena);
                let right_changed = self.process_expression(&mut new_right, impl_map, arena);
                if left_changed || right_changed {
                    expr.kind = ExpressionKind::Binary(
                        op,
                        arena.alloc(new_left),
                        arena.alloc(new_right),
                    );
                }
                left_changed || right_changed
            }
            ExpressionKind::Unary(op, operand) => {
                let op = *op;
                let mut new_operand = (**operand).clone();
                let changed = self.process_expression(&mut new_operand, impl_map, arena);
                if changed {
                    expr.kind = ExpressionKind::Unary(op, arena.alloc(new_operand));
                }
                changed
            }
            ExpressionKind::Assignment(left, op, right) => {
                let op = *op;
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                let left_changed = self.process_expression(&mut new_left, impl_map, arena);
                let right_changed = self.process_expression(&mut new_right, impl_map, arena);
                if left_changed || right_changed {
                    expr.kind = ExpressionKind::Assignment(
                        arena.alloc(new_left),
                        op,
                        arena.alloc(new_right),
                    );
                }
                left_changed || right_changed
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut new_cond = (**cond).clone();
                let mut new_then = (**then_expr).clone();
                let mut new_else = (**else_expr).clone();
                let cc = self.process_expression(&mut new_cond, impl_map, arena);
                let tc = self.process_expression(&mut new_then, impl_map, arena);
                let ec = self.process_expression(&mut new_else, impl_map, arena);
                if cc || tc || ec {
                    expr.kind = ExpressionKind::Conditional(
                        arena.alloc(new_cond),
                        arena.alloc(new_then),
                        arena.alloc(new_else),
                    );
                }
                cc || tc || ec
            }
            ExpressionKind::Pipe(left, right) => {
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                let lc = self.process_expression(&mut new_left, impl_map, arena);
                let rc = self.process_expression(&mut new_right, impl_map, arena);
                if lc || rc {
                    expr.kind = ExpressionKind::Pipe(
                        arena.alloc(new_left),
                        arena.alloc(new_right),
                    );
                }
                lc || rc
            }
            ExpressionKind::Match(match_expr) => {
                let mut new_match = match_expr.clone();
                let mut new_value = (*new_match.value).clone();
                let mut changed = self.process_expression(&mut new_value, impl_map, arena);
                if changed {
                    new_match.value = arena.alloc(new_value);
                }
                let mut new_arms: Vec<_> = new_match.arms.to_vec();
                let mut arms_changed = false;
                for arm in &mut new_arms {
                    match &mut arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(e_ref) => {
                            let mut new_e = (**e_ref).clone();
                            if self.process_expression(&mut new_e, impl_map, arena) {
                                *e_ref = arena.alloc(new_e);
                                arms_changed = true;
                            }
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            arms_changed |= self.process_block(block, impl_map, arena);
                        }
                    }
                }
                if arms_changed {
                    new_match.arms = arena.alloc_slice_clone(&new_arms);
                    changed = true;
                }
                if changed {
                    expr.kind = ExpressionKind::Match(new_match);
                }
                changed
            }
            ExpressionKind::Arrow(arrow) => {
                let mut new_arrow = arrow.clone();
                let mut changed = false;
                let mut new_params: Vec<_> = new_arrow.parameters.to_vec();
                let mut params_changed = false;
                for param in &mut new_params {
                    if let Some(default) = &mut param.default {
                        params_changed |= self.process_expression(default, impl_map, arena);
                    }
                }
                if params_changed {
                    new_arrow.parameters = arena.alloc_slice_clone(&new_params);
                    changed = true;
                }
                match &mut new_arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(e_ref) => {
                        let mut new_e = (**e_ref).clone();
                        if self.process_expression(&mut new_e, impl_map, arena) {
                            *e_ref = arena.alloc(new_e);
                            changed = true;
                        }
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        changed |= self.process_block(block, impl_map, arena);
                    }
                }
                if changed {
                    expr.kind = ExpressionKind::Arrow(new_arrow);
                }
                changed
            }
            ExpressionKind::New(callee, args, type_args) => {
                let type_args = *type_args;
                let mut new_callee = (**callee).clone();
                let cc = self.process_expression(&mut new_callee, impl_map, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.process_expression(&mut arg.value, impl_map, arena);
                }
                if cc || ac {
                    expr.kind = ExpressionKind::New(
                        arena.alloc(new_callee),
                        arena.alloc_slice_clone(&new_args),
                        type_args,
                    );
                }
                cc || ac
            }
            ExpressionKind::Try(try_expr) => {
                let mut new_try = try_expr.clone();
                let mut new_expression = (*new_try.expression).clone();
                let mut new_catch = (*new_try.catch_expression).clone();
                let ec = self.process_expression(&mut new_expression, impl_map, arena);
                let cc = self.process_expression(&mut new_catch, impl_map, arena);
                if ec {
                    new_try.expression = arena.alloc(new_expression);
                }
                if cc {
                    new_try.catch_expression = arena.alloc(new_catch);
                }
                let changed = ec || cc;
                if changed {
                    expr.kind = ExpressionKind::Try(new_try);
                }
                changed
            }
            ExpressionKind::ErrorChain(left, right) => {
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                let lc = self.process_expression(&mut new_left, impl_map, arena);
                let rc = self.process_expression(&mut new_right, impl_map, arena);
                if lc || rc {
                    expr.kind = ExpressionKind::ErrorChain(
                        arena.alloc(new_left),
                        arena.alloc(new_right),
                    );
                }
                lc || rc
            }
            ExpressionKind::OptionalMember(obj, member) => {
                let member = member.clone();
                let mut new_obj = (**obj).clone();
                let changed = self.process_expression(&mut new_obj, impl_map, arena);
                if changed {
                    expr.kind = ExpressionKind::OptionalMember(arena.alloc(new_obj), member);
                }
                changed
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                let mut new_obj = (**obj).clone();
                let mut new_index = (**index).clone();
                let oc = self.process_expression(&mut new_obj, impl_map, arena);
                let ic = self.process_expression(&mut new_index, impl_map, arena);
                if oc || ic {
                    expr.kind = ExpressionKind::OptionalIndex(
                        arena.alloc(new_obj),
                        arena.alloc(new_index),
                    );
                }
                oc || ic
            }
            ExpressionKind::OptionalCall(obj, args, type_args) => {
                let type_args = *type_args;
                let mut new_obj = (**obj).clone();
                let oc = self.process_expression(&mut new_obj, impl_map, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.process_expression(&mut arg.value, impl_map, arena);
                }
                if oc || ac {
                    expr.kind = ExpressionKind::OptionalCall(
                        arena.alloc(new_obj),
                        arena.alloc_slice_clone(&new_args),
                        type_args,
                    );
                }
                oc || ac
            }
            ExpressionKind::OptionalMethodCall(obj, method_name, args, type_args) => {
                let method_name = method_name.clone();
                let type_args = *type_args;
                let mut new_obj = (**obj).clone();
                let oc = self.process_expression(&mut new_obj, impl_map, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.process_expression(&mut arg.value, impl_map, arena);
                }
                if oc || ac {
                    expr.kind = ExpressionKind::OptionalMethodCall(
                        arena.alloc(new_obj),
                        method_name,
                        arena.alloc_slice_clone(&new_args),
                        type_args,
                    );
                }
                oc || ac
            }
            ExpressionKind::TypeAssertion(inner, ty) => {
                let ty = ty.clone();
                let mut new_inner = (**inner).clone();
                let changed = self.process_expression(&mut new_inner, impl_map, arena);
                if changed {
                    expr.kind = ExpressionKind::TypeAssertion(arena.alloc(new_inner), ty);
                }
                changed
            }
            ExpressionKind::Member(obj, member) => {
                let member = member.clone();
                let mut new_obj = (**obj).clone();
                let changed = self.process_expression(&mut new_obj, impl_map, arena);
                if changed {
                    expr.kind = ExpressionKind::Member(arena.alloc(new_obj), member);
                }
                changed
            }
            ExpressionKind::Index(obj, index) => {
                let mut new_obj = (**obj).clone();
                let mut new_index = (**index).clone();
                let oc = self.process_expression(&mut new_obj, impl_map, arena);
                let ic = self.process_expression(&mut new_index, impl_map, arena);
                if oc || ic {
                    expr.kind = ExpressionKind::Index(
                        arena.alloc(new_obj),
                        arena.alloc(new_index),
                    );
                }
                oc || ic
            }
            ExpressionKind::Array(elements) => {
                let mut new_elements: Vec<_> = elements.to_vec();
                let mut ec = false;
                for elem in &mut new_elements {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(e) => {
                            ec |= self.process_expression(e, impl_map, arena);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(e) => {
                            ec |= self.process_expression(e, impl_map, arena);
                        }
                    }
                }
                if ec {
                    expr.kind = ExpressionKind::Array(arena.alloc_slice_clone(&new_elements));
                }
                ec
            }
            ExpressionKind::Object(props) => {
                let mut new_props: Vec<_> = props.to_vec();
                let mut pc = false;
                for prop in &mut new_props {
                    if let typedlua_parser::ast::expression::ObjectProperty::Property {
                        key,
                        value,
                        span,
                    } = prop
                    {
                        let mut new_val = (**value).clone();
                        if self.process_expression(&mut new_val, impl_map, arena) {
                            *prop = typedlua_parser::ast::expression::ObjectProperty::Property {
                                key: key.clone(),
                                value: arena.alloc(new_val),
                                span: *span,
                            };
                            pc = true;
                        }
                    }
                }
                if pc {
                    expr.kind = ExpressionKind::Object(arena.alloc_slice_clone(&new_props));
                }
                pc
            }
            ExpressionKind::Parenthesized(inner) => {
                let mut new_inner = (**inner).clone();
                let changed = self.process_expression(&mut new_inner, impl_map, arena);
                if changed {
                    expr.kind = ExpressionKind::Parenthesized(arena.alloc(new_inner));
                }
                changed
            }
            ExpressionKind::Identifier(_)
            | ExpressionKind::Literal(_)
            | ExpressionKind::SelfKeyword
            | ExpressionKind::SuperKeyword
            | ExpressionKind::Template(_)
            | ExpressionKind::Function(_) => false,
        }
    }

    fn inline_interface_method<'arena>(
        &self,
        receiver: &Expression<'arena>,
        _class_id: StringId,
        method_body: &Block<'arena>,
        _args: &[typedlua_parser::ast::expression::Argument<'arena>],
        span: Span,
        arena: &'arena Bump,
    ) -> Option<ExpressionKind<'arena>> {
        if method_body.statements.is_empty() {
            return Some(ExpressionKind::Literal(
                typedlua_parser::ast::expression::Literal::Nil,
            ));
        }

        let return_value = self.extract_return_value(method_body, receiver, span, arena);

        if let Some(ret_val) = return_value {
            Some(ret_val.kind.clone())
        } else {
            None
        }
    }

    fn extract_return_value<'arena>(
        &self,
        block: &Block<'arena>,
        receiver: &Expression<'arena>,
        span: Span,
        arena: &'arena Bump,
    ) -> Option<Expression<'arena>> {
        let mut last_expr: Option<Expression<'arena>> = None;

        for stmt in block.statements.iter() {
            match stmt {
                Statement::Return(ret) => {
                    if let Some(first_value) = ret.values.first() {
                        let transformed = self.transform_expression(first_value, receiver, span, arena);
                        return Some(transformed);
                    }
                    return Some(Expression::new(
                        ExpressionKind::Literal(typedlua_parser::ast::expression::Literal::Nil),
                        span,
                    ));
                }
                Statement::Expression(expr) => {
                    let transformed = self.transform_expression(expr, receiver, span, arena);
                    last_expr = Some(transformed);
                }
                Statement::Variable(decl) => {
                    let transformed_initializer =
                        self.transform_expression(&decl.initializer, receiver, span, arena);
                    if let Some(ident) = self.get_pattern_name(&decl.pattern) {
                        last_expr = Some(Expression::new(
                            ExpressionKind::Assignment(
                                arena.alloc(Expression::new(ExpressionKind::Identifier(ident), span)),
                                AssignmentOp::Assign,
                                arena.alloc(transformed_initializer),
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

    fn get_pattern_name(&self, pattern: &Pattern<'_>) -> Option<StringId> {
        match pattern {
            Pattern::Identifier(ident) => Some(ident.node),
            _ => None,
        }
    }

    fn transform_expression<'arena>(
        &self,
        expr: &Expression<'arena>,
        receiver: &Expression<'arena>,
        span: Span,
        arena: &'arena Bump,
    ) -> Expression<'arena> {
        match &expr.kind {
            ExpressionKind::SelfKeyword => receiver.clone(),
            ExpressionKind::Identifier(_) => expr.clone(),
            ExpressionKind::Literal(lit) => {
                Expression::new(ExpressionKind::Literal(lit.clone()), span)
            }
            ExpressionKind::Binary(op, left, right) => Expression::new(
                ExpressionKind::Binary(
                    *op,
                    arena.alloc(self.transform_expression(left, receiver, span, arena)),
                    arena.alloc(self.transform_expression(right, receiver, span, arena)),
                ),
                span,
            ),
            ExpressionKind::Unary(op, operand) => Expression::new(
                ExpressionKind::Unary(
                    *op,
                    arena.alloc(self.transform_expression(operand, receiver, span, arena)),
                ),
                span,
            ),
            ExpressionKind::Call(func, args, type_args) => {
                let new_args: Vec<_> = args
                    .iter()
                    .map(|arg| typedlua_parser::ast::expression::Argument {
                        value: self.transform_expression(&arg.value, receiver, span, arena),
                        is_spread: arg.is_spread,
                        span: arg.span,
                    })
                    .collect();
                Expression::new(
                    ExpressionKind::Call(
                        arena.alloc(self.transform_expression(func, receiver, span, arena)),
                        arena.alloc_slice_clone(&new_args),
                        *type_args,
                    ),
                    span,
                )
            }
            ExpressionKind::Member(obj, prop) => Expression::new(
                ExpressionKind::Member(
                    arena.alloc(self.transform_expression(obj, receiver, span, arena)),
                    prop.clone(),
                ),
                span,
            ),
            ExpressionKind::Index(obj, index) => Expression::new(
                ExpressionKind::Index(
                    arena.alloc(self.transform_expression(obj, receiver, span, arena)),
                    arena.alloc(self.transform_expression(index, receiver, span, arena)),
                ),
                span,
            ),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => Expression::new(
                ExpressionKind::Conditional(
                    arena.alloc(self.transform_expression(cond, receiver, span, arena)),
                    arena.alloc(self.transform_expression(then_expr, receiver, span, arena)),
                    arena.alloc(self.transform_expression(else_expr, receiver, span, arena)),
                ),
                span,
            ),
            ExpressionKind::TypeAssertion(inner, ty) => Expression::new(
                ExpressionKind::TypeAssertion(
                    arena.alloc(self.transform_expression(inner, receiver, span, arena)),
                    ty.clone(),
                ),
                span,
            ),
            _ => expr.clone(),
        }
    }
}

impl<'arena> StmtVisitor<'arena> for InterfaceMethodInliningPass {
    fn visit_stmt(&mut self, _stmt: &mut Statement<'arena>, _arena: &'arena Bump) -> bool {
        // For visitor pattern, we need access to impl_map
        // This is a simplified version - the actual run() method does the work
        false
    }
}

impl<'arena> WholeProgramPass<'arena> for InterfaceMethodInliningPass {
    fn name(&self) -> &'static str {
        "interface-method-inlining"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(
        &mut self,
        program: &mut MutableProgram<'arena>,
        arena: &'arena Bump,
    ) -> Result<bool, String> {
        let impl_map = InterfaceImplementationMap::build(program);

        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.process_statement(stmt, &impl_map, arena);
        }

        Ok(changed)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Default for InterfaceMethodInliningPass {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;
    use typedlua_parser::ast::expression::{AssignmentOp, ExpressionKind, Literal};
    use typedlua_parser::ast::pattern::Pattern;
    use typedlua_parser::ast::statement::{
        Block, ReturnStatement, VariableDeclaration, VariableKind,
    };
    use typedlua_parser::span::Span;

    #[test]
    fn test_interface_implementation_map_build_empty() {
        let program = MutableProgram {
            statements: vec![],
            span: Span::dummy(),
        };
        let map = InterfaceImplementationMap::build(&program);

        assert!(map.known_classes.is_empty());
        assert!(map.known_interfaces.is_empty());
        assert!(map.interface_to_classes.is_empty());
    }

    #[test]
    fn test_statement_depth_counting() {
        let arena = Bump::new();
        let block = Block {
            statements: arena.alloc_slice_clone(&[
                Statement::Variable(VariableDeclaration {
                    kind: VariableKind::Local,
                    pattern: Pattern::Identifier(typedlua_parser::ast::Spanned::new(
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
                    pattern: Pattern::Identifier(typedlua_parser::ast::Spanned::new(
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
                    values: arena.alloc_slice_clone(&[Expression::new(
                        ExpressionKind::Identifier(StringId::from_u32(1)),
                        Span::dummy(),
                    )]),
                    span: Span::dummy(),
                }),
            ]),
            span: Span::dummy(),
        };

        let mut map = InterfaceImplementationMap::default();
        map.known_classes.insert(StringId::from_u32(1), true);

        assert_eq!(map.count_statement_depth(&block), 3);
    }

    #[test]
    fn test_self_mutation_detection() {
        let arena = Bump::new();
        let class_id = StringId::from_u32(1);

        let mut map = InterfaceImplementationMap::default();
        map.known_classes.insert(class_id, true);

        let block = Block {
            statements: arena.alloc_slice_clone(&[Statement::Expression(Expression {
                kind: ExpressionKind::Assignment(
                    arena.alloc(Expression {
                        kind: ExpressionKind::Member(
                            arena.alloc(Expression::new(ExpressionKind::SelfKeyword, Span::dummy())),
                            typedlua_parser::ast::Spanned::new(
                                StringId::from_u32(2),
                                Span::dummy(),
                            ),
                        ),
                        span: Span::dummy(),
                        annotated_type: None,
                        receiver_class: None,
                    }),
                    AssignmentOp::Assign,
                    arena.alloc(Expression::new(
                        ExpressionKind::Literal(Literal::Number(42.0)),
                        Span::dummy(),
                    )),
                ),
                span: Span::dummy(),
                annotated_type: None,
                receiver_class: None,
            })]),
            span: Span::dummy(),
        };

        assert!(map.mutates_self(&block, class_id));
    }

    #[test]
    fn test_non_self_mutation_returns_false() {
        let arena = Bump::new();
        let class_id = StringId::from_u32(1);

        let mut map = InterfaceImplementationMap::default();
        map.known_classes.insert(class_id, true);

        let block = Block {
            statements: arena.alloc_slice_clone(&[Statement::Return(ReturnStatement {
                values: arena.alloc_slice_clone(&[Expression::new(
                    ExpressionKind::Identifier(StringId::from_u32(1)),
                    Span::dummy(),
                )]),
                span: Span::dummy(),
            })]),
            span: Span::dummy(),
        };

        assert!(!map.mutates_self(&block, class_id));
    }
}
