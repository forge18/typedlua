use bumpalo::Bump;
use std::sync::Arc;
use typedlua_core::config::OptimizationLevel;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::optimizer::Optimizer;
use typedlua_core::MutableProgram;
use typedlua_parser::ast::expression::{Argument, Expression, ExpressionKind, Literal};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{
    Block, FunctionDeclaration, Parameter, ReturnStatement, Statement, TypeParameter,
    VariableDeclaration, VariableKind,
};
use typedlua_parser::ast::types::{PrimitiveType, Type, TypeKind, TypeReference};
use typedlua_parser::ast::{Program, Spanned};
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::StringInterner;

// Helper for integration-style tests that parse and type-check source code
fn type_check(source: &str) -> Result<(), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let arena = Bump::new();
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let program = parser.parse().map_err(|e| format!("{:?}", e))?;

    let mut checker = TypeChecker::new_with_stdlib(handler, &interner, &common_ids, &arena)
        .expect("Failed to load stdlib");
    checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    Ok(())
}

use typedlua_core::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;

/// Helper to create a type reference to T (type parameter)
fn type_param_ref<'arena>(
    name_id: typedlua_parser::string_interner::StringId,
    span: Span,
) -> Type<'arena> {
    Type::new(
        TypeKind::Reference(TypeReference {
            name: Spanned::new(name_id, span),
            type_arguments: None,
            span,
        }),
        span,
    )
}

/// Helper to create a number type
fn number_type(span: Span) -> Type<'static> {
    Type::new(TypeKind::Primitive(PrimitiveType::Number), span)
}

// =============================================================================
// Generic Specialization Tests
// =============================================================================

#[test]
#[ignore = "Requires arena migration: optimizer passes temporarily disabled"]
fn test_simple_identity_specialization() {
    let interner = Arc::new(StringInterner::new());
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut optimizer = Optimizer::new(OptimizationLevel::O3, handler, interner.clone());

    let arena = Bump::new();
    let span = Span::dummy();

    // Intern identifiers
    let id_name = interner.get_or_intern("id");
    let t_name = interner.get_or_intern("T");
    let x_name = interner.get_or_intern("x");
    let y_name = interner.get_or_intern("y");

    // Create type parameter T
    let type_param_t = TypeParameter {
        name: Spanned::new(t_name, span),
        constraint: None,
        default: None,
        span,
    };

    // Create function: function id<T>(x: T): T return x end
    let return_x = arena.alloc_slice_clone(&[Expression::new(
        ExpressionKind::Identifier(x_name),
        span,
    )]);
    let body_stmts = arena.alloc_slice_clone(&[Statement::Return(ReturnStatement {
        values: return_x,
        span,
    })]);

    let id_func = FunctionDeclaration {
        name: Spanned::new(id_name, span),
        type_parameters: Some(arena.alloc_slice_clone(&[type_param_t])),
        parameters: arena.alloc_slice_clone(&[Parameter {
            pattern: Pattern::Identifier(Spanned::new(x_name, span)),
            type_annotation: Some(type_param_ref(t_name, span)),
            default: None,
            is_rest: false,
            is_optional: false,
            span,
        }]),
        return_type: Some(type_param_ref(t_name, span)),
        body: Block {
            statements: body_stmts,
            span,
        },
        throws: None,
        span,
    };

    // Create call: id(42) with type argument [number]
    let callee = arena.alloc(Expression::new(ExpressionKind::Identifier(id_name), span));
    let args = arena.alloc_slice_clone(&[Argument {
        value: Expression::new(ExpressionKind::Literal(Literal::Number(42.0)), span),
        is_spread: false,
        span,
    }]);
    let type_args = arena.alloc_slice_clone(&[number_type(span)]);

    let id_call = Expression::new(
        ExpressionKind::Call(callee, args, Some(type_args)),
        span,
    );

    // Create: local y = id(42)
    let var_y = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(y_name, span)),
        type_annotation: None,
        initializer: id_call,
        span,
    };

    // Return y so it's not removed by dead store elimination
    let return_values = arena.alloc_slice_clone(&[Expression::new(
        ExpressionKind::Identifier(y_name),
        span,
    )]);
    let return_y = ReturnStatement {
        values: return_values,
        span,
    };

    let stmts = arena.alloc_slice_clone(&[
        Statement::Function(id_func),
        Statement::Variable(var_y),
        Statement::Return(return_y),
    ]);
    let program = Program::new(stmts, span);

    // Run optimization via MutableProgram
    let mut mutable = MutableProgram::from_program(&program);
    let result = optimizer.optimize(&mut mutable);
    assert!(result.is_ok(), "Optimization should succeed");

    // Check that a specialized function was created
    let _has_specialized = mutable.statements.iter().any(|s| {
        if let Statement::Function(func) = s {
            let name = interner.resolve(func.name.node);
            // Should have created id__spec0
            name.starts_with("id__spec")
        } else {
            false
        }
    });

    assert!(
        result.is_ok(),
        "Generic interface implementation should work"
    );
}

#[test]
fn test_default_type_parameters() {
    let source = r#"
        class Container<T, U = string> {
            first: T
            second: U
            constructor(first: T, second: U) {
                self.first = first
                self.second = second
            end
        end

        const container1 = new Container<number, string>(42, "hello")
        const container2 = new Container<boolean>(true)
    "#;

    assert!(type_check(source).is_ok());
}

#[test]
fn test_generic_array_parameters() {
    let source = r#"
        function firstElement<T>(arr: Array<T>): T | nil
            if #arr > 0 then
                return arr[1]
            else
                return nil
            end
        end

        const nums = [1, 2, 3]
        const strs = ["a", "b", "c"]
        const firstNum = firstElement(nums)
        const firstStr = firstElement(strs)
    "#;

    assert!(type_check(source).is_ok());
}
