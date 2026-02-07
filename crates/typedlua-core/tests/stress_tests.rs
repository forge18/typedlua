use bumpalo::Bump;
use std::sync::Arc;
use typedlua_core::di::DiContainer;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

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

fn compile_and_check(input: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile(input)
}

#[test]
fn test_deeply_nested_expressions() {
    // Test with 100 levels - should work now with iterative parsing
    let mut input = String::from("const x: number = ");
    for _ in 0..100 {
        input.push('(');
    }
    input.push_str("42");
    for _ in 0..100 {
        input.push(')');
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
        input.push('(');
    }
    input.push_str("42");
    for _ in 0..500 {
        input.push(')');
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
    input.push('}');
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
    input.push('}');
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
    input.push('}');
    assert!(lex_and_parse(&input), "Should parse large class");
}

#[test]
fn test_lexer_with_very_long_identifier() {
    let long_name = "a".repeat(1000);
    let input = format!("const {}: number = 42", long_name);

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, _common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(&input, handler, &interner);
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
    let (interner, _common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(&input, handler, &interner);
    assert!(lexer.tokenize().is_ok(), "Should tokenize very long string");
}

#[test]
fn test_extremely_long_comment() {
    let long_comment = "x".repeat(10000);
    let input = format!("-- {}\nconst x: number = 42", long_comment);

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, _common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(&input, handler, &interner);
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

// ============================================================================
// Expanded Stress Tests (10K+ scale)
// ============================================================================

#[test]
fn test_large_array_literal_10k() {
    let mut input = String::from("const arr: number[] = {");
    for i in 0..10000 {
        if i > 0 {
            input.push_str(", ");
        }
        input.push_str(&i.to_string());
    }
    input.push('}');
    assert!(
        lex_and_parse(&input),
        "Should parse large array literal with 10K elements"
    );
}

#[test]
fn test_large_object_literal_10k() {
    let mut input = String::from("const obj = {");
    for i in 0..10000 {
        if i > 0 {
            input.push_str(", ");
        }
        input.push_str(&format!("prop{}: {}", i, i % 100));
    }
    input.push('}');
    assert!(
        lex_and_parse(&input),
        "Should parse large object literal with 10K properties"
    );
}

#[test]
fn test_deep_class_inheritance_20_levels() {
    let mut input = String::new();
    input.push_str("class Level0 { method(): void {} }\n");

    for i in 1..=25 {
        input.push_str(&format!(
            "class Level{} extends Level{} {{ method(): void {{}} }}\n",
            i,
            i - 1
        ));
    }

    assert!(
        compile_and_check(&input).is_ok(),
        "Should parse and type-check deep class inheritance (25 levels): {:?}",
        compile_and_check(&input).err()
    );
}

#[test]
fn test_complex_nested_generics_10_layers() {
    let input = r#"
        type Value1<T> = { value: T }
        type Value2<T> = Value1<T>
        type Value3<T> = Value2<T>
        type Value4<T> = Value3<T>
        type Value5<T> = Value4<T>
        type Value6<T> = Value5<T>
        type Value7<T> = Value6<T>
        type Value8<T> = Value7<T>
        type Value9<T> = Value8<T>
        type Value10<T> = Value9<T>
        type Value11<T> = Value10<T>

        type NestedString = Value11<string>
        type NestedNumber = Value11<number>
        type NestedBoolean = Value11<boolean>

        const s: NestedString = { value: "test" }
        const n: NestedNumber = { value: 42 }
        const b: NestedBoolean = { value: true }
    "#;

    assert!(
        compile_and_check(input).is_ok(),
        "Should parse and type-check 10+ layers of nested generics: {:?}",
        compile_and_check(input).err()
    );
}

#[test]
fn test_deeply_nested_generics_with_constraints() {
    let input = r#"
        type Container1<T> = { data: T }
        type Container2<T extends Container1<T>> = Container1<T>
        type Container3<T extends Container2<T>> = Container2<T>
        type Container4<T extends Container3<T>> = Container3<T>
        type Container5<T extends Container4<T>> = Container4<T>

        interface Wrapped { inner: number }

        const container: Container5<Wrapped> = { data: { inner: 42 } }
    "#;

    assert!(
        compile_and_check(input).is_ok(),
        "Should parse deeply nested generics with constraints: {:?}",
        compile_and_check(input).err()
    );
}

#[test]
fn test_long_method_chain_50_calls() {
    let mut input = String::from("const result = obj");
    for i in 0..50 {
        input.push_str(&format!(".method{}()", i));
    }
    assert!(
        lex_and_parse(&input),
        "Should parse long method chain with 50+ method calls"
    );
}

#[test]
fn test_long_method_chain_100_calls() {
    let mut input = String::from("const result = obj");
    for i in 0..100 {
        input.push_str(&format!(".method{}()", i));
    }
    assert!(
        lex_and_parse(&input),
        "Should parse long method chain with 100 method calls"
    );
}

#[test]
fn test_max_identifier_length() {
    let long_name = "a".repeat(200);
    let input = format!("const {}: number = 42", long_name);

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, _common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(&input, handler, &interner);
    assert!(
        lexer.tokenize().is_ok(),
        "Should tokenize identifier with max length (200 chars)"
    );
}

#[test]
fn test_extremely_long_identifier() {
    let long_name = "a".repeat(1000);
    let input = format!("const {}: number = 42", long_name);

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, _common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(&input, handler, &interner);
    assert!(
        lexer.tokenize().is_ok(),
        "Should tokenize extremely long identifier (1000 chars)"
    );
}

#[test]
fn test_maximum_file_size_parsing() {
    let mut input = String::with_capacity(1_000_000);

    for i in 0..50000 {
        input.push_str(&format!("const var{}: number = {}\n", i, i));
    }

    assert!(lex_and_parse(&input), "Should parse approximately 1MB file");
}

#[test]
fn test_mixed_stress_scenario() {
    let mut input = String::new();

    input.push_str("-- Large arrays\n");
    for i in 0..1000 {
        input.push_str(&format!("const arr{}: number[] = {{{}}}", i, i % 100));
        if i % 10 == 0 {
            input.push('\n');
        } else {
            input.push(';');
        }
    }

    input.push_str("\n-- Large objects\n");
    for i in 0..1000 {
        input.push_str(&format!("const obj{} = {{a: {}, b: {}}}", i, i, i + 1));
        if i % 10 == 0 {
            input.push('\n');
        } else {
            input.push(';');
        }
    }

    input.push_str("\n-- Long chains\n");
    for i in 0..50 {
        input.push_str(&format!(
            "const chain{} = data.method{}().field{}.value{}\n",
            i, i, i, i
        ));
    }

    input.push_str("\n-- Deep nesting\n");
    for _ in 0..30 {
        input.push_str("function outer(): void {\n");
    }
    for _ in 0..30 {
        input.push_str("const x: number = 42;\n");
    }
    for _ in 0..30 {
        input.push_str("}\n");
    }

    assert!(
        lex_and_parse(&input),
        "Should parse mixed stress scenario with arrays, objects, chains, and nesting"
    );
}

#[test]
fn test_polymorphic_recursive_types() {
    let input = r#"
        type List1<T> = T | List1<T>[]
        type List2<T> = T | List2<T>[]
        type List3<T> = T | List3<T>[]
        type List4<T> = T | List4<T>[]
        type List5<T> = T | List5<T>[]
        type List6<T> = T | List6<T>[]
        type List7<T> = T | List7<T>[]
        type List8<T> = T | List8<T>[]
        type List9<T> = T | List9<T>[]
        type List10<T> = T | List10<T>[]

        type DeepList = List10<number>

        const nested: DeepList = 1
    "#;

    assert!(
        compile_and_check(input).is_ok(),
        "Should parse polymorphic recursive types: {:?}",
        compile_and_check(input).err()
    );
}

#[test]
fn test_union_intersection_complex() {
    let mut input = String::from("type Complex = ");

    for i in 0..50 {
        if i > 0 {
            input.push_str(" & ");
        }
        input.push_str(&format!("{{ prop{}: number }}", i));
    }

    input.push_str(";\nconst x: Complex = { prop0: 1");

    for i in 1..50 {
        input.push_str(&format!(", prop{}: {}", i, i));
    }

    input.push_str(" }");

    assert!(
        compile_and_check(&input).is_ok(),
        "Should parse complex union/intersection with 50+ types: {:?}",
        compile_and_check(&input).err()
    );
}

#[test]
fn test_tuple_with_many_elements() {
    let mut input = String::from("const tuple: [");
    for i in 0..500 {
        if i > 0 {
            input.push_str(", ");
        }
        input.push_str("number");
    }
    input.push_str("] = [");
    for i in 0..500 {
        if i > 0 {
            input.push_str(", ");
        }
        input.push_str(&i.to_string());
    }
    input.push(']');
    assert!(
        compile_and_check(&input).is_ok(),
        "Should parse tuple with 500 elements: {:?}",
        compile_and_check(&input).err()
    );
}
