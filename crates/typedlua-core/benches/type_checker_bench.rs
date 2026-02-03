use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn bench_type_checker_simple(c: &mut Criterion) {
    let source = r#"
        const x: number = 42
        const y: string = "hello"
        const z: number = x + 10
    "#;

    c.bench_function("type_checker_simple", |b| {
        b.iter(|| {
            let handler = Arc::new(CollectingDiagnosticHandler::new());
            let (interner, common_ids) = StringInterner::new_with_common_identifiers();

            let mut lexer = Lexer::new(source, handler.clone(), &interner);
            let tokens = lexer.tokenize().ok()?;

            let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
            let mut program = parser.parse().ok()?;

            let mut checker = TypeChecker::new_with_stdlib(handler, &interner, &common_ids)
                .with_options(CompilerOptions::default());
            checker.check_program(black_box(&mut program)).ok()
        })
    });
}

fn bench_type_checker_function(c: &mut Criterion) {
    let source = r#"
        function add(a: number, b: number): number {
            return a + b
        }

        function multiply(x: number, y: number): number {
            return x * y
        }

        const result = add(multiply(2, 3), 4)
    "#;

    c.bench_function("type_checker_function", |b| {
        b.iter(|| {
            let handler = Arc::new(CollectingDiagnosticHandler::new());
            let (interner, common_ids) = StringInterner::new_with_common_identifiers();

            let mut lexer = Lexer::new(source, handler.clone(), &interner);
            let tokens = lexer.tokenize().ok()?;

            let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
            let mut program = parser.parse().ok()?;

            let mut checker = TypeChecker::new_with_stdlib(handler, &interner, &common_ids)
                .with_options(CompilerOptions::default());
            checker.check_program(black_box(&mut program)).ok()
        })
    });
}

fn bench_type_checker_class(c: &mut Criterion) {
    let source = r#"
        class Point {
            public x: number
            public y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            public distance(): number {
                return (self.x * self.x + self.y * self.y) ^ 0.5
            }
        }

        const p = new Point(3, 4)
        const dist = p.distance()
    "#;

    c.bench_function("type_checker_class", |b| {
        b.iter(|| {
            let handler = Arc::new(CollectingDiagnosticHandler::new());
            let (interner, common_ids) = StringInterner::new_with_common_identifiers();

            let mut lexer = Lexer::new(source, handler.clone(), &interner);
            let tokens = lexer.tokenize().ok()?;

            let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
            let mut program = parser.parse().ok()?;

            let mut checker = TypeChecker::new_with_stdlib(handler, &interner, &common_ids)
                .with_options(CompilerOptions::default());
            checker.check_program(black_box(&mut program)).ok()
        })
    });
}

fn bench_type_checker_interface(c: &mut Criterion) {
    let source = r#"
        interface Shape {
            area(): number
            perimeter(): number
        }

        class Rectangle implements Shape {
            private width: number
            private height: number

            constructor(width: number, height: number) {
                self.width = width
                self.height = height
            }

            public area(): number {
                return self.width * self.height
            }

            public perimeter(): number {
                return 2 * (self.width + self.height)
            }
        }

        const rect: Shape = new Rectangle(10, 20)
        const a = rect.area()
    "#;

    c.bench_function("type_checker_interface", |b| {
        b.iter(|| {
            let handler = Arc::new(CollectingDiagnosticHandler::new());
            let (interner, common_ids) = StringInterner::new_with_common_identifiers();

            let mut lexer = Lexer::new(source, handler.clone(), &interner);
            let tokens = lexer.tokenize().ok()?;

            let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
            let mut program = parser.parse().ok()?;

            let mut checker = TypeChecker::new_with_stdlib(handler, &interner, &common_ids)
                .with_options(CompilerOptions::default());
            checker.check_program(black_box(&mut program)).ok()
        })
    });
}

fn bench_type_checker_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("type_checker_scaling");

    for size in [10, 25, 50].iter() {
        let source = (0..*size)
            .map(|i| {
                format!(
                    "const var{}: number = {}\nconst result{}: number = var{} + 1",
                    i, i, i, i
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        group.bench_with_input(BenchmarkId::from_parameter(size), &source, |b, s| {
            b.iter(|| {
                let handler = Arc::new(CollectingDiagnosticHandler::new());
                let (interner, common_ids) = StringInterner::new_with_common_identifiers();

                let mut lexer = Lexer::new(s, handler.clone(), &interner);
                let tokens = lexer.tokenize().ok()?;

                let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
                let mut program = parser.parse().ok()?;

                let mut checker = TypeChecker::new_with_stdlib(handler, &interner, &common_ids)
                    .with_options(CompilerOptions::default());
                checker.check_program(black_box(&mut program)).ok()
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_type_checker_simple,
    bench_type_checker_function,
    bench_type_checker_class,
    bench_type_checker_interface,
    bench_type_checker_scaling
);
criterion_main!(benches);
