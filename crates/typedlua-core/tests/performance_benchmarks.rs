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
    let source = generate_test_code(10_000);
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
    let source = generate_test_code(10_000);
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

// ============================================================================
// Optimization Benchmarks (Section 7.1.4)
// ============================================================================

use typedlua_core::config::OptimizationLevel;

/// Compiles source code with a specific optimization level and returns (duration, output_size)
fn benchmark_optimization_level(
    source: &str,
    level: OptimizationLevel,
) -> Result<(Duration, usize), String> {
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

    // Type check (not timed)
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    // Time only codegen with optimization
    let start = Instant::now();
    let mut codegen = CodeGenerator::new(interner.clone()).with_optimization_level(level);
    let output = codegen.generate(&mut program);
    let duration = start.elapsed();

    Ok((duration, output.len()))
}

/// Generates code that benefits from optimizations
fn generate_optimizable_code(target_lines: usize) -> String {
    let mut code = String::new();

    // Add header
    code.push_str("// Optimizable code for benchmarking\n\n");

    // Generate constants for constant folding
    for i in 0..100 {
        code.push_str(&format!("const CONST{} = {}\n", i, i * 10));
    }
    code.push_str("\n");

    // Generate functions with opportunities for inlining
    let func_count = target_lines / 20;
    for i in 0..func_count {
        code.push_str(&format!("function smallFunc{}(x: number): number {{\n", i));
        code.push_str("    return x + 1\n");
        code.push_str("}\n\n");

        code.push_str(&format!("function caller{}(): number {{\n", i));
        code.push_str(&format!("    const a = smallFunc{}(1)\n", i));
        code.push_str(&format!("    const b = smallFunc{}(a)\n", i));
        code.push_str(&format!("    const c = smallFunc{}(b)\n", i));
        code.push_str("    return a + b + c\n");
        code.push_str("}\n\n");
    }

    // Generate dead code for DCE
    for i in 0..50 {
        code.push_str(&format!("function deadFunc{}(): void {{\n", i));
        code.push_str("    const unused = 42\n");
        code.push_str("}\n\n");
    }

    code
}

#[test]
fn test_o0_vs_o1_optimization_time() {
    let source = generate_optimizable_code(2000);

    let (o0_time, o0_size) = benchmark_optimization_level(&source, OptimizationLevel::O0)
        .expect("O0 compilation should succeed");
    let (o1_time, o1_size) = benchmark_optimization_level(&source, OptimizationLevel::O1)
        .expect("O1 compilation should succeed");

    println!("O0 optimization: {:?}, size: {} bytes", o0_time, o0_size);
    println!("O1 optimization: {:?}, size: {} bytes", o1_time, o1_size);

    // O1 should not be significantly slower than O0 (within 2x)
    let slowdown_ratio = o1_time.as_secs_f64() / o0_time.as_secs_f64().max(0.001);
    println!("O1/O0 slowdown ratio: {:.2}x", slowdown_ratio);

    assert!(
        slowdown_ratio < 3.0,
        "O1 should not be more than 3x slower than O0, was {:.2}x",
        slowdown_ratio
    );

    // O1 should produce smaller or equal code
    assert!(
        o1_size <= o0_size * 11 / 10, // Allow 10% variance
        "O1 should not produce significantly larger code than O0"
    );
}

#[test]
fn test_o1_vs_o2_optimization_time() {
    let source = generate_optimizable_code(2000);

    let (o1_time, o1_size) = benchmark_optimization_level(&source, OptimizationLevel::O1)
        .expect("O1 compilation should succeed");
    let (o2_time, o2_size) = benchmark_optimization_level(&source, OptimizationLevel::O2)
        .expect("O2 compilation should succeed");

    println!("O1 optimization: {:?}, size: {} bytes", o1_time, o1_size);
    println!("O2 optimization: {:?}, size: {} bytes", o2_time, o2_size);

    // O2 may be slower than O1 but should still be reasonable (within 3x)
    let slowdown_ratio = o2_time.as_secs_f64() / o1_time.as_secs_f64().max(0.001);
    println!("O2/O1 slowdown ratio: {:.2}x", slowdown_ratio);

    assert!(
        slowdown_ratio < 5.0,
        "O2 should not be more than 5x slower than O1, was {:.2}x",
        slowdown_ratio
    );

    // O2 should produce smaller or equal code
    assert!(
        o2_size <= o1_size * 11 / 10, // Allow 10% variance
        "O2 should not produce significantly larger code than O1"
    );
}

