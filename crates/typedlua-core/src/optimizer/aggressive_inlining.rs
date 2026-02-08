use bumpalo::Bump;
use crate::config::OptimizationLevel;
use crate::MutableProgram;

use crate::optimizer::{StmtVisitor, WholeProgramPass};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use typedlua_parser::ast::expression::{
    Argument, ArrowBody, Expression, ExpressionKind, MatchArmBody,
};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{
    Block, ForStatement, FunctionDeclaration, Parameter, ReturnStatement, Statement,
    VariableDeclaration, VariableKind,
};
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::StringId;
use typedlua_parser::string_interner::StringInterner;

use typedlua_parser::ast::expression::ArrayElement;

enum InlineResult<'arena> {
    Direct(Expression<'arena>),
    Replaced {
        stmts: Vec<Statement<'arena>>,
        result_var: StringId,
    },
}

pub struct AggressiveInliningPass {
    threshold: usize,
    closure_threshold: usize,
    max_total_closure_size: usize,
    max_code_bloat_ratio: f64,
    next_temp_id: usize,
    interner: Option<Arc<StringInterner>>,
    hot_paths: HashSet<StringId>,
}

impl Default for AggressiveInliningPass {
    fn default() -> Self {
        Self {
            threshold: 15,
            closure_threshold: 3,
            max_total_closure_size: 20,
            max_code_bloat_ratio: 3.0,
            next_temp_id: 0,
            interner: None,
            hot_paths: HashSet::new(),
        }
    }
}

impl AggressiveInliningPass {
    pub fn new(interner: Arc<StringInterner>) -> Self {
        Self {
            interner: Some(interner),
            ..Default::default()
        }
    }
}

impl<'arena> StmtVisitor<'arena> for AggressiveInliningPass {
    fn visit_stmt(&mut self, stmt: &mut Statement<'arena>, arena: &'arena Bump) -> bool {
        // When used as a standalone visitor (no pre-collected functions),
        // inline_in_statement still works for expression-level inlining
        self.inline_in_statement(stmt, arena, &HashMap::new())
    }
}

impl<'arena> WholeProgramPass<'arena> for AggressiveInliningPass {
    fn name(&self) -> &'static str {
        "aggressive-inlining"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(
        &mut self,
        program: &mut MutableProgram<'arena>,
        arena: &'arena Bump,
    ) -> Result<bool, String> {
        self.next_temp_id = 0;
        self.hot_paths.clear();

        let mut functions = HashMap::new();
        Self::collect_functions_from_program(program, &mut functions);
        self.detect_hot_paths(program);

        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.inline_in_statement(stmt, arena, &functions);
        }

        Ok(changed)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl AggressiveInliningPass {
    fn collect_functions_from_program<'arena>(
        program: &MutableProgram<'arena>,
        functions: &mut HashMap<StringId, FunctionDeclaration<'arena>>,
    ) {
        for stmt in &program.statements {
            Self::collect_functions_in_stmt(stmt, functions);
        }
    }

