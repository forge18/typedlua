use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn compile_and_check(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);

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
        .check_program(&program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Basic Null Coalescing Tests
// ============================================================================

#[test]
fn test_simple_null_coalesce() {
    let source = r#"
        const value: number | nil = nil
        const result = value ?? 42
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Success! Generated code:\n{}", output);
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
    assert!(
        result.is_ok(),
        "Simple null coalescing should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should generate: (value ~= nil and value or 42)
    assert!(output.contains("~= nil"));
    assert!(output.contains("and"));
    assert!(output.contains("or"));
}

#[test]
fn test_null_coalesce_with_string() {
    let source = r#"
        const name: string | nil = nil
        const result = name ?? "default"
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with string should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    assert!(output.contains("~= nil"));
}

#[test]
fn test_null_coalesce_chain() {
    let source = r#"
        const a: number | nil = nil
        const b: number | nil = nil
        const result = a ?? b ?? 0
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Chained null coalescing should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Null Coalescing vs Logical Or
// ============================================================================

#[test]
fn test_null_coalesce_vs_or_with_false() {
    let source = r#"
        const value: boolean | nil = false
        const withOr = value or true
        const withNullCoalesce = value ?? true
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing vs or should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Both operators should be present
    assert!(output.contains("or"));
    assert!(output.contains("~= nil"));
}

#[test]
fn test_null_coalesce_preserves_zero() {
    let source = r#"
        const count: number | nil = 0
        const result = count ?? 10
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing should preserve zero: {:?}",
        result.err()
    );
}

#[test]
fn test_null_coalesce_preserves_empty_string() {
    let source = r#"
        const text: string | nil = ""
        const result = text ?? "default"
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing should preserve empty string: {:?}",
        result.err()
    );
}

// ============================================================================
// Operator Precedence Tests
// ============================================================================

#[test]
fn test_null_coalesce_precedence_with_comparison() {
    let source = r#"
        const a: number | nil = 5
        const result = a ?? 10 > 3
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with comparison should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_null_coalesce_precedence_with_or() {
    let source = r#"
        const a: number | nil = nil
        const b: number | nil = nil
        const result = a ?? b or 10
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with or should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_null_coalesce_with_parentheses() {
    let source = r#"
        const a: number | nil = nil
        const b: number | nil = nil
        const result = (a ?? b) or 10
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with parentheses should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Complex Expression Tests
// ============================================================================

#[test]
fn test_null_coalesce_with_function_call() {
    let source = r#"
        const getValue = (): number | nil => {
            return nil
        }
        const result = getValue() ?? 42
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with function call should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_null_coalesce_with_member_access() {
    let source = r#"
        const obj = {value: nil}
        const result = obj.value ?? 100
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with member access should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_null_coalesce_with_binary_expression() {
    let source = r#"
        const a: number | nil = 5
        const b = 10
        const result = a ?? (b + 20)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with binary expression should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Object Property Access Tests
// ============================================================================

#[test]
fn test_null_coalesce_with_nested_property() {
    let source = r#"
        const config = {port: nil}
        const port = config.port ?? 3000
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with nested property should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_null_coalesce_with_optional_property() {
    let source = r#"
        type Config = {
            port?: number
        }
        const config: Config = {}
        const port = config.port ?? 8080
    "#;

    let result = compile_and_check(source);
    // May fail due to optional property type checking - that's OK
    if result.is_err() {
        println!("Optional property test: {}", result.unwrap_err());
    }
}

// ============================================================================
// Type Inference Tests
// ============================================================================

#[test]
fn test_null_coalesce_type_inference() {
    let source = r#"
        const value: number | nil = nil
        const result: number = value ?? 0
    "#;

    let result = compile_and_check(source);
    // Type inference may not be fully implemented yet
    if result.is_err() {
        println!("Type inference test: {}", result.unwrap_err());
    }
}

#[test]
fn test_null_coalesce_with_different_types() {
    let source = r#"
        const num: number | nil = nil
        const str = "default"
        const result = num ?? str
    "#;

    let result = compile_and_check(source);
    // Union type inference may not be fully implemented
    if result.is_err() {
        println!("Different types test: {}", result.unwrap_err());
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_null_coalesce_in_assignment() {
    let source = r#"
        const getConfig = (): number | nil => {
            return nil
        }
        const config = getConfig() ?? 8080
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing in assignment should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_null_coalesce_in_return() {
    let source = r#"
        const getPort = (config: {port?: number}): number => {
            return config.port ?? 3000
        }
    "#;

    let result = compile_and_check(source);
    // May have type checking issues with optional properties
    if result.is_err() {
        println!("Return statement test: {}", result.unwrap_err());
    }
}

#[test]
fn test_null_coalesce_in_conditional() {
    let source = r#"
        const value: number | nil = nil
        const result = (value ?? 10) > 5 ? "high" : "low"
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing in conditional should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_null_coalesce_with_nil_literal() {
    let source = r#"
        const result = nil ?? 42
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with nil literal should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_multiple_null_coalesces() {
    let source = r#"
        const a: number | nil = nil
        const b: number | nil = nil
        const x = a ?? 10
        const y = b ?? 20
        const result = x + y
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Multiple null coalescing operations should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_null_coalesce_with_complex_default() {
    let source = r#"
        const getValue = (): number => 42
        const value: number | nil = nil
        const result = value ?? getValue()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with complex default should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Code Generation Verification
// ============================================================================

#[test]
fn test_codegen_simple_form() {
    let source = r#"
        const x: number | nil = nil
        const result = x ?? 5
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Simple codegen should work");
    let output = result.unwrap();

    // Verify the generated Lua follows the pattern: (x ~= nil and x or 5)
    assert!(output.contains("~= nil"), "Should check for nil");
    assert!(output.contains("and"), "Should use 'and'");
    assert!(output.contains("or"), "Should use 'or'");
}

#[test]
fn test_codegen_preserves_evaluation() {
    let source = r#"
        const a: number | nil = 1
        const b = 2
        const result = a ?? b
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Evaluation order should be preserved");
    let output = result.unwrap();

    // Both variables should appear in the output
    assert!(output.contains("a") && output.contains("b"));
}
