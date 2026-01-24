#![cfg(feature = "unimplemented")]

use std::sync::Arc;
use typedlua_core::ast::statement::Statement;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler};
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn parse_source(source: &str) -> Result<(typedlua_core::ast::Program, StringInterner), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let result = parser.parse();

    if let Err(e) = result {
        return Err(e.message);
    }

    if handler.error_count() > 0 {
        let diagnostics = handler.get_diagnostics();
        if let Some(diag) = diagnostics.first() {
            return Err(diag.message.clone());
        }
    }

    result.map(|p| (p, interner)).map_err(|e| e.message)
}

fn compile_and_check(source: &str, options: CompilerOptions) -> Result<String, String> {
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
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options);
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(&interner);
    let output = codegen.generate(&program);

    Ok(output)
}

#[test]
fn test_parse_single_level_namespace() {
    let source = "namespace Math;";
    let (program, interner) = parse_source(source).expect("Failed to parse namespace");

    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        Statement::DeclareNamespace(ns) => {
            let name = interner.resolve(ns.name.node);
            assert_eq!(name, "Math");
        }
        _ => panic!("Expected namespace statement"),
    }
}

#[test]
fn test_parse_multi_level_namespace() {
    let source = "namespace Math.Vector.Utils;";
    let (program, interner) = parse_source(source).expect("Failed to parse namespace");

    assert_eq!(program.statements.len(), 1);

    match &program.statements[0] {
        Statement::DeclareNamespace(ns) => {
            let name = interner.resolve(ns.name.node);
            assert_eq!(name, "Utils");
        }
        _ => panic!("Expected namespace statement"),
    }
}

#[test]
fn test_namespace_must_be_first() {
    let source = "const x = 1\nnamespace Math;";
    let result = parse_source(source);

    assert!(result.is_err(), "Should fail when namespace is not first");

    let err = result.unwrap_err();
    assert!(
        err.contains("Namespace declaration must be the first statement"),
        "Expected error about namespace placement, got: {}",
        err
    );
}

#[test]
fn test_only_one_namespace_allowed() {
    let source = "namespace Math;\nnamespace Vector;";
    let result = parse_source(source);

    assert!(
        result.is_err(),
        "Should fail when multiple namespaces declared"
    );

    let err = result.unwrap_err();
    assert!(
        err.contains("Only one namespace declaration allowed"),
        "Expected error about multiple namespaces, got: {}",
        err
    );
}

#[test]
fn test_namespace_with_exports() {
    let source =
        "namespace Math.Vector;\n\nexport function dot(a: any, b: any): number { return 0 }";
    let (program, interner) = parse_source(source).expect("Failed to parse namespace with export");

    assert_eq!(program.statements.len(), 2);

    // First statement should be namespace
    match &program.statements[0] {
        Statement::DeclareNamespace(ns) => {
            let name = interner.resolve(ns.name.node);
            assert_eq!(name, "Vector");
        }
        _ => panic!("Expected namespace statement"),
    }

    // Second statement should be export
    match &program.statements[1] {
        Statement::Export(_) => {}
        _ => panic!("Expected export statement"),
    }
}

