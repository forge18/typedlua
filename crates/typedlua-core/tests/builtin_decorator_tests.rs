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
// @readonly Decorator Tests
// ============================================================================

#[test]
fn test_readonly_class_decorator() {
    let source = r#"
        @readonly
        class Config {
            value: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "readonly decorator should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.readonly"),
        "Output should contain TypedLua.readonly function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("readonly = TypedLua.readonly"),
        "Runtime should export readonly as global alias"
    );

    // Should apply decorator using the plain name (global alias)
    assert!(
        output.contains("Config = readonly(Config)"),
        "Output should apply readonly decorator but got:\n{}",
        output
    );
}

#[test]
fn test_readonly_method_decorator() {
    let source = r#"
        class MyClass {
            @readonly
            getValue(): number {
                return 42
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "readonly method decorator should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.readonly"),
        "Output should contain TypedLua.readonly function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("readonly = TypedLua.readonly"),
        "Runtime should export readonly as global alias"
    );

    // Should apply decorator using the plain name (global alias)
    assert!(
        output.contains("MyClass.getValue = readonly(MyClass.getValue)"),
        "Output should apply readonly to method but got:\n{}",
        output
    );
}

// ============================================================================
// @sealed Decorator Tests
// ============================================================================

#[test]
fn test_sealed_class_decorator() {
    let source = r#"
        @sealed
        class FinalClass {
            name: string
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "sealed decorator should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.sealed"),
        "Output should contain TypedLua.sealed function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("sealed = TypedLua.sealed"),
        "Runtime should export sealed as global alias"
    );

    // Should apply decorator using the plain name (global alias)
    assert!(
        output.contains("FinalClass = sealed(FinalClass)"),
        "Output should apply sealed decorator but got:\n{}",
        output
    );
}

#[test]
fn test_sealed_method_decorator() {
    let source = r#"
        class MyClass {
            @sealed
            process(): void {
                const x: number = 1
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "sealed method decorator should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.sealed"),
        "Output should contain TypedLua.sealed function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("sealed = TypedLua.sealed"),
        "Runtime should export sealed as global alias"
    );

    // Should apply decorator using the plain name (global alias)
    assert!(
        output.contains("MyClass.process = sealed(MyClass.process)"),
        "Output should apply sealed to method but got:\n{}",
        output
    );
}

// ============================================================================
// @deprecated Decorator Tests
// ============================================================================

#[test]
fn test_deprecated_class_decorator() {
    let source = r#"
        @deprecated
        class OldClass {
            value: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "deprecated decorator should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.deprecated"),
        "Output should contain TypedLua.deprecated function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("deprecated = TypedLua.deprecated"),
        "Runtime should export deprecated as global alias"
    );

    // Should apply decorator using the plain name (global alias)
    assert!(
        output.contains("OldClass = deprecated(OldClass)"),
        "Output should apply deprecated decorator but got:\n{}",
        output
    );
}

#[test]
fn test_deprecated_with_message() {
    let source = r#"
        @deprecated("Use NewClass instead")
        class OldClass {
            value: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "deprecated with message should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.deprecated"),
        "Output should contain TypedLua.deprecated function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("deprecated = TypedLua.deprecated"),
        "Runtime should export deprecated as global alias"
    );

    // Should apply decorator with message using the plain name (global alias)
    assert!(
        output.contains("OldClass = deprecated(\"Use NewClass instead\")(OldClass)"),
        "Output should apply deprecated with message but got:\n{}",
        output
    );
}

#[test]
fn test_deprecated_method() {
    let source = r#"
        class MyClass {
            @deprecated
            oldMethod(): void {
                const x: number = 1
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "deprecated method should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.deprecated"),
        "Output should contain TypedLua.deprecated function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("deprecated = TypedLua.deprecated"),
        "Runtime should export deprecated as global alias"
    );

    // Should apply decorator using the plain name (global alias)
    assert!(
        output.contains("MyClass.oldMethod = deprecated(MyClass.oldMethod)"),
        "Output should apply deprecated to method but got:\n{}",
        output
    );
}

#[test]
fn test_deprecated_method_with_message() {
    let source = r#"
        class MyClass {
            @deprecated("Use newMethod instead")
            oldMethod(): void {
                const x: number = 1
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "deprecated method with message should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.deprecated"),
        "Output should contain TypedLua.deprecated function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("deprecated = TypedLua.deprecated"),
        "Runtime should export deprecated as global alias"
    );

    // Should apply decorator with message using the plain name (global alias)
    assert!(
        output.contains(
            "MyClass.oldMethod = deprecated(\"Use newMethod instead\")(MyClass.oldMethod)"
        ),
        "Output should apply deprecated with message to method but got:\n{}",
        output
    );
}

// ============================================================================
// Multiple Built-in Decorators
// ============================================================================

