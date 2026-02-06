use typedlua_core::config::{CompilerConfig, CompilerOptions};
use typedlua_core::di::DiContainer;

fn type_check(source: &str) -> Result<(), String> {
    let mut config = CompilerConfig::default();
    config.compiler_options = CompilerOptions {
        enable_decorators: true,
        ..Default::default()
    };
    let mut container = DiContainer::test_with_config(config);
    container.compile(source)?;
    Ok(())
}

#[test]
fn test_builtin_readonly_decorator() {
    let source = r#"
        class Config {
            @readonly
            api_key: string = "secret"
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Built-in @readonly decorator should be recognized"
    );
}

#[test]
fn test_builtin_sealed_decorator() {
    let source = r#"
        @sealed
        class FinalClass {
            value: number = 0
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Built-in @sealed decorator should be recognized"
    );
}

#[test]
fn test_builtin_deprecated_decorator() {
    let source = r#"
        @deprecated("Use newFunction instead")
        function oldFunction(): void
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Built-in @deprecated decorator should be recognized"
    );
}

#[test]
fn test_unknown_decorator_no_error() {
    let source = r#"
        function myDecorator(target) return target end

        @myDecorator
        class MyClass {
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Unknown decorator should not cause error"
    );
}

#[test]
fn test_decorator_on_class() {
    let source = r#"
        function classDecorator(target) return target end

        @classDecorator
        class MyClass {
            value: number
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator on class should compile"
    );
}

#[test]
fn test_decorator_on_method() {
    let source = r#"
        function methodDecorator(target: any, key: string, desc: PropertyDescriptor)
            return desc
        end

        class MyClass {
            @methodDecorator
            public myMethod(): void {
            end
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator on method should compile"
    );
}

#[test]
fn test_decorator_on_getter() {
    let source = r#"
        function getterDecorator(target: any, key: string, desc: PropertyDescriptor)
            return desc
        end

        class MyClass {
            private _value: number = 0

            @getterDecorator
            public get value(): number {
                return self._value
            end
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator on getter should compile"
    );
}

#[test]
fn test_decorator_on_field() {
    let source = r#"
        function fieldDecorator(target: any, key: string)
            return target
        end

        class MyClass {
            @fieldDecorator
            public myField: number = 0
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator on field should compile"
    );
}

#[test]
fn test_decorator_chaining() {
    let source = r#"
        function dec1(target) return target end
        function dec2(target) return target end

        @dec1
        @dec2
        class MyClass {
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator chaining should compile"
    );
}

#[test]
fn test_decorator_factory() {
    let source = r#"
        function decoratorFactory(param: string)
            return function(target) {
                return target
            end
        end

        @decoratorFactory("test")
        class MyClass {
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator factory should compile"
    );
}

#[test]
fn test_decorator_on_abstract_class() {
    let source = r#"
        function decorator(target) return target end

        @decorator
        abstract class AbstractClass {
            public abstract method(): void
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator on abstract class should compile"
    );
}

#[test]
fn test_decorator_with_wrong_param_count() {
    let source = r#"
        function badDecorator(a: number, b: number, c: number)
            return function(target) { return target }
        end

        @badDecorator(1, 2, 3)
        class MyClass {
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator with many params should compile"
    );
}

#[test]
fn test_decorator_returning_void() {
    let source = r#"
        function voidDecorator()
            return function(target) {
                // returns nothing (nil)
            end
        end

        @voidDecorator()
        class MyClass {
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator returning void should compile"
    );
}

#[test]
fn test_decorator_on_static_method() {
    let source = r#"
        function decorator(target: any, key: string, desc: PropertyDescriptor)
            return desc
        end

        class MyClass {
            @decorator
            public static myMethod(): void {
            end
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator on static method should compile"
    );
}

#[test]
fn test_decorator_on_static_field() {
    let source = r#"
        function decorator(target: any, key: string)
            return target
        end

        class MyClass {
            @decorator
            public static myField: number = 0
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Decorator on static field should compile"
    );
}

#[test]
fn test_multiple_decorators_same_element() {
    let source = r#"
        function dec1(target) return target end
        function dec2(target) return target end

        class MyClass {
            @dec1
            @dec2
            public myMethod(): void {
            end
        end
    "#;

    assert!(
        type_check(source).is_ok(),
        "Multiple decorators on same element should compile"
    );
}
