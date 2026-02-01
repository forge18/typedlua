use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::config::OptimizationLevel;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::optimizer::Optimizer;
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
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let program = parser.parse().map_err(|e| format!("{:?}", e))?;

    let mut checker = TypeChecker::new(handler, &interner, &common_ids);
    checker
        .check_program(&mut program.clone())
        .map_err(|e| e.message)?;

    Ok(())
}

use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;

fn create_test_interner() -> Rc<StringInterner> {
    Rc::new(StringInterner::new())
}

fn create_test_handler() -> Arc<CollectingDiagnosticHandler> {
    Arc::new(CollectingDiagnosticHandler::new())
}

fn create_test_optimizer(
    level: OptimizationLevel,
    interner: Rc<StringInterner>,
    handler: Arc<CollectingDiagnosticHandler>,
) -> Optimizer {
    Optimizer::new(level, handler, interner)
}

/// Helper to create a type reference to T (type parameter)
fn type_param_ref(name_id: typedlua_parser::string_interner::StringId, span: Span) -> Type {
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
fn number_type(span: Span) -> Type {
    Type::new(TypeKind::Primitive(PrimitiveType::Number), span)
}

/// Helper to create a string type
fn string_type(span: Span) -> Type {
    Type::new(TypeKind::Primitive(PrimitiveType::String), span)
}

// =============================================================================
// Generic Specialization Tests
// =============================================================================

#[test]
fn test_simple_identity_specialization() {
    // Create: function id<T>(x: T): T return x end
    // Then call: local y = id(42)
    let interner = create_test_interner();
    let handler = create_test_handler();
    let mut optimizer = create_test_optimizer(OptimizationLevel::O3, interner.clone(), handler);

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
    let id_func = FunctionDeclaration {
        name: Spanned::new(id_name, span),
        type_parameters: Some(vec![type_param_t]),
        parameters: vec![Parameter {
            pattern: Pattern::Identifier(Spanned::new(x_name, span)),
            type_annotation: Some(type_param_ref(t_name, span)),
            default: None,
            is_rest: false,
            is_optional: false,
            span,
        }],
        return_type: Some(type_param_ref(t_name, span)),
        body: Block {
            statements: vec![Statement::Return(ReturnStatement {
                values: vec![Expression::new(ExpressionKind::Identifier(x_name), span)],
                span,
            })],
            span,
        },
        throws: None,
        span,
    };

    // Create call: id(42) with type argument [number]
    let id_call = Expression::new(
        ExpressionKind::Call(
            Box::new(Expression::new(ExpressionKind::Identifier(id_name), span)),
            vec![Argument {
                value: Expression::new(ExpressionKind::Literal(Literal::Number(42.0)), span),
                is_spread: false,
                span,
            }],
            Some(vec![number_type(span)]), // Type argument: number
        ),
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
    let return_y = ReturnStatement {
        values: vec![Expression::new(ExpressionKind::Identifier(y_name), span)],
        span,
    };

    let mut program = Program::new(
        vec![
            Statement::Function(id_func),
            Statement::Variable(var_y),
            Statement::Return(return_y),
        ],
        span,
    );

    // Run optimization
    let result = optimizer.optimize(&mut program);
    assert!(result.is_ok(), "Optimization should succeed");

    // Check that a specialized function was created
    let has_specialized = program.statements.iter().any(|s| {
        if let Statement::Function(func) = s {
            let name = interner.resolve(func.name.node);
            // Should have created id__spec0
            name.starts_with("id__spec")
        } else {
            false
        }
    });

    assert!(
        has_specialized,
        "Should create specialized version of generic function"
    );

    // Check that the original generic function still exists
    let has_original = program.statements.iter().any(|s| {
        if let Statement::Function(func) = s {
            interner.resolve(func.name.node) == "id"
        } else {
            false
        }
    });

    assert!(has_original, "Original generic function should remain");
}

#[test]
fn test_multiple_type_params() {
    // Create: function pair<A, B>(a: A, b: B): A return a end
    let interner = create_test_interner();
    let handler = create_test_handler();
    let mut optimizer = create_test_optimizer(OptimizationLevel::O3, interner.clone(), handler);

    let span = Span::dummy();

    let pair_name = interner.get_or_intern("pair");
    let a_type_name = interner.get_or_intern("A");
    let b_type_name = interner.get_or_intern("B");
    let a_var = interner.get_or_intern("a");
    let b_var = interner.get_or_intern("b");
    let result_var = interner.get_or_intern("result");

    // Create type parameters A and B
    let type_param_a = TypeParameter {
        name: Spanned::new(a_type_name, span),
        constraint: None,
        default: None,
        span,
    };
    let type_param_b = TypeParameter {
        name: Spanned::new(b_type_name, span),
        constraint: None,
        default: None,
        span,
    };

    // Create function: function pair<A, B>(a: A, b: B): A return a end
    let pair_func = FunctionDeclaration {
        name: Spanned::new(pair_name, span),
        type_parameters: Some(vec![type_param_a, type_param_b]),
        parameters: vec![
            Parameter {
                pattern: Pattern::Identifier(Spanned::new(a_var, span)),
                type_annotation: Some(type_param_ref(a_type_name, span)),
                default: None,
                is_rest: false,
                is_optional: false,
                span,
            },
            Parameter {
                pattern: Pattern::Identifier(Spanned::new(b_var, span)),
                type_annotation: Some(type_param_ref(b_type_name, span)),
                default: None,
                is_rest: false,
                is_optional: false,
                span,
            },
        ],
        return_type: Some(type_param_ref(a_type_name, span)),
        body: Block {
            statements: vec![Statement::Return(ReturnStatement {
                values: vec![Expression::new(ExpressionKind::Identifier(a_var), span)],
                span,
            })],
            span,
        },
        throws: None,
        span,
    };

    // Create call: pair(1, "hello") with type arguments [number, string]
    let pair_call = Expression::new(
        ExpressionKind::Call(
            Box::new(Expression::new(ExpressionKind::Identifier(pair_name), span)),
            vec![
                Argument {
                    value: Expression::new(ExpressionKind::Literal(Literal::Number(1.0)), span),
                    is_spread: false,
                    span,
                },
                Argument {
                    value: Expression::new(
                        ExpressionKind::Literal(Literal::String("hello".to_string())),
                        span,
                    ),
                    is_spread: false,
                    span,
                },
            ],
            Some(vec![number_type(span), string_type(span)]),
        ),
        span,
    );

    let var_result = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(result_var, span)),
        type_annotation: None,
        initializer: pair_call,
        span,
    };

    // Return result so it's not removed by dead store elimination
    let return_result = ReturnStatement {
        values: vec![Expression::new(
            ExpressionKind::Identifier(result_var),
            span,
        )],
        span,
    };

    let mut program = Program::new(
        vec![
            Statement::Function(pair_func),
            Statement::Variable(var_result),
            Statement::Return(return_result),
        ],
        span,
    );

    let result = optimizer.optimize(&mut program);
    assert!(result.is_ok(), "Optimization should succeed");

    // Check for specialized function
    let specialized_count = program
        .statements
        .iter()
        .filter(|s| {
            if let Statement::Function(func) = s {
                interner.resolve(func.name.node).starts_with("pair__spec")
            } else {
                false
            }
        })
        .count();

    assert_eq!(
        specialized_count, 1,
        "Should create exactly one specialized version"
    );
}

