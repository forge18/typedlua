use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use typedlua_core::{Parser, TypeChecker, TypeEnvironment, DiagnosticHandler};

fn bench_type_checker_simple(c: &mut Criterion) {
    let source = r#"
        const x: number = 42
        const y: string = "hello"
        const z: number = x + 10
    "#;

    c.bench_function("type_checker_simple", |b| {
        b.iter(|| {
            let mut parser = Parser::new(source);
            let program = parser.parse().unwrap();
            let mut env = TypeEnvironment::new();
            let handler = DiagnosticHandler::new();
            let mut checker = TypeChecker::new(&mut env, handler);
            checker.check_program(black_box(&program))
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
            let mut parser = Parser::new(source);
            let program = parser.parse().unwrap();
            let mut env = TypeEnvironment::new();
            let handler = DiagnosticHandler::new();
            let mut checker = TypeChecker::new(&mut env, handler);
            checker.check_program(black_box(&program))
        })
    });
}

fn bench_type_checker_class(c: &mut Criterion) {
    let source = r#"
        class Point {
            public x: number
            public y: number

            constructor(x: number, y: number) {
                this.x = x
                this.y = y
            }

            public distance(): number {
                return (this.x * this.x + this.y * this.y) ^ 0.5
            }
        }

        const p = new Point(3, 4)
        const dist = p.distance()
    "#;

    c.bench_function("type_checker_class", |b| {
        b.iter(|| {
            let mut parser = Parser::new(source);
            let program = parser.parse().unwrap();
            let mut env = TypeEnvironment::new();
            let handler = DiagnosticHandler::new();
            let mut checker = TypeChecker::new(&mut env, handler);
            checker.check_program(black_box(&program))
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
                this.width = width
                this.height = height
            }

            public area(): number {
                return this.width * this.height
            }

            public perimeter(): number {
                return 2 * (this.width + this.height)
            }
        }

        const rect: Shape = new Rectangle(10, 20)
        const a = rect.area()
    "#;

    c.bench_function("type_checker_interface", |b| {
        b.iter(|| {
            let mut parser = Parser::new(source);
            let program = parser.parse().unwrap();
            let mut env = TypeEnvironment::new();
            let handler = DiagnosticHandler::new();
            let mut checker = TypeChecker::new(&mut env, handler);
            checker.check_program(black_box(&program))
        })
    });
}

fn bench_type_checker_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("type_checker_scaling");

    for size in [10, 25, 50].iter() {
        let source = format!(
            "{}",
            (0..*size)
                .map(|i| format!(
                    "const var{}: number = {}\nconst result{}: number = var{} + 1",
                    i, i, i, i
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );

        group.bench_with_input(BenchmarkId::from_parameter(size), &source, |b, s| {
            b.iter(|| {
                let mut parser = Parser::new(s);
                let program = parser.parse().unwrap();
                let mut env = TypeEnvironment::new();
                let handler = DiagnosticHandler::new();
                let mut checker = TypeChecker::new(&mut env, handler);
                checker.check_program(black_box(&program))
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
