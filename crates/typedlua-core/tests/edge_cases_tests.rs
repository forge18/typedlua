use bumpalo::Bump;
use std::sync::Arc;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn lex_only(input: &str) -> bool {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, _common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(input, handler, &interner);
    lexer.tokenize().is_ok()
}

fn lex_and_parse(input: &str) -> bool {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let arena = Bump::new();
    let mut lexer = Lexer::new(input, handler.clone(), &interner);
    if let Ok(tokens) = lexer.tokenize() {
        let mut parser = Parser::new(tokens, handler, &interner, &common_ids, &arena);
        parser.parse().is_ok()
    } else {
        false
    }
}

#[test]
fn test_empty_input() {
    assert!(lex_and_parse(""), "Should handle empty input");
}

#[test]
fn test_only_whitespace() {
    assert!(
        lex_and_parse("   \n\t\r\n   "),
        "Should handle only whitespace"
    );
}

#[test]
fn test_only_comments() {
    let input = "-- This is a comment\n-- Another comment\n--[[ Multi-line comment ]]--";
    assert!(lex_and_parse(input), "Should handle only comments");
}

#[test]
fn test_unicode_in_strings() {
    assert!(
        lex_and_parse(r#"const msg: string = "Hello 世界 🌍""#),
        "Should handle unicode in strings"
    );
}

#[test]
fn test_number_edge_cases() {
    let inputs = vec![
        "const x: number = 0",
        "const x: number = 0.0",
        "const x: number = 1e10",
        "const x: number = 0xFF",
        "const x: number = 0b1111",
    ];
    for input in inputs {
        assert!(lex_only(input), "Should parse number: {}", input);
    }
}

#[test]
fn test_string_edge_cases() {
    let inputs = vec![
        r#"const x: string = """#,
        r#"const x: string = "\\""#,
        r#"const x: string = "\n\r\t""#,
    ];
    for input in inputs {
        assert!(lex_only(input), "Should parse string: {}", input);
    }
}

#[test]
fn test_operator_edge_cases() {
    let inputs = vec![
        "const x = a + b",
        "const x = a * b",
        "const x = a // b",
        "const x = a << b",
        "const x = a and b",
        "const x = not a",
        "const x = #a",
    ];
    for input in inputs {
        assert!(lex_only(input), "Should parse operator: {}", input);
    }
}

#[test]
fn test_empty_blocks() {
    let inputs = vec![
        "if true then end",
        "while true do end",
        "function test() end",
        "class Test {}",
    ];
    for input in inputs {
        assert!(lex_and_parse(input), "Should parse empty block: {}", input);
    }
}

#[test]
fn test_empty_collections() {
    let inputs = vec!["const arr: number[] = {}", "const obj: {} = {}"];
    for input in inputs {
        assert!(
            lex_and_parse(input),
            "Should parse empty collection: {}",
            input
        );
    }
}

#[test]
fn test_nested_parentheses() {
    assert!(
        lex_and_parse("const x = (((42)))"),
        "Should parse nested parentheses"
    );
}

#[test]
fn test_nil_handling() {
    let inputs = vec![
        "const x = nil",
        "const x: nil = nil",
        "const x: number? = nil",
        "if x == nil then end",
    ];
    for input in inputs {
        assert!(lex_and_parse(input), "Should handle nil: {}", input);
    }
}

#[test]
fn test_boolean_literals() {
    let inputs = vec![
        "const x = true",
        "const x = false",
        "const x: boolean = true",
    ];
    for input in inputs {
        assert!(lex_and_parse(input), "Should parse boolean: {}", input);
    }
}

#[test]
fn test_arrow_functions() {
    let inputs = vec![
        "const f = () => 42",
        "const f = (x) => x",
        "const f = x => x",
    ];
    for input in inputs {
        assert!(
            lex_and_parse(input),
            "Should parse arrow function: {}",
            input
        );
    }
}

#[test]
fn test_conditional_expressions() {
    let inputs = vec!["const x = true ? 1 : 2", "const x = a > b ? a : b"];
    for input in inputs {
        assert!(lex_and_parse(input), "Should parse conditional: {}", input);
    }
}

#[test]
fn test_member_access() {
    let inputs = vec![
        "const x = obj.prop",
        "const x = obj[0]",
        "const x = obj['key']",
    ];
    for input in inputs {
        assert!(
            lex_and_parse(input),
            "Should parse member access: {}",
            input
        );
    }
}

#[test]
fn test_function_calls() {
    let inputs = vec!["func()", "func(1, 2, 3)", "obj.method()", "obj:method()"];
    for input in inputs {
        assert!(
            lex_and_parse(input),
            "Should parse function call: {}",
            input
        );
    }
}

#[test]
fn test_destructuring() {
    let inputs = vec![
        "const {x} = obj",
        "const {x, y} = obj",
        "const {a, b, c} = arr",
    ];
    for input in inputs {
        assert!(
            lex_and_parse(input),
            "Should parse destructuring: {}",
            input
        );
    }
}

#[test]
fn test_spread_operator() {
    let inputs = vec!["const arr = {...other}", "const obj = {...other}"];
    for input in inputs {
        assert!(lex_and_parse(input), "Should parse spread: {}", input);
    }
}

#[test]
fn test_generics() {
    let inputs = vec![
        "type Box<T> = T",
        "function id<T>(x: T): T return x end",
        "class Container<T> { value: T }",
    ];
    for input in inputs {
        assert!(lex_and_parse(input), "Should parse generic: {}", input);
    }
}

#[test]
fn test_classes() {
    let inputs = vec![
        "class Empty {}",
        "class WithProp { x: number }",
        "class Child extends Parent {}",
    ];
    for input in inputs {
        assert!(lex_and_parse(input), "Should parse class: {}", input);
    }
}

#[test]
fn test_interfaces() {
    let inputs = vec![
        "interface Empty {}",
        "interface WithProp { x: number }",
        "interface Child extends Parent {}",
    ];
    for input in inputs {
        assert!(lex_and_parse(input), "Should parse interface: {}", input);
    }
}

#[test]
fn test_match_expressions() {
    let inputs = vec![
        "const x = match val { 1 => true }",
        "const x = match val { _ => default }",
    ];
    for input in inputs {
        assert!(lex_and_parse(input), "Should parse match: {}", input);
    }
}

#[test]
fn test_pipe_operator() {
    let inputs = vec!["const x = val |> func", "const x = val |> func1 |> func2"];
    for input in inputs {
        assert!(lex_and_parse(input), "Should parse pipe: {}", input);
    }
}

// ============================================================================
// General Edge Cases (Section 7.1.3)
// ============================================================================

#[test]
fn test_very_long_identifier() {
    let long_name = "a".repeat(100);
    let input = format!("const {} = 42", long_name);
    assert!(lex_and_parse(&input), "Should handle very long identifier");
}

#[test]
fn test_deeply_nested_expressions() {
    let input = "const x = ".to_string() + &"((((1 + ".repeat(10) + "2" + &"))))".repeat(10);
    assert!(
        lex_and_parse(&input),
        "Should handle deeply nested expressions"
    );
}

#[test]
fn test_huge_number_literal() {
    let input = "const x = 999999999999999999999";
    assert!(lex_only(input), "Should handle huge number literal");
}

#[test]
fn test_empty_union_type() {
    let input = "type Never = never";
    assert!(lex_and_parse(input), "Should parse empty/never type");
}

#[test]
fn test_recursive_type_alias() {
    let input = "type LinkedList<T> = { value: T, next: LinkedList<T>? }";
    assert!(lex_and_parse(input), "Should parse recursive type alias");
}

#[test]
fn test_self_referential_class() {
    let input = "class Node { value: number, next: Node? }";
    assert!(lex_and_parse(input), "Should parse self-referential class");
}

#[test]
fn test_tuple_length_extremes() {
    let inputs = vec![
        "type Empty = []",
        "type Single = [number]",
        "type Pair = [number, string]",
        "type Ten = [number, number, number, number, number, number, number, number, number, number]",
    ];
    for input in inputs {
        assert!(lex_and_parse(input), "Should parse tuple: {}", input);
    }
}
