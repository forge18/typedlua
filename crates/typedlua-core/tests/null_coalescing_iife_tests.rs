use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn compile_and_check(source: &str) -> Result<String, String> {
    compile_with_optimization(source)
}

fn compile_with_optimization(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let mut options = CompilerOptions::default();

    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options.clone());
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// IIFE Form for Complex Expressions
// ============================================================================

#[test]
fn test_iife_for_function_call() {
    let source = r#"
        function getValue(): number | nil {
            return nil
        }
        const result = getValue() ?? 42
    "#;

    let output = compile_with_optimization(source).unwrap();

    // Should use IIFE to avoid calling getValue() twice
    assert!(
        output.contains("function()"),
        "Should use IIFE for function call"
    );
    assert!(
        output.contains("local __left"),
        "Should use __left variable"
    );
    assert!(
        output.contains("__left = getValue()"),
        "Should assign getValue() result to __left"
    );
    assert!(
        output.contains("__left ~= nil and __left or"),
        "Should use __left in nil check"
    );
}

#[test]
fn test_iife_for_complex_expression() {
    let source = r#"
        const obj = { nested: { value: 42 } }
        const result = obj.nested.value ?? 0
    "#;

    let output = compile_with_optimization(source).unwrap();

    // Member access on identifier is considered simple, so should use simple form
    assert!(
        output.contains("~= nil and"),
        "Should use simple form for member access"
    );
}

#[test]
fn test_iife_for_index_with_expression() {
    let source = r#"
        const arr = [1, 2, 3]
        function getIndex(): number { return 0 }
        const result = arr[getIndex()] ?? 0
    "#;

    let output = compile_with_optimization(source).unwrap();

    // Index with function call is complex
    assert!(
        output.contains("function()"),
        "Should use IIFE for index with function call"
    );
    assert!(
        output.contains("local __left"),
        "Should use __left variable"
    );
}

// ============================================================================
// Simple Expression Optimization
// ============================================================================

#[test]
fn test_simple_form_for_identifier() {
    let source = r#"
        const value: number | nil = nil
        const result = value ?? 42
    "#;

    let output = compile_with_optimization(source).unwrap();

    // Identifier is simple, should use simple form
    assert!(
        output.contains("value ~= nil and value or"),
        "Should use simple form for identifier"
    );
    assert!(
        !output.contains("function()"),
        "Should NOT use IIFE for simple identifier"
    );
}

#[test]
fn test_simple_form_for_literal() {
    let source = r#"
        const result = nil ?? 42
    "#;

    let output = compile_with_optimization(source).unwrap();

    // Literal is simple
    assert!(
        output.contains("~= nil and"),
        "Should use simple form for literal"
    );
    assert!(
        !output.contains("function()"),
        "Should NOT use IIFE for literal"
    );
}

#[test]
fn test_simple_form_for_member_access() {
    let source = r#"
        const obj = { value: 42 }
        const result = obj.value ?? 0
    "#;

    let output = compile_with_optimization(source).unwrap();

    // Simple member access is simple
    assert!(
        output.contains("~= nil and"),
        "Should use simple form for member access"
    );
    assert!(
        !output.contains("function()"),
        "Should NOT use IIFE for simple member access"
    );
}

// ============================================================================
// O2 Optimization: Skip Nil Checks for Guaranteed Non-Nil
// These tests require O2 optimization level which is not yet implemented
// ============================================================================

#[test]
#[ignore]
fn test_o2_skip_check_for_object_literal() {
    let source = r#"
        const result = { value: 42 } ?? { value: 0 }
    "#;

    let output = compile_with_optimization(source).unwrap();

    // Object literal is guaranteed non-nil, should be optimized away
    assert!(
        !output.contains("~= nil"),
        "O2 should skip nil check for object literal"
    );
    assert!(
        output.contains("value = 42"),
        "Should just use the object literal"
    );
}

#[test]
#[ignore]
fn test_o2_skip_check_for_array_literal() {
    let source = r#"
        const result = [1, 2, 3] ?? []
    "#;

    let output = compile_with_optimization(source).unwrap();

    // Array literal is guaranteed non-nil
    assert!(
        !output.contains("~= nil"),
        "O2 should skip nil check for array literal"
    );
}

#[test]
#[ignore]
fn test_o2_skip_check_for_new_expression() {
    let source = r#"
        class MyClass {}
        const result = new MyClass() ?? nil
    "#;

    let output = compile_with_optimization(source).unwrap();

    // new expression is guaranteed non-nil
    assert!(
        !output.contains("~= nil"),
        "O2 should skip nil check for new expression"
    );
}

#[test]
#[ignore]
fn test_o2_skip_check_for_string_literal() {
    let source = r#"
        const result = "hello" ?? "world"
    "#;

    let output = compile_with_optimization(source).unwrap();

    // String literal is guaranteed non-nil
    assert!(
        !output.contains("~= nil"),
        "O2 should skip nil check for string literal"
    );
    assert!(
        output.contains("hello"),
        "Should just use the string literal"
    );
}

#[test]
#[ignore]
fn test_o2_skip_check_for_number_literal() {
    let source = r#"
        const result = 42 ?? 0
    "#;

    let output = compile_with_optimization(source).unwrap();

    // Number literal is guaranteed non-nil
    assert!(
        !output.contains("~= nil"),
        "O2 should skip nil check for number literal"
    );
}

#[test]
fn test_o2_does_not_skip_nil_literal() {
    let source = r#"
        const result = nil ?? 42
    "#;

    let output = compile_with_optimization(source).unwrap();

    // nil literal should still be checked
    assert!(
        output.contains("~= nil"),
        "O2 should NOT skip nil check for nil literal"
    );
}

#[test]
fn test_o2_preserves_check_for_variable() {
    let source = r#"
        const value: number | nil = 42
        const result = value ?? 0
    "#;

    let output = compile_with_optimization(source).unwrap();

    // Variables might be nil, should preserve check
    assert!(
        output.contains("~= nil"),
        "O2 should preserve nil check for variables"
    );
}

// ============================================================================
// Chained Null Coalescing
// ============================================================================

#[test]
fn test_chained_null_coalesce_with_mixed_complexity() {
    let source = r#"
        function getA(): number | nil { return nil }
        const b: number | nil = nil
        const result = getA() ?? b ?? 42
    "#;

    let output = compile_with_optimization(source).unwrap();

    // First ?? should use IIFE (function call)
    // Second ?? should use simple form (identifier)
    assert!(
        output.contains("function()"),
        "Should use IIFE for function call"
    );
    assert!(
        output.contains("~= nil and"),
        "Should also have simple form for identifier"
    );
}

#[test]
#[ignore]
fn test_o2_chained_with_guaranteed_non_nil() {
    let source = r#"
        const value: number | nil = nil
        const result = value ?? { default: true } ?? {}
    "#;

    let output = compile_with_optimization(source).unwrap();

    // Second ?? has object literal on left, should be optimized away by O2
    let _output_lines: Vec<&str> = output.lines().collect();

    // Should contain a nil check for value, but object literal should be used directly
    assert!(output.contains("~= nil"), "Should check value");
    assert!(output.contains("default"), "Should use object literal");
}
