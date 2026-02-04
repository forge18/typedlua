use typedlua_core::config::{CompilerConfig, OptimizationLevel};
use typedlua_core::di::DiContainer;

fn compile_with_optimization_level(
    source: &str,
    level: OptimizationLevel,
) -> Result<String, String> {
    let config = CompilerConfig::default();
    let mut container = DiContainer::production(config);
    container.compile_with_stdlib_and_optimization(source, level)
}

// ============================================================================
// String Concatenation Optimization Tests
// ============================================================================

#[test]
fn test_simple_concat_chain_optimization() {
    let source = r#"
        const a = "hello"
        const b = " "
        const c = "world"
        const result = a .. b .. c
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("table.concat"),
        "Should use table.concat for 3+ concatenations. Output:\n{}",
        output
    );
}

#[test]
fn test_four_part_concat_chain() {
    let source = r#"
        const result = "a" .. "b" .. "c" .. "d"
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("table.concat"),
        "Should use table.concat for 4 concatenations. Output:\n{}",
        output
    );
}

#[test]
fn test_no_optimization_for_two_parts() {
    let source = r#"
        return "hello" .. "world"
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        !output.contains("table.concat"),
        "Should NOT use table.concat for 2 concatenations. Output:\n{}",
        output
    );
    assert!(
        output.contains(".."),
        "Should use .. operator for 2 concatenations. Output:\n{}",
        output
    );
}

#[test]
fn test_no_optimization_at_o1() {
    let source = r#"
        const result = "a" .. "b" .. "c" .. "d"
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O1).unwrap();
    assert!(
        !output.contains("table.concat"),
        "Should NOT use table.concat at O1. Output:\n{}",
        output
    );
}

#[test]
fn test_nested_concat_optimization() {
    let source = r#"
        local a = "a"
        local b = "b"
        local c = "c"
        local d = "d"
        local result = (a .. b) .. (c .. d)
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("table.concat"),
        "Should use table.concat for nested concatenations that flatten to 4 parts. Output:\n{}",
        output
    );
}

#[test]
fn test_concat_in_function_return() {
    let source = r#"
        function greet(name: string): string {
            const greeting = "Hello"
            const sep = " "
            const suffix = "!"
            return greeting .. sep .. name .. suffix
        }
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("table.concat"),
        "Should use table.concat in function return. Output:\n{}",
        output
    );
}

#[test]
fn test_concat_in_variable_declaration() {
    let source = r#"
        const a = "x"
        const b = "y"
        const c = "z"
        const result = a .. b .. c
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("table.concat"),
        "Should use table.concat in const declaration. Output:\n{}",
        output
    );
}

#[test]
fn test_concat_in_expression_statement() {
    let source = r#"
        local result = "a" .. "b" .. "c"
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("table.concat"),
        "Should use table.concat in expression statement. Output:\n{}",
        output
    );
}

#[test]
fn test_three_part_concat() {
    let source = r#"
        local a = "1"
        local b = "2"
        local c = "3"
        local result = a .. b .. c
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("table.concat"),
        "Should use table.concat for 3 part concatenation. Output:\n{}",
        output
    );
}

#[test]
fn test_mixed_concat_optimization() {
    let source = r#"
        const header = "=== "
        const content = "Test"
        const footer = " ==="
        const line = header .. content .. footer
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("table.concat"),
        "Should use table.concat for mixed literals and variables. Output:\n{}",
        output
    );
}
