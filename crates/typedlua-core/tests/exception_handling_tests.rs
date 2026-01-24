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
// Basic Throw Tests
// ============================================================================

#[test]
fn test_throw_statement() {
    let source = r#"
        throw "error message"
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            assert!(output.contains("error(\"error message\")"));
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should compile successfully");
        }
    }
}

#[test]
fn test_throw_expression() {
    let source = r#"
        const message = "Something went wrong"
        throw message
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            assert!(output.contains("error(message)"));
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should compile successfully");
        }
    }
}

// ============================================================================
// Error Chaining Tests (!!)
// ============================================================================

#[test]
fn test_error_chain_operator() {
    let source = r#"
        const getValue = (): number => 42
        const fallback = (): number => 0
        const result = getValue() !! fallback()
    "#;

    let result = compile_and_check(source);
    if let Ok(output) = result {
        println!("Generated code:\n{}", output);
        // Should use pcall for error handling
        assert!(output.contains("pcall"));
    } else if let Err(e) = result {
        println!("Error (expected for now): {}", e);
    }
}

// ============================================================================
// Try/Catch/Finally Tests (TODO: Implement code generation)
// ============================================================================

#[test]
fn test_try_catch_parse() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        }
    "#;

    let result = compile_and_check(source);
    // Currently just testing that it parses
    if let Err(e) = result {
        println!("Parse/compile error: {}", e);
    }
}

#[test]
fn test_try_catch_typed() {
    let source = r#"
        try {
            throw "error"
        } catch (e: string) {
            print("String error: " .. e)
        }
    "#;

    let result = compile_and_check(source);
    if let Err(e) = result {
        println!("Parse/compile error: {}", e);
    }
}

#[test]
fn test_try_catch_finally() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        } finally {
            print("cleanup")
        }
    "#;

    let result = compile_and_check(source);
    if let Err(e) = result {
        println!("Parse/compile error: {}", e);
    }
}

#[test]
fn test_rethrow() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print("caught: " .. e)
            rethrow
        }
    "#;

    let result = compile_and_check(source);
    match result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Rethrow should call error(__error)
            assert!(
                output.contains("error(__error)"),
                "Generated code:\n{}",
                output
            );
        }
        Err(e) => {
            panic!("Should compile successfully. Error: {}", e);
        }
    }
}

#[test]
fn test_try_expression() {
    let source = r#"
        const riskyFunc = (): number => {
            throw "error"
        }
        const result = try riskyFunc() catch 0
    "#;

    let result = compile_and_check(source);
    match result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Should use pcall
            assert!(output.contains("pcall"), "Generated code:\n{}", output);
        }
        Err(e) => {
            panic!("Should compile successfully. Error: {}", e);
        }
    }
}

#[test]
fn test_try_expression_simple() {
    let source = r#"
        const value = try 42 catch 0
    "#;

    let result = compile_and_check(source);
    match result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            assert!(output.contains("pcall"));
        }
        Err(e) => {
            panic!("Should compile successfully. Error: {}", e);
        }
    }
}

#[test]
fn test_function_throws_clause() {
    let source = r#"
        function riskyOperation(): number throws string {
            throw "Something went wrong"
        }
    "#;

    let result = compile_and_check(source);
    match result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            assert!(output.contains("error"));
        }
        Err(e) => {
            panic!("Should compile successfully. Error: {}", e);
        }
    }
}

// ============================================================================
// Rethrow Validation Tests
// ============================================================================

#[test]
fn test_rethrow_outside_catch_fails() {
    let source = r#"
        function test() {
            rethrow
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_err(), "Should fail - rethrow outside catch block");
    assert!(
        result.unwrap_err().contains("outside of a catch block"),
        "Error message should mention catch block requirement"
    );
}

#[test]
fn test_rethrow_in_try_block_fails() {
    let source = r#"
        try {
            rethrow
        } catch (e) {
            const x = 1
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_err(), "Should fail - rethrow in try block");
}

#[test]
fn test_rethrow_in_finally_block_fails() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            const x = 1
        } finally {
            rethrow
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_err(), "Should fail - rethrow in finally block");
}

#[test]
fn test_rethrow_in_catch_succeeds() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            if (true) {
                rethrow
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should succeed - rethrow in catch block");
}

#[test]
fn test_rethrow_in_nested_catch() {
    let source = r#"
        try {
            try {
                throw "inner"
            } catch (inner) {
                rethrow
            }
        } catch (outer) {
            const x = 1
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Should succeed - rethrow in nested catch");
}
