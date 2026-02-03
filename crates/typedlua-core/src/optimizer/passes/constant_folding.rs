use crate::config::OptimizationLevel;
use crate::optimizer::OptimizationPass;
use typedlua_parser::ast::expression::{BinaryOp, Expression, ExpressionKind, Literal, UnaryOp};
use typedlua_parser::ast::statement::{ForStatement, Statement};
use typedlua_parser::ast::Program;

pub struct ConstantFoldingPass;

impl OptimizationPass for ConstantFoldingPass {
    fn name(&self) -> &'static str {
        "constant-folding"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        let mut changed = false;

        for stmt in &mut program.statements {
            changed |= self.fold_statement(stmt);
        }

        Ok(changed)
    }
}

impl ConstantFoldingPass {
    fn fold_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::Variable(decl) => self.fold_expression(&mut decl.initializer),
            Statement::Expression(expr) => self.fold_expression(expr),
            Statement::If(if_stmt) => {
                let mut changed = self.fold_expression(&mut if_stmt.condition);
                changed |= self.fold_block_statements(&mut if_stmt.then_block.statements);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.fold_expression(&mut else_if.condition);
                    changed |= self.fold_block_statements(&mut else_if.block.statements);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.fold_block_statements(&mut else_block.statements);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.fold_expression(&mut while_stmt.condition);
                changed |= self.fold_block_statements(&mut while_stmt.body.statements);
                changed
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Numeric(for_num) => {
                    let mut changed = self.fold_expression(&mut for_num.start);
                    changed |= self.fold_expression(&mut for_num.end);
                    if let Some(step) = &mut for_num.step {
                        changed |= self.fold_expression(step);
                    }
                    changed |= self.fold_block_statements(&mut for_num.body.statements);
                    changed
                }
                ForStatement::Generic(for_gen) => {
                    let mut changed = false;
                    for expr in &mut for_gen.iterators {
                        changed |= self.fold_expression(expr);
                    }
                    changed |= self.fold_block_statements(&mut for_gen.body.statements);
                    changed
                }
            },
            Statement::Return(ret_stmt) => {
                let mut changed = false;
                for expr in &mut ret_stmt.values {
                    changed |= self.fold_expression(expr);
                }
                changed
            }
            Statement::Function(func) => self.fold_block_statements(&mut func.body.statements),
            Statement::Class(_) => false, // Skip for now
            _ => false,
        }
    }

    fn fold_block_statements(&mut self, stmts: &mut [Statement]) -> bool {
        let mut changed = false;
        for stmt in stmts {
            changed |= self.fold_statement(stmt);
        }
        changed
    }

    fn fold_expression(&mut self, expr: &mut Expression) -> bool {
        match &mut expr.kind {
            ExpressionKind::Binary(op, left, right) => {
                let left_changed = self.fold_expression(left);
                let right_changed = self.fold_expression(right);

                // Try to fold if both operands are literals
                if let (
                    ExpressionKind::Literal(Literal::Number(l)),
                    ExpressionKind::Literal(Literal::Number(r)),
                ) = (&left.kind, &right.kind)
                {
                    if let Some(result) = self.fold_numeric_binary_op(*op, *l, *r) {
                        expr.kind = ExpressionKind::Literal(Literal::Number(result));
                        return true;
                    }
                }

                // Try to fold boolean operations
                if let (
                    ExpressionKind::Literal(Literal::Boolean(l)),
                    ExpressionKind::Literal(Literal::Boolean(r)),
                ) = (&left.kind, &right.kind)
                {
                    if let Some(result) = self.fold_boolean_binary_op(*op, *l, *r) {
                        expr.kind = ExpressionKind::Literal(Literal::Boolean(result));
                        return true;
                    }
                }

                left_changed || right_changed
            }
            ExpressionKind::Unary(op, operand) => {
                let changed = self.fold_expression(operand);

                // Try to fold unary operations
                match (&operand.kind, op) {
                    (ExpressionKind::Literal(Literal::Number(n)), UnaryOp::Negate) => {
                        expr.kind = ExpressionKind::Literal(Literal::Number(-n));
                        return true;
                    }
                    (ExpressionKind::Literal(Literal::Boolean(b)), UnaryOp::Not) => {
                        expr.kind = ExpressionKind::Literal(Literal::Boolean(!b));
                        return true;
                    }
                    _ => {}
                }

                changed
            }
            ExpressionKind::Call(func, args, _) => {
                let mut changed = self.fold_expression(func);
                for arg in args {
                    changed |= self.fold_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Index(obj, index) => {
                let obj_changed = self.fold_expression(obj);
                let index_changed = self.fold_expression(index);
                obj_changed || index_changed
            }
            ExpressionKind::Member(obj, _) => self.fold_expression(obj),
            ExpressionKind::Object(fields) => {
                let mut changed = false;
                for field in fields {
                    match field {
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            value,
                            ..
                        } => {
                            changed |= self.fold_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            ..
                        } => {
                            changed |= self.fold_expression(key);
                            changed |= self.fold_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value, ..
                        } => {
                            changed |= self.fold_expression(value);
                        }
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn fold_numeric_binary_op(&self, op: BinaryOp, left: f64, right: f64) -> Option<f64> {
        let l = left;
        let r = right;

        match op {
            BinaryOp::Add => Some(l + r),
            BinaryOp::Subtract => Some(l - r),
            BinaryOp::Multiply => Some(l * r),
            BinaryOp::Divide => {
                if r != 0.0 {
                    Some(l / r)
                } else {
                    None // Don't fold division by zero
                }
            }
            BinaryOp::Modulo => {
                if r != 0.0 {
                    Some(l % r)
                } else {
                    None
                }
            }
            BinaryOp::Power => Some(l.powf(r)),
            _ => None,
        }
    }

    fn fold_boolean_binary_op(&self, op: BinaryOp, left: bool, right: bool) -> Option<bool> {
        match op {
            BinaryOp::And => Some(left && right),
            BinaryOp::Or => Some(left || right),
            BinaryOp::Equal => Some(left == right),
            BinaryOp::NotEqual => Some(left != right),
            _ => None,
        }
    }
}
