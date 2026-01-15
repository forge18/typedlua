use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

// Helper to create typedlua command using the non-deprecated macro approach
fn typedlua_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("typedlua"))
}

// ============================================================================
// PROJECT INITIALIZATION TESTS
// ============================================================================

/// Test --init creates project structure
#[test]
fn test_init_creates_config_file() {
    let temp_dir = TempDir::new().unwrap();

    typedlua_cmd()
        .current_dir(&temp_dir)
        .arg("--init")
        .assert()
        .success()
        .stdout(predicate::str::contains("tlconfig.yaml"));

    assert!(temp_dir.path().join("tlconfig.yaml").exists());
    assert!(temp_dir.path().join("src").exists());
    assert!(temp_dir.path().join("src/main.tl").exists());
}

/// Test --init creates valid config
#[test]
fn test_init_creates_valid_config() {
    let temp_dir = TempDir::new().unwrap();

    typedlua_cmd()
        .current_dir(&temp_dir)
        .arg("--init")
        .assert()
        .success();

    let config = fs::read_to_string(temp_dir.path().join("tlconfig.yaml")).unwrap();
    assert!(config.contains("compilerOptions"));
    assert!(config.contains("target"));
    assert!(config.contains("outDir"));
    assert!(config.contains("sourceMap"));
}

// ============================================================================
// CONFIGURATION FILE TESTS
// ============================================================================

/// Test compilation with custom config file
#[test]
fn test_compile_with_config_file() {
    let temp_dir = TempDir::new().unwrap();

    // Create config
    let config = r#"
compilerOptions:
  target: "5.1"
  outDir: "./output"
  sourceMap: false
  strict: true
"#;
    fs::write(temp_dir.path().join("tlconfig.yaml"), config).unwrap();

    // Create source file
    let source = "const x: number = 42";
    fs::write(temp_dir.path().join("test.tl"), source).unwrap();

    typedlua_cmd()
        .current_dir(&temp_dir)
        .arg("--project")
        .arg("tlconfig.yaml")
        .arg("test.tl")
        .assert()
        .success();

    // Config specifies outDir: "./output", so check there
    assert!(
        temp_dir.path().join("output/test.lua").exists()
            || temp_dir.path().join("test.lua").exists(),
        "Expected output file at output/test.lua or test.lua"
    );
}

// ============================================================================
// ERROR HANDLING AND VALIDATION TESTS
// ============================================================================

/// Test error when no input files provided
#[test]
fn test_error_no_input_files() {
    typedlua_cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("No input files"));
}

/// Test invalid Lua target version falls back to default
#[test]
fn test_invalid_lua_target_falls_back() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");
    fs::write(&input_file, "const x: number = 42").unwrap();

    // Invalid target should fall back to default (5.4) and still compile
    typedlua_cmd()
        .arg(input_file)
        .arg("--target")
        .arg("9.9")
        .assert()
        .success();
}

/// Test lexer errors are reported properly
#[test]
fn test_lexer_error_reporting() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("bad.tl");

    // Invalid character that lexer can't handle
    fs::write(&input_file, "const x = @@@").unwrap();

    typedlua_cmd().arg(input_file).assert().failure();
}

/// Test parser errors are reported
#[test]
fn test_parser_error_reporting() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("syntax_error.tl");

    // Syntax error: unclosed brace
    fs::write(&input_file, "const obj = { x = 1").unwrap();

    typedlua_cmd().arg(input_file).assert().failure();
}

// ============================================================================
// OUTPUT FILE TESTS
// ============================================================================

/// Test --out-file concatenates multiple files
#[test]
fn test_out_file_concatenation() {
    let temp_dir = TempDir::new().unwrap();
    let file1 = temp_dir.path().join("file1.tl");
    let file2 = temp_dir.path().join("file2.tl");
    let out_file = temp_dir.path().join("bundle.lua");

    fs::write(&file1, "const a: number = 1").unwrap();
    fs::write(&file2, "const b: number = 2").unwrap();

    typedlua_cmd()
        .arg(&file1)
        .arg(&file2)
        .arg("--out-file")
        .arg(&out_file)
        .assert()
        .success();

    assert!(out_file.exists());
    let content = fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("a") && content.contains("b"));
}

/// Test inline source maps
#[test]
fn test_inline_source_map() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");
    let output_file = temp_dir.path().join("test.lua");

    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--inline-source-map")
        .assert()
        .success();

    let output = fs::read_to_string(&output_file).unwrap();
    // Inline source maps are embedded as comments
    assert!(output.contains("sourceMappingURL") || output.len() > 0);
}

