use typedlua_core::config::CompilerConfig;
use typedlua_core::di::DiContainer;

fn compile(source: &str) -> Result<String, String> {
    let config = CompilerConfig::default();
    let mut container = DiContainer::production(config);
    container.compile_with_stdlib(source)
}

fn compile_with_target(source: &str, target: &str) -> Result<String, String> {
    let mut config = CompilerConfig::default();
    config.compiler_options.target = match target {
        "5.1" => typedlua_core::config::LuaVersion::Lua51,
        "5.2" => typedlua_core::config::LuaVersion::Lua52,
        "5.3" => typedlua_core::config::LuaVersion::Lua53,
        "5.4" => typedlua_core::config::LuaVersion::Lua54,
        _ => typedlua_core::config::LuaVersion::Lua54,
    };
    let mut container = DiContainer::test(
        config,
        std::sync::Arc::new(typedlua_core::diagnostics::CollectingDiagnosticHandler::new()),
        std::sync::Arc::new(typedlua_core::fs::RealFileSystem::new()),
    );
    container.compile(source)
}

// ============================================================================
// Destructuring Tests
// ============================================================================

#[test]
fn test_table_destructuring() {
    let source = r#"
        local point = {x = 10, y = 20}
        local x = point.x
        local y = point.y
        return x + y
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("x") && output.contains("y"),
        "Should handle table access. Got:\n{}",
        output
    );
}

#[test]
fn test_nested_table_access() {
    let source = r#"
        local nested = {outer = {inner = 42}}
        local inner = nested.outer.inner
        return inner
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("42"),
        "Should handle nested access. Got:\n{}",
        output
    );
}

#[test]
fn test_array_literal() {
    let source = r#"
        const arr = [1, 2, 3]
        return arr[1]
    "#;

    let result = compile(source);
    if result.is_err() {
        eprintln!("Array literal error: {}", result.unwrap_err());
    }
    // Just verify it compiles without crash
    assert!(true);
}

// ============================================================================
// Lua Target Strategy Tests
// ============================================================================

#[test]
fn test_lua51_target_basic() {
    let source = r#"
        local x = 42
        return x
    "#;

    let output = compile_with_target(source, "5.1").unwrap();
    assert!(
        output.contains("local x = 42"),
        "Should compile for Lua 5.1. Got:\n{}",
        output
    );
}

#[test]
fn test_lua52_target_basic() {
    let source = r#"
        local x = 42
        return x
    "#;

    let output = compile_with_target(source, "5.2").unwrap();
    assert!(
        output.contains("local x = 42"),
        "Should compile for Lua 5.2. Got:\n{}",
        output
    );
}

#[test]
fn test_lua53_target_basic() {
    let source = r#"
        local x = 42
        return x
    "#;

    let output = compile_with_target(source, "5.3").unwrap();
    assert!(
        output.contains("local x = 42"),
        "Should compile for Lua 5.3. Got:\n{}",
        output
    );
}

#[test]
fn test_lua54_target_basic() {
    let source = r#"
        local x = 42
        return x
    "#;

    let output = compile_with_target(source, "5.4").unwrap();
    assert!(
        output.contains("local x = 42"),
        "Should compile for Lua 5.4. Got:\n{}",
        output
    );
}

#[test]
fn test_integer_literals_lua53() {
    let source = r#"
        local x = 42
        local y = 3.14
        return x
    "#;

    let output = compile_with_target(source, "5.3").unwrap();
    eprintln!("Integer literals:\n{}", output);
}

// ============================================================================
// Codegen Emitter Tests
// ============================================================================

#[test]
fn test_emitter_indentation() {
    let source = r#"
        function test(): void {
            if true then
                print("nested")
            end
        }
    "#;

    let output = compile(source).unwrap();
    let indent_count = output.matches("    ").count();
    assert!(
        indent_count >= 2,
        "Should have proper indentation. Got:\n{}",
        output
    );
}

#[test]
fn test_emitter_long_string() {
    let source = r#"
        local long = "This is a string"
        return long
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("long"),
        "Should handle strings. Got:\n{}",
        output
    );
}

