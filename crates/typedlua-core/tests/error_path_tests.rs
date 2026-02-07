use bumpalo::Bump;
use std::sync::Arc;
use typedlua_core::{
    codegen::CodeGenerator,
    diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler, DiagnosticLevel},
    MutableProgram, TypeChecker,
};
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

#[test]
fn test_parser_missing_paren() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();
    let mut lexer = Lexer::new("const x = (1 + 2", handler.clone(), &interner);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let _ = parser.parse();
    assert!(handler.has_errors());
}

#[test]
fn test_parser_unexpected_token() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();
    let mut lexer = Lexer::new("const x = + 5", handler.clone(), &interner);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let _ = parser.parse();
    assert!(handler.has_errors());
}

#[test]
fn test_parser_incomplete_expression() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();
    let mut lexer = Lexer::new("const x = 1 +", handler.clone(), &interner);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let _ = parser.parse();
    assert!(handler.has_errors());
}

// Type checker error tests
#[test]
fn test_type_mismatch() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();
    let mut lexer = Lexer::new(r#"const x: number = "hello""#, handler.clone(), &interner);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let program = parser.parse().unwrap();

    let mut tc = TypeChecker::new(handler.clone(), &interner, &common_ids, &arena);
    let _ = tc.check_program(&program);
}

#[test]
fn test_undefined_variable() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();
    let mut lexer = Lexer::new("const x = undefined", handler.clone(), &interner);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let program = parser.parse().unwrap();

    let mut tc = TypeChecker::new(handler.clone(), &interner, &common_ids, &arena);
    let _ = tc.check_program(&program);
}

#[test]
fn test_return_type_mismatch() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();
    let input = "function test(): string\n    return 42\nend";
    let mut lexer = Lexer::new(input, handler.clone(), &interner);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let program = parser.parse().unwrap();

    let mut tc = TypeChecker::new(handler.clone(), &interner, &common_ids, &arena);
    let _ = tc.check_program(&program);
}

// Diagnostic handler tests
#[test]
fn test_diagnostic_handler_counts() {
    let handler = CollectingDiagnosticHandler::new();
    handler.error(typedlua_parser::span::Span::dummy(), "Error 1");
    handler.error(typedlua_parser::span::Span::dummy(), "Error 2");
    handler.warning(typedlua_parser::span::Span::dummy(), "Warning");

    assert_eq!(handler.error_count(), 2);
    assert_eq!(handler.warning_count(), 1);
    assert!(handler.has_errors());
}

#[test]
fn test_diagnostic_levels() {
    let handler = CollectingDiagnosticHandler::new();
    handler.error(typedlua_parser::span::Span::dummy(), "Error");
    handler.warning(typedlua_parser::span::Span::dummy(), "Warning");
    handler.info(typedlua_parser::span::Span::dummy(), "Info");

    let diags = handler.get_diagnostics();
    assert_eq!(diags[0].level, DiagnosticLevel::Error);
    assert_eq!(diags[1].level, DiagnosticLevel::Warning);
    assert_eq!(diags[2].level, DiagnosticLevel::Info);
}

// Code generator tests
#[test]
fn test_codegen_doesnt_panic() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();
    let mut lexer = Lexer::new("const x: number = 42", handler.clone(), &interner);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let program = parser.parse().unwrap();
    let mutable = MutableProgram::from_program(&program);

    let mut generator = CodeGenerator::new(interner.clone());
    let output = generator.generate(&mutable);
    assert!(!output.is_empty());
}

// Integration tests
#[test]
fn test_full_pipeline_with_errors() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();
    let input = "const x: number = \"wrong\"\nconst y = undefined";

    let mut lexer = Lexer::new(input, handler.clone(), &interner);
    if let Ok(tokens) = lexer.tokenize() {
        let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
        if let Ok(program) = parser.parse() {
            let mut tc = TypeChecker::new(handler.clone(), &interner, &common_ids, &arena);
            let _ = tc.check_program(&program);
        }
    }

    // Exercises full pipeline - the key is it doesn't panic
}
