// =============================================================================
// O2: String Concatenation Optimization Pass
// =============================================================================

use crate::config::OptimizationLevel;
use crate::optimizer::{ExprVisitor, WholeProgramPass};
use std::sync::Arc;
use typedlua_parser::ast::expression::{
    Argument, ArrayElement, AssignmentOp, BinaryOp, Expression, ExpressionKind, Literal,
};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{
    Block, ForStatement, Statement, VariableDeclaration, VariableKind,
};
use typedlua_parser::ast::Program;
use typedlua_parser::ast::Spanned;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::{StringId, StringInterner};

const MIN_CONCAT_PARTS_FOR_OPTIMIZATION: usize = 3;

pub struct StringConcatOptimizationPass {
    next_temp_id: usize,
    interner: Arc<StringInterner>,
}

impl StringConcatOptimizationPass {
    pub fn new(interner: Arc<StringInterner>) -> Self {
        Self {
            next_temp_id: 0,
            interner,
        }
    }
}

impl ExprVisitor for StringConcatOptimizationPass {
    fn visit_expr(&mut self, expr: &mut Expression) -> bool {
        // Check if this is a concat expression that can be optimized
        if let ExpressionKind::Binary(BinaryOp::Concatenate, _left, _right) = &expr.kind {
            let parts = self.flatten_concat_chain(expr);
            if parts.len() >= MIN_CONCAT_PARTS_FOR_OPTIMIZATION {
                self.replace_with_table_concat(expr, &parts);
                return true;
            }
        }
        false
    }
}

impl WholeProgramPass for StringConcatOptimizationPass {
    fn name(&self) -> &'static str {
        "string-concat-optimization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        self.next_temp_id = 0;

        let mut changed = false;
        let mut i = 0;
        while i < program.statements.len() {
            if self.optimize_statement(&mut program.statements[i]) {
                changed = true;
            }
            i += 1;
        }

        // Also optimize loop-based string concatenation patterns
        if self.optimize_loop_string_concat(&mut program.statements) {
            changed = true;
        }

        Ok(changed)
    }
}

