# TypedLua Reflection System

TypedLua includes a pure Lua reflection system that provides runtime introspection capabilities for classes. The reflection metadata is automatically generated during compilation and can be used for serialization, dependency injection, debugging, and more.

## Overview

Every class in TypedLua automatically receives reflection metadata during compilation:

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

## Performance Characteristics

- **instanceof checks**: O(1) - single table lookup in pre-computed ancestor table
- **typeof**: O(1) - metatable access
- **getFields()/getMethods() first call**: O(n) where n = number of ancestors
- **getFields()/getMethods() cached calls**: O(1) - return cached array
- **Memory overhead**: ~2KB static + ~3KB lazy caches (if reflection is used)

## Generated Code Example

Given this TypedLua class:

```typescript
class Animal {
    name: string
    age: number

    speak(): void {}
}

class Dog extends Animal {
    breed: string

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
    { name = "name" },
    { name = "age" },
}
Animal.__ownMethods = {
    { name = "speak", isStatic = false },
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
    [2] = true,  -- Dog's own type ID
    -- Parent ancestors merged at runtime:
}
if Animal and Animal.__ancestors then
    for ancestorId, _ in pairs(Animal.__ancestors) do
        Dog.__ancestors[ancestorId] = true  -- Now includes [1] = true from Animal
    end
end
Dog.__ownFields = {
    { name = "breed" },
}
Dog.__ownMethods = {
    { name = "bark", isStatic = false },
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
    -- Similar to _buildAllFields but for methods
    -- Includes parent methods recursively
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

-- Get all fields (including inherited)
local fields = typeInfo.getFields()
-- fields = { {name="breed"}, {name="name"}, {name="age"} }
```

### Example 3: JSON Serializer using reflection

```lua
function toJSON(obj, indent)
    indent = indent or 0
    local typeInfo = typeof(obj)

    if not typeInfo then
        -- Primitive value
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
--   breed: "Golden Retriever",
--   name: "Buddy",
--   age: 5
-- }
```

### Example 4: Object Inspector

```lua
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
        local value = obj[field.name]
        print("  " .. field.name .. " = " .. tostring(value))
    end

    print("\nMethods:")
    local methods = typeInfo.getMethods()
    for _, method in ipairs(methods) do
        local static = method.isStatic and " (static)" or ""
        print("  " .. method.name .. "()" .. static)
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
--   breed = Labrador
--   name = Max
--   age = nil
--
-- Methods:
--   bark()
--   speak()
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

Per class:
- Static metadata (~2KB): Type ID, name, parent reference, ancestors table, own fields/methods arrays
- Lazy caches (~3KB if used): All fields and all methods caches

Total: ~2KB baseline, ~5KB if reflection is actively used on a class.

## Future Enhancements

- **Selective Generation**: Only generate reflection metadata for classes in modules that import `@std/reflection` (reduces code size)
- **Bit Flags**: Use compact bit flags for field modifiers (optional, readonly) to save ~40% memory
- **Method Signatures**: Store parameter types and return type in compact array format
- **Runtime Library**: Provide `@std/reflection.tl` with ready-to-use `Runtime` API

## Limitations

- Reflection metadata is currently generated for ALL classes (not selectively)
- Field metadata doesn't include type information or modifiers yet
- Method signatures are not captured (only name and isStatic flag)
- No built-in runtime API library (users must implement their own helpers)

## Compatibility

The reflection system uses pure Lua and works with:
- ✅ Lua 5.1, 5.2, 5.3, 5.4
- ✅ LuaJIT
- ✅ LÖVE (game framework)
- ✅ Defold (game engine)
- ✅ Roblox (Luau)
- ✅ Any Lua runtime (no native dependencies)
