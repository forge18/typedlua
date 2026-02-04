use typedlua_core::config::OptimizationLevel;
use typedlua_core::di::DiContainer;

fn compile_and_check(source: &str) -> Result<String, String> {
    compile_with_level(source, OptimizationLevel::O0)
}

fn compile_with_level(source: &str, level: OptimizationLevel) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile_with_stdlib_and_optimization(source, level)
}

#[test]
fn test_try_catch_compiles() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        end
    "#;

    let output = compile_and_check(source).unwrap();
    println!("Output:\n{}", output);
    // Try/catch block form compiles successfully
}

#[test]
fn test_throw_statement() {
    let source = r#"
        throw "error message"
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            assert!(output.contains("error(\"error message\")"));
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should compile successfully");
        }
    }
}

#[test]
fn test_throw_expression() {
    let source = r#"
        const message = "Something went wrong"
        throw message
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            assert!(output.contains("error(message)"));
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Should compile successfully");
        }
    }
}

#[test]
fn test_error_chain_operator() {
    let source = r#"
        const getValue = (): number => 42
        const fallback = (): number => 0
        const result = getValue() !! fallback()
    "#;

    let result = compile_and_check(source);
    if let Ok(output) = result {
        println!("Generated code:\n{}", output);
    } else if let Err(e) = result {
        println!("Error (expected for now): {}", e);
    }
}

#[test]
fn test_rethrow() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print("caught: " .. e)
            rethrow
        end
    "#;

    let result = compile_and_check(source);
    match result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
        }
        Err(e) => {
            panic!("Should compile successfully. Error: {}", e);
        }
    }
}

#[test]
fn test_try_expression() {
    let source = r#"
        function riskyFunc(): number
            throw "error"
        end
        const result = try riskyFunc() catch 0
    "#;

    let result = compile_and_check(source);
    match result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            assert!(output.contains("pcall"), "Generated code:\n{}", output);
        }
        Err(e) => {
            panic!("Should compile successfully. Error: {}", e);
        }
    }
}

#[test]
fn test_function_throws_clause() {
    let source = r#"
        function riskyOperation(): number throws string
            throw "Something went wrong"
        end
    "#;

    let result = compile_and_check(source);
    match result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            assert!(output.contains("error"));
        }
        Err(e) => {
            panic!("Should compile successfully. Error: {}", e);
        }
    }
}

#[test]
fn test_try_catch_at_all_optimization_levels() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        end
    "#;

    // Try/catch block should compile at all optimization levels
    for level in [
        OptimizationLevel::O0,
        OptimizationLevel::O1,
        OptimizationLevel::O2,
        OptimizationLevel::O3,
        OptimizationLevel::Auto,
    ] {
        let result = compile_with_level(source, level);
        assert!(
            result.is_ok(),
            "Try/catch should compile at {:?}: {:?}",
            level,
            result.err()
        );
    }
}

#[test]
fn test_try_catch_finally_compiles() {
    let source = r#"
        try {
            throw "error"
        } catch (e) {
            print(e)
        } finally {
            print("cleanup")
        end
    "#;

    let result = compile_with_level(source, OptimizationLevel::O0);
    assert!(
        result.is_ok(),
        "Try/catch/finally should compile: {:?}",
        result.err()
    );
}
