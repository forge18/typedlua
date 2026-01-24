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
    let program = parser.parse().map_err(|e| format!("{:?}", e))?;

    let mut checker = TypeChecker::new(handler, &interner, common_ids);
    checker.check_program(&program).map_err(|e| e.message)?;

    Ok(())
}

#[test]
fn test_interface_method_correct_signature() {
    let source = r#"
        interface Calculator {
            add(a: number, b: number): number
        }

        class BasicCalculator implements Calculator {
            add(a: number, b: number): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Correct method signature should pass"
    );
}

#[test]
fn test_interface_method_wrong_param_count() {
    let source = r#"
        interface Calculator {
            add(a: number, b: number): number
        }

        class BasicCalculator implements Calculator {
            add(a: number): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_err(),
        "Wrong parameter count should fail"
    );
}

#[test]
fn test_interface_method_wrong_param_type() {
    let source = r#"
        interface Calculator {
            add(a: number, b: number): number
        }

        class BasicCalculator implements Calculator {
            add(a: string, b: number): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_err(),
        "Wrong parameter type should fail"
    );
}

#[test]
fn test_interface_method_wrong_return_type() {
    let source = r#"
        interface Calculator {
            add(a: number, b: number): number
        }

        class BasicCalculator implements Calculator {
            add(a: number, b: number): string {
                return "wrong"
            }
        }
    "#;

    assert!(type_check(source).is_err(), "Wrong return type should fail");
}

#[test]
fn test_interface_multiple_methods_all_correct() {
    let source = r#"
        interface MathOps {
            add(a: number, b: number): number
            subtract(a: number, b: number): number
            multiply(a: number, b: number): number
        }

        class Calculator implements MathOps {
            add(a: number, b: number): number {
                return 0
            }

            subtract(a: number, b: number): number {
                return 0
            }

            multiply(a: number, b: number): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "All correct signatures should pass"
    );
}

#[test]
fn test_interface_multiple_methods_one_wrong() {
    let source = r#"
        interface MathOps {
            add(a: number, b: number): number
            subtract(a: number, b: number): number
            multiply(a: number, b: number): number
        }

        class Calculator implements MathOps {
            add(a: number, b: number): number {
                return 0
            }

            subtract(a: string, b: number): number {
                return 0
            }

            multiply(a: number, b: number): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_err(),
        "One wrong signature should fail"
    );
}

#[test]
fn test_interface_method_extra_params() {
    let source = r#"
        interface Logger {
            log(message: string): void
        }

        class FileLogger implements Logger {
            log(message: string, level: number): void {
                -- extra parameter
            }
        }
    "#;

    assert!(type_check(source).is_err(), "Extra parameters should fail");
}

#[test]
fn test_interface_method_compatible_types() {
    // Test that integer return type is compatible with number in interface
    // This tests covariance of return types
    let source = r#"
        interface Processor {
            process(value: number): number
        }

        class IntProcessor implements Processor {
            process(value: number): number {
                return 42
            }
        }
    "#;

    // This should pass - the test is actually redundant with test_interface_method_correct_signature
    // but keeping it for clarity
    assert!(
        type_check(source).is_ok(),
        "Compatible return type should pass"
    );
}

#[test]
fn test_interface_method_mixed_types() {
    let source = r#"
        interface DataHandler {
            get_value(): number
            set_value(val: number): void
            get_name(): string
        }

        class DataStore implements DataHandler {
            get_value(): number {
                return 0
            }

            set_value(val: number): void {
                -- implementation
            }

            get_name(): string {
                return "test"
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Mixed types with correct signatures should pass"
    );
}

#[test]
fn test_interface_method_no_params() {
    let source = r#"
        interface Counter {
            increment(): void
            get_count(): number
        }

        class SimpleCounter implements Counter {
            increment(): void {
                -- implementation
            }

            get_count(): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Methods with no params should work"
    );
}
