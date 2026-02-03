pub struct AlgebraicSimplificationPass;

impl OptimizationPass for AlgebraicSimplificationPass {
    fn name(&self) -> &'static str {
        "algebraic-simplification"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        let mut changed = false;

        for stmt in &mut program.statements {
            changed |= self.simplify_statement(stmt);
        }

        Ok(changed)
    }
}

impl AlgebraicSimplificationPass {
    fn simplify_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::Variable(decl) => self.simplify_expression(&mut decl.initializer),
            Statement::Expression(expr) => self.simplify_expression(expr),
            Statement::If(if_stmt) => {
                let mut changed = self.simplify_expression(&mut if_stmt.condition);
                changed |= self.simplify_block_statements(&mut if_stmt.then_block.statements);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.simplify_expression(&mut else_if.condition);
                    changed |= self.simplify_block_statements(&mut else_if.block.statements);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.simplify_block_statements(&mut else_block.statements);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.simplify_expression(&mut while_stmt.condition);
                changed |= self.simplify_block_statements(&mut while_stmt.body.statements);
                changed
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Numeric(for_num) => {
                    let mut changed = self.simplify_expression(&mut for_num.start);
                    changed |= self.simplify_expression(&mut for_num.end);
                    if let Some(step) = &mut for_num.step {
                        changed |= self.simplify_expression(step);
                    }
                    changed |= self.simplify_block_statements(&mut for_num.body.statements);
                    changed
                }
                ForStatement::Generic(for_gen) => {
                    let mut changed = false;
                    for expr in &mut for_gen.iterators {
                        changed |= self.simplify_expression(expr);
                    }
                    changed |= self.simplify_block_statements(&mut for_gen.body.statements);
                    changed
                }
            },
            Statement::Return(ret_stmt) => {
                let mut changed = false;
                for expr in &mut ret_stmt.values {
                    changed |= self.simplify_expression(expr);
                }
                changed
            }
            _ => false,
        }
    }

    fn simplify_block_statements(&mut self, stmts: &mut [Statement]) -> bool {
        let mut changed = false;
        for stmt in stmts {
            changed |= self.simplify_statement(stmt);
        }
        changed
    }

    fn simplify_expression(&mut self, expr: &mut Expression) -> bool {
        match &mut expr.kind {
            ExpressionKind::Binary(op, left, right) => {
                let mut changed = self.simplify_expression(left);
                changed |= self.simplify_expression(right);

                // Algebraic simplifications
                match op {
                    // x + 0 = x or 0 + x = x
                    BinaryOp::Add => {
                        if is_zero(&right.kind) {
                            *expr = (**left).clone();
                            return true;
                        }
                        if is_zero(&left.kind) {
                            *expr = (**right).clone();
                            return true;
                        }
                    }
                    // x - 0 = x
                    BinaryOp::Subtract => {
                        if is_zero(&right.kind) {
                            *expr = (**left).clone();
                            return true;
                        }
                    }
                    // x * 0 = 0 or 0 * x = 0
                    BinaryOp::Multiply => {
                        if is_zero(&right.kind) || is_zero(&left.kind) {
                            expr.kind = ExpressionKind::Literal(Literal::Number(0.0));
                            return true;
                        }
                        // x * 1 = x or 1 * x = x
                        if is_one(&right.kind) {
                            *expr = (**left).clone();
                            return true;
                        }
                        if is_one(&left.kind) {
                            *expr = (**right).clone();
                            return true;
                        }
                    }
                    // x / 1 = x
                    BinaryOp::Divide => {
                        if is_one(&right.kind) {
                            *expr = (**left).clone();
                            return true;
                        }
                    }
                    // true && x = x, false && x = false
                    BinaryOp::And => {
                        if let ExpressionKind::Literal(Literal::Boolean(b)) = &left.kind {
                            if *b {
                                *expr = (**right).clone();
                            } else {
                                expr.kind = ExpressionKind::Literal(Literal::Boolean(false));
                            }
                            return true;
                        }
                        if let ExpressionKind::Literal(Literal::Boolean(b)) = &right.kind {
                            if *b {
                                *expr = (**left).clone();
                            } else {
                                expr.kind = ExpressionKind::Literal(Literal::Boolean(false));
                            }
                            return true;
                        }
                    }
                    // true || x = true, false || x = x
                    BinaryOp::Or => {
                        if let ExpressionKind::Literal(Literal::Boolean(b)) = &left.kind {
                            if *b {
                                expr.kind = ExpressionKind::Literal(Literal::Boolean(true));
                            } else {
                                *expr = (**right).clone();
                            }
                            return true;
                        }
                        if let ExpressionKind::Literal(Literal::Boolean(b)) = &right.kind {
                            if *b {
                                expr.kind = ExpressionKind::Literal(Literal::Boolean(true));
                            } else {
                                *expr = (**left).clone();
                            }
                            return true;
                        }
                    }
                    _ => {}
                }

                changed
            }
            ExpressionKind::Unary(op, operand) => {
                let changed = self.simplify_expression(operand);

                // !!x = x (double negation)
                if let UnaryOp::Not = op {
                    if let ExpressionKind::Unary(UnaryOp::Not, inner) = &operand.kind {
                        *expr = (**inner).clone();
                        return true;
                    }
                }

                changed
            }
            ExpressionKind::Call(func, args, _) => {
                let mut changed = self.simplify_expression(func);
                for arg in args {
                    changed |= self.simplify_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Member(obj, _) => self.simplify_expression(obj),
            _ => false,
        }
    }
}

// Helper functions
fn is_zero(expr: &ExpressionKind) -> bool {
    matches!(
        expr,
        ExpressionKind::Literal(Literal::Number(n)) if *n == 0.0
    )
}

fn is_one(expr: &ExpressionKind) -> bool {
    matches!(
        expr,
        ExpressionKind::Literal(Literal::Number(n)) if *n == 1.0
    )
}

// =============================================================================
// O1: Table Preallocation Pass
// =============================================================================

/// Table preallocation optimization pass
/// Analyzes table constructors and adds size hints for Lua
/// Note: This is a placeholder - actual hints would be used by codegen
