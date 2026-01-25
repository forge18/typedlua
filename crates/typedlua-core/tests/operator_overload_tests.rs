use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler, DiagnosticLevel};
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
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => return Err(format!("Lexing failed: {:?}", e)),
    };

    // Parse
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = match parser.parse() {
        Ok(p) => p,
        Err(e) => return Err(format!("Parsing failed: {:?}", e)),
    };

    // Type check
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids)
        .with_options(CompilerOptions::default());
    if let Err(e) = type_checker.check_program(&mut program) {
        return Err(format!("Type checking failed: {}", e.message));
    }

    // Check for diagnostics
    let errors: Vec<_> = handler
        .get_diagnostics()
        .iter()
        .filter(|d| d.level == DiagnosticLevel::Error)
        .map(|d| d.message.clone())
        .collect();
    if !errors.is_empty() {
        return Err(format!("Type checking errors: {:?}", errors));
    }

    // Generate code
    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Binary Operator Tests
// ============================================================================

#[test]
fn test_operator_add() {
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

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();
    eprintln!("OUTPUT:\n{}", output);

    // Verify metamethod is generated
    assert!(
        output.contains("Vector.__add"),
        "Should generate __add metamethod"
    );
    assert!(
        output.contains("function Vector.__add(self, other)"),
        "Should have self and other parameters"
    );
}

#[test]
fn test_operator_multiply() {
    let source = r#"
        class Vector {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            operator *(scalar: number): Vector {
                return new Vector(self.x * scalar, self.y * scalar)
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(
        output.contains("Vector.__mul"),
        "Should generate __mul metamethod"
    );
}

// ============================================================================
// Comparison Operator Tests
// ============================================================================

#[test]
fn test_operator_equal() {
    let source = r#"
        class Point {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            operator ==(other: Point): boolean {
                return self.x == other.x and self.y == other.y
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(
        output.contains("Point.__eq"),
        "Should generate __eq metamethod"
    );
}

#[test]
fn test_operator_less_than() {
    let source = r#"
        class Point {
            x: number

            constructor(x: number) {
                self.x = x
            }

            operator <(other: Point): boolean {
                return self.x < other.x
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(
        output.contains("Point.__lt"),
        "Should generate __lt metamethod"
    );
}

// ============================================================================
// Unary Operator Tests
// ============================================================================

#[test]
fn test_operator_unary_minus() {
    let source = r#"
        class Vector {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            operator -(): Vector {
                return new Vector(-self.x, -self.y)
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(
        output.contains("Vector.__unm"),
        "Should generate __unm metamethod"
    );
}

#[test]
fn test_operator_length() {
    let source = r#"
        class CustomArray {
            size: number

            constructor(size: number) {
                self.size = size
            }

            operator #(): number {
                return self.size
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(
        output.contains("CustomArray.__len"),
        "Should generate __len metamethod"
    );
}

// ============================================================================
// Special Operator Tests
// ============================================================================

#[test]
fn test_operator_index() {
    let source = r#"
        class Matrix {
            data: number[]

            constructor() {
                self.data = []
            }

            operator [](index: number): number {
                return self.data[index]
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(
        output.contains("Matrix.__index"),
        "Should generate __index metamethod"
    );
}

#[test]
fn test_operator_new_index() {
    let source = r#"
        class Matrix {
            data: number[]

            constructor() {
                self.data = []
            }

            operator []=(index: number, value: number): void {
                self.data[index] = value
            }
        }
    "#;

    let result = compile_and_check(source);
    let output = match result {
        Ok(o) => o,
        Err(e) => {
            panic!("Failed to compile: {:?}", e);
        }
    };
    eprintln!("OUTPUT:\n{}", output);

    assert!(
        output.contains("Matrix.__newindex"),
        "Should generate __newindex metamethod"
    );
}

#[test]
fn test_operator_call() {
    let source = r#"
        class Adder {
            value: number

            constructor(value: number) {
                self.value = value
            }

            operator ()(x: number): number {
                return self.value + x
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(
        output.contains("Adder.__call"),
        "Should generate __call metamethod"
    );
}

// ============================================================================
// Type Checking Tests
// ============================================================================

#[test]
fn test_operator_wrong_param_count_binary() {
    let source = r#"
        class Vector {
            operator +(): Vector {
                return self
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_err(),
        "Should fail: binary operator needs 1 parameter"
    );
}

#[test]
fn test_operator_wrong_param_count_unary() {
    let source = r#"
        class Vector {
            operator #(x: number): number {
                return x
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_err(),
        "Should fail: unary operator needs 0 parameters"
    );
}

#[test]
fn test_operator_equal_must_return_boolean() {
    let source = r#"
        class Point {
            operator ==(other: Point): number {
                return 1
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_err(), "Should fail: == must return boolean");
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_multiple_operators() {
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

            operator -(other: Vector): Vector {
                return new Vector(self.x - other.x, self.y - other.y)
            }

            operator *(scalar: number): Vector {
                return new Vector(self.x * scalar, self.y * scalar)
            }

            operator #(): number {
                return (self.x * self.x + self.y * self.y) ^ 0.5
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    let output = result.unwrap();

    assert!(output.contains("Vector.__add"));
    assert!(output.contains("Vector.__sub"));
    assert!(output.contains("Vector.__mul"));
    assert!(output.contains("Vector.__len"));
}