impl StringConcatOptimizationPass {
    fn optimize_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::Variable(decl) => self.optimize_concat_in_variable(decl),
            Statement::Expression(expr) => self.optimize_concat_in_expression(expr),
            Statement::Function(func) => {
                let mut changed = false;
                for s in &mut func.body.statements {
                    if self.optimize_statement(s) {
                        changed = true;
                    }
                }
                changed
            }
            Statement::Return(ret) => {
                let mut changed = false;
                for expr in &mut ret.values {
                    if self.optimize_concat_expression(expr) {
                        changed = true;
                    }
                }
                changed
            }
            Statement::If(if_stmt) => {
                let mut changed = false;
                for s in &mut if_stmt.then_block.statements {
                    if self.optimize_statement(s) {
                        changed = true;
                    }
                }
                for else_if in &mut if_stmt.else_ifs {
                    for s in &mut else_if.block.statements {
                        if self.optimize_statement(s) {
                            changed = true;
                        }
                    }
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    for s in &mut else_block.statements {
                        if self.optimize_statement(s) {
                            changed = true;
                        }
                    }
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = false;
                for s in &mut while_stmt.body.statements {
                    if self.optimize_statement(s) {
                        changed = true;
                    }
                }
                changed
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Generic(for_gen) => {
                    let mut changed = false;
                    for s in &mut for_gen.body.statements {
                        if self.optimize_statement(s) {
                            changed = true;
                        }
                    }
                    changed
                }
                ForStatement::Numeric(for_num) => {
                    let mut changed = false;
                    for s in &mut for_num.body.statements {
                        if self.optimize_statement(s) {
                            changed = true;
                        }
                    }
                    changed
                }
            },
            Statement::Repeat(repeat_stmt) => {
                let mut changed = false;
                for s in &mut repeat_stmt.body.statements {
                    if self.optimize_statement(s) {
                        changed = true;
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn optimize_concat_in_variable(&mut self, decl: &mut VariableDeclaration) -> bool {
        self.optimize_concat_expression(&mut decl.initializer)
    }

    /// Optimizes loop-based string concatenation patterns
    /// Transforms: local s = ""; for ... do s = s .. value end
    /// Into: local t = {}; for ... do table.insert(t, value) end; local s = table.concat(t)
    fn optimize_loop_string_concat(&mut self, statements: &mut Vec<Statement>) -> bool {
        let mut changed = false;
        let mut i = 0;

        while i < statements.len() {
            // Look for pattern: local s = "" followed by a loop with s = s .. value
            if let Some((concat_var, loop_idx)) =
                self.find_loop_string_concat_pattern(statements, i)
            {
                // Transform the pattern
                if let Some(new_stmts) =
                    self.transform_loop_string_concat(statements, i, loop_idx, concat_var)
                {
                    // Replace the original statements with transformed ones
                    statements.splice(i..=loop_idx, new_stmts);
                    changed = true;
                    continue;
                }
            }
            i += 1;
        }

        changed
    }

    /// Finds the pattern: local s = "" followed by a loop containing s = s .. value
    fn find_loop_string_concat_pattern(
        &self,
        statements: &[Statement],
        start_idx: usize,
    ) -> Option<(StringId, usize)> {
        // Check for local s = "" at start_idx
        let concat_var = if let Statement::Variable(decl) = &statements[start_idx] {
            if let Pattern::Identifier(ident) = &decl.pattern {
                if let ExpressionKind::Literal(Literal::String(s)) = &decl.initializer.kind {
                    if s.is_empty() {
                        Some(ident.node)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }?;

        // Look for a loop at start_idx + 1 that contains s = s .. value
        if start_idx + 1 < statements.len() {
            let loop_stmt = &statements[start_idx + 1];
            if self.loop_contains_string_concat(loop_stmt, concat_var) {
                return Some((concat_var, start_idx + 1));
            }
        }

        None
    }

    /// Checks if a loop statement contains string concatenation on the given variable
    fn loop_contains_string_concat(&self, stmt: &Statement, var: StringId) -> bool {
        match stmt {
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Generic(for_gen) => {
                    self.block_contains_string_concat(&for_gen.body, var)
                }
                ForStatement::Numeric(for_num) => {
                    self.block_contains_string_concat(&for_num.body, var)
                }
            },
            Statement::While(while_stmt) => {
                self.block_contains_string_concat(&while_stmt.body, var)
            }
            Statement::Repeat(repeat_stmt) => {
                self.block_contains_string_concat(&repeat_stmt.body, var)
            }
            _ => false,
        }
    }

    /// Checks if a block contains string concatenation on the given variable
    fn block_contains_string_concat(&self, block: &Block, var: StringId) -> bool {
        block
            .statements
            .iter()
            .any(|stmt| self.statement_contains_string_concat(stmt, var))
    }

    /// Checks if a statement contains string concatenation on the given variable
    fn statement_contains_string_concat(&self, stmt: &Statement, var: StringId) -> bool {
        match stmt {
            Statement::Expression(expr) => self.expression_is_string_concat(expr, var),
            Statement::Block(block) => self.block_contains_string_concat(block, var),
            Statement::If(if_stmt) => {
                self.block_contains_string_concat(&if_stmt.then_block, var)
                    || if_stmt
                        .else_ifs
                        .iter()
                        .any(|ei| self.block_contains_string_concat(&ei.block, var))
                    || if_stmt
                        .else_block
                        .as_ref()
                        .is_some_and(|b| self.block_contains_string_concat(b, var))
            }
            _ => false,
        }
    }

    /// Checks if an expression is s = s .. value or s ..= value
    fn expression_is_string_concat(&self, expr: &Expression, var: StringId) -> bool {
        match &expr.kind {
            ExpressionKind::Assignment(target, AssignmentOp::Assign, value) => {
                // Check if target is the variable
                if let ExpressionKind::Identifier(target_id) = &target.kind {
                    if *target_id == var {
                        // Check if value is var .. something
                        if let ExpressionKind::Binary(BinaryOp::Concatenate, left, _) = &value.kind
                        {
                            if let ExpressionKind::Identifier(left_id) = &left.kind {
                                return *left_id == var;
                            }
                        }
                    }
                }
                false
            }
            ExpressionKind::Assignment(target, AssignmentOp::ConcatenateAssign, _) => {
                // Check if target is the variable
                if let ExpressionKind::Identifier(target_id) = &target.kind {
                    *target_id == var
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Transforms the loop-based string concatenation pattern
    fn transform_loop_string_concat(
        &mut self,
        statements: &[Statement],
        _var_decl_idx: usize,
        loop_idx: usize,
        concat_var: StringId,
    ) -> Option<Vec<Statement>> {
        let temp_table_var = self.next_temp_id;
        self.next_temp_id += 1;
        let temp_table_name = format!("__str_concat_{}", temp_table_var);
        let temp_table_id = self.interner.get_or_intern(&temp_table_name);

        // Create: local __str_concat_N = {}
        let table_decl = Statement::Variable(VariableDeclaration {
            kind: VariableKind::Local,
            pattern: Pattern::Identifier(Spanned::new(temp_table_id, Span::dummy())),
            type_annotation: None,
            initializer: Expression::new(ExpressionKind::Array(Vec::new()), Span::dummy()),
            span: Span::dummy(),
        });

        // Clone and transform the loop
        let mut transformed_loop = statements[loop_idx].clone();
        self.transform_loop_body(&mut transformed_loop, concat_var, temp_table_id);

        // Create: local s = table.concat(__str_concat_N)
        let concat_decl = Statement::Variable(VariableDeclaration {
            kind: VariableKind::Local,
            pattern: Pattern::Identifier(Spanned::new(concat_var, Span::dummy())),
            type_annotation: None,
            initializer: Expression::new(
                ExpressionKind::Call(
                    Box::new(Expression::new(
                        ExpressionKind::Member(
                            Box::new(Expression::new(
                                ExpressionKind::Identifier(self.interner.get_or_intern("table")),
                                Span::dummy(),
                            )),
                            Spanned::new(self.interner.get_or_intern("concat"), Span::dummy()),
                        ),
                        Span::dummy(),
                    )),
                    vec![Argument {
                        value: Expression::new(
                            ExpressionKind::Identifier(temp_table_id),
                            Span::dummy(),
                        ),
                        is_spread: false,
                        span: Span::dummy(),
                    }],
                    None,
                ),
                Span::dummy(),
            ),
            span: Span::dummy(),
        });

        Some(vec![table_decl, transformed_loop, concat_decl])
    }

    /// Transforms the loop body to use table.insert instead of string concatenation
    fn transform_loop_body(
        &mut self,
        stmt: &mut Statement,
        concat_var: StringId,
        table_var: StringId,
    ) {
        match stmt {
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Generic(for_gen) => {
                    self.transform_block(&mut for_gen.body, concat_var, table_var);
                }
                ForStatement::Numeric(for_num) => {
                    self.transform_block(&mut for_num.body, concat_var, table_var);
                }
            },
            Statement::While(while_stmt) => {
                self.transform_block(&mut while_stmt.body, concat_var, table_var);
            }
            Statement::Repeat(repeat_stmt) => {
                self.transform_block(&mut repeat_stmt.body, concat_var, table_var);
            }
            _ => {}
        }
    }

    /// Transforms a block to use table.insert instead of string concatenation
    fn transform_block(&mut self, block: &mut Block, concat_var: StringId, table_var: StringId) {
        for stmt in &mut block.statements {
            self.transform_statement(stmt, concat_var, table_var);
        }
    }

    /// Transforms a statement to use table.insert instead of string concatenation
    fn transform_statement(
        &mut self,
        stmt: &mut Statement,
        concat_var: StringId,
        table_var: StringId,
    ) {
        match stmt {
            Statement::Expression(expr) => {
                if let Some(new_stmt) = self.transform_string_concat(expr, concat_var, table_var) {
                    *stmt = new_stmt;
                }
            }
            Statement::Block(block) => {
                self.transform_block(block, concat_var, table_var);
            }
            Statement::If(if_stmt) => {
                self.transform_block(&mut if_stmt.then_block, concat_var, table_var);
                for else_if in &mut if_stmt.else_ifs {
                    self.transform_block(&mut else_if.block, concat_var, table_var);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    self.transform_block(else_block, concat_var, table_var);
                }
            }
            _ => {}
        }
    }

    /// Transforms s = s .. value or s ..= value into table.insert(t, value)
    fn transform_string_concat(
        &mut self,
        expr: &Expression,
        concat_var: StringId,
        table_var: StringId,
    ) -> Option<Statement> {
        match &expr.kind {
            ExpressionKind::Assignment(target, AssignmentOp::Assign, value) => {
                if let ExpressionKind::Identifier(target_id) = &target.kind {
                    if *target_id == concat_var {
                        if let ExpressionKind::Binary(BinaryOp::Concatenate, _, right) = &value.kind
                        {
                            // Transform s = s .. right into table.insert(t, right)
                            return Some(Statement::Expression(Expression::new(
                                ExpressionKind::Call(
                                    Box::new(Expression::new(
                                        ExpressionKind::Member(
                                            Box::new(Expression::new(
                                                ExpressionKind::Identifier(
                                                    self.interner.get_or_intern("table"),
                                                ),
                                                Span::dummy(),
                                            )),
                                            Spanned::new(
                                                self.interner.get_or_intern("insert"),
                                                Span::dummy(),
                                            ),
                                        ),
                                        Span::dummy(),
                                    )),
                                    vec![
                                        Argument {
                                            value: Expression::new(
                                                ExpressionKind::Identifier(table_var),
                                                Span::dummy(),
                                            ),
                                            is_spread: false,
                                            span: Span::dummy(),
                                        },
                                        Argument {
                                            value: *right.clone(),
                                            is_spread: false,
                                            span: Span::dummy(),
                                        },
                                    ],
                                    None,
                                ),
                                Span::dummy(),
                            )));
                        }
                    }
                }
                None
            }
            ExpressionKind::Assignment(target, AssignmentOp::ConcatenateAssign, right) => {
                if let ExpressionKind::Identifier(target_id) = &target.kind {
                    if *target_id == concat_var {
                        // Transform s ..= right into table.insert(t, right)
                        return Some(Statement::Expression(Expression::new(
                            ExpressionKind::Call(
                                Box::new(Expression::new(
                                    ExpressionKind::Member(
                                        Box::new(Expression::new(
                                            ExpressionKind::Identifier(
                                                self.interner.get_or_intern("table"),
                                            ),
                                            Span::dummy(),
                                        )),
                                        Spanned::new(
                                            self.interner.get_or_intern("insert"),
                                            Span::dummy(),
                                        ),
                                    ),
                                    Span::dummy(),
                                )),
                                vec![
                                    Argument {
                                        value: Expression::new(
                                            ExpressionKind::Identifier(table_var),
                                            Span::dummy(),
                                        ),
                                        is_spread: false,
                                        span: Span::dummy(),
                                    },
                                    Argument {
                                        value: *right.clone(),
                                        is_spread: false,
                                        span: Span::dummy(),
                                    },
                                ],
                                None,
                            ),
                            Span::dummy(),
                        )));
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn optimize_concat_in_expression(&mut self, expr: &mut Expression) -> bool {
        self.optimize_concat_expression(expr)
    }

    fn optimize_concat_expression(&mut self, expr: &mut Expression) -> bool {
        if let ExpressionKind::Binary(BinaryOp::Concatenate, _left, _right) = &expr.kind {
            let parts = self.flatten_concat_chain(expr);
            if parts.len() >= MIN_CONCAT_PARTS_FOR_OPTIMIZATION {
                self.replace_with_table_concat(expr, &parts);
                return true;
            }
        }
        false
    }

    fn flatten_concat_chain(&self, expr: &Expression) -> Vec<Expression> {
        fn flatten_inner(expr: &Expression, result: &mut Vec<Expression>) {
            match &expr.kind {
                ExpressionKind::Binary(BinaryOp::Concatenate, left, right) => {
                    flatten_inner(left, result);
                    flatten_inner(right, result);
                }
                ExpressionKind::Parenthesized(inner) => {
                    flatten_inner(inner, result);
                }
                _ => {
                    result.push(expr.clone());
                }
            }
        }
        let mut parts = Vec::new();
        flatten_inner(expr, &mut parts);
        parts
    }

    fn replace_with_table_concat(&self, expr: &mut Expression, parts: &[Expression]) {
        let table_expr = Expression::new(
            ExpressionKind::Array(
                parts
                    .iter()
                    .map(|p| ArrayElement::Expression(p.clone()))
                    .collect(),
            ),
            Span::dummy(),
        );

        let concat_call = Expression::new(
            ExpressionKind::Call(
                Box::new(Expression::new(
                    ExpressionKind::Member(
                        Box::new(Expression::new(
                            ExpressionKind::Identifier(self.interner.get_or_intern("table")),
                            Span::dummy(),
                        )),
                        Spanned::new(self.interner.get_or_intern("concat"), Span::dummy()),
                    ),
                    Span::dummy(),
                )),
                vec![Argument {
                    value: table_expr,
                    is_spread: false,
                    span: Span::dummy(),
                }],
                None,
            ),
            Span::dummy(),
        );

        *expr = concat_call;
    }
}

impl Default for StringConcatOptimizationPass {
    fn default() -> Self {
        Self::new(Arc::new(StringInterner::new()))
    }
}
