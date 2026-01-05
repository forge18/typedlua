-- Test TypedLua language features

-- Decorators
@deprecated
function oldFunction(): string
    return "This function is deprecated"
end

@readonly
const MAX_SIZE: number = 1000

-- Template literals
const name = "TypedLua"
const version = "0.1.0"
const message = `Welcome to ${name} version ${version}!`
print(message)

-- Arrow functions
const square = (x: number): number => x * x
const greet = (name: string): string => `Hello, ${name}!`

print(square(5))  -- 25
print(greet("World"))  -- Hello, World!

-- Match expression
enum Status {
    Success = "success",
    Error = "error",
    Pending = "pending"
}

function handleStatus(status: Status): string
    return match status
        when Status.Success then "Operation completed"
        when Status.Error then "Operation failed"
        when Status.Pending then "Operation in progress"
    end
end

-- Optional chaining
type Config = {
    database?: {
        host?: string,
        port?: number
    }
}

const config: Config = {}
local host = config.database?.host ?? "localhost"
local port = config.database?.port ?? 5432

print(`Database: ${host}:${port}`)

-- Spread operator
const numbers1 = {1, 2, 3}
const numbers2 = {4, 5, 6}
const combined = {...numbers1, ...numbers2}

-- Destructuring
local {x, y} = {x = 10, y = 20}
print(`x: ${x}, y: ${y}`)

-- Rest parameters
function sum(...args: number[]): number
    local total = 0
    for i, v in ipairs(args) do
        total = total + v
    end
    return total
end

print(sum(1, 2, 3, 4, 5))  -- 15

-- Type guards
function isString(value: unknown): value is string
    return type(value) == "string"
end

function processValue(value: string | number): void
    if isString(value) then
        print("String: " .. value)
    else
        print("Number: " .. tostring(value))
    end
end

-- Async/await (future feature)
-- async function fetchData(url: string): Promise<string>
--     const response = await fetch(url)
--     return await response:text()
-- end
