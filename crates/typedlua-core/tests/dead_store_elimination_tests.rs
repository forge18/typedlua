use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::{CompilerOptions, OptimizationLevel};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn compile_with_optimization_level(
    source: &str,
    level: OptimizationLevel,
) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let options = CompilerOptions::default();

    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone()).with_optimization_level(level);
    let output = codegen.generate(&mut program);

    Ok(output)
}

fn compile_with_o2(source: &str) -> Result<String, String> {
    compile_with_optimization_level(source, OptimizationLevel::O2)
}

#[test]
fn test_dead_store_simple_unused_variable() {
    let source = r#"
        const unused = 42
        const x = 1
    "#;

    let output = compile_with_o2(source).unwrap();
    assert!(
        !output.contains("unused"),
        "Dead store should be eliminated: {}",
        output
    );
}

#[test]
fn test_dead_store_used_variable_preserved() {
    let source = r#"
        const x = 42
        print(x)
    "#;

    let output = compile_with_o2(source).unwrap();
    assert!(
        output.contains("local x = 42"),
        "Used variable should be preserved: {}",
        output
    );
}

#[test]
fn test_dead_store_constant_with_expression() {
    let source = r#"
        const unused = 1 + 2 + 3
        const x = 1
    "#;

    let output = compile_with_o2(source).unwrap();
    assert!(
        !output.contains("unused"),
        "Dead store with expression should be eliminated: {}",
        output
    );
}

#[test]
fn test_dead_store_in_function() {
    let source = r#"
        function test()
            const unused = 42
            const x = 1
            return x
        end
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        !output.contains("unused"),
        "Dead store in function should be eliminated: {}",
        output
    );
}

#[test]
fn test_dead_store_destructuring() {
    let source = r#"
        const [a, b, c] = [1, 2, 3]
        const x = a + b
        print(x)
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(output.contains("a"), "a is used, should be preserved");
    assert!(output.contains("b"), "b is used, should be preserved");
}

#[test]
fn test_dead_store_destructuring_partial_unused() {
    let source = r#"
        const [a, b, c] = [1, 2, 3]
        const x = a + c
        print(x)
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(output.contains("a"), "a is used, should be preserved");
    // Note: b cannot be eliminated separately from the destructuring pattern
    // since const [a, b, c] = ... is a single statement. DSE operates at
    // statement level, not individual destructured bindings.
    assert!(output.contains("c"), "c is used, should be preserved");
}

#[test]
fn test_dead_store_object_destructuring() {
    let source = r#"
        const {foo, bar} = {foo: 1, bar: 2}
        const x = foo + bar
        print(x)
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(output.contains("foo"), "foo is used, should be preserved");
    assert!(output.contains("bar"), "bar is used, should be preserved");
}

#[test]
fn test_dead_store_closure_capture() {
    let source = r#"
        const x = 1
        const fn = () => x
        print(fn)
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("x"),
        "x is captured by closure, should be preserved: {}",
        output
    );
}

#[test]
fn test_dead_store_loop() {
    let source = r#"
        function test()
            local sum = 0
            for i = 1, 10 do
                const unused = i * 2
                sum = sum + i
            end
            return sum
        end
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        !output.contains("unused"),
        "Dead store in loop should be eliminated: {}",
        output
    );
}

#[test]
fn test_dead_store_conditional() {
    let source = r#"
        function test(x: boolean)
            if x then
                const unused = 1
                print("yes")
            else
                const y = 2
                print(y)
            end
        end
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        !output.contains("unused"),
        "Dead store in if-true should be eliminated: {}",
        output
    );
}

#[test]
fn test_dead_store_multiple_in_block() {
    // Test that multiple dead stores in a block are all eliminated
    let source = r#"
        const a = 1
        const b = 2
        const c = 3
        const d = 4
        const e = 5
        const used = a + b
        print(used)
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    // c, d, e are never used, so they should be eliminated
    assert!(
        !output.contains("local c"),
        "c should be eliminated: {}",
        output
    );
    assert!(
        !output.contains("local d"),
        "d should be eliminated: {}",
        output
    );
    assert!(
        !output.contains("local e"),
        "e should be eliminated: {}",
        output
    );
    // a, b are used to compute `used`, so they should be preserved
    assert!(output.contains("a"), "a is used, should be preserved");
    assert!(output.contains("b"), "b is used, should be preserved");
}

#[test]
fn test_dead_store_none_eliminated_o1() {
    let source = r#"
        const unused = 42
        const x = 1
    "#;

    let output = compile_with_optimization_level(source, OptimizationLevel::O1).unwrap();
    println!("O1 Output:\n{}", output);
    assert!(
        output.contains("unused"),
        "O1 should not eliminate dead stores: {}",
        output
    );
}

#[test]
fn test_dead_store_chain() {
    let source = r#"
        const a = 1
        const b = a
        const c = b
        const d = c
        print(d)
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("d"),
        "d is used, should be preserved: {}",
        output
    );
}

#[test]
fn test_dead_store_return_value() {
    let source = r#"
        function getX()
            const unused = 1
            return 42
        end
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        !output.contains("unused"),
        "Dead store before return should be eliminated: {}",
        output
    );
}

#[test]
fn test_dead_store_no_regression_on_normal_code() {
    // Ensure DSE doesn't break normal code with used variables
    let source = r#"
        function process(x: number): number
            const doubled = x * 2
            const tripled = x * 3
            return doubled + tripled
        end

        const result = process(5)
        print(result)
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    // Function should be preserved (it's called)
    assert!(
        output.contains("function process("),
        "Function should be preserved"
    );
    // Variables inside function are used, should be preserved
    assert!(
        output.contains("doubled") || output.contains("tripled"),
        "Used variables inside function should be preserved: {}",
        output
    );
}

#[test]
fn test_dead_store_nested_functions() {
    let source = r#"
        function outer()
            const x = 1
            const fn = () => x
            return fn
        end
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("x"),
        "x is captured by nested function, should be preserved"
    );
}

#[test]
fn test_dead_store_arrow_function() {
    let source = r#"
        const fn = (x: number) => {
            const unused = 1
            return x * 2
        }
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        !output.contains("unused"),
        "Dead store in arrow function should be eliminated"
    );
}

#[test]
fn test_dead_store_assignment() {
    let source = r#"
        local x = 1
        x = 2
        x = 3
        print(x)
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        output.contains("x = 3"),
        "Last assignment should be preserved: {}",
        output
    );
}

#[test]
fn test_dead_store_generic_for() {
    // Test dead store elimination in generic for loop body
    // Using a simple iterator pattern
    let source = r#"
        function test()
            const iter = () => nil
            for v in iter() do
                const unused = 42
                print(v)
            end
        end
    "#;

    let output = compile_with_o2(source).unwrap();
    println!("Output:\n{}", output);
    assert!(
        !output.contains("unused"),
        "Dead store in for loop should be eliminated: {}",
        output
    );
}
