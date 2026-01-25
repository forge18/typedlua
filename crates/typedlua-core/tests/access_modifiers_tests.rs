use std::sync::Arc;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn type_check(source: &str) -> Result<(), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser.parse().map_err(|e| format!("{:?}", e))?;

    let mut checker = TypeChecker::new(handler, &interner, &common_ids);
    checker.check_program(&mut program).map_err(|e| e.message)?;

    Ok(())
}

#[test]
fn test_class_with_public_members() {
    let source = r#"
        class Point {
            public x: number = 0
            public y: number = 0
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Class with public members should type-check successfully"
    );
}

#[test]
fn test_class_with_private_members() {
    let source = r#"
        class Point {
            private x: number = 0
            private y: number = 0
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Class with private members should type-check successfully"
    );
}

#[test]
fn test_class_with_protected_members() {
    let source = r#"
        class Point {
            protected x: number = 0
            protected y: number = 0
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Class with protected members should type-check successfully"
    );
}

#[test]
fn test_class_with_mixed_access_modifiers() {
    let source = r#"
        class BankAccount {
            private balance: number = 0
            protected owner: string = "unknown"
            public account_id: string = "12345"
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Class with mixed access modifiers should type-check successfully"
    );
}

#[test]
fn test_class_methods_with_access_modifiers() {
    let source = r#"
        class Calculator {
            private helper(): number {
                return 42
            }

            protected internal_calc(): number {
                return 21
            }

            public compute(): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Class methods with access modifiers should type-check successfully"
    );
}

#[test]
fn test_class_getters_setters_with_access_modifiers() {
    let source = r#"
        class Person {
            private _name: string = ""

            public get name(): string {
                return "test"
            }

            private set name(value: string) {
                -- setter body
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Getters and setters with access modifiers should type-check successfully"
    );
}

#[test]
fn test_class_static_members_with_access_modifiers() {
    let source = r#"
        class Counter {
            private static count: number = 0
            protected static limit: number = 100
            public static name: string = "Counter"
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Static members with access modifiers should type-check successfully"
    );
}

#[test]
fn test_inheritance_with_access_modifiers() {
    let source = r#"
        class Animal {
            private secret: string = "hidden"
            protected species: string = "unknown"
            public name: string = "animal"
        }

        class Dog extends Animal {
            private breed: string = "mutt"
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Inheritance with access modifiers should type-check successfully"
    );
}

#[test]
fn test_default_access_modifier_is_public() {
    let source = r#"
        class Point {
            x: number = 0
            y: number = 0

            get_x(): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Members without access modifiers should default to public and type-check successfully"
    );
}

#[test]
fn test_multiple_classes_with_access_modifiers() {
    let source = r#"
        class Point {
            private x: number = 0
            public y: number = 0
        }

        class Circle {
            protected radius: number = 1
            public center: string = "0,0"
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Multiple classes with access modifiers should type-check successfully"
    );
}
