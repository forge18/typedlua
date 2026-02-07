//! Simplified performance benchmarks

use bumpalo::Bump;
use std::sync::Arc;

use std::time::{Duration, Instant};
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::{MutableProgram, TypeChecker};
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

const MAX_TYPECHECK_100K_MS: u64 = 1000;
const MAX_FULL_COMPILE_100K_MS: u64 = 5000;

fn generate_simple_code(target_lines: usize) -> String {
    let mut code = String::new();
    let mut line_count = 0;

    code.push_str("// Performance test\n\n");
    line_count += 2;

    while line_count < target_lines {
        code.push_str(&format!("function f{}(x: number): number {{\n", line_count));
        code.push_str("    return x + 1\n");
        code.push_str("}\n\n");
        line_count += 4;
    }

    code
}

fn benchmark_typecheck(source: &str) -> Result<Duration, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let start = Instant::now();
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids, &arena);
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;
    Ok(start.elapsed())
}

fn benchmark_full_compile(source: &str) -> Result<Duration, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();

    let start = Instant::now();

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids, &arena);
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    let mutable = MutableProgram::from_program(&program);
    let mut codegen = CodeGenerator::new(interner.clone());
    let _output = codegen.generate(&mutable);

    Ok(start.elapsed())
}

#[test]
fn test_typecheck_100k_lines() {
    let source = generate_simple_code(100000);
    let line_count = source.lines().count();
    println!("Testing type checking with {} lines of code", line_count);

    let duration = benchmark_typecheck(&source).expect("Type checking should succeed");
    println!("Type checking 100K lines took: {:?}", duration);

    assert!(
        duration.as_millis() < MAX_TYPECHECK_100K_MS as u128,
        "Type checking 100K lines should complete within {}ms, took {:?}",
        MAX_TYPECHECK_100K_MS,
        duration
    );
}

#[test]
fn test_full_compile_100k_lines() {
    let source = generate_simple_code(100000);
    let line_count = source.lines().count();
    println!("Testing full compilation with {} lines of code", line_count);

    let duration = benchmark_full_compile(&source).expect("Compilation should succeed");
    println!("Full compilation 100K lines took: {:?}", duration);

    assert!(
        duration.as_millis() < MAX_FULL_COMPILE_100K_MS as u128,
        "Full compilation 100K lines should complete within {}ms, took {:?}",
        MAX_FULL_COMPILE_100K_MS,
        duration
    );
}
