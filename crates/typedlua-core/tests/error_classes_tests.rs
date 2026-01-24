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

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids)
        .with_options(CompilerOptions::default());
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(&interner);
    let output = codegen.generate(&program);

    Ok(output)
}

// ============================================================================
// Error Class Tests
// ============================================================================

#[test]
fn test_base_error_class() {
    let source = r#"
        class Error {
            message: string
            stack: string | nil

            constructor(message: string) {
                self.message = message
                self.stack = debug.traceback()
            }

            toString(): string {
                return self.message
            }
        }

        const err = new Error("Something went wrong")
        const msg = err.toString()
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("function Error._init"),
        "Should generate _init method"
    );
    assert!(
        output.contains("function Error.new"),
        "Should generate new method"
    );
    assert!(
        output.contains("function Error:toString()"),
        "Should generate toString method"
    );
    assert!(
        output.contains("Error.new(\"Something went wrong\")"),
        "Should create Error instance"
    );
}

#[test]
fn test_error_inheritance() {
    let source = r#"
        class Error {
            message: string
            constructor(message: string) {
                self.message = message
            }
        }

        class ArgumentError extends Error {
            argumentName: string | nil

            constructor(message: string, argumentName: string | nil = nil) {
                super(message)
                self.argumentName = argumentName
            }
        }

        const err = new ArgumentError("Invalid argument", "name")
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("ArgumentError"),
        "Should generate ArgumentError class"
    );
    assert!(
        output.contains("setmetatable(ArgumentError, { __index = Error })"),
        "Should set up inheritance"
    );
    assert!(
        output.contains("Error._init(self, message)"),
        "Should call super constructor"
    );
}

#[test]
fn test_throw_error_class() {
    let source = r#"
        class Error {
            message: string
            constructor(message: string) {
                self.message = message
            }
        }

        function throwError(): void {
            throw new Error("Test error")
        }
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("error(Error.new(\"Test error\"))"),
        "Should throw Error instance"
    );
}

#[test]
fn test_catch_error_class() {
    let source = r#"
        class Error {
            message: string
            constructor(message: string) {
                self.message = message
            }
        }

        try {
            throw new Error("Test error")
        } catch (e: Error) {
            print(e.message)
        }
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(output.contains("pcall"), "Should use pcall for try-catch");
    assert!(
        output.contains("print(e.message)"),
        "Should access error message"
    );
}

// ============================================================================
// Helper Function Tests
// ============================================================================

#[test]
fn test_require_helper() {
    let source = r#"
        class ArgumentError {
            message: string
            constructor(message: string) {
                self.message = message
            }
        }

        function requireCondition(condition: boolean, message: string): void {
            if (!condition) then
                throw new ArgumentError(message)
            end
        }

        requireCondition(true, "Should not throw")
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("if (not condition) then"),
        "Should check condition"
    );
    assert!(
        output.contains("error(ArgumentError.new(message))"),
        "Should throw ArgumentError"
    );
}

#[test]
fn test_check_helper() {
    let source = r#"
        class ArgumentError {
            message: string
            constructor(message: string) {
                self.message = message
            }
        }

        function checkValue(value: number | nil, message: string): void {
            if (value == nil) then
                throw new ArgumentError(message ?? "Value cannot be nil")
            end
        }

        const x = 42
        checkValue(x, "X cannot be nil")
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("if ((value == nil)) then"),
        "Should check for nil"
    );
    assert!(
        output.contains("error(ArgumentError.new"),
        "Should throw ArgumentError on nil"
    );
}

#[test]
fn test_unreachable_helper() {
    let source = r#"
        class StateError {
            message: string
            constructor(message: string) {
                self.message = message
            }
        }

        function unreachable(message: string | nil): never {
            throw new StateError(message ?? "Reached unreachable code")
        }

        function doSomething(x: number): string {
            if (x > 0) then
                return "positive"
            elseif (x < 0) then
                return "negative"
            else
                unreachable("x should be non-zero")
            end
        }
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("error(StateError.new"),
        "Should throw StateError"
    );
    assert!(
        output.contains("Reached unreachable code"),
        "Should have default message"
    );
}

#[test]
fn test_multiple_error_types() {
    let source = r#"
        class Error {
            message: string
            constructor(message: string) {
                self.message = message
            }
        }

        class ArgumentError extends Error {}
        class StateError extends Error {}
        class IOError extends Error {}

        function test(x: number): void {
            if (x < 0) then
                throw new ArgumentError("x must be positive")
            elseif (x == 0) then
                throw new StateError("x cannot be zero")
            elseif (x > 100) then
                throw new IOError("x is too large")
            end
        }
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("ArgumentError"),
        "Should have ArgumentError"
    );
    assert!(output.contains("StateError"), "Should have StateError");
    assert!(output.contains("IOError"), "Should have IOError");
}

#[test]
fn test_error_with_optional_fields() {
    let source = r#"
        class Error {
            message: string
            constructor(message: string) {
                self.message = message
            }
        }

        class ParseError extends Error {
            line: number | nil
            column: number | nil

            constructor(message: string, line: number | nil = nil, column: number | nil = nil) {
                super(message)
                self.line = line
                self.column = column
            }
        }

        const err1 = new ParseError("Parse error")
        const err2 = new ParseError("Parse error", 10, 5)
    "#;

    let output = compile_and_check(source).unwrap();
    assert!(
        output.contains("ParseError.new(\"Parse error\")"),
        "Should create with no optional args"
    );
    assert!(
        output.contains("ParseError.new(\"Parse error\", 10, 5)"),
        "Should create with optional args"
    );
}
