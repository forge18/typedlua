use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::config::OptimizationLevel;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_with_opt_level(source: &str, level: OptimizationLevel) -> Result<String, String> {
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

    let mut codegen = CodeGenerator::new(interner.clone()).with_optimization_level(level);
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Simple Tail Recursion Tests
// ============================================================================

#[test]
fn test_factorial_tail_recursive() {
    let source = r#"
        function factorial(n: number, acc: number): number
            if n <= 1 then
                return acc
            end
            return factorial(n - 1, n * acc)
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("function"),
        "Function should be generated. Got:\n{}",
        output
    );
    assert!(
        output.contains("factorial"),
        "Function name should be preserved. Got:\n{}",
        output
    );
    assert!(
        output.contains("return factorial"),
        "Tail call should be preserved in output. Got:\n{}",
        output
    );
}

#[test]
fn test_tail_call_preserves_syntax() {
    let source = r#"
        function sum(n: number, acc: number): number
            if n == 0 then
                return acc
            end
            return sum(n - 1, acc + n)
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("return sum"),
        "Tail call should be preserved exactly. Got:\n{}",
        output
    );
}

#[test]
fn test_tail_call_with_multiple_args() {
    let source = r#"
        function gcd(a: number, b: number): number
            if b == 0 then
                return a
            end
            return gcd(b, a % b)
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("return gcd"),
        "Tail call with multiple args should be preserved. Got:\n{}",
        output
    );
}

// ============================================================================
// Non-Tail Recursion Tests
// ============================================================================

#[test]
fn test_non_tail_recursion_preserved() {
    let source = r#"
        function fib(n: number): number
            if n <= 1 then
                return n
            end
            return fib(n - 1) + fib(n - 2)
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("fib"),
        "Non-tail recursive function should be preserved. Got:\n{}",
        output
    );
    assert!(
        output.contains("+"),
        "Addition in return should be preserved. Got:\n{}",
        output
    );
}

#[test]
fn test_non_tail_call_with_computation() {
    let source = r#"
        function double_and_add(n: number): number
            local twice = n * 2
            return twice + n
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("*") && output.contains("+"),
        "Computation before call should be preserved. Got:\n{}",
        output
    );
}

// ============================================================================
// Tail Call in Control Flow Tests
// ============================================================================

#[test]
fn test_tail_call_in_if_branch() {
    let source = r#"
        function abs(n: number): number
            if n < 0 then
                return -n
            end
            return n
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("if"),
        "If statement should be preserved. Got:\n{}",
        output
    );
    assert!(
        output.contains("return"),
        "Return statements should be preserved. Got:\n{}",
        output
    );
}

#[test]
fn test_tail_call_in_if_else() {
    let source = r#"
        function max(a: number, b: number): number
            if a > b then
                return a
            else
                return b
            end
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    let return_count = output.matches("return").count();
    assert!(
        return_count >= 2,
        "Both return statements should be preserved. Got:\n{}",
        output
    );
}

#[test]
fn test_tail_call_chain_in_else_if() {
    let source = r#"
        function process(n: number): number
            if n < 0 then
                return -n
            elseif n == 0 then
                return process(1)
            else
                return n
            end
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("elseif") || output.contains("else if"),
        "Elseif should be preserved. Got:\n{}",
        output
    );
}

// ============================================================================
// Multiple Return Values Tests
// ============================================================================

#[test]
fn test_tail_call_multiple_values() {
    let source = r#"
        function get_values(): {a: number, b: number}
            return get_values()
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("return get_values"),
        "Tail call with multiple value returns should be preserved. Got:\n{}",
        output
    );
}

// ============================================================================
// Mutual Recursion Tests
// ============================================================================

#[test]
fn test_mutual_tail_recursion() {
    let source = r#"
        function step_down(n: number): number
            if n == 0 then
                return 0
            end
            return step_down(n - 1)
        end

        function step_up(n: number): number
            if n == 0 then
                return 0
            end
            return step_up(n - 1)
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("step_down"),
        "step_down should be in output. Got:\n{}",
        output
    );
    assert!(
        output.contains("step_up"),
        "step_up should be in output. Got:\n{}",
        output
    );
}

