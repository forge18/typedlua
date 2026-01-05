-- Test error detection in TypedLua

-- Function with type annotations
function add(a: number, b: number): number
    return a + b
end

-- Type error: passing string to number parameter
-- Should show diagnostic: Type 'string' is not assignable to type 'number'
local result1 = add(10, "20")

-- Type error: assigning number to string variable
local message: string = 42

-- Missing return type error
function broken(x: number)
    return x * 2  -- Should infer return type, but better to be explicit
end

-- Parse error: missing 'end'
function incomplete()
    local x = 1
    print(x)
-- Missing 'end' keyword - should show parse error

-- Type mismatch in table
type Person = {
    name: string,
    age: number
}

-- Error: 'age' should be number, not string
const person: Person = {
    name = "Alice",
    age = "25"  -- Type error
}

-- Undefined variable
print(undefinedVariable)  -- Should show error

-- Wrong number of arguments
function triple(a: number, b: number, c: number): number
    return a + b + c
end

local result2 = triple(1, 2)  -- Missing argument

-- Return type mismatch
function getString(): string
    return 123  -- Should return string, not number
end

-- Invalid operator
local invalid = "hello" - 5  -- Can't subtract number from string