    fn collect_functions_in_stmt<'arena>(
        stmt: &Statement<'arena>,
        functions: &mut HashMap<StringId, FunctionDeclaration<'arena>>,
    ) {
        match stmt {
            Statement::Function(func) => {
                functions.insert(func.name.node, func.clone());
                for s in func.body.statements.iter() {
                    Self::collect_functions_in_stmt(s, functions);
                }
            }
            Statement::If(if_stmt) => {
                for s in if_stmt.then_block.statements.iter() {
                    Self::collect_functions_in_stmt(s, functions);
                }
                for else_if in if_stmt.else_ifs.iter() {
                    for s in else_if.block.statements.iter() {
                        Self::collect_functions_in_stmt(s, functions);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for s in else_block.statements.iter() {
                        Self::collect_functions_in_stmt(s, functions);
                    }
                }
            }
            Statement::While(while_stmt) => {
                for s in while_stmt.body.statements.iter() {
                    Self::collect_functions_in_stmt(s, functions);
                }
            }
            Statement::For(for_stmt) => {
                let body = match &**for_stmt {
                    ForStatement::Numeric(for_num) => &for_num.body,
                    ForStatement::Generic(for_gen) => &for_gen.body,
                };
                for s in body.statements.iter() {
                    Self::collect_functions_in_stmt(s, functions);
                }
            }
            _ => {}
        }
    }

    fn detect_hot_paths<'arena>(&mut self, program: &MutableProgram<'arena>) {
        for stmt in &program.statements {
            self.detect_hot_paths_in_stmt(stmt, &mut HashSet::new());
        }
    }

    fn detect_hot_paths_in_stmt<'arena>(
        &mut self,
        stmt: &Statement<'arena>,
        in_loop: &mut HashSet<StringId>,
    ) {
        match stmt {
            Statement::While(while_stmt) => {
                let mut new_in_loop = in_loop.clone();
                self.detect_calls_in_block(&while_stmt.body, &mut new_in_loop);
            }
            Statement::For(for_stmt) => {
                let body = match &**for_stmt {
                    ForStatement::Numeric(for_num) => &for_num.body,
                    ForStatement::Generic(for_gen) => &for_gen.body,
                };
                let mut new_in_loop = in_loop.clone();
                self.detect_calls_in_block(body, &mut new_in_loop);
            }
            Statement::If(if_stmt) => {
                for s in if_stmt.then_block.statements.iter() {
                    self.detect_hot_paths_in_stmt(s, in_loop);
                }
                for else_if in if_stmt.else_ifs.iter() {
                    for s in else_if.block.statements.iter() {
                        self.detect_hot_paths_in_stmt(s, in_loop);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for s in else_block.statements.iter() {
                        self.detect_hot_paths_in_stmt(s, in_loop);
                    }
                }
            }
            Statement::Function(func) => {
                for s in func.body.statements.iter() {
                    self.detect_hot_paths_in_stmt(s, in_loop);
                }
            }
            Statement::Expression(expr) => {
                self.detect_calls_in_expr(expr, in_loop);
            }
            Statement::Variable(decl) => {
                self.detect_calls_in_expr(&decl.initializer, in_loop);
            }
            Statement::Return(ret) => {
                for val in ret.values.iter() {
                    self.detect_calls_in_expr(val, in_loop);
                }
            }
            _ => {}
        }
    }

    fn detect_calls_in_block<'arena>(
        &mut self,
        block: &Block<'arena>,
        in_loop: &mut HashSet<StringId>,
    ) {
        for stmt in block.statements.iter() {
            self.detect_hot_paths_in_stmt(stmt, in_loop);
        }
    }

    fn detect_calls_in_expr<'arena>(
        &mut self,
        expr: &Expression<'arena>,
        hot_set: &HashSet<StringId>,
    ) {
        match &expr.kind {
            ExpressionKind::Call(func, args, _) => {
                if let ExpressionKind::Identifier(id) = &func.kind {
                    if hot_set.contains(id) {
                        self.hot_paths.insert(*id);
                    }
                }
                self.detect_calls_in_expr(func, hot_set);
                for arg in args.iter() {
                    self.detect_calls_in_expr(&arg.value, hot_set);
                }
            }
            ExpressionKind::MethodCall(obj, method_name, args, _) => {
                self.hot_paths.insert(method_name.node);
                self.detect_calls_in_expr(obj, hot_set);
                for arg in args.iter() {
                    self.detect_calls_in_expr(&arg.value, hot_set);
                }
            }
            ExpressionKind::Binary(_, left, right) => {
                self.detect_calls_in_expr(left, hot_set);
                self.detect_calls_in_expr(right, hot_set);
            }
            ExpressionKind::Unary(_, operand) => self.detect_calls_in_expr(operand, hot_set),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.detect_calls_in_expr(cond, hot_set);
                self.detect_calls_in_expr(then_expr, hot_set);
                self.detect_calls_in_expr(else_expr, hot_set);
            }
            ExpressionKind::Pipe(left, right) => {
                self.detect_calls_in_expr(left, hot_set);
                self.detect_calls_in_expr(right, hot_set);
            }
            ExpressionKind::Match(match_expr) => {
                self.detect_calls_in_expr(match_expr.value, hot_set);
                for arm in match_expr.arms.iter() {
                    match &arm.body {
                        MatchArmBody::Expression(e) => self.detect_calls_in_expr(e, hot_set),
                        MatchArmBody::Block(b) => {
                            self.detect_calls_in_block(b, &mut HashSet::new());
                        }
                    }
                }
            }
            ExpressionKind::Arrow(arrow) => {
                for param in arrow.parameters.iter() {
                    if let Some(default) = &param.default {
                        self.detect_calls_in_expr(default, hot_set);
                    }
                }
                match &arrow.body {
                    ArrowBody::Expression(e) => self.detect_calls_in_expr(e, hot_set),
                    ArrowBody::Block(b) => {
                        self.detect_calls_in_block(b, &mut HashSet::new());
                    }
                }
            }
            ExpressionKind::New(callee, args, _) => {
                self.detect_calls_in_expr(callee, hot_set);
                for arg in args.iter() {
                    self.detect_calls_in_expr(&arg.value, hot_set);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.detect_calls_in_expr(try_expr.expression, hot_set);
                self.detect_calls_in_expr(try_expr.catch_expression, hot_set);
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.detect_calls_in_expr(left, hot_set);
                self.detect_calls_in_expr(right, hot_set);
            }
            ExpressionKind::OptionalMember(obj, _) => self.detect_calls_in_expr(obj, hot_set),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.detect_calls_in_expr(obj, hot_set);
                self.detect_calls_in_expr(index, hot_set);
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.detect_calls_in_expr(obj, hot_set);
                for arg in args.iter() {
                    self.detect_calls_in_expr(&arg.value, hot_set);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.detect_calls_in_expr(obj, hot_set);
                for arg in args.iter() {
                    self.detect_calls_in_expr(&arg.value, hot_set);
                }
            }
            ExpressionKind::TypeAssertion(expr, _) => self.detect_calls_in_expr(expr, hot_set),
            ExpressionKind::Member(obj, _) => self.detect_calls_in_expr(obj, hot_set),
            ExpressionKind::Index(obj, index) => {
                self.detect_calls_in_expr(obj, hot_set);
                self.detect_calls_in_expr(index, hot_set);
            }
            _ => {}
        }
    }
}

