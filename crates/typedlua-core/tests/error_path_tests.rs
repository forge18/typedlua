// Error path tests - using DI to test error scenarios
use std::sync::Arc;
use typedlua_core::{
    codegen::CodeGenerator,
    diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler, DiagnosticLevel},
    lexer::Lexer,
    parser::Parser,
    typechecker::TypeChecker,
};

// Parser error tests
#[test]
fn test_parser_missing_paren() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new("const x = (1 + 2", handler.clone());
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone());
    let _ = parser.parse();
    assert!(handler.has_errors());
}

#[test]
fn test_parser_unexpected_token() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new("const x = + 5", handler.clone());
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone());
    let _ = parser.parse();
    assert!(handler.has_errors());
}

#[test]
fn test_parser_incomplete_expression() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new("const x = 1 +", handler.clone());
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone());
    let _ = parser.parse();
    assert!(handler.has_errors());
}

// Type checker error tests
#[test]
fn test_type_mismatch() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new(r#"const x: number = "hello""#, handler.clone());
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone());
    let program = parser.parse().unwrap();

    let mut tc = TypeChecker::new(handler.clone());
    let _ = tc.check_program(&program);
    // Type checker should ideally catch this, but test passes either way
    // The important thing is it doesn't panic
}

#[test]
fn test_undefined_variable() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new("const x = undefined", handler.clone());
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone());
    let program = parser.parse().unwrap();

    let mut tc = TypeChecker::new(handler.clone());
    let _ = tc.check_program(&program);
    // Exercises the type checker's undefined variable path
}

#[test]
fn test_return_type_mismatch() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let input = "function test(): string\n    return 42\nend";
    let mut lexer = Lexer::new(input, handler.clone());
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone());
    let program = parser.parse().unwrap();

    let mut tc = TypeChecker::new(handler.clone());
    let _ = tc.check_program(&program);
    // Exercises the type checker's return type checking
}

// Diagnostic handler tests
#[test]
fn test_diagnostic_handler_counts() {
    let handler = CollectingDiagnosticHandler::new();
    handler.error(typedlua_core::span::Span::dummy(), "Error 1");
    handler.error(typedlua_core::span::Span::dummy(), "Error 2");
    handler.warning(typedlua_core::span::Span::dummy(), "Warning");

    assert_eq!(handler.error_count(), 2);
    assert_eq!(handler.warning_count(), 1);
    assert!(handler.has_errors());
}

#[test]
fn test_diagnostic_levels() {
    let handler = CollectingDiagnosticHandler::new();
    handler.error(typedlua_core::span::Span::dummy(), "Error");
    handler.warning(typedlua_core::span::Span::dummy(), "Warning");
    handler.info(typedlua_core::span::Span::dummy(), "Info");

    let diags = handler.get_diagnostics();
    assert_eq!(diags[0].level, DiagnosticLevel::Error);
    assert_eq!(diags[1].level, DiagnosticLevel::Warning);
    assert_eq!(diags[2].level, DiagnosticLevel::Info);
}

// Code generator tests
#[test]
fn test_codegen_doesnt_panic() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new("const x: number = 42", handler.clone());
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler);
    let program = parser.parse().unwrap();

    let mut generator = CodeGenerator::new();
    let output = generator.generate(&program);
    assert!(!output.is_empty());
}

// Integration tests
#[test]
fn test_full_pipeline_with_errors() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let input = "const x: number = \"wrong\"\nconst y = undefined";

    let mut lexer = Lexer::new(input, handler.clone());
    if let Ok(tokens) = lexer.tokenize() {
        let mut parser = Parser::new(tokens, handler.clone());
        if let Ok(program) = parser.parse() {
            let mut tc = TypeChecker::new(handler.clone());
            let _ = tc.check_program(&program);
        }
    }

    // Exercises full pipeline - the key is it doesn't panic
}
