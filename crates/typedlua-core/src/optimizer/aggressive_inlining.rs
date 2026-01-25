use crate::config::OptimizationLevel;
use crate::errors::CompilationError;
use crate::optimizer::OptimizationPass;
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
use typedlua_parser::ast::Program;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::StringId;
use typedlua_parser::string_interner::StringInterner;

enum InlineResult {
    Direct(Box<Expression>),
    Replaced {
        stmts: Vec<Statement>,
        result_var: StringId,
    },
}

pub struct AggressiveInliningPass {
    threshold: usize,
    closure_threshold: usize,
    max_total_closure_size: usize,
    max_code_bloat_ratio: f64,
    next_temp_id: usize,
    functions: HashMap<StringId, FunctionDeclaration>,
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
            functions: HashMap::new(),
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

impl OptimizationPass for AggressiveInliningPass {
    fn name(&self) -> &'static str {
        "aggressive-inlining"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        self.next_temp_id = 0;
        self.functions.clear();
        self.hot_paths.clear();

        self.collect_functions(program);
        self.detect_hot_paths(program);

        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.inline_in_statement(stmt);
        }

        Ok(changed)
    }
}

impl AggressiveInliningPass {
    fn collect_functions(&mut self, program: &Program) {
        for stmt in &program.statements {
            self.collect_functions_in_stmt(stmt);
        }
    }

