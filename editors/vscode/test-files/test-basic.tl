-- Test basic TypedLua features
function greet(name: string): string
    return "Hello, " .. name
end

local message = greet("World")
print(message)

-- Test local variables with type annotations
local count: number = 42
local isActive: boolean = true
local data: string = "test data"

-- Test const
const PI: number = 3.14159

-- Test table
local point = {
    x = 10,
    y = 20
}

-- Test function with multiple parameters
function add(a: number, b: number): number
    return a + b
end

local result = add(5, 3)
print("5 + 3 = " .. result)
