use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn compile_and_check(source: &str) -> Result<String, String> {
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

    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids)
        .with_options(CompilerOptions::default());
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Bang Operator Tests
// ============================================================================

#[test]
fn test_bang_with_boolean() {
    let source = r#"
        const a = true
        const b = !a
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(output.contains("not a"), "Should compile ! to 'not'");
}

#[test]
fn test_bang_with_literal() {
    let source = r#"
        const a = !true
        const b = !false
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("not true"),
        "Should compile !true to 'not true'"
    );
    assert!(
        output.contains("not false"),
        "Should compile !false to 'not false'"
    );
}

#[test]
fn test_bang_with_expression() {
    let source = r#"
        const a = 5
        const b = !(a > 3)
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(output.contains("not"), "Should compile ! operator");
}

#[test]
fn test_bang_in_if_condition() {
    let source = r#"
        const a = true
        if (!a) then
            const x = 1
        end
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("if (not a) then"),
        "Should compile !a in if condition"
    );
}

#[test]
fn test_bang_with_function_call() {
    let source = r#"
        function isValid(): boolean {
            return true
        }
        const result = !isValid()
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("not isValid()"),
        "Should compile ! with function call"
    );
}

#[test]
fn test_bang_vs_not_keyword() {
    let source = r#"
        const a = !true
        const b = not false
    "#;

    let output = compile_and_check(source).unwrap();
    // Both should compile to 'not'
    let not_count = output.matches(" not ").count();
    assert!(not_count >= 2, "Both ! and 'not' should compile to 'not'");
}

#[test]
fn test_bang_precedence() {
    let source = r#"
        const a = true
        const b = false
        const result = !a and b
    "#;

    let output = compile_and_check(source).unwrap();
    // ! should have higher precedence than 'and'
    assert!(
        output.contains("not a and"),
        "! should bind tighter than 'and'"
    );
}
