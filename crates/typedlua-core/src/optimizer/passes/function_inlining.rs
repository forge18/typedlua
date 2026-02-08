// O2: Function Inlining Pass
// =============================================================================

use bumpalo::Bump;
use crate::optimizer::{PreAnalysisPass, StmtVisitor};
use crate::MutableProgram;
use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;
use typedlua_parser::ast::expression::{ArrowBody, Expression, ExpressionKind};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{
    Block, ForStatement, FunctionDeclaration, Parameter, ReturnStatement, Statement,
    VariableDeclaration, VariableKind,
};
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::{StringId, StringInterner};

enum InlineResult<'arena> {
    /// Direct expression substitution - for simple single-return functions
    /// The expression can be directly substituted for the call
    Direct(Box<Expression<'arena>>),
    /// Complex inlining - contains statements to insert and the result variable
    Replaced {
        stmts: Vec<Statement<'arena>>,
        result_var: StringId,
    },
}

/// Function inlining optimization pass (threshold: 5 statements)
/// Inlines small functions at call sites
pub struct FunctionInliningPass<'arena> {
    threshold: usize,
    next_temp_id: usize,
    functions: HashMap<StringId, FunctionDeclaration<'arena>>,
    interner: Option<Arc<StringInterner>>,
}

impl Default for FunctionInliningPass<'_> {
    fn default() -> Self {
        Self {
            threshold: 5,
            next_temp_id: 0,
            functions: HashMap::default(),
            interner: None,
        }
    }
}

impl<'arena> FunctionInliningPass<'arena> {
    pub fn new(interner: Arc<StringInterner>) -> Self {
        Self {
            threshold: 5,
            next_temp_id: 0,
            functions: HashMap::default(),
            interner: Some(interner),
        }
    }
}

impl<'arena> PreAnalysisPass<'arena> for FunctionInliningPass<'arena> {
    fn analyze(&mut self, program: &MutableProgram<'arena>) {
        self.functions.clear();
        self.collect_functions(program);
    }
}

impl<'arena> StmtVisitor<'arena> for FunctionInliningPass<'arena> {
    fn visit_stmt(&mut self, stmt: &mut Statement<'arena>, arena: &'arena Bump) -> bool {
        self.inline_in_statement(stmt, arena)
    }
}

impl<'arena> FunctionInliningPass<'arena> {
    fn collect_functions(&mut self, program: &MutableProgram<'arena>) {
        for stmt in &program.statements {
            self.collect_functions_in_stmt(stmt);
        }
    }