#[test]
fn test_specialization_caching() {
    // Test that multiple calls with same type args reuse the same specialization
    let interner = create_test_interner();
    let handler = create_test_handler();
    let mut optimizer = create_test_optimizer(OptimizationLevel::O3, interner.clone(), handler);

    let span = Span::dummy();

    let id_name = interner.get_or_intern("id");
    let t_name = interner.get_or_intern("T");
    let x_name = interner.get_or_intern("x");
    let a_name = interner.get_or_intern("a");
    let b_name = interner.get_or_intern("b");

    let type_param_t = TypeParameter {
        name: Spanned::new(t_name, span),
        constraint: None,
        default: None,
        span,
    };

    let id_func = FunctionDeclaration {
        name: Spanned::new(id_name, span),
        type_parameters: Some(vec![type_param_t]),
        parameters: vec![Parameter {
            pattern: Pattern::Identifier(Spanned::new(x_name, span)),
            type_annotation: Some(type_param_ref(t_name, span)),
            default: None,
            is_rest: false,
            is_optional: false,
            span,
        }],
        return_type: Some(type_param_ref(t_name, span)),
        body: Block {
            statements: vec![Statement::Return(ReturnStatement {
                values: vec![Expression::new(ExpressionKind::Identifier(x_name), span)],
                span,
            })],
            span,
        },
        throws: None,
        span,
    };

    // Create two calls with the same type argument (number)
    let call1 = Expression::new(
        ExpressionKind::Call(
            Box::new(Expression::new(ExpressionKind::Identifier(id_name), span)),
            vec![Argument {
                value: Expression::new(ExpressionKind::Literal(Literal::Number(1.0)), span),
                is_spread: false,
                span,
            }],
            Some(vec![number_type(span)]),
        ),
        span,
    );

    let call2 = Expression::new(
        ExpressionKind::Call(
            Box::new(Expression::new(ExpressionKind::Identifier(id_name), span)),
            vec![Argument {
                value: Expression::new(ExpressionKind::Literal(Literal::Number(2.0)), span),
                is_spread: false,
                span,
            }],
            Some(vec![number_type(span)]),
        ),
        span,
    );

    let var_a = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(a_name, span)),
        type_annotation: None,
        initializer: call1,
        span,
    };

    let var_b = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(b_name, span)),
        type_annotation: None,
        initializer: call2,
        span,
    };

    // Return both a and b so they're not removed by dead store elimination
    let return_ab = ReturnStatement {
        values: vec![
            Expression::new(ExpressionKind::Identifier(a_name), span),
            Expression::new(ExpressionKind::Identifier(b_name), span),
        ],
        span,
    };

    let mut program = Program::new(
        vec![
            Statement::Function(id_func),
            Statement::Variable(var_a),
            Statement::Variable(var_b),
            Statement::Return(return_ab),
        ],
        span,
    );

    let result = optimizer.optimize(&mut program);
    assert!(result.is_ok(), "Optimization should succeed");

    // Count specialized functions - should only be 1 since both use number
    let specialized_count = program
        .statements
        .iter()
        .filter(|s| {
            if let Statement::Function(func) = s {
                interner.resolve(func.name.node).starts_with("id__spec")
            } else {
                false
            }
        })
        .count();

    assert_eq!(
        specialized_count, 1,
        "Should reuse same specialization for identical type args"
    );
}

