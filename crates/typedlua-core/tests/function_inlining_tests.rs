use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::{CompilerOptions, OptimizationLevel};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_with_optimization(source: &str, level: OptimizationLevel) -> Result<String, String> {
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

    let mut type_checker = TypeChecker::new_with_stdlib(handler.clone(), &interner, &common_ids)
        .expect("Failed to load stdlib")
        .with_options(options);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner).with_optimization_level(level);
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Function Inlining Tests
// ============================================================================

#[test]
fn test_simple_function_inlining() {
    let source = r#"
        function add(a: number, b: number): number {
            return a + b
        }

        const x = add(1, 2)
        print(x)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    println!("Generated output (O2):\n{}", output);

    // After inlining, constant folding may further optimize (1 + 2) to 3
    assert!(
        output.contains("local x = (1 + 2)") || output.contains("local x = 3"),
        "Expected inlined expression 'local x = (1 + 2)' or folded 'local x = 3', got:\n{}",
        output
    );
}

#[test]
fn test_simple_function_inlining_o1() {
    let source = r#"
        function add(a: number, b: number): number {
            return a + b
        }

        const x = add(1, 2)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O1).unwrap();
    println!("Generated output (O1):\n{}", output);

    assert!(
        output.contains("add(1, 2)"),
        "O1 should not inline, expected function call 'add(1, 2)', got:\n{}",
        output
    );
}

#[test]
fn test_function_inlining_with_variables() {
    let source = r#"
        function double(n: number): number {
            return n * 2
        }

        const a = 5
        const b = double(a)
        const c = double(b)
        print(a, b, c)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    println!("Generated output:\n{}", output);

    // After inlining and constant folding, we expect the values to be computed
    // The exact output depends on how aggressively the optimizer works:
    // - Inlined: local b = (a * 2), local c = (b * 2)
    // - Partially folded: local b = 10, local c = 20
    // - Fully folded: print(5, 10, 20)
    let has_inlined_form = output.contains("(a * 2)") || output.contains("(5 * 2)");
    let has_folded_values = output.contains("local b = 10") || output.contains("local c = 20");
    let has_full_fold = output.contains("print(5, 10, 20)");

    assert!(
        has_inlined_form || has_folded_values || has_full_fold,
        "Expected inlined or folded output, got:\n{}",
        output
    );
}

#[test]
fn test_recursive_function_not_inlined() {
    let source = r#"
        function factorial(n: number): number {
            if n <= 1 then
                return 1
            end
            return n * factorial(n - 1)
        }

        const result = factorial(5)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    println!("Generated output:\n{}", output);

    assert!(
        output.contains("function factorial("),
        "Recursive function should NOT be inlined, expected function definition, got:\n{}",
        output
    );
    assert!(
        output.contains("factorial(")
            || output.contains("factorial (")
            || output.contains("factorial(n - 1)"),
        "Recursive call should remain, got:\n{}",
        output
    );
}

#[test]
fn test_function_with_multiple_statements_not_inlined() {
    let source = r#"
        function complex(a: number, b: number): number {
            const temp = a * b
            const temp2 = temp + 1
            const temp3 = temp2 * 2
            return temp3
        }

        const result = complex(3, 4)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    println!("Generated output:\n{}", output);

    assert!(
        output.contains("function complex("),
        "Function with 5+ statements should NOT be inlined, expected function definition, got:\n{}",
        output
    );
}

#[test]
fn test_function_with_side_effects_not_inlined() {
    // Functions with side effects (like print) should not be inlined
    let source = r#"
        function logAndAdd(a: number, b: number): number {
            print("Adding numbers")
            return a + b
        }

        const result = logAndAdd(1, 2)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    println!("Generated output:\n{}", output);

    // Function with side effects (print call) should NOT be inlined
    assert!(
        output.contains("function logAndAdd("),
        "Function with side effects should NOT be inlined, expected function definition, got:\n{}",
        output
    );
}

#[test]
fn test_inline_small_pure_function() {
    let source = r#"
        function identity(x: number): number {
            return x
        }

        const result = identity(42)
        print(result)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    println!("Generated output:\n{}", output);

    assert!(
        output.contains("local result = 42"),
        "Expected inlined identity function to 'local result = 42', got:\n{}",
        output
    );
}

#[test]
fn test_inlining_preserves_correctness() {
    let source = r#"
        function addThree(a: number, b: number, c: number): number {
            return a + b + c
        }

        const x = 10
        const y = 20
        const z = 30
        const result = addThree(x, y, z)
        print(result)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    println!("Generated output:\n{}", output);

    // After inlining and constant propagation, we should see either:
    // - The inlined expression: ((x + y) + z)
    // - Or the final folded value: 60
    let inlined = output.contains("((x + y) + z)") || output.contains("local result = 60");
    assert!(
        inlined,
        "Expected inlined expression '((x + y) + z)' or folded '60', got:\n{}",
        output
    );
}

#[test]
fn test_multiple_calls_same_function_inlined() {
    let source = r#"
        function square(n: number): number {
            return n * n
        }

        const a = square(2)
        const b = square(3)
        const c = square(4)
        print(a, b, c)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    println!("Generated output:\n{}", output);

    // After inlining and constant folding, we expect either the inlined form or folded constants
    let a_ok = output.contains("local a = (2 * 2)") || output.contains("local a = 4");
    let b_ok = output.contains("local b = (3 * 3)") || output.contains("local b = 9");
    let c_ok = output.contains("local c = (4 * 4)") || output.contains("local c = 16");

    assert!(
        a_ok,
        "Expected 'local a = (2 * 2)' or 'local a = 4', got:\n{}",
        output
    );
    assert!(
        b_ok,
        "Expected 'local b = (3 * 3)' or 'local b = 9', got:\n{}",
        output
    );
    assert!(
        c_ok,
        "Expected 'local c = (4 * 4)' or 'local c = 16', got:\n{}",
        output
    );
    // Note: Dead code elimination of unused functions is a separate optimization pass
    // The function definition may still be present after inlining
}

#[test]
fn test_function_inlining_with_nested_calls() {
    let source = r#"
        function incr(n: number): number {
            return n + 1
        }

        function double(n: number): number {
            return n * 2
        }

        const result = double(incr(5))
        print(result)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    println!("Generated output:\n{}", output);

    // With nested calls, inlining may happen at different depths.
    // Possible results:
    // - Partial inlining: outer inlined, inner call remains: (incr(5) * 2)
    // - Full inlining: ((5 + 1) * 2)
    // - Constant folded: 12
    let outer_inlined = output.contains("(incr(5) * 2)");
    let fully_inlined = output.contains("((5 + 1) * 2)") || output.contains("((5+1)*2)");
    let folded = output.contains("local result = 12");

    assert!(
        outer_inlined || fully_inlined || folded,
        "Expected some level of inlining: '(incr(5) * 2)', '((5 + 1) * 2)', or '12', got:\n{}",
        output
    );
}

#[test]
fn test_o3_includes_o2_inlining() {
    let source = r#"
        function add(a: number, b: number): number {
            return a + b
        }

        const x = add(1, 2)
        print(x)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O3).unwrap();
    println!("Generated output (O3):\n{}", output);

    // O3 includes O2 inlining plus additional optimizations like constant folding
    let inlined_or_folded = output.contains("local x = (1 + 2)") || output.contains("local x = 3");
    assert!(
        inlined_or_folded,
        "O3 should include O2 inlining, expected 'local x = (1 + 2)' or 'local x = 3', got:\n{}",
        output
    );
}
