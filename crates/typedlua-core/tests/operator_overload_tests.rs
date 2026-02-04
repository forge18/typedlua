use typedlua_core::di::DiContainer;

fn compile_and_check(source: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile(source)
}

#[test]
fn test_operator_add() {
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

        const a = new Vector(1, 2)
        const b = new Vector(3, 4)
        const c = a + b
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Add operator should compile");
}

#[test]
fn test_operator_sub() {
    let source = r#"
        class Point {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            operator -(other: Point): Point {
                return new Point(self.x - other.x, self.y - other.y)
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Subtract operator should compile");
}

#[test]
fn test_operator_mul() {
    let source = r#"
        class Scale {
            value: number

            constructor(value: number) {
                self.value = value
            }

            operator *(n: number): Scale {
                return new Scale(self.value * n)
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Multiply operator should compile");
}

#[test]
fn test_operator_div() {
    let source = r#"
        class Fraction {
            num: number
            den: number

            constructor(num: number, den: number) {
                self.num = num
                self.den = den
            }

            operator /(other: Fraction): Fraction {
                return new Fraction(self.num * other.den, self.den * other.num)
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Divide operator should compile");
}

#[test]
fn test_operator_unary_minus() {
    let source = r#"
        class Negatable {
            value: number

            constructor(value: number) {
                self.value = value
            }

            operator -(): Negatable {
                return new Negatable(-self.value)
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Unary minus should compile");
}

#[test]
fn test_operator_concat() {
    let source = r#"
        class Label {
            text: string

            constructor(text: string) {
                self.text = text
            }

            operator ..(other: Label): Label {
                return new Label(self.text .. other.text)
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Concat operator should compile");
}

#[test]
fn test_operator_eq() {
    let source = r#"
        class Box {
            width: number
            height: number

            constructor(width: number, height: number) {
                self.width = width
                self.height = height
            }

            operator ==(other: Box): boolean {
                return self.width == other.width and self.height == other.height
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Eq operator should compile");
}

#[test]
fn test_operator_lt() {
    let source = r#"
        class Version {
            major: number
            minor: number

            constructor(major: number, minor: number) {
                self.major = major
                self.minor = minor
            }

            operator <(other: Version): boolean {
                if self.major != other.major {
                    return self.major < other.major
                }
                return self.minor < other.minor
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Lt operator should compile");
}

#[test]
fn test_operator_call() {
    let source = r#"
        class FunctionObject {
            fn: (number) => number

            constructor(fn: (number) => number) {
                self.fn = fn
            }

            operator ()(x: number): number {
                return self.fn(x)
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Call operator should compile");
}

#[test]
fn test_operator_index() {
    let source = r#"
        class SparseArray {
            data: { [number]: number } = {}

            constructor() {
            }

            operator [](index: number): number | nil {
                return self.data[index]
            }

            operator []=(index: number, value: number) {
                self.data[index] = value
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Index operators should compile");
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
        const result = a + b * c
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Chained operators should compile");
}

#[test]
fn test_compound_assignment() {
    let source = r#"
        class Counter {
            value: number = 0

            constructor() {
            }

            operator +=(n: number): void {
                self.value = self.value + n
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Compound assignment should compile");
}

#[test]
fn test_relational_operators() {
    let source = r#"
        class Comparable {
            value: number

            constructor(value: number) {
                self.value = value
            }

            operator <(other: Comparable): boolean {
                return self.value < other.value
            }

            operator <=(other: Comparable): boolean {
                return self.value <= other.value
            }

            operator >(other: Comparable): boolean {
                return self.value > other.value
            }

            operator >=(other: Comparable): boolean {
                return self.value >= other.value
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Relational operators should compile");
}

#[test]
fn test_modulo_operator() {
    let source = r#"
        class FixedPoint {
            value: number

            constructor(value: number) {
                self.value = value
            }

            operator %(other: FixedPoint): number {
                return self.value % other.value
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Modulo operator should compile");
}

#[test]
fn test_power_operator() {
    let source = r#"
        class Exponent {
            base: number

            constructor(base: number) {
                self.base = base
            }

            operator ^(exp: number): number {
                return self.base ^ exp
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Power operator should compile");
}

#[test]
fn test_length_operator() {
    let source = r#"
        class Sized {
            items: number[] = []

            constructor() {
            }

            operator #(): number {
                return #self.items
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Length operator should compile");
}

#[test]
fn test_operator_with_self_type() {
    let source = r#"
        class Adder {
            value: number = 0

            constructor() {
            }

            operator +(n: number): Adder {
                self.value = self.value + n
                return self
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Operator with self type should compile");
}