#[test]
fn test_no_specialization_without_type_args() {
    // Calls without explicit type arguments should not be specialized
    let interner = create_test_interner();
    let handler = create_test_handler();
    let mut optimizer = create_test_optimizer(OptimizationLevel::O3, interner.clone(), handler);

    let span = Span::dummy();

    let id_name = interner.get_or_intern("id");
    let t_name = interner.get_or_intern("T");
    let x_name = interner.get_or_intern("x");
    let y_name = interner.get_or_intern("y");

    let type_param_t = TypeParameter {
        name: Spanned::new(t_name, span),
        constraint: None,
        default: None,
        span,
    };

    let id_func = FunctionDeclaration {
        name: Spanned::new(id_name, span),
        type_parameters: Some(vec![type_param_t]),
        parameters: vec![Parameter {
            pattern: Pattern::Identifier(Spanned::new(x_name, span)),
            type_annotation: Some(type_param_ref(t_name, span)),
            default: None,
            is_rest: false,
            is_optional: false,
            span,
        }],
        return_type: Some(type_param_ref(t_name, span)),
        body: Block {
            statements: vec![Statement::Return(ReturnStatement {
                values: vec![Expression::new(ExpressionKind::Identifier(x_name), span)],
                span,
            })],
            span,
        },
        throws: None,
        span,
    };

    // Call without type arguments (None)
    let id_call = Expression::new(
        ExpressionKind::Call(
            Box::new(Expression::new(ExpressionKind::Identifier(id_name), span)),
            vec![Argument {
                value: Expression::new(ExpressionKind::Literal(Literal::Number(42.0)), span),
                is_spread: false,
                span,
            }],
            None, // No type arguments
        ),
        span,
    );

    let var_y = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(y_name, span)),
        type_annotation: None,
        initializer: id_call,
        span,
    };

    // Return y so it's not removed by dead store elimination
    let return_y = ReturnStatement {
        values: vec![Expression::new(ExpressionKind::Identifier(y_name), span)],
        span,
    };

    let mut program = Program::new(
        vec![
            Statement::Function(id_func),
            Statement::Variable(var_y),
            Statement::Return(return_y),
        ],
        span,
    );

    let result = optimizer.optimize(&mut program);
    assert!(result.is_ok(), "Optimization should succeed");

    // Should not create any specialized versions
    let specialized_count = program
        .statements
        .iter()
        .filter(|s| {
            if let Statement::Function(func) = s {
                interner.resolve(func.name.node).starts_with("id__spec")
            } else {
                false
            }
        })
        .count();

    assert_eq!(
        specialized_count, 0,
        "Should not specialize without type arguments"
    );
}