#[test]
fn test_namespace_codegen_single_level() {
    let source = "namespace Math;";
    let result = compile_and_check(source, CompilerOptions::default());

    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Should generate: local Math = {}
            assert!(
                output.contains("local Math = {}"),
                "Expected 'local Math = {{}}' in output:\n{}",
                output
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_namespace_codegen_multi_level() {
    let source = "namespace Math.Vector;";
    let result = compile_and_check(source, CompilerOptions::default());

    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Should generate:
            // local Math = {}
            // Math.Vector = {}
            assert!(
                output.contains("local Math = {}"),
                "Expected 'local Math = {{}}' in output:\n{}",
                output
            );
            assert!(
                output.contains("Math.Vector = {}"),
                "Expected 'Math.Vector = {{}}' in output:\n{}",
                output
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

// ============================================================================
// Namespace Export Tests
// ============================================================================

#[test]
fn test_namespace_with_function_export() {
    let source = r#"
        namespace Math.Vector;

        export function dot(a: any, b: any): number {
            return 0
        }
    "#;

    let result = compile_and_check(source, CompilerOptions::default());
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should have namespace tables
            assert!(
                output.contains("local Math = {}"),
                "Should create Math namespace"
            );
            assert!(
                output.contains("Math.Vector = {}"),
                "Should create Math.Vector namespace"
            );

            // Should have function definition
            assert!(
                output.contains("local function dot"),
                "Should define dot function"
            );

            // Should attach function to namespace
            assert!(
                output.contains("Math.Vector.dot = dot"),
                "Should attach dot to namespace"
            );

            // Should return namespace root
            assert!(
                output.contains("return Math"),
                "Should return Math namespace root"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_namespace_with_multiple_exports() {
    let source = r#"
        namespace Math;

        export function add(a: number, b: number): number {
            return a + b
        }

        export function subtract(a: number, b: number): number {
            return a - b
        }
    "#;

    let result = compile_and_check(source, CompilerOptions::default());
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should attach both functions to namespace
            assert!(
                output.contains("Math.add = add"),
                "Should attach add to namespace"
            );
            assert!(
                output.contains("Math.subtract = subtract"),
                "Should attach subtract to namespace"
            );

            // Should return namespace root
            assert!(
                output.contains("return Math"),
                "Should return Math namespace root"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

// ============================================================================
// Declaration File Tests
// ============================================================================

#[test]
fn test_namespace_in_declaration_file() {
    // Declaration files should support namespace declarations
    let source = r#"
        namespace Godot;

        declare class Node {
            name: string
            get_parent(): Node | nil
        }

        declare class Node2D extends Node {
            position: Vector2
        }
    "#;

    let result = compile_and_check(source, CompilerOptions::default());
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Declaration files should generate namespace structure
            assert!(
                output.contains("local Godot = {}"),
                "Should create Godot namespace"
            );

            // Declare statements are erased, so no class definitions
            // But namespace structure should exist
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_namespace_with_type_declarations() {
    let source = r#"
        namespace Math.Geometry;

        export type Point = { x: number, y: number }
        export type Line = { start: Point, end: Point }

        export declare function distance(p1: Point, p2: Point): number
    "#;

    let result = compile_and_check(source, CompilerOptions::default());
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should have namespace
            assert!(
                output.contains("local Math = {}"),
                "Should create Math namespace"
            );
            assert!(
                output.contains("Math.Geometry = {}"),
                "Should create Math.Geometry namespace"
            );

            // Type aliases and declare statements are erased
            // Should return namespace root
            assert!(
                output.contains("return Math"),
                "Should return Math namespace root"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

// ============================================================================
// Godot Example Test
// ============================================================================

#[test]
fn test_godot_style_declaration_file() {
    // Based on the Godot example from the design doc
    let source = r#"
        namespace Godot.Scene;

        export declare class Node {
            name: string
            add_child(child: Node): void
            remove_child(child: Node): void
        }

        export declare class Sprite extends Node {
            centered: boolean
        }

        export declare class Camera2D extends Node {
            zoom: any
            offset: any
        }
    "#;

    let result = compile_and_check(source, CompilerOptions::default());
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should create nested namespace
            assert!(
                output.contains("local Godot = {}"),
                "Should create Godot namespace"
            );
            assert!(
                output.contains("Godot.Scene = {}"),
                "Should create Godot.Scene namespace"
            );

            // Should return namespace root
            assert!(
                output.contains("return Godot"),
                "Should return Godot namespace root"
            );

            // Declare classes are erased (type-only)
            assert!(
                !output.contains("class Node"),
                "Declare classes should be erased"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

// ============================================================================
// Namespace Import Tests
// ============================================================================

#[test]
fn test_import_from_namespaced_module() {
    // Test that importing from a namespaced module works correctly
    // This tests the export side - the import would be in another module
    let source = r#"
        namespace Math.Vector;

        export function dot(a: any, b: any): number {
            return 0
        }

        export function cross(a: any, b: any): number {
            return 0
        }
    "#;

    let result = compile_and_check(source, CompilerOptions::default());
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Verify exports are attached to namespace
            assert!(output.contains("Math.Vector.dot = dot"));
            assert!(output.contains("Math.Vector.cross = cross"));

            // Verify namespace root is returned (this is what gets imported)
            assert!(output.contains("return Math"));

            // When another module does: import { Math } from "./math/vector"
            // It will receive the Math table with Math.Vector.dot and Math.Vector.cross
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_import_syntax_with_namespace() {
    let source = r#"
        import { Math } from "./math/vector"

        const result = Math.Vector.dot({ x = 1, y = 2 }, { x = 3, y = 4 })
    "#;

    let result = compile_and_check(source, CompilerOptions::default());

    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Should generate require statement
            assert!(output.contains("require("), "Should have require call");
            assert!(
                output.contains("\"./math/vector\""),
                "Should have correct module path"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_namespace_full_path_access() {
    let source = r#"
        import { Math } from "./math/vector"

        function test() {
            return Math.Vector.dot({ x = 1, y = 2 }, { x = 3, y = 4 })
        }
    "#;

    let result = compile_and_check(source, CompilerOptions::default());

    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Should have the import
            assert!(output.contains("local Math ="), "Should import Math");
            // Should have the function accessing Math.Vector.dot
            assert!(
                output.contains("Math.Vector.dot"),
                "Should access Math.Vector.dot"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

// ============================================================================
// Path Enforcement Tests
// ============================================================================

#[test]
fn test_namespace_without_path_enforcement() {
    // Without enforcement, any namespace should work regardless of file path
    let source = "namespace Foo.Bar;";

    let result = compile_and_check(source, CompilerOptions::default());

    // Should succeed - path enforcement is off by default
    assert!(
        result.is_ok(),
        "Should succeed without path enforcement: {:?}",
        result.err()
    );
}

#[test]
fn test_namespace_path_enforcement_requires_module_id() {
    let source = "namespace Math.Vector;";

    let result = compile_and_check(source, CompilerOptions::default());

    // Should succeed
    assert!(
        result.is_ok(),
        "Should succeed when no module ID present: {:?}",
        result.err()
    );
}