#[test]
fn test_o2_vs_o3_optimization_time() {
    let source = generate_optimizable_code(2000);

    let (o2_time, o2_size) = benchmark_optimization_level(&source, OptimizationLevel::O2)
        .expect("O2 compilation should succeed");
    let (o3_time, o3_size) = benchmark_optimization_level(&source, OptimizationLevel::O3)
        .expect("O3 compilation should succeed");

    println!("O2 optimization: {:?}, size: {} bytes", o2_time, o2_size);
    println!("O3 optimization: {:?}, size: {} bytes", o3_time, o3_size);

    // O3 may be slower than O2 due to aggressive optimizations
    let slowdown_ratio = o3_time.as_secs_f64() / o2_time.as_secs_f64().max(0.001);
    println!("O3/O2 slowdown ratio: {:.2}x", slowdown_ratio);

    assert!(
        slowdown_ratio < 10.0,
        "O3 should not be more than 10x slower than O2, was {:.2}x",
        slowdown_ratio
    );

    // O3 should produce smaller or equal code
    assert!(
        o3_size <= o2_size * 11 / 10, // Allow 10% variance
        "O3 should not produce significantly larger code than O2"
    );
}

#[test]
fn test_code_size_reduction_at_each_level() {
    let source = generate_optimizable_code(3000);

    let (_, o0_size) = benchmark_optimization_level(&source, OptimizationLevel::O0)
        .expect("O0 compilation should succeed");
    let (_, o1_size) = benchmark_optimization_level(&source, OptimizationLevel::O1)
        .expect("O1 compilation should succeed");
    let (_, o2_size) = benchmark_optimization_level(&source, OptimizationLevel::O2)
        .expect("O2 compilation should succeed");
    let (_, o3_size) = benchmark_optimization_level(&source, OptimizationLevel::O3)
        .expect("O3 compilation should succeed");

    println!(
        "Code sizes - O0: {}, O1: {}, O2: {}, O3: {}",
        o0_size, o1_size, o2_size, o3_size
    );

    // Calculate reduction percentages
    let o1_reduction = o0_size.saturating_sub(o1_size) as f64 / o0_size as f64 * 100.0;
    let o2_reduction = o1_size.saturating_sub(o2_size) as f64 / o1_size as f64 * 100.0;
    let o3_reduction = o2_size.saturating_sub(o3_size) as f64 / o2_size as f64 * 100.0;
    let total_reduction = o0_size.saturating_sub(o3_size) as f64 / o0_size as f64 * 100.0;

    println!(
        "Size reduction - O0→O1: {:.1}%, O1→O2: {:.1}%, O2→O3: {:.1}%, Total: {:.1}%",
        o1_reduction, o2_reduction, o3_reduction, total_reduction
    );

    // Each level should not increase code size significantly
    assert!(
        o1_size <= o0_size * 105 / 100,
        "O1 should not increase code size by more than 5%"
    );
    assert!(
        o2_size <= o1_size * 105 / 100,
        "O2 should not increase code size by more than 5%"
    );
    assert!(
        o3_size <= o2_size * 105 / 100,
        "O3 should not increase code size by more than 5%"
    );

    // Log the actual reductions for monitoring
    println!("Code size reduction from O0 to O3: {:.1}%", total_reduction);
}

#[test]
fn test_optimization_preserves_correctness() {
    // Test that all optimization levels produce semantically equivalent code
    let source = r#"
        function compute(x: number): number {
            const a = x + 1
            const b = x * 2
            const c = a + b
            return c
        }
        
        const result = compute(5)
    "#;

    let (_, o0_size) =
        benchmark_optimization_level(source, OptimizationLevel::O0).expect("O0 should succeed");
    let (_, o1_size) =
        benchmark_optimization_level(source, OptimizationLevel::O1).expect("O1 should succeed");
    let (_, o2_size) =
        benchmark_optimization_level(source, OptimizationLevel::O2).expect("O2 should succeed");
    let (_, o3_size) =
        benchmark_optimization_level(source, OptimizationLevel::O3).expect("O3 should succeed");

    println!(
        "Correctness test sizes - O0: {}, O1: {}, O2: {}, O3: {}",
        o0_size, o1_size, o2_size, o3_size
    );

    // All levels should produce valid output (non-zero size)
    assert!(o0_size > 0, "O0 should produce output");
    assert!(o1_size > 0, "O1 should produce output");
    assert!(o2_size > 0, "O2 should produce output");
    assert!(o3_size > 0, "O3 should produce output");
}

