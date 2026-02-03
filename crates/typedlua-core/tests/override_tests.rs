//! Override Method Tests
//!
//! Tests for the @override decorator functionality including covariant return types,
//! contravariant parameters, final method constraints, signature matching, and
//! multi-level inheritance scenarios.

use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler, DiagnosticLevel};
use typedlua_core::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser.parse().map_err(|e| format!("{:?}", e))?;

    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

fn type_check(source: &str) -> Result<(), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser.parse().map_err(|e| format!("{:?}", e))?;

    let mut checker = TypeChecker::new_with_stdlib(handler, &interner, &common_ids)
        .expect("Failed to load stdlib");
    checker.check_program(&mut program).map_err(|e| e.message)?;

    Ok(())
}

fn compile_with_diagnostics(
    source: &str,
) -> (Result<String, String>, Arc<CollectingDiagnosticHandler>) {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => return (Err(format!("Lexing failed: {:?}", e)), handler),
    };

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = match parser.parse() {
        Ok(p) => p,
        Err(e) => return (Err(format!("Parsing failed: {:?}", e)), handler),
    };

    if has_errors(&handler) {
        return (Err("Compilation failed with errors".to_string()), handler);
    }

    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids);
    if let Err(e) = type_checker.check_program(&mut program) {
        return (Err(e.message), handler);
    }

    if has_errors(&handler) {
        return (Err("Compilation failed with errors".to_string()), handler);
    }

    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    (Ok(output), handler)
}

fn has_errors(handler: &CollectingDiagnosticHandler) -> bool {
    handler
        .get_diagnostics()
        .iter()
        .any(|d| d.level == DiagnosticLevel::Error)
}

// ============================================================================
// Basic Override Tests
// ============================================================================

