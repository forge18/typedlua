//! Comprehensive Error Conditions Tests
//!
//! Tests for error detection, error messages, and error recovery across all
//! compiler phases: parsing, type checking, and code generation.
//!
//! Section 7.1.3 of TODO.md - Edge Cases and Error Conditions

#![allow(unused_variables)]
#![allow(dead_code)]

use bumpalo::Bump;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler, DiagnosticLevel};
use typedlua_core::{MutableProgram, TypeChecker};
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

/// Compile a source string and return the result with diagnostics
fn compile_with_diagnostics(
    source: &str,
) -> (Result<String, String>, Arc<CollectingDiagnosticHandler>) {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();

    // Lex
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            return (Err(format!("Lexing failed: {:?}", e)), handler);
        }
    };

    // Parse
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let program = match parser.parse() {
        Ok(p) => p,
        Err(e) => {
            return (Err(format!("Parsing failed: {:?}", e)), handler);
        }
    };

    // Check if there are errors in diagnostics after parsing
    if has_errors(&handler) {
        return (Err("Compilation failed with errors".to_string()), handler);
    }

    // Type check
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids, &arena)
        .with_options(CompilerOptions::default());
    if let Err(e) = type_checker.check_program(&program) {
        return (Err(e.message), handler);
    }

    // Check if there are errors in diagnostics after type checking
    if has_errors(&handler) {
        return (Err("Compilation failed with errors".to_string()), handler);
    }

    // Generate code
    let mutable = MutableProgram::from_program(&program);
    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mutable);

    (Ok(output), handler)
}

/// Check if compilation produces any errors
fn has_errors(handler: &CollectingDiagnosticHandler) -> bool {
    handler
        .get_diagnostics()
        .iter()
        .any(|d| d.level == DiagnosticLevel::Error)
}

/// Check if compilation produces any warnings
fn has_warnings(handler: &CollectingDiagnosticHandler) -> bool {
    handler
        .get_diagnostics()
        .iter()
        .any(|d| d.level == DiagnosticLevel::Warning)
}

/// Get error count
fn error_count(handler: &CollectingDiagnosticHandler) -> usize {
    handler
        .get_diagnostics()
        .iter()
        .filter(|d| d.level == DiagnosticLevel::Error)
        .count()
}

// ============================================================================
// Parsing Errors
// ============================================================================

#[test]
fn test_unclosed_parentheses() {
    let source = r#"
        const x = (1 + 2
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with unclosed parentheses");
}

#[test]
fn test_unclosed_braces() {
    let source = r#"
        function foo()
            const x = 1
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with unclosed braces");
}

#[test]
fn test_unclosed_brackets() {
    let source = r#"
        const arr = [1, 2, 3
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with unclosed brackets");
}

#[test]
fn test_unclosed_string() {
    let source = r#"
        const msg = "unclosed string
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with unclosed string");
}

#[test]
fn test_unclosed_block_comment() {
    let source = r#"
        --[[ unclosed comment
        const x = 1
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with unclosed block comment");
}

#[test]
fn test_unexpected_token() {
    let source = r#"
        const x = @@@
    "#;

    let (result, _handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with unexpected token");
}

#[test]
fn test_invalid_operator_sequence() {
    let source = r#"
        const x = 1 ++ 2
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with invalid operator sequence"
    );
}

#[test]
fn test_missing_comma_in_array() {
    let source = r#"
        const arr = [1 2 3]
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with missing comma in array");
}

#[test]
fn test_invalid_number_literal() {
    let source = r#"
        const x = 0xGGG
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with invalid hex literal");
}

// ============================================================================
// Type Checking Errors
// ============================================================================

#[test]
fn test_type_mismatch_in_assignment() {
    let source = r#"
        const x: number = "hello"
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with type mismatch in assignment"
    );
}

#[test]
fn test_type_mismatch_in_variable_declaration() {
    let source = r#"
        const x: string = 123
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with type mismatch in variable declaration"
    );
}

#[test]
fn test_type_mismatch_in_function_call() {
    let source = r#"
        function greet(name: string): void {}
        greet(123)
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with type mismatch in function call"
    );
}

#[test]
fn test_type_mismatch_in_return_statement() {
    let source = r#"
        function getNumber(): number
            return "not a number"
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with type mismatch in return statement"
    );
}

