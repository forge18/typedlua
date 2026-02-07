use typedlua_core::config::OptimizationLevel;
use typedlua_core::di::DiContainer;

fn compile_with_optimization(
    source: &str,
    optimization_level: OptimizationLevel,
) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile_with_optimization(source, optimization_level)
}

#[test]
fn test_simple_add_operator() {
    let source = r#"
        class Vector {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            operator +(other: Vector): Vector {
                return new Vector(self.x + other.x, self.y + other.y)
            }
        }

        const v1 = new Vector(1, 2)
        const v2 = new Vector(3, 4)
        const v3 = v1 + v2
    "#;

    let result = compile_with_optimization(source, OptimizationLevel::O3);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    // Verify the operator is generated
    assert!(
        output.contains("Vector.__add"),
        "Should generate __add metamethod"
    );
    eprintln!("OUTPUT:\n{}", output);
}

#[test]
fn test_operator_multiply() {
    let source = r#"
        class Vector {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            operator *(scalar: number): Vector {
                return new Vector(self.x * scalar, self.y * scalar)
            }
        }

        const v = new Vector(2, 3)
        const result = v * 2
    "#;

    let result = compile_with_optimization(source, OptimizationLevel::O3);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(
        output.contains("Vector.__mul"),
        "Should generate __mul metamethod"
    );
}

#[test]
fn test_comparison_operators() {
    let source = r#"
        class Point {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            operator <(other: Point): boolean {
                return self.x < other.x
            }

            operator ==(other: Point): boolean {
                return self.x == other.x and self.y == other.y
            }
        }

        const p1 = new Point(1, 2)
        const p2 = new Point(3, 4)
        const less = p1 < p2
        const equal = p1 == p2
    "#;

    let result = compile_with_optimization(source, OptimizationLevel::O3);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(
        output.contains("Point.__lt"),
        "Should generate __lt metamethod"
    );
    assert!(
        output.contains("Point.__eq"),
        "Should generate __eq metamethod"
    );
}

#[test]
fn test_multiple_operators() {
    let source = r#"
        class Complex {
            real: number
            imag: number

            constructor(real: number, imag: number) {
                self.real = real
                self.imag = imag
            }

            operator +(other: Complex): Complex {
                return new Complex(self.real + other.real, self.imag + other.imag)
            }

            operator -(other: Complex): Complex {
                return new Complex(self.real - other.real, self.imag - other.imag)
            }

            operator *(other: Complex): Complex {
                return new Complex(
                    self.real * other.real - self.imag * other.imag,
                    self.real * other.imag + self.imag * other.real
                )
            }
        }

        const a = new Complex(1, 2)
        const b = new Complex(3, 4)
        const c = a + b - a * b
    "#;

    let result = compile_with_optimization(source, OptimizationLevel::O3);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(
        output.contains("Complex.__add"),
        "Should generate __add metamethod"
    );
    assert!(
        output.contains("Complex.__sub"),
        "Should generate __sub metamethod"
    );
    assert!(
        output.contains("Complex.__mul"),
        "Should generate __mul metamethod"
    );
}

#[test]
fn test_no_inline_at_o1() {
    let source = r#"
        class Vector {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            operator +(other: Vector): Vector {
                return new Vector(self.x + other.x, self.y + other.y)
            }
        }

        const v1 = new Vector(1, 2)
        const v2 = new Vector(3, 4)
        const v3 = v1 + v2
    "#;

    let result = compile_with_optimization(source, OptimizationLevel::O1);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    // At O1, operator inlining shouldn't run
    assert!(
        output.contains("Vector.__add"),
        "Should still generate __add metamethod at O1"
    );
}

#[test]
fn test_unary_minus_operator() {
    let source = r#"
        class Vector {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            operator -(): Vector {
                return new Vector(-self.x, -self.y)
            }
        }

        const v = new Vector(1, 2)
        const neg = -v
    "#;

    let result = compile_with_optimization(source, OptimizationLevel::O3);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(
        output.contains("Vector.__unm"),
        "Should generate __unm metamethod"
    );
}

#[test]
fn test_chained_operators() {
    let source = r#"
        class Number {
            value: number

            constructor(value: number) {
                self.value = value
            }

            operator +(other: Number): Number {
                return new Number(self.value + other.value)
            }

            operator *(other: Number): Number {
                return new Number(self.value * other.value)
            }
        }

        const a = new Number(1)
        const b = new Number(2)
        const c = new Number(3)
        const result = a + b * c + a
    "#;

    let result = compile_with_optimization(source, OptimizationLevel::O3);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    eprintln!("Chained operators OUTPUT:\n{}", output);

    assert!(
        output.contains("Number.__add"),
        "Should generate __add metamethod"
    );
    assert!(
        output.contains("Number.__mul"),
        "Should generate __mul metamethod"
    );
}

#[test]
fn test_complex_operator() {
    let source = r#"
        class Matrix {
            data: Array<number>

            constructor(data: Array<number>) {
                self.data = data
            }

            operator +(other: Matrix): Matrix {
                local result = {}
                for i = 1, 4 do
                    result[i] = self.data[i] + other.data[i]
                end
                return new Matrix(result)
            }
        }
    "#;

    let result = compile_with_optimization(source, OptimizationLevel::O3);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    eprintln!("Complex operator OUTPUT:\n{}", output);

    assert!(
        output.contains("Matrix.__add"),
        "Should generate __add metamethod"
    );
}

#[test]
fn test_operator_call_in_loop() {
    let source = r#"
        class Counter {
            value: number

            constructor() {
                self.value = 0
            }

            operator +(n: number): Counter {
                local result = new Counter()
                result.value = self.value + n
                return result
            }
        }

        function test()
            local c = new Counter()
            for i = 1, 10 do
                c = c + i
            end
        end
    "#;

    let result = compile_with_optimization(source, OptimizationLevel::O3);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    eprintln!("Loop operator OUTPUT:\n{}", output);
}
