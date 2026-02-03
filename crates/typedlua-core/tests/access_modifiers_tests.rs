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
fn test_class_with_public_members() {
    let source = r#"
        class Point {
            public x: number = 0
            public y: number = 0
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Class with public members should type-check successfully"
    );
}

#[test]
fn test_class_with_private_members() {
    let source = r#"
        class Point {
            private x: number = 0
            private y: number = 0
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Class with private members should type-check successfully"
    );
}

#[test]
fn test_class_with_protected_members() {
    let source = r#"
        class Point {
            protected x: number = 0
            protected y: number = 0
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Class with protected members should type-check successfully"
    );
}

#[test]
fn test_class_with_mixed_access_modifiers() {
    let source = r#"
        class BankAccount {
            private balance: number = 0
            protected owner: string = "unknown"
            public account_id: string = "12345"
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Class with mixed access modifiers should type-check successfully"
    );
}

#[test]
fn test_class_methods_with_access_modifiers() {
    let source = r#"
        class Calculator {
            private helper(): number {
                return 42
            }

            protected internal_calc(): number {
                return 21
            }

            public compute(): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Class methods with access modifiers should type-check successfully"
    );
}

#[test]
fn test_class_getters_setters_with_access_modifiers() {
    let source = r#"
        class Person {
            private _name: string = ""

            public get name(): string {
                return "test"
            }

            private set name(value: string) {
                -- setter body
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Getters and setters with access modifiers should type-check successfully"
    );
}

#[test]
fn test_class_static_members_with_access_modifiers() {
    let source = r#"
        class Counter {
            private static count: number = 0
            protected static limit: number = 100
            public static name: string = "Counter"
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Static members with access modifiers should type-check successfully"
    );
}

#[test]
fn test_inheritance_with_access_modifiers() {
    let source = r#"
        class Animal {
            private secret: string = "hidden"
            protected species: string = "unknown"
            public name: string = "animal"
        }

        class Dog extends Animal {
            private breed: string = "mutt"
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Inheritance with access modifiers should type-check successfully"
    );
}

#[test]
fn test_default_access_modifier_is_public() {
    let source = r#"
        class Point {
            x: number = 0
            y: number = 0

            get_x(): number {
                return 0
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Members without access modifiers should default to public and type-check successfully"
    );
}

#[test]
fn test_multiple_classes_with_access_modifiers() {
    let source = r#"
        class Point {
            private x: number = 0
            public y: number = 0
        }

        class Circle {
            protected radius: number = 1
            public center: string = "0,0"
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Multiple classes with access modifiers should type-check successfully"
    );
}

#[test]
fn test_protected_access_from_subclass() {
    let source = r#"
        class Animal {
            protected species: string = "unknown"
            protected getSpecies(): string {
                return this.species
            }
        }

        class Dog extends Animal {
            private breed: string = "mutt"

            public getInfo(): string {
                return self.getSpecies() .. " " .. self.breed
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Protected members should be accessible from subclass"
    );
}

#[test]
fn test_protected_inheritance_chain() {
    let source = r#"
        class GrandParent {
            protected value: number = 1
        }

        class Parent extends GrandParent {
            protected multiplier: number = 2
        }

        class Child extends Parent {
            public getValue(): number {
                return self.value * self.multiplier
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Protected members should be accessible through inheritance chain"
    );
}

#[test]
fn test_private_in_protected_base() {
    let source = r#"
        class Base {
            private secret: string = "hidden"
            protected exposed: string = "visible"
        }

        class Derived extends Base {
            public useExposed(): string {
                return self.exposed
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Protected members should be accessible in derived class, private members not accessed"
    );
}

#[test]
fn test_static_protected_access_from_subclass() {
    let source = r#"
        class Animal {
            protected static population: number = 0

            protected constructor() {
                Animal.population = Animal.population + 1
            }
        }

        class Dog extends Animal {
            public static getPopulation(): number {
                return Animal.population
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Protected static members should be accessible from subclass"
    );
}

#[test]
fn test_static_private_access_within_class() {
    let source = r#"
        class Counter {
            private static count: number = 0

            public static increment(): number {
                Counter.count = Counter.count + 1
                return Counter.count
            }

            public static getCount(): number {
                return Counter.count
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Private static members should be accessible within the class"
    );
}

#[test]
fn test_private_member_accessibility_within_class() {
    let source = r#"
        class BankAccount {
            private balance: number = 100

            public deposit(amount: number): void {
                self.balance = self.balance + amount
            }

            public getBalance(): number {
                return self.balance
            }

            public transferTo(other: BankAccount, amount: number): void {
                self.balance = self.balance - amount
                other.balance = other.balance + amount
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Private members should be accessible within the same class instance"
    );
}

#[test]
fn test_protected_vs_private_access() {
    let source = r#"
        class Base {
            private privateField: string = "private"
            protected protectedField: string = "protected"
        }

        class Derived extends Base {
            public testAccess(): string {
                return self.protectedField
            }
        }

        class Unrelated {
            public testAccess(obj: Base): string {
                return obj.protectedField -- This should error: protected not accessible
            }
        }
    "#;

    assert!(
        type_check(source).is_err(),
        "Protected members should not be accessible from unrelated classes"
    );
}

#[test]
fn test_getter_setter_access_modifiers() {
    let source = r#"
        class Person {
            private _age: number = 0

            public get age(): number {
                return self._age
            }

            private set age(value: number) {
                if value >= 0 then
                    self._age = value
                end
            }
        }

        class Employee extends Person {
            public celebrateBirthday(): void {
                self.age = self.age + 1
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Getters and setters with different access modifiers should work"
    );
}

#[test]
fn test_protected_access_from_same_class() {
    // Protected members should be accessible within the same class
    let source = r#"
        class Container {
            protected items: Array<string> = {}
            
            protected addItem(item: string): void {
                table.insert(self.items, item)
            }
            
            public getCount(): number {
                self.addItem("internal")  // Can call protected method from same class
                return #self.items
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Protected members should be accessible within the same class"
    );
}

#[test]
fn test_multiple_access_layers_deep_inheritance() {
    // Test multiple layers of access modifiers in deep inheritance
    let source = r#"
        class Level1 {
            private level1Private: number = 1
            protected level1Protected: number = 10
            public level1Public: number = 100
        }
        
        class Level2 extends Level1 {
            private level2Private: number = 2
            protected level2Protected: number = 20
            
            public accessLevel1(): number {
                return self.level1Protected + self.level2Protected
            }
        }
        
        class Level3 extends Level2 {
            public accessAll(): number {
                return self.level1Protected + self.level2Protected
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Multiple access layers should work correctly in deep inheritance"
    );
}

#[test]
fn test_protected_method_override_access() {
    // Test that protected methods can be overridden and accessed correctly
    let source = r#"
        class Shape {
            protected calculateArea(): number {
                return 0
            }
            
            public getArea(): number {
                return self.calculateArea()
            }
        }
        
        class Circle extends Shape {
            private radius: number = 5
            
            protected calculateArea(): number {
                return 3.14159 * self.radius * self.radius
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Protected method overrides should maintain access rules"
    );
}

#[test]
fn test_static_protected_chain() {
    // Test protected static access through inheritance chain
    let source = r#"
        class A {
            protected static value: string = "A"
        }
        
        class B extends A {
            protected static getValue(): string {
                return B.value
            }
        }
        
        class C extends B {
            public static getChainValue(): string {
                return C.getValue() .. "-" .. C.value
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Protected static should be accessible through inheritance chain"
    );
}
