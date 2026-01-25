use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler};
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn compile_and_check(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);

    // Lex
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    // Parse
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    // Check for parse errors
    if handler.has_errors() {
        let diagnostics = handler.get_diagnostics();
        return Err(format!("Parser reported errors: {:?}", diagnostics));
    }

    // Type check
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids)
        .with_options(CompilerOptions::default());
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Basic Interface Default Method Tests
// ============================================================================

#[test]
fn test_interface_with_default_method() {
    let source = r#"
        interface Printable {
            name: string

            print(): void {
                const msg = "Name: " .. self.name
            }
        }
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // For now, interfaces don't generate code since they're type-only
            // This test just verifies that parsing and type checking work
            // Code generation for default methods will be added later
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_abstract_method_must_be_implemented() {
    let source = r#"
        interface Serializable {
            toJSON(): string
        }

        class User implements Serializable {
            name: string
        }
    "#;

    let result = compile_and_check(source);
    // Should fail because toJSON is not implemented
    assert!(
        result.is_err(),
        "Should fail: abstract method not implemented"
    );
    if let Err(e) = result {
        assert!(
            e.contains("toJSON") || e.contains("not implemented"),
            "Error should mention missing method: {}",
            e
        );
    }
}

#[test]
fn test_override_default_method() {
    let source = r#"
        interface Printable {
            name: string

            print(): void {
                const msg = "Default: " .. self.name
            }
        }

        class User implements Printable {
            name: string

            constructor(name: string) {
                self.name = name
            }

            print(): void {
                const msg = "Custom: " .. self.name
            }
        }
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // User should have its own print method
            assert!(
                output.contains("function User:print()"),
                "Should have User's print method"
            );

            // Should not copy default since it's overridden
            // (or copy logic should detect override)
            assert!(output.contains("print"), "Should have print method");
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_multiple_default_methods() {
    let source = r#"
        interface Printable {
            name: string

            print(): void {
                const msg = self.name
            }

            debug(): void {
                const msg = "Debug: " .. self.name
            }
        }

        class User implements Printable {
            name: string

            constructor(name: string) {
                self.name = name
            }
        }
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should have both default methods
            assert!(
                output.contains("print") && output.contains("debug"),
                "Should have both default methods"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_mixed_abstract_and_default_methods() {
    let source = r#"
        interface Serializable {
            name: string

            toJSON(): string

            toString(): string {
                return self.toJSON()
            }
        }

        class User implements Serializable {
            name: string

            constructor(name: string) {
                self.name = name
            }

            toJSON(): string {
                return "{\"name\": \"" .. self.name .. "\"}"
            }
        }
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should have toJSON on User
            assert!(
                output.contains("function User:toJSON()"),
                "Should implement abstract method"
            );

            // Should copy default toString
            assert!(output.contains("toString"), "Should have toString method");
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_debug() {
    let source = r#"
        interface Printable {
            name: string

            print(): void {
                const msg = "Default: " .. self.name
            }
        }

        class User implements Printable {
            name: string

            constructor(name: string) {
                self.name = name
            }

            print(): void {
                const msg = "Custom: " .. self.name
            }
        }
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}
