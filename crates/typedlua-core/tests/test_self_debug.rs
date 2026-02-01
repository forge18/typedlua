// Temporary test file to diagnose self resolution
use std::sync::Arc;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn type_check(source: &str) -> Result<(), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let program = parser.parse().map_err(|e| format!("{:?}", e))?;
    let mut checker = TypeChecker::new(handler, &interner, &common_ids);
    checker
        .check_program(&mut program.clone())
        .map_err(|e| e.message)
}

#[test]
fn test_nongeneric_self_in_return() {
    let r = type_check(
        r#"
        class Foo {
            private x: number
            getX(): number {
                return self.x
            }
        }
    "#,
    );
    eprintln!("Non-generic class with self in method return: {:?}", r);
    assert!(r.is_ok(), "Non-generic class self should work: {:?}", r);
}

#[test]
fn test_generic_self_in_return() {
    let r = type_check(
        r#"
        class Box<T> {
            private value: T
            get(): T {
                return self.value
            }
        }
    "#,
    );
    eprintln!("Generic class with self in method return: {:?}", r);
    assert!(r.is_ok(), "Generic class self should work: {:?}", r);
}

#[test]
fn test_nongeneric_self_assignment() {
    let r = type_check(
        r#"
        class Counter {
            private count: number
            increment(): void {
                self.count = self.count + 1
            }
        }
    "#,
    );
    eprintln!("Non-generic class with self assignment: {:?}", r);
    assert!(r.is_ok(), "Non-generic self assignment should work: {:?}", r);
}
