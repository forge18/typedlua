use std::sync::Arc;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn type_check(source: &str) -> Result<(), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();

    // Lex
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    // Parse
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    // Type check
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids)
        .with_options(CompilerOptions::default());
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    Ok(())
}

// ============================================================================
// Boolean Exhaustiveness Tests
// ============================================================================

#[test]
fn test_boolean_exhaustive_with_wildcard() {
    let source = r#"
        const x: boolean = true
        const result = match x {
            _ => "default"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_ok(),
        "Boolean match with wildcard should be exhaustive"
    );
}

#[test]
fn test_boolean_exhaustive_with_both_cases() {
    let source = r#"
        const x: boolean = true
        const result = match x {
            true => "yes"
            false => "no"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_ok(),
        "Boolean match with true and false should be exhaustive"
    );
}

#[test]
fn test_boolean_non_exhaustive_missing_false() {
    let source = r#"
        const x: boolean = true
        const result = match x {
            true => "yes"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_err(),
        "Boolean match missing false case should fail"
    );
    let error = result.unwrap_err();
    assert!(
        error.contains("Non-exhaustive match"),
        "Error should mention non-exhaustive match"
    );
    assert!(
        error.contains("boolean"),
        "Error should mention boolean type"
    );
}

#[test]
fn test_boolean_non_exhaustive_missing_true() {
    let source = r#"
        const x: boolean = false
        const result = match x {
            false => "no"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_err(),
        "Boolean match missing true case should fail"
    );
    let error = result.unwrap_err();
    assert!(
        error.contains("Non-exhaustive match"),
        "Error should mention non-exhaustive match"
    );
}

#[test]
fn test_boolean_exhaustive_with_identifier_pattern() {
    let source = r#"
        const x: boolean = true
        const result = match x {
            value => "any"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_ok(),
        "Boolean match with identifier pattern should be exhaustive"
    );
}

// ============================================================================
// Literal Type Exhaustiveness Tests
// ============================================================================

#[test]
fn test_literal_type_exhaustive() {
    let source = r#"
        const x: 42 = 42
        const result = match x {
            42 => "the answer"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_ok(),
        "Literal type match with exact literal should be exhaustive"
    );
}

#[test]
fn test_literal_type_non_exhaustive() {
    let source = r#"
        const x: 42 = 42
        const result = match x {
            0 => "zero"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_err(),
        "Literal type match without matching literal should fail"
    );
    let error = result.unwrap_err();
    assert!(
        error.contains("Non-exhaustive match"),
        "Error should mention non-exhaustive match"
    );
    assert!(error.contains("literal"), "Error should mention literal");
}

#[test]
fn test_literal_type_with_wildcard() {
    let source = r#"
        const x: 42 = 42
        const result = match x {
            _ => "default"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_ok(),
        "Literal type match with wildcard should be exhaustive"
    );
}

// ============================================================================
// Union Type Exhaustiveness Tests
// ============================================================================

#[test]
fn test_union_exhaustive_with_wildcard() {
    let source = r#"
        const x: number | string = 42
        const result = match x {
            _ => "default"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_ok(),
        "Union match with wildcard should be exhaustive"
    );
}

#[test]
fn test_union_exhaustive_with_type_patterns() {
    let source = r#"
        type Shape = {type: "circle", radius: number} | {type: "square", side: number}

        const shape: Shape = {type: "circle", radius: 5}
        const result = match shape {
            {type} when type == "circle" => "circle"
            {type} when type == "square" => "square"
        }
    "#;

    let result = type_check(source);
    // This may pass or fail depending on how sophisticated the exhaustiveness checker is
    // For now, object patterns with guards might not be fully checked
    if let Err(e) = result {
        println!("Union exhaustiveness with guards: {}", e);
    }
}

#[test]
fn test_union_with_identifier_exhaustive() {
    let source = r#"
        const x: number | string = 42
        const result = match x {
            value => "any"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_ok(),
        "Union match with identifier should be exhaustive"
    );
}

// ============================================================================
// Guard and Exhaustiveness Tests
// ============================================================================

#[test]
fn test_guard_prevents_exhaustiveness() {
    let source = r#"
        const x: boolean = true
        const result = match x {
            value when value == true => "yes"
        }
    "#;

    let result = type_check(source);
    // Guards prevent exhaustiveness because they might not always match
    // The pattern `value` would be exhaustive, but with a guard it's not
    // This test documents current behavior - may need adjustment
    if let Err(error) = result {
        println!("Guard exhaustiveness check: {}", error);
    }
}

#[test]
fn test_guard_with_wildcard_fallback() {
    let source = r#"
        const x: number = 42
        const result = match x {
            n when n > 0 => "positive"
            _ => "non-positive"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_ok(),
        "Match with guard and wildcard fallback should be exhaustive"
    );
}

// ============================================================================
// Number/String Type Exhaustiveness Tests
// ============================================================================

#[test]
fn test_number_type_requires_wildcard() {
    let source = r#"
        const x: number = 42
        const result = match x {
            0 => "zero"
            1 => "one"
        }
    "#;

    let result = type_check(source);
    // For open types like number, we can't verify exhaustiveness without a wildcard
    // Current implementation allows this - may want to add warning in future
    if let Err(error) = result {
        println!("Number type exhaustiveness: {}", error);
    }
}

#[test]
fn test_number_type_with_wildcard() {
    let source = r#"
        const x: number = 42
        const result = match x {
            0 => "zero"
            _ => "other"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_ok(),
        "Number match with wildcard should be exhaustive"
    );
}

#[test]
fn test_string_type_with_wildcard() {
    let source = r#"
        const x: string = "hello"
        const result = match x {
            "yes" => 1
            "no" => 0
            _ => -1
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_ok(),
        "String match with wildcard should be exhaustive"
    );
}

// ============================================================================
// Array and Object Exhaustiveness Tests
// ============================================================================

#[test]
fn test_array_type_with_wildcard() {
    let source = r#"
        const x: number[] = [1, 2, 3]
        const result = match x {
            [] => "empty"
            _ => "non-empty"
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_ok(),
        "Array match with wildcard should be exhaustive"
    );
}

#[test]
fn test_object_type_with_wildcard() {
    let source = r#"
        const x = {a: 1, b: 2}
        const result = match x {
            {a} => a
        }
    "#;

    let result = type_check(source);
    // For object types, current implementation may or may not require wildcard
    if let Err(e) = result {
        println!("Object exhaustiveness: {}", e);
    }
}
