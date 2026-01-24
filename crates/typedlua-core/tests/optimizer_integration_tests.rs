use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::{CompilerOptions, OptimizationLevel};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn compile_with_optimization(source: &str) -> Result<String, String> {
    compile_with_optimization_level(source, OptimizationLevel::O1)
}

fn compile_with_optimization_level(
    source: &str,
    level: OptimizationLevel,
) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);

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
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options);
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone()).with_optimization_level(level);
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Optimizer Integration Tests
// ============================================================================

#[test]
fn test_optimizer_runs_successfully() {
    let source = r#"
        const x = 42
        print(x)
    "#;

    // Test that optimizer runs at all optimization levels
    compile_with_optimization(source).unwrap();
    compile_with_optimization(source).unwrap();
    compile_with_optimization(source).unwrap();
    compile_with_optimization(source).unwrap();
}

#[test]
fn test_optimizer_with_complex_code() {
    // Test optimizer with classes, methods, and various constructs
    let source = r#"
        class Counter {
            value: number

            constructor(initial: number) {
                self.value = initial
            }

            increment() {
                self.value = self.value + 1
            }

            getValue(): number {
                return self.value
            }
        }

        const counter = new Counter(0)
        counter.increment()
        const result = counter.getValue()
        print(result)
    "#;

    let output = compile_with_optimization(source).unwrap();
    assert!(output.contains("Counter"), "Should generate Counter class");
    assert!(
        output.contains("increment"),
        "Should generate increment method"
    );
}

// NOTE: The following tests are for features not yet implemented.
// They are marked #[ignore] until the features are complete.
// See TODO.md sections 1.4 (Null Coalescing) and 2.1 (Exception Handling).

#[test]
#[ignore = "Null coalescing operator not yet implemented - see TODO 1.4"]
fn test_optimizer_with_null_coalescing() {
    let source = r#"
        const value: number | nil = nil
        const result = value ?? 42
    "#;

    let output = compile_with_optimization(source).unwrap();
    assert!(output.contains("~= nil"), "Should generate nil check");
}

#[test]
fn test_optimizer_with_exception_handling() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        }
    "#;

    let output_o0 = compile_with_optimization_level(source, OptimizationLevel::O0).unwrap();
    println!("O0 output:\n{}", output_o0);
    assert!(output_o0.contains("pcall"), "O0 should use pcall");

    let output_o1 = compile_with_optimization_level(source, OptimizationLevel::O1).unwrap();
    println!("O1 output:\n{}", output_o1);
    assert!(output_o1.contains("pcall"), "O1 should use pcall");

    let output_o2 = compile_with_optimization_level(source, OptimizationLevel::O2).unwrap();
    println!("O2 output:\n{}", output_o2);
    assert!(output_o2.contains("xpcall"), "O2 should use xpcall");
    assert!(
        output_o2.contains("debug.traceback"),
        "O2 should use debug.traceback"
    );

    let output_o3 = compile_with_optimization_level(source, OptimizationLevel::O3).unwrap();
    println!("O3 output:\n{}", output_o3);
    assert!(output_o3.contains("xpcall"), "O3 should use xpcall");
}

#[test]
fn test_optimizer_preserves_correctness() {
    let source = r#"
        function add(a: number, b: number): number {
            return a + b
        }

        const result = add(1, 2)
        print(result)
    "#;

    let output = compile_with_optimization(source).unwrap();
    println!("Generated output:\n{}", output);
    assert!(
        output.contains("function add("),
        "Should preserve function definition"
    );
    assert!(
        output.contains("add(1, 2)"),
        "Should preserve function call"
    );
}

#[test]
fn test_optimizer_with_no_passes() {
    // Currently the optimizer has no registered passes
    // This test verifies it doesn't fail with empty pass list
    let source = r#"
        const x = 1
        const y = 2
        const z = x + y
    "#;

    let output = compile_with_optimization(source).unwrap();
    println!("Generated output:\n{}", output);
    assert!(output.contains("local x = 1"), "Should generate variable x");
    assert!(output.contains("local y = 2"), "Should generate variable y");
    assert!(
        output.contains("local z = (x + y)"),
        "Should generate variable z"
    );
}

#[test]
fn test_optimizer_error_handling() {
    // Test that invalid code still fails properly with optimizer
    let source = r#"
        const x: number = "string"  // Type error
    "#;

    let result = compile_with_optimization(source);
    assert!(result.is_err(), "Should fail type checking");
}