    fn collect_functions_in_stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Function(func) => {
                self.functions.insert(func.name.node, func.clone());
                for s in &func.body.statements {
                    self.collect_functions_in_stmt(s);
                }
            }
            Statement::If(if_stmt) => {
                for s in &if_stmt.then_block.statements {
                    self.collect_functions_in_stmt(s);
                }
                for else_if in &if_stmt.else_ifs {
                    for s in &else_if.block.statements {
                        self.collect_functions_in_stmt(s);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for s in &else_block.statements {
                        self.collect_functions_in_stmt(s);
                    }
                }
            }
            Statement::While(while_stmt) => {
                for s in &while_stmt.body.statements {
                    self.collect_functions_in_stmt(s);
                }
            }
            Statement::For(for_stmt) => {
                let body = match &**for_stmt {
                    ForStatement::Numeric(for_num) => &for_num.body,
                    ForStatement::Generic(for_gen) => &for_gen.body,
                };
                for s in &body.statements {
                    self.collect_functions_in_stmt(s);
                }
            }
            _ => {}
        }
    }

    fn detect_hot_paths(&mut self, program: &Program) {
        for stmt in &program.statements {
            self.detect_hot_paths_in_stmt(stmt, &mut HashSet::new());
        }
    }

    fn detect_hot_paths_in_stmt(&mut self, stmt: &Statement, in_loop: &mut HashSet<StringId>) {
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
                for s in &if_stmt.then_block.statements {
                    self.detect_hot_paths_in_stmt(s, in_loop);
                }
                for else_if in &if_stmt.else_ifs {
                    for s in &else_if.block.statements {
                        self.detect_hot_paths_in_stmt(s, in_loop);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for s in &else_block.statements {
                        self.detect_hot_paths_in_stmt(s, in_loop);
                    }
                }
            }
            Statement::Function(func) => {
                for s in &func.body.statements {
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
                for val in &ret.values {
                    self.detect_calls_in_expr(val, in_loop);
                }
            }
            _ => {}
        }
    }

    fn detect_calls_in_block(&mut self, block: &Block, in_loop: &mut HashSet<StringId>) {
        for stmt in &block.statements {
            self.detect_hot_paths_in_stmt(stmt, in_loop);
        }
    }

    fn detect_calls_in_expr(&mut self, expr: &Expression, hot_set: &HashSet<StringId>) {
        match &expr.kind {
            ExpressionKind::Call(func, args, _) => {
                if let ExpressionKind::Identifier(id) = &func.kind {
                    if hot_set.contains(id) {
                        self.hot_paths.insert(*id);
                    }
                }
                self.detect_calls_in_expr(func, hot_set);
                for arg in args {
                    self.detect_calls_in_expr(&arg.value, hot_set);
                }
            }
            ExpressionKind::MethodCall(obj, method_name, args, _) => {
                self.hot_paths.insert(method_name.node);
                self.detect_calls_in_expr(obj, hot_set);
                for arg in args {
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
                self.detect_calls_in_expr(&match_expr.value, hot_set);
                for arm in &match_expr.arms {
                    match &arm.body {
                        MatchArmBody::Expression(e) => self.detect_calls_in_expr(e, hot_set),
                        MatchArmBody::Block(b) => {
                            self.detect_calls_in_block(b, &mut HashSet::new());
                        }
                    }
                }
            }
            ExpressionKind::Arrow(arrow) => {
                for param in &arrow.parameters {
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
            ExpressionKind::New(callee, args) => {
                self.detect_calls_in_expr(callee, hot_set);
                for arg in args {
                    self.detect_calls_in_expr(&arg.value, hot_set);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.detect_calls_in_expr(&try_expr.expression, hot_set);
                self.detect_calls_in_expr(&try_expr.catch_expression, hot_set);
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
                for arg in args {
                    self.detect_calls_in_expr(&arg.value, hot_set);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.detect_calls_in_expr(obj, hot_set);
                for arg in args {
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
    fn inline_in_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::Function(func) => {
                let mut changed = false;
                for s in &mut func.body.statements {
                    changed |= self.inline_in_statement(s);
                }
                changed
            }
            Statement::If(if_stmt) => {
                let mut changed = self.inline_in_expression(&mut if_stmt.condition);
                changed |= self.inline_in_block(&mut if_stmt.then_block);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.inline_in_expression(&mut else_if.condition);
                    changed |= self.inline_in_block(&mut else_if.block);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.inline_in_block(else_block);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.inline_in_expression(&mut while_stmt.condition);
                changed |= self.inline_in_block(&mut while_stmt.body);
                changed
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Numeric(for_num) => {
                    let mut changed = self.inline_in_expression(&mut for_num.start);
                    changed |= self.inline_in_expression(&mut for_num.end);
                    if let Some(step) = &mut for_num.step {
                        changed |= self.inline_in_expression(step);
                    }
                    changed |= self.inline_in_block(&mut for_num.body);
                    changed
                }
                ForStatement::Generic(for_gen) => {
                    let mut changed = false;
                    for expr in &mut for_gen.iterators {
                        changed |= self.inline_in_expression(expr);
                    }
                    changed |= self.inline_in_block(&mut for_gen.body);
                    changed
                }
            },
            Statement::Variable(decl) => {
                if let Some(result) = self.try_inline_call(&mut decl.initializer) {
                    match result {
                        InlineResult::Direct(_) => true,
                        InlineResult::Replaced { stmts, .. } => {
                            let span = decl.span;
                            let var_stmt = Statement::Variable(decl.clone());
                            *stmt = Statement::Block(Block {
                                statements: {
                                    let mut new_stmts = stmts;
                                    new_stmts.push(var_stmt);
                                    new_stmts
                                },
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
                if let Some(result) = self.try_inline_call(expr) {
                    match result {
                        InlineResult::Direct(_) => true,
                        InlineResult::Replaced { stmts, .. } => {
                            let span = expr.span;
                            *stmt = Statement::Block(Block {
                                statements: stmts,
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

                for expr in ret.values.iter_mut() {
                    if let Some(result) = self.try_inline_call(expr) {
                        match result {
                            InlineResult::Direct(_) => {
                                changed = true;
                            }
                            InlineResult::Replaced { stmts, .. } => {
                                let new_ret = ReturnStatement {
                                    values: ret.values.clone(),
                                    span: ret_span,
                                };
                                *stmt = Statement::Block(Block {
                                    statements: {
                                        let mut new_stmts = stmts;
                                        new_stmts.push(Statement::Return(new_ret));
                                        new_stmts
                                    },
                                    span: ret_span,
                                });
                                changed = true;
                                break;
                            }
                        }
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn inline_in_block(&mut self, block: &mut Block) -> bool {
        let mut changed = false;
        let mut i = 0;
        while i < block.statements.len() {
            changed |= self.inline_in_statement(&mut block.statements[i]);
            i += 1;
        }
        changed
    }

    fn inline_in_expression(&mut self, expr: &mut Expression) -> bool {
        match &mut expr.kind {
            ExpressionKind::Call(func, args, _) => {
                let mut changed = self.inline_in_expression(func);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                if let Some(InlineResult::Direct(_)) = self.try_inline_call(expr) {
                    changed = true;
                }
                changed
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                let mut changed = self.inline_in_expression(obj);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Binary(_op, left, right) => {
                let mut changed = self.inline_in_expression(left);
                changed |= self.inline_in_expression(right);
                changed
            }
            ExpressionKind::Unary(_op, operand) => self.inline_in_expression(operand),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut changed = self.inline_in_expression(cond);
                changed |= self.inline_in_expression(then_expr);
                changed |= self.inline_in_expression(else_expr);
                changed
            }
            ExpressionKind::Pipe(left, right) => {
                let mut changed = self.inline_in_expression(left);
                changed |= self.inline_in_expression(right);
                changed
            }
            ExpressionKind::Match(match_expr) => {
                let mut changed = self.inline_in_expression(&mut match_expr.value);
                for arm in &mut match_expr.arms {
                    match &mut arm.body {
                        MatchArmBody::Expression(expr) => {
                            changed |= self.inline_in_expression(expr);
                        }
                        MatchArmBody::Block(block) => {
                            changed |= self.inline_in_block(block);
                        }
                    }
                }
                changed
            }
            ExpressionKind::Arrow(arrow) => {
                let mut changed = false;
                for param in &mut arrow.parameters {
                    if let Some(default) = &mut param.default {
                        changed |= self.inline_in_expression(default);
                    }
                }
                match &mut arrow.body {
                    ArrowBody::Expression(expr) => {
                        changed |= self.inline_in_expression(expr);
                    }
                    ArrowBody::Block(block) => {
                        changed |= self.inline_in_block(block);
                    }
                }
                changed
            }
            ExpressionKind::New(callee, args) => {
                let mut changed = self.inline_in_expression(callee);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Try(try_expr) => {
                let mut changed = self.inline_in_expression(&mut try_expr.expression);
                changed |= self.inline_in_expression(&mut try_expr.catch_expression);
                changed
            }
            ExpressionKind::ErrorChain(left, right) => {
                let mut changed = self.inline_in_expression(left);
                changed |= self.inline_in_expression(right);
                changed
            }
            ExpressionKind::OptionalMember(obj, _) => self.inline_in_expression(obj),
            ExpressionKind::OptionalIndex(obj, index) => {
                let mut changed = self.inline_in_expression(obj);
                changed |= self.inline_in_expression(index);
                changed
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                let mut changed = self.inline_in_expression(obj);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                let mut changed = self.inline_in_expression(obj);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::TypeAssertion(expr, _) => self.inline_in_expression(expr),
            ExpressionKind::Member(obj, _) => self.inline_in_expression(obj),
            ExpressionKind::Index(obj, index) => {
                let mut changed = self.inline_in_expression(obj);
                changed |= self.inline_in_expression(index);
                changed
            }
            _ => false,
        }
    }
}

impl AggressiveInliningPass {
    fn try_inline_call(&mut self, expr: &mut Expression) -> Option<InlineResult> {
        if let ExpressionKind::Call(func, args, _) = &expr.kind.clone() {
            if let ExpressionKind::Identifier(func_name) = &func.kind {
                if let Some(func_decl) = self.find_function_definition(*func_name) {
                    if self.is_aggressively_inlinable(func_decl, *func_name) {
                        let original_size = self.count_statements(func_decl);
                        let result = self.inline_call(func_decl.clone(), args);
                        if let InlineResult::Replaced { ref stmts, .. } = result {
                            let inlined_size = stmts.len();
                            if self.would_exceed_bloat_guard(original_size, inlined_size) {
                                return None;
                            }
                        }
                        match &result {
                            InlineResult::Direct(inlined_expr) => {
                                *expr = (**inlined_expr).clone();
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

    fn find_function_definition(&self, name: StringId) -> Option<&FunctionDeclaration> {
        self.functions.get(&name)
    }

    fn count_statements(&self, func: &FunctionDeclaration) -> usize {
        func.body.statements.len()
    }

    fn would_exceed_bloat_guard(&self, original: usize, inlined: usize) -> bool {
        if original == 0 {
            return false;
        }
        let ratio = inlined as f64 / original as f64;
        ratio > self.max_code_bloat_ratio
    }

    fn is_aggressively_inlinable(&self, func: &FunctionDeclaration, func_name: StringId) -> bool {
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

    fn is_recursive(&self, func: &FunctionDeclaration, func_name: StringId) -> bool {
        for stmt in &func.body.statements {
            if self.contains_call_to(stmt, func_name) {
                return true;
            }
        }
        false
    }

    fn contains_call_to(&self, stmt: &Statement, name: StringId) -> bool {
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

    fn expr_contains_call_to(&self, expr: &Expression, name: StringId) -> bool {
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
                for param in &arrow.parameters {
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
                self.expr_contains_call_to(&match_expr.value, name)
                    || match_expr.arms.iter().any(|arm| match &arm.body {
                        MatchArmBody::Expression(expr) => self.expr_contains_call_to(expr, name),
                        MatchArmBody::Block(block) => block
                            .statements
                            .iter()
                            .any(|s| self.contains_call_to(s, name)),
                    })
            }
            ExpressionKind::New(callee, args) => {
                self.expr_contains_call_to(callee, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::Try(try_expr) => {
                self.expr_contains_call_to(&try_expr.expression, name)
                    || self.expr_contains_call_to(&try_expr.catch_expression, name)
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
    fn has_complex_control_flow(&self, body: &Block) -> bool {
        for stmt in &body.statements {
            if self.stmt_has_complex_flow(stmt) {
                return true;
            }
        }
        false
    }

    fn stmt_has_complex_flow(&self, stmt: &Statement) -> bool {
        match stmt {
            Statement::If(if_stmt) => {
                if self.block_has_multiple_returns(&if_stmt.then_block) {
                    return true;
                }
                for else_if in &if_stmt.else_ifs {
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

    fn block_has_multiple_returns(&self, block: &Block) -> bool {
        let mut return_count = 0;
        for stmt in &block.statements {
            if matches!(stmt, Statement::Return(_)) {
                return_count += 1;
                if return_count > 1 {
                    return true;
                }
            }
        }
        false
    }

    fn count_closure_statements(&self, body: &Block) -> usize {
        self.count_closures_in_block(body)
    }

    fn count_closures_in_block(&self, block: &Block) -> usize {
        let mut total = 0;
        for stmt in &block.statements {
            total += self.count_closures_in_stmt(stmt);
        }
        total
    }

    fn count_closures_in_stmt(&self, stmt: &Statement) -> usize {
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

    fn count_closures_in_expr(&self, expr: &Expression) -> usize {
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
                self.count_closures_in_expr(&match_expr.value)
                    + match_expr.arms.iter().fold(0, |acc, arm| match &arm.body {
                        MatchArmBody::Expression(e) => acc + self.count_closures_in_expr(e),
                        MatchArmBody::Block(b) => acc + self.count_closures_in_block(b),
                    })
            }
            ExpressionKind::New(callee, args) => {
                self.count_closures_in_expr(callee)
                    + args
                        .iter()
                        .fold(0, |acc, a| acc + self.count_closures_in_expr(&a.value))
            }
            ExpressionKind::Try(try_expr) => {
                self.count_closures_in_expr(&try_expr.expression)
                    + self.count_closures_in_expr(&try_expr.catch_expression)
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
    fn inline_call(&mut self, func: FunctionDeclaration, args: &[Argument]) -> InlineResult {
        let param_subst = self.create_parameter_substitution(&func.parameters, args);

        if func.body.statements.len() == 1 {
            if let Statement::Return(ret) = &func.body.statements[0] {
                if ret.values.len() == 1 {
                    let mut inlined_expr = ret.values[0].clone();
                    self.inline_expression(&mut inlined_expr, &param_subst);
                    return InlineResult::Direct(Box::new(inlined_expr));
                }
            }
        }

        let mut inlined_body = Vec::new();
        let return_var = self.create_temp_variable();
        let has_return = self.has_return_value(&func.body);

        for stmt in &func.body.statements.clone() {
            let inlined_stmt = self.inline_statement(stmt, &param_subst, &return_var, has_return);
            inlined_body.push(inlined_stmt);
        }

        InlineResult::Replaced {
            stmts: inlined_body,
            result_var: return_var,
        }
    }

    fn create_parameter_substitution(
        &self,
        parameters: &[Parameter],
        args: &[Argument],
    ) -> HashMap<StringId, Expression> {
        let mut subst = HashMap::new();
        for (param, arg) in parameters.iter().zip(args.iter()) {
            if let Pattern::Identifier(ident) = &param.pattern {
                subst.insert(ident.node, arg.value.clone());
            }
        }
        subst
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
            None => unsafe { std::hint::unreachable_unchecked() },
        }
    }

    fn has_return_value(&self, body: &Block) -> bool {
        for stmt in &body.statements {
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
        stmt: &Statement,
        param_subst: &HashMap<StringId, Expression>,
        return_var: &StringId,
        has_return: bool,
    ) -> Statement {
        match stmt {
            Statement::Variable(decl) => {
                let mut new_decl = decl.clone();
                self.inline_expression(&mut new_decl.initializer, param_subst);
                Statement::Variable(new_decl)
            }
            Statement::Expression(expr) => {
                let mut new_expr = expr.clone();
                self.inline_expression(&mut new_expr, param_subst);
                Statement::Expression(new_expr)
            }
            Statement::Return(ret) => {
                if !ret.values.is_empty() && has_return {
                    let values: Vec<Expression> = ret
                        .values
                        .iter()
                        .map(|v| {
                            let val = v.clone();
                            let mut substituted = val.clone();
                            self.inline_expression(&mut substituted, param_subst);
                            substituted
                        })
                        .collect();
                    Statement::Variable(VariableDeclaration {
                        kind: VariableKind::Local,
                        pattern: Pattern::Identifier(crate::ast::Spanned::new(
                            *return_var,
                            Span::dummy(),
                        )),
                        type_annotation: None,
                        initializer: Expression::new(
                            if values.len() == 1 {
                                values[0].kind.clone()
                            } else {
                                ExpressionKind::Array(
                                    values
                                        .iter()
                                        .map(|e| ArrayElement::Expression(e.clone()))
                                        .collect(),
                                )
                            },
                            Span::dummy(),
                        ),
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
        expr: &mut Expression,
        param_subst: &HashMap<StringId, Expression>,
    ) {
        match &mut expr.kind {
            ExpressionKind::Identifier(id) => {
                if let Some(substituted) = param_subst.get(id) {
                    expr.kind = substituted.kind.clone();
                }
            }
            ExpressionKind::Call(func, args, _) => {
                self.inline_expression(func, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.inline_expression(obj, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::Binary(_op, left, right) => {
                self.inline_expression(left, param_subst);
                self.inline_expression(right, param_subst);
            }
            ExpressionKind::Unary(_op, operand) => {
                self.inline_expression(operand, param_subst);
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.inline_expression(cond, param_subst);
                self.inline_expression(then_expr, param_subst);
                self.inline_expression(else_expr, param_subst);
            }
            ExpressionKind::Arrow(arrow) => {
                for param in &mut arrow.parameters {
                    if let Some(default) = &mut param.default {
                        self.inline_expression(default, param_subst);
                    }
                }
                match &mut arrow.body {
                    ArrowBody::Expression(expr) => self.inline_expression(expr, param_subst),
                    ArrowBody::Block(_) => {}
                }
            }
            ExpressionKind::Match(match_expr) => {
                self.inline_expression(&mut match_expr.value, param_subst);
                for arm in &mut match_expr.arms {
                    match &mut arm.body {
                        MatchArmBody::Expression(expr) => self.inline_expression(expr, param_subst),
                        MatchArmBody::Block(_) => {}
                    }
                }
            }
            ExpressionKind::New(callee, args) => {
                self.inline_expression(callee, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.inline_expression(&mut try_expr.expression, param_subst);
                self.inline_expression(&mut try_expr.catch_expression, param_subst);
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.inline_expression(left, param_subst);
                self.inline_expression(right, param_subst);
            }
            ExpressionKind::OptionalMember(obj, _) => self.inline_expression(obj, param_subst),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.inline_expression(obj, param_subst);
                self.inline_expression(index, param_subst);
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.inline_expression(obj, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.inline_expression(obj, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::TypeAssertion(expr, _) => self.inline_expression(expr, param_subst),
            ExpressionKind::Member(obj, _) => self.inline_expression(obj, param_subst),
            ExpressionKind::Index(obj, index) => {
                self.inline_expression(obj, param_subst);
                self.inline_expression(index, param_subst);
            }
            _ => {}
        }
    }
}

use crate::ast::expression::ArrayElement;
