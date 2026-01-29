use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_and_generate(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);

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

    // Type check
    let mut type_checker =
        TypeChecker::new(handler, &interner, &common_ids).with_options(CompilerOptions::default());
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Basic Reflection Metadata Tests
// ============================================================================

#[test]
fn test_class_has_type_metadata() {
    let source = r#"
        class User {
            name: string
            age: number

            constructor(name: string, age: number) {
                self.name = name
                self.age = age
            }

            getName(): string {
                return self.name
            }
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should have type metadata
            assert!(
                output.contains("__typeName"),
                "Should have __typeName field"
            );
            assert!(
                output.contains("\"User\""),
                "Should have class name as type name"
            );

            // Should have type ID (numeric)
            assert!(output.contains("__typeId"), "Should have __typeId field");

            // Should have fields metadata
            assert!(
                output.contains("__ownFields"),
                "Should have __ownFields array"
            );

            // Should have methods metadata
            assert!(
                output.contains("__ownMethods"),
                "Should have __ownMethods array"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_class_with_inheritance_has_parent_metadata() {
    let source = r#"
        class Animal {
            name: string

            constructor(name: string) {
                self.name = name
            }
        }

        class Dog extends Animal {
            breed: string

            constructor(name: string, breed: string) {
                super(name)
                self.breed = breed
            }
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Dog should have parent reference
            assert!(
                output.contains("__parent"),
                "Should have __parent field for Dog"
            );

            // Dog should have ancestors table
            assert!(
                output.contains("__ancestors"),
                "Should have __ancestors table for Dog"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_fields_metadata_includes_name_and_type() {
    let source = r#"
        class Point {
            x: number
            y: number
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should have field metadata with names
            assert!(
                output.contains("\"x\"") || output.contains("'x'"),
                "Should have field name 'x'"
            );
            assert!(
                output.contains("\"y\"") || output.contains("'y'"),
                "Should have field name 'y'"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_methods_metadata_includes_name() {
    let source = r#"
        class Calculator {
            add(a: number, b: number): number {
                return a + b
            }

            subtract(a: number, b: number): number {
                return a - b
            }
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should have method metadata with names
            assert!(
                output.contains("\"add\"") || output.contains("'add'"),
                "Should have method name 'add'"
            );
            assert!(
                output.contains("\"subtract\"") || output.contains("'subtract'"),
                "Should have method name 'subtract'"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_ancestors_table_precomputed() {
    let source = r#"
        class A {}
        class B extends A {}
        class C extends B {}
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // C should have ancestors table including A and B
            assert!(
                output.contains("__ancestors"),
                "Should have __ancestors table"
            );

            // Ancestors should be pre-computed at compile time (not lazy)
            // The table should map type IDs to true for O(1) lookups
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_lazy_building_functions_generated() {
    let source = r#"
        class Animal {
            name: string
            age: number

            speak(): void {}
            eat(): void {}
        }

        class Dog extends Animal {
            breed: string

            bark(): void {}
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should have lazy building functions
            assert!(
                output.contains("function Animal._buildAllFields()"),
                "Should have _buildAllFields for Animal"
            );
            assert!(
                output.contains("function Animal._buildAllMethods()"),
                "Should have _buildAllMethods for Animal"
            );
            assert!(
                output.contains("function Dog._buildAllFields()"),
                "Should have _buildAllFields for Dog"
            );
            assert!(
                output.contains("function Dog._buildAllMethods()"),
                "Should have _buildAllMethods for Dog"
            );

            // Should have caching
            assert!(
                output.contains("_allFieldsCache"),
                "Should cache all fields"
            );
            assert!(
                output.contains("_allMethodsCache"),
                "Should cache all methods"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

// ============================================================================
// Runtime API Tests
// ============================================================================

#[test]
fn test_isinstance_with_single_inheritance() {
    let source = r#"
        class Animal {
            name: string

            constructor(name: string) {
                self.name = name
            }
        }

        class Dog extends Animal {
            breed: string

            constructor(name: string, breed: string) {
                super(name)
                self.breed = breed
            }
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Dog should have ancestors table including both Dog and Animal type IDs
            assert!(
                output.contains("__ancestors"),
                "Should have __ancestors table"
            );

            // Dog's ancestors should be merged with Animal's ancestors
            // Check for the merging code
            assert!(
                output.contains("if Animal and Animal.__ancestors then")
                    || output.contains("if Animal then"),
                "Should merge parent ancestors at runtime"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_isinstance_with_multi_level_inheritance() {
    let source = r#"
        class A {}
        class B extends A {}
        class C extends B {}
        class D extends C {}
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Each class should have __ancestors table
            assert!(
                output.contains("A.__ancestors"),
                "A should have __ancestors"
            );
            assert!(
                output.contains("B.__ancestors"),
                "B should have __ancestors"
            );
            assert!(
                output.contains("C.__ancestors"),
                "C should have __ancestors"
            );
            assert!(
                output.contains("D.__ancestors"),
                "D should have __ancestors"
            );

            // Each child should merge parent ancestors
            // D's ancestors should include its entire chain: D, C, B, A
            assert!(
                output.matches("if ").count() >= 3,
                "Should have ancestor merging for B, C, and D"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_typeof_returns_type_info() {
    let source = r#"
        class User {
            name: string
            age: number

            constructor(name: string, age: number) {
                self.name = name
                self.age = age
            }

            getName(): string {
                return self.name
            }
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should have all metadata needed for typeof()
            assert!(output.contains("__typeId"), "Should have type ID");
            assert!(output.contains("__typeName"), "Should have type name");
            assert!(output.contains("\"User\""), "Should have class name");
            assert!(output.contains("__ownFields"), "Should have own fields");
            assert!(output.contains("__ownMethods"), "Should have own methods");
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_get_all_fields_includes_inherited() {
    let source = r#"
        class Animal {
            name: string
            age: number
        }

        class Dog extends Animal {
            breed: string
            color: string
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Dog's _buildAllFields should walk parent chain
            assert!(
                output.contains("function Dog._buildAllFields()"),
                "Should have _buildAllFields for Dog"
            );

            // Should check for parent and call parent's _buildAllFields
            assert!(
                output.contains("Dog.__parent") && output.contains("_buildAllFields"),
                "Should walk parent chain to collect fields"
            );

            // Should have own fields for Dog
            assert!(
                output.contains("\"breed\"") || output.contains("'breed'"),
                "Should have Dog's own field 'breed'"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_caching_happens_once() {
    let source = r#"
        class Base {
            x: number

            foo(): void {}
        }

        class Derived extends Base {
            y: number

            bar(): void {}
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should check cache before building
            assert!(
                output.contains("if Base._allFieldsCache then"),
                "Base should check fields cache"
            );
            assert!(
                output.contains("if Base._allMethodsCache then"),
                "Base should check methods cache"
            );
            assert!(
                output.contains("if Derived._allFieldsCache then"),
                "Derived should check fields cache"
            );
            assert!(
                output.contains("if Derived._allMethodsCache then"),
                "Derived should check methods cache"
            );

            // Should set cache after building
            assert!(
                output.contains("Base._allFieldsCache = fields"),
                "Base should cache fields"
            );
            assert!(
                output.contains("Derived._allFieldsCache = fields"),
                "Derived should cache fields"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}
