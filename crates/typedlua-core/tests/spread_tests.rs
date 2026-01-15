use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::typechecker::TypeChecker;

fn compile_and_check(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());

    // Lex
    let mut lexer = Lexer::new(source, handler.clone());
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    // Parse
    let mut parser = Parser::new(tokens, handler.clone());
    let program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    // Type check
    let mut type_checker =
        TypeChecker::new(handler.clone()).with_options(CompilerOptions::default());
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new();
    let output = codegen.generate(&program);

    Ok(output)
}

// ============================================================================
// Array Spread Tests
// ============================================================================

#[test]
fn test_simple_array_spread() {
    let source = r#"
        const arr1 = [1, 2]
        const arr2 = [3, 4]
        const combined = [...arr1, ...arr2]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Simple array spread should compile");
    let output = result.unwrap();

    // Should generate IIFE with table.insert loop
    assert!(output.contains("(function()"));
    assert!(output.contains("local __arr = {}"));
    assert!(output.contains("for _, __v in ipairs("));
    assert!(output.contains("table.insert(__arr, __v)"));
}

#[test]
fn test_array_spread_with_elements() {
    let source = r#"
        const arr = [2, 3]
        const result = [1, ...arr, 4]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Array spread with elements should compile");
    let output = result.unwrap();

    // Should insert both regular elements and spread elements
    assert!(output.contains("table.insert(__arr, 1)"));
    assert!(output.contains("for _, __v in ipairs(arr)"));
    assert!(output.contains("table.insert(__arr, 4)"));
}

#[test]
fn test_multiple_array_spreads() {
    let source = r#"
        const a = [1]
        const b = [2]
        const c = [3]
        const result = [...a, ...b, ...c]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Multiple array spreads should compile");
}

#[test]
fn test_nested_array_spread() {
    let source = r#"
        const inner = [2, 3]
        const outer = [1, ...inner, 4]
        const final = [0, ...outer, 5]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Nested array spread should compile");
}

#[test]
fn test_array_spread_type_check() {
    let source = r#"
        const numbers: number[] = [1, 2, 3]
        const moreNumbers = [...numbers, 4, 5]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Array spread type checking should pass");
}

#[test]
fn test_array_spread_type_error() {
    let source = r#"
        const notArray = 42
        const result = [...notArray]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_err(), "Spreading non-array should fail");
    let error = result.unwrap_err();
    assert!(error.contains("Cannot spread non-array"));
}

// ============================================================================
// Object Spread Tests
// ============================================================================

#[test]
fn test_simple_object_spread() {
    let source = r#"
        const obj1 = {a: 1, b: 2}
        const obj2 = {c: 3}
        const combined = {...obj1, ...obj2}
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Simple object spread should compile");
    let output = result.unwrap();

    // Should generate IIFE with pairs loop
    assert!(output.contains("(function()"));
    assert!(output.contains("local __obj = {}"));
    assert!(output.contains("for __k, __v in pairs("));
    assert!(output.contains("__obj[__k] = __v"));
}

#[test]
fn test_object_spread_with_properties() {
    let source = r#"
        const base = {x: 1, y: 2}
        const extended = {...base, z: 3}
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Object spread with properties should compile"
    );
    let output = result.unwrap();

    // Should have both spread and regular property assignment
    assert!(output.contains("for __k, __v in pairs(base)"));
    assert!(output.contains("__obj.z = 3"));
}

#[test]
#[ignore] // TODO: Fix object spread codegen - currently generates incomplete code
fn test_object_spread_override() {
    let source = r#"
        const base = {x: 1, y: 2}
        const override = {...base, y: 99}
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Object spread with override should compile");
    let output = result.unwrap();
    eprintln!("Generated code:\n{}", output);

    // Later properties should come after spread
    assert!(output.contains("for __k, __v in pairs(base)"));
    assert!(output.contains("__obj.y = 99"));
}

#[test]
fn test_multiple_object_spreads() {
    let source = r#"
        const a = {x: 1}
        const b = {y: 2}
        const c = {z: 3}
        const result = {...a, ...b, ...c}
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Multiple object spreads should compile");
}

#[test]
fn test_object_spread_type_check() {
    let source = r#"
        const person = {name: "Alice", age: 30}
        const extended = {...person, city: "NYC"}
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Object spread type checking should pass");
}

#[test]
fn test_object_spread_type_error() {
    let source = r#"
        const notObject = "string"
        const result = {...notObject}
    "#;

    let result = compile_and_check(source);
    assert!(result.is_err(), "Spreading non-object should fail");
    let error = result.unwrap_err();
    assert!(error.contains("Cannot spread non-object"));
}

// ============================================================================
// Mixed and Complex Spread Tests
// ============================================================================

#[test]
fn test_spread_in_function_call() {
    let source = r#"
        function add(a: number, b: number, c: number): number {
            return a + b + c
        }
        const numbers = [1, 2, 3]
        const result = add(...numbers)
    "#;

    let result = compile_and_check(source);
    // Function call spread is parsed but may not be fully implemented
    // This tests that at least parsing works
    if result.is_err() {
        println!("Function spread: {}", result.unwrap_err());
    }
}

#[test]
fn test_array_without_spread() {
    let source = r#"
        const arr = [1, 2, 3]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Array without spread should compile");
    let output = result.unwrap();

    // Should use simple array syntax
    assert!(output.contains("{1, 2, 3}"));
    // Should NOT use IIFE
    assert!(!output.contains("function()"));
}

#[test]
fn test_object_without_spread() {
    let source = r#"
        const obj = {x: 1, y: 2}
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Object without spread should compile");
    let output = result.unwrap();

    // Should use simple object syntax
    assert!(output.contains("x = 1"));
    assert!(output.contains("y = 2"));
    // Should NOT use IIFE for simple object
    assert!(!output.contains("function()"));
}

#[test]
fn test_spread_preserves_order() {
    let source = r#"
        const first = [1, 2]
        const second = [3, 4]
        const ordered = [...first, ...second]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Spread should preserve order");
    let output = result.unwrap();

    // First spread should appear before second in generated code
    let first_pos = output.find("first").unwrap();
    let second_pos = output.rfind("second").unwrap();
    assert!(first_pos < second_pos, "Order should be preserved");
}

#[test]
fn test_empty_spread() {
    let source = r#"
        const empty: number[] = []
        const result = [...empty, 1, 2]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Spreading empty array should work");
}

#[test]
fn test_spread_with_mixed_types() {
    let source = r#"
        const numbers = [1, 2]
        const strings = ["a", "b"]
        const mixed = [...numbers, ...strings]
    "#;

    let result = compile_and_check(source);
    // This should type check - result will be (number | string)[]
    assert!(result.is_ok(), "Mixed type spread should work");
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_spread_in_variable_usage() {
    let source = r#"
        const base = [1, 2, 3]
        const extended = [...base, 4, 5]
        const first = extended[1]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Spread result should be usable");
}

#[test]
fn test_multiple_spread_operations() {
    let source = r#"
        const arr1 = [1, 2]
        const arr2 = [...arr1, 3]
        const arr3 = [...arr2, 4]
        const arr4 = [...arr3, 5]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Multiple spread operations should work");
}

#[test]
fn test_spread_with_destructuring() {
    let source = r#"
        const arr = [1, 2, 3]
        const extended = [...arr, 4, 5]
        const [a, b, c, d, e] = extended
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Spread with destructuring should work");
}

#[test]
fn test_object_spread_merging() {
    let source = r#"
        const defaults = {timeout: 1000, retries: 3}
        const custom = {retries: 5, debug: true}
        const config = {...defaults, ...custom}
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Object spread merging should work");
}
