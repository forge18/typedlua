#[cfg(test)]
mod tests {
    use super::super::super::super::symbol_table::SymbolTable;
    use super::super::super::super::type_environment::TypeEnvironment;
    use super::super::super::super::visitors::{
        AccessControl, TypeCheckVisitor, TypeInferenceVisitor, TypeInferrer,
    };
    use crate::diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler};
    use std::sync::Arc;
    use typedlua_parser::ast::expression::*;
    use typedlua_parser::ast::types::*;
    use typedlua_parser::ast::Ident;
    use typedlua_parser::span::Span;
    use typedlua_parser::string_interner::StringInterner;

    fn create_test_inferrer<'a>(
        symbol_table: &'a mut SymbolTable,
        type_env: &'a mut TypeEnvironment,
        narrowing_context: &'a mut super::super::super::super::narrowing::NarrowingContext,
        access_control: &'a AccessControl,
        interner: &'a StringInterner,
        diagnostic_handler: &'a Arc<dyn DiagnosticHandler>,
    ) -> TypeInferrer<'a> {
        TypeInferrer::new(
            symbol_table,
            type_env,
            narrowing_context,
            access_control,
            interner,
            diagnostic_handler,
        )
    }

    #[test]
    fn test_infer_literal_number() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();
        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let mut expr = Expression {
            kind: ExpressionKind::Literal(Literal::Number(42.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(typ.kind, TypeKind::Literal(Literal::Number(n)) if n == 42.0));
    }

    #[test]
    fn test_infer_literal_string() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let mut expr = Expression {
            kind: ExpressionKind::Literal(Literal::String("hello".to_string())),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(typ.kind, TypeKind::Literal(Literal::String(_))));
    }

    #[test]
    fn test_infer_literal_boolean() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let mut expr = Expression {
            kind: ExpressionKind::Literal(Literal::Boolean(true)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Literal(Literal::Boolean(true))
        ));
    }

    #[test]
    fn test_infer_binary_op_add() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let left = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(1.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let right = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(2.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Binary(BinaryOp::Add, left, right),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Number)
        ));
    }

    #[test]
    fn test_infer_binary_op_concat() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let left = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::String("hello".to_string())),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let right = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::String(" world".to_string())),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Binary(BinaryOp::Concatenate, left, right),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::String)
        ));
    }

    #[test]
    fn test_infer_unary_op_negate() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let operand = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(5.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Unary(UnaryOp::Negate, operand),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Number)
        ));
    }

    #[test]
    fn test_infer_unary_op_not() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let operand = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Boolean(true)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Unary(UnaryOp::Not, operand),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Boolean)
        ));
    }

    #[test]
    fn test_infer_array() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let elements = vec![
            ArrayElement::Expression(Expression {
                kind: ExpressionKind::Literal(Literal::Number(1.0)),
                span: Span::default(),
                annotated_type: None,
                receiver_class: None,
            }),
            ArrayElement::Expression(Expression {
                kind: ExpressionKind::Literal(Literal::Number(2.0)),
                span: Span::default(),
                annotated_type: None,
                receiver_class: None,
            }),
        ];

        let mut expr = Expression {
            kind: ExpressionKind::Array(elements),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(typ.kind, TypeKind::Array(_)));
    }

    #[test]
    fn test_infer_empty_array() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let mut expr = Expression {
            kind: ExpressionKind::Array(vec![]),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(typ.kind, TypeKind::Array(_)));
    }

    #[test]
    fn test_infer_conditional() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let cond = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Boolean(true)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let then_expr = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(1.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let else_expr = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(2.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Conditional(cond, then_expr, else_expr),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        // Conditional with different literal numbers returns a union
        assert!(matches!(typ.kind, TypeKind::Union(_)));
    }

    #[test]
    fn test_infer_binary_op_comparison() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let left = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(1.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let right = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(2.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Binary(BinaryOp::LessThan, left, right),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Boolean)
        ));
    }

    #[test]
    fn test_visitor_name() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();
        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        assert_eq!(inferrer.name(), "TypeInferrer");
    }

    // ========================================================================
    // Additional Comprehensive Type Inference Tests
    // ========================================================================

    #[test]
    fn test_infer_literal_nil() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let mut expr = Expression {
            kind: ExpressionKind::Literal(Literal::Nil),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(typ.kind, TypeKind::Literal(Literal::Nil)));
    }

    #[test]
    fn test_infer_array_expression() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        // Array of numbers: [1, 2, 3]
        let mut expr = Expression {
            kind: ExpressionKind::Array(vec![
                ArrayElement::Expression(Expression {
                    kind: ExpressionKind::Literal(Literal::Number(1.0)),
                    span: Span::default(),
                    annotated_type: None,
                    receiver_class: None,
                }),
                ArrayElement::Expression(Expression {
                    kind: ExpressionKind::Literal(Literal::Number(2.0)),
                    span: Span::default(),
                    annotated_type: None,
                    receiver_class: None,
                }),
                ArrayElement::Expression(Expression {
                    kind: ExpressionKind::Literal(Literal::Number(3.0)),
                    span: Span::default(),
                    annotated_type: None,
                    receiver_class: None,
                }),
            ]),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        // Should infer as Array<number>
        assert!(matches!(typ.kind, TypeKind::Array(_)));
    }

    #[test]
    fn test_infer_array_empty() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        // Empty array: []
        let mut expr = Expression {
            kind: ExpressionKind::Array(vec![]),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        // Should infer as Array<unknown>
        assert!(matches!(typ.kind, TypeKind::Array(_)));
    }

    #[test]
    fn test_infer_binary_op_sub() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let left = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(10.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let right = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(3.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Binary(BinaryOp::Subtract, left, right),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Number)
        ));
    }

    #[test]
    fn test_infer_binary_op_mul() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let left = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(6.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let right = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(7.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Binary(BinaryOp::Multiply, left, right),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Number)
        ));
    }

    #[test]
    fn test_infer_binary_op_div() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let left = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(10.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let right = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(2.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Binary(BinaryOp::Divide, left, right),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Number)
        ));
    }

    #[test]
    fn test_infer_binary_op_mod() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let left = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(10.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let right = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(3.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Binary(BinaryOp::Modulo, left, right),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Number)
        ));
    }

    #[test]
    fn test_infer_binary_op_eq() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let left = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(5.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let right = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(5.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Binary(BinaryOp::Equal, left, right),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Boolean)
        ));
    }

    #[test]
    fn test_infer_binary_op_and() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let left = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Boolean(true)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let right = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Boolean(false)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Binary(BinaryOp::And, left, right),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        // In Lua, 'and' returns one of its operands, so type is Unknown
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Unknown)
        ));
    }

    #[test]
    fn test_infer_binary_op_or() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let left = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Boolean(true)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });
        let right = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Boolean(false)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Binary(BinaryOp::Or, left, right),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        // In Lua, 'or' returns one of its operands, so type is Unknown
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Unknown)
        ));
    }

    #[test]
    fn test_infer_unary_op_len() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let operand = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::String("hello".to_string())),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Unary(UnaryOp::Length, operand),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        // Length operator returns number
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Number)
        ));
    }

    #[test]
    fn test_infer_parenthesized() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let inner = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(42.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let mut expr = Expression {
            kind: ExpressionKind::Parenthesized(inner),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        // Parenthesized expressions now correctly infer the type of their inner expression
        assert!(matches!(typ.kind, TypeKind::Literal(Literal::Number(n)) if (n - 42.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_infer_type_assertion() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let inner = Box::new(Expression {
            kind: ExpressionKind::Literal(Literal::Number(42.0)),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        });

        let assert_type = Type {
            kind: TypeKind::Primitive(PrimitiveType::Number),
            span: Span::default(),
        };

        let mut expr = Expression {
            kind: ExpressionKind::TypeAssertion(inner, assert_type),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        // Type assertions currently return Unknown (not yet fully implemented)
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Unknown)
        ));
    }

    #[test]
    fn test_infer_object_expression() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        // Object literal: { x: 1, y: 2 }
        let name_id = interner.intern("x");
        let y_id = interner.intern("y");

        let mut expr = Expression {
            kind: ExpressionKind::Object(vec![
                ObjectProperty::Property {
                    key: Ident::new(name_id, Span::default()),
                    value: Box::new(Expression {
                        kind: ExpressionKind::Literal(Literal::Number(1.0)),
                        span: Span::default(),
                        annotated_type: None,
                        receiver_class: None,
                    }),
                    span: Span::default(),
                },
                ObjectProperty::Property {
                    key: Ident::new(y_id, Span::default()),
                    value: Box::new(Expression {
                        kind: ExpressionKind::Literal(Literal::Number(2.0)),
                        span: Span::default(),
                        annotated_type: None,
                        receiver_class: None,
                    }),
                    span: Span::default(),
                },
            ]),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        // Should infer as object type
        assert!(matches!(typ.kind, TypeKind::Object(_)));
    }

    #[test]
    fn test_infer_identifier_not_found() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let x_id = interner.intern("x");
        let mut expr = Expression {
            kind: ExpressionKind::Identifier(x_id),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        // Should fail because x is not defined
        assert!(result.is_err());
    }

    #[test]
    fn test_infer_identifier_with_type() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();
        let diagnostic_handler: Arc<dyn DiagnosticHandler> =
            Arc::new(CollectingDiagnosticHandler::new());

        // Register a variable with a type
        let x_id = interner.intern("x");
        let x_type = Type {
            kind: TypeKind::Primitive(PrimitiveType::Number),
            span: Span::default(),
        };

        symbol_table
            .declare(super::super::super::super::symbol_table::Symbol::new(
                "x".to_string(),
                super::super::super::super::symbol_table::SymbolKind::Variable,
                x_type.clone(),
                Span::default(),
            ))
            .unwrap();

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
            &diagnostic_handler,
        );

        let mut expr = Expression {
            kind: ExpressionKind::Identifier(x_id),
            span: Span::default(),
            annotated_type: None,
            receiver_class: None,
        };

        let result = inferrer.infer_expression(&mut expr);
        assert!(result.is_ok());
        let typ = result.unwrap();
        assert!(matches!(
            typ.kind,
            TypeKind::Primitive(PrimitiveType::Number)
        ));
    }
}
