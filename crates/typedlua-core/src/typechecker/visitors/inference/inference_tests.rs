#[cfg(test)]
mod tests {
    use super::super::super::super::symbol_table::SymbolTable;
    use super::super::super::super::type_environment::TypeEnvironment;
    use super::super::super::super::visitors::{
        AccessControl, TypeCheckVisitor, TypeInferenceVisitor, TypeInferrer,
    };
    use typedlua_parser::ast::expression::*;
    use typedlua_parser::ast::types::*;
    use typedlua_parser::span::Span;
    use typedlua_parser::string_interner::StringInterner;

    fn create_test_inferrer<'a>(
        symbol_table: &'a mut SymbolTable,
        type_env: &'a mut TypeEnvironment,
        narrowing_context: &'a mut super::super::super::super::narrowing::NarrowingContext,
        access_control: &'a AccessControl,
        interner: &'a StringInterner,
    ) -> TypeInferrer<'a> {
        TypeInferrer::new(
            symbol_table,
            type_env,
            narrowing_context,
            access_control,
            interner,
        )
    }

    #[test]
    fn test_infer_literal_number() {
        let interner = StringInterner::new();
        let mut symbol_table = SymbolTable::new();
        let mut type_env = TypeEnvironment::new();
        let mut narrowing_context = super::super::super::super::narrowing::NarrowingContext::new();
        let access_control = AccessControl::new();

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
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

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
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

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
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

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
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

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
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

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
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

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
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

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
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

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
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

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
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

        let mut inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
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

        let inferrer = create_test_inferrer(
            &mut symbol_table,
            &mut type_env,
            &mut narrowing_context,
            &access_control,
            &interner,
        );

        assert_eq!(inferrer.name(), "TypeInferrer");
    }
}