#[test]
fn test_o3_only() {
    // Generic specialization should only run at O3, not at O2
    let interner = create_test_interner();
    let handler = create_test_handler();
    let mut optimizer = create_test_optimizer(OptimizationLevel::O2, interner.clone(), handler);

    let span = Span::dummy();

    let id_name = interner.get_or_intern("id");
    let t_name = interner.get_or_intern("T");
    let x_name = interner.get_or_intern("x");
    let y_name = interner.get_or_intern("y");

    let type_param_t = TypeParameter {
        name: Spanned::new(t_name, span),
        constraint: None,
        default: None,
        span,
    };

    let id_func = FunctionDeclaration {
        name: Spanned::new(id_name, span),
        type_parameters: Some(vec![type_param_t]),
        parameters: vec![Parameter {
            pattern: Pattern::Identifier(Spanned::new(x_name, span)),
            type_annotation: Some(type_param_ref(t_name, span)),
            default: None,
            is_rest: false,
            is_optional: false,
            span,
        }],
        return_type: Some(type_param_ref(t_name, span)),
        body: Block {
            statements: vec![Statement::Return(ReturnStatement {
                values: vec![Expression::new(ExpressionKind::Identifier(x_name), span)],
                span,
            })],
            span,
        },
        throws: None,
        span,
    };

    // Call with type arguments
    let id_call = Expression::new(
        ExpressionKind::Call(
            Box::new(Expression::new(ExpressionKind::Identifier(id_name), span)),
            vec![Argument {
                value: Expression::new(ExpressionKind::Literal(Literal::Number(42.0)), span),
                is_spread: false,
                span,
            }],
            Some(vec![number_type(span)]),
        ),
        span,
    );

    let var_y = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(y_name, span)),
        type_annotation: None,
        initializer: id_call,
        span,
    };

    // Return y so it's not removed by dead store elimination
    let return_y = ReturnStatement {
        values: vec![Expression::new(ExpressionKind::Identifier(y_name), span)],
        span,
    };

    let mut program = Program::new(
        vec![
            Statement::Function(id_func),
            Statement::Variable(var_y),
            Statement::Return(return_y),
        ],
        span,
    );

    let result = optimizer.optimize(&mut program);
    assert!(result.is_ok(), "Optimization should succeed");

    // At O2, should NOT create any specialized versions
    let specialized_count = program
        .statements
        .iter()
        .filter(|s| {
            if let Statement::Function(func) = s {
                interner.resolve(func.name.node).starts_with("id__spec")
            } else {
                false
            }
        })
        .count();

    assert_eq!(specialized_count, 0, "Should not specialize at O2 level");
}

#[test]
fn test_different_type_args_create_different_specializations() {
    // Calls with different type arguments should create different specializations
    let interner = create_test_interner();
    let handler = create_test_handler();
    let mut optimizer = create_test_optimizer(OptimizationLevel::O3, interner.clone(), handler);

    let span = Span::dummy();

    let id_name = interner.get_or_intern("id");
    let t_name = interner.get_or_intern("T");
    let x_name = interner.get_or_intern("x");
    let a_name = interner.get_or_intern("a");
    let b_name = interner.get_or_intern("b");

    let type_param_t = TypeParameter {
        name: Spanned::new(t_name, span),
        constraint: None,
        default: None,
        span,
    };

    let id_func = FunctionDeclaration {
        name: Spanned::new(id_name, span),
        type_parameters: Some(vec![type_param_t]),
        parameters: vec![Parameter {
            pattern: Pattern::Identifier(Spanned::new(x_name, span)),
            type_annotation: Some(type_param_ref(t_name, span)),
            default: None,
            is_rest: false,
            is_optional: false,
            span,
        }],
        return_type: Some(type_param_ref(t_name, span)),
        body: Block {
            statements: vec![Statement::Return(ReturnStatement {
                values: vec![Expression::new(ExpressionKind::Identifier(x_name), span)],
                span,
            })],
            span,
        },
        throws: None,
        span,
    };

    // Call with number type
    let call_number = Expression::new(
        ExpressionKind::Call(
            Box::new(Expression::new(ExpressionKind::Identifier(id_name), span)),
            vec![Argument {
                value: Expression::new(ExpressionKind::Literal(Literal::Number(1.0)), span),
                is_spread: false,
                span,
            }],
            Some(vec![number_type(span)]),
        ),
        span,
    );

    // Call with string type
    let call_string = Expression::new(
        ExpressionKind::Call(
            Box::new(Expression::new(ExpressionKind::Identifier(id_name), span)),
            vec![Argument {
                value: Expression::new(
                    ExpressionKind::Literal(Literal::String("test".to_string())),
                    span,
                ),
                is_spread: false,
                span,
            }],
            Some(vec![string_type(span)]),
        ),
        span,
    );

    let var_a = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(a_name, span)),
        type_annotation: None,
        initializer: call_number,
        span,
    };

    let var_b = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(b_name, span)),
        type_annotation: None,
        initializer: call_string,
        span,
    };

    // Return both a and b so they're not removed by dead store elimination
    let return_ab = ReturnStatement {
        values: vec![
            Expression::new(ExpressionKind::Identifier(a_name), span),
            Expression::new(ExpressionKind::Identifier(b_name), span),
        ],
        span,
    };

    let mut program = Program::new(
        vec![
            Statement::Function(id_func),
            Statement::Variable(var_a),
            Statement::Variable(var_b),
            Statement::Return(return_ab),
        ],
        span,
    );

    let result = optimizer.optimize(&mut program);
    assert!(result.is_ok(), "Optimization should succeed");

    // Should create 2 different specializations (one for number, one for string)
    let specialized_count = program
        .statements
        .iter()
        .filter(|s| {
            if let Statement::Function(func) = s {
                interner.resolve(func.name.node).starts_with("id__spec")
            } else {
                false
            }
        })
        .count();

    assert_eq!(
        specialized_count, 2,
        "Should create different specializations for different type args"
    );
}

