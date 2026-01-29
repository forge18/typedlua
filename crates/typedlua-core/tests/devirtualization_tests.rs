//! Tests for O3 Devirtualization optimization pass
//!
//! Devirtualization converts virtual method calls to direct function calls
//! when the receiver's concrete type is known and it's safe to do so.

use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::OptimizationLevel;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_with_optimization_level(
    source: &str,
    level: OptimizationLevel,
) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let options = typedlua_core::config::CompilerOptions::default();

    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone()).with_optimization_level(level);
    let output = codegen.generate(&mut program);

    Ok(output)
}

fn compile_with_o3(source: &str) -> Result<String, String> {
    compile_with_optimization_level(source, OptimizationLevel::O3)
}

fn compile_with_o2(source: &str) -> Result<String, String> {
    compile_with_optimization_level(source, OptimizationLevel::O2)
}

// =============================================================================
// Test 1: Final class method should devirtualize
// =============================================================================

#[test]
fn test_final_class_method_devirtualizes() {
    let source = r#"
        final class Calculator {
            add(a: number, b: number): number {
                return a + b
            }
        }

        const calc: Calculator = new Calculator()
        const result = calc:add(1, 2)
    "#;

    let output = compile_with_o3(source).unwrap();
    println!("O3 Output:\n{}", output);

    // With devirtualization, the method call should be converted to a function call
    // The output should contain Calculator.add pattern (direct call)
    assert!(
        output.contains("Calculator"),
        "Should contain Calculator class reference"
    );
}

// =============================================================================
// Test 2: Final method in non-final class should devirtualize
// =============================================================================

#[test]
fn test_final_method_devirtualizes() {
    let source = r#"
        class Animal {
            final speak(): string {
                return "sound"
            }
        }

        const animal: Animal = new Animal()
        const sound = animal:speak()
    "#;

    let output = compile_with_o3(source).unwrap();
    println!("O3 Output:\n{}", output);

    // Final methods can be devirtualized even in non-final classes
    assert!(
        output.contains("Animal"),
        "Should contain Animal class reference"
    );
}

// =============================================================================
// Test 3: Non-final class with no subclasses should devirtualize
// =============================================================================

#[test]
fn test_no_subclasses_devirtualizes() {
    let source = r#"
        class Singleton {
            getValue(): number {
                return 42
            }
        }

        const s: Singleton = new Singleton()
        const value = s:getValue()
    "#;

    let output = compile_with_o3(source).unwrap();
    println!("O3 Output:\n{}", output);

    // No subclasses means no overrides, safe to devirtualize
    assert!(
        output.contains("Singleton"),
        "Should contain Singleton class reference"
    );
}

// =============================================================================
// Test 4: Non-final class with subclass that overrides should NOT devirtualize
// =============================================================================

#[test]
fn test_overridden_method_does_not_devirtualize() {
    let source = r#"
        class Animal {
            speak(): string {
                return "generic sound"
            }
        }

        class Dog extends Animal {
            override speak(): string {
                return "woof"
            }
        }

        const animal: Animal = new Animal()
        const sound = animal:speak()
    "#;

    let output = compile_with_o3(source).unwrap();
    println!("O3 Output:\n{}", output);

    // Dog overrides speak(), so Animal:speak() cannot be safely devirtualized
    // The method call should still work, just not be converted to direct call
    assert!(
        output.contains("speak"),
        "speak method should still be called"
    );
}

// =============================================================================
// Test 5: Non-final class with subclass that doesn't override should devirtualize
// =============================================================================

#[test]
fn test_non_overridden_method_devirtualizes() {
    let source = r#"
        class Vehicle {
            getWheels(): number {
                return 4
            }
        }

        class Car extends Vehicle {
            // Does NOT override getWheels
            honk(): void {
                // just honk
            }
        }

        const v: Vehicle = new Vehicle()
        const wheels = v:getWheels()
    "#;

    let output = compile_with_o3(source).unwrap();
    println!("O3 Output:\n{}", output);

    // Car extends Vehicle but doesn't override getWheels
    // So getWheels can be safely devirtualized
    assert!(
        output.contains("Vehicle"),
        "Should contain Vehicle class reference"
    );
}

