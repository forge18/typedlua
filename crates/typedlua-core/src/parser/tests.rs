use super::*;
use crate::diagnostics::CollectingDiagnosticHandler;
use crate::lexer::Lexer;
use std::sync::Arc;

fn parse_source(source: &str) -> Result<Program, ParserError> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new(source, handler.clone());
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, handler);
    parser.parse()
}

#[test]
fn test_parse_variable_declaration() {
    let source = "const x: number = 42";
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Variable(decl) => {
            assert!(matches!(
                decl.kind,
                crate::ast::statement::VariableKind::Const
            ));
            assert!(decl.type_annotation.is_some());
        }
        _ => panic!("Expected variable declaration"),
    }
}

#[test]
fn test_parse_function_declaration() {
    let source = r#"
        function add(a: number, b: number): number
            return a + b
        end
    "#;
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Function(func) => {
            assert_eq!(func.parameters.len(), 2);
            assert!(func.return_type.is_some());
        }
        _ => panic!("Expected function declaration"),
    }
}

#[test]
fn test_parse_if_statement() {
    let source = r#"
        if x > 0 then
            print("positive")
        elseif x < 0 then
            print("negative")
        else
            print("zero")
        end
    "#;
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::If(if_stmt) => {
            assert_eq!(if_stmt.else_ifs.len(), 1);
            assert!(if_stmt.else_block.is_some());
        }
        _ => panic!("Expected if statement"),
    }
}

#[test]
fn test_parse_for_numeric() {
    let source = r#"
        for i = 1, 10, 2 do
            print(i)
        end
    "#;
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::For(for_stmt) => match for_stmt {
            crate::ast::statement::ForStatement::Numeric(numeric) => {
                assert!(numeric.step.is_some());
            }
            _ => panic!("Expected numeric for"),
        },
        _ => panic!("Expected for statement"),
    }
}

#[test]
fn test_parse_for_generic() {
    let source = r#"
        for k, v in pairs(t) do
            print(k, v)
        end
    "#;
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::For(for_stmt) => match for_stmt {
            crate::ast::statement::ForStatement::Generic(generic) => {
                assert_eq!(generic.variables.len(), 2);
                assert_eq!(generic.iterators.len(), 1);
            }
            _ => panic!("Expected generic for"),
        },
        _ => panic!("Expected for statement"),
    }
}

#[test]
fn test_parse_interface_declaration() {
    let source = r#"
        interface Point {
            x: number,
            y: number
        }
    "#;
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Interface(iface) => {
            assert_eq!(iface.members.len(), 2);
        }
        _ => panic!("Expected interface declaration"),
    }
}

#[test]
fn test_parse_type_alias() {
    let source = "type Point = { x: number, y: number }";
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::TypeAlias(_) => {}
        _ => panic!("Expected type alias"),
    }
}

#[test]
fn test_parse_enum() {
    let source = r#"
        enum Color {
            Red = 1,
            Green = 2,
            Blue = 3
        }
    "#;
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Enum(enum_decl) => {
            assert_eq!(enum_decl.members.len(), 3);
        }
        _ => panic!("Expected enum declaration"),
    }
}

#[test]
fn test_parse_binary_expression() {
    let source = "const result = 1 + 2 * 3";
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Variable(decl) => {
            // Check that multiplication has higher precedence than addition
            match &decl.initializer.kind {
                crate::ast::expression::ExpressionKind::Binary(
                    crate::ast::expression::BinaryOp::Add,
                    _,
                    right,
                ) => match &right.kind {
                    crate::ast::expression::ExpressionKind::Binary(
                        crate::ast::expression::BinaryOp::Multiply,
                        _,
                        _,
                    ) => {}
                    _ => panic!("Expected multiplication on right side"),
                },
                _ => panic!("Expected addition at top level"),
            }
        }
        _ => panic!("Expected variable declaration"),
    }
}

#[test]
fn test_parse_array_literal() {
    let source = "const arr = [1, 2, 3]";
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Variable(decl) => {
            match &decl.initializer.kind {
                crate::ast::expression::ExpressionKind::Array(elements) => {
                    assert_eq!(elements.len(), 3);
                }
                _ => panic!("Expected array literal"),
            }
        }
        _ => panic!("Expected variable declaration"),
    }
}

