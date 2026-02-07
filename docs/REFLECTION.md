# TypedLua Reflection System

TypedLua includes a pure Lua reflection system that provides runtime introspection capabilities for classes. The reflection metadata is automatically generated during compilation and can be used for serialization, dependency injection, debugging, and more.

## Overview

Every class can receive reflection metadata during compilation when the module imports `@std/reflection`:

- `__typeId`: Unique numeric identifier for the class
- `__typeName`: String name of the class
- `__parent`: Reference to parent class (for inheritance)
- `__ancestors`: Table mapping ancestor type IDs to `true` for O(1) instanceof checks
- `__ownFields`: Array of field metadata for fields declared in this class
- `__ownMethods`: Array of method metadata for methods declared in this class
- `_buildAllFields()`: Lazy function to build complete field list including inherited fields
- `_buildAllMethods()`: Lazy function to build complete method list including inherited methods
- `__allFieldsCache`: Cache for complete field list (populated on first call)
- `__allMethodsCache`: Cache for complete method list (populated on first call)

## Selective Generation

Reflection metadata is only generated for classes in modules that import `@std/reflection`:

```typescript
import { reflect } from '@std/reflection'

class Animal {
    name: string
    age: number

    speak(): void {}
}
```

The import triggers metadata generation for `Animal`. Classes in modules without this import have no reflection overhead.

### Import Patterns

```typescript
-- Side-effect import (enables reflection for all classes in module)
import '@std/reflection'

-- Named import for explicit API access
import { reflect, Runtime } from '@std/reflection'

-- Default import
import reflection from '@std/reflection'
```

## Bit Flags for Field Modifiers

Field modifiers are encoded as compact 8-bit flags in the `__ownFields` array:

| Bit | Value | Meaning |
|-----|-------|---------|
| 0 | 1 | Public |
| 1 | 2 | Private |
| 2 | 4 | Protected |
| 3 | 8 | Readonly |
| 4 | 16 | Static |

```typescript
class Example {
    public value: number              -- _flags = 1
    private data: string              -- _flags = 2
    protected id: number              -- _flags = 4
    readonly config: table            -- _flags = 8
    static instance: Example          -- _flags = 16
    public static readonly counter: number  -- _flags = 1 | 16 | 8 = 25
}
```

Generated metadata format:
```lua
Example.__ownFields = {
    { name = "value", type = "number", _flags = 1 },
    { name = "data", type = "string", _flags = 2 },
    { name = "id", type = "number", _flags = 4 },
    { name = "config", type = "table", _flags = 8 },
    { name = "instance", type = "Example", _flags = 16 },
    { name = "counter", type = "number", _flags = 25 },
}
```

### Decoding Flags at Runtime

```lua
local FLAG_PUBLIC = 1
local FLAG_PRIVATE = 2
local FLAG_PROTECTED = 4
local FLAG_READONLY = 8
local FLAG_STATIC = 16

function decodeFieldFlags(flags)
    return {
        isPublic = (flags & FLAG_PUBLIC) ~= 0,
        isPrivate = (flags & FLAG_PRIVATE) ~= 0,
        isProtected = (flags & FLAG_PROTECTED) ~= 0,
        isReadonly = (flags & FLAG_READONLY) ~= 0,
        isStatic = (flags & FLAG_STATIC) ~= 0,
    }
end
```

## Compact Method Signatures

Method signatures use a compact string encoding for parameters and return types:

### Type Abbreviation Codes

| Code | Type |
|------|------|
| `n` | number |
| `s` | string |
| `b` | boolean |
| `t` | table |
| `f` | function |
| `v` | void |
| `o` | any/object |
| `?` | Optional prefix |

### Encoding Examples

```typescript
class Calculator {
    add(a: number, b: number): number {}        -- params = "nn", ret = "n"
    greet(name: string): void {}                 -- params = "s", ret = "v"
    getValue(): number { return 0 }              -- params = "", ret = "n"
    setData(data: table): void {}                -- params = "t", ret = "v"
    process(a: number, b: string): boolean {}    -- params = "ns", ret = "b"
}
```

Generated metadata:
```lua
Calculator.__ownMethods = {
    { name = "add", params = "nn", ret = "n" },
    { name = "greet", params = "s", ret = "v" },
    { name = "getValue", params = "", ret = "n" },
    { name = "setData", params = "t", ret = "v" },
    { name = "process", params = "ns", ret = "b" },
}
```

### Complex Types

Union types, optional types, and arrays use extended notation:

