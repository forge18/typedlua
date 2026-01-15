use std::sync::Arc;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::typechecker::TypeChecker;

fn type_check(source: &str) -> Result<(), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new(source, handler.clone());
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone());
    let program = parser.parse().map_err(|e| format!("{:?}", e))?;

    let mut checker = TypeChecker::new(handler);
    checker = checker.with_options(CompilerOptions {
        enable_decorators: true,
        ..Default::default()
    });

    checker.check_program(&program).map_err(|e| e.message)?;

    Ok(())
}

#[test]
fn test_builtin_readonly_decorator() {
    let source = r#"
        class Config {
            @readonly
            api_key: string = "secret"
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Built-in @readonly decorator should be recognized"
    );
}

#[test]
fn test_builtin_sealed_decorator() {
    let source = r#"
        @sealed
        class FinalClass {
            value: number = 0
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Built-in @sealed decorator should be recognized"
    );
}

#[test]
fn test_builtin_deprecated_decorator() {
    let source = r#"
        class LegacyApi {
            @deprecated
            old_method(): void {
                -- implementation
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Built-in @deprecated decorator should be recognized"
    );
}

#[test]
fn test_builtin_override_decorator() {
    let source = r#"
        class Base {
            get_value(): number {
                return 0
            }
        }

        class Derived extends Base {
            @override
            get_value(): number {
                return 42
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Built-in @override decorator should be recognized"
    );
}

#[test]
fn test_builtin_experimental_decorator() {
    let source = r#"
        class ExperimentalFeatures {
            @experimental
            new_feature(): void {
                -- implementation
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Built-in @experimental decorator should be recognized"
    );
}

#[test]
fn test_custom_decorator_function() {
    let source = r#"
        function logged(target: any): any {
            return target
        }

        @logged
        class MyClass {
            value: number = 0
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Custom decorator function should be recognized"
    );
}

#[test]
fn test_decorator_with_arguments() {
    let source = r#"
        function configurable(enabled: boolean): any {
            return function(target: any): any {
                return target
            }
        }

        class Settings {
            @configurable(true)
            option: string = "default"
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator with arguments should be validated"
    );
}

#[test]
fn test_multiple_decorators() {
    let source = r#"
        class ApiEndpoint {
            @readonly
            @deprecated
            endpoint: string = "/api/v1"
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Multiple decorators should all be validated"
    );
}

#[test]
fn test_decorator_on_method() {
    let source = r#"
        class Service {
            @readonly
            get_status(): string {
                return "active"
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorators on methods should be validated"
    );
}

#[test]
fn test_decorator_on_getter() {
    let source = r#"
        class DataStore {
            @deprecated
            get value(): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorators on getters should be validated"
    );
}

#[test]
fn test_decorator_on_setter() {
    let source = r#"
        class DataStore {
            @deprecated
            set value(v: number) {
                -- setter body
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorators on setters should be validated"
    );
}

#[test]
fn test_decorator_disabled_by_config() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let source = r#"
        class MyClass {
            @readonly
            value: number = 0
        }
    "#;

    let mut lexer = Lexer::new(source, handler.clone());
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone());
    let program = parser.parse().unwrap();

    let mut checker = TypeChecker::new(handler);
    checker = checker.with_options(CompilerOptions {
        enable_decorators: false, // Decorators disabled
        ..Default::default()
    });

    let result = checker.check_program(&program);
    assert!(
        result.is_err(),
        "Decorators should fail when disabled in config"
    );
}

#[test]
fn test_unknown_decorator_allowed() {
    // Unknown decorators are allowed (could be from imports/libraries)
    let source = r#"
        @unknownDecorator
        class MyClass {
            value: number = 0
        }
    "#;

    // This should pass - unknown decorators are allowed
    assert!(
        type_check(source).is_ok(),
        "Unknown decorators should be allowed (could be from imports)"
    );
}
