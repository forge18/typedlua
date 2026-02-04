use typedlua_core::di::DiContainer;

fn compile_and_check(source: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile(source)
}

#[test]
fn test_simple_rest_parameter() {
    let source = r#"
        function sum(...numbers: number): number
            let total = 0
            for n in numbers {
                total = total + n
            }
            return total
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Simple rest parameter should compile");
}

#[test]
fn test_rest_parameter_with_fixed_params() {
    let source = r#"
        function concat(separator: string, ...parts: string): string
            return table.concat(parts, separator)
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest with fixed params should compile");
}

#[test]
fn test_rest_parameter_type() {
    let source = r#"
        function first<T>(first: T, ...rest: T): T {
            return first
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Rest parameter with generic type should compile"
    );
}

#[test]
fn test_rest_parameter_multiple_args() {
    let source = r#"
        function max(first: number, ...rest: number): number {
            let m = first
            for n in rest {
                if n > m {
                    m = n
                }
            }
            return m
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Rest parameter with multiple args should compile"
    );
}

#[test]
fn test_rest_parameter_empty() {
    let source = r#"
        function foo(...args: any): number {
            return #args
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Empty rest parameter should compile");
}

#[test]
fn test_rest_parameter_in_closure() {
    let source = r#"
        const makeAdder = (base: number) => {
            return (...addends: number) => {
                let sum = base
                for a in addends {
                    sum = sum + a
                }
                return sum
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest in closure should compile");
}

#[test]
fn test_rest_parameter_method() {
    let source = r#"
        class Logger {
            public log(level: string, ...message: any): void {
                print(level .. ": " .. tostring(message[0]))
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest in method should compile");
}

#[test]
fn test_rest_parameter_assignment() {
    let source = r#"
        function getAll(...items: string): string[] {
            return items
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest assignment should compile");
}

#[test]
fn test_rest_parameter_in_type() {
    let source = r#"
        function printf(format: string, ...args: any): void {
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest in function type should compile");
}

#[test]
fn test_rest_parameter_variadic() {
    let source = r#"
        function compose<T, U, V>(
            f: (U) => V,
            g: (...T) => U,
            ...xs: T
        ): V[] {
            const results: V[] = []
            for x in xs {
                results.push(f(g(x)))
            }
            return results
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Variadic rest should compile");
}

#[test]
fn test_rest_parameter_nested() {
    let source = r#"
        function outer(a: number, ...b: number[]): number[] {
            function inner(...c: number[]): number[] {
                return c
            }
            return inner(...b)
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Nested rest should compile");
}

#[test]
fn test_rest_parameter_with_table() {
    let source = r#"
        function makeTable(...kv: [string, any]): { [string]: any } {
            const result: { [string]: any } = {}
            for [k, v] in kv {
                result[k] = v
            }
            return result
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest with table should compile");
}

#[test]
fn test_rest_parameter_arrow_function() {
    let source = r#"
        const sum = (...nums: number): number => {
            let s = 0
            for n in nums {
                s = s + n
            }
            return s
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest in arrow function should compile");
}

#[test]
fn test_rest_parameter_generic_variadic() {
    let source = r#"
        function zip<T>(...lists: T[][]): T[][] {
            return lists
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Generic variadic should compile");
}

#[test]
fn test_rest_parameter_destructuring() {
    let source = r#"
        function process(...items: { name: string, value: number }[]): number {
            let total = 0
            for { name, value } in items {
                total = total + value
            }
            return total
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest with destructuring should compile");
}

#[test]
fn test_rest_parameter_length() {
    let source = r#"
        function count<T>(...args: T): number {
            return #args
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest length should compile");
}

#[test]
fn test_rest_parameter_spread() {
    let source = r#"
        function apply<T>(fn: (...T) => void, ...args: T): void {
            fn(...args)
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Rest with spread should compile");
}

#[test]
fn test_rest_parameter_multiple_restrictions() {
    let source = r#"
        function mixed(
            a: string,
            b: number,
            ...rest: boolean[]
        ): [string, number, boolean[]] {
            return [a, b, rest]
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Multiple restrictions should compile");
}
