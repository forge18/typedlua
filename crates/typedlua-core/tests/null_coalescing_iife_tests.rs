use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::{CompilerOptions, OptimizationLevel};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_with_level(source: &str, level: OptimizationLevel) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let options = CompilerOptions::default();

    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options.clone());
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone()).with_optimization_level(level);
    let output = codegen.generate(&mut program);

    Ok(output)
}

/// Use O1 for pattern tests (no aggressive DCE)
fn compile_o1(source: &str) -> Result<String, String> {
    compile_with_level(source, OptimizationLevel::O1)
}

/// Use O2 for optimization-specific tests
fn compile_o2(source: &str) -> Result<String, String> {
    compile_with_level(source, OptimizationLevel::O2)
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

    let output = compile_o1(source).unwrap();

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

    let output = compile_o1(source).unwrap();

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

    let output = compile_o1(source).unwrap();

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

    let output = compile_o1(source).unwrap();

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

    let output = compile_o1(source).unwrap();

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

    let output = compile_o1(source).unwrap();

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
fn test_o2_skip_check_for_object_literal() {
    let source = r#"
        return { value: 42 } ?? { value: 0 }
    "#;

    let output = compile_o2(source).unwrap();

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
fn test_o2_skip_check_for_array_literal() {
    let source = r#"
        const result = [1, 2, 3] ?? []
    "#;

    let output = compile_o2(source).unwrap();

    // Array literal is guaranteed non-nil
    assert!(
        !output.contains("~= nil"),
        "O2 should skip nil check for array literal"
    );
}

#[test]
fn test_o2_skip_check_for_new_expression() {
    let source = r#"
        class MyClass {}
        const result = new MyClass() ?? nil
    "#;

    let output = compile_o2(source).unwrap();

    // new expression is guaranteed non-nil
    assert!(
        !output.contains("~= nil"),
        "O2 should skip nil check for new expression"
    );
}

#[test]
fn test_o2_skip_check_for_string_literal() {
    let source = r#"
        return "hello" ?? "world"
    "#;

    let output = compile_o2(source).unwrap();

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
fn test_o2_skip_check_for_number_literal() {
    let source = r#"
        const result = 42 ?? 0
    "#;

    let output = compile_o2(source).unwrap();

    // Number literal is guaranteed non-nil
    assert!(
        !output.contains("~= nil"),
        "O2 should skip nil check for number literal"
    );
}

#[test]
fn test_o2_does_not_skip_nil_literal() {
    let source = r#"
        return nil ?? 42
    "#;

    let output = compile_o2(source).unwrap();

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
        return value ?? 0
    "#;

    let output = compile_o2(source).unwrap();

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
        return getA() ?? b ?? 42
    "#;

    let output = compile_o1(source).unwrap();

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
fn test_o2_chained_with_guaranteed_non_nil() {
    let source = r#"
        const value: number | nil = nil
        return value ?? { default: true } ?? {}
    "#;

    let output = compile_o2(source).unwrap();

    // Second ?? has object literal on left, should be optimized away by O2
    // Should contain a nil check for value, but object literal should be used directly
    assert!(output.contains("~= nil"), "Should check value");
    assert!(output.contains("default"), "Should use object literal");
}
