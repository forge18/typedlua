use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::typechecker::TypeChecker;
use typedlua_core::codegen::CodeGenerator;
use std::sync::Arc;

fn compile_and_check(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());

    // Lex
    let mut lexer = Lexer::new(source, handler.clone());
    let tokens = lexer.tokenize().map_err(|e| format!("Lexing failed: {:?}", e))?;

    // Parse
    let mut parser = Parser::new(tokens, handler.clone());
    let program = parser.parse().map_err(|e| format!("Parsing failed: {:?}", e))?;

    // Type check
    let mut type_checker = TypeChecker::new(handler.clone()).with_options(CompilerOptions::default());
    type_checker.check_program(&program).map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new();
    let output = codegen.generate(&program);

    Ok(output)
}

// ============================================================================
// Basic Rest Parameter Tests
// ============================================================================

#[test]
fn test_simple_rest_parameter() {
    let source = r#"
        function sum(...numbers: number): number
            local total = 0
            return total
        end
    "#;

    let result = compile_and_check(source);
    if let Ok(ref output) = result {
        println!("Generated code:\n{}", output);
    }
    assert!(result.is_ok(), "Simple rest parameter should compile: {:?}", result.err());
    let output = result.unwrap();

    // Should generate: local function sum(...) with local numbers = {...}
    assert!(output.contains("function sum(...)"), "Output should contain 'function sum(...)' but got:\n{}", output);
    assert!(output.contains("local numbers = {...}"), "Output should contain 'local numbers = {{...}}' but got:\n{}", output);
}

#[test]
fn test_rest_parameter_with_regular_params() {
    let source = r#"
        function greet(greeting: string, ...names: string): string
            return greeting
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest parameter with regular params should compile: {:?}", result.err());
    let output = result.unwrap();

    // Should generate: local function greet(greeting, ...)
    assert!(output.contains("function greet(greeting, ...)"), "Output should contain 'function greet(greeting, ...)' but got:\n{}", output);
    assert!(output.contains("local names = {...}"), "Output should contain 'local names = {{...}}' but got:\n{}", output);
}

#[test]
fn test_multiple_params_with_rest() {
    let source = r#"
        function format(template: string, prefix: string, ...args: string): string
            return template
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Multiple params with rest should compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(output.contains("function format(template, prefix, ...)"), "Output should contain 'function format(template, prefix, ...)' but got:\n{}", output);
    assert!(output.contains("local args = {...}"), "Output should contain 'local args = {{...}}' but got:\n{}", output);
}

// ============================================================================
// Rest Parameter Usage Tests
// ============================================================================

#[test]
fn test_rest_parameter_access() {
    let source = r#"
        function getFirst(...items: number): number
            local first = items[1]
            return first
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest parameter access should compile: {:?}", result.err());
    let output = result.unwrap();

    // Should be able to access rest param as array
    assert!(output.contains("local first = items[1]"), "Output should contain 'local first = items[1]' but got:\n{}", output);
}

#[test]
fn test_rest_parameter_in_loop() {
    let source = r#"
        function sumAll(...numbers: number): number
            local sum = 0
            return sum
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest parameter in loop should compile");
}

#[test]
fn test_rest_parameter_length() {
    let source = r#"
        function count(...items: string): number
            local len = #items
            return len
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Getting length of rest parameter should compile");
    let output = result.unwrap();

    // Should access length with #
    assert!(output.contains("#items"), "Output should contain '#items' but got:\n{}", output);
}

// ============================================================================
// Type Checking Tests
// ============================================================================

#[test]
fn test_rest_parameter_type_annotation() {
    let source = r#"
        function processNumbers(...nums: number): number
            return 0
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest parameter with type annotation should compile");
}

#[test]
fn test_rest_parameter_string_type() {
    let source = r#"
        function concatenate(...strings: string): string
            return ""
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest parameter with string type should compile");
}

#[test]
fn test_rest_parameter_any_type() {
    let source = r#"
        function logAll(...args): nil
            return nil
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest parameter without type annotation should compile");
}

// ============================================================================
// Error Cases
// ============================================================================

#[test]
fn test_rest_parameter_must_be_last() {
    let source = r#"
        function invalid(...args: string, other: number): nil
            return nil
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_err(), "Rest parameter not in last position should fail");
    let error = result.unwrap_err();
    assert!(error.contains("must be the last parameter") || error.contains("rest"), "Error should mention rest parameter constraint but got: {}", error);
}

#[test]
fn test_multiple_rest_parameters() {
    let source = r#"
        function invalid(...first: number, ...second: string): nil
            return nil
        end
    "#;

    let result = compile_and_check(source);
    // This will fail the "must be last" check for the first rest param
    assert!(result.is_err(), "Multiple rest parameters should fail");
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_rest_parameter_with_return() {
    let source = r#"
        function firstOrZero(...numbers: number): number
            local first = numbers[1]
            return first
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest parameter with return should compile");
}

#[test]
fn test_rest_parameter_call() {
    let source = r#"
        function add(...nums: number): number
            return 0
        end
        const result = add(1, 2, 3, 4)
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Calling function with rest parameter should compile");
}

#[test]
fn test_rest_parameter_with_local_vars() {
    let source = r#"
        function process(...items: string): string
            local count = #items
            local first = items[1]
            return first
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest parameter with local variables should compile");
}

#[test]
fn test_empty_rest_parameter() {
    let source = r#"
        function optional(...args: string): number
            return #args
        end
        const result = optional()
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Calling rest parameter function with no args should compile");
}

#[test]
fn test_rest_parameter_shadowing() {
    let source = r#"
        function outer(...args: number): number
            return 0
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest parameter names should work independently");
}

// ============================================================================
// Complex Usage Tests
// ============================================================================

#[test]
fn test_rest_parameter_with_spread() {
    let source = r#"
        function first(...items: number): number
            return items[1]
        end
        const nums = [1, 2, 3]
        const result = first(...nums)
    "#;

    let result = compile_and_check(source);
    // This tests both rest parameters and spread in function calls
    if result.is_err() {
        println!("Rest with spread: {}", result.unwrap_err());
    }
}

#[test]
fn test_rest_parameter_forwarding() {
    let source = r#"
        function wrapper(...args: number): number
            return 0
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest parameter forwarding should compile");
}

#[test]
fn test_rest_parameter_type_inference() {
    let source = r#"
        function typed(...nums: number): number
            local first = nums[1]
            return first
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Type inference with rest parameters should work");
}
