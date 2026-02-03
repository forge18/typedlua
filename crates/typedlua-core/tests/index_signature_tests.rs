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

    let mut checker = TypeChecker::new(handler, &interner, &common_ids);
    checker.check_program(&mut program).map_err(|e| e.message)?;

    Ok(())
}

#[test]
fn test_interface_with_string_index_signature_compatible_properties() {
    let source = r#"
        interface StringMap {
            [key: string]: number
        }

        class NumberMap implements StringMap {
            count: number = 0
            total: number = 0
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "All properties compatible with index signature should pass"
    );
}

#[test]
fn test_interface_with_string_index_signature_incompatible_property() {
    let source = r#"
        interface StringMap {
            [key: string]: number
        }

        class MixedMap implements StringMap {
            count: number = 0
            name: string = "invalid"
        }
    "#;

    assert!(
        type_check(source).is_err(),
        "Property incompatible with index signature should fail"
    );
}

#[test]
fn test_interface_with_number_index_signature() {
    let source = r#"
        interface NumberIndexed {
            [index: number]: string
        }

        class ArrayLike implements NumberIndexed {
            -- Number index signatures are not fully validated yet
        }
    "#;

    // This should pass for now since we don't fully validate number index signatures
    assert!(
        type_check(source).is_ok(),
        "Number index signatures are not fully validated"
    );
}

#[test]
fn test_interface_with_index_signature_and_methods() {
    let source = r#"
        interface Dictionary {
            [key: string]: number
            get_value(key: string): number
        }

        class SimpleDictionary implements Dictionary {
            count: number = 0

            get_value(key: string): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Methods should not conflict with index signature"
    );
}

#[test]
fn test_multiple_properties_all_compatible() {
    let source = r#"
        interface AllNumbers {
            [key: string]: number
        }

        class Stats implements AllNumbers {
            min: number = 0
            max: number = 100
            avg: number = 50
            count: number = 10
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "All number properties should be compatible"
    );
}

#[test]
fn test_empty_class_with_index_signature() {
    let source = r#"
        interface EmptyIndexed {
            [key: string]: number
        }

        class Empty implements EmptyIndexed {
            -- No properties
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Empty class should be compatible with index signature"
    );
}
