use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::typechecker::TypeChecker;
use std::sync::Arc;

fn type_check_with_options(source: &str, options: CompilerOptions) -> Result<(), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());

    // Lex
    let mut lexer = Lexer::new(source, handler.clone());
    let tokens = lexer.tokenize().map_err(|e| format!("Lexing failed: {:?}", e))?;

    // Parse
    let mut parser = Parser::new(tokens, handler.clone());
    let program = parser.parse().map_err(|e| format!("Parsing failed: {:?}", e))?;

    // Type check with options
    let mut type_checker = TypeChecker::new(handler.clone()).with_options(options);
    type_checker.check_program(&program).map_err(|e| e.message)?;

    Ok(())
}

// ============================================================================
// OOP Configuration Tests
// ============================================================================

#[test]
fn test_class_disabled_oop() {
    let source = r#"
        class Person {
            name: string
        }
    "#;

    let mut options = CompilerOptions::default();
    options.enable_oop = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_err(), "Class should fail when OOP is disabled");

    let error = result.unwrap_err();
    assert!(error.contains("OOP features are disabled"));
    assert!(error.contains("Person"));
    assert!(error.contains("enableOOP"));
}

#[test]
fn test_class_enabled_oop() {
    let source = r#"
        class Person {
            name: string
        }
    "#;

    let mut options = CompilerOptions::default();
    options.enable_oop = true;

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Class should succeed when OOP is enabled");
}

#[test]
fn test_interface_disabled_oop() {
    let source = r#"
        interface Drawable {
            draw(): void
        }
    "#;

    let mut options = CompilerOptions::default();
    options.enable_oop = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_err(), "Interface should fail when OOP is disabled");

    let error = result.unwrap_err();
    assert!(error.contains("OOP features are disabled"));
    assert!(error.contains("Drawable"));
    assert!(error.contains("enableOOP"));
}

#[test]
fn test_interface_enabled_oop() {
    let source = r#"
        interface Drawable {
            draw(): void
        }
    "#;

    let mut options = CompilerOptions::default();
    options.enable_oop = true;

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Interface should succeed when OOP is enabled");
}

#[test]
fn test_inheritance_disabled_oop() {
    let source = r#"
        class Animal {
            name: string
        }

        class Dog extends Animal {
            breed: string
        }
    "#;

    let mut options = CompilerOptions::default();
    options.enable_oop = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_err(), "Inheritance should fail when OOP is disabled");

    let error = result.unwrap_err();
    assert!(error.contains("OOP features are disabled"));
    // Should fail on the first class
    assert!(error.contains("Animal"));
}

#[test]
fn test_default_config_allows_oop() {
    let source = r#"
        class Person {
            name: string
        }
    "#;

    // Default configuration should have OOP enabled
    let options = CompilerOptions::default();
    assert!(options.enable_oop, "Default config should have OOP enabled");

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Default config should allow OOP");
}

#[test]
fn test_class_implements_disabled_oop() {
    let source = r#"
        interface Drawable {
            draw(): void
        }

        class Circle implements Drawable {
            draw(): void {
                const x: number = 1
            }
        }
    "#;

    let mut options = CompilerOptions::default();
    options.enable_oop = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_err(), "Class implementing interface should fail when OOP is disabled");

    let error = result.unwrap_err();
    assert!(error.contains("OOP features are disabled"));
}

#[test]
fn test_non_oop_code_unaffected() {
    let source = r#"
        function add(a: number, b: number): number {
            return a + b
        }

        const x: number = add(1, 2)
    "#;

    let mut options = CompilerOptions::default();
    options.enable_oop = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Non-OOP code should work even when OOP is disabled");
}

// ============================================================================
// FP Configuration Tests
// ============================================================================

#[test]
fn test_match_disabled_fp() {
    let source = r#"
        const x = 5
        const result = match x {
            1 => "one",
            _ => "other"
        }
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_err(), "Match expressions should fail when FP is disabled");

    let error = result.unwrap_err();
    assert!(error.contains("FP features"));
    assert!(error.contains("enableFP"));
}

#[test]
fn test_match_enabled_fp() {
    let source = r#"
        const x = 5
        const result = match x {
            1 => "one",
            _ => "other"
        }
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = true;

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Match expressions should succeed when FP is enabled");
}

