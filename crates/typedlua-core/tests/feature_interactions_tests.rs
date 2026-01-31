use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::{CompilerOptions, OptimizationLevel};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_and_check(source: &str) -> Result<String, String> {
    compile_with_optimization(source, OptimizationLevel::O0)
}

fn compile_with_optimization(source: &str, level: OptimizationLevel) -> Result<String, String> {
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

    let mut codegen = CodeGenerator::new(interner.clone()).with_optimization_level(level);
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Override + Generics
// ============================================================================

#[test]
fn test_generic_method_overriding_generic_parent() {
    let source = r#"
        class Container<T> {
            getValue(): T {
                throw "Not implemented"
            }
        }
        
        class NumberContainer extends Container<number> {
            override getValue(): number {
                return 42
            }
        }
        
        const container = new NumberContainer()
        const val = container.getValue()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic method overriding generic parent should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_generic_method_overriding_non_generic_parent() {
    let source = r#"
        class Processor {
            process(value: any): any {
                return value
            }
        }
        
        class TypedProcessor<T> extends Processor {
            override process(value: T): T {
                return value
            }
        }
        
        const processor = new TypedProcessor<string>()
        const result = processor.process("hello")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic method overriding non-generic parent should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_non_generic_method_overriding_generic_parent() {
    let source = r#"
        class GenericProcessor<T> {
            process(value: T): T {
                return value
            }
        }
        
        class StringProcessor extends GenericProcessor<string> {
            override process(value: string): string {
                return value.toUpperCase()
            }
        }
        
        const processor = new StringProcessor()
        const result = processor.process("hello")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Non-generic method overriding generic parent should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Final + Generics
// ============================================================================

#[test]
fn test_final_generic_class() {
    let source = r#"
        final class ImmutableBox<T> {
            private value: T
            
            constructor(val: T) {
                this.value = val
            }
            
            final getValue(): T {
                return this.value
            }
        }
        
        const box = new ImmutableBox<number>(42)
        const val = box.getValue()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Final generic class should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_final_generic_methods() {
    let source = r#"
        class Container<T> {
            private value: T
            
            constructor(val: T) {
                this.value = val
            }
            
            final getValue(): T {
                return this.value
            }
            
            final setValue(val: T): void {
                this.value = val
            }
        }
        
        class ExtendedContainer<T> extends Container<T> {
            getSize(): number {
                return 1
            }
        }
        
        const container = new ExtendedContainer<string>("hello")
        const val = container.getValue()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Final generic methods should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_generic_class_with_final_methods() {
    let source = r#"
        class DataStore<T> {
            private data: T[]
            
            constructor() {
                this.data = []
            }
            
            final add(item: T): void {
                this.data.push(item)
            }
            
            final getAll(): T[] {
                return this.data
            }
            
            process(item: T): T {
                return item
            }
        }
        
        class NumberStore extends DataStore<number> {
            override process(item: number): number {
                return item * 2
            }
        }
        
        const store = new NumberStore()
        store.add(1)
        store.add(2)
        const all = store.getAll()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic class with final methods should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Primary Constructor + Generics
// ============================================================================

#[test]
fn test_generic_primary_constructor() {
    let source = r#"
        class Container<T>(public value: T) {
            getValue(): T {
                return this.value
            }
        }
        
        const intContainer = new Container<number>(42)
        const strContainer = new Container<string>("hello")
        const intVal = intContainer.getValue()
        const strVal = strContainer.getValue()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generic primary constructor should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_primary_constructor_generic_interface() {
    let source = r#"
        interface Storable<T> {
            getValue(): T
            setValue(val: T): void
        }
        
        class Box<T>(private value: T) implements Storable<T> {
            getValue(): T {
                return this.value
            }
            
            setValue(val: T): void {
                this.value = val
            }
        }
        
        const box: Storable<number> = new Box<number>(42)
        const val = box.getValue()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Primary constructor implementing generic interface should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_primary_constructor_with_multiple_generic_params() {
    let source = r#"
        class Pair<K, V>(public key: K, public value: V) {
            getKey(): K {
                return this.key
            }
            
            getValue(): V {
                return this.value
            }
        }
        
        const pair = new Pair<string, number>("age", 25)
        const k = pair.getKey()
        const v = pair.getValue()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Primary constructor with multiple generic params should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Pattern Matching + Generics
// ============================================================================

#[test]
fn test_pattern_matching_generic_union_types() {
    let source = r#"
        class Result<T, E> {
            private value: T | E
            private isOk: boolean
            
            constructor(val: T | E, ok: boolean) {
                this.value = val
                this.isOk = ok
            }
            
            match<R>(okHandler: (T) => R, errHandler: (E) => R): R {
                if (this.isOk) {
                    return okHandler(this.value as T)
                } else {
                    return errHandler(this.value as E)
                }
            }
        }
        
        const success = new Result<number, string>(42, true)
        const result = success.match(
            (n) => n * 2,
            (e) => 0
        )
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pattern matching with generic union types should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_pattern_guards_with_generic_constraints() {
    let source = r#"
        interface Identifiable {
            id: number
        }
        
        function process<T implements Identifiable>(item: T): string {
            if (item.id > 0) {
                return `Valid: ${item.id}`
            } else {
                return "Invalid"
            }
        }
        
        class User implements Identifiable {
            id: number
            name: string
            
            constructor(id: number, name: string) {
                this.id = id
                this.name = name
            }
        }
        
        const user = new User(1, "Alice")
        const result = process(user)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pattern guards with generic constraints should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Decorators + Primary Constructor
// ============================================================================

#[test]
fn test_class_decorator_on_primary_constructor() {
    let source = r#"
        @sealed
        class User(public name: string, public age: number) {
            greet(): string {
                return `Hello, ${this.name}`
            }
        }
        
        const user = new User("Alice", 25)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Class decorator on primary constructor should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_decorator_on_primary_constructor_params() {
    let source = r#"
        class Config(
            @readonly public host: string,
            @readonly public port: number
        ) {
            getUrl(): string {
                return `${this.host}:${this.port}`
            }
        }
        
        const config = new Config("localhost", 8080)
        const url = config.getUrl()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Decorators on primary constructor params should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_multiple_decorators_primary_constructor() {
    let source = r#"
        @deprecated("Use NewService instead")
        @sealed
        class Service(public name: string) {
            run(): void {
                print(`Running ${this.name}`)
            }
        }
        
        const service = new Service("MyService")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Multiple decorators on primary constructor should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Safe Navigation + Type Narrowing
// ============================================================================

#[test]
fn test_safe_navigation_with_type_narrowing() {
    let source = r#"
        interface Address {
            street: string
            city: string
        }
        
        interface User {
            name: string
            address: Address | nil
        }
        
        function getCity(user: User): string | nil {
            if (user?.address) {
                return user.address.city
            }
            return nil
        }
        
        const user: User = { name: "Alice", address: { street: "123 Main", city: "NYC" } }
        const city = getCity(user)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Safe navigation with type narrowing should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_safe_navigation_chain_with_narrowing() {
    let source = r#"
        interface Company {
            ceo: Person | nil
        }
        
        interface Person {
            name: string
            address: Address | nil
        }
        
        interface Address {
            country: string
        }
        
        function getCeoCountry(company: Company): string | nil {
            const country = company?.ceo?.address?.country
            return country
        }
        
        const company: Company = {
            ceo: {
                name: "Bob",
                address: { country: "USA" }
            }
        }
        const country = getCeoCountry(company)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Safe navigation chain with narrowing should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_safe_navigation_in_conditional() {
    let source = r#"
        interface Response {
            data: Data | nil
        }
        
        interface Data {
            items: string[]
        }
        
        function processResponse(response: Response): number {
            if (response?.data?.items) {
                return response.data.items.length
            }
            return 0
        }
        
        const response: Response = { data: { items: ["a", "b", "c"] } }
        const count = processResponse(response)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Safe navigation in conditional should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Null Coalescing + Type Inference
// ============================================================================

#[test]
fn test_null_coalescing_type_inference() {
    let source = r#"
        function getValueOrDefault<T>(value: T | nil, defaultValue: T): T {
            return value ?? defaultValue
        }
        
        const num = getValueOrDefault<number>(nil, 42)
        const str = getValueOrDefault<string>(nil, "default")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with type inference should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_null_coalescing_chained() {
    let source = r#"
        interface Config {
            host: string | nil
            port: number | nil
        }
        
        function getHost(config: Config): string {
            return config.host ?? "localhost"
        }
        
        function getPort(config: Config): number {
            return config.port ?? 8080
        }
        
        const config: Config = { host: nil, port: nil }
        const host = getHost(config)
        const port = getPort(config)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing chained should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_null_coalescing_with_method_call() {
    let source = r#"
        class Container<T> {
            private value: T | nil
            
            constructor(val: T | nil) {
                this.value = val
            }
            
            getOrDefault(defaultValue: T): T {
                return this.value ?? defaultValue
            }
        }
        
        const container = new Container<number>(nil)
        const value = container.getOrDefault(100)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Null coalescing with method call should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Reflect + Inheritance
// ============================================================================

#[test]
fn test_reflect_getfields_inherited() {
    let source = r#"
        class Animal {
            species: string
            constructor(species: string) {
                this.species = species
            }
        }
        
        class Dog extends Animal {
            breed: string
            constructor(species: string, breed: string) {
                super(species)
                this.breed = breed
            }
        }
        
        const dog = new Dog("Canine", "Labrador")
        const fields = Reflect.getFields(dog)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Reflect.getFields with inheritance should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_reflect_getmethods_inherited() {
    let source = r#"
        class Base {
            baseMethod(): string {
                return "base"
            }
        }
        
        class Derived extends Base {
            derivedMethod(): string {
                return "derived"
            }
        }
        
        const obj = new Derived()
        const methods = Reflect.getMethods(obj)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Reflect.getMethods with inheritance should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_reflect_isinstance_with_inheritance() {
    let source = r#"
        class Shape {
            area(): number {
                return 0
            }
        }
        
        class Circle extends Shape {
            radius: number
            constructor(r: number) {
                super()
                this.radius = r
            }
            
            override area(): number {
                return 3.14159 * this.radius * this.radius
            }
        }
        
        class Rectangle extends Shape {
            width: number
            height: number
            constructor(w: number, h: number) {
                super()
                this.width = w
                this.height = h
            }
            
            override area(): number {
                return this.width * this.height
            }
        }
        
        const circle = new Circle(5)
        const rect = new Rectangle(4, 6)
        
        const isCircleShape = Reflect.isInstance(circle, Shape)
        const isRectShape = Reflect.isInstance(rect, Shape)
        const isCircleRect = Reflect.isInstance(circle, Rectangle)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Reflect.isInstance with inheritance should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Method-to-Function + Virtual Dispatch
// ============================================================================

#[test]
fn test_method_to_function_preserves_polymorphism() {
    let source = r#"
        class Animal {
            speak(): string {
                return "..."
            }
        }
        
        class Dog extends Animal {
            override speak(): string {
                return "Woof!"
            }
        }
        
        class Cat extends Animal {
            override speak(): string {
                return "Meow!"
            }
        }
        
        function makeSpeak(animal: Animal): string {
            return animal.speak()
        }
        
        const dog = new Dog()
        const cat = new Cat()
        const dogSound = makeSpeak(dog)
        const catSound = makeSpeak(cat)
    "#;

    let result = compile_with_optimization(source, OptimizationLevel::O2);
    assert!(
        result.is_ok(),
        "Method-to-function with virtual dispatch should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_method_to_function_chained_calls() {
    let source = r#"
        class Processor {
            process(input: string): string {
                return input.toUpperCase()
            }
        }
        
        class ChainProcessor extends Processor {
            next: Processor | nil
            
            constructor(next: Processor | nil = nil) {
                super()
                this.next = next
            }
            
            override process(input: string): string {
                const result = super.process(input)
                if (this.next) {
                    return this.next.process(result)
                }
                return result
            }
        }
        
        const processor = new ChainProcessor(new Processor())
        const result = processor.process("hello")
    "#;

    let result = compile_with_optimization(source, OptimizationLevel::O2);
    assert!(
        result.is_ok(),
        "Method-to-function with chained calls should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Combined Feature Tests
// ============================================================================

#[test]
fn test_generics_with_decorators_and_override() {
    let source = r#"
        @sealed
        class BaseService<T> {
            protected data: T
            
            constructor(data: T) {
                this.data = data
            }
            
            getData(): T {
                return this.data
            }
            
            process(item: T): T {
                return item
            }
        }
        
        class DataService extends BaseService<number> {
            override process(item: number): number {
                return item * 2
            }
        }
        
        const service = new DataService(10)
        const processed = service.process(5)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Generics with decorators and override should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_safe_navigation_null_coalescing_combined() {
    let source = r#"
        interface User {
            profile: Profile | nil
        }
        
        interface Profile {
            name: string | nil
            age: number | nil
        }
        
        function getUserInfo(user: User): string {
            const name = user?.profile?.name ?? "Anonymous"
            const age = user?.profile?.age ?? 0
            return `${name}, ${age}`
        }
        
        const user: User = { profile: { name: "Alice", age: 25 } }
        const info = getUserInfo(user)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Safe navigation with null coalescing combined should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_pattern_matching_with_null_coalescing() {
    let source = r#"
        type Result<T> = { ok: true, value: T } | { ok: false, error: string }
        
        function unwrapOrDefault<T>(result: Result<T>, defaultValue: T): T {
            if (result.ok) {
                return result.value
            }
            return defaultValue
        }
        
        const success: Result<number> = { ok: true, value: 42 }
        const failure: Result<number> = { ok: false, error: "failed" }
        
        const val1 = unwrapOrDefault(success, 0)
        const val2 = unwrapOrDefault(failure, 0)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Pattern matching with null coalescing should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_interface_default_with_generics() {
    let source = r#"
        interface Container<T> {
            getValue(): T
            setValue(val: T): void {
                // default implementation
            }
        }
        
        class Box implements Container<number> {
            private value: number
            
            constructor(val: number) {
                this.value = val
            }
            
            getValue(): number {
                return this.value
            }
        }
        
        const box = new Box(42)
        const val = box.getValue()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Interface default with generics should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_rich_enum_with_interface() {
    let source = r#"
        interface Printable {
            print(): void
        }
        
        enum Status implements Printable {
            Pending,
            Active,
            Completed
            
            print(): void {
                print(`Status: ${this.name()}`)
            }
        }
        
        const status = Status.Active
        status.print()
        const name = status.name()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Rich enum with interface should compile: {:?}",
        result.err()
    );
}
