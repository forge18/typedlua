# TypedLua Language Features

This document describes the language features available in TypedLua.

## Table of Contents

- [Override Keyword](#override-keyword)
- [Final Keyword](#final-keyword)
- [Primary Constructors](#primary-constructors)

---

## Override Keyword

The `override` keyword provides explicit method overriding semantics, similar to Java, C#, and TypeScript's approach to inheritance.

### Syntax

```typescript
class Parent {
    methodName(): ReturnType {
        // implementation
    }
}

class Child extends Parent {
    override methodName(): ReturnType {
        // overridden implementation
    }
}
```

### Purpose

The `override` keyword serves two purposes:

1. **Documentation**: Makes inheritance relationships explicit and easier to understand
2. **Safety**: Catches typos and accidental shadowing at compile time

### Behavior

When you use the `override` keyword on a method:

- ✅ The type checker **validates** that:
  - The class has a parent class
  - The parent class has a method with the same name
  - The method signatures are compatible (same parameters and return type)

- ❌ The type checker **errors** if:
  - The class has no parent class
  - The parent class doesn't have a method with that name
  - The method signature is incompatible

- ⚠️ The type checker **warns** if:
  - A method overrides a parent method but is missing the `override` keyword

### Examples

#### Valid Override

```typescript
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
```

#### Error: Missing Parent Method

```typescript
class Animal {
    speak(): void {
        print("...")
    }
}

class Dog extends Animal {
    override bark(): void {  // ERROR: Parent has no 'bark' method
        print("Woof!")
    }
}
```

#### Error: No Parent Class

```typescript
class Animal {
    override speak(): void {  // ERROR: Animal has no parent class
        print("...")
    }
}
```

#### Warning: Missing Override Keyword

```typescript
class Animal {
    speak(): void {
        print("...")
    }
}

class Dog extends Animal {
    speak(): void {  // WARNING: Missing 'override' keyword
        print("Woof!")
    }
}
```

### Design Rationale

TypedLua requires explicit `override` keywords to:

1. **Prevent Accidental Shadowing**: If you rename a parent method, child methods with `override` will immediately error, alerting you to update them
2. **Improve Code Readability**: Readers can instantly see which methods are inherited and which are new
3. **Catch Typos**: Misspelling a method name in a child class will error instead of silently creating a new method

---

## Final Keyword

The `final` keyword prevents inheritance and method overriding, similar to Java's `final` and C#'s `sealed`.

### Syntax

#### Final Classes

```typescript
final class ClassName {
    // Cannot be extended
}
```

#### Final Methods

```typescript
class Parent {
    final methodName(): ReturnType {
        // Cannot be overridden
    }
}
```

### Purpose

The `final` keyword serves several purposes:

1. **Design Enforcement**: Explicitly prevents inheritance when a class wasn't designed for it
2. **Security**: Prevents subclasses from overriding critical methods
3. **Optimization**: Enables compiler optimizations (devirtualization)
4. **API Stability**: Signals that a class/method is not meant to be extended

### Behavior

#### Final Classes

When a class is marked `final`:

- ❌ Cannot be extended by another class
- ✅ Can have final or non-final methods
- ✅ Can extend non-final classes

```typescript
final class Animal {
    speak(): void {
        print("...")
    }
}

class Dog extends Animal {  // ERROR: Cannot extend final class 'Animal'
    speak(): void {
        print("Woof!")
    }
}
```

#### Final Methods

When a method is marked `final`:

- ❌ Cannot be overridden in child classes
- ✅ Can exist in final or non-final classes
- ✅ Can override parent methods (with `override final`)

```typescript
class Animal {
    final speak(): void {
        print("...")
    }
}

class Dog extends Animal {
    override speak(): void {  // ERROR: Cannot override final method 'speak'
        print("Woof!")
    }
}
```

### Examples

#### Final Class

```typescript
final class ImmutablePoint {
    constructor(public readonly x: number, public readonly y: number) {}

    distance(): number {
        return Math.sqrt(this.x * this.x + this.y * this.y)
    }
}

// This won't compile:
// class MutablePoint extends ImmutablePoint {}  // ERROR
```

#### Final Method

```typescript
class Shape {
    final validate(): boolean {
        // Critical validation logic that must not be overridden
        return true
    }

    area(): number {
        return 0  // Can be overridden
    }
}

class Circle extends Shape {
    override area(): number {
        return Math.pi * this.radius * this.radius
    }

    // This won't compile:
    // override validate(): boolean { return false }  // ERROR
}
```

#### Combining Final and Abstract

You can use `final` and `abstract` together on a class:

```typescript
abstract final class Utility {
    // Abstract: Cannot be instantiated
    // Final: Cannot be extended

    static helper(): void {
        // Static utility methods only
    }
}
```

This is useful for utility classes that only contain static methods.

### Design Rationale

TypedLua includes `final` to:

1. **Enable Better APIs**: Library authors can explicitly close inheritance hierarchies
2. **Improve Safety**: Prevent subclasses from breaking invariants by overriding critical methods
3. **Enable Optimizations**: Compilers can devirtualize calls to final methods (O3 optimization)
4. **Express Intent**: Make design decisions explicit in code

### Current Limitations

- Final method checking only validates immediate parent class (not full inheritance chain)
- This is sufficient for most use cases and will be enhanced in a future version if needed

---

## Interaction Between Override and Final

The `override` and `final` keywords work together:

### Final Override

A method can be both an override and final:

```typescript
class Animal {
    speak(): void {
        print("...")
    }
}

class Mammal extends Animal {
    override final speak(): void {
        print("Mammal sound")
        // This is the final implementation, no further overrides allowed
    }
}

class Dog extends Mammal {
    override speak(): void {  // ERROR: Cannot override final method
        print("Woof!")
    }
}
```

### Order of Modifiers

The `abstract`, `final`, and `override` keywords can appear in any order:

```typescript
// All of these are valid:
override final speak(): void {}
final override speak(): void {}
```

---

## Compilation

Both `override` and `final` keywords are type-checking features only. They are **erased during compilation** and produce no runtime overhead. The generated Lua code is identical whether you use these keywords or not.

```typescript
// TypedLua source:
class Dog extends Animal {
    override speak(): void {
        print("Woof!")
    }
}

// Compiled Lua output:
Dog = setmetatable({}, { __index = Animal })
function Dog:speak()
    print("Woof!")
end
```

The validation happens entirely at compile time.

---

## Primary Constructors

Primary constructors provide a concise syntax for declaring class properties directly in the constructor parameter list, eliminating boilerplate code. This feature is inspired by C# 12's primary constructors and Kotlin's primary constructors.

### Syntax

```typescript
class ClassName(param1: Type1, param2: Type2, ...) {
    // Properties automatically created from parameters
    // Additional methods and properties can be added
}
```

### Purpose

Primary constructors serve several purposes:

1. **Reduce Boilerplate**: Eliminates repetitive property declarations and assignments
2. **Improve Readability**: Class structure is immediately clear from the declaration
3. **Type Safety**: Parameters are type-checked and properties are correctly typed
4. **Consistency**: Encourages consistent property initialization patterns

### Basic Usage

#### Before Primary Constructors (Verbose)

```typescript
class Point {
    x: number
    y: number

    constructor(x: number, y: number) {
        self.x = x
        self.y = y
    }
}
```

#### After Primary Constructors (Concise)

```typescript
class Point(public x: number, public y: number) {
    // Properties automatically created
}
```

The generated Lua code includes both `._init()` and `.new()` methods:

```lua
function Point._init(self, x, y)
    self.x = x
    self.y = y
end

function Point.new(x, y)
    local self = setmetatable({}, Point)
    Point._init(self, x, y)
    return self
end
```

### Access Modifiers

Primary constructor parameters support access modifiers that control property visibility:

```typescript
class Person(
    public name: string,      // Public property
    private age: number,       // Private property (prefixed with _)
    protected id: string       // Protected property
) {}
```

Generated Lua code applies naming conventions:

```lua
function Person._init(self, name, age, id)
    self.name = name      -- public
    self._age = age       -- private (underscore prefix)
    self.id = id          -- protected
end
```

### Readonly Parameters

Parameters can be marked as `readonly` to prevent modification after initialization:

```typescript
class Circle(public readonly radius: number) {
    // radius cannot be modified after construction
}
```

### Additional Properties and Methods

Primary constructors work seamlessly with additional properties and methods:

```typescript
class Rectangle(public width: number, public height: number) {
    // Additional property with default value
    color: string = "black"

    // Method using primary constructor properties
    area(): number {
        return self.width * self.height
    }

    perimeter(): number {
        return 2 * (self.width + self.height)
    }
}
```

### Inheritance with Primary Constructors

Primary constructors support inheritance with parent constructor argument forwarding:

```typescript
class Shape(public color: string) {
    describe(): string {
        return "A " .. self.color .. " shape"
    }
}

class Circle(public radius: number) extends Shape("red") {
    // Automatically forwards "red" to Shape's constructor
    // Creates own property: radius

    area(): number {
        return 3.14159 * self.radius * self.radius
    }
}
```

Generated Lua code:

```lua
function Circle._init(self, radius)
    Shape._init(self, "red")  -- Forward to parent
    self.radius = radius      -- Initialize own property
end
```

### Empty Primary Constructor

An empty primary constructor is valid and generates a default constructor:

```typescript
class Empty() {
    // No parameters, but still generates constructor methods
}
```

### Combining with Constructor Body

You cannot have both a primary constructor and a parameterized constructor. This is enforced by the parser:

```typescript
// ❌ ERROR: Cannot mix primary and parameterized constructor
class Point(public x: number, public y: number) {
    constructor(x: number, y: number) {  // ERROR
        self.x = x
        self.y = y
    }
}
```

### Examples

#### Simple Data Class

```typescript
class User(
    public readonly id: number,
    public name: string,
    private email: string
) {}

// Usage:
local user = User.new(1, "Alice", "alice@example.com")
print(user.name)  -- "Alice"
-- user.id = 2    -- Would be prevented by readonly (if enforced at runtime)
```

#### With Methods

```typescript
class BankAccount(
    public readonly accountNumber: string,
    private balance: number
) {
    deposit(amount: number): void {
        self.balance = self.balance + amount
    }

    getBalance(): number {
        return self.balance
    }
}
```

#### Multi-Level Inheritance

```typescript
class Animal(public name: string) {
    speak(): void {
        print(self.name .. " makes a sound")
    }
}

class Mammal(public name: string, protected furColor: string) extends Animal(name) {
    // Forwards name to Animal
    // Adds furColor property
}

class Dog(public name: string, public breed: string) extends Mammal(name, "brown") {
    // Forwards name and "brown" to Mammal
    // Adds breed property

    override speak(): void {
        print(self.name .. " barks!")
    }
}
```

### Design Rationale

TypedLua includes primary constructors to:

1. **Reduce Code Size**: Typical class declarations reduced by 40-50%
2. **Improve Maintainability**: Fewer lines of code to maintain
3. **Enhance Readability**: Class structure visible at a glance
4. **Prevent Errors**: No risk of forgetting to assign a parameter to a property
5. **Modern Syntax**: Aligns with modern language features (C#, Kotlin, TypeScript)

### Compilation

Primary constructors generate standard Lua OOP patterns:

```typescript
// TypedLua source:
class Point(public x: number, public y: number) {}

// Compiled Lua output:
local Point = {}
Point.__index = Point

function Point._init(self, x, y)
    self.x = x
    self.y = y
end

function Point.new(x, y)
    local self = setmetatable({}, Point)
    Point._init(self, x, y)
    return self
end
```

The `._init()` method is generated for consistency with inheritance patterns, allowing child classes to call the parent's `._init()` method.

### Type Checking

The type checker validates:

- ✅ Parameter types are valid
- ✅ Access modifiers are correctly applied
- ✅ Parent constructor arguments match parent's primary constructor
- ✅ No duplicate property names between primary constructor and class body
- ❌ Error if both primary constructor and parameterized constructor exist
- ❌ Error if property declared in class body conflicts with primary constructor parameter

### Current Limitations

- Constructor body validation is not yet implemented (validation logic would go in separate methods)
- Default parameter values are parsed but not yet enforced in code generation

---

## Related Documentation

- [Type System](./designs/TypedLua-Design.md)
- [OOP Features](./designs/TypedLua-Design.md#classes)
- [Additional Features](./designs/Additional-Features-Design.md)
