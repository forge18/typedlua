use std::sync::Arc;
use typedlua_core::diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler};
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
    let program = parser.parse().map_err(|e| format!("{:?}", e))?;

    let mut checker = TypeChecker::new(handler, &interner, common_ids);
    checker.check_program(&program).map_err(|e| e.message)?;

    Ok(())
}

fn type_check_with_handler(source: &str) -> Result<Arc<CollectingDiagnosticHandler>, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let program = parser.parse().map_err(|e| format!("{:?}", e))?;

    let mut checker = TypeChecker::new(handler.clone(), &interner, common_ids);
    checker.check_program(&program).map_err(|e| e.message)?;

    Ok(handler)
}

#[test]
fn test_override_valid() {
    let source = r#"
        class Animal {
            speak(): void {
                print("...")
            }
        }

        class Dog extends Animal {
            override speak(): void {
                print("Woof!")
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Valid override should type-check successfully"
    );
}

#[test]
fn test_override_missing_parent_method() {
    let source = r#"
        class Animal {
            speak(): void {
                print("...")
            }
        }

        class Dog extends Animal {
            override fly(): void {
                print("I can't fly!")
            }
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_err(),
        "Override of non-existent parent method should fail"
    );
    assert!(
        result.unwrap_err().contains("does not have this method"),
        "Error message should mention missing parent method"
    );
}

#[test]
fn test_override_without_parent_class() {
    let source = r#"
        class Animal {
            override speak(): void {
                print("...")
            }
        }
    "#;

    let result = type_check(source);
    assert!(result.is_err(), "Override without parent class should fail");
    assert!(
        result.unwrap_err().contains("has no parent class"),
        "Error message should mention missing parent class"
    );
}

#[test]
fn test_method_without_override_keyword() {
    let source = r#"
        class Animal {
            speak(): void {
                print("...")
            }
        }

        class Dog extends Animal {
            speak(): void {
                print("Woof!")
            }
        }
    "#;

    // Without override keyword should still work but emit a warning
    let handler = type_check_with_handler(source).expect("Type checking should succeed");

    assert_eq!(
        handler.warning_count(),
        1,
        "Should emit one warning for missing override keyword"
    );

    let diagnostics = handler.get_diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("missing the 'override' keyword")),
        "Warning message should mention missing override keyword"
    );
}

#[test]
fn test_override_with_multiple_inheritance_levels() {
    let source = r#"
        class Animal {
            speak(): void {
                print("...")
            }
        }

        class Mammal extends Animal {
            override speak(): void {
                print("Mammal sound")
            }
        }

        class Dog extends Mammal {
            override speak(): void {
                print("Woof!")
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Override across multiple inheritance levels should work"
    );
}
