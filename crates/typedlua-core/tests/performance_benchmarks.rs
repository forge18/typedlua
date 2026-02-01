//! Performance Benchmarks for TypedLua Compilation
//!
//! These benchmarks measure compilation performance at different scales:
//! - Type checking: 1K, 10K, 100K lines of code
//! - Full compilation (parse + typecheck + codegen)

use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

/// Maximum acceptable time for benchmarks (to catch performance regressions)
/// Set to ~5x observed debug-mode times for CI variance headroom
const MAX_TYPECHECK_1K_MS: u64 = 50;
const MAX_TYPECHECK_10K_MS: u64 = 100;
const MAX_TYPECHECK_100K_MS: u64 = 1000;
const MAX_FULL_COMPILE_1K_MS: u64 = 100;
const MAX_FULL_COMPILE_10K_MS: u64 = 500;
const MAX_FULL_COMPILE_100K_MS: u64 = 5000;

/// Generates TypedLua source code with approximately target_lines lines
fn generate_test_code(target_lines: usize) -> String {
    let mut code = String::new();
    let mut line_count = 0;

    // Add a module header
    code.push_str("// Auto-generated performance test code\n");
    code.push_str("// Target: ~");
    code.push_str(&target_lines.to_string());
    code.push_str(" lines\n\n");
    line_count += 3;

    // Generate interfaces (about 10% of lines)
    let interface_count = target_lines / 100;
    for i in 0..interface_count {
        code.push_str(&format!(
            "interface I{} {{\n    value: number\n    name: string\n}}\n\n",
            i
        ));
        line_count += 4;
    }

    // Generate classes with methods (about 20% of lines)
    let class_count = target_lines / 150;
    for i in 0..class_count {
        code.push_str(&format!("class Class{} {{\n", i));
        code.push_str("    private _value: number\n");
        code.push_str("    public name: string\n\n");
        code.push_str("    constructor(value: number, name: string) {\n");
        code.push_str("        self._value = value\n");
        code.push_str("        self.name = name\n");
        code.push_str("    }\n\n");
        code.push_str("    getValue(): number {\n");
        code.push_str("        return self._value\n");
        code.push_str("    }\n\n");
        code.push_str("    setValue(v: number): void {\n");
        code.push_str("        self._value = v\n");
        code.push_str("    }\n");
        code.push_str("}\n\n");
        line_count += 15;
    }

    // Generate functions (remaining lines)
    while line_count < target_lines {
        let func_index = line_count / 5;
        code.push_str(&format!(
            "function compute{}(a: number, b: number): number {{\n",
            func_index
        ));
        code.push_str("    const x = a + b\n");
        code.push_str("    const y = a * b\n");
        code.push_str("    const z = x - y\n");
        code.push_str("    return z\n");
        code.push_str("}\n\n");
        line_count += 6;
    }

    code
}

/// Measures type checking performance
fn benchmark_typecheck(source: &str) -> Result<Duration, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);

    // Lex and parse first (not timed)
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    // Time only type checking
    let start = Instant::now();
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;
    let duration = start.elapsed();

    Ok(duration)
}

/// Measures full compilation performance (parse + typecheck + codegen)
fn benchmark_full_compile(source: &str) -> Result<Duration, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);

    let start = Instant::now();

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
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    // Codegen
    let mut codegen = CodeGenerator::new(interner.clone());
    let _output = codegen.generate(&mut program);

    let duration = start.elapsed();

    Ok(duration)
}

#[test]
fn test_typecheck_1k_lines() {
    let source = generate_test_code(1000);
    let line_count = source.lines().count();
    println!("Testing type checking with {} lines of code", line_count);

    let duration = benchmark_typecheck(&source).expect("Type checking should succeed");
    println!("Type checking 1K lines took: {:?}", duration);

    assert!(
        duration.as_millis() < MAX_TYPECHECK_1K_MS as u128,
        "Type checking 1K lines should complete within {}ms, took {:?}",
        MAX_TYPECHECK_1K_MS,
        duration
    );
}

#[test]
fn test_typecheck_10k_lines() {
    let source = generate_test_code(10000);
    let line_count = source.lines().count();
    println!("Testing type checking with {} lines of code", line_count);

    let duration = benchmark_typecheck(&source).expect("Type checking should succeed");
    println!("Type checking 10K lines took: {:?}", duration);

    assert!(
        duration.as_millis() < MAX_TYPECHECK_10K_MS as u128,
        "Type checking 10K lines should complete within {}ms, took {:?}",
        MAX_TYPECHECK_10K_MS,
        duration
    );
}

