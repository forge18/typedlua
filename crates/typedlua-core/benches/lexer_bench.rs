use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use typedlua_core::Lexer;

fn bench_lexer_simple(c: &mut Criterion) {
    let source = r#"
        const x: number = 42
        const y: string = "hello"
        function add(a: number, b: number): number {
            return a + b
        }
    "#;

    c.bench_function("lexer_simple", |b| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.collect::<Vec<_>>()
        })
    });
}

fn bench_lexer_class(c: &mut Criterion) {
    let source = r#"
        class User {
            public name: string
            private age: number

            constructor(name: string, age: number) {
                this.name = name
                this.age = age
            }

            public greet(): void {
                print(`Hello, ${this.name}!`)
            }
        }
    "#;

    c.bench_function("lexer_class", |b| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(source));
            lexer.collect::<Vec<_>>()
        })
    });
}

fn bench_lexer_size_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer_scaling");

    for size in [10, 50, 100, 500].iter() {
        let source = format!(
            "{}",
            (0..*size)
                .map(|i| format!("const var{}: number = {}", i, i))
                .collect::<Vec<_>>()
                .join("\n")
        );

        group.bench_with_input(BenchmarkId::from_parameter(size), &source, |b, s| {
            b.iter(|| {
                let lexer = Lexer::new(black_box(s));
                lexer.collect::<Vec<_>>()
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_lexer_simple, bench_lexer_class, bench_lexer_size_scaling);
criterion_main!(benches);
