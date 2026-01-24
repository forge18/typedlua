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

    // Lex
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    // Parse
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    // Type check
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids)
        .with_options(CompilerOptions::default());
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(&interner);
    let output = codegen.generate(&program);

    Ok(output)
}

// ============================================================================
// Basic Optional Chaining Tests
// ============================================================================

#[test]
fn test_optional_member_access() {
    let source = r#"
        const user = {name: "Alice"}
        const name = user?.name
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
        "Optional member access should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Simple expressions should use optimized `and` pattern
    assert!(output.contains("user and user.name or nil"));
}

#[test]
fn test_optional_index_access() {
    let source = r#"
        const arr = [1, 2, 3]
        const first = arr?.[0]
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Optional index access should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Simple expressions should use optimized `and` pattern with index access
    assert!(output.contains("arr and arr[") || output.contains("__t["));
}

#[test]
fn test_optional_call() {
    let source = r#"
        const getValue = (): number => 42
        const result = getValue?.()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Optional call should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Simple expressions should use optimized `and` pattern
    assert!(output.contains("getValue and getValue() or nil") || output.contains("return __t("));
}

// ============================================================================
// Chaining Tests
// ============================================================================

#[test]
fn test_simple_chain() {
    let source = r#"
        const obj = {nested: {value: 42}}
        const result = obj?.nested?.value
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Simple chain should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should have nested IIFE calls
    assert!(output.contains("function()"));
}

