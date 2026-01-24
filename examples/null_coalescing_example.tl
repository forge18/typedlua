-- Null Coalescing Operator Example
-- This demonstrates the ?? operator for handling nil values

-- Basic usage: provide default values for nil
const port: number | nil = nil
const defaultPort = port ?? 3000
print("Port: " .. defaultPort)  -- Output: Port: 3000

-- Difference from 'or': ?? preserves falsy values like 0 and false
const count: number | nil = 0
const withOr = count or 10           -- Wrong! Result: 10
const withNullCoalesce = count ?? 10 -- Correct! Result: 0
print("With 'or': " .. withOr)
print("With '??': " .. withNullCoalesce)

-- Chaining: check multiple fallback values
const primary: number | nil = nil
const secondary: number | nil = nil
const tertiary = 100
const result = primary ?? secondary ?? tertiary
print("Chained result: " .. result)  -- Output: 100

-- Configuration objects with optional properties
type Config = {
    host?: string,
    port?: number,
    timeout?: number
}

const config: Config = {
    port: 8080
}

const host = config.host ?? "localhost"
const serverPort = config.port ?? 3000
const timeout = config.timeout ?? 5000

print("Server config: " .. host .. ":" .. serverPort .. " (timeout: " .. timeout .. "ms)")

-- Function return values
const getUserPreference = (): number | nil => {
    return nil
}

const pageSize = getUserPreference() ?? 25
print("Page size: " .. pageSize)

-- Nested property access with defaults
const settings = {
    database: {
        port: nil
    }
}

const dbPort = settings.database.port ?? 5432
print("Database port: " .. dbPort)
