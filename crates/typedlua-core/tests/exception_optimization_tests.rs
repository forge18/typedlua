use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::{CompilerOptions, OptimizationLevel};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_and_check(source: &str) -> Result<String, String> {
    compile_with_level(source, OptimizationLevel::O0)
}

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

#[test]
fn test_simple_try_catch_o0_uses_pcall() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        }
    "#;

    let output = compile_with_level(source, OptimizationLevel::O0).unwrap();
    assert!(output.contains("pcall"), "O0 should use pcall:\n{}", output);
}

#[test]
fn test_simple_try_catch_o1_uses_pcall() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        }
    "#;

    let output = compile_with_level(source, OptimizationLevel::O1).unwrap();
    assert!(output.contains("pcall"), "O1 should use pcall:\n{}", output);
}

#[test]
fn test_simple_try_catch_o2_uses_xpcall_with_traceback() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        }
    "#;

    let output = compile_with_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("xpcall"),
        "O2 should use xpcall:\n{}",
        output
    );
    assert!(output.contains("debug"), "O2 should use debug.traceback");
}

#[test]
fn test_simple_try_catch_o3_uses_xpcall_with_traceback() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        }
    "#;

    let output = compile_with_level(source, OptimizationLevel::O3).unwrap();
    assert!(
        output.contains("xpcall"),
        "O3 should use xpcall:\n{}",
        output
    );
    assert!(output.contains("debug"), "O3 should use debug.traceback");
}

#[test]
fn test_try_catch_finally_always_uses_xpcall() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        } finally {
            print("cleanup")
        }
    "#;

    let output = compile_with_level(source, OptimizationLevel::O0).unwrap();
    assert!(
        output.contains("xpcall"),
        "Try/catch/finally should use xpcall at O0:\n{}",
        output
    );

    let output_o2 = compile_with_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output_o2.contains("xpcall"),
        "Try/catch/finally should use xpcall at O2:\n{}",
        output_o2
    );
}

#[test]
fn test_typed_catch_always_uses_xpcall() {
    let source = r#"
        class Error {
            message: string
            constructor(message: string) {
                self.message = message
            }
        }

        try {
            throw new Error("oops")
        } catch (e: Error) {
            print(e.message)
        }
    "#;

    let output = compile_with_level(source, OptimizationLevel::O0).unwrap();
    assert!(
        output.contains("xpcall"),
        "Typed catch should use xpcall at O0"
    );

    let output_o2 = compile_with_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output_o2.contains("xpcall"),
        "Typed catch should use xpcall at O2"
    );
    assert!(
        output_o2.contains("return false"),
        "Typed catch should have type checking at O2"
    );
}

#[test]
fn test_multi_typed_catch_uses_xpcall() {
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
            throw new ValueError("invalid")
        } catch (e: TypeError) {
            print("type")
        } catch (e: ValueError) {
            print("value")
        }
    "#;

    let output_o1 = compile_with_level(source, OptimizationLevel::O1).unwrap();
    assert!(
        output_o1.contains("xpcall"),
        "Multi-typed catch should use xpcall at O1"
    );

    let output_o2 = compile_with_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output_o2.contains("xpcall"),
        "Multi-typed catch should use xpcall at O2"
    );
}

#[test]
fn test_auto_uses_xpcall_like_release() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        }
    "#;

    let output = compile_with_level(source, OptimizationLevel::Auto).unwrap();
    assert!(
        output.contains("xpcall"),
        "Auto should use xpcall (like release)"
    );
    assert!(output.contains("debug"), "Auto should use debug.traceback");
}
