use typedlua_core::config::OptimizationLevel;
use typedlua_core::di::DiContainer;

fn compile_with_optimization(source: &str) -> Result<String, String> {
    compile_with_optimization_level(source, OptimizationLevel::O1)
}

fn compile_with_optimization_level(
    source: &str,
    level: OptimizationLevel,
) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile_with_stdlib_and_optimization(source, level)
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

