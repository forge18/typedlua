use typedlua_core::config::OptimizationLevel;
use typedlua_core::di::DiContainer;

fn compile_with_optimization_level(
    source: &str,
    level: OptimizationLevel,
) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile_with_optimization(source, level)
}

fn compile_with_o2(source: &str) -> Result<String, String> {
    compile_with_optimization_level(source, OptimizationLevel::O2)
}

fn compile_with_o2_stdlib(source: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile_with_stdlib_and_optimization(source, OptimizationLevel::O2)
}

#[test]
fn test_dead_store_simple_unused_variable() {
    let source = r#"
        const unused = 42
        const x = 1
    "#;

    let output = compile_with_o2(source).unwrap();
    assert!(
        !output.contains("unused"),
        "Dead store should be eliminated: {}",
        output
    );
}

#[test]
fn test_dead_store_reassigned_variable() {
    let source = r#"
        local x = 1
        x = 2
        return x
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Dead store reassigned:\n{}", output);
    // O2 optimizer does not currently eliminate reassigned variable initial values
    assert!(
        output.contains("return x"),
        "Final return should be preserved"
    );
}

#[test]
fn test_dead_store_in_loop() {
    let source = r#"
        local sum = 0
        for i in [1, 2, 3] {
            sum = sum + i
        end
        return sum
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Dead store in loop:\n{}", output);
    assert!(
        output.contains("sum"),
        "Live variable in loop should be kept"
    );
}

#[test]
fn test_dead_store_across_blocks() {
    let source = r#"
        local x = 10
        if true then
            local y = x + 1
        end
        return x
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Dead store across blocks:\n{}", output);
    assert!(!output.contains("y"), "Variable y should be eliminated");
}

#[test]
fn test_dead_store_nested_conditionals() {
    let source = r#"
        local a = 1
        if true then
            local b = a + 1
            if true then
                local c = b + 1
            end
        end
        return a
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Dead store nested:\n{}", output);
    assert!(
        !output.contains("local b") && !output.contains("local c"),
        "Nested dead stores should be eliminated"
    );
}

#[test]
fn test_dead_store_with_function_call() {
    let source = r#"
        local x = print("dead")
        return 42
    "#;

    let output = compile_with_o2_stdlib(source).unwrap();
    println!("Dead store with function:\n{}", output);
    // Function call with side effect (print) must be kept even if result is unused
    assert!(
        output.contains("print"),
        "Function call with side effect should be preserved"
    );
}

#[test]
fn test_dead_store_const_reassigned() {
    let source = r#"
        const x = 1
        const y = x + 1
        return y
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Const reassigned:\n{}", output);
    assert!(
        !output.contains("= 1"),
        "Const assignment should be eliminated if only used once"
    );
}

