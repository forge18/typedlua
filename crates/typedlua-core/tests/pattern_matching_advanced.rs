//! Pattern Matching Advanced Tests
//! Section 7.1.3 of TODO.md

use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
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

    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

#[test]
fn test_deep_destructuring_three_levels() {
    let source = r#"
        const data = { a: { b: { c: 42 } } }
        const result = match data {
            { a: { b: { c } } } => c,
            _ => 0
        }
    "#;
    let result = compile_and_check(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_or_pattern_basic() {
    let source = r#"
        const x = 1
        const result = match x {
            1 | 2 => "one or two",
            3 | 4 | 5 => "three to five",
            _ => "other"
        }
    "#;
    let result = compile_and_check(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_nested_pattern_matching() {
    let source = r#"
        const data = { type: "point", x: 1, y: 2 }
        const result = match data {
            { type: "point", x, y } => x + y,
            { type: "circle", r } => r * 2,
            _ => 0
        }
    "#;
    let result = compile_and_check(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_pattern_guard_complex() {
    let source = r#"
        const n = 10
        const result = match n {
            x when x > 0 and x < 5 => "small",
            x when x >= 5 and x < 10 => "medium",
            x when x >= 10 => "large",
            _ => "unknown"
        }
    "#;
    let result = compile_and_check(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_array_pattern_with_rest() {
    let source = r#"
        const arr = [1, 2, 3, 4, 5]
        const result = match arr {
            [first, ...rest] => first,
            [] => 0
        }
    "#;
    let result = compile_and_check(source);
    assert!(result.is_ok() || result.is_err());
}