```typescript
class Complex {
    value: number | string                        -- "n|s"
    optional?: number                             -- "?n"
    items: number[]                               -- "[n]"
    callback: (a: number) => void                 -- "fn(n)->v"
    union: string | number | boolean              -- "n|s|b"
}
```

Generated:
```lua
Complex.__ownFields = {
    { name = "value", type = "n|s", _flags = 1 },
    { name = "optional", type = "?n", _flags = 1 },
    { name = "items", type = "[n]", _flags = 1 },
    { name = "callback", type = "fn(n)->v", _flags = 1 },
    { name = "union", type = "n|s|b", _flags = 1 },
}
```

## Performance Characteristics

- **instanceof checks**: O(1) - single table lookup in pre-computed ancestor table
- **typeof**: O(1) - metatable access
- **getFields()/getMethods() first call**: O(n) where n = number of ancestors
- **getFields()/getMethods() cached calls**: O(1) - return cached array
- **Memory overhead**: ~800 bytes static + ~2KB lazy caches (if reflection is used)

## Generated Code Example

Given this TypedLua class:

```typescript
import '@std/reflection'

class Animal {
    public name: string
    protected age: number

    speak(): void {}
}

class Dog extends Animal {
    readonly breed: string

    bark(): void {}
}
```

TypedLua generates the following reflection metadata:

```lua
-- Animal class
Animal.__typeId = 1
Animal.__typeName = "Animal"
Animal.__ancestors = {
    [1] = true,
}
Animal.__ownFields = {
    { name = "name", type = "string", _flags = 1 },
    { name = "age", type = "number", _flags = 4 },
}
Animal.__ownMethods = {
    { name = "speak", params = "", ret = "v" },
}

function Animal._buildAllFields()
    if Animal.__allFieldsCache then
        return Animal.__allFieldsCache
    end
    local fields = {}
    for _, field in ipairs(Animal.__ownFields) do
        table.insert(fields, field)
    end
    Animal.__allFieldsCache = fields
    return fields
end

function Animal._buildAllMethods()
    if Animal.__allMethodsCache then
        return Animal.__allMethodsCache
    end
    local methods = {}
    for _, method in ipairs(Animal.__ownMethods) do
        table.insert(methods, method)
    end
    Animal.__allMethodsCache = methods
    return methods
end

-- Dog class (with inheritance)
Dog.__typeId = 2
Dog.__typeName = "Dog"
Dog.__parent = Animal
Dog.__ancestors = {
    [2] = true,
}
if Animal and Animal.__ancestors then
    for ancestorId, _ in pairs(Animal.__ancestors) do
        Dog.__ancestors[ancestorId] = true
    end
end
Dog.__ownFields = {
    { name = "breed", type = "string", _flags = 9 },
}
Dog.__ownMethods = {
    { name = "bark", params = "", ret = "v" },
}

function Dog._buildAllFields()
    if Dog.__allFieldsCache then
        return Dog.__allFieldsCache
    end
    local fields = {}
    for _, field in ipairs(Dog.__ownFields) do
        table.insert(fields, field)
    end
    if Dog.__parent and Dog.__parent._buildAllFields then
        local parentFields = Dog.__parent._buildAllFields()
        for _, field in ipairs(parentFields) do
            table.insert(fields, field)
        end
    end
    Dog.__allFieldsCache = fields
    return fields
end

function Dog._buildAllMethods()
    if Dog.__allMethodsCache then
        return Dog.__allMethodsCache
    end
    local methods = {}
    for _, method in ipairs(Dog.__ownMethods) do
        table.insert(methods, method)
    end
    if Dog.__parent and Dog.__parent._buildAllMethods then
        local parentMethods = Dog.__parent._buildAllMethods()
        for _, method in ipairs(parentMethods) do
            table.insert(methods, method)
        end
    end
    Dog.__allMethodsCache = methods
    return methods
end
```

## Usage Examples

### Example 1: Check if object is instance of a type (O(1))

```lua
function isInstance(obj, Type)
    local mt = getmetatable(obj)
    if not mt or not mt.__typeId then
        return false
    end

    if not Type.__ancestors then
        return false
    end

    return Type.__ancestors[mt.__typeId] == true
end

local dog = Dog.new()
print(isInstance(dog, Dog))     -- true
print(isInstance(dog, Animal))  -- true (inheritance)
print(isInstance(dog, Cat))     -- false
```

### Example 2: Get type information

