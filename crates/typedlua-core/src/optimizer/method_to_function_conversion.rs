use crate::config::OptimizationLevel;

use crate::optimizer::{StmtVisitor, WholeProgramPass};
use std::rc::Rc;
use typedlua_parser::ast::expression::{Expression, ExpressionKind, ReceiverClassInfo};
use typedlua_parser::ast::statement::Statement;
use typedlua_parser::ast::Program;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::StringInterner;

pub struct MethodToFunctionConversionPass {
    interner: Rc<StringInterner>,
}

impl MethodToFunctionConversionPass {
    pub fn new(interner: Rc<StringInterner>) -> Self {
        Self { interner }
    }

    fn convert_in_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::Function(func) => {
                let mut changed = false;
                for s in &mut func.body.statements {
                    changed |= self.convert_in_statement(s);
                }
                changed
            }
            Statement::If(if_stmt) => {
                let mut changed = self.convert_in_expression(&mut if_stmt.condition);
                changed |= self.convert_in_block(&mut if_stmt.then_block);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.convert_in_expression(&mut else_if.condition);
                    changed |= self.convert_in_block(&mut else_if.block);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.convert_in_block(else_block);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.convert_in_expression(&mut while_stmt.condition);
                changed |= self.convert_in_block(&mut while_stmt.body);
                changed
            }
            Statement::For(for_stmt) => {
                use typedlua_parser::ast::statement::ForStatement;
                let body = match &mut **for_stmt {
                    ForStatement::Numeric(for_num) => &mut for_num.body,
                    ForStatement::Generic(for_gen) => &mut for_gen.body,
                };
                let mut changed = false;
                for s in &mut body.statements {
                    changed |= self.convert_in_statement(s);
                }
                changed
            }
            Statement::Repeat(repeat_stmt) => {
                let mut changed = self.convert_in_expression(&mut repeat_stmt.until);
                changed |= self.convert_in_block(&mut repeat_stmt.body);
                changed
            }
            Statement::Return(return_stmt) => {
                let mut changed = false;
                for value in &mut return_stmt.values {
                    changed |= self.convert_in_expression(value);
                }
                changed
            }
            Statement::Expression(expr) => self.convert_in_expression(expr),
            Statement::Block(block) => self.convert_in_block(block),
            Statement::Try(try_stmt) => {
                let mut changed = self.convert_in_block(&mut try_stmt.try_block);
                for clause in &mut try_stmt.catch_clauses {
                    changed |= self.convert_in_block(&mut clause.body);
                }
                if let Some(finally) = &mut try_stmt.finally_block {
                    changed |= self.convert_in_block(finally);
                }
                changed
            }
            _ => false,
        }
    }

    fn convert_in_block(&mut self, block: &mut typedlua_parser::ast::statement::Block) -> bool {
        let mut changed = false;
        for stmt in &mut block.statements {
            changed |= self.convert_in_statement(stmt);
        }
        changed
    }

    fn convert_in_expression(&mut self, expr: &mut Expression) -> bool {
        match &mut expr.kind {
            ExpressionKind::Call(func, args, _) => {
                let mut changed = self.convert_in_expression(func);
                for arg in args.iter_mut() {
                    changed |= self.convert_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::MethodCall(obj, method_name, args, _) => {
                let mut changed = self.convert_in_expression(obj);
                for arg in args.iter_mut() {
                    changed |= self.convert_in_expression(&mut arg.value);
                }

                if let Some(receiver_info) = &expr.receiver_class {
                    if let Some(converted) = self.convert_method_call_to_function_call(
                        obj,
                        receiver_info,
                        method_name,
                        args,
                        expr.span,
                    ) {
                        expr.kind = converted;
                        expr.receiver_class = None;
                        changed = true;
                    }
                }

                changed
            }
            ExpressionKind::Binary(_op, left, right) => {
                let mut changed = self.convert_in_expression(left);
                changed |= self.convert_in_expression(right);
                changed
            }
            ExpressionKind::Unary(_op, operand) => self.convert_in_expression(operand),
            ExpressionKind::Assignment(left, _op, right) => {
                let mut changed = self.convert_in_expression(left);
                changed |= self.convert_in_expression(right);
                changed
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut changed = self.convert_in_expression(cond);
                changed |= self.convert_in_expression(then_expr);
                changed |= self.convert_in_expression(else_expr);
                changed
            }
            ExpressionKind::Pipe(left, right) => {
                let mut changed = self.convert_in_expression(left);
                changed |= self.convert_in_expression(right);
                changed
            }
            ExpressionKind::Match(match_expr) => {
                let mut changed = self.convert_in_expression(&mut match_expr.value);
                for arm in &mut match_expr.arms {
                    match &mut arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                            changed |= self.convert_in_expression(expr);
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            changed |= self.convert_in_block(block);
                        }
                    }
                }
                changed
            }
            ExpressionKind::Arrow(arrow) => {
                let mut changed = false;
                for param in &mut arrow.parameters {
                    if let Some(default) = &mut param.default {
                        changed |= self.convert_in_expression(default);
                    }
                }
                match &mut arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                        changed |= self.convert_in_expression(expr);
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        changed |= self.convert_in_block(block);
                    }
                }
                changed
            }
            ExpressionKind::New(callee, args, _) => {
                let mut changed = self.convert_in_expression(callee);
                for arg in args {
                    changed |= self.convert_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Try(try_expr) => {
                let mut changed = self.convert_in_expression(&mut try_expr.expression);
                changed |= self.convert_in_expression(&mut try_expr.catch_expression);
                changed
            }
            ExpressionKind::ErrorChain(left, right) => {
                let mut changed = self.convert_in_expression(left);
                changed |= self.convert_in_expression(right);
                changed
            }
            ExpressionKind::OptionalMember(obj, _) => self.convert_in_expression(obj),
            ExpressionKind::OptionalIndex(obj, index) => {
                let mut changed = self.convert_in_expression(obj);
                changed |= self.convert_in_expression(index);
                changed
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                let mut changed = self.convert_in_expression(obj);
                for arg in args {
                    changed |= self.convert_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::OptionalMethodCall(obj, _method_name, args, _) => {
                let mut changed = self.convert_in_expression(obj);
                for arg in args {
                    changed |= self.convert_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Member(..)
            | ExpressionKind::Index(..)
            | ExpressionKind::Identifier(..)
            | ExpressionKind::Literal(..)
            | ExpressionKind::SelfKeyword
            | ExpressionKind::SuperKeyword
            | ExpressionKind::Template(..)
            | ExpressionKind::TypeAssertion(..)
            | ExpressionKind::Array(..)
            | ExpressionKind::Object(..)
            | ExpressionKind::Function(..)
            | ExpressionKind::Parenthesized(..) => false,
        }
    }

    fn convert_method_call_to_function_call(
        &self,
        obj: &Expression,
        receiver_info: &ReceiverClassInfo,
        method_name: &typedlua_parser::ast::Ident,
        args: &[typedlua_parser::ast::expression::Argument],
        span: Span,
    ) -> Option<ExpressionKind> {
        let class_name_str = self.interner.resolve(receiver_info.class_name);
        let class_id = self.interner.get_or_intern(&class_name_str);

        let class_expr = Expression {
            kind: ExpressionKind::Member(
                Box::new(Expression {
                    kind: ExpressionKind::Identifier(class_id),
                    span,
                    annotated_type: None,
                    receiver_class: None,
                }),
                method_name.clone(),
            ),
            span,
            annotated_type: None,
            receiver_class: None,
        };

        let new_args = std::iter::once(typedlua_parser::ast::expression::Argument {
            value: obj.clone(),
            is_spread: false,
            span,
        })
        .chain(args.iter().cloned())
        .collect();

        Some(ExpressionKind::Call(Box::new(class_expr), new_args, None))
    }
}

impl StmtVisitor for MethodToFunctionConversionPass {
    fn visit_stmt(&mut self, stmt: &mut Statement) -> bool {
        self.convert_in_statement(stmt)
    }
}

impl WholeProgramPass for MethodToFunctionConversionPass {
    fn name(&self) -> &'static str {
        "method-to-function-conversion"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.convert_in_statement(stmt);
        }
        Ok(changed)
    }
}

impl Default for MethodToFunctionConversionPass {
    #[allow(clippy::arc_with_non_send_sync)]
    fn default() -> Self {
        Self {
            interner: Rc::new(StringInterner::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use typedlua_parser::ast::expression::{ExpressionKind, Literal};
    use typedlua_parser::ast::statement::{Block, Statement};
    use typedlua_parser::ast::types::{PrimitiveType, Type, TypeKind};
    use typedlua_parser::ast::Spanned;
    use typedlua_parser::span::Span;

    #[test]
    #[allow(clippy::arc_with_non_send_sync)]
    fn test_method_call_to_function_call_conversion() {
        let interner = Rc::new(StringInterner::new());
        let mut pass = MethodToFunctionConversionPass::new(interner.clone());

        let obj_id = interner.get_or_intern("myObj");
        let method_id = interner.get_or_intern("calculate");
        let class_id = interner.get_or_intern("Calculator");

        let obj_expr = Expression {
            kind: ExpressionKind::Identifier(obj_id),
            span: Span::dummy(),
            annotated_type: None,
            receiver_class: None,
        };

        let arg_expr = Expression {
            kind: ExpressionKind::Literal(Literal::Number(42.0)),
            span: Span::dummy(),
            annotated_type: None,
            receiver_class: None,
        };

        let arguments = vec![typedlua_parser::ast::expression::Argument {
            value: arg_expr,
            is_spread: false,
            span: Span::dummy(),
        }];

        let receiver_class = Some(ReceiverClassInfo {
            class_name: class_id,
            is_static: false,
        });

        let expr = Expression {
            kind: ExpressionKind::MethodCall(
                Box::new(obj_expr),
                Spanned::new(method_id, Span::dummy()),
                arguments,
                None,
            ),
            span: Span::dummy(),
            annotated_type: Some(Type::new(
                TypeKind::Primitive(PrimitiveType::Number),
                Span::dummy(),
            )),
            receiver_class,
        };

        let mut block = Block {
            statements: vec![Statement::Expression(expr)],
            span: Span::dummy(),
        };

        let result = pass.convert_in_block(&mut block);
        assert!(result, "Should have made changes");

        if let Statement::Expression(converted_expr) = &block.statements[0] {
            if let ExpressionKind::Call(callee, args, _) = &converted_expr.kind {
                if let ExpressionKind::Member(class_expr, method) = &callee.kind {
                    let class_str = interner.resolve(match &class_expr.kind {
                        ExpressionKind::Identifier(id) => *id,
                        _ => panic!("Expected identifier"),
                    });
                    assert_eq!(class_str, "Calculator", "Class name should be Calculator");

                    let method_str = interner.resolve(method.node);
                    assert_eq!(method_str, "calculate", "Method name should be calculate");

                    assert_eq!(args.len(), 2, "Should have 2 args (obj + original arg)");
                    assert!(
                        matches!(args[0].value.kind, ExpressionKind::Identifier(_)),
                        "First arg should be the object"
                    );
                } else {
                    panic!("Expected Member expression");
                }
            } else {
                panic!("Expected Call expression");
            }
        }
    }

    #[test]
    #[allow(clippy::arc_with_non_send_sync)]
    fn test_preserves_receiver_class_info() {
        let interner = Rc::new(StringInterner::new());
        let mut pass = MethodToFunctionConversionPass::new(interner.clone());

        let obj_id = interner.get_or_intern("myObj");
        let method_id = interner.get_or_intern("test");
        let class_id = interner.get_or_intern("TestClass");

        let obj_expr = Expression {
            kind: ExpressionKind::Identifier(obj_id),
            span: Span::dummy(),
            annotated_type: None,
            receiver_class: None,
        };

        let receiver_class = Some(ReceiverClassInfo {
            class_name: class_id,
            is_static: false,
        });

        let expr = Expression {
            kind: ExpressionKind::MethodCall(
                Box::new(obj_expr),
                Spanned::new(method_id, Span::dummy()),
                vec![],
                None,
            ),
            span: Span::dummy(),
            annotated_type: Some(Type::new(
                TypeKind::Primitive(PrimitiveType::Number),
                Span::dummy(),
            )),
            receiver_class,
        };

        let mut block = Block {
            statements: vec![Statement::Expression(expr)],
            span: Span::dummy(),
        };

        pass.convert_in_block(&mut block);

        if let Statement::Expression(converted_expr) = &block.statements[0] {
            assert!(
                converted_expr.receiver_class.is_none(),
                "receiver_class should be cleared after conversion"
            );
        }
    }
}
