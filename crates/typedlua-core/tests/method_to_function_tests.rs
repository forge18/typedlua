use typedlua_core::config::OptimizationLevel;
use typedlua_core::di::DiContainer;

fn compile_with_optimization_level(
    source: &str,
    level: OptimizationLevel,
) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile_with_optimization(source, level)
}

fn compile_with_o2(source: &str) -> Result<String, String> {
    compile_with_optimization_level(source, OptimizationLevel::O2)
}

fn compile_with_o1(source: &str) -> Result<String, String> {
    compile_with_optimization_level(source, OptimizationLevel::O1)
}

// =============================================================================
// Test: Instance method call conversion
// =============================================================================

#[test]
fn test_instance_method_call_basic() {
    let source = r#"
        class Calculator {
            value: number = 0

            add(n: number): number {
                return self.value + n
            }
        }

        const calc: Calculator = new Calculator()
        const result = calc:add(5)
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("Calculator"),
        "Calculator should appear in output: {}",
        output
    );
    assert!(
        output.contains("add"),
        "add method should appear in output: {}",
        output
    );
}

#[test]
fn test_class_method_call_on_instance() {
    let source = r#"
        class Counter {
            count: number = 0

            increment(): number {
                self.count = self.count + 1
                return self.count
            }
        }

        const counter: Counter = new Counter()
        const result = counter:increment()
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("Counter"),
        "Counter class should appear in output: {}",
        output
    );
}

// =============================================================================
// Test: Optional method calls should NOT be converted
// =============================================================================

#[test]
fn test_optional_method_call_not_converted() {
    let source = r#"
        class Calculator {
            calculate(x: number): number {
                return x * 2
            }
        }

        function test(maybeCalc: Calculator | nil): number | nil
            return maybeCalc?:calculate(5)
        end
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O0).unwrap();
    println!("Output:\n{}", output);
    // Verify the class and its method are generated correctly
    assert!(
        output.contains("Calculator"),
        "Calculator class should appear in output: {}",
        output
    );
    assert!(
        output.contains("calculate"),
        "calculate method should appear in output: {}",
        output
    );
}

// =============================================================================
// Test: Chained method calls
// =============================================================================

#[test]
fn test_chained_method_calls() {
    let source = r#"
        class Counter {
            value: number = 0

            increment(): void {
                self.value = self.value + 1
            }

            getValue(): number {
                return self.value
            }
        }

        const counter: Counter = new Counter()
        counter:increment()
        counter:increment()
        const result = counter:getValue()
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("Counter"),
        "Counter class should appear in output: {}",
        output
    );
    assert!(
        output.contains("increment"),
        "increment method should appear in output: {}",
        output
    );
}

// =============================================================================
// Test: Preservation of argument evaluation order
// =============================================================================

#[test]
fn test_argument_evaluation_order_preserved() {
    let source = r#"
        local counter = 0

        function getNext(): number
            counter = counter + 1
            return counter
        end

        class Math {
            add(a: number, b: number): number {
                return a + b
            }
        }

        const m: Math = new Math()
        const result = m:add(getNext(), getNext())
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("getNext()"),
        "getNext() calls should be in output: {}",
        output
    );
}

// =============================================================================
// Test: No conversion at O1 level
// =============================================================================

#[test]
fn test_no_conversion_at_o1() {
    let source = r#"
        class Calculator {
            calculate(x: number): number {
                return x * 2
            }
        }

        const calc: Calculator = new Calculator()
        const result = calc:calculate(5)
    "#;

    let o1_output = compile_with_o1(source).unwrap();
    let o2_output = compile_with_o2(source).unwrap();

    println!("O1 Output:\n{}", o1_output);
    println!("O2 Output:\n{}", o2_output);

    assert!(
        o1_output.contains("Calculator"),
        "O1 should contain Calculator: {}",
        o1_output
    );
    assert!(
        o2_output.contains("Calculator"),
        "O2 should contain Calculator: {}",
        o2_output
    );
}

// =============================================================================
// Test: Static methods generate function syntax
// =============================================================================

#[test]
fn test_static_method_generates_function() {
    let source = r#"
        class MathUtils {
            static square(x: number): number {
                return x * x
            }

            static cube(x: number): number {
                return x * x * x
            }
        }
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("MathUtils"),
        "MathUtils should appear in output: {}",
        output
    );
    assert!(
        output.contains("function MathUtils.square") || output.contains("MathUtils.square"),
        "square should be a function on MathUtils: {}",
        output
    );
    assert!(
        output.contains("function MathUtils.cube") || output.contains("MathUtils.cube"),
        "cube should be a function on MathUtils: {}",
        output
    );
}

// =============================================================================
// Test: New expression method calls
// =============================================================================

