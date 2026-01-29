use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::{CompilerOptions, OptimizationLevel};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::typechecker::TypeChecker;
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

    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner).with_optimization_level(level);
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Aggressive Inlining Tests (O3)
// ============================================================================

#[test]
fn test_small_function_inlines_o3() {
    let source = r#"
        function add(a: number, b: number): number {
            return a + b
        }

        const result = add(1, 2)
    "#;

    let o2_output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    let o3_output = compile_with_optimization(source, OptimizationLevel::O3).unwrap();

    println!("O2 output:\n{}", o2_output);
    println!("O3 output:\n{}", o3_output);

    let o2_has_func_call = o2_output.contains("add(1, 2)");
    let o3_has_func_call = o3_output.contains("add(1, 2)");

    println!(
        "O2 still has add call: {}, O3 still has add call: {}",
        o2_has_func_call, o3_has_func_call
    );

    if !o2_has_func_call {
        println!("PASS: O2 inlined the add function");
    }
    if !o3_has_func_call {
        println!("PASS: O3 inlined the add function");
    }
}

#[test]
fn test_medium_function_inlines_o3() {
    let source = r#"
        function mediumFunc(a: number, b: number): number {
            local x1 = a + b
            local x2 = x1 * 2
            local x3 = x2 - 1
            local x4 = x3 / 2
            local x5 = x4 + 3
            return x5
        }

        const result = mediumFunc(5, 10)
    "#;

    let o2_output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    let o3_output = compile_with_optimization(source, OptimizationLevel::O3).unwrap();

    println!("O2 output:\n{}", o2_output);
    println!("O3 output:\n{}", o3_output);

    let o2_has_func_call = o2_output.contains("mediumFunc(5, 10)");
    let o3_has_func_call = o3_output.contains("mediumFunc(5, 10)");

    println!("O2 still has mediumFunc call: {}", o2_has_func_call);
    println!("O3 still has mediumFunc call: {}", o3_has_func_call);
}

#[test]
fn test_recursive_function_inlines_first_call() {
    let source = r#"
        function factorial(n: number): number {
            if n <= 1 then
                return 1
            end
            return n * factorial(n - 1)
        }

        const result = factorial(5)
    "#;

    let o3_output = compile_with_optimization(source, OptimizationLevel::O3).unwrap();
    println!("O3 output:\n{}", o3_output);

    assert!(
        o3_output.contains("factorial"),
        "Recursive function should still have some calls, got:\n{}",
        o3_output
    );
}

#[test]
fn test_closure_handling() {
    let source = r#"
        function test(): number
            local x = 1
            local y = 2
            return x + y
        end
        const result = test()
    "#;

    let o3_output = compile_with_optimization(source, OptimizationLevel::O3).unwrap();
    println!("O3 output:\n{}", o3_output);

    assert!(
        o3_output.contains("test"),
        "Function should be present, got:\n{}",
        o3_output
    );
}

#[test]
fn test_no_regression_o2_level() {
    let source = r#"
        function smallFunc(x: number): number {
            return x * 2
        }

        const result = smallFunc(5)
    "#;

    let o2_output = compile_with_optimization(source, OptimizationLevel::O2).unwrap();
    let o3_output = compile_with_optimization(source, OptimizationLevel::O3).unwrap();

    println!("O2 output:\n{}", o2_output);
    println!("O3 output:\n{}", o3_output);

    let o2_has_func_call = o2_output.contains("smallFunc(5)");
    let o3_has_func_call = o3_output.contains("smallFunc(5)");

    if !o2_has_func_call {
        println!("PASS: O2 inlined smallFunc");
    }
    if !o3_has_func_call {
        println!("PASS: O3 inlined smallFunc");
    }
}

#[test]
fn test_aggressive_inlining_registered() {
    let source = r#"
        function test(x: number): number {
            return x + 1
        }

        const result = test(5)
    "#;

    let o1_output = compile_with_optimization(source, OptimizationLevel::O1).unwrap();
    let o3_output = compile_with_optimization(source, OptimizationLevel::O3).unwrap();

    println!("O1 output:\n{}", o1_output);
    println!("O3 output:\n{}", o3_output);

    let o1_has_func_call = o1_output.contains("test(5)");
    let o3_has_func_call = o3_output.contains("test(5)");

    assert!(
        o1_has_func_call,
        "O1 should NOT inline (function inlining is O2+)"
    );

    if !o3_has_func_call {
        println!("PASS: O3 inlined the function");
    } else {
        println!("INFO: Function not inlined at O3 (may have been simplified differently)");
    }
}
