use typedlua_core::config::{CompilerConfig, OptimizationLevel};
use typedlua_core::di::DiContainer;

fn compile_with_opt_level(source: &str, level: OptimizationLevel) -> Result<String, String> {
    let config = CompilerConfig::default();
    let mut container = DiContainer::production(config);
    container.compile_with_stdlib_and_optimization(source, level)
}

// ============================================================================
// Dead Loop Removal Tests
// ============================================================================

#[test]
fn test_while_false_body_cleared() {
    let source = r#"
        while false do
            print("never")
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        !output.contains("never"),
        "Dead while loop body should be cleared. Got:\n{}",
        output
    );
    assert!(
        output.contains("while"),
        "While keyword remains (loop structure preserved). Got:\n{}",
        output
    );
}

#[test]
fn test_for_zero_iterations_body_cleared() {
    let source = r#"
        for i = 10, 5 do
            print(i)
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        !output.contains("print"),
        "Zero-iteration for loop body should be cleared. Got:\n{}",
        output
    );
    assert!(
        output.contains("for"),
        "For keyword remains (loop structure preserved). Got:\n{}",
        output
    );
}

#[test]
fn test_for_negative_step_zero_iterations_body_cleared() {
    let source = r#"
        for i = 1, 10, -1 do
            print(i)
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        !output.contains("print"),
        "Zero-iteration for loop body with negative step should be cleared. Got:\n{}",
        output
    );
    assert!(
        output.contains("for"),
        "For keyword remains. Got:\n{}",
        output
    );
}

#[test]
fn test_repeat_until_true_body_cleared() {
    let source = r#"
        repeat
            print("once")
        until true
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        !output.contains("once"),
        "Repeat-until-true body should be cleared. Got:\n{}",
        output
    );
    assert!(
        output.contains("repeat"),
        "Repeat keyword remains (loop structure preserved). Got:\n{}",
        output
    );
}

#[test]
fn test_preserve_while_true() {
    let source = r#"
        while true do
            print("infinite")
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("while true do"),
        "Infinite while loop should be preserved for debugger compatibility. Got:\n{}",
        output
    );
    assert!(
        output.contains("infinite"),
        "Loop body should be preserved. Got:\n{}",
        output
    );
}

#[test]
fn test_dead_loop_body_cleared_at_o2_not_o1() {
    let source = r#"
        while false do
            print("never")
        end
    "#;

    let o1_output = compile_with_opt_level(source, OptimizationLevel::O1).unwrap();
    let o2_output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();

    assert!(
        o1_output.contains("never"),
        "Dead loop body should be preserved at O1. Got:\n{}",
        o1_output
    );
    assert!(
        !o2_output.contains("never"),
        "Dead loop body should be cleared at O2. Got:\n{}",
        o2_output
    );
}

// ============================================================================
// Repeat Loop Support Tests
// ============================================================================

#[test]
fn test_repeat_until_false_preserved() {
    let source = r#"
        local count: number = 0
        repeat
            count = count + 1
        until count >= 10
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("repeat"),
        "Repeat loop should be preserved. Got:\n{}",
        output
    );
    assert!(
        output.contains("until"),
        "Until condition should be preserved. Got:\n{}",
        output
    );
}

#[test]
fn test_repeat_nested() {
    let source = r#"
        repeat
            repeat
                print("inner")
            until false
            print("outer")
        until false
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("repeat"),
        "Nested repeat loops should be preserved. Got:\n{}",
        output
    );
    let first_repeat = output.find("repeat").unwrap();
    let second_repeat = output[first_repeat + 6..].find("repeat");
    assert!(
        second_repeat.is_some(),
        "Should have nested repeat. Got:\n{}",
        output
    );
}

// ============================================================================
// Combined Optimization Tests
// ============================================================================

#[test]
fn test_loop_processing_at_o2() {
    let source = r#"
        for i = 1, 5 do
            print(i)
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("for"),
        "For loop should be processed at O2. Got:\n{}",
        output
    );
}

#[test]
fn test_while_loop_processing_at_o2() {
    let source = r#"
        local x: number = 0
        while x < 10 do
            x = x + 1
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("while"),
        "While loop should be processed at O2. Got:\n{}",
        output
    );
}

#[test]
fn test_function_with_loops() {
    let source = r#"
        function test(): void
            local x: number = 0
            while x < 10 do
                x = x + 1
            end
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("while"),
        "While loop in function should be processed. Got:\n{}",
        output
    );
}

#[test]
fn test_nested_loops() {
    let source = r#"
        for i = 1, 3 do
            local j: number = 0
            while j < 2 do
                j = j + 1
            end
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    let for_count = output.matches("for").count();
    let while_count = output.matches("while").count();
    assert!(
        for_count >= 1 && while_count >= 1,
        "Nested loops should be processed. Got:\n{}",
        output
    );
}

#[test]
fn test_if_with_loops() {
    let source = r#"
        local cond: boolean = true
        if cond then
            for i = 1, 5 do
                print(i)
            end
        end
    "#;

    let output = compile_with_opt_level(source, OptimizationLevel::O2).unwrap();
    assert!(
        output.contains("for"),
        "Loop in if-branch should be processed. Got:\n{}",
        output
    );
}