```lua
function typeof(obj)
    local mt = getmetatable(obj)
    if not mt or not mt.__typeName then
        return nil
    end

    return {
        id = mt.__typeId,
        name = mt.__typeName,
        parent = mt.__parent,
        getFields = function() return mt._buildAllFields() end,
        getMethods = function() return mt._buildAllMethods() end,
        getOwnFields = function() return mt.__ownFields end,
        getOwnMethods = function() return mt.__ownMethods end,
    }
end

local dog = Dog.new()
local typeInfo = typeof(dog)
print(typeInfo.name)  -- "Dog"
print(typeInfo.id)    -- 2

local fields = typeInfo.getFields()
-- fields = { {name="name", type="string", _flags=1}, {name="age"...} }
```

### Example 3: JSON Serializer with Type Information

```lua
local FLAG_PUBLIC = 1
local FLAG_PRIVATE = 2
local FLAG_PROTECTED = 4

function toJSON(obj, indent)
    indent = indent or 0
    local typeInfo = typeof(obj)

    if not typeInfo then
        if type(obj) == "string" then
            return string.format('"%s"', obj)
        else
            return tostring(obj)
        end
    end

    local padding = string.rep("  ", indent)
    local result = "{\n"

    local fields = typeInfo.getFields()
    for i, field in ipairs(fields) do
        -- Skip private/protected fields
        if (field._flags & (FLAG_PRIVATE | FLAG_PROTECTED)) == 0 then
            local value = obj[field.name]
            result = result .. padding .. "  " .. field.name .. ": "

            if type(value) == "table" then
                result = result .. toJSON(value, indent + 1)
            elseif type(value) == "string" then
                result = result .. string.format('"%s"', value)
            else
                result = result .. tostring(value)
            end

            if i < #fields then
                result = result .. ","
            end
            result = result .. "\n"
        end
    end

    result = result .. padding .. "}"
    return result
end

local dog = Dog.new()
dog.name = "Buddy"
dog.age = 5
dog.breed = "Golden Retriever"

print(toJSON(dog))
-- Output:
-- {
--   name: "Buddy",
--   breed: "Golden Retriever"
-- }
-- Note: age is protected, so it's excluded
```

### Example 4: Object Inspector

```lua
local FLAG_PUBLIC = 1
local FLAG_PRIVATE = 2
local FLAG_PROTECTED = 4
local FLAG_READONLY = 8
local FLAG_STATIC = 16

function inspect(obj)
    local typeInfo = typeof(obj)

    if not typeInfo then
        print("Not a class instance")
        return
    end

    print("Type: " .. typeInfo.name)
    print("Type ID: " .. typeInfo.id)

    if typeInfo.parent then
        print("Extends: " .. typeInfo.parent.__typeName)
    end

    print("\nFields:")
    local fields = typeInfo.getFields()
    for _, field in ipairs(fields) do
        local flags = decodeFieldFlags(field._flags)
        local modifiers = {}
        if flags.isPublic then table.insert(modifiers, "public") end
        if flags.isPrivate then table.insert(modifiers, "private") end
        if flags.isProtected then table.insert(modifiers, "protected") end
        if flags.isReadonly then table.insert(modifiers, "readonly") end
        if flags.isStatic then table.insert(modifiers, "static") end

        local modifierStr = #modifiers > 0
            and (" [" .. table.concat(modifiers, " ") .. "]")
            or ""

        local value = obj[field.name]
        print(string.format("  %s: %s%s = %s",
            field.name, field.type, modifierStr, tostring(value)))
    end

    print("\nMethods:")
    local methods = typeInfo.getMethods()
    for _, method in ipairs(methods) do
        local paramTypes = parseParams(method.params)
        print(string.format("  %s(%s): %s",
            method.name, paramTypes, method.ret))
    end
end

local dog = Dog.new()
dog.name = "Max"
dog.breed = "Labrador"

inspect(dog)
-- Output:
-- Type: Dog
-- Type ID: 2
-- Extends: Animal
--
-- Fields:
--   name: string [public] = Max
--   age: number [protected] = nil
--   breed: string [readonly] = Labrador
--
-- Methods:
--   bark(): void
--   speak(): void
```

### Example 5: Method Signature Analysis

```lua
function analyzeMethods(Type)
    local methods = Type.__ownMethods or {}

    print("Method signatures for " .. (Type.__typeName or "unknown") .. ":\n")

    for _, method in ipairs(methods) do
        print(string.format("  %s(%s) -> %s",
            method.name,
            method.params,
            method.ret))
    end
end

analyzeMethods(Dog)
-- Output:
-- Method signatures for Dog:
--   bark() -> void
--   speak() -> void
```

