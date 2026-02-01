# Runtime Validation from Types

## Overview

TypedLua generates runtime validation code directly from type annotations. Types are the single source of truth - no separate schema definitions, no validation libraries. The compiler knows the types at compile time and emits specialized validator code in the output Lua.

## Constraint Syntax: `Refined<>`

`Refined<Base, Constraints>` is a built-in utility type that attaches runtime validation constraints to a base type.

```typescript
type Username = Refined<string, { minLength: 3, maxLength: 20 }>
type Email = Refined<string, { pattern: "^[^@]+@[^@]+$" }>
type Port = Refined<number, { min: 1, max: 65535, integer: true }>
type Percentage = Refined<number, { min: 0, max: 100 }>
```

### Why `Refined<>` Instead of Intersection Syntax

The naive approach `string & { minLength: 3 }` is ambiguous. `{ minLength: 3 }` is already a valid table type (a table with a `minLength` field of literal type `3`). The intersection `string & { minLength: 3 }` means "a value that is simultaneously a string AND a table" - nonsensical, but syntactically valid. The type checker would need special-case logic to decide when an object literal in an intersection is "actually constraints" based on what it's intersected with. That's fragile.

`Refined<>` is unambiguous. It's a utility type (like `Partial<T>` or `Record<K,V>`). The parser sees `Refined<arg1, arg2>` as a generic type reference. The type checker recognizes `Refined` by name and treats the second argument as constraint metadata. No parser changes needed - it's already valid syntax.

### Built-in Constraint Keys

**String:**

- `minLength: number` - minimum string length
- `maxLength: number` - maximum string length
- `pattern: string` - Lua pattern match
- `nonEmpty: true` - shorthand for `minLength: 1`

**Number:**

- `min: number` - minimum value (inclusive)
- `max: number` - maximum value (inclusive)
- `integer: true` - must be a whole number

**Array:**

- `minLength: number` - minimum array length
- `maxLength: number` - maximum array length
- `nonEmpty: true` - shorthand for `minLength: 1`

The type checker validates constraint keys against the base type:

- `Refined<string, { minLength: 3 }>` - valid
- `Refined<number, { minLength: 3 }>` - compile error, numbers don't have length
- `Refined<string, { min: 0 }>` - compile error, `min` is a number constraint

### Custom Validators

The `custom` key takes a pure function literal for constraints that built-in keys can't express. The compiler inlines the function body directly into the generated validator:

```typescript
type EvenNumber = Refined<number, { custom: (n: number) => n % 2 == 0 }>
type MultipleOfThree = Refined<number, { custom: (n: number) => n % 3 == 0 }>
type ISODate = Refined<string, { custom: (s: string) => string.match(s, "^%d%d%d%d%-%d%d%-%d%d$") ~= nil }>
```

Can be combined with built-in keys:

```typescript
type PositiveEven = Refined<number, { min: 1, custom: (n: number) => n % 2 == 0 }>
```

The function `(n: number) => n % 2 == 0` is not called at runtime as a function. The compiler extracts the expression body and splices it into the validator as a condition. No function call overhead, no closure allocation.

**Constraint:** the function literal must be a pure expression with no side effects and no outer scope references. The compiler rejects closures that capture variables.

### Composability

`Refined<>` types compose naturally with unions, optionals, arrays, and generics:

```typescript
type Name = Refined<string, { minLength: 1, maxLength: 50 }>
type Age = Refined<number, { min: 0, max: 150, integer: true }>

type Person = { name: Name, age: Age, email?: Email }
type People = Array<Person>
type MaybePerson = Person | nil
```

---

## Validation Modes

### Mode 1: Auto-validate at boundaries (default)

The compiler inserts validation at function boundaries where unvalidated data enters. Works with any type - `Refined<>` is not required. Plain types get structural checks, refined types get structural + constraint checks.