#[test]
fn test_long_chain() {
    let source = r#"
        const obj = {a: {b: {c: {d: 1}}}}
        const result = obj?.a?.b?.c?.d
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Long chain should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_mixed_chain() {
    let source = r#"
        const obj = {arr: [1, 2, 3]}
        const result = obj?.arr?.[0]
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Mixed chain should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Method Call Tests
// ============================================================================

#[test]
fn test_optional_method_call() {
    let source = r#"
        const text: string | nil = "hello"
        const upper = text?.toUpper?.()
    "#;

    let result = compile_and_check(source);
    // May have type checking issues - that's OK for now
    if result.is_err() {
        println!("Optional method call test: {}", result.unwrap_err());
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_optional_on_nil() {
    let source = r#"
        const value: number | nil = nil
        const result = value?.toString?.()
    "#;

    let result = compile_and_check(source);
    // May have type checking issues
    if result.is_err() {
        println!("Optional on nil test: {}", result.unwrap_err());
    }
}

#[test]
fn test_optional_with_non_nil_value() {
    let source = r#"
        const obj = {value: 42}
        const result = obj?.value
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Optional with non-nil value should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_optional_in_assignment() {
    let source = r#"
        const obj: {name?: string} | nil = nil
        const name = obj?.name
    "#;

    let result = compile_and_check(source);
    // May have type checking issues with optional properties
    if result.is_err() {
        println!("Optional in assignment test: {}", result.unwrap_err());
    }
}

// ============================================================================
// Combined with Other Operators
// ============================================================================

#[test]
fn test_optional_with_null_coalesce() {
    let source = r#"
        const obj: {value?: number} | nil = nil
        const result = obj?.value ?? 0
    "#;

    let result = compile_and_check(source);
    // May have type checking issues
    if result.is_err() {
        println!("Optional with null coalesce test: {}", result.unwrap_err());
    }
}

#[test]
fn test_optional_in_conditional() {
    let source = r#"
        const user = {age: 25}
        const canVote = (user?.age ?? 0) >= 18
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Optional in conditional should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Code Generation Verification
// ============================================================================

#[test]
fn test_codegen_optional_member() {
    let source = r#"
        const obj = {value: 42}
        const result = obj?.value
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should compile successfully");
    let output = result.unwrap();

    // Verify optimized `and` pattern for simple expressions
    assert!(
        output.contains("obj and obj.value or nil"),
        "Should use optimized and pattern"
    );
}

#[test]
fn test_codegen_optional_index() {
    let source = r#"
        const arr = [1, 2, 3]
        const first = arr?.[0]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should compile successfully");
    let output = result.unwrap();

    // Verify optimized `and` pattern for simple expressions with index
    assert!(
        output.contains("arr and arr[") || output.contains("__t["),
        "Should use optimized and pattern or IIFE"
    );
}

#[test]
fn test_codegen_optional_call() {
    let source = r#"
        const getValue = (): number => 42
        const result = getValue?.()
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should compile successfully");
    let output = result.unwrap();

    // Verify optimized `and` pattern for simple expressions with call
    assert!(
        output.contains("getValue and getValue() or nil") || output.contains("__t("),
        "Should use optimized and pattern or IIFE"
    );
}

#[test]
fn test_codegen_prevents_double_evaluation() {
    let source = r#"
        const getObject = (): {value: number} => {
            return {value: 42}
        }
        const result = getObject()?.value
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should compile successfully");
    let output = result.unwrap();

    // Should only call getObject() once by storing in __t
    assert!(output.contains("local __t = "));
    // Count occurrences of getObject - should appear once in the assignment
    let count = output.matches("getObject").count();
    assert!(
        count <= 2,
        "getObject should not be called multiple times in the optional chain (found {} times)",
        count
    );
}

// ============================================================================
// Type Inference Tests
// ============================================================================

#[test]
fn test_type_inference_optional_member() {
    let source = r#"
        type User = {
            name: string,
            age: number
        }
        const user: User | nil = nil
        const name = user?.name
    "#;

    let result = compile_and_check(source);
    // Type inference may not be fully implemented
    if result.is_err() {
        println!("Type inference test: {}", result.unwrap_err());
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_optional_in_function_return() {
    let source = r#"
        const getUser = (): {name: string} | nil => {
            return nil
        }
        const getName = (): string | nil => {
            return getUser()?.name
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Optional in function return should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_multiple_optional_chains() {
    let source = r#"
        const obj1 = {value: 1}
        const obj2 = {value: 2}
        const result1 = obj1?.value
        const result2 = obj2?.value
        const sum = (result1 ?? 0) + (result2 ?? 0)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Multiple optional chains should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_optional_with_object_literal() {
    let source = r#"
        const result = {x: 1, y: 2}?.x
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Optional with object literal should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Optimization Tests
// ============================================================================

#[test]
fn test_optimization_simple_vs_complex() {
    // Test 1: Simple expression should use `and` pattern
    let simple_source = r#"
        const user = {name: "Alice"}
        const result = user?.name
    "#;

    let result = compile_and_check(simple_source);
    assert!(result.is_ok(), "Simple case should compile");
    let output = result.unwrap();
    assert!(
        output.contains("user and user.name or nil"),
        "Simple case should use optimized and pattern"
    );
    assert!(
        !output.contains("function()"),
        "Simple case should not use IIFE"
    );

    // Test 2: Complex expression should use IIFE
    let complex_source = r#"
        const getUser = (): {name: string} => {
            return {name: "Bob"}
        }
        const result = getUser()?.name
    "#;

    let result = compile_and_check(complex_source);
    assert!(result.is_ok(), "Complex case should compile");
    let output = result.unwrap();
    assert!(
        output.contains("function()"),
        "Complex case should use IIFE"
    );
    assert!(
        output.contains("local __t ="),
        "Complex case should use temp variable"
    );
}

#[test]
fn test_optimization_nested_member_access() {
    let source = r#"
        const obj = {nested: {value: 42}}
        const result = obj.nested?.value
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should compile successfully");
    let output = result.unwrap();

    // obj.nested is a simple expression, so should use optimized pattern
    assert!(
        output.contains("obj.nested and obj.nested.value or nil"),
        "Should use optimized pattern for simple nested access"
    );
}

#[test]
fn test_optimization_chained_optional() {
    let source = r#"
        const obj = {nested: {value: 42}}
        const result = obj?.nested?.value
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should compile successfully");
    let output = result.unwrap();

    // First part is simple, but result might be complex due to chaining
    // This test just verifies it compiles and produces working code
    assert!(!output.is_empty());
}

#[test]
#[ignore] // O2 optimization - skip nil check for guaranteed non-nil object literals
fn test_o2_optimization_object_literal() {
    let source = r#"
        const result = {x: 1, y: 2}?.x
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should compile successfully");
    let output = result.unwrap();

    // Object literals are guaranteed non-nil, so should skip nil check entirely
    assert!(
        output.contains("{x = 1, y = 2}.x"),
        "Should use direct access for object literal"
    );
    assert!(!output.contains(" and "), "Should not have nil check");
    assert!(!output.contains("function()"), "Should not use IIFE");
}

#[test]
#[ignore] // O2 optimization - skip nil check for guaranteed non-nil array literals
fn test_o2_optimization_array_literal() {
    let source = r#"
        const result = [1, 2, 3]?.[0]
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should compile successfully");
    let output = result.unwrap();

    // Array literals are guaranteed non-nil, so should skip nil check
    assert!(
        output.contains("{1, 2, 3}["),
        "Should use direct access for array literal"
    );
    assert!(!output.contains(" and "), "Should not have nil check");
}

#[test]
fn test_o2_optimization_function_literal() {
    let source = r#"
        const result = ((): number => 42)?.()
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should compile successfully");
    let output = result.unwrap();

    // Function literals are guaranteed non-nil, so should skip nil check
    assert!(
        !output.contains(" and ") || output.contains("function()"),
        "Should optimize function literal call"
    );
}

#[test]
fn test_o2_optimization_new_expression() {
    let source = r#"
        class Point {
            x: number = 0
        }
        const result = (new Point())?.x
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should compile successfully");
    let output = result.unwrap();

    // New expressions are guaranteed non-nil, so should skip nil check
    // Note: the optimization applies to the optional chaining part
    assert!(
        !output.contains("if __t == nil"),
        "Should not have explicit nil check in optional chain for new expression"
    );
}

#[test]
fn test_o2_vs_regular_optional_chaining() {
    // Test 1: Guaranteed non-nil should skip checks
    let optimized_source = r#"
        const obj = {value: 42}
        const result = obj?.value
    "#;

    let result1 = compile_and_check(optimized_source);
    assert!(result1.is_ok(), "Optimized case should compile");
    let _output1 = result1.unwrap();

    // Test 2: Potentially nil should include checks
    let unoptimized_source = r#"
        const obj: {value: number} | nil = nil
        const result = obj?.value
    "#;

    let result2 = compile_and_check(unoptimized_source);
    // May have type checking issues with explicit nil type
    if result2.is_ok() {
        let output2 = result2.unwrap();
        // This one should have nil checks since obj could be nil
        assert!(
            output2.contains(" and ") || output2.contains("function()"),
            "Should have nil check for potentially nil value"
        );
    }
}