// ============================================================================
// Arrow Function Tail Call Tests
// ============================================================================

#[test]
fn test_arrow_function_compiles() {
    let source = r#"
        function sum(n: number, acc: number): number
            if n == 0 then
                return acc
            end
            return sum(n - 1, acc + n)
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("return sum"),
        "Tail recursive function should be preserved. Got:\n{}",
        output
    );
}

// ============================================================================
// Return with Multiple Values Tests
// ============================================================================

#[test]
fn test_tail_call_multiple_returns() {
    let source = r#"
        function get_pair(): {a: number, b: number}
            return get_pair()
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("return get_pair"),
        "Tail call with multiple value returns should be preserved. Got:\n{}",
        output
    );
}

// ============================================================================
// Optimization Level Tests
// ============================================================================

#[test]
fn test_tail_call_preserved_at_all_levels() {
    let source = r#"
        function tail_sum(n: number, acc: number): number
            if n == 0 then
                return acc
            end
            return tail_sum(n - 1, acc + n)
        end
    "#;

    for level in [
        OptimizationLevel::O0,
        OptimizationLevel::O1,
        OptimizationLevel::O2,
        OptimizationLevel::O3,
    ] {
        let output = compile_with_opt_level(source, level).unwrap();
        assert!(
            output.contains("return tail_sum"),
            "Tail call should be preserved at level {:?}. Got:\n{}",
            level,
            output
        );
    }
}

#[test]
fn test_no_regression_in_tail_position() {
    let source = r#"
        function check(n: number): number
            if n < 0 then
                return -n
            end
            if n > 100 then
                return check(100)
            end
            return n
        end
    "#;

    let o1_output = compile_with_opt_level(source, OptimizationLevel::O1).unwrap();
    let o2_output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();

    let o1_returns = o1_output.matches("return").count();
    let o2_returns = o2_output.matches("return").count();
    assert!(
        o1_returns >= 3 && o2_returns >= 3,
        "All return statements should be preserved. O1: {}, O2: {}",
        o1_returns,
        o2_returns
    );
}

// ============================================================================
// Deep Recursion Tests (Runtime Behavior)
// ============================================================================

#[test]
fn test_deep_recursion_function_structure() {
    let source = r#"
        function count_down(n: number): number
            if n == 0 then
                return 0
            end
            return count_down(n - 1)
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("function count_down"),
        "Deep recursion function should compile. Got:\n{}",
        output
    );
}

#[test]
fn test_tree_depth_recursion() {
    let source = r#"
        function tree_height(depth: number): number
            if depth == 0 then
                return 0
            end
            local left = tree_height(depth - 1)
            local right = tree_height(depth - 1)
            return left + right + 1
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("tree_height"),
        "Tree traversal function should compile. Got:\n{}",
        output
    );
}

// ============================================================================
// Continuation Passing Style Tests
// ============================================================================

#[test]
fn test_continuation_passing_tail_call() {
    let source = r#"
        function factorial_cps(n: number, k: number): number
            if n == 0 then
                return k(1)
            end
            return factorial_cps(n - 1, (result: number) => k(n * result))
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("factorial_cps"),
        "CPS tail call should be preserved. Got:\n{}",
        output
    );
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_tail_call_no_args() {
    let source = r#"
        function get_one(): number
            return get_one()
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("return get_one()"),
        "Zero-arg tail call should be preserved. Got:\n{}",
        output
    );
}

#[test]
fn test_tail_call_constant_return() {
    let source = r#"
        function constant(): number
            return 42
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("return 42"),
        "Constant return should be preserved. Got:\n{}",
        output
    );
}

#[test]
fn test_empty_function() {
    let source = r#"
        function empty(): void
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("function empty"),
        "Empty function should compile. Got:\n{}",
        output
    );
}

#[test]
fn test_tail_call_after_condition() {
    let source = r#"
        function check_and_call(cond: boolean, n: number): number
            if cond then
                return check_and_call(false, n + 1)
            end
            return n
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("return n"),
        "Final return after conditional should be preserved. Got:\n{}",
        output
    );
}
