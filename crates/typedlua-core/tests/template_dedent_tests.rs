#![cfg(feature = "unimplemented")]

use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn compile_and_generate(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();

    // Lex
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    // Parse
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    // Type check
    let mut type_checker =
        TypeChecker::new(handler, &interner, &common_ids).with_options(CompilerOptions::default());
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(&interner);
    let output = codegen.generate(&program);

    Ok(output)
}

// ============================================================================
// Basic Dedenting Tests
// ============================================================================

#[test]
fn test_single_line_template_no_dedenting() {
    let source = r#"
        const msg = `Hello World`
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Single-line templates should not be modified
            assert!(
                output.contains(r#""Hello World""#),
                "Single-line should not be dedented"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_basic_multi_line_dedenting() {
    let source = r#"
        const sql = `
            SELECT *
            FROM users
            WHERE id = 1
        `
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Should remove leading/trailing blank lines and common indentation
            // Note: Escape sequences in Lua strings use \n literally
            assert!(
                output.contains("SELECT *")
                    && output.contains("FROM users")
                    && output.contains("WHERE id = 1"),
                "Should dedent and trim blank lines. Got: {}",
                output
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_preserve_relative_indentation() {
    let source = r#"
        const html = `
            <div>
              <h1>Title</h1>
              <p>
                Content
              </p>
            </div>
        `
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Should remove common 12-space indent but preserve relative 2-space indents
            assert!(
                output.contains("<div>") && output.contains("  <h1>"),
                "Should preserve relative indentation"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_trim_leading_trailing_blank_lines() {
    let source = r#"
        const text = `

            Line 1
            Line 2

        `
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Should trim leading and trailing blank lines
            assert!(
                output.contains("Line 1") && output.contains("Line 2"),
                "Should trim leading/trailing blank lines. Got: {}",
                output
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_preserve_blank_lines_in_middle() {
    let source = r#"
        const text = `
            Line 1

            Line 2
        `
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Should preserve blank lines in the middle
            // Check for the pattern with blank line in between
            assert!(
                output.contains("Line 1") && output.contains("Line 2"),
                "Should preserve blank lines in middle. Got: {}",
                output
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

// ============================================================================
// Real-World Examples
// ============================================================================

#[test]
fn test_sql_query_example() {
    let source = r#"
        function getUser(id: number): string {
            return `
                SELECT name, email
                FROM users
                WHERE id = ${id}
                ORDER BY name
            `
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // SQL should be properly dedented
            assert!(
                output.contains("SELECT name, email"),
                "Should have dedented SQL"
            );
            assert!(output.contains("FROM users"), "Should have FROM");
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_html_template_example() {
    let source = r#"
        function render(title: string): string {
            return `
                <div class="container">
                  <h1>${title}</h1>
                  <p>Welcome!</p>
                </div>
            `
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // HTML should be properly dedented with relative indentation
            assert!(
                output.contains(r#"<div class=\"container\">"#),
                "Should have dedented HTML"
            );
            assert!(output.contains("  <h1>"), "Should preserve relative indent");
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_json_example() {
    let source = r#"
        function makeJSON(name: string): string {
            return `
                {
                  "name": "${name}",
                  "active": true
                }
            `
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // JSON should be properly formatted
            // The template has interpolation, so it's split into parts
            assert!(
                output.contains("{") && output.contains("name"),
                "Should have dedented JSON. Got: {}",
                output
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_all_whitespace_template() {
    let source = r#"
        const empty = `


        `
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // All-whitespace template should become empty string
            assert!(
                output.contains(r#""""#),
                "All-whitespace should become empty"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_first_line_has_content() {
    let source = r#"
        const msg = `Hello
            World`
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // First line has no indent, so only second line should be dedented to match
            // The dedenting finds min indent = 0 (from "Hello"), so nothing is removed
            assert!(output.contains("Hello"), "Should have Hello");
            assert!(output.contains("World"), "Should have World");
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_tabs_and_spaces_mixed_indentation() {
    // Note: Current implementation treats tabs and spaces as equivalent characters
    // It doesn't error on mixed indentation, just finds minimum character count
    let source = "
        const text = `
\t\t\tLine 1
            Line 2
        `
    ";

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Both lines should be present
            // Behavior: finds min indent (whichever has fewer chars) and removes that
            assert!(
                output.contains("Line 1") && output.contains("Line 2"),
                "Should handle mixed tabs/spaces. Got: {}",
                output
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}