## Implementation Details

### Ancestor Table Pre-computation

The `__ancestors` table is pre-computed at compile time with runtime merging:

1. Each class adds its own `__typeId` to its ancestors table
2. At class load time, parent ancestors are merged using `pairs()` iteration
3. This creates a flattened ancestry chain for O(1) instanceof checks

### Lazy Building with Caching

The `_buildAllFields()` and `_buildAllMethods()` functions:

1. Check if cache exists (`__allFieldsCache` / `__allMethodsCache`)
2. If cached, return immediately (O(1))
3. If not cached:
   - Start with own fields/methods
   - Recursively walk parent chain to collect inherited fields/methods
   - Store result in cache
   - Return cached result

This ensures inheritance traversal only happens once per class, with subsequent calls being O(1).

### Memory Overhead

Per class with reflection enabled:
- Static metadata (~800 bytes): Type ID, name, parent reference, ancestors table, compact field/method arrays with bit flags
- Lazy caches (~2KB if used): All fields and all methods caches

Total: ~800 bytes baseline, ~3KB if reflection is actively used on a class.

### Memory Savings from Bit Flags

| Feature | Old Format | New Format | Savings |
|---------|-----------|------------|---------|
| Field modifiers | Table per field | 1 byte integer | ~60% |
| Method signatures | Nested tables | Compact string | ~50% |
| Overall metadata | ~2KB | ~800 bytes | ~60% |

## Runtime Library

The `@std/reflection` module provides utility functions:

```typescript
import { reflect, Runtime, decodeFlags, parseParams } from '@std/reflection'

-- Get type information
local info = reflect(obj)

-- Check if type matches
if Runtime.isInstance(obj, SomeClass) then
    -- ...
end

-- Get all fields with decoded flags
local fields = Runtime.getFields(obj)

-- Decode field flags
local flags = decodeFlags(field._flags)

-- Parse parameter signature string
local types = parseParams("nns")  -- returns {"number", "number", "string"}
```

## Configuration

### CLI Options

```bash
--reflection=full     # Generate for all classes (no import needed)
--reflection=selective # Only for imported modules (default)
--reflection=none     # Disable reflection
```

### Compiler API

```typescript
import { Compiler } from '@std/compiler'

const compiler = new Compiler({
    reflection: 'selective' | 'full' | 'none'
})
```

## Compatibility

The reflection system uses pure Lua and works with:
- ✅ Lua 5.1, 5.2, 5.3, 5.4
- ✅ LuaJIT
- ✅ LÖVE (game framework)
- ✅ Defold (game engine)
- ✅ Roblox (Luau)
- ✅ Any Lua runtime (no native dependencies)

## Migration from v1

The v2 format is a breaking change from v1 reflection:

### Breaking Changes

| v1 Field | v2 Field |
|----------|----------|
| `field.modifiers = { "public", "readonly" }` | `field._flags = 9` |
| `method.params = { {name="a", type="number"} }` | `method.params = "n"` |
| `method.returnType = "number"` | `method.ret = "n"` |

### Migration Helper

```lua
local function migrateField(field)
    local newField = {
        name = field.name,
        type = field.type or "",
    }

    -- Decode old modifiers table to flags
    if field.modifiers then
        local flags = 0
        for _, mod in ipairs(field.modifiers) do
            if mod == "public" then flags = flags | 1
            elseif mod == "private" then flags = flags | 2
            elseif mod == "protected" then flags = flags | 4
            elseif mod == "readonly" then flags = flags | 8
            elseif mod == "static" then flags = flags | 16
            end
        end
        newField._flags = flags
    end

    return newField
end

local function migrateMethod(method)
    local params = {}
    local ret = ""

    if method.params then
        local paramStr = {}
        for _, p in ipairs(method.params) do
            table.insert(paramStr, typeToCode(p.type))
        end
        params = table.concat(paramStr)
    end

    if method.returnType then
        ret = typeToCode(method.returnType)
    end

    return {
        name = method.name,
        params = params,
        ret = ret,
    }
end
```

## Limitations

- Reflection must be explicitly imported per module (cannot be globally enabled)
- Complex generic types are stored as strings, not structured data
- Cross-module inheritance requires both modules to import reflection
- Private/protected fields are still accessible via raw Lua tables (convention only)