#[test]
fn test_pipe_disabled_fp() {
    let source = r#"
        const double = (x: number): number => x * 2
        const value = 5
        const result = value |> double
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_err(), "Pipe operator should fail when FP is disabled");

    let error = result.unwrap_err();
    assert!(error.contains("Pipe operator"));
    assert!(error.contains("FP features"));
    assert!(error.contains("enableFP"));
}

#[test]
fn test_pipe_enabled_fp() {
    let source = r#"
        const double = (x: number): number => x * 2
        const value = 5
        const result = value |> double
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = true;

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Pipe operator should succeed when FP is enabled");
}

#[test]
fn test_array_spread_disabled_fp() {
    let source = r#"
        const arr1 = [1, 2]
        const arr2 = [...arr1, 3]
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_err(), "Array spread should fail when FP is disabled");

    let error = result.unwrap_err();
    assert!(error.contains("Spread operator"));
    assert!(error.contains("FP features"));
}

#[test]
fn test_array_spread_enabled_fp() {
    let source = r#"
        const arr1 = [1, 2]
        const arr2 = [...arr1, 3]
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = true;

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Array spread should succeed when FP is enabled");
}

#[test]
fn test_object_spread_disabled_fp() {
    let source = r#"
        const obj1 = {x: 1}
        const obj2 = {...obj1, y: 2}
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_err(), "Object spread should fail when FP is disabled");

    let error = result.unwrap_err();
    assert!(error.contains("Spread operator"));
    assert!(error.contains("FP features"));
}

#[test]
fn test_object_spread_enabled_fp() {
    let source = r#"
        const obj1 = {x: 1}
        const obj2 = {...obj1, y: 2}
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = true;

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Object spread should succeed when FP is enabled");
}

#[test]
fn test_array_destructuring_disabled_fp() {
    let source = r#"
        const arr = [1, 2, 3]
        const [a, b, c] = arr
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_err(), "Array destructuring should fail when FP is disabled");

    let error = result.unwrap_err();
    assert!(error.contains("destructuring"));
    assert!(error.contains("FP features"));
}

#[test]
fn test_array_destructuring_enabled_fp() {
    let source = r#"
        const arr = [1, 2, 3]
        const [a, b, c] = arr
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = true;

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Array destructuring should succeed when FP is enabled");
}

#[test]
fn test_object_destructuring_disabled_fp() {
    let source = r#"
        const obj = {x: 10, y: 20}
        const {x, y} = obj
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_err(), "Object destructuring should fail when FP is disabled");

    let error = result.unwrap_err();
    assert!(error.contains("destructuring"));
    assert!(error.contains("FP features"));
}

#[test]
fn test_object_destructuring_enabled_fp() {
    let source = r#"
        const obj = {x: 10, y: 20}
        const {x, y} = obj
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = true;

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Object destructuring should succeed when FP is enabled");
}

#[test]
fn test_rest_parameters_disabled_fp() {
    let source = r#"
        function sum(...numbers: number): number
            return 0
        end
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_err(), "Rest parameters should fail when FP is disabled");

    let error = result.unwrap_err();
    assert!(error.contains("Rest parameters"));
    assert!(error.contains("FP features"));
}

#[test]
fn test_rest_parameters_enabled_fp() {
    let source = r#"
        function sum(...numbers: number): number
            return 0
        end
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = true;

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Rest parameters should succeed when FP is enabled");
}

#[test]
fn test_default_config_allows_fp() {
    let source = r#"
        const arr = [1, 2]
        const result = [...arr, 3]
    "#;

    // Default configuration should have FP enabled
    let options = CompilerOptions::default();
    assert!(options.enable_fp, "Default config should have FP enabled");

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Default config should allow FP features");
}

#[test]
fn test_non_fp_code_unaffected() {
    let source = r#"
        function add(a: number, b: number): number
            return a + b
        end

        const x: number = add(1, 2)
    "#;

    let mut options = CompilerOptions::default();
    options.enable_fp = false;

    let result = type_check_with_options(source, options);
    assert!(result.is_ok(), "Non-FP code should work even when FP is disabled");
}