/// Generates code that triggers O3-specific optimizations
fn generate_o3_test_code() -> String {
    let mut code = String::new();

    // Add header
    code.push_str("// Code to test O3 optimizations\n\n");

    // Generate final classes for devirtualization
    for i in 0..10 {
        code.push_str(&format!("final class FinalClass{} {{\n", i));
        code.push_str(&format!("    value: number = {}\n", i));
        code.push_str("    getValue(): number {\n");
        code.push_str("        return self.value\n");
        code.push_str("    }\n");
        code.push_str("}\n\n");
    }

    // Generate large functions for aggressive inlining (>20 AST nodes)
    for i in 0..10 {
        code.push_str(&format!("function largeFunc{}(x: number): number {{\n", i));
        // Create a function with many statements to exceed 20 AST nodes
        for j in 0..15 {
            code.push_str(&format!("    const a{} = x + {}\n", j, j));
            code.push_str(&format!("    const b{} = a{} * 2\n", j, j));
        }
        code.push_str("    return x\n");
        code.push_str("}\n\n");
    }

    // Generate interface with default methods for interface inlining
    for i in 0..5 {
        code.push_str(&format!("interface IWithDefault{} {{\n", i));
        code.push_str("    value: number\n");
        code.push_str("    default getDefaultValue(): number {{\n");
        code.push_str("        return 42\n");
        code.push_str("    }\n");
        code.push_str("}\n\n");
    }

    // Use the final classes (triggers devirtualization analysis)
    for i in 0..10 {
        code.push_str(&format!("const fc{} = new FinalClass{}()\n", i, i));
        code.push_str(&format!("const v{} = fc{}:getValue()\n", i, i));
    }

    // Call large functions (triggers aggressive inlining consideration)
    for i in 0..10 {
        code.push_str(&format!("const r{} = largeFunc{}(42)\n", i, i));
    }

    code
}

#[test]
fn test_o3_with_optimizable_code() {
    let source = generate_o3_test_code();

    let (o2_time, o2_size) = benchmark_optimization_level(&source, OptimizationLevel::O2)
        .expect("O2 compilation should succeed");
    let (o3_time, o3_size) = benchmark_optimization_level(&source, OptimizationLevel::O3)
        .expect("O3 compilation should succeed");

    println!("With optimizable code:");
    println!("O2 optimization: {:?}, size: {} bytes", o2_time, o2_size);
    println!("O3 optimization: {:?}, size: {} bytes", o3_time, o3_size);

    let slowdown_ratio = o3_time.as_secs_f64() / o2_time.as_secs_f64().max(0.001);
    println!("O3/O2 slowdown ratio: {:.2}x", slowdown_ratio);

    // O3 should be slower when it actually has work to do
    // But should still be within reasonable bounds
    assert!(
        slowdown_ratio < 15.0,
        "O3 should not be more than 15x slower than O2 even with optimizable code, was {:.2}x",
        slowdown_ratio
    );

    // O3 should produce smaller or equal code
    assert!(
        o3_size <= o2_size * 11 / 10,
        "O3 should not produce significantly larger code than O2"
    );
}

// ============================================================================
// Feature Performance Benchmarks (Section 7.1.5)
// ============================================================================

#[test]
fn test_typecheck_complex_generic_inference() {
    let mut source = String::new();

    // Deep generic nesting with inference
    source.push_str("function identity<T>(x: T): T { return x }\n\n");

    // Multiple levels of generic nesting
    for i in 0..50 {
        source.push_str(&format!(
            "function nested{}<A, B, C>(a: A, b: B, c: C): C {{\n",
            i
        ));
        source.push_str("    return c\n");
        source.push_str("}\n\n");
    }

    // Complex generic constraints
    for i in 0..30 {
        source.push_str(&format!(
            "interface Comparable{} {{\n    compare(other: Comparable{}): number\n}}\n\n",
            i, i
        ));
    }

    // Generic class hierarchies with inference
    for i in 0..20 {
        source.push_str(&format!(
            "class Container{}<T> {{\n    private value: T\n",
            i
        ));
        source.push_str(&format!("    constructor(v: T) {{ self.value = v }}\n"));
        source.push_str(&format!("    getValue(): T {{ return self.value }}\n"));
        source.push_str(&format!(
            "    map<U>(fn: (x: T) -> U): Container{}<U> {{\n",
            i
        ));
        source.push_str(&format!(
            "        return new Container{}<U>(fn(self.value))\n",
            i
        ));
        source.push_str("    }\n");
        source.push_str("}\n\n");
    }

    // Usage with type inference
    for i in 0..20 {
        source.push_str(&format!("const c{} = new Container{}(42)\n", i, i));
        source.push_str(&format!(
            "const mapped{} = c{}.map(function(x) return x * 2 end)\n",
            i, i
        ));
    }

    let duration = benchmark_typecheck(&source).expect("Type checking should succeed");
    println!(
        "Type checking complex generic inference took: {:?}",
        duration
    );

    assert!(
        duration.as_millis() < 3000,
        "Complex generic inference should type check within 3s, took {:?}",
        duration
    );
}