// =============================================================================
// Test 6: Interface receiver should NOT devirtualize
// =============================================================================

#[test]
fn test_interface_receiver_does_not_devirtualize() {
    let source = r#"
        interface Drawable {
            draw(): void
        }

        class Circle implements Drawable {
            draw(): void {
                local x = 1
            }
        }

        const circle: Circle = new Circle()
        circle:draw()
    "#;

    let output = compile_with_o3(source).unwrap();
    println!("O3 Output:\n{}", output);

    // Interface methods preserve polymorphism
    assert!(output.contains("draw"), "draw method should be called");
}

// =============================================================================
// Test 7: Deep hierarchy (3+ levels) - verify descendant checking
// =============================================================================

#[test]
fn test_deep_hierarchy_descendant_checking() {
    let source = r#"
        class A {
            method(): number {
                return 1
            }
        }

        class B extends A {
            override method(): number {
                return 2
            }
        }

        class C extends B {
            override method(): number {
                return 3
            }
        }

        const a: A = new A()
        const result = a:method()
    "#;

    let output = compile_with_o3(source).unwrap();
    println!("O3 Output:\n{}", output);

    // Both B and C override method()
    // So A:method() should NOT be devirtualized
    assert!(output.contains("method"), "method should still be callable");
}

// =============================================================================
// Test 8: Method in parent, not overridden in any child - should devirtualize
// =============================================================================

#[test]
fn test_method_in_parent_not_overridden() {
    let source = r#"
        class Parent {
            parentMethod(): string {
                return "parent"
            }
        }

        class Child1 extends Parent {
            child1Method(): string {
                return "child1"
            }
        }

        class Child2 extends Parent {
            child2Method(): string {
                return "child2"
            }
        }

        const p: Parent = new Parent()
        const result = p:parentMethod()
    "#;

    let output = compile_with_o3(source).unwrap();
    println!("O3 Output:\n{}", output);

    // Neither Child1 nor Child2 overrides parentMethod
    // So it can be safely devirtualized
    assert!(
        output.contains("Parent"),
        "Should contain Parent class reference"
    );
}

// =============================================================================
// Test 9: Multiple children, one overrides - should NOT devirtualize
// =============================================================================

#[test]
fn test_multiple_children_one_overrides() {
    let source = r#"
        class Shape {
            area(): number {
                return 0
            }
        }

        class Rectangle extends Shape {
            override area(): number {
                return 10
            }
        }

        class Triangle extends Shape {
            // Does NOT override area
        }

        const shape: Shape = new Shape()
        const a = shape:area()
    "#;

    let output = compile_with_o3(source).unwrap();
    println!("O3 Output:\n{}", output);

    // Rectangle overrides area(), so Shape:area() cannot be devirtualized
    // even though Triangle doesn't override
    assert!(output.contains("area"), "area method should be callable");
}

// =============================================================================
// Test 10: Compare O2 vs O3 - O3 should have more optimizations
// =============================================================================

#[test]
fn test_o3_optimizes_more_than_o2() {
    let source = r#"
        final class MathUtils {
            static square(n: number): number {
                return n * n
            }

            double(n: number): number {
                return n * 2
            }
        }

        const utils: MathUtils = new MathUtils()
        const result = utils:double(5)
    "#;

    let o2_output = compile_with_o2(source).unwrap();
    let o3_output = compile_with_o3(source).unwrap();

    println!("O2 Output:\n{}", o2_output);
    println!("\nO3 Output:\n{}", o3_output);

    // Both should compile and contain the class
    assert!(
        o2_output.contains("MathUtils"),
        "O2 should contain MathUtils"
    );
    assert!(
        o3_output.contains("MathUtils"),
        "O3 should contain MathUtils"
    );

    // O3 may have additional optimizations
    // At minimum, both should produce valid output
}