```typescript
// Plain types: structural checks generated (is name a string? is age a number?)
function createUser(name: string, age: number) {
    // generated: type checks for name and age
}

// Refined types: structural + constraint checks generated
function createAdmin(name: Refined<string, {minLength: 3}>, age: Refined<number, {min: 18}>) {
    // generated: type checks AND constraint checks
}

// Compound types: full structural validation
function savePerson(person: { name: string, age: number, address: { city: string } }) {
    // generated: table check, field type checks, nested table checks
}
```

"Boundaries" means:

- Function parameters with typed annotations
- Return values with typed annotations
- Assignment from `unknown` to a typed variable

Internal assignments between variables of the same type do not re-validate.

### Mode 2: `@validate` decorator opt-in

Validation only runs on functions decorated with `@validate`. Same behavior: plain types get structural checks, refined types get structural + constraint checks.

```typescript
@validate
function createUser(name: string, age: number) {
    // structural checks generated: is name a string? is age a number?
}

@validate
function createAdmin(name: Refined<string, {minLength: 3}>, age: Refined<number, {min: 18}>) {
    // structural + constraint checks generated
}

function greet(name: string) {
    // no @validate = no runtime checks
}
```

### Compiler Intrinsics

`parse<T>()`, `safeParse<T>()`, and `is` always emit validation regardless of mode setting.

```typescript
// parse - throws on failure
local user = parse<Person>(json_decode(raw_input))

// safeParse - returns result object
local result = safeParse<Person>(json_decode(raw_input))
if result.success then
    local user = result.data  -- narrowed to Person
else
    print(result.errors)
end

// is - boolean type guard, integrates with control flow narrowing
if data is Person then
    print(data.name)  -- data narrowed to Person
end
```

These are compiler intrinsics, not library functions. The compiler sees `parse<Person>(expr)` and emits the validator call directly. No runtime dispatch, no type metadata at runtime.

---

## Validator Code Generation

### Specialized Functions

The compiler generates a dedicated validator per type containing only the checks that apply:

```lua
-- For: type Username = Refined<string, {minLength: 3, maxLength: 20}>
function __validate_Username(val, path)
    if type(val) ~= "string" then return false, path..": expected string, got "..type(val) end
    if #val < 3 then return false, path..": minLength 3, got "..#val end
    if #val > 20 then return false, path..": maxLength 20, got "..#val end
    return true
end

-- For: type Person = { name: Username, age: Refined<number, {min: 0}> }
function __validate_Person(val, path)
    if type(val) ~= "table" then return false, path..": expected table, got "..type(val) end
    local ok, err
    ok, err = __validate_Username(val.name, path..".name")
    if not ok then return false, err end
    if type(val.age) ~= "number" then return false, path..".age: expected number, got "..type(val.age) end
    if val.age < 0 then return false, path..".age: min 0, got "..val.age end
    return true
end
```

No runtime interpretation of constraint tables. Each validator is a straight-line sequence of checks. Nested types call their sub-validators directly.

### Inlining

For simple refined primitives (roughly <= 5 checks, used at <= 3 parse sites), the compiler inlines validation directly at the call site:

```lua
-- parse<Port>(input) where Port = Refined<number, {min: 1, max: 65535, integer: true}>
-- Inlined directly (no function call):
if type(input) ~= "number" then error("expected number, got " .. type(input)) end
if input < 1 or input > 65535 then error("must be between 1 and 65535, got " .. input) end
if input ~= math.floor(input) then error("must be integer") end
local port = input
```

Inlining is not profitable when:

- The type is complex (nested objects, arrays) - duplicates large code blocks
- The type is parsed at many call sites - each site gets a full copy
- Collect-mode errors are enabled - error accumulation logic adds bulk

At O0-O1, always generate functions (predictable, debuggable). At O2+, inline where profitable using the same heuristic as the existing aggressive inlining pass.

---

## Error Handling

### Fail-fast mode

Stops at the first validation failure.

`parse<T>()` throws. `safeParse<T>()` returns the first error:

```lua
local ok, err = __validate_Person(data, "")
if not ok then error("Validation failed: " .. err) end
```

### Collect mode

Accumulates all validation errors:

