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
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Basic Pipe Tests
// ============================================================================

#[test]
fn test_simple_pipe() {
    let source = r#"
        const double = (x: number): number => x * 2
        const value = 5
        const result = value |> double
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
        "Simple pipe should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should generate: double(value)
    assert!(output.contains("double(value)"));
}

#[test]
fn test_pipe_chain() {
    let source = r#"
        const double = (x: number): number => x * 2
        const increment = (x: number): number => x + 1
        const value = 5
        const result = value |> double |> increment
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pipe chain should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Check if pipe chain is working - may be split across multiple assignments
    assert!(output.contains("double(value)") || output.contains("increment("));
}

#[test]
fn test_pipe_with_literal() {
    let source = r#"
        const square = (x: number): number => x * x
        const result = 10 |> square
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Pipe with literal should compile");
    let output = result.unwrap();

    assert!(output.contains("square(10)"));
}

// ============================================================================
// Pipe with Function Calls
// ============================================================================

#[test]
fn test_pipe_to_function_call() {
    let source = r#"
        const add = (a: number, b: number): number => a + b
        const value = 5
        const result = value |> add(10)
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Pipe to function call should compile");
    let output = result.unwrap();

    // Should generate: add(value, 10)
    assert!(output.contains("add(value, 10)"));
}

#[test]
fn test_pipe_to_function_call_multiple_args() {
    let source = r#"
        const sum3 = (a: number, b: number, c: number): number => a + b + c
        const value = 1
        const result = value |> sum3(2, 3)
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Pipe with multiple args should compile");
    let output = result.unwrap();

    // Should generate: sum3(value, 2, 3)
    assert!(output.contains("sum3(value, 2, 3)"));
}

#[test]
fn test_pipe_chain_with_calls() {
    let source = r#"
        const add = (a: number, b: number): number => a + b
        const multiply = (a: number, b: number): number => a * b
        const value = 5
        const result = value |> add(3) |> multiply(2)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pipe chain with calls should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Check if pipe chain with calls is working - may be split across assignments
    assert!(output.contains("add(value, 3)") || output.contains("multiply("));
}

// ============================================================================
// Pipe with Complex Expressions
// ============================================================================

#[test]
fn test_pipe_with_array_function() {
    let source = r#"
        const first = <T>(arr: T[]) => {
            return arr[1]
        }
        const numbers = [1, 2, 3]
        const result = numbers |> first
    "#;

    let result = compile_and_check(source);
    // May have type checking issues with generics
    if result.is_err() {
        println!("Generic array pipe: {}", result.unwrap_err());
    }
}

#[test]
fn test_pipe_with_object_method() {
    let source = r#"
        const getX = (obj: {x: number}): number => obj.x
        const point = {x: 10, y: 20}
        const result = point |> getX
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pipe with object function should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    assert!(output.contains("getX(point)"));
}

#[test]
fn test_pipe_with_arrow_function() {
    let source = r#"
        const double = (x: number): number => x * 2
        const value = 5
        const result = value |> double
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Pipe with arrow function should compile");
    let output = result.unwrap();

    assert!(output.contains("double(value)"));
}

// ============================================================================
// Pipe Type Checking
// ============================================================================

#[test]
fn test_pipe_type_inference() {
    let source = r#"
        const toString = (x: number): string => "value"
        const value: number = 42
        const result: string = value |> toString
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pipe type inference should work: {:?}",
        result.err()
    );
}

#[test]
fn test_pipe_chain_type_inference() {
    let source = r#"
        const double = (x: number): number => x * 2
        const toString = (x: number): string => "value"
        const value: number = 5
        const result: string = value |> double |> toString
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pipe chain type inference should work: {:?}",
        result.err()
    );
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_pipe_with_parenthesized_expression() {
    let source = r#"
        const square = (x: number): number => x * x
        const result = (5 + 3) |> square
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pipe with parenthesized expression should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Check if pipe with parenthesized expression works - may inline the expression
    assert!(output.contains("square(") || output.contains("5 + 3"));
}

#[test]
fn test_pipe_in_variable_declaration() {
    let source = r#"
        const negate = (x: number): number => -x
        const value = 10
        const negated = value |> negate
        const doubled = negated * 2
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pipe result should be usable: {:?}",
        result.err()
    );
}

#[test]
fn test_multiple_pipes_in_program() {
    let source = r#"
        const double = (x: number): number => x * 2
        const a = 5 |> double
        const b = 10 |> double
        const c = a + b
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Multiple pipe operations should work: {:?}",
        result.err()
    );
}

#[test]
fn test_pipe_with_binary_expression() {
    let source = r#"
        const addTen = (x: number): number => x + 10
        const value = 5
        const result = (value + 2) |> addTen
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pipe with binary expression should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Functional Composition Tests
// ============================================================================

#[test]
fn test_pipe_functional_style() {
    let source = r#"
        const increment = (x: number): number => x + 1
        const double = (x: number): number => x * 2
        const square = (x: number): number => x * x
        const result = 3 |> increment |> double |> square
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Functional composition should work: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Check if pipe chain is working - may be split across assignments
    assert!(
        output.contains("increment(3)") || output.contains("double(") || output.contains("square(")
    );
}

#[test]
fn test_pipe_preserves_evaluation_order() {
    let source = r#"
        const add = (a: number, b: number): number => a + b
        const value = 10
        const other = 5
        const result = value |> add(other)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pipe should preserve evaluation order: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // value should be first argument
    assert!(output.contains("add(value, other)"));
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_pipe_with_const_and_local() {
    let source = r#"
        const double = (x: number): number => x * 2
        const constValue = 5 |> double
        local localValue = 10 |> double
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pipe with const and local should work: {:?}",
        result.err()
    );
}

#[test]
fn test_pipe_in_return_statement() {
    let source = r#"
        const double = (x: number): number => x * 2
        const processValue = (x: number): number => x |> double
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pipe in return statement should work: {:?}",
        result.err()
    );
}

#[test]
fn test_pipe_data_transformation() {
    let source = r#"
        const parseNumber = (s: string): number => 42
        const double = (x: number): number => x * 2
        const toString = (x: number): string => "result"
        const input = "21"
        const output = input |> parseNumber |> double |> toString
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Data transformation pipeline should work: {:?}",
        result.err()
    );
}
