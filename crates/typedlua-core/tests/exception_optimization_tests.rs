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

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let options = CompilerOptions::default();

    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options.clone());
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(&interner);
    let output = codegen.generate(&program);

    Ok(output)
}

#[test]
fn test_try_catch_uses_pcall_or_xpcall() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        }
    "#;

    let output = compile_and_check(source).unwrap();
    println!("Output:\n{}", output);

    assert!(
        output.contains("pcall") || output.contains("xpcall"),
        "Should use pcall or xpcall"
    );
}

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
    } else if let Err(e) = result {
        println!("Error (expected for now): {}", e);
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
            assert!(output.contains("pcall"), "Generated code:\n{}", output);
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

#[test]
fn test_multiple_try_catch_clauses() {
    let source = r#"
        class Error {
            message: string
            constructor(message: string) {
                self.message = message
            }
        }

        class TypeError extends Error {}
        class ValueError extends Error {}

        try {
            throw new ValueError("invalid value")
        } catch (e: TypeError) {
            print("Type error: " .. e.message)
        } catch (e: ValueError) {
            print("Value error: " .. e.message)
        } catch (e: Error) {
            print("Generic error: " .. e.message)
        }
    "#;

    let result = compile_and_check(source);
    match result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            assert!(output.contains("ValueError"));
        }
        Err(e) => {
            panic!("Should compile successfully. Error: {}", e);
        }
    }
}
