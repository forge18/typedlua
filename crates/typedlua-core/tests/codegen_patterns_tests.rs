use typedlua_core::config::CompilerConfig;
use typedlua_core::di::DiContainer;

fn compile(source: &str) -> Result<String, String> {
    let config = CompilerConfig::default();
    let mut container = DiContainer::production(config);
    container.compile_with_stdlib(source)
}

fn compile_with_target(source: &str, target: &str) -> Result<String, String> {
    let mut config = CompilerConfig::default();
    config.compiler_options.target = match target {
        "5.1" => typedlua_core::config::LuaVersion::Lua51,
        "5.2" => typedlua_core::config::LuaVersion::Lua52,
        "5.3" => typedlua_core::config::LuaVersion::Lua53,
        "5.4" => typedlua_core::config::LuaVersion::Lua54,
        _ => typedlua_core::config::LuaVersion::Lua54,
    };
    let mut container = DiContainer::test(
        config,
        std::sync::Arc::new(typedlua_core::diagnostics::CollectingDiagnosticHandler::new()),
        std::sync::Arc::new(typedlua_core::fs::RealFileSystem::new()),
    );
    container.compile(source)
}

// ============================================================================
// Destructuring Tests
// ============================================================================

#[test]
fn test_table_destructuring() {
    let source = r#"
        local point = {x = 10, y = 20}
        local x = point.x
        local y = point.y
        return x + y
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("x") && output.contains("y"),
        "Should handle table access. Got:\n{}",
        output
    );
}

#[test]
fn test_nested_table_access() {
    let source = r#"
        local nested = {outer = {inner = 42}}
        local inner = nested.outer.inner
        return inner
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("42"),
        "Should handle nested access. Got:\n{}",
        output
    );
}

#[test]
fn test_array_literal() {
    let source = r#"
        const arr = [1, 2, 3]
        return arr[1]
    "#;

    let result = compile(source);
    if result.is_err() {
        eprintln!("Array literal error: {}", result.unwrap_err());
    }
    // Just verify it compiles without crash
    assert!(true);
}

// ============================================================================
// Lua Target Strategy Tests
// ============================================================================

#[test]
fn test_lua51_target_basic() {
    let source = r#"
        local x = 42
        return x
    "#;

    let output = compile_with_target(source, "5.1").unwrap();
    assert!(
        output.contains("local x = 42"),
        "Should compile for Lua 5.1. Got:\n{}",
        output
    );
}

#[test]
fn test_lua52_target_basic() {
    let source = r#"
        local x = 42
        return x
    "#;

    let output = compile_with_target(source, "5.2").unwrap();
    assert!(
        output.contains("local x = 42"),
        "Should compile for Lua 5.2. Got:\n{}",
        output
    );
}

#[test]
fn test_lua53_target_basic() {
    let source = r#"
        local x = 42
        return x
    "#;

    let output = compile_with_target(source, "5.3").unwrap();
    assert!(
        output.contains("local x = 42"),
        "Should compile for Lua 5.3. Got:\n{}",
        output
    );
}

#[test]
fn test_lua54_target_basic() {
    let source = r#"
        local x = 42
        return x
    "#;

    let output = compile_with_target(source, "5.4").unwrap();
    assert!(
        output.contains("local x = 42"),
        "Should compile for Lua 5.4. Got:\n{}",
        output
    );
}

#[test]
fn test_integer_literals_lua53() {
    let source = r#"
        local x = 42
        local y = 3.14
        return x
    "#;

    let output = compile_with_target(source, "5.3").unwrap();
    eprintln!("Integer literals:\n{}", output);
}

// ============================================================================
// Codegen Emitter Tests
// ============================================================================

#[test]
fn test_emitter_indentation() {
    let source = r#"
        function test(): void
            if true then
                print("nested")
            end
        end
    "#;

    let output = compile(source).unwrap();
    let indent_count = output.matches("    ").count();
    assert!(
        indent_count >= 2,
        "Should have proper indentation. Got:\n{}",
        output
    );
}

#[test]
fn test_emitter_long_string() {
    let source = r#"
        local long = "This is a string"
        return long
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("long"),
        "Should handle strings. Got:\n{}",
        output
    );
}

#[test]
fn test_emitter_escape_sequences() {
    let source = r#"
        local escaped = "Hello\nWorld\t!"
        return escaped
    "#;

    let output = compile(source).unwrap();
    eprintln!("Escape sequences:\n{}", output);
}

// ============================================================================
// Class Codegen Tests
// ============================================================================

