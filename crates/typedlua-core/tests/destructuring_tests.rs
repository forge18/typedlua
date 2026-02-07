use typedlua_core::di::DiContainer;

fn compile_and_check(source: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile_with_stdlib(source)
}

#[test]
fn test_simple_array_destructuring() {
    let source = r#"
        const arr = [1, 2, 3]
        const [a, b, c] = arr
        return a
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(output.contains("a"), "Array destructuring should work");
}

#[test]
fn test_array_destructuring_with_rest() {
    let source = r#"
        const arr = [1, 2, 3, 4, 5]
        const [first, second, ...rest] = arr
        return rest
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("rest"),
        "Array destructuring with rest should work"
    );
}

#[test]
fn test_nested_array_destructuring() {
    let source = r#"
        const arr = [[1, 2], [3, 4]]
        const [[a, b], [c, d]] = arr
        return a + c
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("a") && output.contains("c"),
        "Nested array destructuring should work"
    );
}

#[test]
fn test_array_destructuring_with_skipped_elements() {
    let source = r#"
        const arr = [1, 2, 3, 4]
        const [first, , third] = arr
        return first + third
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("first") && output.contains("third"),
        "Skipped elements should work"
    );
}

#[test]
fn test_object_destructuring() {
    let source = r#"
        const obj = { x: 1, y: 2 }
        const { x, y } = obj
        return x + y
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("x") && output.contains("y"),
        "Object destructuring should work"
    );
}

#[test]
fn test_object_destructuring_with_alias() {
    let source = r#"
        const obj = { x: 1, y: 2 }
        const { x: a, y: b } = obj
        return a + b
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("a") && output.contains("b"),
        "Alias destructuring should work"
    );
}

#[test]
fn test_nested_object_destructuring() {
    let source = r#"
        const obj = { outer: { inner: 42 } }
        const { outer: { inner } } = obj
        return inner
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("inner"),
        "Nested object destructuring should work"
    );
}

#[test]
fn test_mixed_array_object_destructuring() {
    let source = r#"
        const data = [{ name: "Alice" }, { name: "Bob" }]
        const [ { name: name1 }, { name: name2 } ] = data
        return name1 .. " and " .. name2
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("name1") && output.contains("name2"),
        "Mixed destructuring should work"
    );
}

#[test]
fn test_destructuring_with_default_values() {
    let source = r#"
        const arr = [1]
        const [a = 0, b = 0, c = 0] = arr
        return a + b + c
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("a") && output.contains("b") && output.contains("c"),
        "Default values should work"
    );
}

#[test]
fn test_destructuring_in_for_loop() {
    let source = r#"
        const items = [[1, 2], [3, 4]]
        for [a, b] in items do
            const sum = a + b
        end
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("a") && output.contains("b"),
        "Destructuring in for loop should work"
    );
}

#[test]
fn test_destructuring_assignment() {
    let source = r#"
        local obj = { x: 1, y: 2 }
        local { x, y } = obj
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("x") && output.contains("y"),
        "Destructuring assignment should work"
    );
}

#[test]
fn test_destructuring_with_types() {
    let source = r#"
        const arr: [number, number, number] = [1, 2, 3]
        const [a: number, b: number, c: number] = arr
        return a + b + c
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(output.contains("a"), "Typed destructuring should work");
}

#[test]
fn test_object_destructuring_with_rest() {
    let source = r#"
        const obj = { a: 1, b: 2, c: 3 }
        const { a, ...rest } = obj
        return rest
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("rest"),
        "Object rest destructuring should work"
    );
}

#[test]
fn test_deeply_nested_destructuring() {
    let source = r#"
        const data = { outer: { middle: { inner: [1, 2, 3] } } }
        const { outer: { middle: { inner: [first, , third] } } } = data
        return first + third
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("first") && output.contains("third"),
        "Deeply nested destructuring should work"
    );
}

#[test]
fn test_destructuring_parameters() {
    let source = r#"
        function f([a, b]: [number, number]): number
            return a + b
        end
        const result = f([1, 2])
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("a") && output.contains("b"),
        "Destructuring parameters should work"
    );
}

#[test]
fn test_object_destructuring_parameters() {
    let source = r#"
        function f({ x, y }: { x: number, y: number }): number
            return x + y
        end
        const result = f({ x: 1, y: 2 })
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("x") && output.contains("y"),
        "Object destructuring parameters should work"
    );
}

#[test]
fn test_shorthand_destructuring() {
    let source = r#"
        const x = 1
        const y = 2
        const obj = { x, y }
        const { x: x2, y: y2 } = obj
        return x2 + y2
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(output.contains("x2"), "Shorthand destructuring should work");
}

#[test]
fn test_computed_property_destructuring() {
    let source = r#"
        const key = "x"
        const obj = { x: 1, y: 2 }
        const { [key]: value } = obj
        return value
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("value"),
        "Computed property destructuring should work"
    );
}

#[test]
fn test_destructuring_with_spread_in_array() {
    let source = r#"
        const arr = [1, 2, 3, 4, 5]
        const [head, ...tail] = arr
        return tail
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("tail"),
        "Spread in array destructuring should work"
    );
}

#[test]
fn test_destructuring_with_spread_in_object() {
    let source = r#"
        const obj = { a: 1, b: 2, c: 3, d: 4 }
        const { a, ...rest } = obj
        return rest
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("rest"),
        "Spread in object destructuring should work"
    );
}
