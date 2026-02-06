use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Generate a test TypedLua module with the specified number of statements
fn generate_module(name: &str, statement_count: usize) -> String {
    let mut code = format!("-- Generated module: {}\n\n", name);

    // Add some type declarations
    code.push_str("interface IProcessor {\n");
    code.push_str("    process(value: number): number\n");
    code.push_str("}\n\n");

    // Generate functions with optimizable code
    for i in 0..statement_count {
        code.push_str(&format!(
            r#"function compute_{}(n: number): number
    -- Constant folding opportunity
    const x: number = 5 + 3
    const y: number = x * 2

    -- Dead code elimination opportunity
    if false then
        return 0
    end

    -- Expression optimization
    return n * y + (10 - 5)
end

"#,
            i
        ));
    }

    // Add a class with methods (devirtualization opportunity)
    code.push_str(&format!(
        r#"class Processor implements IProcessor {{
    value: number

    constructor(initial: number)
        self.value = initial
    end

    process(input: number): number
        return input + self.value
    end
}}

export {{ Processor }}
"#
    ));

    code
}

/// Generate a test project with the specified number of modules
fn generate_test_project(module_count: usize, statements_per_module: usize) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    for i in 0..module_count {
        let module_name = format!("module_{}", i);
        let module_content = generate_module(&module_name, statements_per_module);
        let module_path = temp_dir.path().join(format!("{}.tl", module_name));
        fs::write(module_path, module_content).expect("Failed to write module");
    }

    // Create a main module that imports all others
    let mut main_content = String::from("-- Main module\n\n");
    for i in 0..module_count {
        main_content.push_str(&format!("import {{ Processor as P{} }} from \"./module_{}\"\n", i, i));
    }
    main_content.push_str("\nfunction main()\n");
    for i in 0..module_count {
        main_content.push_str(&format!("    const p{} = P{}({})\n", i, i, i));
    }
    main_content.push_str("end\n");

    let main_path = temp_dir.path().join("main.tl");
    fs::write(main_path, main_content).expect("Failed to write main module");

    temp_dir
}

/// Compile a project using the TypedLua CLI
fn compile_project(project_path: &PathBuf, parallel: bool) -> Result<(), String> {
    use std::process::Command;

    let binary_path = env!("CARGO_BIN_EXE_typedlua");
    let mut cmd = Command::new(binary_path);

    cmd.arg("compile")
        .arg(project_path)
        .arg("--no-cache")
        .arg("--no-emit")
        .arg("--optimize");

    if !parallel {
        cmd.arg("--no-parallel-optimization");
    }

    let output = cmd.output().map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(format!(
            "Compilation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

/// Benchmark: Module count scaling
/// Tests how parallelization benefits projects with varying module counts
fn benchmark_module_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("module_parallelism");
    group.sample_size(10); // Reduce sample size for faster benchmarking

    // Test with increasing module counts
    for module_count in [1, 2, 4, 8, 16] {
        let project = generate_test_project(module_count, 5); // Small modules
        let project_path = project.path().to_path_buf();

        // Sequential optimization
        group.bench_with_input(
            BenchmarkId::new("sequential", module_count),
            &project_path,
            |b, path| {
                b.iter(|| {
                    compile_project(path, false).expect("Sequential compilation failed");
                })
            },
        );

        // Parallel optimization (only if module_count > 1)
        if module_count > 1 {
            group.bench_with_input(
                BenchmarkId::new("parallel", module_count),
                &project_path,
                |b, path| {
                    b.iter(|| {
                        compile_project(path, true).expect("Parallel compilation failed");
                    })
                },
            );
        }
    }

    group.finish();
}

/// Benchmark: Module size scaling
/// Tests how statement count affects optimization time (visitor-level parallelism)
fn benchmark_module_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("visitor_parallelism");
    group.sample_size(10);

    // Single module with varying statement counts
    for statement_count in [10, 50, 100, 200] {
        let project = generate_test_project(1, statement_count);
        let project_path = project.path().to_path_buf();

        group.bench_with_input(
            BenchmarkId::new("statements", statement_count),
            &project_path,
            |b, path| {
                b.iter(|| {
                    compile_project(path, true).expect("Compilation failed");
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Optimization level impact
/// Tests which optimization level benefits most from parallelization
fn benchmark_optimization_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimization_levels");
    group.sample_size(10);

    let project = generate_test_project(8, 10); // Medium-sized project
    let project_path = project.path().to_path_buf();

    // Sequential
    group.bench_function("O3_sequential", |b| {
        b.iter(|| {
            compile_project(&project_path, false).expect("Compilation failed");
        })
    });

    // Parallel
    group.bench_function("O3_parallel", |b| {
        b.iter(|| {
            compile_project(&project_path, true).expect("Compilation failed");
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_module_count,
    benchmark_module_size,
    benchmark_optimization_levels
);
criterion_main!(benches);
