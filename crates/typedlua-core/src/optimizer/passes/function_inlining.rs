// O2: Function Inlining Pass
// =============================================================================

use crate::config::OptimizationLevel;
use crate::optimizer::{PreAnalysisPass, StmtVisitor, WholeProgramPass};
use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;
use typedlua_parser::ast::expression::{ArrowBody, Expression, ExpressionKind};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{
    Block, ForStatement, FunctionDeclaration, Parameter, ReturnStatement, Statement,
    VariableDeclaration, VariableKind,
};
use typedlua_parser::ast::Program;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::{StringId, StringInterner};

enum InlineResult {
    /// Direct expression substitution - for simple single-return functions
    /// The expression can be directly substituted for the call
    Direct(Box<Expression>),
    /// Complex inlining - contains statements to insert and the result variable
    Replaced {
        stmts: Vec<Statement>,
        result_var: StringId,
    },
}

/// Function inlining optimization pass (threshold: 5 statements)
/// Inlines small functions at call sites
pub struct FunctionInliningPass {
    threshold: usize,
    next_temp_id: usize,
    functions: HashMap<StringId, FunctionDeclaration>,
    interner: Option<Arc<StringInterner>>,
}

impl Default for FunctionInliningPass {
    fn default() -> Self {
        Self {
            threshold: 5,
            next_temp_id: 0,
            functions: HashMap::default(),
            interner: None,
        }
    }
}

impl FunctionInliningPass {
    pub fn new(interner: Arc<StringInterner>) -> Self {
        Self {
            threshold: 5,
            next_temp_id: 0,
            functions: HashMap::default(),
            interner: Some(interner),
        }
    }
}

impl PreAnalysisPass for FunctionInliningPass {
    fn analyze(&mut self, program: &Program) {
        self.functions.clear();
        self.collect_functions(program);
    }
}

impl StmtVisitor for FunctionInliningPass {
    fn visit_stmt(&mut self, stmt: &mut Statement) -> bool {
        self.inline_in_statement(stmt)
    }
}

impl WholeProgramPass for FunctionInliningPass {
    fn name(&self) -> &'static str {
        "function-inlining"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        self.next_temp_id = 0;

        // Run pre-analysis
        self.analyze(program);

        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.visit_stmt(stmt);
        }

        Ok(changed)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl FunctionInliningPass {
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
}

impl FunctionInliningPass {
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
                        InlineResult::Direct(_) => {
                            // Expression was directly substituted - no extra statements needed
                            true
                        }
                        InlineResult::Replaced { stmts, .. } => {
                            // decl.initializer has been modified to reference the result variable
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
                        InlineResult::Direct(_) => {
                            // Expression was directly substituted - no extra statements needed
                            true
                        }
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
                                // Expression was directly substituted - no extra statements needed
                                changed = true;
                            }
                            InlineResult::Replaced { stmts, .. } => {
                                // expr has been modified to reference the result variable
                                let span = ret_span;
                                let new_ret = ReturnStatement {
                                    values: ret.values.clone(),
                                    span,
                                };
                                *stmt = Statement::Block(Block {
                                    statements: {
                                        let mut new_stmts = stmts;
                                        new_stmts.push(Statement::Return(new_ret));
                                        new_stmts
                                    },
                                    span,
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
                // Also try to inline this call itself (for nested inlining)
                // Note: For Replaced, we can't easily insert statements here,
                // so we rely on the fixed-point iteration to catch it in the next pass
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
            ExpressionKind::New(callee, args, _) => {
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

    fn try_inline_call(&mut self, expr: &mut Expression) -> Option<InlineResult> {
        if let ExpressionKind::Call(func, args, _) = &expr.kind.clone() {
            if let ExpressionKind::Identifier(func_name) = &func.kind {
                if let Some(func_decl) = self.find_function_definition(expr, *func_name) {
                    if self.is_inlinable(func_decl) {
                        let result = self.inline_call(func_decl.clone(), args);
                        // Replace the call expression based on the inline result
                        match &result {
                            InlineResult::Direct(inlined_expr) => {
                                // Direct substitution - replace call with the inlined expression
                                *expr = (**inlined_expr).clone();
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
        _expr: &Expression,
        name: StringId,
    ) -> Option<&FunctionDeclaration> {
        self.functions.get(&name)
    }

    fn is_inlinable(&self, func: &FunctionDeclaration) -> bool {
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

    fn is_recursive(&self, func: &FunctionDeclaration) -> bool {
        let name = func.name.node;
        for stmt in &func.body.statements {
            if self.contains_call_to(stmt, name) {
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
            ExpressionKind::New(callee, args, _) => {
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

    fn has_closures(&self, body: &Block) -> bool {
        self.block_has_closures(body)
    }

    fn block_has_closures(&self, block: &Block) -> bool {
        for stmt in &block.statements {
            if self.stmt_has_closures(stmt) {
                return true;
            }
        }
        false
    }

    fn stmt_has_closures(&self, stmt: &Statement) -> bool {
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

    fn expr_has_closures(&self, expr: &Expression) -> bool {
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
                self.expr_has_closures(&match_expr.value)
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
                self.expr_has_closures(&try_expr.expression)
                    || self.expr_has_closures(&try_expr.catch_expression)
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

    fn inline_call(&mut self, func: FunctionDeclaration, args: &[Argument]) -> InlineResult {
        let param_subst = self.create_parameter_substitution(&func.parameters, args);

        // Check for simple single-return function: just `return expr`
        if func.body.statements.len() == 1 {
            if let Statement::Return(ret) = &func.body.statements[0] {
                if ret.values.len() == 1 {
                    // Simple case: directly substitute the return expression
                    let mut inlined_expr = ret.values[0].clone();
                    self.inline_expression(&mut inlined_expr, &param_subst);
                    return InlineResult::Direct(Box::new(inlined_expr));
                }
            }
        }

        // Complex case: create intermediate variable
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
                        pattern: Pattern::Identifier(typedlua_parser::ast::Spanned::new(
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
            ExpressionKind::New(callee, args, _) => {
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

use typedlua_parser::ast::expression::Argument;
use typedlua_parser::ast::expression::ArrayElement;
use typedlua_parser::ast::expression::MatchArmBody;
