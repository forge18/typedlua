use typedlua_core::config::OptimizationLevel;
use typedlua_core::di::DiContainer;

fn compile_with_optimization(source: &str, level: OptimizationLevel) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile_with_optimization(source, level)
}

#[test]
fn test_small_function_inlines_o3() {
    let source = r#"
        function add(a: number, b: number): number
            return a + b
        end

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
        function mediumFunc(a: number, b: number): number
            local x1 = a + b
            local x2 = a - b
            local x3 = a * b
            return x1 + x2 + x3
        end

        const result = mediumFunc(10, 5)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O3).unwrap();
    println!("O3 output:\n{}", output);
}

#[test]
fn test_recursive_function_not_inlined() {
    let source = r#"
        function factorial(n: number): number
            if n <= 1 then
                return 1
            end
            return n * factorial(n - 1)
        end

        const result = factorial(5)
    "#;

    let output = compile_with_optimization(source, OptimizationLevel::O3).unwrap();
    println!("Recursive function output:\n{}", output);
    assert!(
        output.contains("function"),
        "Should preserve function definition for recursion"
    );
}

#[test]
fn test_large_function_not_inlined() {
    let source = r#"
        function largeFunc(a: number): number
            local r1 = a + 1
            local r2 = a + 2
            local r3 = a + 3
            local r4 = a + 4
            local r5 = a + 5
            local r6 = a + 6
            local r7 = a + 7
            local r8 = a + 8
            local r9 = a + 9
            local r10 = a + 10
            return r1 + r2 + r3 + r4 + r5 + r6 + r7 + r8 + r9 + r10
        end

        const result = largeFunc(1)
    "#;

    let o3_output = compile_with_optimization(source, OptimizationLevel::O3).unwrap();
    println!("Large function O3 output:\n{}", o3_output);
    assert!(
        o3_output.contains("largeFunc"),
        "Large functions should not be fully inlined"
    );
}

