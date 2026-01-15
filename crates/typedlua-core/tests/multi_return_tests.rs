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
        ..Default::default()
    });

    checker.check_program(&program).map_err(|e| e.message)?;

    Ok(())
}

#[test]
fn test_single_return_value() {
    let source = r#"
        function get_number(): number {
            return 42
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Single return value should type-check"
    );
}

#[test]
fn test_tuple_return_type() {
    let source = r#"
        function get_coords(): [number, number] {
            return 10, 20
        }
    "#;

    assert!(type_check(source).is_ok(), "Tuple return should type-check");
}

#[test]
fn test_multi_return_all_checked() {
    // This test verifies that ALL return values are type-checked, not just the first
    let source = r#"
        function get_values(): [number, number, number] {
            return 1, 2, 3
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "All return values should be checked"
    );
}

#[test]
fn test_multi_return_type_mismatch_first() {
    // Type error in first return value
    let source = r#"
        function get_values(): [number, number] {
            return "wrong", 2
        }
    "#;

    assert!(
        type_check(source).is_err(),
        "Type error in first return value should be caught"
    );
}

#[test]
fn test_multi_return_type_mismatch_second() {
    // Type error in second return value - this tests that we're not just checking the first value
    let source = r#"
        function get_values(): [number, number] {
            return 1, "wrong"
        }
    "#;

    assert!(
        type_check(source).is_err(),
        "Type error in second return value should be caught"
    );
}

#[test]
fn test_multi_return_type_mismatch_third() {
    // Type error in third return value
    let source = r#"
        function get_values(): [number, number, number] {
            return 1, 2, "wrong"
        }
    "#;

    assert!(
        type_check(source).is_err(),
        "Type error in third return value should be caught"
    );
}

#[test]
fn test_multi_return_wrong_count() {
    // Wrong number of return values
    let source = r#"
        function get_values(): [number, number] {
            return 1, 2, 3
        }
    "#;

    assert!(
        type_check(source).is_err(),
        "Wrong number of return values should be an error"
    );
}

#[test]
fn test_multi_return_too_few() {
    // Too few return values
    let source = r#"
        function get_values(): [number, number] {
            return 1
        }
    "#;

    assert!(
        type_check(source).is_err(),
        "Too few return values should be an error"
    );
}

#[test]
fn test_triple_return_correct() {
    let source = r#"
        function get_rgb(): [number, number, number] {
            return 255, 128, 64
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Triple return with correct types should pass"
    );
}

#[test]
fn test_mixed_types_return() {
    let source = r#"
        function get_mixed(): [string, number, boolean] {
            return "hello", 42, true
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Mixed type tuple return should work"
    );
}