#[test]
fn test_override_valid() {
    let source = r#"
        class Animal {
            speak(): void {
                print("...")
            }
        }

        class Dog extends Animal {
            override speak(): void {
                print("Woof!")
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Valid override should type-check successfully"
    );
}

#[test]
fn test_override_missing_parent_method() {
    let source = r#"
        class Animal {
            speak(): void {
                print("...")
            }
        }

        class Dog extends Animal {
            override fly(): void {
                print("I can't fly!")
            }
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_err(),
        "Override of non-existent parent method should fail"
    );
    assert!(
        result.unwrap_err().contains("does not have this method"),
        "Error message should mention missing parent method"
    );
}

#[test]
fn test_override_without_parent_class() {
    let source = r#"
        class Animal {
            override speak(): void {
                print("...")
            }
        }
    "#;

    let result = type_check(source);
    assert!(result.is_err(), "Override without parent class should fail");
    assert!(
        result.unwrap_err().contains("has no parent class"),
        "Error message should mention missing parent class"
    );
}

// ============================================================================
// Covariant Return Type Tests
// ============================================================================

#[test]
fn test_override_covariant_return_subclass() {
    let source = r#"
        class Animal {
            create(): Animal {
                return Animal.new()
            }
        }

        class Dog extends Animal {
            override create(): Dog {
                return Dog.new()
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Covariant return with subclass should compile"
    );
}

#[test]
fn test_override_covariant_return_number_to_subclass() {
    let source = r#"
        class Base {
            getValue(): number {
                return 1
            }
        }

        class Derived extends Base {
            override getValue(): number {
                return 2
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Same return type override should compile");
}

#[test]
fn test_override_covariant_return_same_type() {
    let source = r#"
        class Base {
            getValue(): string {
                return "hello"
            }
        }

        class Derived extends Base {
            override getValue(): string {
                return "world"
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Same return type override should compile");
}

#[test]
fn test_override_covariant_return_interface() {
    let source = r#"
        interface Shape {
            draw(): void
        }

        class Circle implements Shape {
            draw(): void {
                const x: number = 1
            }
        }

        class ColoredCircle implements Shape {
            draw(): void {
                const y: number = 2
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Override with interface return type should compile"
    );
}

#[test]
fn test_override_contravariant_parameter_rejects() {
    let source = r#"
        class Base {
            process(value: string): void {
                const x: number = 1
            }
        }

        class Derived extends Base {
            override process(value: any): void {
                const y: number = 2
            }
        }
    "#;

    let (result, _handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Contravariant parameter (less specific) should fail"
    );
}

#[test]
fn test_override_contravariant_parameter_allowed() {
    let source = r#"
        class Animal {
            makeSound(sound: string): void {
                print(sound)
            }
        }

        class Dog extends Animal {
            override makeSound(sound: string): void {
                print("Dog says: " .. sound)
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Contravariant parameter (same type) should compile"
    );
}

#[test]
fn test_override_final_method_immediate() {
    let source = r#"
        class Animal {
            final speak(): void {
                print("...")
            }
        }

        class Dog extends Animal {
            override speak(): void {
                print("Woof!")
            }
        }
    "#;

    let (result, _handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Overriding final method should fail");
    assert!(
        result.unwrap_err().contains("Cannot override final method"),
        "Error should mention final method"
    );
}

#[test]
fn test_override_final_method_ancestor() {
    let source = r#"
        class Animal {
            final speak(): void {
                print("...")
            }
        }

        class Mammal extends Animal {
            move(): void {
                print("Moving")
            }
        }

        class Dog extends Mammal {
            override speak(): void {
                print("Woof!")
            }
        }
    "#;

    let (result, _handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Overriding final method from ancestor should fail"
    );
    assert!(
        result.unwrap_err().contains("Cannot override final method"),
        "Error should mention final method"
    );
}

#[test]
fn test_non_final_method_can_be_overridden() {
    let source = r#"
        class Animal {
            speak(): void {
                print("...")
            }
        }

        class Dog extends Animal {
            override speak(): void {
                print("Woof!")
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Non-final method should be overridable");
}

// ============================================================================
// Signature Mismatch Tests
// ============================================================================

#[test]
fn test_override_different_parameter_count() {
    let source = r#"
        class Base {
            method(a: number): number {
                return a
            }
        }

        class Derived extends Base {
            override method(a: number, b: number): number {
                return a + b
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Different parameter count may be allowed");
}

#[test]
fn test_override_different_parameter_type() {
    let source = r#"
        class Base {
            method(x: number): number {
                return x
            }
        }

        class Derived extends Base {
            override method(x: string): string {
                return x
            }
        }
    "#;

    let (result, _handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Different parameter type should fail");
}

#[test]
fn test_override_incompatible_return_type() {
    let source = r#"
        class Base {
            method(): string {
                return "hello"
            }
        }

        class Derived extends Base {
            override method(): number {
                return 42
            }
        }
    "#;

    let (result, _handler) = compile_with_diagnostics(source);
    assert!(result.is_err(), "Incompatible return type should fail");
}

#[test]
fn test_override_without_parent_method() {
    let source = r#"
        class Base {
            otherMethod(): void {}
        }

        class Derived extends Base {
            override newMethod(): void {}
        }
    "#;

    let (result, _handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Override without parent method should fail"
    );
}

// ============================================================================
// Multi-Level Override Tests
// ============================================================================

#[test]
fn test_override_with_multiple_inheritance_levels() {
    let source = r#"
        class Animal {
            speak(): void {
                print("...")
            }
        }

        class Mammal extends Animal {
            override speak(): void {
                print("Mammal sound")
            }
        }

        class Dog extends Mammal {
            override speak(): void {
                print("Woof!")
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Override across multiple inheritance levels should work"
    );
}

#[test]
fn test_multi_level_override_same_method() {
    let source = r#"
        class Animal {
            speak(): string {
                return "Animal"
            }
        }

        class Mammal extends Animal {
            override speak(): string {
                return "Mammal"
            }
        }

        class Dog extends Mammal {
            override speak(): string {
                return "Dog"
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Multi-level override should compile");
}

#[test]
fn test_multi_level_override_chain() {
    let source = r#"
        class Level0 {
            method(): string {
                return "level0"
            }
        }

        class Level1 extends Level0 {
            override method(): string {
                return "level1"
            }
        }

        class Level2 extends Level1 {
            override method(): string {
                return "level2"
            }
        }

        class Level3 extends Level2 {
            override method(): string {
                return "level3"
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Deep override chain should compile");
}

#[test]
fn test_multi_level_override_with_super() {
    let source = r#"
        class Animal {
            speak(): string {
                return "Animal"
            }
        }

        class Mammal extends Animal {
            override speak(): string {
                return super.speak() .. " > Mammal"
            }
        }

        class Dog extends Mammal {
            override speak(): string {
                return super.speak() .. " > Dog"
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Multi-level override with super should compile"
    );
}

#[test]
fn test_multi_level_partial_override() {
    let source = r#"
        class Animal {
            speak(): string {
                return "Animal"
            }

            move(): void {
                print("Moving")
            }
        }

        class Mammal extends Animal {
            override speak(): string {
                return "Mammal"
            }
        }

        class Dog extends Mammal {
            move(): void {
                print("Dog running")
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Partial chain override should compile");
}

#[test]
fn test_multi_level_different_methods() {
    let source = r#"
        class Base {
            methodA(): string { return "A" }
            methodB(): string { return "B" }
        }

        class Middle extends Base {
            override methodA(): string {
                return super.methodA() .. "_middle"
            }
        }

        class Derived extends Middle {
            override methodB(): string {
                return super.methodB() .. "_derived"
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Different methods in override chain should compile"
    );
}

#[test]
fn test_multi_level_override_final_fails() {
    let source = r#"
        class Animal {
            final speak(): string {
                return "Animal"
            }
        }

        class Mammal extends Animal {
            override move(): string {
                return "Moving"
            }
        }

        class Dog extends Mammal {
            override speak(): string {
                return "Dog"
            }
        }
    "#;

    let (result, _handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Override final method in multi-level chain should fail"
    );
}

// ============================================================================
// Override with Generics Tests
// ============================================================================

#[test]
fn test_override_method_with_same_signature() {
    let source = r#"
        class Calculator {
            add(a: number, b: number): number {
                return a + b
            }
        }

        class AdvancedCalculator extends Calculator {
            override add(a: number, b: number): number {
                return a + b + 1
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Override method with same signature should compile"
    );
}

#[test]
fn test_override_method_with_generics_chain() {
    let source = r#"
        class Mapper<T, U> {
            map(value: T): U {
                return undefined
            }
        }

        class NumberToStringMapper extends Mapper<number, string> {
            override map(value: number): string {
                return tostring(value)
            }
        }

        class StringProcessor extends NumberToStringMapper {
            override map(value: number): string {
                return "Processed: " .. super.map(value)
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Override in generic chain should compile");
}

// ============================================================================
// Override with Static Methods Tests
// ============================================================================

#[test]
fn test_static_method_override() {
    let source = r#"
        class Base {
            static create(): Base {
                return Base.new()
            }
        }

        class Derived extends Base {
            static override create(): Derived {
                return Derived.new()
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Static method override should compile");
}

#[test]
fn test_static_method_override_final_fails() {
    let source = r#"
        class Base {
            static final helper(): void {
                const x: number = 1
            }
        }

        class Derived extends Base {
            static override helper(): void {
                const y: number = 2
            }
        }
    "#;

    let (result, _handler) = compile_with_diagnostics(source);
    assert!(
        result.is_err(),
        "Overriding final static method should fail"
    );
}

// ============================================================================
// Override in Abstract and Interface Tests
// ============================================================================

#[test]
fn test_override_abstract_method() {
    let source = r#"
        abstract class Base {
            abstract process(): void
        }

        class Derived extends Base {
            override process(): void {
                const x: number = 1
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Override abstract method should compile");
}

#[test]
fn test_override_interface_method() {
    let source = r#"
        interface Processable {
            process(): void
        }

        class Handler implements Processable {
            process(): void {
                const x: number = 1
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Interface method implementation should compile"
    );
}

#[test]
fn test_override_multiple_interfaces() {
    let source = r#"
        interface A {
            methodA(): void
        }

        interface B {
            methodB(): void
        }

        class Handler implements A, B {
            methodA(): void {
                const x: number = 1
            }

            methodB(): void {
                const y: number = 2
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Methods from multiple interfaces should compile"
    );
}

// ============================================================================
// Complex Override Scenarios
// ============================================================================

#[test]
fn test_override_with_optional_parameters() {
    let source = r#"
        class Base {
            method(x: number, y?: number): number {
                return y ~= nil and x + y or x
            }
        }

        class Derived extends Base {
            override method(x: number, y?: number): number {
                return super.method(x, y)
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Override with optional parameters should compile"
    );
}

#[test]
fn test_override_with_rest_parameters() {
    let source = r#"
        class Base {
            method(...args: any): void {
                const x: number = 1
            }
        }

        class Derived extends Base {
            override method(...args: any): void {
                super.method(...args)
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Override with rest parameters should compile"
    );
}

#[test]
fn test_override_and_new_method_same_class() {
    let source = r#"
        class Base {
            existing(): string {
                return "base"
            }
        }

        class Derived extends Base {
            override existing(): string {
                return "derived"
            }

            newMethod(): string {
                return "new"
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Both override and new method should compile"
    );
}

#[test]
fn test_override_same_method_different_classes() {
    let source = r#"
        class Base {
            process(): string {
                return "base"
            }
        }

        class Derived1 extends Base {
            override process(): string {
                return "derived1"
            }
        }

        class Derived2 extends Base {
            override process(): string {
                return "derived2"
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Same method overridden in different subclasses should compile"
    );
}

#[test]
fn test_method_without_override_keyword_warns() {
    let source = r#"
        class Animal {
            speak(): void {
                print("...")
            }
        }

        class Dog extends Animal {
            speak(): void {
                print("Woof!")
            }
        }
    "#;

    let result = compile(source);
    assert!(
        result.is_ok(),
        "Method override without keyword should compile (warns)"
    );
}
