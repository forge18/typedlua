use std::sync::Arc;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

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

#[test]
fn test_final_class_cannot_be_extended() {
    let source = r#"
        final class Animal {
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

    let result = type_check(source);
    assert!(result.is_err(), "Extending a final class should fail");
    assert!(
        result.unwrap_err().contains("Cannot extend final class"),
        "Error message should mention final class"
    );
}

#[test]
fn test_final_method_cannot_be_overridden() {
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

    let result = type_check(source);
    assert!(result.is_err(), "Overriding a final method should fail");
    assert!(
        result.unwrap_err().contains("Cannot override final method"),
        "Error message should mention final method"
    );
}

#[test]
fn test_final_class_can_exist_without_inheritance() {
    let source = r#"
        final class Animal {
            speak(): void {
                print("...")
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Final class without inheritance should work"
    );
}

#[test]
fn test_final_method_can_exist_without_override() {
    let source = r#"
        class Animal {
            final speak(): void {
                print("...")
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Final method without override should work"
    );
}

#[test]
fn test_non_final_class_can_be_extended() {
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
        "Non-final class should be extendable"
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

    assert!(
        type_check(source).is_ok(),
        "Non-final method should be overridable"
    );
}

#[test]
fn test_final_class_with_final_methods() {
    let source = r#"
        final class Animal {
            final speak(): void {
                print("...")
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Final class can have final methods"
    );
}

#[test]
fn test_final_method_in_inheritance_chain() {
    let source = r#"
        class Animal {
            final speak(): void {
                print("...")
            }
        }

        class Mammal extends Animal {
            override speak(): void {
                print("Mammal sound")
            }
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_err(),
        "Cannot override final method in immediate child"
    );
    assert!(
        result.unwrap_err().contains("Cannot override final method"),
        "Error message should mention final method"
    );
}

#[test]
fn test_final_method_across_multiple_inheritance_levels() {
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

    let result = type_check(source);
    assert!(
        result.is_err(),
        "Cannot override final method from ancestor multiple levels up"
    );
    assert!(
        result.unwrap_err().contains("Cannot override final method"),
        "Error message should mention final method"
    );
}

#[test]
fn test_abstract_final_class() {
    let source = r#"
        abstract final class Shape {
            abstract getArea(): number
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Abstract final class should be allowed (can't be extended, must be implemented)"
    );
}

#[test]
fn test_extend_abstract_final_class_fails() {
    let source = r#"
        abstract final class Shape {
            abstract getArea(): number
        }

        class Circle extends Shape {
            radius: number

            constructor(radius: number) {
                self.radius = radius
            }

            getArea(): number {
                return 3.14 * self.radius * self.radius
            }
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_err(),
        "Extending abstract final class should fail (abstract means must be extended, final means can't be extended)"
    );
}

#[test]
fn test_final_class_with_abstract_methods() {
    let source = r#"
        abstract final class Base {
            abstract method1(): void
            abstract method2(): number
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Final class can have abstract methods"
    );
}
