# TypedLua Feature Roadmap

This document specifies new features to be implemented in the TypedLua compiler. TypedLua is a Rust-based compiler that compiles a TypeScript-inspired typed language to Lua.

**Repository:** `/Users/forge18/Repos/typed-lua`

**Architecture:**
- `crates/typedlua-core/` — Lexer, parser, type checker, codegen
- `crates/typedlua-lsp/` — Language Server Protocol
- `crates/typedlua-cli/` — CLI

---

## Table of Contents

1. [Core Language Features](#core-language-features)
   - [Override Keyword](#override-keyword)
   - [Final Keyword](#final-keyword)
   - [Primary Constructors](#primary-constructors)
   - [Null Coalescing Operator](#null-coalescing-operator)
   - [Safe Navigation Operator](#safe-navigation-operator)
   - [Operator Overloading](#operator-overloading)
2. [Exception Handling](#exception-handling)
3. [Rich Enums (Java-style)](#rich-enums-java-style)
4. [Interfaces with Default Implementations](#interfaces-with-default-implementations)
5. [File-Based Namespaces](#file-based-namespaces)
6. [Template Literal Enhancements](#template-literal-enhancements)
7. [Reflection System](#reflection-system)
8. [Compiler Optimizations](#compiler-optimizations)
9. [Implementation Priority](#implementation-priority)
10. [Excluded Features](#excluded-features)

---

## Core Language Features

### Override Keyword

**Effort:** 2-4 hours

**Description:** Add `override` keyword for methods that override a parent class method. The compiler validates that the parent class actually has the method being overridden.

**Syntax:**

```lua
class Animal {
  speak(): void {
    print("...")
  }
}

class Dog extends Animal {
  override speak(): void {
    print("Woof!")
  }
  
  override fly(): void { }  -- ERROR: Animal has no method 'fly'
}
```

**Implementation:**

1. **Lexer** (`crates/typedlua-core/src/lexer/token.rs`):
   - Add `Override` to `TokenKind` enum
   - Add to `from_keyword()` match

2. **AST** (`crates/typedlua-core/src/ast/statement.rs`):
   - Add `is_override: bool` to `MethodDeclaration`

3. **Parser** (`crates/typedlua-core/src/parser/`):
   - Parse `override` keyword before method declarations in classes
   - Set `is_override: true` on the `MethodDeclaration`

4. **Type Checker** (`crates/typedlua-core/src/typechecker/`):
   - When checking a method with `is_override: true`:
     - Verify the class has a parent (`extends`)
     - Verify the parent class has a method with the same name
     - Verify the signatures are compatible
   - Error if override is specified but no parent method exists
   - Optional: Warn if overriding without `override` keyword

**Codegen:** No changes needed — `override` is compile-time only.

---

### Final Keyword

**Effort:** 1-2 days

**Description:** Add `final` keyword to prevent inheritance (on classes) or overriding (on methods).

**Syntax:**

```lua
-- Final class: cannot be extended
final class Singleton {
  private static instance: Singleton | nil = nil
  
  static getInstance(): Singleton {
    if Singleton.instance == nil then
      Singleton.instance = Singleton.new()
    end
    return Singleton.instance
  }
}

class MySingleton extends Singleton { }  -- ERROR: Cannot extend final class

-- Final method: cannot be overridden
class Base {
  final getId(): number {
    return self.id
  }
}

class Derived extends Base {
  override getId(): number { }  -- ERROR: Cannot override final method
}
```

**Implementation:**

1. **Lexer**:
   - Add `Final` to `TokenKind`

2. **AST**:
   - Add `is_final: bool` to `ClassDeclaration`
   - Add `is_final: bool` to `MethodDeclaration`

3. **Parser**:
   - Parse `final` before `class` keyword
   - Parse `final` before method declarations

4. **Type Checker**:
   - When a class extends another: check parent is not `is_final`
   - When a method has `is_override`: check parent method is not `is_final`

**Codegen:** No changes — `final` is compile-time only.

---

### Primary Constructors

**Effort:** 1 week

**Description:** C# 12-style primary constructors that eliminate boilerplate by automatically creating properties from constructor parameters. Reduces typical class declarations by 40-50%.

**Syntax:**

```lua
-- Before: Verbose pattern
class User {
  id: number
  name: string
  email: string

  constructor(id: number, name: string, email: string) {
    self.id = id
    self.name = name
    self.email = email
  }
}

-- After: Primary constructor
class User(id: number, name: string, email: string) {
  // Properties automatically created

  greet(): string {
    return "Hello, " .. self.name
  }
}
```

**With Access Modifiers:**

```lua
class BankAccount(public id: string, private balance: number, protected pin: string) {
  // id: public property
  // balance: private property
  // pin: protected property

  deposit(amount: number): void {
    self.balance = self.balance + amount
  }
}
```

**With Additional Properties:**

```lua
class Rectangle(width: number, height: number) {
  // width and height are properties from primary constructor

  color: string = "black"  // Additional property with default

  area(): number {
    return self.width * self.height
  }
}
```

**With Constructor Body (Validation/Initialization):**

```lua
class Circle(radius: number) {
  constructor() {
    if self.radius <= 0 then
      error("Radius must be positive")
    end
  }

  area(): number {
    return math.pi * self.radius ^ 2
  }
}
```

**With Inheritance:**

```lua
class Person(name: string, age: number) {
  greet(): string {
    return "I'm " .. self.name .. ", age " .. self.age
  }
}

class Employee(name: string, age: number, salary: number) extends Person(name, age) {
  // Automatically forwards name, age to parent primary constructor

  info(): string {
    return self:greet() .. ", salary: $" .. self.salary
  }
}
```

**Complex Example:**

```lua
class User(
  public readonly id: number,
  private name: string,
  protected email: string
) {
  // Additional properties
  status: string = "active"
  createdAt: number

  // Constructor body for complex initialization
  constructor() {
    self.createdAt = os.time()
    self:validateEmail()
  }

  private validateEmail(): void {
    if not self.email:match("^[^@]+@[^@]+%.[^@]+$") then
      error("Invalid email format")
    end
  }

  getName(): string {
    return self.name
  }
}

const user = User.new(1, "Alice", "alice@example.com")
print(user.id)          // 1 (public)
print(user:getName())   // "Alice" (via getter)
print(user.email)       // ERROR: protected member
```

### Rules

1. **Parameters become properties** - All primary constructor parameters automatically create properties
2. **Access modifiers apply** - `public`, `private`, `protected`, `readonly` work on parameters
3. **Default is public** - Parameters without modifiers create public properties
4. **Constructor body optional** - Can add `constructor() { }` for validation/initialization logic
5. **Runs after property creation** - Constructor body executes after all properties are assigned
6. **Mix with regular properties** - Can still declare additional properties normally
7. **Inheritance** - Child class forwards parameters to parent: `extends Parent(param1, param2)`
8. **Only one primary constructor** - Cannot have both `class C(params)` and separate `constructor(params)`

### Implementation

1. **Lexer**: No changes needed

2. **AST** (`crates/typedlua-core/src/ast/statement.rs`):
   ```rust
   pub struct ClassDeclaration {
       pub name: String,
       pub primary_constructor: Option<Vec<ConstructorParameter>>,
       pub parent: Option<ParentClass>,
       pub implements: Vec<String>,
       pub members: Vec<ClassMember>,
       // ...
   }

   pub struct ConstructorParameter {
       pub name: String,
       pub type_annotation: Type,
       pub access_modifier: Option<AccessModifier>,  // public, private, protected
       pub is_readonly: bool,
   }

   pub struct ParentClass {
       pub name: String,
       pub arguments: Vec<String>,  // For primary constructor forwarding
   }
   ```

3. **Parser**:
   - Parse `class Name(params)` vs `class Name`
   - If `(` follows class name, parse primary constructor parameters
   - Parse access modifiers on parameters: `public id: number`
   - Parse `extends Parent(arg1, arg2)` for constructor forwarding
   - Error if both primary constructor and regular `constructor()` with parameters exist

4. **Type Checker**:
   - Create property declarations from primary constructor parameters
   - Validate access modifiers on parameters
   - Check parent constructor argument count/types match parent primary constructor
   - Ensure constructor body (if present) doesn't redeclare parameter properties

5. **Codegen**:

```lua
-- Input
class User(public id: number, private name: string) {
  email: string = "unknown"

  constructor() {
    print("User created: " .. self.name)
  }

  getName(): string {
    return self.name
  }
}

-- Output
local User = {}
User.__index = User

function User.new(id, name)
  local self = setmetatable({}, User)

  -- Primary constructor properties
  self.id = id         -- public
  self._name = name    -- private (prefixed)

  -- Additional properties
  self.email = "unknown"

  -- Constructor body
  print("User created: " .. self._name)

  return self
end

function User:getName()
  return self._name
end

return User
```

**With Inheritance:**

```lua
-- Input
class Person(name: string, age: number) { }
class Employee(name: string, age: number, salary: number) extends Person(name, age) { }

-- Output
local Person = {}
Person.__index = Person

function Person.new(name, age)
  local self = setmetatable({}, Person)
  self.name = name
  self.age = age
  return self
end

local Employee = {}
Employee.__index = Employee
setmetatable(Employee, { __index = Person })

function Employee.new(name, age, salary)
  local self = setmetatable({}, Employee)

  -- Call parent constructor with forwarded params
  Person.new.__call(self, name, age)

  -- Child-specific property
  self.salary = salary

  return self
end

return Employee
```

### Benefits

**Before Primary Constructors:**
```lua
class Rectangle {
  width: number
  height: number
  color: string
  x: number
  y: number

  constructor(width: number, height: number, color: string, x: number, y: number) {
    self.width = width
    self.height = height
    self.color = color
    self.x = x
    self.y = y
  }
}
```
**Lines:** 15

**After Primary Constructors:**
```lua
class Rectangle(width: number, height: number, color: string, x: number, y: number) {
  area(): number {
    return self.width * self.height
  }
}
```
**Lines:** 5

**Reduction:** 67% fewer lines for property-heavy classes

### Edge Cases

**Empty Primary Constructor:**
```lua
class Singleton() {
  private static instance: Singleton | nil = nil
}
// Valid but uncommon
```

**Readonly Properties:**
```lua
class Config(readonly host: string, readonly port: number) {
  // host and port cannot be reassigned
}

const config = Config.new("localhost", 8080)
config.port = 9000  // ERROR: Cannot assign to readonly property
```

**Mixing Patterns:**
```lua
class User(id: number) {
  name: string

  constructor(name: string) {  // ERROR: Cannot have both primary constructor and parameterized constructor
    self.name = name
  }
}

// Correct approach:
class User(id: number, name: string) {
  constructor() {
    // Initialization only, no parameters
    print("User created")
  }
}
```

---

### Null Coalescing Operator

**Effort:** 2-3 days

**Description:** Add `??` operator that returns the right operand only when the left operand is `nil`. Unlike `or`, it does not treat `false` as falsy.

**Syntax:**

```lua
const port = config.port ?? 3000
const enabled = settings.debug ?? false  -- Correctly handles explicit false

-- Difference from `or`:
const x = false
const y = x or true   -- y = true (WRONG for boolean config)
const z = x ?? true   -- z = false (CORRECT)
```

**Implementation:**

1. **Lexer**:
   - Add `QuestionQuestion` to `TokenKind`
   - Parse `??` as a single token (not two `?`)

2. **AST** (`crates/typedlua-core/src/ast/expression.rs`):
   - Add `NullCoalesce` to `BinaryOp` enum

3. **Parser**:
   - Parse `??` as binary operator with appropriate precedence (lower than comparison, higher than `or`)

4. **Type Checker**:
   - Left operand can be any type
   - Right operand should be compatible with non-nil version of left
   - Result type: non-nil union of both sides

5. **Codegen**:

```lua
-- Input
a ?? b

-- Output (simple, evaluates `a` twice)
(a ~= nil and a or b)

-- Output (complex expressions, evaluate once)
(function()
  local __t = a
  if __t ~= nil then return __t else return b end
end)()
```

**Performance:** The optimizer (O2) can eliminate nil checks when type analysis proves the left operand is non-nil:
- `x ?? b` where `x: number` → compiled directly to `x` (check eliminated)
- `config.port ?? 3000` where `port: number | nil` → uses runtime check as shown above

---

### Safe Navigation Operator

**Effort:** 3-4 days

**Description:** Add `?.` operator for safe property/method access that short-circuits to `nil` if the receiver is `nil`.

**Syntax:**

```lua
-- Property access
const street = user?.address?.street?.name

-- Method calls
const upper = text?.toUpper()

-- Indexed access
const first = arr?.[0]

-- Mixed
const result = obj?.method()?.property?.[index]
```

**Implementation:**

1. **Lexer**:
   - Add `QuestionDot` to `TokenKind` for `?.`
   - Add `QuestionLeftBracket` to `TokenKind` for `?.[`

2. **AST**:
   - Add `is_optional: bool` to member access expressions
   - Or add new `OptionalMember`, `OptionalIndex`, `OptionalCall` expression kinds

3. **Parser**:
   - Parse `?.` as optional member access
   - Parse `?.[` as optional index access
   - Parse `?.method()` as optional method call

4. **Type Checker**:
   - If receiver type is `T | nil`, result is `PropertyType | nil`
   - Chain of `?.` accumulates nil possibility

5. **Codegen**:

```lua
-- Input
user?.address?.street

-- Output
(function()
  local __t = user
  if __t == nil then return nil end
  __t = __t.address
  if __t == nil then return nil end
  return __t.street
end)()
```

Optimization: For simple chains, use `and` chaining:
```lua
user and user.address and user.address.street
```

**Performance:** The optimizer (O2) can skip nil checks when type analysis proves the receiver is non-nil:
- `obj?.method()` where `obj: MyClass` → compiled to `obj.method()` (check eliminated)
- `config?.port` where `config: Config | nil` → uses runtime check as shown above
- This optimization is especially valuable in hot loops where the type is known non-nil

---

### Operator Overloading

**Effort:** 1-2 weeks

**Description:** Allow classes to define custom operators via special `operator` methods. Compiles to Lua metamethods.

**Syntax:**

```lua
class Vector {
  x: number
  y: number
  
  constructor(x: number, y: number) {
    self.x = x
    self.y = y
  }
  
  operator +(other: Vector): Vector {
    return Vector.new(self.x + other.x, self.y + other.y)
  }
  
  operator -(other: Vector): Vector {
    return Vector.new(self.x - other.x, self.y - other.y)
  }
  
  operator ==(other: Vector): boolean {
    return self.x == other.x and self.y == other.y
  }
  
  operator #(): number {
    return math.sqrt(self.x ^ 2 + self.y ^ 2)
  }
  
  operator [](index: number): number {
    if index == 0 then return self.x end
    if index == 1 then return self.y end
    error("Index out of bounds")
  }
}

-- Usage
const v1 = Vector.new(1, 2)
const v2 = Vector.new(3, 4)
const v3 = v1 + v2  -- Uses operator +
const len = #v1     -- Uses operator #
```

**Supported Operators:**

| TypedLua Operator | Lua Metamethod |
|-------------------|----------------|
| `operator +` | `__add` |
| `operator -` (binary) | `__sub` |
| `operator -` (unary) | `__unm` |
| `operator *` | `__mul` |
| `operator /` | `__div` |
| `operator %` | `__mod` |
| `operator ^` | `__pow` |
| `operator ==` | `__eq` |
| `operator <` | `__lt` |
| `operator <=` | `__le` |
| `operator ..` | `__concat` |
| `operator #` | `__len` |
| `operator []` (get) | `__index` (function form) |
| `operator []=` (set) | `__newindex` |
| `operator ()` | `__call` |

**Implementation:**

1. **Lexer**:
   - Add `Operator` keyword to `TokenKind`

2. **AST**:
   - Add `OperatorDeclaration` to `ClassMember`
   ```rust
   pub struct OperatorDeclaration {
       pub operator: OperatorKind,
       pub parameters: Vec<Parameter>,
       pub return_type: Type,
       pub body: Block,
       pub span: Span,
   }
   
   pub enum OperatorKind {
       Add, Sub, Mul, Div, Mod, Pow,
       Eq, Lt, Le,
       Concat, Len,
       Index, NewIndex, Call,
       Unm,  // Unary minus
   }
   ```

3. **Parser**:
   - In class body, parse `operator` followed by the operator symbol
   - Parse parameter list and body

4. **Type Checker**:
   - Validate operator signature matches expected types
   - `operator ==` must return `boolean`
   - Binary operators take one parameter (the right operand)
   - Unary operators take no parameters

5. **Codegen**:

```lua
-- Input
class Vector {
  operator +(other: Vector): Vector { ... }
  operator #(): number { ... }
}

-- Output (with performance optimization)
local Vector = {}
Vector.__index = Vector

-- Cached metamethod (single function allocation)
local function vector_add(self, other)
  return Vector.new(self.x + other.x, self.y + other.y)
end

local function vector_len(self)
  return math.sqrt(self.x ^ 2 + self.y ^ 2)
end

-- Assign to metamethods
Vector.__add = vector_add
Vector.__len = vector_len

-- Also store as direct methods for O3 devirtualization
Vector.add = vector_add
Vector.len = vector_len

setmetatable(Vector, {
  __call = function(_, ...) return Vector.new(...) end
})
```

**Performance Note:** Caching operator functions as named locals enables the O3 optimizer to inline them when types are known, and provides direct method access for devirtualization.

---

## Exception Handling

**Total Effort:** 2-3 weeks

**Description:** Kotlin-style exception handling with improvements. Unchecked exceptions, no forced handling, good ergonomics.

### Core Syntax (Required)

```lua
-- throw keyword (sugar for error())
throw IllegalArgumentError.new("Invalid input")

-- try/catch/finally
try
  const data = loadFile(path)
  const parsed = parseJson(data)
  return parsed
catch e: IOError
  log("IO failed: " .. e.message)
  return nil
catch e: ParseError
  log("Parse failed at line " .. e.line)
  return nil
catch e
  log("Unknown: " .. tostring(e))
  rethrow
finally
  cleanup()
end

-- Multi-type catch
catch e: IOError | ParseError
  handleFileError(e)
end

-- Try as expression
const result = try parse(input) catch 0
const data = try loadFile(path) catch nil

-- Rethrow keyword
catch e
  log(e)
  rethrow  -- Rethrows current exception
end

-- Built-in helpers
require(age >= 0, "Age must be non-negative")  -- Throws ArgumentError if false
check(isInitialized, "Not initialized")         -- Throws StateError if false
unreachable("Should never happen")              -- Always throws, return type is never
```

### Optional Syntax (Available, Not Required)

```lua
-- Pattern matching catch
try
  riskyOperation()
catch
  IOError("ENOENT", _) => return defaultFile(),
  IOError(code, msg) => log("IO: " .. code .. " - " .. msg),
  ParseError(line, col, _) => log("Parse error at " .. line .. ":" .. col),
  e => rethrow
end

-- Error chaining operator
const data = readFile(path) !! (e) => ConfigError.wrap("Read failed", e)
const parsed = parseJson(data) !! (e) => ConfigError.wrap("Parse failed", e)

-- Typed throws annotation (documentation only, not enforced)
function parse(s: string): number throws ParseError
function load(path: string): Config throws IOError | ParseError
```

### Implementation

1. **Lexer**:
   - Add `Throw`, `Try`, `Catch`, `Finally`, `Rethrow` to `TokenKind`
   - Add `BangBang` for `!!` operator

2. **AST**:
   ```rust
   pub struct TryStatement {
       pub body: Block,
       pub catches: Vec<CatchClause>,
       pub finally: Option<Block>,
       pub span: Span,
   }
   
   pub struct CatchClause {
       pub pattern: CatchPattern,
       pub body: Block,
       pub span: Span,
   }
   
   pub enum CatchPattern {
       // catch e
       Untyped(Ident),
       // catch e: ErrorType
       Typed(Ident, Type),
       // catch e: TypeA | TypeB
       MultiTyped(Ident, Vec<Type>),
       // catch ErrorType(field1, field2) => ...
       Destructured(Vec<PatternCatchArm>),
   }
   
   pub struct PatternCatchArm {
       pub type_name: Ident,
       pub bindings: Vec<Pattern>,
       pub guard: Option<Expression>,
       pub body: CatchArmBody,
       pub span: Span,
   }
   
   pub struct ThrowStatement {
       pub expression: Expression,
       pub span: Span,
   }
   
   pub struct TryExpression {
       pub expression: Box<Expression>,
       pub catch_value: Box<Expression>,
       pub span: Span,
   }
   
   // For !! operator
   pub struct ErrorChainExpression {
       pub expression: Box<Expression>,
       pub handler: Box<Expression>,  // (e) => NewError
       pub span: Span,
   }
   
   // For throws annotation
   pub struct FunctionDeclaration {
       // ... existing fields
       pub throws: Option<Vec<Type>>,  // NEW
   }
   ```

3. **Parser**:
   - Parse `throw` statement
   - Parse `try`/`catch`/`finally` blocks
   - Parse catch patterns (simple, typed, multi-typed, destructured)
   - Parse `rethrow` statement
   - Parse `try ... catch ...` as expression
   - Parse `!!` operator
   - Parse `throws` clause on functions

4. **Type Checker**:
   - `throw` expression must be a value (any type in Lua)
   - In catch blocks, `e` has the declared type
   - Try expression: result type is union of try result and catch value
   - `rethrow` only valid inside catch block
   - `throws` annotation is informational only (no enforcement)

5. **Codegen**:

**Performance Optimization:** The compiler automatically chooses `pcall` (faster) or `xpcall` (full-featured) based on try-catch structure:

| Scenario | Uses | Reason |
|----------|------|--------|
| Simple `catch e` (no finally) | `pcall` | 30% faster, no type discrimination needed |
| Typed catch (`catch e: ErrorType`) | `xpcall` | Needs type checking in error handler |
| Multiple catch clauses | `xpcall` | Type discrimination required |
| `finally` block present | `xpcall` | Guaranteed cleanup execution |
| Try expression | `pcall` | Always simple form |
| Pattern matching catch | `xpcall` | Complex matching logic |

```lua
-- Example 1: Simple catch (uses pcall - FAST)
try
  doSomething()
catch e
  log(e)
end

-- Output (pcall optimization)
local __ok, __err = pcall(doSomething)
if not __ok then
  log(__err)
end
```

```lua
-- Example 2: Typed catch (uses xpcall - FULL FEATURED)
try
  const result = riskyOperation()
  return result
catch e: IOError
  log(e.message)
  return nil
catch e
  rethrow
finally
  cleanup()
end

-- Output (xpcall for type checking + finally)
local __finally = function()
  cleanup()
end

local __ok, __result = xpcall(function()
  local result = riskyOperation()
  return result
end, function(__err)
  if __instanceof(__err, IOError) then
    log(__err.message)
    return nil
  else
    local e = __err
    error(e)  -- rethrow
  end
end)

__finally()

if not __ok then
  error(__result)
end
return __result
```

```lua
-- Example 3: Try expression (uses pcall - FAST)
const result = try parse(input) catch 0

-- Output (simple pcall)
local __ok, __val = pcall(parse, input)
local result = __ok and __val or 0
```

```lua
-- Example 4: Error chaining (uses pcall)
const data = readFile(path) !! (e) => ConfigError.wrap(e)

-- Output
local data = (function()
  local __ok, __val = pcall(readFile, path)
  if __ok then
    return __val
  else
    error(ConfigError.wrap(__val))
  end
end)()
```

6. **Built-in Error Classes**:

```lua
-- Runtime library provides base error classes
class Error {
  message: string
  file: string | nil
  line: number | nil
  stack: string | nil
  cause: Error | nil
  
  constructor(message: string, cause?: Error) {
    self.message = message
    self.cause = cause
    -- Capture location automatically
    const info = debug.getinfo(2, "Sl")
    self.file = info and info.source
    self.line = info and info.currentline
    self.stack = debug.traceback()
  }
}

class ArgumentError extends Error { }
class StateError extends Error { }
class IOError extends Error { code: string }
class ParseError extends Error { line: number, column: number }
```

---

## Rich Enums (Java-style)

**Effort:** 2-3 weeks

**Description:** Enums with constructors, instance fields, instance methods, and static utility methods. Pure Lua implementation.

**Syntax:**

```lua
enum Planet {
  Mercury(mass: 3.303e23, radius: 2.4397e6),
  Venus(mass: 4.869e24, radius: 6.0518e6),
  Earth(mass: 5.976e24, radius: 6.37814e6),
  Mars(mass: 6.421e23, radius: 3.3972e6),
  
  -- Instance fields (from constructor)
  mass: number
  radius: number
  
  -- Constructor
  constructor(mass: number, radius: number) {
    self.mass = mass
    self.radius = radius
  }
  
  -- Instance methods
  surfaceGravity(): number {
    const G = 6.67430e-11
    return G * self.mass / (self.radius ^ 2)
  }
  
  surfaceWeight(otherMass: number): number {
    return otherMass * self:surfaceGravity()
  }
  
  -- Built-in methods (auto-generated)
  -- name(): string — returns enum constant name
  -- ordinal(): number — returns position (0-indexed)
  
  -- Built-in static methods (auto-generated)
  -- static values(): Planet[] — returns all enum values
  -- static valueOf(name: string): Planet | nil — lookup by name
}

-- Usage
const earth = Planet.Earth
print(earth:name())           -- "Earth"
print(earth:ordinal())        -- 2
print(earth:surfaceGravity()) -- 9.798...

for _, planet in ipairs(Planet.values()) do
  print(planet:name() .. ": " .. planet:surfaceGravity())
end

const mars = Planet.valueOf("Mars")
```

**Implementation:**

1. **AST** — Extend `EnumDeclaration`:
   ```rust
   pub struct EnumDeclaration {
       pub name: Ident,
       pub members: Vec<EnumMember>,
       pub fields: Vec<EnumField>,           // NEW
       pub constructor: Option<Constructor>, // NEW
       pub methods: Vec<MethodDeclaration>,  // NEW
       pub span: Span,
   }
   
   pub struct EnumMember {
       pub name: Ident,
       pub arguments: Vec<Expression>,  // NEW: constructor args
       pub span: Span,
   }
   
   pub struct EnumField {
       pub name: Ident,
       pub type_annotation: Type,
       pub span: Span,
   }
   ```

2. **Parser**:
   - Parse enum members with optional `(arg1, arg2, ...)` syntax
   - Parse field declarations inside enum
   - Parse constructor inside enum
   - Parse methods inside enum

3. **Type Checker**:
   - Validate constructor parameters match field declarations
   - Validate enum member arguments match constructor signature
   - Type check methods with `self` bound to enum type
   - Auto-generate `name()`, `ordinal()`, `values()`, `valueOf()` signatures

4. **Codegen**:

```lua
-- Input
enum Planet {
  Mercury(mass: 3.303e23, radius: 2.4397e6),
  Earth(mass: 5.976e24, radius: 6.37814e6),
  
  mass: number
  radius: number
  
  constructor(mass: number, radius: number) {
    self.mass = mass
    self.radius = radius
  }
  
  surfaceGravity(): number {
    local G = 6.67430e-11
    return G * self.mass / (self.radius ^ 2)
  }
}

-- Output
local Planet = {}
Planet.__index = Planet

local function Planet__new(name, ordinal, mass, radius)
  local self = setmetatable({}, Planet)
  self.__name = name
  self.__ordinal = ordinal
  self.mass = mass
  self.radius = radius
  return self
end

function Planet:name()
  return self.__name
end

function Planet:ordinal()
  return self.__ordinal
end

-- Small methods are inlinable by optimizer (O3)
function Planet:surfaceGravity()
  local G = 6.67430e-11
  return G * self.mass / (self.radius ^ 2)
end

-- Enum instances (constants known at compile time)
Planet.Mercury = Planet__new("Mercury", 0, 3.303e23, 2.4397e6)
Planet.Earth = Planet__new("Earth", 1, 5.976e24, 6.37814e6)

-- Static methods
Planet.__values = { Planet.Mercury, Planet.Earth }

-- Hash lookup table for O(1) valueOf()
Planet.__byName = {
  Mercury = Planet.Mercury,
  Earth = Planet.Earth
}

function Planet.values()
  return Planet.__values
end

-- O(1) hash lookup instead of O(n) iteration
function Planet.valueOf(name)
  return Planet.__byName[name]
end

-- Prevent instantiation
setmetatable(Planet, {
  __call = function()
    error("Cannot instantiate enum Planet directly")
  end
})

return Planet
```

---

## Interfaces with Default Implementations

**Effort:** 1-2 weeks

**Description:** C#-style interfaces where methods can have default implementations. Classes implementing the interface get default methods unless they override them.

**Syntax:**

```lua
interface Printable {
  -- Abstract method (must implement)
  toString(): string
  
  -- Default implementation (optional to override)
  print(): void {
    io.write(self:toString())
    io.write("\n")
  }
  
  printN(n: number): void {
    for i = 1, n do
      self:print()
    end
  }
}

interface Serializable {
  serialize(): string
  
  -- Default using JSON
  toJson(): string {
    return '{"data":"' .. self:serialize() .. '"}'
  }
}

class User implements Printable, Serializable {
  name: string
  age: number
  
  constructor(name: string, age: number) {
    self.name = name
    self.age = age
  }
  
  -- Must implement abstract methods
  toString(): string {
    return "User(" .. self.name .. ", " .. self.age .. ")"
  }
  
  serialize(): string {
    return self.name .. ":" .. self.age
  }
  
  -- Gets print(), printN(), toJson() for free
  -- Can override if needed
}

const user = User.new("Alice", 30)
user:print()     -- Uses default from Printable
user:printN(3)   -- Uses default from Printable
print(user:toJson())  -- Uses default from Serializable
```

**Implementation:**

1. **AST** — Extend `InterfaceMember`:
   ```rust
   pub enum InterfaceMember {
       Property(PropertySignature),
       Method(MethodSignature),
       Index(IndexSignature),
       DefaultMethod(MethodDeclaration),  // NEW: method with body
   }
   ```

2. **Parser**:
   - When parsing interface methods, check for `{` after signature
   - If present, parse as `DefaultMethod` with body
   - If absent, parse as abstract `Method`

3. **Type Checker**:
   - Track which methods are abstract vs default
   - When class implements interface:
     - Error if abstract method not implemented
     - OK if default method not implemented (uses default)
     - OK if default method is overridden
   - `self` in default methods typed as implementing class

4. **Codegen**:

```lua
-- Input
interface Printable {
  toString(): string
  print(): void {
    io.write(self:toString() .. "\n")
  }
}

class User implements Printable {
  name: string
  toString(): string { return self.name }
}

-- Output
local Printable = {}

function Printable:print()
  io.write(self:toString() .. "\n")
end

local User = {}
User.__index = User

function User.new(name)
  local self = setmetatable({}, User)
  self.name = name
  return self
end

function User:toString()
  return self.name
end

-- Copy default implementations if not overridden
User.print = User.print or Printable.print

return User
```

**Performance:** For pure default methods (no side effects, deterministic output), the optimizer can add memoization hints:
- Methods marked as `@pure` in decorators can be cached per instance
- Example: `toJson()` in Serializable could cache its result if object is immutable
- The optimizer (O3) can inline small default methods when type is known at compile time

---

## File-Based Namespaces

**Effort:** 1-2 weeks

**Description:** C#-style file-scoped namespaces for organizing code with dot notation. Complements the existing module system - users can choose modules OR namespaces based on their needs.

### Syntax

**File-scoped namespace (entire file in one namespace):**

```lua
-- File: math/vector.tl
namespace Math.Vector;  -- Semicolon = file-scoped

export function dot(a: Vec2, b: Vec2): number {
  return a.x * b.x + a.y * b.y
}

export function cross(a: Vec2, b: Vec2): number {
  return a.x * b.y - a.y * b.x
}

export function length(v: Vec2): number {
  return math.sqrt(v.x ^ 2 + v.y ^ 2)
}
```

**Usage:**

```lua
import { Math } from "./math/vector"

const v1: Math.Vector = { x = 1, y = 2 }
const v2: Math.Vector = { x = 3, y = 4 }
const result = Math.Vector.dot(v1, v2)
```

### Key Features

1. **File-scoped only** - One `namespace X;` declaration per file (no nested blocks)
2. **Must be first statement** - Before imports or any code
3. **Optional** - Files can use modules instead (no namespace declaration needed)
4. **Path matching** - Configurable enforcement of namespace matching file path

### Import Patterns

```lua
-- Full namespace path
import { Math } from "./math/vector"
Math.Vector.dot(v1, v2)

-- Nested namespace directly (aliasing)
import { Vector } from "./math/vector" as Math.Vector
Vector.dot(v1, v2)

-- Specific exports
import { dot, cross } from "./math/vector"
dot(v1, v2)
```

### Primary Use Case: Declaration Files

Namespaces excel at organizing large type declaration files:

```lua
-- godot.d.tl (single file for entire engine API)
namespace Godot.Scene;

export interface Node {
  name: string
  add_child(child: Node): void
  remove_child(child: Node): void
}

export interface Sprite extends Node {
  texture: Texture
  centered: boolean
}

export interface Camera2D extends Node {
  zoom: Vector2
  offset: Vector2
}
```

```lua
-- godot-physics.d.tl
namespace Godot.Physics;

export interface RigidBody2D {
  mass: number
  apply_impulse(impulse: Vector2): void
}

export interface CollisionShape2D {
  shape: Shape2D
  disabled: boolean
}
```

**Usage:**

```lua
import { Godot } from "./godot"

class Player extends Godot.Scene.Sprite {
  override _ready(): void {
    print("Player ready!")
  }
}
```

### Implementation

1. **Lexer**:
   - Add `Namespace` to `TokenKind`
   - Add to `from_keyword()` match

2. **AST** (`crates/typedlua-core/src/ast/statement.rs`):
   ```rust
   pub enum Statement {
       // ... existing variants
       NamespaceDeclaration {
           path: Vec<String>,  // ["Math", "Vector"]
           span: Span,
       }
   }
   ```

3. **Parser**:
   - Parse `namespace Math.Vector;` at file start
   - Error if namespace appears after other statements
   - Only allow semicolon syntax (no block `{}` syntax)
   - Store namespace path in module metadata

4. **Type Checker**:
   - Track namespace for each module
   - When resolving imports, include namespace prefix
   - If `enforceNamespacePath: true`, verify namespace matches file path:
     - `math/vector.tl` with `namespace Math.Vector;` → OK
     - `math/vector.tl` with `namespace Foo.Bar;` → ERROR
   - Namespace types are accessible via dot notation

5. **Codegen**:

```lua
-- Input: math/vector.tl
namespace Math.Vector;

export function dot(a: Vec2, b: Vec2): number {
  return a.x * b.x + a.y * b.y
}

-- Output: math/vector.lua
local Math = {}
Math.Vector = {}

function Math.Vector.dot(a, b)
  return a.x * b.x + a.y * b.y
end

return Math
```

### Modules vs Namespaces

| Feature | Modules | Namespaces |
|---------|---------|------------|
| **Syntax** | `import * as Vec from "./vector"` | `namespace Math.Vector;` |
| **Organization** | File/folder structure | Logical dot notation |
| **Import** | `Vec.dot()` | `Math.Vector.dot()` |
| **Best For** | Code organization, lazy loading | Type declarations, logical grouping |
| **Multiple per file** | No | No (file-scoped only) |
| **Path enforcement** | Implicit (file system) | Configurable |

### Config Option

```yaml
compilerOptions:
  enforceNamespacePath: true  # Default: false
```

**When `enforceNamespacePath: true`:**
- `math/vector.tl` MUST declare `namespace Math.Vector;`
- `utils/string/helpers.tl` MUST declare `namespace Utils.String.Helpers;`
- Mismatch is a compile error

**When `enforceNamespacePath: false`:**
- `math/vector.tl` can declare any namespace (e.g., `namespace Engine.Math.Vector;`)
- Provides flexibility for organizing code differently than file structure

---

## Template Literal Enhancements

**Effort:** 3-5 days

**Description:** Extend existing template literal support with multi-line auto-dedenting for cleaner embedded strings (SQL, HTML, JSON, etc.).

### Multi-Line Auto-Dedenting

**Problem:** Multi-line strings with indentation look messy:

```lua
function generateSQL(table: string, id: number): string {
  -- Indentation included in string (ugly)
  return `
    SELECT *
    FROM ${table}
    WHERE id = ${id}
  `
  -- Result: "\n    SELECT *\n    FROM users\n    WHERE id = 42\n  "
}
```

**Solution:** Auto-dedent to minimum common indentation:

```lua
function generateSQL(table: string, id: number): string {
  return `
    SELECT *
    FROM ${table}
    WHERE id = ${id}
  `
  -- Result: "SELECT *\nFROM users\nWHERE id = 42"
  -- Leading/trailing newlines trimmed, indentation removed
}
```

### Dedenting Rules

1. **Find minimum indentation** - Count leading spaces/tabs on non-empty lines
2. **Remove common indent** - Strip that amount from all lines
3. **Trim first/last lines** - Remove leading/trailing blank lines if they're only whitespace
4. **Preserve relative indentation** - Lines with extra indent keep it

**Example:**

```lua
const html = `
  <div>
    <h1>${title}</h1>
    <p>
      ${content}
    </p>
  </div>
`

-- After dedenting:
-- "<div>\n  <h1>Hello</h1>\n  <p>\n    World\n  </p>\n</div>"
-- Common 2-space indent removed, relative indentation preserved
```

### Edge Cases

**Tabs vs Spaces:**
```lua
-- Mixed tabs/spaces - treat tab = 1 indent unit
const query = `
	SELECT *
    FROM users  -- Mix of tab and spaces
`
-- ERROR: Inconsistent indentation (tab vs spaces)
```

**First line with content:**
```lua
-- First line on same line as backtick
const msg = `Hello
  World`

-- Result: "Hello\n  World" (no dedenting, first line has no indent)
```

**Explicit newlines:**
```lua
const text = `
  Line 1\n\n  Line 2
`
-- Result: "Line 1\n\nLine 2" (explicit \n preserved, dedenting applied)
```

### Implementation

1. **Lexer** (`crates/typedlua-core/src/lexer/mod.rs`):
   - When parsing template literal, track indentation of each line
   - Store raw string with indentation metadata

2. **Parser**:
   - Template literals already parsed into `TemplateLiteral` AST node
   - No changes needed

3. **Codegen** (`crates/typedlua-core/src/codegen/mod.rs`):
   - During codegen, apply dedenting algorithm:
     1. Split string into lines
     2. Find minimum leading whitespace (excluding empty lines)
     3. Remove that amount from all lines
     4. Trim first/last lines if blank
     5. Join back with `\n`

**Dedenting Algorithm:**

```rust
fn dedent_template_literal(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();

    // Find first and last non-empty lines
    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
    let end = lines.iter().rposition(|l| !l.trim().is_empty()).unwrap_or(lines.len() - 1);

    let content_lines = &lines[start..=end];

    // Find minimum indentation
    let min_indent = content_lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    // Remove common indentation
    content_lines
        .iter()
        .map(|l| {
            if l.len() > min_indent {
                &l[min_indent..]
            } else {
                *l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

### Codegen Example

```lua
-- Input
const query = `
  SELECT *
  FROM users
  WHERE id = ${userId}
`

-- Output (string concatenation with dedenting applied)
local query = "SELECT *\nFROM users\nWHERE id = " .. tostring(userId)
```

### When Dedenting Applies

**Always applied** - No opt-out, this is standard behavior for multi-line template literals

**Single-line templates** - No dedenting (no newlines)
```lua
const msg = `Hello ${name}`  -- No dedenting needed
```

**Escaped backticks** - Not affected
```lua
const code = `const str = \`nested\``  -- Inner backtick escaped
```

### Config

No configuration needed - dedenting is automatic for all multi-line template literals.

---

## Reflection System

**Effort:** 2-3 weeks

**Description:** Full runtime reflection with performance optimizations. Pure Lua implementation with lazy metadata generation and type interning.

### API

```lua
-- Get type info for an object
const typeInfo = typeof(user)

-- Type info structure
typeInfo.name          -- "User"
typeInfo:getFields()   -- Array of field info
typeInfo:getField("name")  -- Single field info
typeInfo:getMethods()  -- Array of method info
typeInfo:getMethod("greet")  -- Single method info
typeInfo:getInterfaces()  -- Array of implemented interfaces
typeInfo:getParent()   -- Parent class type info or nil

-- Field info structure
fieldInfo.name         -- "name" (Tier 1)
fieldInfo.isOptional   -- boolean (Tier 1)
fieldInfo.isReadonly   -- boolean (Tier 1)
fieldInfo:getType()    -- Type info (Tier 2, lazy)

-- Method info structure
methodInfo.name        -- "greet" (Tier 1)
methodInfo.isStatic    -- boolean (Tier 1)
methodInfo.isAbstract  -- boolean (Tier 1)
methodInfo:getSignature()  -- { parameters, returnType } (Tier 2, lazy)

-- Instance checks
isInstance(obj, User)  -- true if obj is a User
isInstance(obj, "User")  -- Also works with string name

-- Get all fields as key-value pairs
const fields = getFields(obj)
for name, value in pairs(fields) do
  print(name .. " = " .. tostring(value))
end
```

### Performance Strategies

**Rust Native Implementation:**

The reflection system is implemented entirely in Rust for maximum performance:

1. **Lazy Metadata Generation**: Type info is built on first access, then cached.

2. **Type Interning**: Primitive types and common type references are shared globally.

3. **Auto-Adaptive Depth**: Automatically upgrades from minimal to full metadata based on actual access patterns. No configuration needed - the system starts minimal and builds more detail only when accessed.

4. **Ancestor Bitmasks**: Each type stores a bitset of all ancestor types for O(1) `isInstance()` checks (no chain walking).

5. **Compact Binary Metadata**: Field modifiers stored as bitflags, using native memory instead of Lua tables.

6. **Hash-Based Lookups**: O(1) field/method lookup using Rust HashMap.

7. **Zero-Copy Strings**: String interning via Rust's static lifetime strings.

**Distribution:**

- Pre-compiled binary rocks for common platforms (Linux x64/ARM, macOS x64/ARM, Windows x64)
- Distributed via LuaRocks and GitHub releases
- Users install via: `luarocks install typedlua-reflect`

**Expected Performance:**
- **50-500x faster** than naive Lua reflection implementation
- `isInstance()`: ~5ns (vs ~200ns in pure Lua)
- `getField()`: ~10ns (vs ~100ns in pure Lua)
- Deep type resolution: ~200ns (vs ~5µs in pure Lua)

### Implementation

#### **Rust Native Module** (`crates/typedlua-reflect-native/src/lib.rs`):

Core reflection implementation using `mlua` for Lua integration:

```rust
use mlua::prelude::*;
use std::collections::HashMap;

// Global type registry (compile-time generated)
static TYPE_REGISTRY: &[TypeInfo] = &[
    TypeInfo { name: "string", ancestor_mask: 0b0001 },
    TypeInfo { name: "number", ancestor_mask: 0b0010 },
    // ... generated for all types
];

struct TypeInfo {
    name: &'static str,
    ancestor_mask: u64,
}

#[mlua::lua_module]
fn typedlua_reflect(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;

    exports.set("is_instance", lua.create_function(is_instance)?)?;
    exports.set("typeof", lua.create_function(typeof_impl)?)?;

    Ok(exports)
}

fn is_instance(_lua: &Lua, (obj_type_id, target_type_id): (u32, u32)) -> LuaResult<bool> {
    let obj_info = &TYPE_REGISTRY[obj_type_id as usize];
    Ok((obj_info.ancestor_mask & (1 << target_type_id)) != 0)
}

fn typeof_impl(_lua: &Lua, type_id: u32) -> LuaResult<LuaTable> {
    // Return cached type info
    // ... implementation
    unimplemented!()
}
```

**Distribution:**

```lua
-- typedlua-reflect-scm-1.rockspec
package = "typedlua-reflect"
version = "scm-1"

source = {
   url = "git://github.com/yourorg/typedlua",
}

dependencies = { "lua >= 5.1" }

build = {
   type = "command",
   build_command = "cargo build --release",
   install = {
      lib = {
         ["typedlua.reflect.native"] = "target/release/libtypedlua_reflect.*"
      }
   }
}
```

**Binary Distribution:**
- Pre-compiled `.rock` files for Linux (x64, ARM), macOS (x64, ARM), Windows (x64)
- Available via LuaRocks: `luarocks install typedlua-reflect`
- GitHub releases: Manual download + install

#### **Runtime Integration:**

The Lua runtime loads the native module directly:

```lua
-- typedlua_runtime.lua
local native = require("typedlua.reflect.native")

local Runtime = {}

function Runtime.isInstance(obj, typeRef)
  if obj == nil then return false end
  return native.is_instance(
    getmetatable(obj).__typeId,
    typeRef.__typeId
  )
end

function Runtime.typeof(obj)
  if obj == nil then return nil end
  return native.typeof(getmetatable(obj).__typeId)
end

function Runtime.getFields(obj)
  local typeId = getmetatable(obj).__typeId
  return native.get_fields(typeId)
end

return Runtime
```

#### **Codegen** — Generate type metadata for each class:

```lua
-- Input
class User {
  readonly name: string
  age: number
  email?: string

  constructor(name: string, age: number) { ... }

  greet(): string { ... }
  static create(name: string): User { ... }
}

-- Output (with optimized reflection metadata)
local User = {}
User.__index = User

-- Compile-time assigned type ID and ancestor mask
User.__typeId = 10
User.__ancestorMask = 0x400  -- Bit 10 set (just User, no parent)

-- Tier 0: Type info with pre-computed bitmask
User.__type = {
  name = __S.User,
  typeId = 10,
  ancestorMask = 0x400,

  -- Tier 1: Build basic field info on first getFields() call
  _buildFields = function()
    local READONLY = 0x01
    local OPTIONAL = 0x02

    return {
      {
        name = __S.name,           -- String interning
        flags = READONLY,          -- Bitflags instead of booleans
        _resolveType = function() return __typeRegistry.string end
      },
      {
        name = __S.age,
        flags = 0x00,
        _resolveType = function() return __typeRegistry.number end
      },
      {
        name = __S.email,
        flags = OPTIONAL,
        _resolveType = function() return __typeRegistry.string end
      },
    }
  end,

  -- Tier 1: Build method info on first getMethods() call
  _buildMethods = function()
    return {
      {
        name = __S.greet,
        isStatic = false,
        -- Tier 2: Resolve signature on first access
        _resolveSignature = function()
          return {
            parameters = {},
            returnType = __typeRegistry.string
          }
        end
      },
      {
        name = __S.create,
        isStatic = true,
        _resolveSignature = function()
          return {
            parameters = {{ name = __S.name, type = __typeRegistry.string }},
            returnType = User.__type
          }
        end
      },
    }
  end,
}

function User.new(name, age) ... end
function User:greet() ... end
function User.create(name) ... end
```

### Performance Characteristics

**Rust Native Module:**

| Operation | Cost | Notes |
|-----------|------|-------|
| `isInstance(obj, User)` | ~5ns | O(1) bitset check in native code |
| `typeof(obj).name` | ~8ns | Direct field access from Rust |
| `getField("name")` | ~10ns | O(1) HashMap lookup in Rust |
| `getFields()` first call | ~100ns | Build array from native metadata |
| `getFields()` cached | ~5ns | Return cached Lua table |
| Deep type resolution | ~200ns | Native traversal + lazy caching |
| Memory overhead per type | ~60 bytes | Compact binary representation |

**Comparison to Pure Lua:**

| Operation | Rust Native | Pure Lua (Optimized) | Speedup |
|-----------|-------------|----------------------|---------|
| `isInstance()` | ~5ns | ~20ns | **4x** |
| `getField()` | ~10ns | ~100ns | **10x** |
| `getFields()` first call | ~100ns | ~500ns | **5x** |
| Deep type resolution | ~200ns | ~5µs | **25x** |

### Auto-Adaptive Tier Progression

The system automatically builds metadata based on what's accessed:

| User Code | Data Built | Memory Cost |
|-----------|------------|-------------|
| `typeof(obj).name` | Just name + typeId | **~30 bytes** (native) |
| `typeof(obj):getFields()` | Field metadata structures | **~80 bytes** (compact bitflags) |
| `field:getType()` | Full type tree | **~150 bytes** (interned strings) |
| `method:getSignature()` | Method signatures | **~100 bytes** (native structs) |
| Never use reflection | Minimal (typeId + mask) | **~20 bytes** (native only) |

**Key optimizations:**
- Rust HashMap for O(1) field/method lookup
- Compact binary metadata (bitflags, static strings)
- Ancestor bitmasks for O(1) inheritance checks
- Lazy metadata generation with native caching
- No configuration needed - system pays only for what you use

---

## Compiler Optimizations

**Effort:** 5-7 weeks

**Description:** Compile-time optimizations that generate faster Lua code. Since TypedLua targets standard Lua VMs (5.1-5.4) without JIT compilation, these source-to-source transformations are critical for achieving good performance while maintaining compatibility with all Lua environments and game engines.

### Optimization Levels

```yaml
compilerOptions:
  optimization: 2  # 0=none, 1=basic, 2=standard, 3=aggressive
```

| Level | Optimizations |
|-------|---------------|
| O0 | None (fast compile, debug) |
| O1 | Constant folding, dead code elimination |
| O2 | O1 + function inlining, loop optimization |
| O3 | O2 + devirtualization, generic specialization |

### Optimization Passes

#### 1. Constant Folding (O1)

Evaluate constant expressions at compile time.

```lua
-- Input
const TAX = 0.08
const PRICE = 100
const total = PRICE * (1 + TAX)

-- Output
local total = 108.0
```

#### 2. Dead Code Elimination (O1)

Remove unreachable code.

```lua
-- Input
function process(x: number): number
  if false then
    expensiveSetup()
  end
  return x * 2
end

-- Output
function process(x)
  return x * 2
end
```

#### 3. Function Inlining (O2)

Replace small function calls with function body.

```lua
-- Input
function square(x: number): number
  return x * x
end

function magnitude(v: Vector): number
  return math.sqrt(square(v.x) + square(v.y))
end

-- Output
function magnitude(v)
  return math.sqrt(v.x * v.x + v.y * v.y)
end
```

**Inlining heuristics:**
- Function body ≤ 5 statements
- No recursion
- No escaping closures
- Called in hot path (loop) = more likely to inline

#### 4. Loop Optimization (O2)

Transform slow loop patterns to fast ones. Iterator-based loops create closures and have function call overhead; numeric for loops are 3-5x faster in standard Lua VMs.

```lua
-- Input
for _, v in ipairs(arr) do
  total = total + v
end

-- Output (numeric for loop, no closure overhead)
local __n = #arr
for __i = 1, __n do
  total = total + arr[__i]
end
```

#### 5. Devirtualization (O3)

When type is known, use direct calls instead of metatable lookups.

```lua
-- Input
function process(v: Vector): number
  return v:magnitude()
end

-- Output (type is known)
function process(v)
  return Vector.magnitude(v)
end
```

#### 6. Generic Specialization (O3)

Generate type-specific versions of generic functions.

```lua
-- Input
function map<T, U>(arr: T[], fn: (T) -> U): U[]

const result = map(numbers, (n) => n * 2)

-- Output
function __map_number_number(arr, fn)
  -- Specialized implementation
end

local result = __map_number_number(numbers, function(n) return n * 2 end)
```

#### 7. Null Coalescing Optimization (O2)

Optimize `??` operator based on operand complexity.

```lua
-- Input (simple identifier)
const port = config.port ?? 3000

-- Output (inline, no IIFE)
local port = config.port ~= nil and config.port or 3000

-- Input (complex expression)
const result = getUserProfile(id).data ?? defaultData

-- Output (IIFE to avoid double evaluation)
local result = (function()
  local __t = getUserProfile(id).data
  return __t ~= nil and __t or defaultData
end)()
```

#### 8. Safe Navigation Optimization (O2)

Optimize `?.` chains based on length and complexity.

```lua
-- Input (short chain)
const name = user?.name

-- Output (simple check)
local name = user and user.name or nil

-- Input (long chain, 3+ levels)
const street = user?.address?.street?.name

-- Output (early exit pattern)
local street
if user ~= nil then
  local __t1 = user.address
  if __t1 ~= nil then
    local __t2 = __t1.street
    if __t2 ~= nil then
      street = __t2.name
    end
  end
end
```

#### 9. Exception Handling Optimization (O2)

Inline simple try-catch blocks to reduce pcall overhead.

```lua
-- Input (try expression)
const result = try parse(input) catch 0

-- Output (inline pcall)
local __ok, __val = pcall(parse, input)
local result = __ok and __val or 0

-- Input (simple try-catch with single catch)
try
  doSomething()
catch e
  log(e)
end

-- Output (no finally, inline)
local __ok, __err = pcall(doSomething)
if not __ok then
  log(__err)
end
```

#### 10. Operator Overloading Inlining (O3)

Inline simple operator methods when type is known.

```lua
-- Input
class Vector {
  operator +(other: Vector): Vector {
    return Vector.new(self.x + other.x, self.y + other.y)
  }
}

function add(v1: Vector, v2: Vector): Vector {
  return v1 + v2
}

-- Output (type known, inline the operator)
function add(v1, v2)
  -- Devirtualized and inlined
  return Vector.new(v1.x + v2.x, v1.y + v2.y)
end
```

#### 11. Rich Enum Optimization (O2)

Pre-compute enum instances at compile time instead of runtime initialization.

```lua
-- Input
enum Planet {
  Mercury(mass: 3.303e23, radius: 2.4397e6),
  Earth(mass: 5.976e24, radius: 6.37814e6),
}

-- Output (pre-initialized, no runtime overhead)
local Planet = {}
Planet.__index = Planet

-- Pre-computed instances (compile-time constant folding)
Planet.Mercury = setmetatable({
  __name = "Mercury",
  __ordinal = 0,
  mass = 3.303e23,
  radius = 2.4397e6
}, Planet)

Planet.Earth = setmetatable({
  __name = "Earth",
  __ordinal = 1,
  mass = 5.976e24,
  radius = 6.37814e6
}, Planet)

-- Static arrays pre-built
Planet.__values = { Planet.Mercury, Planet.Earth }
```

#### 12. Interface Default Method Inlining (O3)

Inline default interface methods when implementing class is known.

```lua
-- Input
interface Printable {
  toString(): string
  print(): void {
    io.write(self:toString())
  }
}

class User implements Printable {
  toString(): string { return self.name }
}

function printUser(u: User): void {
  u:print()  -- Uses default implementation
}

-- Output (inline default method since type is User)
function printUser(u)
  io.write(u:toString())  -- Inlined, no extra function call
end
```

#### 13. Table Pre-allocation (O1)

Pre-size tables when size is known at compile time.

```lua
-- Input (array literal)
const arr = [1, 2, 3, 4, 5]

-- Output (pre-allocate with table.create if available)
local arr = table.create and table.create(5, 0) or {}
arr[1] = 1
arr[2] = 2
arr[3] = 3
arr[4] = 4
arr[5] = 5

-- Input (object literal with known field count)
const obj = { name: "Alice", age: 30, city: "NYC" }

-- Output (hint to Lua VM about table size)
local obj = {}
obj.name = "Alice"
obj.age = 30
obj.city = "NYC"

-- Input (loop with known bounds)
const result: number[] = []
for i = 1, 1000 do
  result[i] = i * 2
end

-- Output (pre-allocate)
local result = table.create and table.create(1000, 0) or {}
for i = 1, 1000 do
  result[i] = i * 2
end
```

#### 14. String Concatenation Optimization (O2)

Use `table.concat` for template literals and multi-part strings (3+ parts).

```lua
-- Input (template literal)
const msg = `Hello ${name}, you are ${age} years old`

-- Without optimization (naive)
local msg = "Hello " .. name .. ", you are " .. tostring(age) .. " years old"

-- With optimization (5-10x faster for 3+ parts)
local msg = table.concat({"Hello ", name, ", you are ", tostring(age), " years old"})

-- Input (loop concatenation)
let result = ""
for i = 1, #items do
  result = result .. items[i]
end

-- Output (accumulate in table, much faster)
local __parts = {}
for i = 1, #items do
  __parts[i] = items[i]
end
local result = table.concat(__parts)
```

#### 15. Constant Global Localization (O1)

Cache frequently-used globals (math.*, string.*, table.*) as locals in function scope.

```lua
-- Input (repeated global access in loop)
function distance(points: Point[]): number {
  let total = 0
  for i = 1, #points do
    total = total + math.sqrt(points[i].x ^ 2 + points[i].y ^ 2)
  end
  return total
}

-- Output (localize globals - 5-20% speedup)
local math_sqrt = math.sqrt

function distance(points)
  local total = 0
  for i = 1, #points do
    total = total + math_sqrt(points[i].x ^ 2 + points[i].y ^ 2)
  end
  return total
end

-- Auto-detect globals used 3+ times in a function:
-- math.sqrt, math.abs, math.floor, math.ceil
-- string.sub, string.find, string.gsub
-- table.insert, table.remove, table.concat
```

#### 16. Algebraic Simplification (O1)

Simplify mathematical identities and strength reduction.

```lua
-- Input (identity operations)
const a = x * 1
const b = y + 0
const c = z - 0
const d = w / 1

-- Output (eliminate identities)
local a = x
local b = y
local c = z
local d = w

-- Input (strength reduction)
const double = n * 2
const quad = n * 4
const half = n / 2

-- Output (cheaper operations)
local double = n + n         -- Addition faster than multiplication
local quad = n + n + n + n   -- Or n << 2 if bit ops available
local half = n * 0.5         -- Multiplication faster than division

-- Input (boolean algebra)
const a = x and true
const b = y or false
const c = not not z

-- Output
local a = x
local b = y
local c = not not z  -- Keep double negation (!!z pattern)
```

#### 17. Dead Store Elimination (O2)

Remove assignments to variables that are never read.

```lua
-- Input
function compute(x: number): number {
  let temp = x * 2      -- Dead store (never read)
  temp = x * 3          -- Overwrites previous value
  let unused = 42       -- Dead store (never used)
  return temp
}

-- Output
function compute(x)
  local temp = x * 3
  return temp
end

-- Further optimize to:
function compute(x)
  return x * 3
end
```

#### 18. Method Call to Function Call (O2)

Convert method syntax (`:`) to function calls (`.`) when type is known, avoiding metatable lookup.

```lua
-- Input
class Counter {
  count: number

  increment(): void {
    self.count = self.count + 1
  }
}

function incrementMany(c: Counter, n: number): void {
  for i = 1, n do
    c:increment()  -- Method call (metatable __index lookup)
  end
}

-- Output (type known, devirtualize)
function incrementMany(c, n)
  for i = 1, n do
    Counter.increment(c)  -- Direct function call, no metatable
  end
end
```

#### 19. Tail Call Optimization (O2)

Convert tail-recursive functions to loops to avoid stack overflow.

```lua
-- Input (tail recursion)
function factorial(n: number, acc: number = 1): number {
  if n <= 1 then
    return acc
  end
  return factorial(n - 1, n * acc)  -- Tail call
}

-- Output (convert to loop)
function factorial(n, acc)
  acc = acc or 1
  while true do
    if n <= 1 then
      return acc
    end
    n, acc = n - 1, n * acc  -- Update and loop instead of recurse
  end
end

-- Also works for mutual recursion
function isEven(n: number): boolean {
  if n == 0 then return true end
  return isOdd(n - 1)
}

function isOdd(n: number): boolean {
  if n == 0 then return false end
  return isEven(n - 1)
}

-- Output (trampoline pattern if mutual recursion detected)
-- Or inline if simple case
```

### Implementation Architecture

```rust
pub struct Optimizer {
    passes: Vec<Box<dyn OptimizationPass>>,
}

impl Optimizer {
    pub fn new(level: u8) -> Self {
        let mut passes: Vec<Box<dyn OptimizationPass>> = vec![];

        if level >= 1 {
            // O1: Basic optimizations
            passes.push(Box::new(ConstantFolder::new()));
            passes.push(Box::new(DeadCodeEliminator::new()));
            passes.push(Box::new(TablePreAllocator::new()));
            passes.push(Box::new(GlobalLocalizer::new()));
            passes.push(Box::new(AlgebraicSimplifier::new()));
        }

        if level >= 2 {
            // O2: Standard optimizations
            passes.push(Box::new(Inliner::new(threshold: 5)));
            passes.push(Box::new(LoopOptimizer::new()));
            passes.push(Box::new(NullCoalescingOptimizer::new()));
            passes.push(Box::new(SafeNavigationOptimizer::new()));
            passes.push(Box::new(ExceptionOptimizer::new()));
            passes.push(Box::new(StringConcatOptimizer::new()));
            passes.push(Box::new(DeadStoreEliminator::new()));
            passes.push(Box::new(MethodToFunctionConverter::new()));
            passes.push(Box::new(TailCallOptimizer::new()));
            passes.push(Box::new(RichEnumOptimizer::new()));
        }

        if level >= 3 {
            // O3: Aggressive optimizations
            passes.push(Box::new(Devirtualizer::new()));
            passes.push(Box::new(GenericSpecializer::new()));
            passes.push(Box::new(OperatorInliner::new()));
            passes.push(Box::new(InterfaceMethodInliner::new()));
            passes.push(Box::new(Inliner::new(threshold: 15)));
        }

        Self { passes }
    }

    pub fn optimize(&self, ast: &mut Program, type_info: &TypeInfo) {
        for pass in &self.passes {
            pass.run(ast, type_info);
        }
    }
}

pub trait OptimizationPass {
    fn run(&self, ast: &mut Program, type_info: &TypeInfo);
}
```

### Summary

**Total Optimizations: 19**

| Level | Count | Optimizations |
|-------|-------|---------------|
| **O1** | 5 | Constant folding, dead code elimination, table pre-allocation, global localization, algebraic simplification |
| **O2** | 9 | Function inlining, loop optimization, null coalescing, safe navigation, exception handling, string concatenation, dead store elimination, method devirtualization, tail call optimization, enum optimization |
| **O3** | 5 | Advanced devirtualization, generic specialization, operator inlining, interface method inlining, aggressive inlining |

**Expected Performance:**
- **O0 (Debug):** Baseline (fast compile, readable output)
- **O1 (Basic):** ~1.5-2x faster than naive codegen
- **O2 (Standard):** ~2-5x faster than naive codegen
- **O3 (Aggressive):** ~3-7x faster than naive codegen (hot code paths)

**Key Benefits:**
- No JIT required - all optimizations happen at compile time
- Works with all Lua VMs (5.1-5.4)
- Compatible with game engines (Löve2D, Defold, Roblox, etc.)
- Generates clean, debuggable Lua code
- Leverages TypedLua's type information for safe optimizations

---

## Implementation Priority

| Priority | Feature | Effort | Dependencies |
|----------|---------|--------|--------------|
| 1 | Override keyword | 2-4 hours | None |
| 2 | Final keyword | 1-2 days | None |
| 3 | Primary constructors | 1 week | Class system |
| 4 | Null coalescing `??` | 2-3 days | None |
| 5 | Safe navigation `?.` | 3-4 days | None |
| 6 | Template literal dedenting | 3-5 days | Template literals (already exist) |
| 7 | File-based namespaces | 1-2 weeks | Module system |
| 8 | Exception handling | 2-3 weeks | None |
| 9 | Rich enums | 2-3 weeks | None |
| 10 | Interfaces with defaults | 1-2 weeks | None |
| 11 | Operator overloading | 1-2 weeks | None |
| 12 | Reflection system | 2-3 weeks | Rust native module infrastructure |
| 13 | Compiler optimizations | 5-7 weeks | Type checker info |

**Total estimated effort:** 22-31 weeks (5.5-7.75 months)

---

## Excluded Features

| Feature | Reason |
|---------|--------|
| JIT compilation | LuaJIT unmaintained; building JIT from scratch requires 2-5 years |
| Native AOT executables | Breaks compatibility with Lua game engines |
| WASM compilation | Out of scope |
| Result/Option types | Library concern, not language core |
| Lazy evaluation keyword | Manual getter pattern sufficient |
| Checked exceptions | Kotlin-style unchecked preferred |
| Full trait system | C#-style interfaces sufficient |
| Async/Await syntax | Coroutines already available, no need for sugar |
| Nested namespace blocks | File-scoped only (C# style) is simpler |
| Generator functions | Not needed |
| Extension methods | Not needed |
| Non-null assertion (`!`) | Bad practice, type narrowing preferred |
| Partial type argument inference | Too confusing |
| Tagged template literals | Not needed initially (can add later if requested) |

---

## Config Options Summary

```yaml
compilerOptions:
  # Existing options
  enableDecorators: true
  target: "5.4"

  # New options
  optimization: 2                # 0, 1, 2, or 3 (compiler optimizations)
  enforceNamespacePath: false    # Require namespace to match file path
```

**Notes:**
- No config flags needed for language features — they are always available
- `enforceNamespacePath`: When true, `math/vector.tl` must declare `namespace Math.Vector;`
- Reflection uses Rust native module (required dependency via LuaRocks)
- Compiler optimizations are critical for performance since JIT is not available
- Template literal auto-dedenting is always enabled (no config option)