```lua
function __validate_Person(val, path, errors)
    errors = errors or {}
    if type(val) ~= "table" then
        errors[#errors+1] = { path = path, expected = "table", got = type(val) }
        return false, errors
    end
    __validate_Username(val.name, path..".name", errors)
    if type(val.age) ~= "number" then
        errors[#errors+1] = { path = path..".age", expected = "number", got = type(val.age) }
    elseif val.age < 0 then
        errors[#errors+1] = { path = path..".age", constraint = "min", expected = 0, got = val.age }
    end
    return #errors == 0, errors
end
```

`safeParse<T>()` returns the full error list:

```lua
{
    success = false,
    errors = {
        { path = ".name", expected = "string", got = "number" },
        { path = ".age", constraint = "min", expected = 0, got = -5 },
    }
}
```

Sensible short-circuits still apply in collect mode. If `type(val) ~= "table"`, field constraints are skipped since the whole branch is invalid.

---

## Optimizations

1. **Dead validator elimination** - Types with `Refined<>` never used with `parse`/`safeParse`/`@validate` don't get validators generated. Handled by existing DCE pass.

2. **Validator deduplication** - Structurally identical types share a validator. Hash the generated validator body and dedup at link time.

3. **Short-circuit type checks for unions** - Check `type()` first to narrow quickly before checking fields:

   ```lua
   local t = type(val)
   if t == "string" then return true end
   if t == "number" then return true end
   if t == "table" then ... end
   ```

4. **Monomorphize generic validators** - `parse<Box<string>>()` and `parse<Box<number>>()` generate separate specialized validators at compile time. No runtime type parameter dispatch.

5. **Hoist string patterns** - Regex patterns in `Refined<string, {pattern: ...}>` are hoisted to module-level locals to avoid re-creation on each validation call.

---

## Known Limitations

### Recursive Types

Recursive types like `type Tree = { value: number, children: Tree[] }` can cause stack overflow in naive validators. Two configurable strategies:

**Depth limit** - Counter decremented on each recursive call. At zero, skip deeper validation (assume valid). Fast and predictable.

**Seen-set** - Track visited table references. Skip tables already validated. Correct for cyclic structures (Lua tables can reference themselves). Slightly more overhead per check.

### Metatables

Generated validators use plain field access (`data.name`), which transparently follows the `__index` metatable chain. Class instances with inherited fields validate correctly. Validators do not use `rawget`, which would bypass metatables and miss inherited fields.

### Class Instances vs. Plain Tables

Three configurable strategies for `parse<MyClass>(data)`:

**Structural** (default) - Check that fields exist with correct types. Right for JSON parsing and external data.

**Identity** - Check instanceof via `__ancestors` table (uses existing reflection system). Right for internal assertions.

**Both** - Identity check first (fast single table lookup), structural fallback if identity fails.

Per-call override:

```typescript
local user = parse<Person>(data, "structural")
local user = parse<Person>(data, "identity")
```

### Nil vs. Absent Fields

Lua cannot distinguish `{name = nil}` from `{}`. Both return `nil` for `t.name`. Validation semantics:

- `field?: T` (optional) - may be absent or nil. Validator skips if nil.
- `field: T | nil` (nilable) - treated identically to optional at the validation level.
- `field: T` (required) - must be present and non-nil. Validator errors if nil.

`T | nil` and `T?` are equivalent at the validation level. To distinguish present-nil from absent, use a sentinel value.

### Number Precision

`Refined<number, {integer: true}>` generates `val == math.floor(val)`, which works on all Lua versions but loses precision above 2^53 on Lua 5.1/5.2/LuaJIT (double-based). For Lua 5.3+, prefer the `integer` type for native integer semantics.

---

## Configuration

Project-level configuration, overridable per-file:

```toml
[validation]
mode = "auto"              # "auto" | "explicit" | "off"
errors = "fail_fast"       # "fail_fast" | "collect"
recursion = "depth_limit"  # "depth_limit" | "seen_set"
max_depth = 32             # only used with depth_limit
class_mode = "structural"  # "structural" | "identity" | "both"
```

Per-file override:

```typescript
// @typedlua validation-mode: explicit
// @typedlua validation-errors: collect
```
