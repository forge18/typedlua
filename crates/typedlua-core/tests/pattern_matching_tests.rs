use typedlua_core::di::DiContainer;

fn compile_and_check(source: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile(source)
}

#[test]
fn test_simple_literal_match() {
    let source = r#"
        const x = 5
        const result = match x {
            1 => "one",
            2 => "two",
            5 => "five",
            _ => "other"
        }
    "#;

    let result = compile_and_check(source);
    if let Err(e) = &result {
        eprintln!("Error: {}", e);
    }
    assert!(result.is_ok(), "Simple literal match should compile");
    let output = result.unwrap();

    assert!(output.contains("(function()"), "Should generate an IIFE");
}

#[test]
fn test_match_with_boolean() {
    let source = r#"
        const flag = true
        const result = match flag {
            true => "yes"
            false => "no"
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Boolean match should compile");
}

#[test]
fn test_match_with_enum() {
    let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
        }

        const c: Color = Color.Red
        const result = match c {
            Red => "red"
            Green => "green"
            Blue => "blue"
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Enum match should compile");
}

#[test]
fn test_match_with_string() {
    let source = r#"
        const status = "active"
        const result = match status {
            "active" => 1
            "inactive" => 0
            _ => -1
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "String match should compile");
}

#[test]
fn test_match_with_guard() {
    let source = r#"
        const x: number = 15
        const result = match x {
            n if n > 10 => "big"
            n if n > 5 => "medium"
            _ => "small"
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match with guard should compile");
}

#[test]
fn test_match_with_destructuring() {
    let source = r#"
        const point = { x: 10, y: 20 }
        const result = match point {
            { x: 10, y: 20 } => "origin"
            { x } if x > 0 => "positive x"
            _ => "other"
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match with destructuring should compile");
}

#[test]
fn test_match_nested() {
    let source = r#"
        const data = { nested: { value: 42 } }
        const result = match data {
            { nested: { value: 42 } } => "found"
            _ => "not found"
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Nested match should compile");
}

#[test]
fn test_match_array() {
    let source = r#"
        const arr = [1, 2, 3]
        const result = match arr {
            [1, 2, 3] => "one two three"
            [1, 2] => "one two"
            _ => "other"
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match array should compile");
}

#[test]
fn test_match_rest_pattern() {
    let source = r#"
        const arr = [1, 2, 3, 4, 5]
        const result = match arr {
            [first, ...rest] => first
            _ => 0
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match with rest should compile");
}

#[test]
fn test_match_union_type() {
    let source = r#"
        const x: number | string = 42
        const result = match x {
            n: number => n * 2
            s: string => #s
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match union type should compile");
}

#[test]
fn test_match_with_capture() {
    let source = r#"
        const result = match 42 {
            n => n * 2
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match with capture should compile");
}

#[test]
fn test_match_in_expression() {
    let source = r#"
        const x = 5
        const result = match x {
            1 => 10
            2 => 20
            3 => 30
            _ => 0
        } + 5
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match in expression should compile: {:?}", result.err());
}

#[test]
fn test_match_function_result() {
    let source = r#"
        function getValue(): number
            return 42
        end

        const result = match getValue() {
            42 => "answer"
            _ => "other"
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match function result should compile");
}

#[test]
fn test_match_table_with_rest() {
    let source = r#"
        const t = { a: 1, b: 2, c: 3 }
        const result = match t {
            { a, ...rest } => a
            _ => 0
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match table with rest should compile");
}

#[test]
fn test_match_nested_guard() {
    let source = r#"
        const point = { x: 15, y: 20 }
        const result = match point {
            { x, y } if x > 10 and y > 15 => " quadrant 1"
            { x, y } if x <= 10 and y > 15 => "quadrant 2"
            _ => "other"
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Nested guard should compile");
}

#[test]
fn test_match_type_annotation() {
    let source = r#"
        const result: string = match 42 {
            n: number => "number"
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match type annotation should compile");
}

#[test]
fn test_match_skipped_values() {
    let source = r#"
        const arr = [1, 2, 3, 4, 5]
        const result = match arr {
            [first, , , , last] => first + last
            _ => 0
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match skipped values should compile");
}

#[test]
fn test_match_object_with_methods() {
    let source = r#"
        const obj = { type: "circle", radius: 10 }
        const result = match obj {
            { type: "circle", radius: r } => 3.14 * r * r
            { type: "square", size: s } => s * s
            _ => 0
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match object with methods should compile");
}

#[test]
fn test_match_with_nil() {
    let source = r#"
        const x: number | nil = nil
        const result = match x {
            nil => "no value"
            n: number => "has value"
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match with nil should compile");
}

#[test]
fn test_match_tuple_pattern() {
    let source = r#"
        const point = [10, 20]
        const result = match point {
            [x, y] => x + y
            _ => 0
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Match tuple pattern should compile");
}