#[test]
fn test_parse_object_literal() {
    let source = "const obj = { x = 1, y = 2 }";
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Variable(decl) => {
            match &decl.initializer.kind {
                crate::ast::expression::ExpressionKind::Object(properties) => {
                    assert_eq!(properties.len(), 2);
                }
                _ => panic!("Expected object literal"),
            }
        }
        _ => panic!("Expected variable declaration"),
    }
}

#[test]
fn test_parse_function_call() {
    let source = "print(x, y, z)";
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Expression(expr) => match &expr.kind {
            crate::ast::expression::ExpressionKind::Call(_, args) => {
                assert_eq!(args.len(), 3);
            }
            _ => panic!("Expected function call"),
        },
        _ => panic!("Expected expression statement"),
    }
}

#[test]
fn test_parse_member_access() {
    let source = "obj.field.nested";
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Expression(expr) => match &expr.kind {
            crate::ast::expression::ExpressionKind::Member(inner, _) => match &inner.kind {
                crate::ast::expression::ExpressionKind::Member(_, _) => {}
                _ => panic!("Expected nested member access"),
            },
            _ => panic!("Expected member access"),
        },
        _ => panic!("Expected expression statement"),
    }
}

#[test]
fn test_parse_array_pattern() {
    let source = "const [a, b, c] = arr";
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Variable(decl) => {
            match &decl.pattern {
                crate::ast::pattern::Pattern::Array(arr_pattern) => {
                    assert_eq!(arr_pattern.elements.len(), 3);
                }
                _ => panic!("Expected array pattern"),
            }
        }
        _ => panic!("Expected variable declaration"),
    }
}

#[test]
fn test_parse_object_pattern() {
    let source = "const { x, y } = point";
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Variable(decl) => {
            match &decl.pattern {
                crate::ast::pattern::Pattern::Object(obj_pattern) => {
                    assert_eq!(obj_pattern.properties.len(), 2);
                }
                _ => panic!("Expected object pattern"),
            }
        }
        _ => panic!("Expected variable declaration"),
    }
}

#[test]
fn test_parse_union_type() {
    let source = "const x: string | number = 42";
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Variable(decl) => {
            match &decl.type_annotation {
                Some(ty) => match &ty.kind {
                    crate::ast::types::TypeKind::Union(types) => {
                        assert_eq!(types.len(), 2);
                    }
                    _ => panic!("Expected union type"),
                },
                None => panic!("Expected type annotation"),
            }
        }
        _ => panic!("Expected variable declaration"),
    }
}

#[test]
fn test_parse_array_type() {
    let source = "const arr: number[] = [1, 2, 3]";
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        crate::ast::statement::Statement::Variable(decl) => {
            match &decl.type_annotation {
                Some(ty) => match &ty.kind {
                    crate::ast::types::TypeKind::Array(_) => {}
                    _ => panic!("Expected array type"),
                },
                None => panic!("Expected type annotation"),
            }
        }
        _ => panic!("Expected variable declaration"),
    }
}

#[test]
fn test_parse_complex_program() {
    let source = r#"
        interface User {
            name: string,
            age: number
        }

        function greet(user: User): string
            return "Hello, " .. user.name
        end

        const user: User = { name = "Alice", age = 30 }
        print(greet(user))
    "#;
    let program = parse_source(source).expect("Parse failed");
    assert_eq!(program.statements.len(), 4);

    // Check that we have an interface, function, const, and expression statement
    assert!(matches!(
        &program.statements[0],
        crate::ast::statement::Statement::Interface(_)
    ));
    assert!(matches!(
        &program.statements[1],
        crate::ast::statement::Statement::Function(_)
    ));
    assert!(matches!(
        &program.statements[2],
        crate::ast::statement::Statement::Variable(_)
    ));
    assert!(matches!(
        &program.statements[3],
        crate::ast::statement::Statement::Expression(_)
    ));
}
