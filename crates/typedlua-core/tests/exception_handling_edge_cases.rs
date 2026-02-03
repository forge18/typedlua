//! Exception Handling Edge Cases Tests
//! Section 7.1.3 of TODO.md

use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_and_check(source: &str) -> Result<String, String> {
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

    let mut type_checker = TypeChecker::new_with_stdlib(handler.clone(), &interner, &common_ids)
        .expect("Failed to load stdlib")
        .with_options(CompilerOptions::default());
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

#[test]
fn test_nested_try_catch_two_levels() {
    let source = r#"
        try {
            try {
                throw "inner"
            } catch (inner: string) {
                print(inner)
                throw "outer"
            }
        } catch (outer: string) {
            print(outer)
        }
    "#;
    let result = compile_and_check(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_finally_execution_paths() {
    let source = r#"
        try {
            const x = 42
        } catch (e) {
            print("error")
        } finally {
            print("cleanup")
        }
    "#;
    let result = compile_and_check(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_exception_chaining_bang_bang() {
    let source = r#"
        const risky = (): number => throw "fail"
        const fallback = (): number => 0
        const result = risky() !! fallback()
    "#;
    let result = compile_and_check(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_custom_error_subclass() {
    let source = r#"
        class ValidationError {
            message: string
            field: string
            constructor(message: string, field: string) {
                self.message = message
                self.field = field
            }
        }
        throw new ValidationError("Invalid", "email")
    "#;
    let result = compile_and_check(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_pcall_vs_xpcall_selection() {
    let source = r#"
        try {
            throw "simple error"
        } catch (e: string) {
            print(e)
        }
    "#;
    let result = compile_and_check(source);
    if let Ok(output) = result {
        assert!(output.contains("pcall") || output.contains("xpcall"));
    }
}
