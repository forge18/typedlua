use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
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

    // Type check
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Class Declaration Tests
// ============================================================================

#[test]
fn test_simple_class_declaration() {
    let source = r#"
        class Person {
            name: string
            age: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Simple class should compile");

    let output = result.unwrap();
    assert!(output.contains("local Person = {}"));
    assert!(output.contains("Person.__index = Person"));
}

#[test]
fn test_class_with_constructor() {
    let source = r#"
        class Person {
            name: string

            constructor(name: string) {
                self.name = name
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Class with constructor should compile");

    let output = result.unwrap();
    assert!(output.contains("function Person._init(self, name)"));
    assert!(output.contains("function Person.new(name)"));
}

#[test]
fn test_class_with_methods() {
    let source = r#"
        class Calculator {
            add(a: number, b: number): number {
                return a + b
            }

            multiply(a: number, b: number): number {
                return a * b
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Class with methods should compile");

    let output = result.unwrap();
    assert!(output.contains("function Calculator:add(a, b)"));
    assert!(output.contains("function Calculator:multiply(a, b)"));
}

#[test]
fn test_class_with_static_methods() {
    let source = r#"
        class MathUtils {
            static PI: number = 3.14159

            static square(x: number): number {
                return x * x
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Class with static methods should compile");

    let output = result.unwrap();
    assert!(output.contains("function MathUtils.square(x)"));
}

// ============================================================================
// Inheritance Tests
// ============================================================================

#[test]
fn test_basic_inheritance() {
    let source = r#"
        class Animal {
            name: string

            constructor(name: string) {
                self.name = name
            }
        }

        class Dog extends Animal {
            breed: string
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Basic inheritance should compile");

    let output = result.unwrap();
    assert!(output.contains("setmetatable(Dog, { __index = Animal })"));
}

#[test]
fn test_inheritance_with_constructor_chaining() {
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

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Inheritance with super() should compile");

    let output = result.unwrap();
    assert!(output.contains("Animal._init(self, name)"));
    assert!(output.contains("function Dog._init(self, name, breed)"));
}

#[test]
fn test_multi_level_inheritance() {
    let source = r#"
        class Animal {
            name: string

            constructor(name: string) {
                self.name = name
            }
        }

        class Mammal extends Animal {
            furColor: string

            constructor(name: string, furColor: string) {
                super(name)
                self.furColor = furColor
            }
        }

        class Dog extends Mammal {
            breed: string

            constructor(name: string, furColor: string, breed: string) {
                super(name, furColor)
                self.breed = breed
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Multi-level inheritance should compile");

    let output = result.unwrap();
    assert!(output.contains("Mammal._init(self, name, furColor)"));
    assert!(output.contains("Animal._init(self, name)"));
}

// ============================================================================
// Method Overriding Tests
// ============================================================================

#[test]
fn test_method_override() {
    let source = r#"
        class Animal {
            speak(): string {
                return "Some sound"
            }
        }

        class Dog extends Animal {
            speak(): string {
                return "Woof!"
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Method override should compile");

    let output = result.unwrap();
    assert!(output.contains("function Animal:speak()"));
    assert!(output.contains("function Dog:speak()"));
}

#[test]
fn test_method_override_with_super() {
    let source = r#"
        class Animal {
            speak(): string {
                return "Animal sound"
            }
        }

        class Dog extends Animal {
            speak(): string {
                const baseSound: string = super.speak()
                return baseSound .. " and Woof!"
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Method override with super should compile");

    let output = result.unwrap();
    assert!(output.contains("Animal.speak(self)"));
}

// ============================================================================
// Abstract Class Tests
// ============================================================================

#[test]
fn test_abstract_class() {
    let source = r#"
        abstract class Shape {
            abstract getArea(): number

            describe(): string {
                return "I am a shape"
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Abstract class should compile");

    let output = result.unwrap();
    // Abstract method should not be generated
    assert!(!output.contains("function Shape:getArea"));
    // Concrete method should be generated
    assert!(output.contains("function Shape:describe()"));
}

#[test]
fn test_abstract_class_implementation() {
    let source = r#"
        abstract class Shape {
            abstract getArea(): number
        }

        class Rectangle extends Shape {
            width: number
            height: number

            constructor(w: number, h: number) {
                const x: number = w + h
            }

            getArea(): number {
                return 100
            }
        }
    "#;

    let result = compile_and_check(source);
    if let Err(ref e) = result {
        eprintln!("Error: {}", e);
    }
    assert!(
        result.is_ok(),
        "Abstract class implementation should compile"
    );

    let output = result.unwrap();
    assert!(output.contains("function Rectangle:getArea()"));
}

// ============================================================================
// Interface Implementation Tests
// ============================================================================

#[test]
fn test_class_implements_interface() {
    let source = r#"
        interface Drawable {
            draw(): void
        }

        class Circle implements Drawable {
            radius: number

            constructor(radius: number) {
                self.radius = radius
            }

            draw(): void {
                const x: number = 1
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Class implementing interface should compile"
    );

    let output = result.unwrap();
    assert!(output.contains("function Circle:draw()"));
}

#[test]
fn test_class_implements_multiple_interfaces() {
    let source = r#"
        interface Drawable {
            draw(): void
        }

        interface Movable {
            move(x: number, y: number): void
        }

        class GameObject implements Drawable, Movable {
            x: number
            y: number

            draw(): void {
                const z: number = 1
            }

            move(newX: number, newY: number): void {
                self.x = newX
                self.y = newY
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Class implementing multiple interfaces should compile"
    );

    let output = result.unwrap();
    assert!(output.contains("function GameObject:draw()"));
    assert!(output.contains("function GameObject:move(newX, newY)"));
}

// ============================================================================
// Combined Tests
// ============================================================================

#[test]
fn test_full_oop_example() {
    let source = r#"
        abstract class Vehicle {
            brand: string

            constructor(brand: string) {
                self.brand = brand
            }

            abstract start(): void

            stop(): void {
                const x: number = 1
            }
        }

        class Car extends Vehicle {
            doors: number

            constructor(brand: string, doors: number) {
                super(brand)
                self.doors = doors
            }

            start(): void {
                const y: number = 2
            }

            honk(): void {
                const z: number = 3
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Full OOP example should compile");

    let output = result.unwrap();

    // Check inheritance
    assert!(output.contains("setmetatable(Car, { __index = Vehicle })"));

    // Check constructor chaining
    assert!(output.contains("Vehicle._init(self, brand)"));

    // Check abstract method not generated
    assert!(!output.contains("function Vehicle:start"));

    // Check concrete methods generated
    assert!(output.contains("function Vehicle:stop()"));
    assert!(output.contains("function Car:start()"));
    assert!(output.contains("function Car:honk()"));
}