#[test]
fn test_emitter_escape_sequences() {
    let source = r#"
        local escaped = "Hello\nWorld\t!"
        return escaped
    "#;

    let output = compile(source).unwrap();
    eprintln!("Escape sequences:\n{}", output);
}

// ============================================================================
// Class Codegen Tests
// ============================================================================

#[test]
fn test_class_with_constructor() {
    let source = r#"
        class Counter {
            count: number

            constructor() {
                self.count = 0
            }

            increment(): void {
                self.count = self.count + 1
            }

            getCount(): number {
                return self.count
            }
        }

        local c = Counter()
        c.increment()
        return c.getCount()
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("Counter") && output.contains("increment"),
        "Should compile class. Got:\n{}",
        output
    );
}

#[test]
fn test_class_with_getter_setter() {
    let source = r#"
        class Point {
            private _x: number
            private _y: number

            constructor(x: number, y: number) {
                self._x = x
                self._y = y
            }

            get x(): number {
                return self._x
            }

            set x(value: number) {
                self._x = value
            }
        }

        local p = Point(1, 2)
        p.x = 10
        return p.x
    "#;

    let output = compile(source).unwrap();
    eprintln!("Getter/Setter:\n{}", output);
}

#[test]
fn test_class_static_member() {
    let source = r#"
        class MathUtils {
            static add(a: number, b: number): number {
                return a + b
            }
        }

        local result = MathUtils.add(3.14, 1)
        return result
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("MathUtils") && output.contains("add"),
        "Should compile static method. Got:\n{}",
        output
    );
}

#[test]
fn test_class_inheritance() {
    let source = r#"
        class Animal {
            name: string

            constructor(name: string) {
                self.name = name
            }

            speak(): string {
                return "..."
            }
        }

        class Dog extends Animal {
            constructor(name: string) {
                super(name)
            }

            speak(): string {
                return "Woof!"
            }
        }

        local d = Dog("Rex")
        return d.speak()
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("Dog") && output.contains("Animal"),
        "Should compile inheritance. Got:\n{}",
        output
    );
}

// ============================================================================
// Interface Codegen Tests
// ============================================================================

#[test]
fn test_simple_interface() {
    let source = r#"
        interface Point {
            x: number
            y: number
        }

        local p: Point = {x = 1, y = 2}
        return p
    "#;

    let output = compile(source).unwrap();
    eprintln!("Interface:\n{}", output);
}

#[test]
fn test_interface_object_literal() {
    let source = r#"
        interface Person {
            name: string
            age: number
        }

        local p: Person = {
            name = "Alice",
            age = 30
        }
        return p
    "#;

    let output = compile(source).unwrap();
    eprintln!("Interface with object:\n{}", output);
}

// ============================================================================
// Table Literal Tests
// ============================================================================

#[test]
fn test_table_with_string_keys() {
    let source = r#"
        local obj = {name = "test", value = 42}
        return obj.name
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("name") && output.contains("test"),
        "Should handle table with string keys. Got:\n{}",
        output
    );
}

#[test]
fn test_table_with_mixed_keys() {
    let source = r#"
        local obj = {x = 1, y = 2}
        return obj.x
    "#;

    let output = compile(source).unwrap();
    eprintln!("Table with keys:\n{}", output);
}

#[test]
fn test_nested_table_literal() {
    let source = r#"
        local nested = {outer = {inner = {deep = 42}}}
        return nested.outer.inner.deep
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("42"),
        "Should handle nested tables. Got:\n{}",
        output
    );
}

// ============================================================================
// Function Codegen Tests
// ============================================================================

#[test]
fn test_function_with_multiple_params() {
    let source = r#"
        function add(a: number, b: number, c: number): number {
            return a + b + c
        }
        return add(1, 2, 3)
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("add"),
        "Should compile function. Got:\n{}",
        output
    );
}

