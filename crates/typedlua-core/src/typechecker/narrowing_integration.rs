/// Integration module for type narrowing with the type checker
/// This provides the scaffolding for how narrowing will be used during type checking

use super::narrowing::{NarrowingContext, narrow_type_from_condition};
use crate::ast::expression::Expression;
use crate::ast::types::Type;
use rustc_hash::FxHashMap;

/// Demonstration of how type narrowing integrates with if statement checking
///
/// This is a template/example showing how the type checker would use narrowing
/// when checking if statements. Full integration requires statement type checking
/// which is not yet implemented.
pub struct IfStatementNarrowingExample;

impl IfStatementNarrowingExample {
    /// Example: How to narrow types when type checking an if statement
    ///
    /// ```text
    /// // Given code:
    /// local x: string | nil = getValue()
    /// if x != nil then
    ///     -- In this branch, x is narrowed to string
    ///     print(x.upper(x))  // Valid: x is string here
    /// else
    ///     -- In this branch, x is nil
    ///     print("x is nil")
    /// end
    /// ```
    #[allow(dead_code)]
    pub fn check_if_statement_with_narrowing(
        condition: &Expression,
        base_context: &NarrowingContext,
        variable_types: &FxHashMap<String, Type>,
    ) -> (NarrowingContext, NarrowingContext) {
        // Step 1: Analyze the condition to produce narrowed contexts
        let (then_context, else_context) = narrow_type_from_condition(
            condition,
            base_context,
            variable_types,
        );

        // Step 2: When type checking the then-branch, use then_context
        // to get narrowed types for variables
        //
        // Example:
        // for var_name in then_branch_variables {
        //     let var_type = then_context.get_narrowed_type(var_name)
        //         .unwrap_or_else(|| variable_types.get(var_name));
        //     // Use var_type when checking expressions in then branch
        // }

        // Step 3: When type checking the else-branch, use else_context
        //
        // Example:
        // for var_name in else_branch_variables {
        //     let var_type = else_context.get_narrowed_type(var_name)
        //         .unwrap_or_else(|| variable_types.get(var_name));
        //     // Use var_type when checking expressions in else branch
        // }

        // Step 4: After both branches, merge the contexts for the continuation
        // let merged_context = NarrowingContext::merge(&then_context, &else_context);

        (then_context, else_context)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::expression::{BinaryOp, ExpressionKind, Literal};
    use crate::ast::types::{PrimitiveType, TypeKind};
    use crate::span::Span;

    fn make_span() -> Span {
        Span::new(0, 0, 0, 0)
    }

    #[test]
    fn test_if_statement_narrowing_example() {
        // Setup: x: string | nil
        let mut variable_types = FxHashMap::default();
        variable_types.insert(
            "x".to_string(),
            Type::new(
                TypeKind::Union(vec![
                    Type::new(TypeKind::Primitive(PrimitiveType::String), make_span()),
                    Type::new(TypeKind::Primitive(PrimitiveType::Nil), make_span()),
                ]),
                make_span(),
            ),
        );

        // Condition: x != nil
        let condition = Expression::new(
            ExpressionKind::Binary(
                BinaryOp::NotEqual,
                Box::new(Expression::new(
                    ExpressionKind::Identifier("x".to_string()),
                    make_span(),
                )),
                Box::new(Expression::new(
                    ExpressionKind::Literal(Literal::Nil),
                    make_span(),
                )),
            ),
            make_span(),
        );

        let base_context = NarrowingContext::new();

        // Apply narrowing
        let (then_ctx, else_ctx) = IfStatementNarrowingExample::check_if_statement_with_narrowing(
            &condition,
            &base_context,
            &variable_types,
        );

        // In then branch: x should be narrowed to string
        let then_type = then_ctx.get_narrowed_type("x").unwrap();
        assert!(matches!(then_type.kind, TypeKind::Primitive(PrimitiveType::String)));

        // In else branch: x should be nil
        let else_type = else_ctx.get_narrowed_type("x").unwrap();
        assert!(matches!(else_type.kind, TypeKind::Primitive(PrimitiveType::Nil)));
    }

    #[test]
    fn test_typeof_narrowing_example() {
        // Setup: x: string | number
        let mut variable_types = FxHashMap::default();
        variable_types.insert(
            "x".to_string(),
            Type::new(
                TypeKind::Union(vec![
                    Type::new(TypeKind::Primitive(PrimitiveType::String), make_span()),
                    Type::new(TypeKind::Primitive(PrimitiveType::Number), make_span()),
                ]),
                make_span(),
            ),
        );

        // Condition: typeof(x) == "string"
        let condition = Expression::new(
            ExpressionKind::Binary(
                BinaryOp::Equal,
                Box::new(Expression::new(
                    ExpressionKind::Call(
                        Box::new(Expression::new(
                            ExpressionKind::Identifier("typeof".to_string()),
                            make_span(),
                        )),
                        vec![crate::ast::expression::Argument {
                            value: Expression::new(
                                ExpressionKind::Identifier("x".to_string()),
                                make_span(),
                            ),
                            is_spread: false,
                            span: make_span(),
                        }],
                    ),
                    make_span(),
                )),
                Box::new(Expression::new(
                    ExpressionKind::Literal(Literal::String("string".to_string())),
                    make_span(),
                )),
            ),
            make_span(),
        );

        let base_context = NarrowingContext::new();

        // Apply narrowing
        let (then_ctx, else_ctx) = IfStatementNarrowingExample::check_if_statement_with_narrowing(
            &condition,
            &base_context,
            &variable_types,
        );

        // In then branch: x should be string
        let then_type = then_ctx.get_narrowed_type("x").unwrap();
        assert!(matches!(then_type.kind, TypeKind::Primitive(PrimitiveType::String)));

        // In else branch: x should be number
        let else_type = else_ctx.get_narrowed_type("x").unwrap();
        assert!(matches!(else_type.kind, TypeKind::Primitive(PrimitiveType::Number)));
    }

    #[test]
    fn test_type_guard_narrowing() {
        // Setup: x: string | number | nil
        let mut variable_types = FxHashMap::default();
        variable_types.insert(
            "x".to_string(),
            Type::new(
                TypeKind::Union(vec![
                    Type::new(TypeKind::Primitive(PrimitiveType::String), make_span()),
                    Type::new(TypeKind::Primitive(PrimitiveType::Number), make_span()),
                    Type::new(TypeKind::Primitive(PrimitiveType::Nil), make_span()),
                ]),
                make_span(),
            ),
        );

        // Condition: isString(x)
        let condition = Expression::new(
            ExpressionKind::Call(
                Box::new(Expression::new(
                    ExpressionKind::Identifier("isString".to_string()),
                    make_span(),
                )),
                vec![crate::ast::expression::Argument {
                    value: Expression::new(
                        ExpressionKind::Identifier("x".to_string()),
                        make_span(),
                    ),
                    is_spread: false,
                    span: make_span(),
                }],
            ),
            make_span(),
        );

        let base_context = NarrowingContext::new();

        // Apply narrowing
        let (then_ctx, else_ctx) = IfStatementNarrowingExample::check_if_statement_with_narrowing(
            &condition,
            &base_context,
            &variable_types,
        );

        // In then branch: x should be narrowed to string
        let then_type = then_ctx.get_narrowed_type("x").unwrap();
        assert!(matches!(then_type.kind, TypeKind::Primitive(PrimitiveType::String)));

        // In else branch: x should be number | nil
        let else_type = else_ctx.get_narrowed_type("x").unwrap();
        if let TypeKind::Union(types) = &else_type.kind {
            assert_eq!(types.len(), 2);
            assert!(types.iter().any(|t| matches!(t.kind, TypeKind::Primitive(PrimitiveType::Number))));
            assert!(types.iter().any(|t| matches!(t.kind, TypeKind::Primitive(PrimitiveType::Nil))));
        } else {
            panic!("Expected union type for else branch");
        }
    }

    #[test]
    fn test_instanceof_narrowing() {
        use crate::ast::types::TypeReference;
        use crate::ast::Ident;

        // Setup: pet: Animal | Dog (union of two class types)
        let mut variable_types = FxHashMap::default();
        variable_types.insert(
            "pet".to_string(),
            Type::new(
                TypeKind::Union(vec![
                    Type::new(
                        TypeKind::Reference(TypeReference {
                            name: Ident::new("Animal".to_string(), make_span()),
                            type_arguments: None,
                            span: make_span(),
                        }),
                        make_span(),
                    ),
                    Type::new(
                        TypeKind::Reference(TypeReference {
                            name: Ident::new("Dog".to_string(), make_span()),
                            type_arguments: None,
                            span: make_span(),
                        }),
                        make_span(),
                    ),
                ]),
                make_span(),
            ),
        );

        // Condition: pet instanceof Dog
        let condition = Expression::new(
            ExpressionKind::Binary(
                BinaryOp::Instanceof,
                Box::new(Expression::new(
                    ExpressionKind::Identifier("pet".to_string()),
                    make_span(),
                )),
                Box::new(Expression::new(
                    ExpressionKind::Identifier("Dog".to_string()),
                    make_span(),
                )),
            ),
            make_span(),
        );

        let base_context = NarrowingContext::new();

        // Apply narrowing
        let (then_ctx, _else_ctx) = IfStatementNarrowingExample::check_if_statement_with_narrowing(
            &condition,
            &base_context,
            &variable_types,
        );

        // In then branch: pet should be narrowed to Dog
        let then_type = then_ctx.get_narrowed_type("pet").unwrap();
        if let TypeKind::Reference(type_ref) = &then_type.kind {
            assert_eq!(type_ref.name.node, "Dog");
        } else {
            panic!("Expected reference type for then branch");
        }
    }
}
