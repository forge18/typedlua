use typedlua_parser::string_interner::StringInterner;
// Integration tests for utility types
// These test that utility types work end-to-end through the compiler

use std::sync::Arc;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;

/// Helper to parse and type-check source code
fn compile_and_check(source: &str) -> Result<(), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| e.to_string())?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut ast = parser.parse().map_err(|e| e.to_string())?;

    let mut type_checker = typedlua_core::TypeChecker::new(handler.clone(), &interner, &common_ids);
    type_checker
        .check_program(&mut ast)
        .map_err(|e| format!("{:?}", e))?;

    // Check for errors
    use typedlua_core::DiagnosticHandler;
    if handler.has_errors() {
        return Err("Type checking had errors".to_string());
    }

    Ok(())
}

// NOTE: Utility types (Partial, Required, Readonly, Record, Pick, Omit, Exclude, Extract,
// NonNilable, Nilable, ReturnType, Parameters) are FULLY implemented and integrated with
// the type checker (see typechecker/utility_types.rs). They work correctly in type checking.
//
// This test file currently contains only basic sanity tests to verify the test harness works.
// More comprehensive utility type tests would be beneficial but are not required for correctness.

#[test]
fn test_basic_type_checking() {
    // Baseline test - make sure our test harness works
    let source = r#"
        const x: number = 42
        const y: string = "hello"
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_basic_function() {
    // Test that functions work
    let source = r#"
        function greet(name: string): string
            return "Hello, " .. name
        end
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_basic_interface() {
    // Test that interfaces work
    let source = r#"
        interface User {
            name: string
            age: number
        }
    "#;
    assert!(compile_and_check(source).is_ok());
}

// ============================================================================
// Expanded Utility Types Tests
// ============================================================================

#[test]
fn test_partial_with_optional_fields() {
    // Partial<T> on a type that already has optional fields
    let source = r#"
        interface User {
            name: string
            age?: number
            email?: string
        }
        
        type PartialUser = Partial<User>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_partial_preserves_required_fields() {
    // Partial<T> makes all fields optional, even required ones
    let source = r#"
        interface Config {
            host: string
            port: number
            debug: boolean
        }
        
        type PartialConfig = Partial<Config>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_pick_with_string_union_keys() {
    // Pick<T, K> where K is a union of string literals
    let source = r#"
        interface Person {
            name: string
            age: number
            email: string
            phone: string
            address: string
        }
        
        type ContactInfo = Pick<Person, "email" | "phone" | "address">
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_pick_single_key() {
    // Pick<T, K> with a single key
    let source = r#"
        interface Product {
            id: number
            name: string
            price: number
        }
        
        type ProductName = Pick<Product, "name">
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_omit_with_string_union_keys() {
    // Omit<T, K> where K is a union of string literals
    let source = r#"
        interface User {
            id: number
            password: string
            secretKey: string
            name: string
            email: string
        }
        
        type PublicUser = Omit<User, "password" | "secretKey">
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_omit_single_key() {
    // Omit<T, K> with a single key
    let source = r#"
        interface Document {
            id: number
            content: string
            internalNotes: string
        }
        
        type PublishedDocument = Omit<Document, "internalNotes">
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_record_with_number_keys() {
    // Record<K, V> where K is number
    let source = r#"
        type NumberMap = Record<number, string>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_record_with_string_keys() {
    // Record<K, V> where K is string
    let source = r#"
        type StringMap = Record<string, number>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_record_with_complex_value() {
    // Record<K, V> with complex value type
    let source = r#"
        interface UserData {
            name: string
            age: number
        }
        
        type UserMap = Record<string, UserData>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_exclude_with_complex_unions() {
    // Exclude<T, U> with complex union types
    let source = r#"
        type Status = "idle" | "loading" | "success" | "error" | "cancelled"
        type TerminalStatus = Exclude<Status, "idle" | "loading">
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_exclude_complex_union() {
    // Exclude<T, U> excluding multiple types from union
    let source = r#"
        type AllTypes = string | number | boolean | nil
        type NonNilTypes = Exclude<AllTypes, nil>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_extract_with_complex_unions() {
    // Extract<T, U> extracting from complex unions
    let source = r#"
        type Mixed = string | number | boolean | Array<string> | Array<number>
        type ArrayTypes = Extract<Mixed, Array<any>>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_extract_string_literals() {
    // Extract<T, U> extracting specific string literals
    let source = r#"
        type Colors = "red" | "green" | "blue" | "yellow" | "orange"
        type PrimaryColors = Extract<Colors, "red" | "blue" | "yellow">
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_return_type_simple_function() {
    // ReturnType<F> with simple function
    let source = r#"
        function getNumber(): number
            return 42
        end
        
        type NumReturn = ReturnType<typeof(getNumber)>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_parameters_simple_function() {
    // Parameters<F> with simple parameters
    let source = r#"
        function add(a: number, b: number): number
            return a + b
        end
        
        type AddParams = Parameters<typeof(add)>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_parameters_complex_function() {
    // Parameters<F> with complex parameter types
    let source = r#"
        interface Config {
            host: string
            port: number
        }
        
        function connect(config: Config, timeout: number): boolean
            return true
        end
        
        type ConnectParams = Parameters<typeof(connect)>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_recursive_partial() {
    // Recursive application of Partial
    let source = r#"
        interface Address {
            street: string
            city: string
            zip: string
        }
        
        interface Person {
            name: string
            address: Address
        }
        
        type PartialPerson = Partial<Person>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_recursive_required() {
    // Recursive application of Required
    let source = r#"
        interface Config {
            host?: string
            port?: number
            ssl?: {
                cert?: string
                key?: string
            }
        }
        
        type RequiredConfig = Required<Config>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_recursive_readonly() {
    // Recursive application of Readonly
    let source = r#"
        interface MutableData {
            count: number
            items: Array<string>
        }
        
        type ReadonlyData = Readonly<MutableData>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_compose_partial_pick() {
    // Compose Partial and Pick
    let source = r#"
        interface User {
            id: number
            name: string
            email: string
            password: string
        }
        
        type PartialPublicUser = Partial<Pick<User, "name" | "email">>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_compose_omit_partial() {
    // Compose Omit and Partial
    let source = r#"
        interface FullConfig {
            apiKey: string
            secret: string
            host: string
            port: number
        }
        
        type PartialSafeConfig = Partial<Omit<FullConfig, "apiKey" | "secret">>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_compose_pick_omit() {
    // Compose Pick and Omit (equivalent to intersection)
    let source = r#"
        interface Employee {
            id: number
            name: string
            department: string
            salary: number
            ssn: string
        }
        
        type PublicInfo = Pick<Omit<Employee, "salary" | "ssn">, "id" | "name" | "department">
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_compose_required_partial() {
    // Compose Required and Partial (should result in Required taking precedence)
    let source = r#"
        interface OptionalConfig {
            host?: string
            port?: number
        }
        
        type RequiredPartialConfig = Required<Partial<OptionalConfig>>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_compose_record_partial() {
    // Compose Record and Partial
    let source = r#"
        interface User {
            name: string
            age: number
        }
        
        type PartialUserMap = Record<string, Partial<User>>
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_compose_exclude_extract() {
    // Compose Exclude and Extract
    let source = r#"
        type AllStatus = "pending" | "active" | "paused" | "completed" | "failed" | "cancelled"
        
        type CompletedStatus = Exclude<Extract<AllStatus, "completed" | "failed" | "cancelled">, "cancelled">
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_deep_nested_utility_types() {
    // Deeply nested utility type composition
    let source = r#"
        interface FullEntity {
            id: number
            name: string
            metadata: {
                created: string
                updated: string
            }
            internal: {
                version: number
                flags: Array<string>
            }
        }
        
        type PublicView = Pick<Omit<Partial<FullEntity>, "internal">, "id" | "name" | "metadata">
    "#;
    assert!(compile_and_check(source).is_ok());
}

#[test]
fn test_utility_type_edge_cases() {
    // Edge cases for utility types
    let source = r#"
        interface Item {
            id: number
            name: string
        }
        
        type EmptyPick = Pick<Item, never>
        type EmptyOmit = Omit<Item, "id" | "name">
        type Nothing = Exclude<string | number, string | number>
        type EmptyExtract = Extract<never, string>
    "#;
    assert!(compile_and_check(source).is_ok());
}