// =============================================================================
// Integration Tests for Generic Classes and Interfaces
// These tests verify that generic classes, interfaces, and constraints work correctly
// =============================================================================

#[test]
fn test_generic_class_definition() {
    let source = r#"
        class Box<T> {
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Generic class with type parameter should type-check successfully"
    );
}

#[test]
fn test_generic_class_instantiation() {
    let source = r#"
        class Container<T> {
            private item: T

            constructor(item: T) {
                self.item = item
            }

            public getItem(): T {
                return self.item
            }
        }

        const numContainer = Container<number>(42)
        const strContainer = Container<string>("hello")
    "#;

    assert!(
        type_check(source).is_ok(),
        "Generic class instantiation with type arguments should work"
    );
}

#[test]
fn test_generic_interface() {
    let source = r#"
        interface Repository<T> {
            findById(id: number): T | nil
            save(entity: T): boolean
            delete(id: number): boolean
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Generic interface should type-check successfully"
    );
}

#[test]
fn test_generic_nested_class() {
    let source = r#"
        class Outer<T> {
            class Inner<U> {
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Nested generic class should type-check successfully"
    );
}

#[test]
fn test_generic_constraint_with_extends() {
    let source = r#"
        interface HasId {
            id: number
        }

        function processItem<T extends HasId>(item: T): number {
            return item.id
        }

        class Product implements HasId {
            id: number = 0
            name: string = ""
        }

        class Order implements HasId {
            id: number = 0
            total: number = 0
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Generic function with extends constraint should work"
    );
}

#[test]
fn test_generic_class_with_constraint() {
    let source = r#"
        interface Identifiable {
            getId(): number
        }

        class Registry<T extends Identifiable> {
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Generic class with constraint should work"
    );
}

#[test]
fn test_generic_multiple_constraints() {
    let source = r#"
        interface Serializable {
            serialize(): string
        }

        interface Comparable {
            compare(other: any): number
        }

        class DataSet<T extends Serializable & Comparable> {
            private data: Array<T> = {}

            public add(item: T): void {
                table.insert(self.data, item)
            }

            public serializeAll(): string {
                local result = ""
                for _, item in ipairs(self.data) do
                    result = result .. item.serialize()
                end
                return result
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Generic with multiple constraints should type-check"
    );
}

#[test]
fn test_generic_method_in_class() {
    let source = r#"
        class MathUtils {
            public identity<T>(x: T): T {
                return x
            }

            public double<T>(x: T): T {
                return x
            }

            public swap<T, U>(a: T, b: U): { first: U, second: T } {
                return { first = b, second = a }
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Generic methods in non-generic class should work"
    );
}

#[test]
fn test_generic_inheritance() {
    let source = r#"
        class Base<T> {
        }

        class Derived<T> extends Base<T> {
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Generic class inheritance should work"
    );
}

#[test]
fn test_generic_interface_implementation() {
    let source = r#"
        interface Functor<T> {
        }

        class Maybe<T> implements Functor<T> {
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Generic interface implementation should work"
    );
}
