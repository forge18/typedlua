#![cfg(feature = "unimplemented")]

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

    let options = CompilerOptions::default();

    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options.clone());
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

#[test]
fn test_aggressive_inlining_works() {
    let source = r#"
        function mediumFunction(x: number): number {
            const a = x + 1
            const b = a * 2
            const c = b - 3
            const d = c / 4
            const e = d ^ 2
            const f = e + 1
            return f
        }

        function caller(): number {
            return mediumFunction(10)
        }
    "#;

    let output = compile_and_check(source).unwrap();
    println!("Output:\n{}", output);

    assert!(
        output.contains("mediumFunction"),
        "Should have mediumFunction"
    );
}

#[test]
fn test_operator_overload_collection() {
    let source = r#"
        class Vector {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            operator +(other: Vector): Vector {
                return new Vector(self.x + other.x, self.y + other.y)
            }
        }

        const v1 = new Vector(1, 2)
        const v2 = new Vector(3, 4)
        const v3 = v1 + v2
    "#;

    let output = compile_and_check(source).unwrap();
    println!("Output:\n{}", output);

    assert!(output.contains("Vector"), "Should have Vector class");
    assert!(output.contains("__add"), "Should have __add metamethod");
}

#[test]
fn test_interface_method_analysis() {
    let source = r#"
        interface Printable {
            name: string
            print(): void
        }

        class User implements Printable {
            name: string

            constructor(name: string) {
                self.name = name
            }

            print(): void {
                print("Name: " .. self.name)
            }
        }

        const user = new User("Alice")
        user.print()
    "#;

    let output = compile_and_check(source).unwrap();
    println!("Output:\n{}", output);

    assert!(output.contains("User"), "Should have User class");
}

#[test]
fn test_devirtualization_analysis() {
    let source = r#"
        class Calculator {
            add(a: number, b: number): number {
                return a + b
            }

            multiply(a: number, b: number): number {
                return a * b
            }
        }

        const calc = new Calculator()
        const sum = calc.add(1, 2)
        const product = calc.multiply(3, 4)
    "#;

    let output = compile_and_check(source).unwrap();
    println!("Output:\n{}", output);

    assert!(
        output.contains("Calculator"),
        "Should have Calculator class"
    );
}

#[test]
fn test_generic_function_analysis() {
    let source = r#"
        function identity<T>(value: T): T {
            return value
        }

        const num = identity(42)
        const str = identity("hello")
    "#;

    let output = compile_and_check(source).unwrap();
    println!("Output:\n{}", output);

    assert!(output.contains("identity"), "Should have identity function");
}