impl AggressiveInliningPass {
    fn inline_in_statement<'arena>(
        &mut self,
        stmt: &mut Statement<'arena>,
        arena: &'arena Bump,
        functions: &HashMap<StringId, FunctionDeclaration<'arena>>,
    ) -> bool {
        match stmt {
            Statement::Function(func) => {
                let mut stmts: Vec<Statement<'arena>> = func.body.statements.to_vec();
                let mut changed = false;
                for s in &mut stmts {
                    changed |= self.inline_in_statement(s, arena, functions);
                }
                if changed {
                    func.body.statements = arena.alloc_slice_clone(&stmts);
                }
                changed
            }
            Statement::If(if_stmt) => {
                let mut changed = self.inline_in_expression(&mut if_stmt.condition, arena);
                changed |= self.inline_in_block(&mut if_stmt.then_block, arena, functions);
                let mut new_else_ifs: Vec<_> = if_stmt.else_ifs.to_vec();
                let mut eic = false;
                for else_if in &mut new_else_ifs {
                    eic |= self.inline_in_expression(&mut else_if.condition, arena);
                    eic |= self.inline_in_block(&mut else_if.block, arena, functions);
                }
                if eic {
                    if_stmt.else_ifs = arena.alloc_slice_clone(&new_else_ifs);
                    changed = true;
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.inline_in_block(else_block, arena, functions);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.inline_in_expression(&mut while_stmt.condition, arena);
                changed |= self.inline_in_block(&mut while_stmt.body, arena, functions);
                changed
            }
            Statement::For(for_stmt) => {
                match &**for_stmt {
                    ForStatement::Numeric(for_num_ref) => {
                        let mut new_num = (**for_num_ref).clone();
                        let mut changed = self.inline_in_expression(&mut new_num.start, arena);
                        changed |= self.inline_in_expression(&mut new_num.end, arena);
                        if let Some(step) = &mut new_num.step {
                            changed |= self.inline_in_expression(step, arena);
                        }
                        changed |= self.inline_in_block(&mut new_num.body, arena, functions);
                        if changed {
                            *stmt = Statement::For(
                                arena.alloc(ForStatement::Numeric(arena.alloc(new_num))),
                            );
                        }
                        changed
                    }
                    ForStatement::Generic(for_gen_ref) => {
                        let mut new_gen = for_gen_ref.clone();
                        let mut changed = false;
                        let mut new_iters: Vec<_> = new_gen.iterators.to_vec();
                        for expr in &mut new_iters {
                            changed |= self.inline_in_expression(expr, arena);
                        }
                        if changed {
                            new_gen.iterators = arena.alloc_slice_clone(&new_iters);
                        }
                        changed |= self.inline_in_block(&mut new_gen.body, arena, functions);
                        if changed {
                            *stmt = Statement::For(
                                arena.alloc(ForStatement::Generic(new_gen)),
                            );
                        }
                        changed
                    }
                }
            }
            Statement::Variable(decl) => {
                if let Some(result) = self.try_inline_call(&mut decl.initializer, arena, functions) {
                    match result {
                        InlineResult::Direct(_) => true,
                        InlineResult::Replaced { stmts, .. } => {
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
                if let Some(result) = self.try_inline_call(expr, arena, functions) {
                    match result {
                        InlineResult::Direct(_) => true,
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
                let mut changed = false;
                let ret_span = ret.span;

                let mut values: Vec<Expression<'arena>> = ret.values.to_vec();
                let mut values_changed = false;

                for expr in values.iter_mut() {
                    if let Some(result) = self.try_inline_call(expr, arena, functions) {
                        match result {
                            InlineResult::Direct(_) => {
                                values_changed = true;
                                changed = true;
                            }
                            InlineResult::Replaced { stmts, .. } => {
                                // Update the values slice before creating new return
                                if values_changed {
                                    ret.values = arena.alloc_slice_clone(&values);
                                }
                                let new_ret = ReturnStatement {
                                    values: ret.values,
                                    span: ret_span,
                                };
                                let mut new_stmts = stmts;
                                new_stmts.push(Statement::Return(new_ret));
                                *stmt = Statement::Block(Block {
                                    statements: arena.alloc_slice_clone(&new_stmts),
                                    span: ret_span,
                                });
                                changed = true;
                                return changed;
                            }
                        }
                    }
                }
                if values_changed {
                    ret.values = arena.alloc_slice_clone(&values);
                }
                changed
            }
            _ => false,
        }
    }

    fn inline_in_block<'arena>(
        &mut self,
        block: &mut Block<'arena>,
        arena: &'arena Bump,
        functions: &HashMap<StringId, FunctionDeclaration<'arena>>,
    ) -> bool {
        let mut stmts: Vec<Statement<'arena>> = block.statements.to_vec();
        let mut changed = false;
        for s in &mut stmts {
            changed |= self.inline_in_statement(s, arena, functions);
        }
        if changed {
            block.statements = arena.alloc_slice_clone(&stmts);
        }
        changed
    }

    fn inline_in_expression<'arena>(
        &mut self,
        expr: &mut Expression<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        match &expr.kind {
            ExpressionKind::Call(func, args, type_args) => {
                let type_args = *type_args;
                let mut new_func = (**func).clone();
                let mut changed = self.inline_in_expression(&mut new_func, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.inline_in_expression(&mut arg.value, arena);
                }
                if changed || ac {
                    expr.kind = ExpressionKind::Call(
                        arena.alloc(new_func),
                        arena.alloc_slice_clone(&new_args),
                        type_args,
                    );
                    changed = true;
                }
                changed
            }
            ExpressionKind::MethodCall(obj, method, args, type_args) => {
                let method = method.clone();
                let type_args = *type_args;
                let mut new_obj = (**obj).clone();
                let mut changed = self.inline_in_expression(&mut new_obj, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut ac = false;
                for arg in &mut new_args {
                    ac |= self.inline_in_expression(&mut arg.value, arena);
                }
                if changed || ac {
                    expr.kind = ExpressionKind::MethodCall(
                        arena.alloc(new_obj),
                        method,
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
            ExpressionKind::Unary(op, operand) => {
                let op = *op;
                let mut new_operand = (**operand).clone();
                let changed = self.inline_in_expression(&mut new_operand, arena);
                if changed {
                    expr.kind = ExpressionKind::Unary(op, arena.alloc(new_operand));
                }
                changed
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut new_cond = (**cond).clone();
                let mut new_then = (**then_expr).clone();
                let mut new_else = (**else_expr).clone();
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
            ExpressionKind::Pipe(left, right) => {
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
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
                let mut new_match = match_expr.clone();
                let mut new_value = (*new_match.value).clone();
                let mut changed = self.inline_in_expression(&mut new_value, arena);
                if changed {
                    new_match.value = arena.alloc(new_value);
                }
                let mut new_arms: Vec<_> = new_match.arms.to_vec();
                let mut arms_changed = false;
                for arm in &mut new_arms {
                    match &mut arm.body {
                        MatchArmBody::Expression(e_ref) => {
                            let mut new_e = (**e_ref).clone();
                            if self.inline_in_expression(&mut new_e, arena) {
                                *e_ref = arena.alloc(new_e);
                                arms_changed = true;
                            }
                        }
                        MatchArmBody::Block(block) => {
                            arms_changed |= self.inline_in_block_no_functions(block, arena);
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
                        params_changed |= self.inline_in_expression(default, arena);
                    }
                }
                if params_changed {
                    new_arrow.parameters = arena.alloc_slice_clone(&new_params);
                    changed = true;
                }
                match &mut new_arrow.body {
                    ArrowBody::Expression(e_ref) => {
                        let mut new_e = (**e_ref).clone();
                        if self.inline_in_expression(&mut new_e, arena) {
                            *e_ref = arena.alloc(new_e);
                            changed = true;
                        }
                    }
                    ArrowBody::Block(block) => {
                        changed |= self.inline_in_block_no_functions(block, arena);
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
                let cc = self.inline_in_expression(&mut new_callee, arena);
                let mut new_args: Vec<_> = args.to_vec();
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
                let mut new_try = try_expr.clone();
                let mut new_expression = (*new_try.expression).clone();
                let mut new_catch = (*new_try.catch_expression).clone();
                let ec = self.inline_in_expression(&mut new_expression, arena);
                let cc = self.inline_in_expression(&mut new_catch, arena);
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
            ExpressionKind::OptionalMember(obj, member) => {
                let member = member.clone();
                let mut new_obj = (**obj).clone();
                let changed = self.inline_in_expression(&mut new_obj, arena);
                if changed {
                    expr.kind = ExpressionKind::OptionalMember(arena.alloc(new_obj), member);
                }
                changed
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                let mut new_obj = (**obj).clone();
                let mut new_index = (**index).clone();
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
            ExpressionKind::OptionalCall(obj, args, type_args) => {
                let type_args = *type_args;
                let mut new_obj = (**obj).clone();
                let oc = self.inline_in_expression(&mut new_obj, arena);
                let mut new_args: Vec<_> = args.to_vec();
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
            ExpressionKind::OptionalMethodCall(obj, method, args, type_args) => {
                let method = method.clone();
                let type_args = *type_args;
                let mut new_obj = (**obj).clone();
                let oc = self.inline_in_expression(&mut new_obj, arena);
                let mut new_args: Vec<_> = args.to_vec();
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
            ExpressionKind::TypeAssertion(inner, ty) => {
                let ty = ty.clone();
                let mut new_inner = (**inner).clone();
                let changed = self.inline_in_expression(&mut new_inner, arena);
                if changed {
                    expr.kind = ExpressionKind::TypeAssertion(arena.alloc(new_inner), ty);
                }
                changed
            }
            ExpressionKind::Member(obj, member) => {
                let member = member.clone();
                let mut new_obj = (**obj).clone();
                let changed = self.inline_in_expression(&mut new_obj, arena);
                if changed {
                    expr.kind = ExpressionKind::Member(arena.alloc(new_obj), member);
                }
                changed
            }
            ExpressionKind::Index(obj, index) => {
                let mut new_obj = (**obj).clone();
                let mut new_index = (**index).clone();
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

    /// Helper to inline in a block without function context (used within expression visitors)
    fn inline_in_block_no_functions<'arena>(
        &mut self,
        block: &mut Block<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        let mut stmts: Vec<Statement<'arena>> = block.statements.to_vec();
        let mut changed = false;
        for s in &mut stmts {
            changed |= self.inline_in_statement(s, arena, &HashMap::new());
        }
        if changed {
            block.statements = arena.alloc_slice_clone(&stmts);
        }
        changed
    }
}

impl AggressiveInliningPass {
    fn try_inline_call<'arena>(
        &mut self,
        expr: &mut Expression<'arena>,
        arena: &'arena Bump,
        functions: &HashMap<StringId, FunctionDeclaration<'arena>>,
    ) -> Option<InlineResult<'arena>> {
        if let ExpressionKind::Call(func, args, _) = &expr.kind.clone() {
            if let ExpressionKind::Identifier(func_name) = &func.kind {
                if let Some(func_decl) = functions.get(func_name) {
                    if self.is_aggressively_inlinable(func_decl, *func_name) {
                        let original_size = func_decl.body.statements.len();
                        let result = self.inline_call(func_decl.clone(), args, arena);
                        if let InlineResult::Replaced { ref stmts, .. } = result {
                            let inlined_size = stmts.len();
                            if self.would_exceed_bloat_guard(original_size, inlined_size) {
                                return None;
                            }
                        }
                        match &result {
                            InlineResult::Direct(inlined_expr) => {
                                *expr = inlined_expr.clone();
                            }
                            InlineResult::Replaced { result_var, .. } => {
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

    fn would_exceed_bloat_guard(&self, original: usize, inlined: usize) -> bool {
        if original == 0 {
            return false;
        }
        let ratio = inlined as f64 / original as f64;
        ratio > self.max_code_bloat_ratio
    }

    fn is_aggressively_inlinable<'arena>(
        &self,
        func: &FunctionDeclaration<'arena>,
        func_name: StringId,
    ) -> bool {
        if func.type_parameters.is_some() {
            return false;
        }
        if func.body.statements.len() > self.threshold {
            return false;
        }
        if self.has_complex_control_flow(&func.body) {
            return false;
        }
        let closure_size = self.count_closure_statements(&func.body);
        if closure_size > self.max_total_closure_size {
            return false;
        }
        if self.is_recursive(func, func_name) && !self.hot_paths.contains(&func_name) {
            return false;
        }
        true
    }

    fn is_recursive<'arena>(
        &self,
        func: &FunctionDeclaration<'arena>,
        func_name: StringId,
    ) -> bool {
        for stmt in func.body.statements.iter() {
            if self.contains_call_to(stmt, func_name) {
                return true;
            }
        }
        false
    }

    fn contains_call_to<'arena>(&self, stmt: &Statement<'arena>, name: StringId) -> bool {
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
                    ForStatement::Numeric(for_num) => &for_num.body.statements,
                    ForStatement::Generic(for_gen) => &for_gen.body.statements,
                };
                stmts.iter().any(|s| self.contains_call_to(s, name))
            }
            _ => false,
        }
    }

    fn expr_contains_call_to<'arena>(
        &self,
        expr: &Expression<'arena>,
        name: StringId,
    ) -> bool {
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
}

impl AggressiveInliningPass {
    fn has_complex_control_flow<'arena>(&self, body: &Block<'arena>) -> bool {
        for stmt in body.statements.iter() {
            if self.stmt_has_complex_flow(stmt) {
                return true;
            }
        }
        false
    }

    fn stmt_has_complex_flow<'arena>(&self, stmt: &Statement<'arena>) -> bool {
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

    fn block_has_multiple_returns<'arena>(&self, block: &Block<'arena>) -> bool {
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

    fn count_closure_statements<'arena>(&self, body: &Block<'arena>) -> usize {
        self.count_closures_in_block(body)
    }

    fn count_closures_in_block<'arena>(&self, block: &Block<'arena>) -> usize {
        let mut total = 0;
        for stmt in block.statements.iter() {
            total += self.count_closures_in_stmt(stmt);
        }
        total
    }

    fn count_closures_in_stmt<'arena>(&self, stmt: &Statement<'arena>) -> usize {
        match stmt {
            Statement::Function(func) => {
                let body_size = func.body.statements.len();
                if body_size <= self.closure_threshold {
                    body_size + self.count_closures_in_block(&func.body)
                } else {
                    self.count_closures_in_block(&func.body)
                }
            }
            Statement::Variable(decl) => self.count_closures_in_expr(&decl.initializer),
            Statement::Expression(expr) => self.count_closures_in_expr(expr),
            Statement::If(if_stmt) => {
                self.count_closures_in_expr(&if_stmt.condition)
                    + self.count_closures_in_block(&if_stmt.then_block)
                    + if_stmt.else_ifs.iter().fold(0, |acc, ei| {
                        acc + self.count_closures_in_expr(&ei.condition)
                            + self.count_closures_in_block(&ei.block)
                    })
                    + if_stmt
                        .else_block
                        .as_ref()
                        .map_or(0, |eb| self.count_closures_in_block(eb))
            }
            Statement::While(while_stmt) => {
                self.count_closures_in_expr(&while_stmt.condition)
                    + self.count_closures_in_block(&while_stmt.body)
            }
            Statement::For(for_stmt) => {
                let body = match &**for_stmt {
                    ForStatement::Numeric(for_num) => &for_num.body,
                    ForStatement::Generic(for_gen) => &for_gen.body,
                };
                self.count_closures_in_block(body)
            }
            Statement::Return(ret) => ret
                .values
                .iter()
                .fold(0, |acc, e| acc + self.count_closures_in_expr(e)),
            _ => 0,
        }
    }

    fn count_closures_in_expr<'arena>(&self, expr: &Expression<'arena>) -> usize {
        match &expr.kind {
            ExpressionKind::Function(func) => {
                let body_size = func.body.statements.len();
                if body_size <= self.closure_threshold {
                    body_size + self.count_closures_in_block(&func.body)
                } else {
                    self.count_closures_in_block(&func.body)
                }
            }
            ExpressionKind::Arrow(arrow) => match &arrow.body {
                ArrowBody::Expression(expr) => self.count_closures_in_expr(expr),
                ArrowBody::Block(block) => self.count_closures_in_block(block),
            },
            ExpressionKind::Call(func, args, _) => {
                self.count_closures_in_expr(func)
                    + args
                        .iter()
                        .fold(0, |acc, a| acc + self.count_closures_in_expr(&a.value))
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.count_closures_in_expr(obj)
                    + args
                        .iter()
                        .fold(0, |acc, a| acc + self.count_closures_in_expr(&a.value))
            }
            ExpressionKind::Binary(_, left, right) => {
                self.count_closures_in_expr(left) + self.count_closures_in_expr(right)
            }
            ExpressionKind::Unary(_, operand) => self.count_closures_in_expr(operand),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.count_closures_in_expr(cond)
                    + self.count_closures_in_expr(then_expr)
                    + self.count_closures_in_expr(else_expr)
            }
            ExpressionKind::Pipe(left, right) => {
                self.count_closures_in_expr(left) + self.count_closures_in_expr(right)
            }
            ExpressionKind::Match(match_expr) => {
                self.count_closures_in_expr(match_expr.value)
                    + match_expr.arms.iter().fold(0, |acc, arm| match &arm.body {
                        MatchArmBody::Expression(e) => acc + self.count_closures_in_expr(e),
                        MatchArmBody::Block(b) => acc + self.count_closures_in_block(b),
                    })
            }
            ExpressionKind::New(callee, args, _) => {
                self.count_closures_in_expr(callee)
                    + args
                        .iter()
                        .fold(0, |acc, a| acc + self.count_closures_in_expr(&a.value))
            }
            ExpressionKind::Try(try_expr) => {
                self.count_closures_in_expr(try_expr.expression)
                    + self.count_closures_in_expr(try_expr.catch_expression)
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.count_closures_in_expr(left) + self.count_closures_in_expr(right)
            }
            ExpressionKind::OptionalMember(obj, _) => self.count_closures_in_expr(obj),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.count_closures_in_expr(obj) + self.count_closures_in_expr(index)
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.count_closures_in_expr(obj)
                    + args
                        .iter()
                        .fold(0, |acc, a| acc + self.count_closures_in_expr(&a.value))
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.count_closures_in_expr(obj)
                    + args
                        .iter()
                        .fold(0, |acc, a| acc + self.count_closures_in_expr(&a.value))
            }
            ExpressionKind::TypeAssertion(expr, _) => self.count_closures_in_expr(expr),
            ExpressionKind::Member(obj, _) => self.count_closures_in_expr(obj),
            ExpressionKind::Index(obj, index) => {
                self.count_closures_in_expr(obj) + self.count_closures_in_expr(index)
            }
            _ => 0,
        }
    }
}

impl AggressiveInliningPass {
    fn inline_call<'arena>(
        &mut self,
        func: FunctionDeclaration<'arena>,
        args: &[Argument<'arena>],
        arena: &'arena Bump,
    ) -> InlineResult<'arena> {
        let param_subst = Self::create_parameter_substitution(func.parameters, args);

        if func.body.statements.len() == 1 {
            if let Statement::Return(ret) = &func.body.statements[0] {
                if ret.values.len() == 1 {
                    let mut inlined_expr = ret.values[0].clone();
                    Self::substitute_expression(&mut inlined_expr, &param_subst, arena);
                    return InlineResult::Direct(inlined_expr);
                }
            }
        }

        let mut inlined_body = Vec::new();
        let return_var = self.create_temp_variable();
        let has_return = Self::has_return_value_in_block(&func.body);

        let body_stmts: Vec<Statement<'arena>> = func.body.statements.to_vec();
        for stmt in &body_stmts {
            let inlined_stmt =
                Self::inline_statement_with_subst(stmt, &param_subst, &return_var, has_return, arena);
            inlined_body.push(inlined_stmt);
        }

        InlineResult::Replaced {
            stmts: inlined_body,
            result_var: return_var,
        }
    }

    fn create_parameter_substitution<'arena>(
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
            "String interner not set for AggressiveInliningPass"
        );
        match &self.interner {
            Some(interner) => interner.get_or_intern(&name),
            None => self.unreachable_interner(),
        }
    }

    fn has_return_value_in_block<'arena>(body: &Block<'arena>) -> bool {
        for stmt in body.statements.iter() {
            if let Statement::Return(ret) = stmt {
                if !ret.values.is_empty() {
                    return true;
                }
            }
        }
        false
    }

    fn inline_statement_with_subst<'arena>(
        stmt: &Statement<'arena>,
        param_subst: &HashMap<StringId, Expression<'arena>>,
        return_var: &StringId,
        has_return: bool,
        arena: &'arena Bump,
    ) -> Statement<'arena> {
        match stmt {
            Statement::Variable(decl) => {
                let mut new_decl = decl.clone();
                Self::substitute_expression(&mut new_decl.initializer, param_subst, arena);
                Statement::Variable(new_decl)
            }
            Statement::Expression(expr) => {
                let mut new_expr = expr.clone();
                Self::substitute_expression(&mut new_expr, param_subst, arena);
                Statement::Expression(new_expr)
            }
            Statement::Return(ret) => {
                if !ret.values.is_empty() && has_return {
                    let values: Vec<Expression<'arena>> = ret
                        .values
                        .iter()
                        .map(|v| {
                            let mut substituted = v.clone();
                            Self::substitute_expression(&mut substituted, param_subst, arena);
                            substituted
                        })
                        .collect();
                    let initializer_kind = if values.len() == 1 {
                        values[0].kind.clone()
                    } else {
                        ExpressionKind::Array(arena.alloc_slice_clone(
                            &values
                                .iter()
                                .map(|e| ArrayElement::Expression(e.clone()))
                                .collect::<Vec<_>>(),
                        ))
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

    fn substitute_expression<'arena>(
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
            ExpressionKind::Call(func, args, type_args) => {
                let type_args = *type_args;
                let mut new_func = (**func).clone();
                Self::substitute_expression(&mut new_func, param_subst, arena);
                let mut new_args: Vec<_> = args.to_vec();
                for arg in &mut new_args {
                    Self::substitute_expression(&mut arg.value, param_subst, arena);
                }
                expr.kind = ExpressionKind::Call(
                    arena.alloc(new_func),
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::MethodCall(obj, method, args, type_args) => {
                let method = method.clone();
                let type_args = *type_args;
                let mut new_obj = (**obj).clone();
                Self::substitute_expression(&mut new_obj, param_subst, arena);
                let mut new_args: Vec<_> = args.to_vec();
                for arg in &mut new_args {
                    Self::substitute_expression(&mut arg.value, param_subst, arena);
                }
                expr.kind = ExpressionKind::MethodCall(
                    arena.alloc(new_obj),
                    method,
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::Binary(op, left, right) => {
                let op = *op;
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                Self::substitute_expression(&mut new_left, param_subst, arena);
                Self::substitute_expression(&mut new_right, param_subst, arena);
                expr.kind = ExpressionKind::Binary(
                    op,
                    arena.alloc(new_left),
                    arena.alloc(new_right),
                );
            }
            ExpressionKind::Unary(op, operand) => {
                let op = *op;
                let mut new_operand = (**operand).clone();
                Self::substitute_expression(&mut new_operand, param_subst, arena);
                expr.kind = ExpressionKind::Unary(op, arena.alloc(new_operand));
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut new_cond = (**cond).clone();
                let mut new_then = (**then_expr).clone();
                let mut new_else = (**else_expr).clone();
                Self::substitute_expression(&mut new_cond, param_subst, arena);
                Self::substitute_expression(&mut new_then, param_subst, arena);
                Self::substitute_expression(&mut new_else, param_subst, arena);
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
                        Self::substitute_expression(default, param_subst, arena);
                    }
                }
                new_arrow.parameters = arena.alloc_slice_clone(&new_params);
                match &mut new_arrow.body {
                    ArrowBody::Expression(e_ref) => {
                        let mut new_e = (**e_ref).clone();
                        Self::substitute_expression(&mut new_e, param_subst, arena);
                        *e_ref = arena.alloc(new_e);
                    }
                    ArrowBody::Block(_) => {}
                }
                expr.kind = ExpressionKind::Arrow(new_arrow);
            }
            ExpressionKind::Match(match_expr) => {
                let mut new_match = match_expr.clone();
                let mut new_value = (*new_match.value).clone();
                Self::substitute_expression(&mut new_value, param_subst, arena);
                new_match.value = arena.alloc(new_value);
                let mut new_arms: Vec<_> = new_match.arms.to_vec();
                for arm in &mut new_arms {
                    match &mut arm.body {
                        MatchArmBody::Expression(e_ref) => {
                            let mut new_e = (**e_ref).clone();
                            Self::substitute_expression(&mut new_e, param_subst, arena);
                            *e_ref = arena.alloc(new_e);
                        }
                        MatchArmBody::Block(_) => {}
                    }
                }
                new_match.arms = arena.alloc_slice_clone(&new_arms);
                expr.kind = ExpressionKind::Match(new_match);
            }
            ExpressionKind::New(callee, args, type_args) => {
                let type_args = *type_args;
                let mut new_callee = (**callee).clone();
                Self::substitute_expression(&mut new_callee, param_subst, arena);
                let mut new_args: Vec<_> = args.to_vec();
                for arg in &mut new_args {
                    Self::substitute_expression(&mut arg.value, param_subst, arena);
                }
                expr.kind = ExpressionKind::New(
                    arena.alloc(new_callee),
                    arena.alloc_slice_clone(&new_args),
                    type_args,
                );
            }
            ExpressionKind::Try(try_expr) => {
                let mut new_try = try_expr.clone();
                let mut new_expression = (*new_try.expression).clone();
                let mut new_catch = (*new_try.catch_expression).clone();
                Self::substitute_expression(&mut new_expression, param_subst, arena);
                Self::substitute_expression(&mut new_catch, param_subst, arena);
                new_try.expression = arena.alloc(new_expression);
                new_try.catch_expression = arena.alloc(new_catch);
                expr.kind = ExpressionKind::Try(new_try);
            }
            ExpressionKind::ErrorChain(left, right) => {
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                Self::substitute_expression(&mut new_left, param_subst, arena);
                Self::substitute_expression(&mut new_right, param_subst, arena);
                expr.kind = ExpressionKind::ErrorChain(
                    arena.alloc(new_left),
                    arena.alloc(new_right),
                );
            }
            ExpressionKind::OptionalMember(obj, member) => {
                let member = member.clone();
                let mut new_obj = (**obj).clone();
                Self::substitute_expression(&mut new_obj, param_subst, arena);
                expr.kind = ExpressionKind::OptionalMember(arena.alloc(new_obj), member);
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                let mut new_obj = (**obj).clone();
                let mut new_index = (**index).clone();
                Self::substitute_expression(&mut new_obj, param_subst, arena);
                Self::substitute_expression(&mut new_index, param_subst, arena);
                expr.kind = ExpressionKind::OptionalIndex(
                    arena.alloc(new_obj),
                    arena.alloc(new_index),
                );
            }
            ExpressionKind::OptionalCall(obj, args, type_args) => {
                let type_args = *type_args;
                let mut new_obj = (**obj).clone();
                Self::substitute_expression(&mut new_obj, param_subst, arena);
                let mut new_args: Vec<_> = args.to_vec();
                for arg in &mut new_args {
                    Self::substitute_expression(&mut arg.value, param_subst, arena);
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
                Self::substitute_expression(&mut new_obj, param_subst, arena);
                let mut new_args: Vec<_> = args.to_vec();
                for arg in &mut new_args {
                    Self::substitute_expression(&mut arg.value, param_subst, arena);
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
                Self::substitute_expression(&mut new_inner, param_subst, arena);
                expr.kind = ExpressionKind::TypeAssertion(arena.alloc(new_inner), ty);
            }
            ExpressionKind::Member(obj, member) => {
                let member = member.clone();
                let mut new_obj = (**obj).clone();
                Self::substitute_expression(&mut new_obj, param_subst, arena);
                expr.kind = ExpressionKind::Member(arena.alloc(new_obj), member);
            }
            ExpressionKind::Index(obj, index) => {
                let mut new_obj = (**obj).clone();
                let mut new_index = (**index).clone();
                Self::substitute_expression(&mut new_obj, param_subst, arena);
                Self::substitute_expression(&mut new_index, param_subst, arena);
                expr.kind = ExpressionKind::Index(
                    arena.alloc(new_obj),
                    arena.alloc(new_index),
                );
            }
            _ => {}
        }
    }
}
