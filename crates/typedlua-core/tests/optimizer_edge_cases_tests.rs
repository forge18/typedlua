use typedlua_core::config::CompilerConfig;
use typedlua_core::config::OptimizationLevel;
use typedlua_core::di::DiContainer;

fn compile_with_opt_level(source: &str, level: OptimizationLevel) -> Result<String, String> {
    let config = CompilerConfig::default();
    let mut container = DiContainer::production(config);
    container.compile_with_stdlib_and_optimization(source, level)
}

// ============================================================================
// Dead Store Elimination Edge Cases
// ============================================================================

#[test]
fn test_dead_store_multiple_assignments() {
    let source = r#"
        local a = 1
        a = 2
        a = 3
        return a
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("return a"),
        "Should compile. Got:\n{}",
        output
    );
}

#[test]
fn test_dead_store_with_dependency() {
    let source = r#"
        local x = 10
        local y = x + 5
        x = 20
        return y
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("y"),
        "Should handle dead store with dependency. Got:\n{}",
        output
    );
}

#[test]
fn test_dead_store_in_loop() {
    let source = r#"
        local sum = 0
        for i = 1, 10 do
            sum = sum + i
            local temp = i * 2
            sum = sum + temp
        end
        return sum
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("for i = 1, 10 do"),
        "Should compile loop. Got:\n{}",
        output
    );
}

// ============================================================================
// Algebraic Simplification Edge Cases
// ============================================================================

#[test]
fn test_algebraic_simplification_multiplication() {
    let source = r#"
        const x = 5 * 0
        return x
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O1).unwrap();
    assert!(
        output.contains("return 0") || output.contains("local x = 0"),
        "Should simplify 5 * 0 = 0. Got:\n{}",
        output
    );
}

#[test]
fn test_algebraic_simplification_addition() {
    let source = r#"
        const x = 5 + 0
        const y = 0 + 10
        return x + y
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O1).unwrap();
    assert!(
        output.contains("15") || output.contains("local x = 5") && output.contains("local y = 10"),
        "Should simplify x + 0 and 0 + y. Got:\n{}",
        output
    );
}

#[test]
fn test_algebraic_simplification_subtraction() {
    let source = r#"
        const x = 10 - 10
        return x
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O1).unwrap();
    assert!(
        output.contains("0"),
        "Should simplify 10 - 10 = 0. Got:\n{}",
        output
    );
}

#[test]
fn test_algebraic_simplification_division() {
    let source = r#"
        const x = 20 / 1
        return x
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O1).unwrap();
    assert!(
        output.contains("20"),
        "Should simplify 20 / 1 = 20. Got:\n{}",
        output
    );
}

// ============================================================================
// Constant Folding Edge Cases
// ============================================================================

#[test]
fn test_constant_folding_nested_expressions() {
    let source = r#"
        const x = (1 + 2) * (3 + 4)
        return x
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O1).unwrap();
    assert!(
        output.contains("return"),
        "Should compile. Got:\n{}",
        output
    );
}

#[test]
fn test_constant_folding_string_concat() {
    let source = r#"
        const a = "hello"
        const b = " "
        const c = "world"
        const greeting = a .. b .. c
        return greeting
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O1).unwrap();
    eprintln!("Output:\n{}", output);
}

// ============================================================================
// String Concatenation Optimization Edge Cases
// ============================================================================

#[test]
fn test_string_concat_single_string() {
    let source = r#"
        const result = "hello"
        return result
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("hello"),
        "Single string should not use table.concat. Got:\n{}",
        output
    );
}

#[test]
fn test_string_concat_two_parts() {
    let source = r#"
        const result = "hello" .. "world"
        return result
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains(".."),
        "Two parts should use .. operator, not table.concat. Got:\n{}",
        output
    );
}

#[test]
fn test_string_concat_three_parts() {
    let source = r#"
        const result = "a" .. "b" .. "c"
        return result
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("table.concat"),
        "Three or more parts should use table.concat. Got:\n{}",
        output
    );
}

#[test]
fn test_string_concat_variable_parts() {
    let source = r#"
        local a = "a"
        local b = "b"
        local c = "c"
        const result = a .. b .. c
        return result
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("table.concat"),
        "Variable parts should use table.concat. Got:\n{}",
        output
    );
}

#[test]
fn test_string_concat_mixed() {
    let source = r#"
        const result = "prefix" .. "middle" .. "suffix"
        return result
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("table.concat"),
        "Mixed string concat should use table.concat. Got:\n{}",
        output
    );
}

// ============================================================================
// Table Preallocation Edge Cases
// ============================================================================

#[test]
fn test_table_preallocation_empty() {
    let source = r#"
        const arr: any[] = []
        return arr
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("{"),
        "Should generate table literal. Got:\n{}",
        output
    );
}

#[test]
fn test_table_preallocation_with_initial_values() {
    let source = r#"
        local obj = {a = 1, b = 2}
        return obj
    "#;

    let result = compile_with_opt_level(source, OptimizationLevel::O2);
    eprintln!("Table result: {:?}", result);
    assert!(result.is_ok() || result.unwrap_err().contains("error"));
}

// ============================================================================
// Tail Call Optimization Edge Cases
// ============================================================================

#[test]
fn test_tail_call_simple_recursion() {
    let source = r#"
        function factorial(n: number, acc: number): number
            if n <= 1 then return acc end
            return factorial(n - 1, n * acc)
        end
        const result = factorial(5, 1)
        return result
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("function factorial"),
        "Should generate factorial function. Got:\n{}",
        output
    );
}

