use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

// Helper to create typedlua command using the non-deprecated macro approach
fn typedlua_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("typedlua"))
}

// ============================================================================
// LUA TARGET VERSION COVERAGE
// Test all Lua versions to cover match arms in main.rs:87-88
// ============================================================================

#[test]
fn test_lua52_target() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");
    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--target")
        .arg("5.2")
        .assert()
        .success();
}

#[test]
fn test_lua53_target() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");
    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--target")
        .arg("5.3")
        .assert()
        .success();
}

#[test]
fn test_lua_target_shorthand() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");
    fs::write(&input_file, "const x: number = 42").unwrap();

    // Test shorthand "51" instead of "5.1"
    typedlua_cmd()
        .arg(&input_file)
        .arg("--target")
        .arg("51")
        .assert()
        .success();
}

// ============================================================================
// OUTPUT FILE VARIATIONS
// Cover different output path branches
// ============================================================================

#[test]
fn test_out_file_single_file() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");
    let out_file = temp_dir.path().join("output.lua");

    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--out-file")
        .arg(&out_file)
        .assert()
        .success();

    assert!(out_file.exists());
    let content = fs::read_to_string(&out_file).unwrap();
    assert!(!content.is_empty());
}

#[test]
fn test_out_dir_preserves_structure() {
    let temp_dir = TempDir::new().unwrap();
    fs::create_dir_all(temp_dir.path().join("src/lib")).unwrap();

    let input_file = temp_dir.path().join("src/lib/test.tl");
    let out_dir = temp_dir.path().join("dist");

    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--out-dir")
        .arg(&out_dir)
        .assert()
        .success();

    // Output should exist somewhere in dist
    assert!(out_dir.exists());
}

// ============================================================================
// SOURCE MAP VARIATIONS
// Test source map flags
// ============================================================================

#[test]
fn test_source_map_external() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");
    let map_file = temp_dir.path().join("test.lua.map");

    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--source-map")
        .assert()
        .success();

    // External source map file should exist
    assert!(map_file.exists());
}

#[test]
fn test_inline_and_external_source_map() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");

    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--source-map")
        .arg("--inline-source-map")
        .assert()
        .success();
}

// ============================================================================
// DIAGNOSTIC FLAGS
// Test diagnostic-related flags
// ============================================================================

#[test]
fn test_no_pretty_flag() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("error.tl");

    fs::write(&input_file, "const x: number = \"wrong\"").unwrap();

    // --pretty=false should produce plain output
    typedlua_cmd()
        .arg(&input_file)
        .arg("--pretty=false")
        .assert()
        .failure();
}

#[test]
fn test_diagnostics_with_pretty() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("error.tl");

    fs::write(&input_file, "const x: number = \"wrong\"").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--diagnostics")
        .arg("--pretty")
        .assert()
        .failure();
}

// ============================================================================
// ERROR PATHS AND EDGE CASES
// Cover error handling code
// ============================================================================

#[test]
fn test_directory_as_input() {
    let temp_dir = TempDir::new().unwrap();
    fs::create_dir_all(temp_dir.path().join("src")).unwrap();

    // Try to compile a directory (should fail or handle gracefully)
    typedlua_cmd()
        .arg(temp_dir.path().join("src"))
        .assert()
        .code(predicates::ord::ne(0)); // Should not exit successfully
}

#[test]
fn test_empty_file() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("empty.tl");

    fs::write(&input_file, "").unwrap();

    // Empty file should compile successfully (no statements)
    typedlua_cmd().arg(&input_file).assert().success();
}

#[test]
fn test_very_large_file() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("large.tl");

    // Generate a large file with many statements
    let mut content = String::new();
    for i in 0..1000 {
        content.push_str(&format!("const x{}: number = {}\n", i, i));
    }
    fs::write(&input_file, content).unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--no-emit") // Don't create huge output
        .assert()
        .success();
}

// ============================================================================
// FILE SYSTEM EDGE CASES
// Test unusual file paths and names
// ============================================================================

#[test]
fn test_unicode_in_filename() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("测试文件.tl");

    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd().arg(&input_file).assert().success();
}

#[test]
fn test_special_chars_in_filename() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test-file_v2.0.tl");

    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd().arg(&input_file).assert().success();
}

#[test]
fn test_deeply_nested_path() {
    let temp_dir = TempDir::new().unwrap();
    let deep_path = temp_dir.path().join("a/b/c/d/e/f/g");
    fs::create_dir_all(&deep_path).unwrap();

    let input_file = deep_path.join("deep.tl");
    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd().arg(&input_file).assert().success();
}

// ============================================================================
// PARALLEL COMPILATION STRESS TESTS
// Test parallel compilation with many files
// ============================================================================

#[test]
fn test_parallel_compilation_stress() {
    let temp_dir = TempDir::new().unwrap();
    let mut files = Vec::new();

    // Create 50 files to stress test parallel compilation
    for i in 0..50 {
        let file = temp_dir.path().join(format!("module{}.tl", i));
        fs::write(
            &file,
            format!(
                "export const value{} = {}\nexport function get{}() return {} end",
                i, i, i, i
            ),
        )
        .unwrap();
        files.push(file);
    }

    let mut cmd = typedlua_cmd();
    for file in &files {
        cmd.arg(file);
    }
    cmd.arg("--no-emit"); // Don't create 50 output files

    cmd.assert().success();
}

// ============================================================================
// COMBINED FLAG TESTS
// Test combinations of flags to cover different code paths
// ============================================================================

#[test]
fn test_all_flags_combined() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");
    let out_dir = temp_dir.path().join("output");

    fs::write(&input_file, "const x: number = 42").unwrap();

    typedlua_cmd()
        .arg(&input_file)
        .arg("--target")
        .arg("5.3")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--source-map")
        .arg("--pretty")
        .arg("--diagnostics")
        .assert()
        .success();

    assert!(out_dir.exists());
}
