use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_and_check(source: &str) -> Result<String, String> {
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

    let options = CompilerOptions::default();

    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Generic Classes with Fields and Methods
// ============================================================================

#[test]
fn test_generic_class_with_fields() {
    let source = r#"
        class Container<T> {
            value: T
            constructor(val: T) {
                self.value = val
            }
        }
        
        const intContainer = new Container<number>(42)
        const stringContainer = new Container<string>("hello")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic class with fields should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_generic_class_with_methods() {
    let source = r#"
        class Box<T> {
            private value: T
            
            constructor(val: T) {
                self.value = val
            }
            
            getValue(): T {
                return self.value
            }
            
            setValue(val: T): void {
                self.value = val
            }
        }
        
        const box = new Box<number>(10)
        const val = box.getValue()
        box.setValue(20)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic class with methods should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_generic_methods_on_non_generic_class() {
    let source = r#"
        class Processor {
            process<T>(value: T): T {
                return value
            }
            
            transform<T, U>(value: T, transformer: (T) => U): U {
                return transformer(value)
            }
        }
        
        const processor = new Processor()
        const num = processor.process<number>(42)
        const str = processor.process<string>("hello")
        const doubled = processor.transform<number, number>(5, (n) => n * 2)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic methods on non-generic class should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Nested Generic Types
// ============================================================================

#[test]
fn test_nested_generic_types() {
    let source = r#"
        class Box<T> {
            value: T
            constructor(val: T) {
                self.value = val
            }
        }
        
        class Container<T> {
            inner: T
            constructor(val: T) {
                self.inner = val
            }
        }
        
        const nestedBox = new Container<Box<number>>(new Box<number>(42))
        const extracted = nestedBox.inner.value
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Nested generic types should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_triple_nested_generics() {
    let source = r#"
        class Wrapper<T> {
            data: T
            constructor(d: T) {
                self.data = d
            }
        }
        
        const triple = new Wrapper<Wrapper<Wrapper<string>>>(
            new Wrapper<Wrapper<string>>(
                new Wrapper<string>("deep")
            )
        )
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Triple nested generics should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Recursive Generic Types
// ============================================================================

#[test]
fn test_recursive_generic_tree_node() {
    let source = r#"
        class TreeNode<T> {
            value: T
            left: TreeNode<T> | nil
            right: TreeNode<T> | nil
            
            constructor(val: T) {
                self.value = val
                self.left = nil
                self.right = nil
            }
            
            addLeft(val: T): TreeNode<T> {
                const node = new TreeNode<T>(val)
                self.left = node
                return node
            }
            
            addRight(val: T): TreeNode<T> {
                const node = new TreeNode<T>(val)
                self.right = node
                return node
            }
        }
        
        const root = new TreeNode<number>(10)
        root.addLeft(5)
        root.addRight(15)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Recursive generic TreeNode should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_recursive_generic_linked_list() {
    let source = r#"
        class ListNode<T> {
            value: T
            next: ListNode<T> | nil
            
            constructor(val: T) {
                self.value = val
                self.next = nil
            }
            
            append(val: T): ListNode<T> {
                const node = new ListNode<T>(val)
                self.next = node
                return node
            }
        }
        
        const head = new ListNode<string>("first")
        head.append("second").append("third")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Recursive generic linked list should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Generic Constraints with Multiple Interfaces
// ============================================================================

#[test]
fn test_generic_constraints_with_multiple_interfaces() {
    let source = r#"
        interface Comparable {
            compareTo(other: Comparable): number
        }
        
        interface Serializable {
            serialize(): string
        }
        
        class SortedList<T implements Comparable & Serializable> {
            items: T[]
            
            constructor() {
                self.items = []
            }
            
            add(item: T): void {
                self.items.push(item)
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic constraints with multiple interfaces should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_generic_constraints_with_intersection() {
    let source = r#"
        interface Named {
            name: string
        }
        
        interface Aged {
            age: number
        }
        
        class PersonRegistry<T implements Named & Aged> {
            people: T[]
            
            constructor() {
                self.people = []
            }
            
            register(person: T): void {
                self.people.push(person)
            }
            
            findByName(n: string): T | nil {
                for (const p of self.people) {
                    if (p.name == n) {
                        return p
                    }
                }
                return nil
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic constraints with intersection should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Conditional Types
// ============================================================================

#[test]
fn test_conditional_type_basic() {
    let source = r#"
        type IsString<T> = T extends string ? true : false
        
        const isStr: IsString<string> = true
        const isNum: IsString<number> = false
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Basic conditional type should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_conditional_type_with_union() {
    let source = r#"
        type ExtractString<T> = T extends string ? T : never
        
        type StringOrNumber = string | number
        type OnlyString = ExtractString<StringOrNumber>
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Conditional type with union should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_conditional_type_nested() {
    let source = r#"
        type DeepCheck<T> = T extends string 
            ? "string" 
            : T extends number 
                ? "number" 
                : "other"
        
        const strType: DeepCheck<string> = "string"
        const numType: DeepCheck<number> = "number"
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Nested conditional type should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Mapped Types
// ============================================================================

#[test]
fn test_mapped_type_basic() {
    let source = r#"
        type Nullable<T> = { [K in keyof T]: T[K] | nil }
        
        interface User {
            name: string
            age: number
        }
        
        type NullableUser = Nullable<User>
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Basic mapped type should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_mapped_type_readonly() {
    let source = r#"
        type Readonly<T> = { readonly [K in keyof T]: T[K] }
        
        interface Config {
            host: string
            port: number
        }
        
        type ReadonlyConfig = Readonly<Config>
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Readonly mapped type should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_mapped_type_optional() {
    let source = r#"
        type Optional<T> = { [K in keyof T]?: T[K] }
        
        interface RequiredFields {
            id: number
            name: string
        }
        
        type OptionalFields = Optional<RequiredFields>
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Optional mapped type should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_mapped_type_remove_modifiers() {
    let source = r#"
        type Mutable<T> = { -readonly [K in keyof T]: T[K] }
        type Required<T> = { [K in keyof T]-?: T[K] }
        
        interface ImmutableUser {
            readonly id: number
            readonly name?: string
        }
        
        type MutableUser = Mutable<ImmutableUser>
        type RequiredUser = Required<ImmutableUser>
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Mapped type with modifier removal should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Template Literal Types
// ============================================================================

#[test]
fn test_template_literal_type_basic() {
    let source = r#"
        type EventName<T extends string> = `on${T}`
        
        type ClickEvent = EventName<"Click">
        type HoverEvent = EventName<"Hover">
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Template literal type should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_template_literal_type_with_union() {
    let source = r#"
        type HttpMethod = "GET" | "POST" | "PUT" | "DELETE"
        type Endpoint = `/api/${HttpMethod}`
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Template literal type with union should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Infer Keyword
// ============================================================================

#[test]
fn test_infer_keyword_basic() {
    let source = r#"
        type ReturnType<T> = T extends (...args: any[]) => infer R ? R : never
        
        declare function greet(): string
        type GreetReturn = ReturnType<typeof greet>
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Infer keyword in conditional type should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_infer_keyword_with_array() {
    let source = r#"
        type ElementType<T> = T extends (infer E)[] ? E : never
        
        type Numbers = number[]
        type Num = ElementType<Numbers>
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Infer keyword with array should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Recursive Utility Types
// ============================================================================

#[test]
fn test_deep_partial() {
    let source = r#"
        type DeepPartial<T> = {
            [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K]
        }
        
        interface NestedUser {
            name: string
            address: {
                street: string
                city: string
            }
        }
        
        type PartialUser = DeepPartial<NestedUser>
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "DeepPartial recursive utility type should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_deep_readonly() {
    let source = r#"
        type DeepReadonly<T> = {
            readonly [K in keyof T]: T[K] extends object ? DeepReadonly<T[K]> : T[K]
        }
        
        interface MutableConfig {
            database: {
                host: string
                port: number
            }
            cache: {
                ttl: number
            }
        }
        
        type ImmutableConfig = DeepReadonly<MutableConfig>
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "DeepReadonly recursive utility type should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_deep_required() {
    let source = r#"
        type DeepRequired<T> = {
            [K in keyof T]-?: T[K] extends object ? DeepRequired<T[K]> : T[K]
        }
        
        interface OptionalNested {
            user?: {
                name?: string
                age?: number
            }
        }
        
        type RequiredNested = DeepRequired<OptionalNested>
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "DeepRequired recursive utility type should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Generic Type Inference
// ============================================================================

#[test]
fn test_generic_type_inference_basic() {
    let source = r#"
        function identity<T>(value: T): T {
            return value
        }
        
        const num = identity(42)
        const str = identity("hello")
        const arr = identity([1, 2, 3])
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic type inference should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_generic_type_inference_with_constraints() {
    let source = r#"
        interface HasLength {
            length: number
        }
        
        function logLength<T implements HasLength>(item: T): T {
            print(item.length)
            return item
        }
        
        const str = logLength("hello")
        const arr = logLength([1, 2, 3])
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic type inference with constraints should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Generic Default Parameters
// ============================================================================

#[test]
fn test_generic_default_parameters() {
    let source = r#"
        class Container<T = string> {
            value: T
            constructor(val: T) {
                self.value = val
            }
        }
        
        const defaultContainer = new Container("default")
        const explicitContainer = new Container<number>(42)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic default parameters should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_generic_default_with_multiple_params() {
    let source = r#"
        class Map<K, V = string> {
            entries: [K, V][]
            
            constructor() {
                self.entries = []
            }
            
            set(key: K, value: V): void {
                self.entries.push([key, value])
            }
        }
        
        const stringMap = new Map<number>()
        const numberMap = new Map<string, number>()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic default with multiple params should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Generic Type Aliases
// ============================================================================

#[test]
fn test_generic_type_alias_basic() {
    let source = r#"
        type Pair<T, U> = [T, U]
        type Result<T, E> = { ok: true, value: T } | { ok: false, error: E }
        
        const pair: Pair<string, number> = ["age", 25]
        const success: Result<number, string> = { ok: true, value: 42 }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic type aliases should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_generic_type_alias_with_constraints() {
    let source = r#"
        interface Comparable {
            compareTo(other: Comparable): number
        }
        
        type SortedArray<T implements Comparable> = T[]
        
        class NumberWrapper implements Comparable {
            value: number
            constructor(v: number) {
                self.value = v
            }
            compareTo(other: Comparable): number {
                if (other instanceof NumberWrapper) {
                    return self.value - other.value
                }
                return 0
            }
        }
        
        const sorted: SortedArray<NumberWrapper> = []
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic type alias with constraints should compile: {:?}",
        result.err()
    );
}