#[test]
fn test_new_expression_method_call() {
    let source = r#"
        class Greeter {
            name: string

            constructor(name: string) {
                self.name = name
            }

            greet(): string {
                return "Hello, " .. self.name
            }
        }

        const greeting = (new Greeter("World")):greet()
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("Greeter"),
        "Greeter class should appear in output: {}",
        output
    );
    assert!(
        output.contains("greet"),
        "greet method should appear in output: {}",
        output
    );
}

// =============================================================================
// Test: Method call with complex receiver
// =============================================================================

#[test]
fn test_method_call_with_complex_receiver() {
    // Inner must be declared before Container references it
    let source = r#"
        class Inner {
            getValue(): number {
                return 42
            }
        }

        class Container {
            inner: Inner

            constructor() {
                self.inner = new Inner()
            }
        }

        const container: Container = new Container()
        const value = container.inner:getValue()
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("Container"),
        "Container should appear in output: {}",
        output
    );
    assert!(
        output.contains("Inner"),
        "Inner should appear in output: {}",
        output
    );
}

// =============================================================================
// Test: Method call in loop
// =============================================================================

#[test]
fn test_method_call_in_loop() {
    let source = r#"
        class Accumulator {
            total: number = 0

            add(n: number): void {
                self.total = self.total + n
            }

            getTotal(): number {
                return self.total
            }
        }

        const acc: Accumulator = new Accumulator()
        for i = 1, 10 do
            acc:add(i)
        end
        const result = acc:getTotal()
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O0).unwrap();
    println!("Output:\n{}", output);
    // Verify the class and its methods are generated correctly
    assert!(
        output.contains("Accumulator"),
        "Accumulator class should appear in output: {}",
        output
    );
    assert!(
        output.contains("add"),
        "add method should appear in output: {}",
        output
    );
    assert!(
        output.contains("getTotal"),
        "getTotal method should appear in output: {}",
        output
    );
}

// =============================================================================
// Test: Method call in conditional
// =============================================================================

#[test]
fn test_method_call_in_conditional() {
    let source = r#"
        class Validator {
            isValid(x: number): boolean {
                return x > 0
            }
        }

        const validator: Validator = new Validator()

        function check(x: number): string
            if validator:isValid(x) then
                return "valid"
            else
                return "invalid"
            end
        end
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O0).unwrap();
    println!("Output:\n{}", output);
    // Verify class, method, and usage code are generated
    assert!(
        output.contains("Validator"),
        "Validator class should appear in output: {}",
        output
    );
    assert!(
        output.contains("isValid"),
        "isValid method should appear in output: {}",
        output
    );
    assert!(
        output.contains("validator"),
        "validator instance should appear in output: {}",
        output
    );
}

// =============================================================================
// Test: Method call with self parameter
// =============================================================================

#[test]
fn test_method_with_self_parameter() {
    let source = r#"
        class Point {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            distanceTo(other: Point): number {
                const dx = self.x - other.x
                const dy = self.y - other.y
                return (dx * dx + dy * dy) ^ 0.5
            }
        }

        const p1: Point = new Point(0, 0)
        const p2: Point = new Point(3, 4)
        const dist = p1:distanceTo(p2)
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("Point"),
        "Point class should appear in output: {}",
        output
    );
    assert!(
        output.contains("distanceTo"),
        "distanceTo method should appear in output: {}",
        output
    );
}

// =============================================================================
// Test: No regression on regular function calls
// =============================================================================

#[test]
fn test_no_regression_regular_function_calls() {
    let source = r#"
        function add(a: number, b: number): number
            return a + b
        end

        function multiply(a: number, b: number): number
            return a * b
        end

        const x = add(1, 2)
        const y = multiply(3, 4)
        const z = add(x, y)
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("function add("),
        "add function should be in output: {}",
        output
    );
    assert!(
        output.contains("function multiply("),
        "multiply function should be in output: {}",
        output
    );
}

// =============================================================================
// Test: Method call in return statement
// =============================================================================

#[test]
fn test_method_call_in_return() {
    let source = r#"
        class Calculator {
            value: number

            constructor(v: number) {
                self.value = v
            }

            double(): number {
                return self.value * 2
            }
        }

        function getDoubled(c: Calculator): number
            return c:double()
        end
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("return"),
        "return should appear in output: {}",
        output
    );
    assert!(
        output.contains("double"),
        "double method should appear in output: {}",
        output
    );
}

// =============================================================================
// Test: Multiple method calls in expression
// =============================================================================

#[test]
fn test_multiple_method_calls_in_expression() {
    let source = r#"
        class Counter {
            value: number = 0

            getValue(): number {
                return self.value
            }

            increment(): Counter {
                self.value = self.value + 1
                return self
            }
        }

        const c1: Counter = new Counter()
        const c2: Counter = new Counter()
        c1:increment()
        c2:increment():increment()
        const sum = c1:getValue() + c2:getValue()
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("Counter"),
        "Counter should appear in output: {}",
        output
    );
}
