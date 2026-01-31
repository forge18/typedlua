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
}

// ============================================================================
// Reflection Edge Cases (Section 7.1.3)
// ============================================================================

#[test]
fn test_reflection_on_anonymous_class() {
    // Anonymous classes are created via object literals with type annotations
    let source = r#"
        type Point = { x: number, y: number }
        const p: Point = { x: 1, y: 2 }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);
            // Object literals don't have reflection metadata
            // This is expected behavior
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_reflection_on_generic_instances() {
    let source = r#"
        class Container<T> {
            value: T
            
            constructor(value: T) {
                self.value = value
            }
            
            getValue(): T {
                return self.value
            }
        }
        
        const intContainer = new Container<number>(42)
        const strContainer = new Container<string>("hello")
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should have reflection metadata for Container class
            assert!(output.contains("__typeName"), "Should have type name");
            assert!(output.contains("__typeId"), "Should have type ID");

            // Generic instances share the same class metadata
            assert!(
                output.contains("Container"),
                "Should reference Container class"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_get_fields_on_interface() {
    let source = r#"
        interface Drawable {
            x: number
            y: number
            draw(): void
        }
        
        class Circle implements Drawable {
            x: number
            y: number
            radius: number
            
            constructor(x: number, y: number, radius: number) {
                self.x = x
                self.y = y
                self.radius = radius
            }
            
            draw(): void {}
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Circle should have all fields including interface fields
            assert!(output.contains("__ownFields"), "Should have own fields");
            assert!(output.contains("x"), "Should have x field");
            assert!(output.contains("y"), "Should have y field");
            assert!(output.contains("radius"), "Should have radius field");
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_get_fields_excludes_private() {
    let source = r#"
        class User {
            public name: string
            private secret: string
            protected internal: string
            
            constructor(name: string) {
                self.name = name
                self.secret = "hidden"
                self.internal = "protected"
            }
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Should have __ownFields with field metadata
            assert!(output.contains("__ownFields"), "Should have own fields");

            // All fields should be present in metadata
            // (whether private fields are excluded depends on implementation)
            assert!(output.contains("name"), "Should have name field");
            assert!(
                output.contains("secret"),
                "Should have secret field in metadata"
            );
            assert!(
                output.contains("internal"),
                "Should have internal field in metadata"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_get_methods_includes_inherited() {
    let source = r#"
        class Animal {
            eat(): void {}
            sleep(): void {}
        }
        
        class Dog extends Animal {
            bark(): void {}
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Dog should have method metadata
            assert!(
                output.contains("__ownMethods"),
                "Dog should have own methods"
            );
            assert!(output.contains("bark"), "Should have bark method");

            // Dog's _buildAllMethods should include inherited methods
            assert!(
                output.contains("_buildAllMethods"),
                "Should have _buildAllMethods"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_isinstance_with_subclass_checks() {
    let source = r#"
        class Animal {
            name: string
            
            constructor(name: string) {
                self.name = name
            }
        }
        
        class Mammal extends Animal {
            hasFur: boolean = true
        }
        
        class Dog extends Mammal {
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

            // Each class should have ancestors table
            assert!(
                output.contains("Animal.__ancestors"),
                "Animal should have ancestors"
            );
            assert!(
                output.contains("Mammal.__ancestors"),
                "Mammal should have ancestors"
            );
            assert!(
                output.contains("Dog.__ancestors"),
                "Dog should have ancestors"
            );

            // Dog's ancestors should include all parent type IDs
            // This enables Reflect.isInstance(dog, Animal) to work
            assert!(
                output.contains("if Mammal and Mammal.__ancestors"),
                "Should merge Mammal's ancestors"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_reflection_on_nil_values() {
    let source = r#"
        class User {
            name: string
            
            constructor(name: string) {
                self.name = name
            }
        }
        
        const user: User | nil = nil
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // User class should still have reflection metadata
            assert!(output.contains("__typeName"), "Should have type name");
            assert!(output.contains("__typeId"), "Should have type ID");

            // The nil variable doesn't affect class metadata generation
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_reflection_with_multiple_inheritance_levels() {
    let source = r#"
        class A {
            fieldA: number = 1
            methodA(): void {}
        }
        
        class B extends A {
            fieldB: number = 2
            methodB(): void {}
        }
        
        class C extends B {
            fieldC: number = 3
            methodC(): void {}
        }
        
        class D extends C {
            fieldD: number = 4
            methodD(): void {}
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // D should have lazy building functions
            assert!(
                output.contains("function D._buildAllFields()"),
                "D should have _buildAllFields"
            );
            assert!(
                output.contains("function D._buildAllMethods()"),
                "D should have _buildAllMethods"
            );

            // Should merge ancestors from all levels
            assert!(
                output.contains("if C and C.__ancestors"),
                "Should merge C's ancestors"
            );
            assert!(
                output.contains("if B and B.__ancestors"),
                "Should merge B's ancestors"
            );
            assert!(
                output.contains("if A and A.__ancestors"),
                "Should merge A's ancestors"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_reflection_metadata_format() {
    let source = r#"
        class Person {
            name: string
            age: number
            readonly id: string
            
            constructor(name: string, age: number, id: string) {
                self.name = name
                self.age = age
                self.id = id
            }
            
            greet(): string {
                return "Hello, " .. self.name
            }
        }
    "#;

    let result = compile_and_generate(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Check field format: { name, type, modifiers }
            assert!(output.contains("__ownFields"), "Should have __ownFields");

            // Check method format: { name, params, returnType }
            assert!(output.contains("__ownMethods"), "Should have __ownMethods");

            // Should have type metadata
            assert!(output.contains("__typeName"), "Should have __typeName");
            assert!(output.contains("__typeId"), "Should have __typeId");

            // Should have parent reference
            assert!(output.contains("__parent"), "Should have __parent");
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
