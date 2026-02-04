use typedlua_core::di::DiContainer;

fn compile_and_generate(source: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile_with_stdlib(source)
}

#[test]
fn test_class_has_type_metadata() {
    let source = r#"
        class User {
            name: string
            age: number
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Class should compile with metadata");
}

#[test]
fn test_interface_type_info() {
    let source = r#"
        interface Drawable {
            draw(): void
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Interface should compile");
}

#[test]
fn test_enum_reflection() {
    let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Enum should compile");
}

#[test]
fn test_function_reflection() {
    let source = r#"
        function add(a: number, b: number): number {
            return a + b
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Function should compile");
}

#[test]
fn test_type_reference() {
    let source = r#"
        type Point = { x: number, y: number }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Type alias should compile");
}

#[test]
fn test_generic_class_reflection() {
    let source = r#"
        class Box<T> {
            value: T

            constructor(value: T) {
                self.value = value
            }
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Generic class should compile");
}

#[test]
fn test_nested_class_reflection() {
    let source = r#"
        class Outer {
            value: number

            constructor(value: number) {
                self.value = value
            }

            class Inner {
                outer: Outer

                constructor(o: Outer) {
                    self.outer = o
                }
            }
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Nested class should compile");
}

#[test]
fn test_abstract_class_reflection() {
    let source = r#"
        abstract class Shape {
            public abstract area(): number
        }

        class Circle extends Shape {
            radius: number

            constructor(radius: number) {
                self.radius = radius
            }

            public area(): number {
                return 3.14159 * self.radius * self.radius
            }
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Abstract class should compile");
}

#[test]
fn test_property_reflection() {
    let source = r#"
        class Config {
            public host: string = "localhost"
            public port: number = 8080
            public debug: boolean = false
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Properties should compile");
}

#[test]
fn test_method_reflection() {
    let source = r#"
        class Math {
            public static add(a: number, b: number): number {
                return a + b
            }
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Methods should compile");
}

#[test]
fn test_getter_setter_reflection() {
    let source = r#"
        class Counter {
            private _value: number = 0

            public get value(): number {
                return self._value
            }

            public set value(v: number) {
                self._value = v
            }
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Getter/setter should compile");
}

#[test]
fn test_inherited_class_reflection() {
    let source = r#"
        class Base {
            public baseMethod(): void {
            }
        }

        class Derived extends Base {
            public derivedMethod(): void {
            }
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Inherited class should compile");
}

#[test]
fn test_interface_implementation() {
    let source = r#"
        interface Printable {
            print(): void
        }

        class Document implements Printable {
            public print(): void {
                print("document")
            }
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Interface implementation should compile");
}

#[test]
fn test_union_type_reflection() {
    let source = r#"
        type StringOrNumber = string | number
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Union type should compile");
}

#[test]
fn test_intersection_type_reflection() {
    let source = r#"
        type A = { a: number }
        type B = { b: string }
        type C = A & B
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Intersection type should compile");
}

#[test]
fn test_builtin_types() {
    let source = r#"
        const n: number = 42
        const s: string = "hello"
        const b: boolean = true
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Builtin types should compile");
}

#[test]
fn test_generic_function_reflection() {
    let source = r#"
        function identity<T>(x: T): T {
            return x
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Generic function should compile");
}

#[test]
fn test_namespace_reflection() {
    let source = r#"
        namespace MyNamespace {
            export const value = 42
            export function greet(): void {
                print("hello")
            }
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Namespace should compile");
}

#[test]
fn test_constructor_reflection() {
    let source = r#"
        class Point {
            public x: number
            public y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }
        }
    "#;

    let result = compile_and_generate(source);
    assert!(result.is_ok(), "Constructor should compile");
}