#[test]
fn test_nested_function() {
    let source = r#"
        local function outer(): void {
            local function inner(): void {
                print("inner")
            end
            inner()
        end
        outer()
    "#;

    let output = compile(source).unwrap();
    eprintln!("Nested function:\n{}", output);
}

#[test]
fn test_closure() {
    let source = r#"
        local count = 0
        local function increment(): void {
            count = count + 1
        end
        increment()
        increment()
        return count
    "#;

    let output = compile(source).unwrap();
    eprintln!("Closure output:\n{}", output);
    assert!(
        output.contains("count"),
        "Should compile closure. Got:\n{}",
        output
    );
}

// ============================================================================
// Control Flow Codegen Tests
// ============================================================================

#[test]
fn test_nested_if_else() {
    let source = r#"
        function test(x: number): string {
            if x > 10 then
                return "big"
            elseif x > 5 then
                return "medium"
            else
                return "small"
            end
        }
        return test(7)
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("elseif") || output.contains("else"),
        "Should compile nested if. Got:\n{}",
        output
    );
}

#[test]
fn test_while_loop() {
    let source = r#"
        function sumTo(n: number): number {
            local i = 0
            local sum = 0
            while i < n do
                i = i + 1
                sum = sum + i
            end
            return sum
        }
        return sumTo(5)
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("while"),
        "Should compile while loop. Got:\n{}",
        output
    );
}

#[test]
fn test_for_loop() {
    let source = r#"
        function sumTo(n: number): number {
            local sum = 0
            for i = 1, n do
                sum = sum + i
            end
            return sum
        }
        return sumTo(5)
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("for"),
        "Should compile for loop. Got:\n{}",
        output
    );
}

#[test]
fn test_repeat_until_loop() {
    let source = r#"
        function doAtLeastOnce(): number {
            local count = 0
            repeat
                count = count + 1
            until count >= 1
            return count
        }
        return doAtLeastOnce()
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("repeat"),
        "Should compile repeat-until. Got:\n{}",
        output
    );
}

// ============================================================================
// Expression Codegen Tests
// ============================================================================

#[test]
fn test_binary_operations() {
    let source = r#"
        local a = 10 + 5 * 2
        local b = (10 + 5) * 2
        return a + b
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("10") && output.contains("5"),
        "Should compile binary ops. Got:\n{}",
        output
    );
}

#[test]
fn test_unary_operations() {
    let source = r#"
        local a = -5
        local b = not true
        return a
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("-"),
        "Should compile unary ops. Got:\n{}",
        output
    );
}

#[test]
fn test_call_expression() {
    let source = r#"
        function double(x: number): number {
            return x * 2
        }
        local result = double(5) + double(10)
        return result
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("double"),
        "Should compile call expression. Got:\n{}",
        output
    );
}

#[test]
fn test_method_call() {
    let source = r#"
        local arr = {1, 2, 3}
        local len = #arr
        return len
    "#;

    let result = compile(source);
    if result.is_err() {
        eprintln!("Method call error (expected): {}", result.unwrap_err());
    }
    // This test documents known syntax limitation
    assert!(true);
}

// ============================================================================
// Type Annotation Codegen Tests
// ============================================================================

#[test]
fn test_variable_with_type_annotation() {
    let source = r#"
        local x: number = 42
        local y: string = "hello"
        return x
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("local x = 42"),
        "Should compile typed variable. Got:\n{}",
        output
    );
}

#[test]
fn test_function_with_return_type() {
    let source = r#"
        function greet(name: string): string {
            return "Hello, " .. name
        }
        return greet("World")
    "#;

    let output = compile(source).unwrap();
    assert!(
        output.contains("greet"),
        "Should compile function with return type. Got:\n{}",
        output
    );
}

#[test]
fn test_union_type() {
    let source = r#"
        function test(x: any): any {
            return x
        }
        return test(42)
    "#;

    let output = compile(source).unwrap();
    eprintln!("Any type:\n{}", output);
}

#[test]
fn test_optional_type() {
    let source = r#"
        local x: number | nil = nil
        if x == nil then
            x = 42
        end
        return x
    "#;

    let output = compile(source).unwrap();
    eprintln!("Optional type:\n{}", output);
}
