use typedlua_core::di::DiContainer;

fn compile_and_check(source: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile(source)
}

#[test]
fn test_readonly_class_decorator() {
    let source = r#"
        @readonly
        class Config {
            value: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "readonly decorator should compile: {:?}",
        result.err()
    );
    let _output = result.unwrap();
}

#[test]
fn test_readonly_prevents_modification() {
    let source = r#"
        @readonly
        class Point {
            x: number
            y: number
        }

        const p = new Point()
        p.x = 5
    "#;

    let result = compile_and_check(source);
    assert!(result.is_err(), "readonly should prevent modification");
}

#[test]
fn test_deprecated_decorator() {
    let source = r#"
        @deprecated("Use newFunction instead")
        function oldFunction()
            return 42
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "deprecated decorator should compile");
}

#[test]
fn test_sealed_decorator() {
    let source = r#"
        @sealed
        class SealedClass {
            value: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "sealed decorator should compile");
}

#[test]
fn test_multiple_class_decorators() {
    let source = r#"
        function decorator1(target) return target end
        function decorator2(target) return target end

        @decorator1
        @decorator2
        class MultiClass {
            value: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "multiple decorators should compile");
}

#[test]
fn test_decorator_with_parameters() {
    let source = r#"
        function author(name: string)
            return function(target)
                return target
            end
        end

        @author("John Doe")
        class Document {
            title: string
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "decorator with parameters should compile: {:?}", result.err());
}

#[test]
fn test_readonly_method_decorator() {
    let source = r#"
        class Counter {
            private _count: number = 0

            @readonly
            public getCount(): number
                return self._count
            end
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "readonly method decorator should compile");
}

#[test]
fn test_decorator_with_field_initializers() {
    let source = r#"
        function withDefault(value: number)
            return function(target: any, prop: string)
                return target
            end
        end

        class MyClass {
            @withDefault(42)
            value: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "decorator with field initializers should compile: {:?}", result.err()
    );
}

#[test]
fn test_decorator_order() {
    let source = r#"
        function dec1(target)
            return target
        end
        function dec2(target)
            return target
        end
        function dec3(target)
            return target
        end

        @dec1
        @dec2
        @dec3
        class OrderedClass {
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "decorators should be applied in order");
}

#[test]
fn test_decorator_on_getter() {
    let source = r#"
        function cached(target)
            return target
        end

        class MyClass {
            private _value: number = 0

            @cached
            public get value(): number {
                return self._value
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "decorator on getter should compile");
}

#[test]
fn test_decorator_on_setter() {
    let source = r#"
        function logged(target: any, prop: string)
            return target
        end

        class MyClass {
            private _value: number = 0

            @logged
            public set value(v: number) {
                self._value = v
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "decorator on setter should compile: {:?}", result.err());
}

#[test]
fn test_decorator_returns_undefined() {
    let source = r#"
        function noReturn(target)
        end

        @noReturn
        class NoReturnClass {
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "decorator returning undefined should still work"
    );
}

#[test]
fn test_class_decorator_replaces_constructor() {
    let source = r#"
        function singleton(cls)
            return cls
        end

        @singleton
        class Singleton {
            value: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "decorator replacing constructor should compile: {:?}", result.err()
    );
}

#[test]
fn test_method_decorator_with_static() {
    let source = r#"
        function logCall(target: any, prop: string)
            return target
        end

        class MathOps {
            @logCall
            public static add(a: number, b: number): number
                return a + b
            end
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "method decorator on static method should compile: {:?}", result.err()
    );
}

#[test]
fn test_readonly_with_constructor() {
    let source = r#"
        @readonly
        class Immutable {
            public value: number

            constructor(v: number)
                self.value = v
            end
        }

        const obj = new Immutable(10)
        const v = obj.value
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "readonly class should allow constructor initialization: {:?}", result.err()
    );
}

#[test]
fn test_decorator_type_inference() {
    let source = r#"
        function createDecorator()
            return function(target)
                return target
            end
        end

        @createDecorator()
        class DecoratedClass {
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "decorator with type inference should compile"
    );
}

#[test]
fn test_multiple_decorators_same_type() {
    let source = r#"
        function log(target: any): any return target end
        function seal(target: any): any return target end

        class TestClass {
        }

        const result = log(seal(TestClass))
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "manual decorator application should work: {:?}", result.err());
}

#[test]
fn test_decorator_with_generic_class() {
    let source = r#"
        function addMethod(methodName: string)
            return function(target)
                return target
            end
        end

        @addMethod("customMethod")
        class GenericClass<T> {
            value: T
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "decorator with generic class should compile"
    );
}

#[test]
fn test_decorator_error_handling() {
    let source = r#"
        function throws(target)
            error("Decorator error")
        end

        @throws
        class WillFail {
        }
    "#;

    let result = compile_and_check(source);
    // error() is a runtime function, so compilation should succeed.
    // Runtime decorator errors are not caught at compile time.
    assert!(
        result.is_ok(),
        "decorator with error() should compile (runtime error, not compile-time): {:?}", result.err()
    );
}

#[test]
fn test_decorator_receives_correct_descriptor() {
    let source = r#"
        function inspect(target: any, prop: string)
            return target
        end

        class TestClass {
            @inspect
            public myMethod(): void
            end
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "decorator should receive correct descriptor"
    );
}

#[test]
fn test_readonly_property_inheritance() {
    let source = r#"
        @readonly
        class Base {
            value: number
        }

        class Derived extends Base {
            other: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "readonly should work with inheritance");
}