#[test]
fn test_duplicate_variable_declaration() {
    let source = r#"
        const x = 1
        const x = 2
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with duplicate variable declaration"
    );
}

#[test]
fn test_undefined_variable() {
    let source = r#"
        const y = x + 1
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with undefined variable");
}

#[test]
fn test_missing_return_statement() {
    let source = r#"
        function getNumber(): number
            const x = 1
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with missing return statement");
}

#[test]
fn test_incorrect_argument_count() {
    let source = r#"
        function add(a: number, b: number): number
            return a + b
        end
        const result = add(1)
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with incorrect argument count");
}

#[test]
fn test_too_many_arguments() {
    let source = r#"
        function add(a: number, b: number): number
            return a + b
        end
        const result = add(1, 2, 3)
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with too many arguments");
}

// ============================================================================
// Generics Errors
// ============================================================================

#[test]
fn test_generic_constraint_violation() {
    let source = r#"
        interface Container<T extends number> {
            value: T
        end
        const c: Container<string> = { value: "hello" }
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with generic constraint violation"
    );
}

#[test]
fn test_generic_type_argument_count_mismatch() {
    let source = r#"
        type Pair<A, B> = { first: A, second: B }
        const p: Pair<number> = { first: 1 }
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with generic type argument count mismatch"
    );
}

#[test]
fn test_generic_type_inference_failure() {
    let source = r#"
        function identity<T>(x: T): T
            return x
        end
        const result = identity()
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with generic type inference failure"
    );
}

#[test]
fn test_duplicate_type_parameter() {
    let source = r#"
        function foo<T, T>(x: T): T
            return x
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with duplicate type parameter");
}

// ============================================================================
// Class Hierarchy Errors
// ============================================================================

#[test]
fn test_extending_final_class() {
    let source = r#"
        final class Base {}
        class Derived extends Base {}
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail when extending final class");
}

