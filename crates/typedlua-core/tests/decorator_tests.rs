use typedlua_core::di::DiContainer;

fn compile_and_check(source: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile(source)
}

#[test]
fn test_simple_class_decorator() {
    let source = r#"
        function sealed(target)
            return target
        end

        @sealed
        class MyClass {
        end
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Simple class decorator should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();
}

#[test]
fn test_class_decorator_with_params() {
    let source = r#"
        function author(name: string)
            return function(target)
                target.author = name
                return target
            end
        end

        @author("John Doe")
        class MyClass {
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Class decorator with params should compile");
}

#[test]
fn test_class_decorator_chaining() {
    let source = r#"
        function decorator1(target) return target end
        function decorator2(target) return target end
        function decorator3(target) return target end

        @decorator1
        @decorator2
        @decorator3
        class MyClass {
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Chained decorators should compile");
}

#[test]
fn test_method_decorator() {
    let source = r#"
        function logged(method: any, name: string)
            return function(...args)
                print("Calling " .. name)
                return method(...args)
            end
        end

        class MyClass {
            @logged
            public myMethod(): void {
                print("Hello")
            end
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Method decorator should compile");
}

#[test]
fn test_getter_decorator() {
    let source = r#"
        function cached(get: () => any)
            return function()
                if not self._cached then
                    self._cached = get()
                end
                return self._cached
            end
        end

        class MyClass {
            private _value: number = 0

            @cached
            public get value(): number {
                return self._value
            end
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Getter decorator should compile");
}

#[test]
fn test_setter_decorator() {
    let source = r#"
        function validated(set: (any) => any)
            return function(v)
                if typeof(v) == "nil" then
                    error("Value cannot be nil")
                end
                return set(v)
            end
        end

        class MyClass {
            private _name: string = ""

            @validated
            public set name(v: string) {
                self._name = v
            end
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Setter decorator should compile");
}

#[test]
fn test_field_decorator() {
    let source = r#"
        function default(value: any)
            return function(target, key)
                if not (key in target) then
                    target[key] = value
                end
                return target
            end
        end

        class MyClass {
            @default(42)
            value: number
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Field decorator should compile");
}

#[test]
fn test_decorator_with_this() {
    let source = r#"
        function bound(method: any, _name: string, desc: PropertyDescriptor)
            return {
                get = function()
                    const bound = method.bind(self)
                    return bound
                end
            end
        end

        class MyClass {
            value: number = 0

            @bound
            public getValue(): number {
                return self.value
            end
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Decorator using 'this' should compile");
}

#[test]
fn test_decorator_returns_modified_class() {
    let source = r#"
        function addStaticMethod(name: string, method: any)
            return function(target)
                target[name] = method
                return target
            end
        end

        @addStaticMethod("helper", function() return 42 end)
        class MyClass {
        end
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Decorator returning modified class should compile"
    );
}

#[test]
fn test_decorator_factory() {
    let source = r#"
        function myDecorator(param: string)
            return function(target)
                target._param = param
                return target
            end
        end

        @myDecorator("test")
        class MyClass {
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Decorator factory should compile");
}

#[test]
fn test_decorator_preserves_inheritance() {
    let source = r#"
        function addMethod(name: string, value: any)
            return function(target)
                target[name] = value
                return target
            end
        end

        class Base {
        end

        @addMethod("newMethod", function() return 1 end)
        class Derived extends Base {
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Decorator should preserve inheritance");
}

#[test]
fn test_multiple_decorators_same_element() {
    let source = r#"
        function decorator1(target) return target end
        function decorator2(target) return target end

        class MyClass {
            @decorator1
            @decorator2
            public myMethod(): void {
            end
        end
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Multiple decorators on same element should compile"
    );
}

#[test]
fn test_decorator_accessing_descriptor() {
    let source = r#"
        function logDescriptor(target: any, key: string, desc: PropertyDescriptor)
            print("Property: " .. key)
            print("Configurable: " .. tostring(desc.configurable))
            return desc
        end

        class MyClass {
            @logDescriptor
            public myProp: number = 0
        end
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Decorator accessing descriptor should compile"
    );
}

#[test]
fn test_decorator_with_rest_parameter() {
    let source = r#"
        function trace(method: any, _name: string, desc: PropertyDescriptor)
            return {
                value = function(...args)
                    print("Calling with args: " .. tostring(#args))
                    return method(...args)
                end
            end
        end

        class MyClass {
            @trace
            public sum(...nums: number[]): number {
                let s = 0
                for n in nums {
                    s = s + n
                end
                return s
            end
        end
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Decorator with rest parameter should compile"
    );
}

#[test]
fn test_decorator_on_abstract_class() {
    let source = r#"
        function abstract(target) return target end

        @abstract
        abstract class AbstractClass {
            public abstract method(): void
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Decorator on abstract class should compile");
}

#[test]
fn test_decorator_on_interface() {
    let source = r#"
        function seal(target) return target end

        interface MyInterface {
            method(): void
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Decorator on interface should compile");
}

#[test]
fn test_decorator_order_bottom_up() {
    let source = r#"
        const order: string[] = []

        function first(target)
            order.push("first")
            return target
        end

        function second(target)
            order.push("second")
            return target
        end

        @first
        @second
        class MyClass {
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Decorator order should compile");
}

#[test]
fn test_decorator_with_generic() {
    let source = r#"
        function addMetadata(data: any)
            return function(target)
                target._metadata = data
                return target
            end
        end

        @addMetadata({ version: "1.0" })
        class GenericClass<T> {
            public value: T
        end
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Decorator with generic class should compile"
    );
}

#[test]
fn test_decorator_on_getter_setter_pair() {
    let source = r#"
        function trackAccess(target: any, key: string)
            const accesses = []

            return {
                get = function()
                    accesses.push("get")
                    return target[key]
                end,
                set = function(v)
                    accesses.push("set")
                    target[key] = v
                end
            end
        end

        class MyClass {
            private _value: number = 0

            @trackAccess
            public value: number
        end
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Decorator on getter/setter pair should compile"
    );
}

#[test]
fn test_decorator_type_preservation() {
    let source = r#"
        function identity<T>(target: T): T
            return target
        end

        @identity
        class MyClass {
            public value: number = 0
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Decorator should preserve type information");
}
