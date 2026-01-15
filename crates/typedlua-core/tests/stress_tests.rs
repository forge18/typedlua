// Simplified stress tests - tests parser/lexer don't panic on large inputs
use std::sync::Arc;
use typedlua_core::{diagnostics::CollectingDiagnosticHandler, lexer::Lexer, parser::Parser};

fn lex_and_parse(input: &str) -> bool {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new(input, handler.clone());
    if let Ok(tokens) = lexer.tokenize() {
        let mut parser = Parser::new(tokens, handler);
        parser.parse().is_ok()
    } else {
        false
    }
}

#[test]
fn test_deeply_nested_expressions() {
    // Test with 100 levels - should work now with iterative parsing
    let mut input = String::from("const x: number = ");
    for _ in 0..100 {
        input.push_str("(");
    }
    input.push_str("42");
    for _ in 0..100 {
        input.push_str(")");
    }
    assert!(
        lex_and_parse(&input),
        "Should parse deeply nested expressions (100 levels)"
    );
}

#[test]
fn test_extremely_deep_nesting() {
    // Test with 500 levels to ensure no stack overflow
    let mut input = String::from("const x: number = ");
    for _ in 0..500 {
        input.push_str("(");
    }
    input.push_str("42");
    for _ in 0..500 {
        input.push_str(")");
    }
    assert!(
        lex_and_parse(&input),
        "Should parse extremely deep nesting (500 levels)"
    );
}

#[test]
fn test_deeply_nested_blocks() {
    let mut input = String::new();
    for _ in 0..30 {
        input.push_str("if true then\n");
    }
    input.push_str("const x: number = 42\n");
    for _ in 0..30 {
        input.push_str("end\n");
    }
    assert!(lex_and_parse(&input), "Should parse deeply nested blocks");
}

#[test]
fn test_large_number_of_variables() {
    let mut input = String::new();
    for i in 0..1000 {
        input.push_str(&format!("const var{}: number = {}\n", i, i));
    }
    assert!(
        lex_and_parse(&input),
        "Should parse many variable declarations"
    );
}

#[test]
fn test_large_array_literal() {
    let mut input = String::from("const arr: number[] = {");
    for i in 0..5000 {
        if i > 0 {
            input.push_str(", ");
        }
        input.push_str(&i.to_string());
    }
    input.push_str("}");
    assert!(lex_and_parse(&input), "Should parse large array literal");
}

#[test]
fn test_large_object_literal() {
    let mut input = String::from("const obj = {");
    for i in 0..1000 {
        if i > 0 {
            input.push_str(", ");
        }
        input.push_str(&format!("prop{} = {}", i, i));
    }
    input.push_str("}");
    assert!(lex_and_parse(&input), "Should parse large object literal");
}

#[test]
fn test_long_function_chain() {
    let mut input = String::from("const result = obj");
    for i in 0..100 {
        input.push_str(&format!(".method{}()", i));
    }
    assert!(lex_and_parse(&input), "Should parse long function chain");
}

#[test]
fn test_large_type_union() {
    let mut input = String::from("type BigUnion = ");
    for i in 0..100 {
        if i > 0 {
            input.push_str(" | ");
        }
        input.push_str(&format!("Type{}", i));
    }
    assert!(lex_and_parse(&input), "Should parse large type union");
}

#[test]
fn test_long_binary_expression_chain() {
    let mut input = String::from("const result: number = 1");
    for i in 2..=200 {
        input.push_str(&format!(" + {}", i));
    }
    assert!(
        lex_and_parse(&input),
        "Should parse long binary expression chain"
    );
}

#[test]
fn test_many_function_parameters() {
    let mut input = String::from("function test(");
    for i in 0..100 {
        if i > 0 {
            input.push_str(", ");
        }
        input.push_str(&format!("param{}: number", i));
    }
    input.push_str("): void end");
    assert!(
        lex_and_parse(&input),
        "Should parse function with many parameters"
    );
}

#[test]
fn test_large_class_with_many_members() {
    let mut input = String::from("class BigClass {\n");
    for i in 0..100 {
        input.push_str(&format!("    prop{}: number\n", i));
    }
    for i in 0..100 {
        input.push_str(&format!("    method{}(): void {{}}\n", i));
    }
    input.push_str("}");
    assert!(lex_and_parse(&input), "Should parse large class");
}

#[test]
fn test_lexer_with_very_long_identifier() {
    let long_name = "a".repeat(1000);
    let input = format!("const {}: number = 42", long_name);

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new(&input, handler);
    assert!(
        lexer.tokenize().is_ok(),
        "Should tokenize very long identifier"
    );
}

#[test]
fn test_lexer_with_very_long_string() {
    let long_string = "x".repeat(10000);
    let input = format!("const msg: string = \"{}\"", long_string);

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new(&input, handler);
    assert!(lexer.tokenize().is_ok(), "Should tokenize very long string");
}

#[test]
fn test_extremely_long_comment() {
    let long_comment = "x".repeat(10000);
    let input = format!("-- {}\nconst x: number = 42", long_comment);

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new(&input, handler);
    assert!(lexer.tokenize().is_ok(), "Should handle very long comment");
}

#[test]
fn test_many_imports() {
    let mut input = String::new();
    for i in 0..100 {
        input.push_str(&format!("import {{ Module{} }} from \"module{}\"\n", i, i));
    }
    assert!(lex_and_parse(&input), "Should parse many imports");
}

#[test]
fn test_parser_doesnt_panic_on_errors() {
    // These inputs have errors but shouldn't cause panics
    let inputs = vec![
        "const x: number =",
        "function broken(a: number, b: number",
        "class Incomplete {",
        "if true then",
        "match x {",
    ];

    for input in inputs {
        let _ = lex_and_parse(input); // Don't care about result, just shouldn't panic
    }
}
