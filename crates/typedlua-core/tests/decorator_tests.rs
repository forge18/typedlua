use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_and_check(source: &str) -> Result<String, String> {
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
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids)
        .with_options(CompilerOptions::default());
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Class Decorator Tests
// ============================================================================

#[test]
fn test_simple_class_decorator() {
    let source = r#"
        function sealed(target)
            return target
        end

        @sealed
        class MyClass {
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Simple class decorator should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should apply decorator: MyClass = sealed(MyClass)
    assert!(
        output.contains("MyClass = sealed(MyClass)"),
        "Output should contain decorator application but got:\n{}",
        output
    );
}

#[test]
fn test_class_decorator_with_arguments() {
    let source = r#"
        function component(name: string)
            return function(target)
                return target
            end
        end

        @component("my-component")
        class MyComponent {
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Class decorator with arguments should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should apply decorator: MyComponent = component("my-component")(MyComponent)
    assert!(
        output.contains("MyComponent = component(\"my-component\")(MyComponent)"),
        "Output should contain decorator with arguments but got:\n{}",
        output
    );
}

#[test]
fn test_multiple_class_decorators() {
    let source = r#"
        function sealed(target) return target end
        function logged(target) return target end

        @sealed
        @logged
        class MyClass {
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Multiple class decorators should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should apply both decorators
    assert!(
        output.contains("MyClass = sealed(MyClass)"),
        "Output should contain first decorator"
    );
    assert!(
        output.contains("MyClass = logged(MyClass)"),
        "Output should contain second decorator"
    );
}

#[test]
fn test_namespaced_decorator() {
    let source = r#"
        const validators = {
            validate: function(target)
                return target
            end
        }

        @validators.validate
        class User {
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Namespaced decorator should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should apply decorator: User = validators.validate(User)
    assert!(
        output.contains("User = validators.validate(User)"),
        "Output should contain namespaced decorator but got:\n{}",
        output
    );
}

// ============================================================================
// Method Decorator Tests
// ============================================================================

#[test]
fn test_method_decorator() {
    let source = r#"
        function log(target)
            return target
        end

        class MyClass {
            @log
            myMethod(): void {
                const x: number = 1
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Method decorator should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should apply decorator: MyClass.myMethod = log(MyClass.myMethod)
    assert!(
        output.contains("MyClass.myMethod = log(MyClass.myMethod)"),
        "Output should contain method decorator but got:\n{}",
        output
    );
}

#[test]
fn test_static_method_decorator() {
    let source = r#"
        function cache(target)
            return target
        end

        class Utils {
            @cache
            static calculate(x: number): number {
                return x * 2
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Static method decorator should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should apply decorator to static method
    assert!(
        output.contains("Utils.calculate = cache(Utils.calculate)"),
        "Output should contain static method decorator but got:\n{}",
        output
    );
}

#[test]
fn test_method_decorator_with_arguments() {
    let source = r#"
        function throttle(ms: number)
            return function(target)
                return target
            end
        end

        class MyClass {
            @throttle(1000)
            handleClick(): void {
                const x: number = 1
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Method decorator with arguments should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should apply decorator: MyClass.handleClick = throttle(1000)(MyClass.handleClick)
    assert!(
        output.contains("MyClass.handleClick = throttle(1000)(MyClass.handleClick)"),
        "Output should contain method decorator with arguments but got:\n{}",
        output
    );
}

// ============================================================================
// Configuration Tests
// ============================================================================

#[test]
fn test_decorator_disabled() {
    let source = r#"
        @sealed
        class MyClass {
        }
    "#;

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser.parse().unwrap();

    let options = CompilerOptions {
        enable_decorators: false,
        ..Default::default()
    };

    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options);
    let result = type_checker.check_program(&mut program);

    assert!(result.is_err(), "Decorators should fail when disabled");
    let error = result.unwrap_err();
    assert!(
        error
            .message
            .contains("Decorators require decorator features"),
        "Error should mention decorator features but got: {}",
        error.message
    );
    assert!(
        error.message.contains("enableDecorators"),
        "Error should mention enableDecorators but got: {}",
        error.message
    );
}

#[test]
fn test_decorator_enabled_by_default() {
    let source = r#"
        function sealed(target)
            return target
        end

        @sealed
        class MyClass {
        }
    "#;

    let options = CompilerOptions::default();
    assert!(
        options.enable_decorators,
        "Decorators should be enabled by default"
    );

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Decorators should work when enabled: {:?}",
        result.err()
    );
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_decorator_with_class_inheritance() {
    let source = r#"
        function logged(target)
            return target
        end

        class Base {
        }

        @logged
        class Derived extends Base {
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Decorator with inheritance should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    assert!(output.contains("Derived = logged(Derived)"));
}

#[test]
fn test_decorator_with_class_interface() {
    let source = r#"
        interface Serializable {
            serialize(): string
        }

        function serializable(target)
            return target
        end

        @serializable
        class Data implements Serializable {
            serialize(): string {
                return ""
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Decorator with interface should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    assert!(output.contains("Data = serializable(Data)"));
}

#[test]
fn test_decorator_preserves_class_structure() {
    let source = r#"
        function enhance(target)
            return target
        end

        @enhance
        class MyClass {
            constructor(value: number) {
                const x: number = value
            }

            getValue(): number {
                return 0
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Decorator should preserve class structure: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Class should still have its structure
    assert!(output.contains("MyClass.new"));
    assert!(output.contains("function MyClass:getValue"));
    // And decorator should be applied
    assert!(output.contains("MyClass = enhance(MyClass)"));
}

#[test]
fn test_no_decorators_when_not_used() {
    let source = r#"
        class MyClass {
            myMethod(): void {
                const x: number = 1
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Class without decorators should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should not have any decorator applications
    assert!(
        !output.contains(" = log("),
        "Output should not contain decorator applications"
    );
    assert!(
        !output.contains(" = sealed("),
        "Output should not contain decorator applications"
    );
}

#[test]
fn test_mixed_decorated_and_plain_methods() {
    let source = r#"
        function log(target)
            return target
        end

        class MyClass {
            @log
            decoratedMethod(): void {
                const x: number = 1
            }

            plainMethod(): void {
                const y: number = 2
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Mixed decorated and plain methods should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should have decorator on first method
    assert!(output.contains("MyClass.decoratedMethod = log(MyClass.decoratedMethod)"));
    // But not on second method
    assert!(!output.contains("MyClass.plainMethod = log(MyClass.plainMethod)"));
}