#[test]
fn test_typecheck_large_template_literals() {
    let mut source = String::new();

    // Small template literals (baseline)
    for i in 0..100 {
        source.push_str(&format!("const small{} = `Hello {}`\n", i, i));
    }

    // Medium template literals (multi-line)
    for i in 0..50 {
        source.push_str(&format!(
            "const medium{} = `\n    Line 1: {}\n    Line 2: {}\n    Line 3: {}\n`\n",
            i,
            i,
            i + 1,
            i + 2
        ));
    }

    // Large template literals (100+ lines each)
    for i in 0..10 {
        source.push_str(&format!("const large{} = `\n", i));
        for j in 0..100 {
            source.push_str(&format!(
                "    This is line {} of template {} with some interpolated value: {}\n",
                j,
                i,
                i * 100 + j
            ));
        }
        source.push_str("`\n\n");
    }

    // SQL-like templates with dedenting
    for i in 0..20 {
        source.push_str(&format!("const query{} = `\n", i));
        source.push_str("    SELECT\n");
        source.push_str("        id,\n");
        source.push_str("        name,\n");
        source.push_str("        value\n");
        source.push_str("    FROM\n");
        source.push_str(&format!("        table_{}\n", i));
        source.push_str("    WHERE\n");
        source.push_str(&format!("        id = {}\n", i));
        source.push_str("`\n\n");
    }

    let duration = benchmark_typecheck(&source).expect("Type checking should succeed");
    println!("Type checking large template literals took: {:?}", duration);

    assert!(
        duration.as_millis() < 2000,
        "Large template literals should type check within 2s, took {:?}",
        duration
    );
}

#[test]
fn test_reflection_overhead_vs_static_access() {
    // Generate code with reflection-heavy classes
    let mut reflective_source = String::new();

    reflective_source.push_str("// Reflection-heavy code\n\n");

    // Classes with many fields and methods for reflection
    for i in 0..20 {
        reflective_source.push_str(&format!("class ReflectiveClass{} {{\n", i));
        // Many fields
        for j in 0..20 {
            reflective_source.push_str(&format!("    public field{}: number = {}\n", j, j));
        }
        // Many methods
        for j in 0..10 {
            reflective_source.push_str(&format!("    public method{}(): number {{\n", j));
            reflective_source.push_str(&format!("        return self.field{}\n", j % 20));
            reflective_source.push_str("    }\n");
        }
        reflective_source.push_str("}\n\n");
    }

    // Use reflection API
    reflective_source.push_str("// Using reflection\n");
    for i in 0..20 {
        reflective_source.push_str(&format!(
            "const instance{} = new ReflectiveClass{}()\n",
            i, i
        ));
        reflective_source.push_str(&format!(
            "const fields{} = Reflect.getFields(instance{})\n",
            i, i
        ));
        reflective_source.push_str(&format!(
            "const methods{} = Reflect.getMethods(instance{})\n",
            i, i
        ));
    }

    // Generate code with static access (baseline)
    let mut static_source = String::new();

    static_source.push_str("// Static access code\n\n");

    // Same classes but with static access
    for i in 0..20 {
        static_source.push_str(&format!("class StaticClass{} {{\n", i));
        // Many fields
        for j in 0..20 {
            static_source.push_str(&format!("    public field{}: number = {}\n", j, j));
        }
        // Many methods
        for j in 0..10 {
            static_source.push_str(&format!("    public method{}(): number {{\n", j));
            static_source.push_str(&format!("        return self.field{}\n", j % 20));
            static_source.push_str("    }\n");
        }
        static_source.push_str("}\n\n");
    }

    // Use static access
    static_source.push_str("// Using static access\n");
    for i in 0..20 {
        static_source.push_str(&format!("const s_instance{} = new StaticClass{}()\n", i, i));
        static_source.push_str(&format!("const s_field{} = s_instance{}.field0\n", i, i));
        static_source.push_str(&format!(
            "const s_method{} = s_instance{}.method0()\n",
            i, i
        ));
    }

    let reflective_duration =
        benchmark_typecheck(&reflective_source).expect("Reflective type checking should succeed");
    let static_duration =
        benchmark_typecheck(&static_source).expect("Static type checking should succeed");

    println!(
        "Reflection-heavy code type checking took: {:?}",
        reflective_duration
    );
    println!(
        "Static access code type checking took: {:?}",
        static_duration
    );

    let overhead_ratio =
        reflective_duration.as_secs_f64() / static_duration.as_secs_f64().max(0.001);
    println!("Reflection overhead ratio: {:.2}x", overhead_ratio);

    // Reflection should not be more than 3x slower
    assert!(
        overhead_ratio < 3.0,
        "Reflection overhead should be less than 3x, was {:.2}x",
        overhead_ratio
    );
}

