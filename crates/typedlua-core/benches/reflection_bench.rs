use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn compile_and_generate(source: &str) -> String {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().expect("Lexing failed");

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let program = parser.parse().expect("Parsing failed");

    let mut type_checker =
        TypeChecker::new(handler, &interner, &common_ids).with_options(CompilerOptions::default());
    type_checker
        .check_program(&program)
        .expect("Type checking failed");

    let mut codegen = CodeGenerator::new(&interner);
    codegen.generate(&program)
}

fn reflection_codegen_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("reflection_codegen");

    // Benchmark reflection metadata generation for varying class depths
    for depth in [1, 5, 10, 20].iter() {
        let source = generate_class_hierarchy(*depth);

        group.bench_with_input(
            BenchmarkId::new("class_hierarchy", depth),
            &source,
            |b, s| {
                b.iter(|| {
                    let output = compile_and_generate(s);
                    black_box(output);
                });
            },
        );
    }

    group.finish();
}

fn reflection_memory_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("reflection_memory");

    // Measure generated code size for different class sizes
    for num_fields in [1, 5, 10, 20].iter() {
        let source = generate_class_with_fields(*num_fields);
        let output = compile_and_generate(&source);

        group.bench_with_input(
            BenchmarkId::new("fields", num_fields),
            &output,
            |b, code| {
                b.iter(|| {
                    // Measure code size (proxy for memory overhead)
                    let size = code.len();
                    black_box(size);
                });
            },
        );

        // Report actual size in bytes
        println!(
            "Class with {} fields generates {} bytes of Lua code",
            num_fields,
            output.len()
        );

        // Count reflection metadata lines
        let metadata_lines = output
            .lines()
            .filter(|line| {
                line.contains("__typeId")
                    || line.contains("__typeName")
                    || line.contains("__ancestors")
                    || line.contains("__ownFields")
                    || line.contains("__ownMethods")
                    || line.contains("_buildAllFields")
                    || line.contains("_buildAllMethods")
            })
            .count();

        println!(
            "  Reflection metadata: {} lines ({:.1}% of total)",
            metadata_lines,
            (metadata_lines as f64 / output.lines().count() as f64) * 100.0
        );
    }

    group.finish();
}

fn generate_class_hierarchy(depth: usize) -> String {
    let mut source = String::new();

    // Generate base class
    source.push_str("class A0 { x: number }\n");

    // Generate inheritance chain
    for i in 1..depth {
        source.push_str(&format!(
            "class A{} extends A{} {{ y{}: number }}\n",
            i,
            i - 1,
            i
        ));
    }

    source
}

fn generate_class_with_fields(num_fields: usize) -> String {
    let mut source = String::from("class TestClass {\n");

    for i in 0..num_fields {
        source.push_str(&format!("    field{}: number\n", i));
    }

    source.push_str("}\n");
    source
}

criterion_group!(
    benches,
    reflection_codegen_benchmark,
    reflection_memory_benchmark
);
criterion_main!(benches);