#[test]
fn test_typecheck_100k_lines() {
    let source = generate_test_code(100000);
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
fn test_full_compile_1k_lines() {
    let source = generate_test_code(1000);
    let line_count = source.lines().count();
    println!("Testing full compilation with {} lines of code", line_count);

    let duration = benchmark_full_compile(&source).expect("Compilation should succeed");
    println!("Full compilation 1K lines took: {:?}", duration);

    assert!(
        duration.as_millis() < MAX_FULL_COMPILE_1K_MS as u128,
        "Full compilation 1K lines should complete within {}ms, took {:?}",
        MAX_FULL_COMPILE_1K_MS,
        duration
    );
}

#[test]
fn test_full_compile_10k_lines() {
    let source = generate_test_code(10000);
    let line_count = source.lines().count();
    println!("Testing full compilation with {} lines of code", line_count);

    let duration = benchmark_full_compile(&source).expect("Compilation should succeed");
    println!("Full compilation 10K lines took: {:?}", duration);

    assert!(
        duration.as_millis() < MAX_FULL_COMPILE_10K_MS as u128,
        "Full compilation 10K lines should complete within {}ms, took {:?}",
        MAX_FULL_COMPILE_10K_MS,
        duration
    );
}

#[test]
fn test_full_compile_100k_lines() {
    let source = generate_test_code(100000);
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

#[test]
fn test_typecheck_complex_generics() {
    let mut source = String::new();
    source.push_str("interface Container<T> {\n");
    source.push_str("    value: T\n");
    source.push_str("}\n\n");

    for i in 0..100 {
        source.push_str(&format!(
            "const c{}: Container<Container<Container<Container<number>>>> = {{\n",
            i
        ));
        source.push_str("    value: {\n");
        source.push_str("        value: {\n");
        source.push_str("            value: {\n");
        source.push_str("                value: 42\n");
        source.push_str("            }\n");
        source.push_str("        }\n");
        source.push_str("    }\n");
        source.push_str("}\n");
    }

    let duration = benchmark_typecheck(&source).expect("Type checking should succeed");
    println!("Type checking complex generics took: {:?}", duration);

    assert!(
        duration.as_millis() < 5000,
        "Complex generics should type check within 5s, took {:?}",
        duration
    );
}

#[test]
fn test_typecheck_deep_inheritance() {
    let mut source = String::new();

    // Base class with method
    source.push_str("class Base {\n");
    source.push_str("    value: number = 0\n");
    source.push_str("    getValue(): number { return self.value }\n");
    source.push_str("}\n\n");

    // 20 levels of inheritance - each level adds a method to avoid inheritance issues
    for i in 1..=20 {
        source.push_str(&format!("class Level{} extends Level{} {{\n", i, i - 1));
        source.push_str(&format!("    level{}Value: number = {}\n", i, i));
        // Add a method at each level to test method resolution performance
        source.push_str(&format!(
            "    getLevel{}Value(): number {{ return self.level{}Value }}\n",
            i, i
        ));
        source.push_str("}\n\n");
    }

    // Use the deepest class - call its own method, not inherited one
    source.push_str("const deep = new Level20()\n");
    source.push_str("const v = deep.getLevel20Value()\n");

    let duration = benchmark_typecheck(&source).expect("Type checking should succeed");
    println!(
        "Type checking deep inheritance (20 levels) took: {:?}",
        duration
    );

    assert!(
        duration.as_millis() < 2000,
        "Deep inheritance should type check within 2s, took {:?}",
        duration
    );
}

#[test]
fn test_typecheck_many_type_unions() {
    let mut source = String::new();

    for i in 0..500 {
        source.push_str(&format!("type T{} = number | string | boolean | null\n", i));
        source.push_str(&format!("const v{}: T{} = 42\n", i, i));
    }

    let duration = benchmark_typecheck(&source).expect("Type checking should succeed");
    println!("Type checking many unions (500) took: {:?}", duration);

    assert!(
        duration.as_millis() < 3000,
        "Many unions should type check within 3s, took {:?}",
        duration
    );
}