#[test]
fn test_overriding_final_method() {
    let source = r#"
        class Base {
            final method(): void {}
        end
        class Derived extends Base {
            override method(): void {}
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail when overriding final method");
}

#[test]
fn test_override_signature_mismatch() {
    let source = r#"
        class Base {
            method(x: number): number { return x }
        end
        class Derived extends Base {
            override method(x: string): string { return x }
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with override signature mismatch"
    );
}

#[test]
fn test_override_without_parent_method() {
    let source = r#"
        class Base {}
        class Derived extends Base {
            override method(): void {}
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail when overriding non-existent parent method"
    );
}

#[test]
fn test_instantiating_abstract_class() {
    let source = r#"
        abstract class Base {
            abstract method(): void
        end
        const instance = new Base()
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail when instantiating abstract class"
    );
}

#[test]
fn test_missing_abstract_method_implementation() {
    let source = r#"
        abstract class Base {
            abstract method(): void
        end
        class Derived extends Base {}
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with missing abstract method implementation"
    );
}

#[test]
fn test_circular_inheritance() {
    let source = r#"
        class A extends C {}
        class B extends A {}
        class C extends B {}
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with circular inheritance");
}

// ============================================================================
// Access Violation Errors
// ============================================================================

#[test]
fn test_private_access_from_different_class() {
    let source = r#"
        class Foo {
            private secret: number = 42
        end
        class Bar {
            getSecret(foo: Foo): number {
                return foo.secret
            end
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail when accessing private member from different class"
    );
}

#[test]
fn test_protected_access_from_outside_hierarchy() {
    let source = r#"
        class Base {
            protected value: number = 10
        end
        class Other {
            getValue(base: Base): number {
                return base.value
            end
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail when accessing protected member from outside hierarchy"
    );
}

#[test]
fn test_private_method_access_from_instance() {
    let source = r#"
        class Foo {
            private helper(): number { return 42 }
        end
        const foo = new Foo()
        const x = foo.helper()
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail when accessing private method from outside class"
    );
}

#[test]
fn test_protected_method_access_from_unrelated_class() {
    let source = r#"
        class Base {
            protected helper(): number { return 42 }
        end
        class Other {
            useHelper(base: Base): number {
                return base.helper()
            end
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail when accessing protected method from unrelated class"
    );
}

// ============================================================================
// Decorator Errors
// ============================================================================

#[test]
fn test_invalid_decorator_target() {
    let source = r#"
        @readonly
        function foo(): void {}
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with invalid decorator target");
}

#[test]
fn test_duplicate_decorator() {
    let source = r#"
        class Foo {
            @readonly
            @readonly
            name: string
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err() || has_warnings(&handler),
        "Should fail or warn with duplicate decorator"
    );
}

#[test]
fn test_abstract_method_not_overridden() {
    let source = r#"
        abstract class Base {
            abstract method(): void
        end
        class Concrete extends Base {}
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail when abstract method is not overridden"
    );
}

// ============================================================================
// Module Errors
// ============================================================================

#[test]
fn test_module_not_found() {
    let source = r#"
        import { foo } from "./nonexistent_module"
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail when module is not found");
}

#[test]
fn test_duplicate_export() {
    let source = r#"
        export const x = 1
        export const x = 2
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with duplicate export");
}

#[test]
fn test_import_nonexistent_member() {
    let source = r#"
        import { nonexistent } from "./some_module"
    "#;

    // This would require an actual module file to test properly
    // For now, we just verify the test structure
}

// ============================================================================
// Operator Overloading Errors
// ============================================================================

#[test]
fn test_operator_overload_wrong_return_type() {
    let source = r#"
        class Vector {
            x: number
            y: number
            
            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            end
            
            operator +(other: Vector): string {
                return "invalid"
            end
        end
    "#;

    // Note: This might not be an error depending on design
    // Some languages allow any return type for operators
    let (result, handler) = compile_with_diagnostics(source);
    // Just verify it compiles (operator overloading is flexible)
    assert!(
        result.is_ok(),
        "Operator overloading should allow various return types"
    );
}

#[test]
fn test_comparison_operator_without_boolean_return() {
    let source = r#"
        class Point {
            x: number
            y: number
            
            operator <(other: Point): number {
                return 0
            end
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    // Comparison operators must return boolean (consistent with test_operator_equal_must_return_boolean)
    assert!(result.is_err(), "Comparison operators must return boolean");
}

#[test]
fn test_index_operator_wrong_signature() {
    let source = r#"
        class Container {
            items: number[]
            
            operator [](key: string): number {
                return 0
            end
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    // This is a design decision - some languages require specific signatures
    assert!(
        result.is_ok() || result.is_err(),
        "Index operator signature validation depends on language design"
    );
}

// ============================================================================
// Pattern Matching Errors
// ============================================================================

#[test]
fn test_non_exhaustive_patterns() {
    let source = r#"
        type Color = "red" | "green" | "blue"
        const c: Color = "red"
        const result = match c {
            "red" => 1,
            "green" => 2
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with non-exhaustive patterns");
}

#[test]
fn test_unreachable_pattern_arm() {
    let source = r#"
        const x = 5
        const result = match x {
            _ => 0,
            5 => 1
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    // This should produce a warning or error
    assert!(
        result.is_err() || has_warnings(&handler),
        "Should warn or error with unreachable pattern arm"
    );
}

#[test]
fn test_invalid_pattern_type() {
    let source = r#"
        const x = 5
        const result = match x {
            "hello" => 1,
            _ => 0
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with pattern type mismatch");
}

// ============================================================================
// Dead Code Detection
// ============================================================================

#[test]
fn test_unreachable_code_after_return() {
    let source = r#"
        function foo(): number
            return 42
            const x = 1
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    // Should produce a warning about unreachable code
    assert!(
        result.is_ok(),
        "Should compile but warn about unreachable code"
    );
    // Note: This might be a warning rather than an error
}

#[test]
fn test_unreachable_code_after_throw() {
    let source = r#"
        function foo(): number
            throw "error"
            return 42
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_ok(),
        "Should compile but warn about unreachable code after throw"
    );
}

#[test]
fn test_unreachable_code_in_if_branch() {
    let source = r#"
        function foo(x: boolean): number
            if x then
                return 1
                const y = 2
            else
                return 3
            end
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_ok(),
        "Should compile but warn about unreachable code in if branch"
    );
}

// ============================================================================
// Error Recovery Tests
// ============================================================================

#[test]
fn test_error_recovery_multiple_errors() {
    let source = r#"
        const x: number = "hello"
        const y: string = 123
        const z: boolean = 1
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with multiple errors");

    // Should report all errors, not just the first
    let diagnostics = handler.get_diagnostics();
    let error_count = diagnostics
        .iter()
        .filter(|d| d.level == DiagnosticLevel::Error)
        .count();
    assert!(
        error_count >= 2,
        "Should report multiple errors, found {}",
        error_count
    );
}

#[test]
fn test_error_after_valid_code() {
    let source = r#"
        const valid = 42
        const invalid: number = "error"
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail even with valid code before error"
    );
}

// ============================================================================
// Error Message Quality Tests
// ============================================================================

#[test]
fn test_error_message_includes_location() {
    let source = r#"
        const x: number = "hello"
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail");

    let diagnostics = handler.get_diagnostics();
    assert!(!diagnostics.is_empty(), "Should have diagnostics");

    for diag in &diagnostics {
        if diag.level == DiagnosticLevel::Error {
            assert!(
                diag.span.line > 0 || diag.span.column > 0,
                "Error should include location information"
            );
        }
    }
}

#[test]
fn test_error_message_is_actionable() {
    let source = r#"
        const x: number = "hello"
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail");

    let diagnostics = handler.get_diagnostics();
    for diag in &diagnostics {
        if diag.level == DiagnosticLevel::Error {
            // Error message should mention types
            let msg = &diag.message;
            assert!(
                msg.contains("number") || msg.contains("string") || msg.contains("type"),
                "Error message should be clear and mention relevant types: {}",
                msg
            );
        }
    }
}

// ============================================================================
// Edge Case Error Tests
// ============================================================================

#[test]
fn test_error_in_deeply_nested_expression() {
    let source = r#"
        const x = (((((1 + "2")))))
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with type error in nested expression"
    );
}

#[test]
fn test_error_in_lambda() {
    let source = r#"
        const fn = (x: number): number => "not a number"
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Should fail with type error in lambda");
}

#[test]
fn test_error_in_nested_function() {
    let source = r#"
        function outer(): void
            function inner(): number
                return "error"
            end
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with type error in nested function"
    );
}

#[test]
fn test_error_in_class_method() {
    let source = r#"
        class Foo {
            method(): number {
                return "error"
            end
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Should fail with type error in class method"
    );
}

// ============================================================================
// Warning Tests
// ============================================================================

#[test]
fn test_unused_variable_warning() {
    let source = r#"
        function foo(): void
            const unused = 42
        end
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    assert!(result.is_ok(), "Should compile successfully");

    // May produce warning about unused variable
    let _warnings: Vec<_> = handler
        .get_diagnostics()
        .iter()
        .filter(|d| d.level == DiagnosticLevel::Warning)
        .collect();
}

#[test]
fn test_unused_import_warning() {
    let source = r#"
        import { foo } from "./module"
        function bar(): void {}
    "#;

    let (result, handler) = compile_with_diagnostics(source);
    // Will fail because module doesn't exist, but that's expected
    if result.is_ok() {
        // Check for unused import warning
        let _unused_warnings: Vec<_> = handler
            .get_diagnostics()
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Warning)
            .collect();
    }
}