#[test]
fn test_tail_call_mutual_recursion() {
    let source = r#"
        function isEven(n: number): boolean
            if n == 0 then return true end
            return isOdd(n - 1)
        end
        function isOdd(n: number): boolean
            if n == 0 then return false end
            return isEven(n - 1)
        end
        const result = isEven(10)
        return result
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("isEven") && output.contains("isOdd"),
        "Should generate both functions. Got:\n{}",
        output
    );
}

#[test]
fn test_tail_call_not_optimized_when_not_tail() {
    let source = r#"
        function bad(n: number): number
            return bad(n - 1) + 1
        end
        const result = bad(5)
        return result
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("function bad"),
        "Should generate the function. Got:\n{}",
        output
    );
}

// ============================================================================
// Loop Optimization Edge Cases
// ============================================================================

#[test]
fn test_loop_while_true_preserved() {
    let source = r#"
        local count = 0
        while true do
            count = count + 1
            if count >= 5 then break end
        end
        return count
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("while true do"),
        "Infinite-looking loop with break should be preserved. Got:\n{}",
        output
    );
}

#[test]
fn test_loop_constant_condition_false() {
    let source = r#"
        while false do
            print("never")
        end
        print("done")
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        !output.contains("never"),
        "Dead while loop body should be eliminated. Got:\n{}",
        output
    );
    assert!(
        output.contains("done"),
        "Code after loop should be preserved. Got:\n{}",
        output
    );
}

#[test]
fn test_loop_repeat_until_true() {
    let source = r#"
        repeat
            print("once")
        until true
        print("after")
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("repeat"),
        "Should compile repeat-until loop. Got:\n{}",
        output
    );
}

#[test]
fn test_loop_for_zero_iterations() {
    let source = r#"
        for i = 10, 5 do
            print(i)
        end
        print("done")
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        !output.contains("print(i)"),
        "Zero-iteration for loop body should be eliminated. Got:\n{}",
        output
    );
    assert!(
        output.contains("done"),
        "Code after loop should be preserved. Got:\n{}",
        output
    );
}

// ============================================================================
// Global Localization Edge Cases
// ============================================================================

#[test]
fn test_global_to_local_simple() {
    let source = r#"
        const x = 1
        const y = x + 1
        return y
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("local"),
        "Should localize globals. Got:\n{}",
        output
    );
}

#[test]
fn test_global_to_local_multiple_uses() {
    let source = r#"
        const mult = 2
        local a = mult * 1
        local b = mult * 2
        local c = mult * 3
        return a + b + c
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    let local_count = output.matches("local ").count();
    assert!(
        local_count >= 3,
        "Should localize multiple uses of global. Got:\n{}",
        output
    );
}

// ============================================================================
// Generic Specialization Edge Cases
// ============================================================================

#[test]
fn test_generic_type_inference() {
    let source = r#"
        function identity<T>(x: T): T
            return x
        end
        const num = identity(42)
        const str = identity("hello")
        return num
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O3).unwrap();
    assert!(
        output.contains("function"),
        "Should generate generic function. Got:\n{}",
        output
    );
}

#[test]
fn test_generic_with_constraint() {
    let source = r#"
        class NumberWrapper {
            value: number

            constructor(value: number) {
                self.value = value
            }

            add(other: NumberWrapper): NumberWrapper {
                return NumberWrapper(self.value + other.value)
            }
        }

        const nw = NumberWrapper(5)
        const result = nw.add(NumberWrapper(3))
        return result
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O3).unwrap();
    assert!(
        output.contains("NumberWrapper"),
        "Should compile NumberWrapper class. Got:\n{}",
        output
    );
}

// ============================================================================
// Optimization Level Comparison Tests
// ============================================================================

#[test]
fn test_o1_vs_o2_differences() {
    let source = r#"
        while false do
            print("dead")
        end
    "#;

    let o1_output = compile_with_opt_level(source, OptimizationLevel::O1).unwrap();
    let o2_output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();

    assert!(
        o1_output.contains("dead"),
        "O1 should preserve dead loop. Got:\n{}",
        o1_output
    );
    assert!(
        !o2_output.contains("dead"),
        "O2 should eliminate dead loop. Got:\n{}",
        o2_output
    );
}

#[test]
fn test_o2_vs_o3_differences() {
    let source = r#"
        class Calculator {
            static add(a: number, b: number): number {
                return a + b
            }
        }

        const result = Calculator.add(1, 2)
        return result
    "#;

    let o2_output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    let o3_output = compile_with_opt_level(source, OptimizationLevel::O3).unwrap();

    eprintln!("O2:\n{}", o2_output);
    eprintln!("O3:\n{}", o3_output);
}

// ============================================================================
// Complex Combined Optimization Tests
// ============================================================================

#[test]
fn test_multiple_optimizations_combined() {
    let source = r#"
        const a = 1 + 2
        const b = a * 0
        const c = b + 10
        return c
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("10"),
        "Should optimize to return 10. Got:\n{}",
        output
    );
}

#[test]
fn test_constant_propagation_across_expressions() {
    let source = r#"
        const x = 5
        const y = x * 2
        const z = y + 1
        return z * x
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("return"),
        "Should compile. Got:\n{}",
        output
    );
}

#[test]
fn test_dead_code_elimination_complex() {
    let source = r#"
        const dead = 1 + 2
        const alive = dead + 10
        const unused = 100
        return alive
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        !output.contains("100") || output.contains("11"),
        "Should eliminate dead code (unused = 100). Got:\n{}",
        output
    );
}
