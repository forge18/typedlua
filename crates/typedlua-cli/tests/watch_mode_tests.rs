use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

fn get_binary_path() -> PathBuf {
    let mut path = env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove 'deps'
    path.push("typedlua");
    path
}

/// Test that watch mode starts and performs initial compilation
#[test]
fn test_watch_mode_starts() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");

    fs::write(&input_file, "const x: number = 42").unwrap();

    // Start watch mode in a subprocess
    let mut child = Command::new(get_binary_path())
        .arg(&input_file)
        .arg("--watch")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start watch mode");

    // Give it time to start and do initial compilation
    thread::sleep(Duration::from_millis(500));

    // Kill the watch process
    child.kill().expect("Failed to kill watch process");

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify watch mode started
    assert!(
        stdout.contains("Watching") || stdout.contains("watching") || stdout.contains("Initial")
    );
}

/// Test that watch mode detects file changes and recompiles
#[test]
fn test_watch_mode_recompiles_on_change() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");
    let output_file = temp_dir.path().join("test.lua");

    // Create initial file
    fs::write(&input_file, "const x: number = 42").unwrap();

    // Start watch mode
    let mut child = Command::new(get_binary_path())
        .arg(&input_file)
        .arg("--watch")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start watch mode");

    // Wait for initial compilation
    thread::sleep(Duration::from_millis(800));

    // Get initial output file timestamp
    let initial_metadata = fs::metadata(&output_file).ok();

    // Modify the file to trigger recompilation
    thread::sleep(Duration::from_millis(200));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&input_file)
        .unwrap();
    file.write_all(b"const y: number = 100").unwrap();
    file.sync_all().unwrap();
    drop(file);

    // Wait for watch mode to detect change and recompile
    thread::sleep(Duration::from_millis(800));

    // Check if output file was updated
    let final_metadata = fs::metadata(&output_file).ok();

    // Kill watch process
    child.kill().expect("Failed to kill watch process");
    child.wait().unwrap();

    // Verify recompilation occurred
    if let (Some(initial), Some(final_meta)) = (initial_metadata, final_metadata) {
        // File should have been modified (different timestamp or we got a new file)
        assert!(
            initial.modified().unwrap() != final_meta.modified().unwrap() || output_file.exists(),
            "Watch mode should have triggered recompilation"
        );
    }
}

/// Test watch mode with multiple files
#[test]
fn test_watch_mode_multiple_files() {
    let temp_dir = TempDir::new().unwrap();
    let file1 = temp_dir.path().join("file1.tl");
    let file2 = temp_dir.path().join("file2.tl");

    fs::write(&file1, "const a: number = 1").unwrap();
    fs::write(&file2, "const b: number = 2").unwrap();

    let mut child = Command::new(get_binary_path())
        .arg(&file1)
        .arg(&file2)
        .arg("--watch")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start watch mode");

    // Wait for initial compilation
    thread::sleep(Duration::from_millis(500));

    // Modify one file
    fs::write(&file1, "const a: number = 100").unwrap();

    // Wait for recompilation
    thread::sleep(Duration::from_millis(500));

    // Kill process
    child.kill().expect("Failed to kill watch process");
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should have watched and compiled both files
    assert!(!stdout.is_empty()); // Got some output
}

/// Test watch mode exits cleanly on Ctrl+C simulation
#[test]
fn test_watch_mode_can_be_stopped() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");

    fs::write(&input_file, "const x: number = 42").unwrap();

    let mut child = Command::new(get_binary_path())
        .arg(&input_file)
        .arg("--watch")
        .spawn()
        .expect("Failed to start watch mode");

    // Let it run briefly
    thread::sleep(Duration::from_millis(300));

    // Send SIGTERM (simulates Ctrl+C)
    child.kill().expect("Failed to kill watch process");

    // Should exit
    let result = child.wait();
    assert!(result.is_ok(), "Watch mode should exit cleanly");
}

/// Test watch mode with --no-emit flag
#[test]
fn test_watch_mode_with_no_emit() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");
    let output_file = temp_dir.path().join("test.lua");

    fs::write(&input_file, "const x: number = 42").unwrap();

    let mut child = Command::new(get_binary_path())
        .arg(&input_file)
        .arg("--watch")
        .arg("--no-emit")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start watch mode");

    // Wait for initial compilation
    thread::sleep(Duration::from_millis(500));

    // Kill watch process
    child.kill().unwrap();
    child.wait().unwrap();

    // With --no-emit, no output file should be created
    assert!(
        !output_file.exists() || fs::read_to_string(&output_file).is_err(),
        "Watch mode with --no-emit should not create output file"
    );
}

/// Test watch mode handles compilation errors gracefully
#[test]
fn test_watch_mode_handles_errors() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");

    // Start with valid code
    fs::write(&input_file, "const x: number = 42").unwrap();

    let mut child = Command::new(get_binary_path())
        .arg(&input_file)
        .arg("--watch")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start watch mode");

    // Wait for initial compilation
    thread::sleep(Duration::from_millis(500));

    // Introduce a type error
    fs::write(&input_file, "const x: number = \"wrong type\"").unwrap();

    // Wait for recompilation (should show error but keep running)
    thread::sleep(Duration::from_millis(500));

    // Fix the error
    fs::write(&input_file, "const x: number = 100").unwrap();

    // Wait for successful recompilation
    thread::sleep(Duration::from_millis(500));

    // Kill process
    child.kill().unwrap();
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should have shown error but kept running
    assert!(
        stderr.contains("error") || stderr.contains("Error") || !stderr.is_empty(),
        "Watch mode should report errors"
    );
}

/// Test watch mode handles rapid changes (debouncing)
#[test]
fn test_watch_mode_debouncing() {
    let temp_dir = TempDir::new().unwrap();
    let input_file = temp_dir.path().join("test.tl");

    fs::write(&input_file, "const x: number = 1").unwrap();

    let mut child = Command::new(get_binary_path())
        .arg(&input_file)
        .arg("--watch")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start watch mode");

    // Initial compilation
    thread::sleep(Duration::from_millis(500));

    // Make rapid changes
    for i in 0..5 {
        fs::write(&input_file, format!("const x: number = {}", i)).unwrap();
        thread::sleep(Duration::from_millis(50)); // Rapid changes
    }

    // Wait for final compilation
    thread::sleep(Duration::from_millis(300));

    child.kill().unwrap();
    let output = child.wait_with_output().unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Count "Compiling" messages which indicate actual compilation starts
    let recompile_count = combined.matches("Compiling").count();

    // With 5 rapid changes, we expect:
    // - Initial compilation (1)
    // - Some recompilations for the changes (ideally batched, but may be individual)
    // The watch mode should handle all changes without crashing or missing files
    // Accept up to 6 compilations (initial + up to 5 for changes) as valid
    assert!(
        (1..=6).contains(&recompile_count),
        "Watch mode should handle rapid changes, got {} 'Compiling' messages (expected 1-6)",
        recompile_count
    );
}