    fn collect_functions_in_stmt(&mut self, stmt: &Statement<'arena>) {
        match stmt {
            Statement::Function(func) => {
                self.functions.insert(func.name.node, func.clone());
                for s in func.body.statements.iter() {
                    self.collect_functions_in_stmt(s);
                }
            }
            Statement::If(if_stmt) => {
                for s in if_stmt.then_block.statements.iter() {
                    self.collect_functions_in_stmt(s);
                }
                for else_if in if_stmt.else_ifs.iter() {
                    for s in else_if.block.statements.iter() {
                        self.collect_functions_in_stmt(s);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for s in else_block.statements.iter() {
                        self.collect_functions_in_stmt(s);
                    }
                }
            }
            Statement::While(while_stmt) => {
                for s in while_stmt.body.statements.iter() {
                    self.collect_functions_in_stmt(s);
                }
            }
            Statement::For(for_stmt) => {
                let body = match &**for_stmt {
                    ForStatement::Numeric(for_num) => &for_num.body,
                    ForStatement::Generic(for_gen) => &for_gen.body,
                };
                for s in body.statements.iter() {
                    self.collect_functions_in_stmt(s);
                }
            }
            _ => {}
        }
    }
}

impl<'arena> FunctionInliningPass<'arena> {
    fn inline_in_statement(
        &mut self,
        stmt: &mut Statement<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        match stmt {
            Statement::Function(func) => {
                let mut changed = false;
                let mut stmts: Vec<Statement<'arena>> = func.body.statements.to_vec();
                let mut body_changed = false;
                for s in &mut stmts {
                    body_changed |= self.inline_in_statement(s, arena);
                }
                if body_changed {
                    func.body.statements = arena.alloc_slice_clone(&stmts);
                    changed = true;
                }
                changed
            }
            Statement::If(if_stmt) => {
                let mut changed = self.inline_in_expression(&mut if_stmt.condition, arena);
                changed |= self.inline_in_block(&mut if_stmt.then_block, arena);
                let mut new_else_ifs: Vec<_> = if_stmt.else_ifs.to_vec();
                let mut eic = false;
                for else_if in &mut new_else_ifs {
                    eic |= self.inline_in_expression(&mut else_if.condition, arena);
                    eic |= self.inline_in_block(&mut else_if.block, arena);
                }
                if eic {
                    if_stmt.else_ifs = arena.alloc_slice_clone(&new_else_ifs);
                    changed = true;
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.inline_in_block(else_block, arena);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.inline_in_expression(&mut while_stmt.condition, arena);
                changed |= self.inline_in_block(&mut while_stmt.body, arena);
                changed
            }
            Statement::For(for_stmt) => {
                match &**for_stmt {
                    ForStatement::Numeric(for_num_ref) => {
                        let mut new_num = (**for_num_ref).clone();
                        let mut fc = false;
                        fc |= self.inline_in_expression(&mut new_num.start, arena);
                        fc |= self.inline_in_expression(&mut new_num.end, arena);
                        if let Some(step) = &mut new_num.step {
                            fc |= self.inline_in_expression(step, arena);
                        }
                        fc |= self.inline_in_block(&mut new_num.body, arena);
                        if fc {
                            *stmt = Statement::For(
                                arena.alloc(ForStatement::Numeric(arena.alloc(new_num))),
                            );
                        }
                        fc
                    }
                    ForStatement::Generic(for_gen_ref) => {
                        let mut new_gen = for_gen_ref.clone();
                        let mut fc = false;
                        let mut new_iters: Vec<_> = new_gen.iterators.to_vec();
                        for expr in &mut new_iters {
                            fc |= self.inline_in_expression(expr, arena);
                        }
                        if fc {
                            new_gen.iterators = arena.alloc_slice_clone(&new_iters);
                        }
                        fc |= self.inline_in_block(&mut new_gen.body, arena);
                        if fc {
                            *stmt = Statement::For(
                                arena.alloc(ForStatement::Generic(new_gen)),
                            );
                        }
                        fc
                    }
                }
            }
            Statement::Variable(decl) => {
                if let Some(result) = self.try_inline_call(&mut decl.initializer, arena) {
                    match result {
                        InlineResult::Direct(_) => {
                            // Expression was directly substituted - no extra statements needed
                            true
                        }
                        InlineResult::Replaced { stmts, .. } => {
                            // decl.initializer has been modified to reference the result variable
                            let span = decl.span;
                            let var_stmt = Statement::Variable(decl.clone());
                            let mut new_stmts = stmts;
                            new_stmts.push(var_stmt);
                            *stmt = Statement::Block(Block {
                                statements: arena.alloc_slice_clone(&new_stmts),
                                span,
                            });
                            true
                        }
                    }
                } else {
                    false
                }
            }
            Statement::Expression(expr) => {
                if let Some(result) = self.try_inline_call(expr, arena) {
                    match result {
                        InlineResult::Direct(_) => {
                            // Expression was directly substituted - no extra statements needed
                            true
                        }
                        InlineResult::Replaced { stmts, .. } => {
                            let span = expr.span;
                            *stmt = Statement::Block(Block {
                                statements: arena.alloc_slice_clone(&stmts),
                                span,
                            });
                            true
                        }
                    }
                } else {
                    false
                }
            }
            Statement::Return(ret) => {
                let ret_span = ret.span;
                let mut values: Vec<Expression<'arena>> = ret.values.to_vec();

                let mut changed = false;
                let mut replaced_block = None;
                let mut i = 0;
                while i < values.len() {
                    if let Some(result) = self.try_inline_call(&mut values[i], arena) {
                        match result {
                            InlineResult::Direct(_) => {
                                // Expression was directly substituted - no extra statements needed
                                changed = true;
                            }
                            InlineResult::Replaced { stmts, .. } => {
                                // values[i] has been modified to reference the result variable
                                let new_ret = ReturnStatement {
                                    values: arena.alloc_slice_clone(&values),
                                    span: ret_span,
                                };
                                let mut new_stmts = stmts;
                                new_stmts.push(Statement::Return(new_ret));
                                replaced_block = Some(Statement::Block(Block {
                                    statements: arena.alloc_slice_clone(&new_stmts),
                                    span: ret_span,
                                }));
                                changed = true;
                                break;
                            }
                        }
                    }
                    i += 1;
                }
                if let Some(block_stmt) = replaced_block {
                    *stmt = block_stmt;
                } else if changed {
                    // Only Direct results -- update the values slice in the Return
                    if let Statement::Return(ret) = stmt {
                        ret.values = arena.alloc_slice_clone(&values);
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn inline_in_block(
        &mut self,
        block: &mut Block<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        let mut stmts: Vec<Statement<'arena>> = block.statements.to_vec();
        let mut changed = false;
        let mut i = 0;
        while i < stmts.len() {
            changed |= self.inline_in_statement(&mut stmts[i], arena);
            i += 1;
        }
        if changed {
            block.statements = arena.alloc_slice_clone(&stmts);
        }
        changed
    }

    fn inline_in_expression(
        &mut self,
        expr: &mut Expression<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        match &expr.kind {
            ExpressionKind::Call(func_ref, args_ref, type_args) => {
                let type_args = *type_args;
                let mut new_func = (**func_ref).clone();
                let fc = self.inline_in_expression(&mut new_func, arena);
                let mut new_args: Vec<_> = args_ref.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.inline_in_expression(&mut arg.value, arena);
                }
                if fc || ac {
                    expr.kind = ExpressionKind::Call(
                        arena.alloc(new_func),
                        arena.alloc_slice_clone(&new_args),
                        type_args,
                    );
                }
                let mut changed = fc || ac;
                // Also try to inline this call itself (for nested inlining)
                if let Some(InlineResult::Direct(_)) = self.try_inline_call(expr, arena) {
                    changed = true;
                }
                changed
            }
            ExpressionKind::MethodCall(obj_ref, method, args_ref, type_args) => {
                let method = method.clone();
                let type_args = *type_args;
                let mut new_obj = (**obj_ref).clone();
                let oc = self.inline_in_expression(&mut new_obj, arena);
                let mut new_args: Vec<_> = args_ref.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.inline_in_expression(&mut arg.value, arena);
                }
                if oc || ac {
                    expr.kind = ExpressionKind::MethodCall(
                        arena.alloc(new_obj),
                        method,
                        arena.alloc_slice_clone(&new_args),
                        type_args,
                    );
                }
                oc || ac
            }
            ExpressionKind::Binary(op, left_ref, right_ref) => {
                let op = *op;
                let mut new_left = (**left_ref).clone();
                let mut new_right = (**right_ref).clone();
                let lc = self.inline_in_expression(&mut new_left, arena);
                let rc = self.inline_in_expression(&mut new_right, arena);
                if lc || rc {
                    expr.kind = ExpressionKind::Binary(
                        op,
                        arena.alloc(new_left),
                        arena.alloc(new_right),
                    );
                }
                lc || rc
            }
            ExpressionKind::Unary(op, operand_ref) => {
                let op = *op;
                let mut new_operand = (**operand_ref).clone();
                let changed = self.inline_in_expression(&mut new_operand, arena);
                if changed {
                    expr.kind = ExpressionKind::Unary(op, arena.alloc(new_operand));
                }
                changed
            }
            ExpressionKind::Conditional(cond_ref, then_ref, else_ref) => {
                let mut new_cond = (**cond_ref).clone();
                let mut new_then = (**then_ref).clone();
                let mut new_else = (**else_ref).clone();
                let cc = self.inline_in_expression(&mut new_cond, arena);
                let tc = self.inline_in_expression(&mut new_then, arena);
                let ec = self.inline_in_expression(&mut new_else, arena);
                if cc || tc || ec {
                    expr.kind = ExpressionKind::Conditional(
                        arena.alloc(new_cond),
                        arena.alloc(new_then),
                        arena.alloc(new_else),
                    );
                }
                cc || tc || ec
            }
            ExpressionKind::Pipe(left_ref, right_ref) => {
                let mut new_left = (**left_ref).clone();
                let mut new_right = (**right_ref).clone();
                let lc = self.inline_in_expression(&mut new_left, arena);
                let rc = self.inline_in_expression(&mut new_right, arena);
                if lc || rc {
                    expr.kind = ExpressionKind::Pipe(
                        arena.alloc(new_left),
                        arena.alloc(new_right),
                    );
                }
                lc || rc
            }
            ExpressionKind::Match(match_expr) => {
                let mut new_value = (*match_expr.value).clone();
                let vc = self.inline_in_expression(&mut new_value, arena);
                let mut new_arms: Vec<_> = match_expr.arms.to_vec();
                let mut ac = false;
                for arm in &mut new_arms {
                    match &mut arm.body {
                        MatchArmBody::Expression(expr_ref) => {
                            let mut new_expr = (**expr_ref).clone();
                            if self.inline_in_expression(&mut new_expr, arena) {
                                arm.body = MatchArmBody::Expression(arena.alloc(new_expr));
                                ac = true;
                            }
                        }
                        MatchArmBody::Block(block) => {
                            ac |= self.inline_in_block(block, arena);
                        }
                    }
                }
                if vc || ac {
                    expr.kind = ExpressionKind::Match(typedlua_parser::ast::expression::MatchExpression {
                        value: arena.alloc(new_value),
                        arms: arena.alloc_slice_clone(&new_arms),
                        span: match_expr.span,
                    });
                }
                vc || ac
            }
            ExpressionKind::Arrow(arrow) => {
                let mut new_arrow = arrow.clone();
                let mut changed = false;
                let mut new_params: Vec<_> = new_arrow.parameters.to_vec();
                let mut pc = false;
                for param in &mut new_params {
                    if let Some(default) = &mut param.default {
                        pc |= self.inline_in_expression(default, arena);
                    }
                }
                if pc {
                    new_arrow.parameters = arena.alloc_slice_clone(&new_params);
                    changed = true;
                }
                match &mut new_arrow.body {
                    ArrowBody::Expression(expr_ref) => {
                        let mut new_expr = (**expr_ref).clone();
                        if self.inline_in_expression(&mut new_expr, arena) {
                            new_arrow.body = ArrowBody::Expression(arena.alloc(new_expr));
                            changed = true;
                        }
                    }
                    ArrowBody::Block(block) => {
                        changed |= self.inline_in_block(block, arena);
                    }
                }
                if changed {
                    expr.kind = ExpressionKind::Arrow(new_arrow);
                }
                changed
            }
            ExpressionKind::New(callee_ref, args_ref, type_args) => {
                let type_args = *type_args;
                let mut new_callee = (**callee_ref).clone();
                let cc = self.inline_in_expression(&mut new_callee, arena);
                let mut new_args: Vec<_> = args_ref.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.inline_in_expression(&mut arg.value, arena);
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
                let mut new_expression = (*try_expr.expression).clone();
                let mut new_catch = (*try_expr.catch_expression).clone();
                let ec = self.inline_in_expression(&mut new_expression, arena);
                let cc = self.inline_in_expression(&mut new_catch, arena);
                if ec || cc {
                    expr.kind = ExpressionKind::Try(typedlua_parser::ast::expression::TryExpression {
                        expression: arena.alloc(new_expression),
                        catch_variable: try_expr.catch_variable.clone(),
                        catch_expression: arena.alloc(new_catch),
                        span: try_expr.span,
                    });
                }
                ec || cc
            }
            ExpressionKind::ErrorChain(left_ref, right_ref) => {
                let mut new_left = (**left_ref).clone();
                let mut new_right = (**right_ref).clone();
                let lc = self.inline_in_expression(&mut new_left, arena);
                let rc = self.inline_in_expression(&mut new_right, arena);
                if lc || rc {
                    expr.kind = ExpressionKind::ErrorChain(
                        arena.alloc(new_left),
                        arena.alloc(new_right),
                    );
                }
                lc || rc
            }
            ExpressionKind::OptionalMember(obj_ref, member) => {
                let member = member.clone();
                let mut new_obj = (**obj_ref).clone();
                let changed = self.inline_in_expression(&mut new_obj, arena);
                if changed {
                    expr.kind = ExpressionKind::OptionalMember(arena.alloc(new_obj), member);
                }
                changed
            }
            ExpressionKind::OptionalIndex(obj_ref, index_ref) => {
                let mut new_obj = (**obj_ref).clone();
                let mut new_index = (**index_ref).clone();
                let oc = self.inline_in_expression(&mut new_obj, arena);
                let ic = self.inline_in_expression(&mut new_index, arena);
                if oc || ic {
                    expr.kind = ExpressionKind::OptionalIndex(
                        arena.alloc(new_obj),
                        arena.alloc(new_index),
                    );
                }
                oc || ic
            }
            ExpressionKind::OptionalCall(obj_ref, args_ref, type_args) => {
                let type_args = *type_args;
                let mut new_obj = (**obj_ref).clone();
                let oc = self.inline_in_expression(&mut new_obj, arena);
                let mut new_args: Vec<_> = args_ref.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.inline_in_expression(&mut arg.value, arena);
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
            ExpressionKind::OptionalMethodCall(obj_ref, method, args_ref, type_args) => {
                let method = method.clone();
                let type_args = *type_args;
                let mut new_obj = (**obj_ref).clone();
                let oc = self.inline_in_expression(&mut new_obj, arena);
                let mut new_args: Vec<_> = args_ref.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.inline_in_expression(&mut arg.value, arena);
                }
                if oc || ac {
                    expr.kind = ExpressionKind::OptionalMethodCall(
                        arena.alloc(new_obj),
                        method,
                        arena.alloc_slice_clone(&new_args),
                        type_args,
                    );
                }
                oc || ac
            }
            ExpressionKind::TypeAssertion(inner_ref, ty) => {
                let ty = ty.clone();
                let mut new_inner = (**inner_ref).clone();
                let changed = self.inline_in_expression(&mut new_inner, arena);
                if changed {
                    expr.kind = ExpressionKind::TypeAssertion(arena.alloc(new_inner), ty);
                }
                changed
            }
            ExpressionKind::Member(obj_ref, member) => {
                let member = member.clone();
                let mut new_obj = (**obj_ref).clone();
                let changed = self.inline_in_expression(&mut new_obj, arena);
                if changed {
                    expr.kind = ExpressionKind::Member(arena.alloc(new_obj), member);
                }
                changed
            }
            ExpressionKind::Index(obj_ref, index_ref) => {
                let mut new_obj = (**obj_ref).clone();
                let mut new_index = (**index_ref).clone();
                let oc = self.inline_in_expression(&mut new_obj, arena);
                let ic = self.inline_in_expression(&mut new_index, arena);
                if oc || ic {
                    expr.kind = ExpressionKind::Index(
                        arena.alloc(new_obj),
                        arena.alloc(new_index),
                    );
                }
                oc || ic
            }
            _ => false,
        }
    }

    fn try_inline_call(
        &mut self,
        expr: &mut Expression<'arena>,
        arena: &'arena Bump,
    ) -> Option<InlineResult<'arena>> {
        if let ExpressionKind::Call(func, args, _) = &expr.kind.clone() {
            if let ExpressionKind::Identifier(func_name) = &func.kind {
                if let Some(func_decl) = self.find_function_definition(expr, *func_name) {
                    if self.is_inlinable(func_decl) {
                        let args_vec: Vec<_> = args.to_vec();
                        let result = self.inline_call(func_decl.clone(), &args_vec, arena);
                        // Replace the call expression based on the inline result
                        match &result {
                            InlineResult::Direct(inlined_expr) => {
                                // Direct substitution - replace call with the inlined expression
                                *expr = *inlined_expr.clone();
                            }
                            InlineResult::Replaced { result_var, .. } => {
                                // Reference the result variable
                                expr.kind = ExpressionKind::Identifier(*result_var);
                            }
                        }
                        return Some(result);
                    }
                }
            }
        }
        None
    }

    fn find_function_definition(
        &self,
        _expr: &Expression<'arena>,
        name: StringId,
    ) -> Option<&FunctionDeclaration<'arena>> {
        self.functions.get(&name)
    }

    fn is_inlinable(&self, func: &FunctionDeclaration<'arena>) -> bool {
        // Skip generic functions - let GenericSpecializationPass handle them first
        if func.type_parameters.is_some() {
            return false;
        }
        if func.body.statements.len() > self.threshold {
            return false;
        }
        if self.is_recursive(func) {
            return false;
        }
        if self.has_complex_control_flow(&func.body) {
            return false;
        }
        if self.has_closures(&func.body) {
            return false;
        }
        true
    }

    fn is_recursive(&self, func: &FunctionDeclaration<'arena>) -> bool {
        let name = func.name.node;
        for stmt in func.body.statements.iter() {
            if self.contains_call_to(stmt, name) {
                return true;
            }
        }
        false
    }

    fn contains_call_to(&self, stmt: &Statement<'arena>, name: StringId) -> bool {
        match stmt {
            Statement::Expression(expr) => self.expr_contains_call_to(expr, name),
            Statement::Variable(decl) => self.expr_contains_call_to(&decl.initializer, name),
            Statement::Return(ret) => ret
                .values
                .iter()
                .any(|e| self.expr_contains_call_to(e, name)),
            Statement::If(if_stmt) => {
                self.expr_contains_call_to(&if_stmt.condition, name)
                    || if_stmt
                        .then_block
                        .statements
                        .iter()
                        .any(|s| self.contains_call_to(s, name))
                    || if_stmt.else_ifs.iter().any(|ei| {
                        self.expr_contains_call_to(&ei.condition, name)
                            || ei
                                .block
                                .statements
                                .iter()
                                .any(|s| self.contains_call_to(s, name))
                    })
                    || if_stmt.else_block.as_ref().is_some_and(|eb| {
                        eb.statements.iter().any(|s| self.contains_call_to(s, name))
                    })
            }
            Statement::While(while_stmt) => {
                self.expr_contains_call_to(&while_stmt.condition, name)
                    || while_stmt
                        .body
                        .statements
                        .iter()
                        .any(|s| self.contains_call_to(s, name))
            }
            Statement::For(for_stmt) => {
                let stmts = match &**for_stmt {
                    ForStatement::Numeric(for_num) => for_num.body.statements,
                    ForStatement::Generic(for_gen) => for_gen.body.statements,
                };
                stmts.iter().any(|s| self.contains_call_to(s, name))
            }
            _ => false,
        }
    }

    fn expr_contains_call_to(&self, expr: &Expression<'arena>, name: StringId) -> bool {
        match &expr.kind {
            ExpressionKind::Call(func, args, _) => {
                if let ExpressionKind::Identifier(id) = &func.kind {
                    if *id == name {
                        return true;
                    }
                }
                self.expr_contains_call_to(func, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::MethodCall(obj, method_name, args, _) => {
                if method_name.node == name {
                    return true;
                }
                self.expr_contains_call_to(obj, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::Binary(_, left, right) => {
                self.expr_contains_call_to(left, name) || self.expr_contains_call_to(right, name)
            }
            ExpressionKind::Unary(_, operand) => self.expr_contains_call_to(operand, name),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.expr_contains_call_to(cond, name)
                    || self.expr_contains_call_to(then_expr, name)
                    || self.expr_contains_call_to(else_expr, name)
            }
            ExpressionKind::Arrow(arrow) => {
                for param in arrow.parameters.iter() {
                    if let Some(default) = &param.default {
                        if self.expr_contains_call_to(default, name) {
                            return true;
                        }
                    }
                }
                match &arrow.body {
                    ArrowBody::Expression(expr) => self.expr_contains_call_to(expr, name),
                    ArrowBody::Block(block) => block
                        .statements
                        .iter()
                        .any(|s| self.contains_call_to(s, name)),
                }
            }
            ExpressionKind::Match(match_expr) => {
                self.expr_contains_call_to(match_expr.value, name)
                    || match_expr.arms.iter().any(|arm| match &arm.body {
                        MatchArmBody::Expression(expr) => self.expr_contains_call_to(expr, name),
                        MatchArmBody::Block(block) => block
                            .statements
                            .iter()
                            .any(|s| self.contains_call_to(s, name)),
                    })
            }
            ExpressionKind::New(callee, args, _) => {
                self.expr_contains_call_to(callee, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::Try(try_expr) => {
                self.expr_contains_call_to(try_expr.expression, name)
                    || self.expr_contains_call_to(try_expr.catch_expression, name)
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.expr_contains_call_to(left, name) || self.expr_contains_call_to(right, name)
            }
            ExpressionKind::OptionalMember(obj, _) => self.expr_contains_call_to(obj, name),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.expr_contains_call_to(obj, name) || self.expr_contains_call_to(index, name)
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.expr_contains_call_to(obj, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.expr_contains_call_to(obj, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::TypeAssertion(expr, _) => self.expr_contains_call_to(expr, name),
            ExpressionKind::Member(obj, _) => self.expr_contains_call_to(obj, name),
            ExpressionKind::Index(obj, index) => {
                self.expr_contains_call_to(obj, name) || self.expr_contains_call_to(index, name)
            }
            _ => false,
        }
    }

    fn has_complex_control_flow(&self, body: &Block<'arena>) -> bool {
        for stmt in body.statements.iter() {
            if self.stmt_has_complex_flow(stmt) {
                return true;
            }
        }
        false
    }

    fn stmt_has_complex_flow(&self, stmt: &Statement<'arena>) -> bool {
        match stmt {
            Statement::If(if_stmt) => {
                if self.block_has_multiple_returns(&if_stmt.then_block) {
                    return true;
                }
                for else_if in if_stmt.else_ifs.iter() {
                    if self.block_has_multiple_returns(&else_if.block) {
                        return true;
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    if self.block_has_multiple_returns(else_block) {
                        return true;
                    }
                }
                false
            }
            Statement::While(_) | Statement::For(_) | Statement::Repeat(_) => true,
            _ => false,
        }
    }

    fn block_has_multiple_returns(&self, block: &Block<'arena>) -> bool {
        let mut return_count = 0;
        for stmt in block.statements.iter() {
            if matches!(stmt, Statement::Return(_)) {
                return_count += 1;
                if return_count > 1 {
                    return true;
                }
            }
        }
        false
    }

    fn has_closures(&self, body: &Block<'arena>) -> bool {
        self.block_has_closures(body)
    }

    fn block_has_closures(&self, block: &Block<'arena>) -> bool {
        for stmt in block.statements.iter() {
            if self.stmt_has_closures(stmt) {
                return true;
            }
        }
        false
    }

    fn stmt_has_closures(&self, stmt: &Statement<'arena>) -> bool {
        match stmt {
            Statement::Function(func) => self.block_has_closures(&func.body),
            Statement::Variable(decl) => self.expr_has_closures(&decl.initializer),
            Statement::Expression(expr) => self.expr_has_closures(expr),
            Statement::If(if_stmt) => {
                self.expr_has_closures(&if_stmt.condition)
                    || self.block_has_closures(&if_stmt.then_block)
                    || if_stmt.else_ifs.iter().any(|ei| {
                        self.expr_has_closures(&ei.condition) || self.block_has_closures(&ei.block)
                    })
                    || if_stmt
                        .else_block
                        .as_ref()
                        .is_some_and(|eb| self.block_has_closures(eb))
            }
            Statement::While(while_stmt) => {
                self.expr_has_closures(&while_stmt.condition)
                    || self.block_has_closures(&while_stmt.body)
            }
            Statement::For(for_stmt) => {
                let body = match &**for_stmt {
                    ForStatement::Numeric(for_num) => &for_num.body,
                    ForStatement::Generic(for_gen) => &for_gen.body,
                };
                self.block_has_closures(body)
            }
            Statement::Return(ret) => ret.values.iter().any(|e| self.expr_has_closures(e)),
            _ => false,
        }
    }

    fn expr_has_closures(&self, expr: &Expression<'arena>) -> bool {
        match &expr.kind {
            ExpressionKind::Function(func) => self.block_has_closures(&func.body),
            ExpressionKind::Arrow(arrow) => match &arrow.body {
                ArrowBody::Expression(expr) => self.expr_has_closures(expr),
                ArrowBody::Block(block) => self.block_has_closures(block),
            },
            ExpressionKind::Call(func, args, _) => {
                self.expr_has_closures(func)
                    || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.expr_has_closures(obj) || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::Binary(_, left, right) => {
                self.expr_has_closures(left) || self.expr_has_closures(right)
            }
            ExpressionKind::Unary(_, operand) => self.expr_has_closures(operand),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.expr_has_closures(cond)
                    || self.expr_has_closures(then_expr)
                    || self.expr_has_closures(else_expr)
            }
            ExpressionKind::Pipe(left, right) => {
                self.expr_has_closures(left) || self.expr_has_closures(right)
            }
            ExpressionKind::Match(match_expr) => {
                self.expr_has_closures(match_expr.value)
                    || match_expr.arms.iter().any(|arm| match &arm.body {
                        MatchArmBody::Expression(expr) => self.expr_has_closures(expr),
                        MatchArmBody::Block(block) => self.block_has_closures(block),
                    })
            }
            ExpressionKind::New(callee, args, _) => {
                self.expr_has_closures(callee)
                    || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::Try(try_expr) => {
                self.expr_has_closures(try_expr.expression)
                    || self.expr_has_closures(try_expr.catch_expression)
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.expr_has_closures(left) || self.expr_has_closures(right)
            }
            ExpressionKind::OptionalMember(obj, _) => self.expr_has_closures(obj),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.expr_has_closures(obj) || self.expr_has_closures(index)
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.expr_has_closures(obj) || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.expr_has_closures(obj) || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::TypeAssertion(expr, _) => self.expr_has_closures(expr),
            ExpressionKind::Member(obj, _) => self.expr_has_closures(obj),
            ExpressionKind::Index(obj, index) => {
                self.expr_has_closures(obj) || self.expr_has_closures(index)
            }
            _ => false,
        }
    }

    fn inline_call(
        &mut self,
        func: FunctionDeclaration<'arena>,
        args: &[Argument<'arena>],
        arena: &'arena Bump,
    ) -> InlineResult<'arena> {
        let param_subst = self.create_parameter_substitution(func.parameters, args);

        // Check for simple single-return function: just `return expr`
        if func.body.statements.len() == 1 {
            if let Statement::Return(ret) = &func.body.statements[0] {
                if ret.values.len() == 1 {
                    // Simple case: directly substitute the return expression
                    let mut inlined_expr = ret.values[0].clone();
                    self.inline_expression(&mut inlined_expr, &param_subst, arena);
                    return InlineResult::Direct(Box::new(inlined_expr));
                }
            }
        }

        // Complex case: create intermediate variable
        let mut inlined_body = Vec::new();
        let return_var = self.create_temp_variable();
        let has_return = self.has_return_value(&func.body);

        let body_stmts: Vec<_> = func.body.statements.to_vec();
        for stmt in &body_stmts {
            let inlined_stmt =
                self.inline_statement(stmt, &param_subst, &return_var, has_return, arena);
            inlined_body.push(inlined_stmt);
        }

        InlineResult::Replaced {
            stmts: inlined_body,
            result_var: return_var,
        }
    }

    fn create_parameter_substitution(
        &self,
        parameters: &[Parameter<'arena>],
        args: &[Argument<'arena>],
    ) -> HashMap<StringId, Expression<'arena>> {
        let mut subst = HashMap::default();
        for (param, arg) in parameters.iter().zip(args.iter()) {
            if let Pattern::Identifier(ident) = &param.pattern {
                subst.insert(ident.node, arg.value.clone());
            }
        }
        subst
    }

    #[cold]
    fn unreachable_interner(&self) -> ! {
        unsafe { std::hint::unreachable_unchecked() }
    }

    fn create_temp_variable(&mut self) -> StringId {
        let name = format!("_inline_result_{}", self.next_temp_id);
        self.next_temp_id += 1;
        debug_assert!(
            self.interner.is_some(),
            "String interner not set for FunctionInliningPass"
        );
        match &self.interner {
            Some(interner) => interner.get_or_intern(&name),
            None => self.unreachable_interner(),
        }
    }

    fn has_return_value(&self, body: &Block<'arena>) -> bool {
        for stmt in body.statements.iter() {
            if let Statement::Return(ret) = stmt {
                if !ret.values.is_empty() {
                    return true;
                }
            }
        }
        false
    }

    fn inline_statement(
        &self,
        stmt: &Statement<'arena>,
        param_subst: &HashMap<StringId, Expression<'arena>>,
        return_var: &StringId,
        has_return: bool,
        arena: &'arena Bump,
    ) -> Statement<'arena> {
        match stmt {
            Statement::Variable(decl) => {
                let mut new_decl = decl.clone();
                self.inline_expression(&mut new_decl.initializer, param_subst, arena);
                Statement::Variable(new_decl)
            }
            Statement::Expression(expr) => {
                let mut new_expr = expr.clone();
                self.inline_expression(&mut new_expr, param_subst, arena);
                Statement::Expression(new_expr)
            }
            Statement::Return(ret) => {
                if !ret.values.is_empty() && has_return {
                    let values: Vec<Expression<'arena>> = ret
                        .values
                        .iter()
                        .map(|v| {
                            let mut substituted = v.clone();
                            self.inline_expression(&mut substituted, param_subst, arena);
                            substituted
                        })
                        .collect();
                    let initializer_kind = if values.len() == 1 {
                        values[0].kind.clone()
                    } else {
                        ExpressionKind::Array(
                            arena.alloc_slice_clone(
                                &values
                                    .iter()
                                    .map(|e| ArrayElement::Expression(e.clone()))
                                    .collect::<Vec<_>>(),
                            ),
                        )
                    };
                    Statement::Variable(VariableDeclaration {
                        kind: VariableKind::Local,
                        pattern: Pattern::Identifier(typedlua_parser::ast::Spanned::new(
                            *return_var,
                            Span::dummy(),
                        )),
                        type_annotation: None,
                        initializer: Expression::new(initializer_kind, Span::dummy()),
                        span: Span::dummy(),
                    })
                } else {
                    Statement::Return(ret.clone())
                }
            }
            _ => stmt.clone(),
        }
    }

    fn inline_expression(
        &self,
        expr: &mut Expression<'arena>,
        param_subst: &HashMap<StringId, Expression<'arena>>,
        arena: &'arena Bump,
    ) {
        match &expr.kind {
            ExpressionKind::Identifier(id) => {
                if let Some(substituted) = param_subst.get(id) {
                    expr.kind = substituted.kind.clone();
                }
            }
            ExpressionKind::Call(func_ref, args_ref, type_args) => {
                let type_args = *type_args;
                let mut new_func = (**func_ref).clone();
                self.inline_expression(&mut new_func, param_subst, arena);
                let mut new_args: Vec<_> = args_ref.to_vec();
                for arg in &mut new_args {
                    self.inline_expression(&mut arg.value, param_subst, arena);
                }
                expr.kind = ExpressionKind::Call(
                    arena.alloc(new_func),
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::MethodCall(obj_ref, method, args_ref, type_args) => {
                let method = method.clone();
                let type_args = *type_args;
                let mut new_obj = (**obj_ref).clone();
                self.inline_expression(&mut new_obj, param_subst, arena);
                let mut new_args: Vec<_> = args_ref.to_vec();
                for arg in &mut new_args {
                    self.inline_expression(&mut arg.value, param_subst, arena);
                }
                expr.kind = ExpressionKind::MethodCall(
                    arena.alloc(new_obj),
                    method,
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::Binary(op, left_ref, right_ref) => {
                let op = *op;
                let mut new_left = (**left_ref).clone();
                let mut new_right = (**right_ref).clone();
                self.inline_expression(&mut new_left, param_subst, arena);
                self.inline_expression(&mut new_right, param_subst, arena);
                expr.kind = ExpressionKind::Binary(
                    op,
                    arena.alloc(new_left),
                    arena.alloc(new_right),
                );
            }
            ExpressionKind::Unary(op, operand_ref) => {
                let op = *op;
                let mut new_operand = (**operand_ref).clone();
                self.inline_expression(&mut new_operand, param_subst, arena);
                expr.kind = ExpressionKind::Unary(op, arena.alloc(new_operand));
            }
            ExpressionKind::Conditional(cond_ref, then_ref, else_ref) => {
                let mut new_cond = (**cond_ref).clone();
                let mut new_then = (**then_ref).clone();
                let mut new_else = (**else_ref).clone();
                self.inline_expression(&mut new_cond, param_subst, arena);
                self.inline_expression(&mut new_then, param_subst, arena);
                self.inline_expression(&mut new_else, param_subst, arena);
                expr.kind = ExpressionKind::Conditional(
                    arena.alloc(new_cond),
                    arena.alloc(new_then),
                    arena.alloc(new_else),
                );
            }
            ExpressionKind::Arrow(arrow) => {
                let mut new_arrow = arrow.clone();
                let mut new_params: Vec<_> = new_arrow.parameters.to_vec();
                for param in &mut new_params {
                    if let Some(default) = &mut param.default {
                        self.inline_expression(default, param_subst, arena);
                    }
                }
                new_arrow.parameters = arena.alloc_slice_clone(&new_params);
                match &mut new_arrow.body {
                    ArrowBody::Expression(expr_ref) => {
                        let mut new_body_expr = (**expr_ref).clone();
                        self.inline_expression(&mut new_body_expr, param_subst, arena);
                        new_arrow.body = ArrowBody::Expression(arena.alloc(new_body_expr));
                    }
                    ArrowBody::Block(_) => {}
                }
                expr.kind = ExpressionKind::Arrow(new_arrow);
            }
            ExpressionKind::Match(match_expr) => {
                let mut new_value = (*match_expr.value).clone();
                self.inline_expression(&mut new_value, param_subst, arena);
                let mut new_arms: Vec<_> = match_expr.arms.to_vec();
                for arm in &mut new_arms {
                    match &mut arm.body {
                        MatchArmBody::Expression(expr_ref) => {
                            let mut new_expr = (**expr_ref).clone();
                            self.inline_expression(&mut new_expr, param_subst, arena);
                            arm.body = MatchArmBody::Expression(arena.alloc(new_expr));
                        }
                        MatchArmBody::Block(_) => {}
                    }
                }
                expr.kind = ExpressionKind::Match(typedlua_parser::ast::expression::MatchExpression {
                    value: arena.alloc(new_value),
                    arms: arena.alloc_slice_clone(&new_arms),
                    span: match_expr.span,
                });
            }
            ExpressionKind::New(callee_ref, args_ref, type_args) => {
                let type_args = *type_args;
                let mut new_callee = (**callee_ref).clone();
                self.inline_expression(&mut new_callee, param_subst, arena);
                let mut new_args: Vec<_> = args_ref.to_vec();
                for arg in &mut new_args {
                    self.inline_expression(&mut arg.value, param_subst, arena);
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
                self.inline_expression(&mut new_expression, param_subst, arena);
                self.inline_expression(&mut new_catch, param_subst, arena);
                expr.kind = ExpressionKind::Try(typedlua_parser::ast::expression::TryExpression {
                    expression: arena.alloc(new_expression),
                    catch_variable: try_expr.catch_variable.clone(),
                    catch_expression: arena.alloc(new_catch),
                    span: try_expr.span,
                });
            }
            ExpressionKind::ErrorChain(left_ref, right_ref) => {
                let mut new_left = (**left_ref).clone();
                let mut new_right = (**right_ref).clone();
                self.inline_expression(&mut new_left, param_subst, arena);
                self.inline_expression(&mut new_right, param_subst, arena);
                expr.kind = ExpressionKind::ErrorChain(
                    arena.alloc(new_left),
                    arena.alloc(new_right),
                );
            }
            ExpressionKind::OptionalMember(obj_ref, member) => {
                let member = member.clone();
                let mut new_obj = (**obj_ref).clone();
                self.inline_expression(&mut new_obj, param_subst, arena);
                expr.kind = ExpressionKind::OptionalMember(arena.alloc(new_obj), member);
            }
            ExpressionKind::OptionalIndex(obj_ref, index_ref) => {
                let mut new_obj = (**obj_ref).clone();
                let mut new_index = (**index_ref).clone();
                self.inline_expression(&mut new_obj, param_subst, arena);
                self.inline_expression(&mut new_index, param_subst, arena);
                expr.kind = ExpressionKind::OptionalIndex(
                    arena.alloc(new_obj),
                    arena.alloc(new_index),
                );
            }
            ExpressionKind::OptionalCall(obj_ref, args_ref, type_args) => {
                let type_args = *type_args;
                let mut new_obj = (**obj_ref).clone();
                self.inline_expression(&mut new_obj, param_subst, arena);
                let mut new_args: Vec<_> = args_ref.to_vec();
                for arg in &mut new_args {
                    self.inline_expression(&mut arg.value, param_subst, arena);
                }
                expr.kind = ExpressionKind::OptionalCall(
                    arena.alloc(new_obj),
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::OptionalMethodCall(obj_ref, method, args_ref, type_args) => {
                let method = method.clone();
                let type_args = *type_args;
                let mut new_obj = (**obj_ref).clone();
                self.inline_expression(&mut new_obj, param_subst, arena);
                let mut new_args: Vec<_> = args_ref.to_vec();
                for arg in &mut new_args {
                    self.inline_expression(&mut arg.value, param_subst, arena);
                }
                expr.kind = ExpressionKind::OptionalMethodCall(
                    arena.alloc(new_obj),
                    method,
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::TypeAssertion(inner_ref, ty) => {
                let ty = ty.clone();
                let mut new_inner = (**inner_ref).clone();
                self.inline_expression(&mut new_inner, param_subst, arena);
                expr.kind = ExpressionKind::TypeAssertion(arena.alloc(new_inner), ty);
            }
            ExpressionKind::Member(obj_ref, member) => {
                let member = member.clone();
                let mut new_obj = (**obj_ref).clone();
                self.inline_expression(&mut new_obj, param_subst, arena);
                expr.kind = ExpressionKind::Member(arena.alloc(new_obj), member);
            }
            ExpressionKind::Index(obj_ref, index_ref) => {
                let mut new_obj = (**obj_ref).clone();
                let mut new_index = (**index_ref).clone();
                self.inline_expression(&mut new_obj, param_subst, arena);
                self.inline_expression(&mut new_index, param_subst, arena);
                expr.kind = ExpressionKind::Index(
                    arena.alloc(new_obj),
                    arena.alloc(new_index),
                );
            }
            _ => {}
        }
    }
}

use typedlua_parser::ast::expression::Argument;
use typedlua_parser::ast::expression::ArrayElement;
use typedlua_parser::ast::expression::MatchArmBody;