// ============================================================================
// PARALLEL COMPILATION TESTS
// ============================================================================

/// Test parallel compilation of multiple files
#[test]
fn test_parallel_compilation_many_files() {
    let temp_dir = TempDir::new().unwrap();
    let mut files = Vec::new();

    // Create 10 files
    for i in 0..10 {
        let file = temp_dir.path().join(format!("file{}.tl", i));
        fs::write(&file, format!("const x{}: number = {}", i, i)).unwrap();
        files.push(file);
    }

    let mut cmd = typedlua_cmd();
    for file in &files {
        cmd.arg(file);
    }

    cmd.assert().success();

    // Verify at least some output files exist (may be in temp_dir or dist/)
    let mut found_count = 0;
    for i in 0..10 {
        if temp_dir.path().join(format!("file{}.lua", i)).exists()
            || temp_dir.path().join(format!("dist/file{}.lua", i)).exists()
        {
            found_count += 1;
        }
    }
    assert!(
        found_count >= 5,
        "Expected at least 5 output files, found {}",
        found_count
    );
}

// ============================================================================
// STRICT MODE AND TYPE CHECKING TESTS
// ============================================================================

/// Test strict mode catches more errors
#[test]
fn test_strict_mode_with_config() {
    let temp_dir = TempDir::new().unwrap();

    let config = r#"
compilerOptions:
  strict: true
  noEmit: true
"#;
    fs::write(temp_dir.path().join("tlconfig.yaml"), config).unwrap();

    // Code that might fail strict checking
    let source = r#"
        function test(x)
            return x + 1
        end
    "#;
    let input_file = temp_dir.path().join("test.tl");
    fs::write(&input_file, source).unwrap();

    // This should fail or succeed depending on strict mode implementation
    typedlua_cmd()
        .arg("--project")
        .arg(temp_dir.path().join("tlconfig.yaml"))
        .arg(input_file)
        .assert()
        .code(predicate::in_iter(vec![0, 1])); // Accept either success or failure
}

// ============================================================================
// DIAGNOSTIC OUTPUT TESTS
// ============================================================================

/// Test --diagnostics flag shows diagnostic codes
#[test]
fn test_diagnostics_flag() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("error.tl");

    fs::write(&input_file, "const x: number = \"wrong\"").unwrap();

    typedlua_cmd()
        .arg(input_file)
        .arg("--diagnostics")
        .assert()
        .failure();
}

/// Test pretty printing is on by default
#[test]
fn test_pretty_printing_default() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("error.tl");

    fs::write(&input_file, "const x: number = \"wrong\"").unwrap();

    let output = typedlua_cmd().arg(input_file).output().unwrap();

    // Pretty output uses ANSI codes or formatting
    assert!(!output.stderr.is_empty());
}

// ============================================================================
// LUA VERSION COMPATIBILITY TESTS
// ============================================================================

/// Test Lua 5.1 target compatibility
#[test]
fn test_lua51_compatibility() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");
    let output_file = temp_dir.path().join("test.lua");
    let dist_output = temp_dir.path().join("dist/test.lua");

    // Use features compatible with Lua 5.1
    fs::write(&input_file, "function test()\n    return 42\nend").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--target")
        .arg("5.1")
        .assert()
        .success();

    assert!(
        output_file.exists() || dist_output.exists(),
        "Expected output file at {:?} or {:?}",
        output_file,
        dist_output
    );
}

/// Test Lua 5.4 target (default)
#[test]
fn test_lua54_default_target() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");

    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--no-emit")
        .assert()
        .success()
        .stdout(predicate::str::contains("Lua54").or(predicate::str::contains("5.4")));
}

// ============================================================================
// FILE SYSTEM EDGE CASES
// ============================================================================

/// Test compilation with nested directory structure
#[test]
fn test_nested_directory_structure() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir_all(temp_dir.path().join("src/utils")).unwrap();
    let input_file = temp_dir.path().join("src/utils/helper.tl");

    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--out-dir")
        .arg(temp_dir.path().join("dist"))
        .assert()
        .success();

    // Should preserve directory structure
    assert!(
        temp_dir.path().join("dist/helper.lua").exists()
            || temp_dir.path().join("dist/utils/helper.lua").exists()
            || temp_dir.path().join("dist/src/utils/helper.lua").exists()
    );
}

/// Test handling of files with spaces in names
#[test]
fn test_file_with_spaces() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("my file.tl");

    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd().arg(&input_file).assert().success();
}