#[test]
fn test_multiple_builtin_decorators() {
    let source = r#"
        @sealed
        @readonly
        class ImmutableClass {
            value: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "multiple built-in decorators should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library only once
    let runtime_count = output.matches("TypedLua Runtime Library").count();
    assert_eq!(
        runtime_count, 1,
        "Runtime library should be included exactly once"
    );

    // Runtime should export global aliases
    assert!(
        output.contains("sealed = TypedLua.sealed"),
        "Runtime should export sealed as global alias"
    );
    assert!(
        output.contains("readonly = TypedLua.readonly"),
        "Runtime should export readonly as global alias"
    );

    // Should apply both decorators using plain names
    assert!(output.contains("ImmutableClass = sealed(ImmutableClass)"));
    assert!(output.contains("ImmutableClass = readonly(ImmutableClass)"));
}

#[test]
fn test_mix_builtin_and_custom_decorators() {
    let source = r#"
        function logged(target)
            return target
        end

        @logged
        @readonly
        class MyClass {
            value: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "mix of built-in and custom decorators should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.readonly"),
        "Output should contain TypedLua.readonly function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("readonly = TypedLua.readonly"),
        "Runtime should export readonly as global alias"
    );

    // Should apply both decorators (custom uses plain name, built-in uses alias)
    assert!(output.contains("MyClass = logged(MyClass)"));
    assert!(output.contains("MyClass = readonly(MyClass)"));
}

// ============================================================================
// Runtime Library Embedding Tests
// ============================================================================

#[test]
fn test_runtime_library_embedded_only_when_needed() {
    let source = r#"
        function custom(target)
            return target
        end

        @custom
        class MyClass {
            value: number
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "custom decorator should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should NOT include runtime library when no built-in decorators are used
    assert!(
        !output.contains("TypedLua Runtime Library"),
        "Runtime library should not be embedded when only custom decorators are used"
    );
    assert!(!output.contains("TypedLua.readonly"));
    assert!(!output.contains("TypedLua.sealed"));
    assert!(!output.contains("TypedLua.deprecated"));
}

#[test]
fn test_runtime_library_embedded_with_readonly() {
    let source = r#"
        @readonly
        class MyClass {
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok());
    let output = result.unwrap();

    // Should include runtime library
    assert!(output.contains("TypedLua Runtime Library"));
    assert!(output.contains("function TypedLua.readonly(target)"));
    assert!(output.contains("function TypedLua.sealed(target)"));
    assert!(output.contains("function TypedLua.deprecated(message)"));
}

#[test]
fn test_no_runtime_when_no_decorators() {
    let source = r#"
        class MyClass {
            getValue(): number {
                return 42
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok());
    let output = result.unwrap();

    // Should NOT include runtime library when no decorators are used
    assert!(!output.contains("TypedLua Runtime Library"));
    assert!(!output.contains("TypedLua.readonly"));
}

// ============================================================================
// Built-in Decorator Integration Tests
// ============================================================================

#[test]
fn test_builtin_decorator_with_inheritance() {
    let source = r#"
        class Base {
            value: number
        }

        @readonly
        class Derived extends Base {
            name: string
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "built-in decorator with inheritance should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.readonly"),
        "Output should contain TypedLua.readonly function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("readonly = TypedLua.readonly"),
        "Runtime should export readonly as global alias"
    );

    // Should apply decorator using the plain name (global alias)
    assert!(output.contains("Derived = readonly(Derived)"));
}

#[test]
fn test_builtin_decorator_with_interface() {
    let source = r#"
        interface Countable {
            count(): number
        }

        @sealed
        class Counter implements Countable {
            count(): number {
                return 0
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "built-in decorator with interface should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.sealed"),
        "Output should contain TypedLua.sealed function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("sealed = TypedLua.sealed"),
        "Runtime should export sealed as global alias"
    );

    // Should apply decorator using the plain name (global alias)
    assert!(output.contains("Counter = sealed(Counter)"));
}

#[test]
fn test_builtin_decorator_preserves_class_structure() {
    let source = r#"
        @readonly
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
        "built-in decorator should preserve class structure: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library
    assert!(
        output.contains("TypedLua.readonly"),
        "Output should contain TypedLua.readonly function definition"
    );

    // Runtime should export global alias
    assert!(
        output.contains("readonly = TypedLua.readonly"),
        "Runtime should export readonly as global alias"
    );

    // Class should still have its structure
    assert!(output.contains("MyClass.new"));
    assert!(output.contains("function MyClass:getValue"));

    // And decorator should be applied using the plain name (global alias)
    assert!(output.contains("MyClass = readonly(MyClass)"));
}

#[test]
fn test_all_builtin_decorators_together() {
    let source = r#"
        @readonly
        class ReadonlyClass {
        }

        @sealed
        class SealedClass {
        }

        @deprecated
        class DeprecatedClass {
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "all built-in decorators should compile: {:?}",
        result.err()
    );
    let output = result.unwrap();

    // Should include runtime library only once
    let runtime_count = output.matches("TypedLua Runtime Library").count();
    assert_eq!(
        runtime_count, 1,
        "Runtime library should be included exactly once"
    );

    // Runtime should export global aliases
    assert!(
        output.contains("readonly = TypedLua.readonly"),
        "Runtime should export readonly as global alias"
    );
    assert!(
        output.contains("sealed = TypedLua.sealed"),
        "Runtime should export sealed as global alias"
    );
    assert!(
        output.contains("deprecated = TypedLua.deprecated"),
        "Runtime should export deprecated as global alias"
    );

    // Should apply all decorators using plain names
    assert!(output.contains("ReadonlyClass = readonly(ReadonlyClass)"));
    assert!(output.contains("SealedClass = sealed(SealedClass)"));
    assert!(output.contains("DeprecatedClass = deprecated(DeprecatedClass)"));
}