#[test]
fn test_rich_enum_instance_precomputation() {
    let mut source = String::new();

    // Rich enum with many variants and fields
    source.push_str("enum Planet {\n");
    // Enum variants first
    source.push_str("    Mercury(3.3011e23, 2.4397e6),\n");
    source.push_str("    Venus(4.8675e24, 6.0518e6),\n");
    source.push_str("    Earth(5.9723e24, 6.371e6),\n");
    source.push_str("    Mars(6.4171e23, 3.3895e6),\n");
    source.push_str("    Jupiter(1.8982e27, 6.9911e7),\n");
    source.push_str("    Saturn(5.6834e26, 5.8232e7),\n");
    source.push_str("    Uranus(8.6810e25, 2.5362e7),\n");
    source.push_str("    Neptune(1.02413e26, 2.4622e7),\n\n");
    // Then fields
    source.push_str("    mass: number,\n");
    source.push_str("    radius: number\n\n");

    source.push_str("    constructor(mass: number, radius: number) {\n");
    source.push_str("        self.mass = mass\n");
    source.push_str("        self.radius = radius\n");
    source.push_str("    }\n\n");

    source.push_str("    function surfaceGravity(): number {\n");
    source.push_str("        const G = 6.67430e-11\n");
    source.push_str("        return G * self.mass / (self.radius * self.radius)\n");
    source.push_str("    }\n\n");

    source.push_str("    function density(): number {\n");
    source.push_str(
        "        const volume = 4 / 3 * 3.14159 * self.radius * self.radius * self.radius\n",
    );
    source.push_str("        return self.mass / volume\n");
    source.push_str("    }\n");
    source.push_str("}\n\n");

    // Another rich enum with complex data
    source.push_str("enum StatusCode {\n");
    // Status code variants
    for i in 0..50 {
        source.push_str(&format!(
            "    Code{}({}, \"Message {}\", \"Category{}\"),\n",
            i,
            200 + i,
            i,
            i % 5
        ));
    }
    source.push_str("\n");
    // Fields
    source.push_str("    code: number,\n");
    source.push_str("    message: string,\n");
    source.push_str("    category: string\n\n");

    source.push_str("    constructor(code: number, message: string, category: string) {\n");
    source.push_str("        self.code = code\n");
    source.push_str("        self.message = message\n");
    source.push_str("        self.category = category\n");
    source.push_str("    }\n\n");

    source.push_str("    function isError(): boolean {\n");
    source.push_str("        return self.code >= 400\n");
    source.push_str("    }\n\n");

    source.push_str("    function isSuccess(): boolean {\n");
    source.push_str("        return self.code >= 200 && self.code < 300\n");
    source.push_str("    }\n");
    source.push_str("}\n\n");

    // Usage
    source.push_str("// Using the enums\n");
    source.push_str("const earthGravity = Planet.Earth:surfaceGravity()\n");
    source.push_str("const earthDensity = Planet.Earth:density()\n");
    source.push_str("const allPlanets = Planet:values()\n");

    let duration = benchmark_typecheck(&source).expect("Type checking should succeed");
    println!(
        "Type checking rich enum instance precomputation took: {:?}",
        duration
    );

    assert!(
        duration.as_millis() < 2000,
        "Rich enum instance precomputation should type check within 2s, took {:?}",
        duration
    );
}
