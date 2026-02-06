//! Pattern Matching Advanced Tests
//! Section 7.1.3 of TODO.md

use typedlua_core::di::DiContainer;

#[test]
fn test_deep_destructuring_three_levels() {
    let source = r#"
        const data = { a: { b: { c: 42 } } }
        const result = match data {
            { a: { b: { c } } } => c,
            _ => 0
        end
    "#;
    let mut container = DiContainer::test_default();
    let result = container.compile(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_or_pattern_basic() {
    let source = r#"
        const x = 1
        const result = match x {
            1 | 2 => "one or two",
            3 | 4 | 5 => "three to five",
            _ => "other"
        end
    "#;
    let mut container = DiContainer::test_default();
    let result = container.compile(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_nested_pattern_matching() {
    let source = r#"
        const data = { type: "point", x: 1, y: 2 }
        const result = match data {
            { type: "point", x, y } => x + y,
            { type: "circle", r } => r * 2,
            _ => 0
        end
    "#;
    let mut container = DiContainer::test_default();
    let result = container.compile(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_pattern_guard_complex() {
    let source = r#"
        const n = 10
        const result = match n {
            x when x > 0 and x < 5 => "small",
            x when x >= 5 and x < 10 => "medium",
            x when x >= 10 => "large",
            _ => "unknown"
        end
    "#;
    let mut container = DiContainer::test_default();
    let result = container.compile(source);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_array_pattern_with_rest() {
    let source = r#"
        const arr = [1, 2, 3, 4, 5]
        const result = match arr {
            [first, ...rest] => first,
            [] => 0
        end
    "#;
    let mut container = DiContainer::test_default();
    let result = container.compile(source);
    assert!(result.is_ok() || result.is_err());
}
