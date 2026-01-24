-- TypedLua Built-in Error Classes
-- Provides a hierarchy of error types for exception handling

--[[
 Base Error class - all errors inherit from this
]]
export class Error {
    message: string
    stack: string | nil

    constructor(message: string) {
        self.message = message
        self.stack = debug.traceback()
    }

    toString(): string {
        return self.message
    }
}

--[[
 ArgumentError - thrown when invalid arguments are passed to a function
]]
export class ArgumentError extends Error {
    argumentName: string | nil

    constructor(message: string, argumentName: string | nil = nil) {
        super(message)
        self.argumentName = argumentName
    }
}

--[[
 StateError - thrown when an operation is invalid in the current state
]]
export class StateError extends Error {
    constructor(message: string) {
        super(message)
    }
}

--[[
 IOError - thrown when I/O operations fail
]]
export class IOError extends Error {
    path: string | nil

    constructor(message: string, path: string | nil = nil) {
        super(message)
        self.path = path
    }
}

--[[
 ParseError - thrown when parsing fails
]]
export class ParseError extends Error {
    line: number | nil
    column: number | nil

    constructor(message: string, line: number | nil = nil, column: number | nil = nil) {
        super(message)
        self.line = line
        self.column = column
    }
}

-- ============================================================================
-- Helper Functions
-- ============================================================================

-- Require - assert a condition is true, throw ArgumentError if false
export function require(condition: boolean, message: string): void {
    if (!condition) then
        throw new ArgumentError(message)
    end
}

-- Check - validate a value is non-nil, throw ArgumentError if nil
export function check<T>(value: T | nil, message: string): T {
    if (value == nil) then
        throw new ArgumentError(message ?? "Value cannot be nil")
    end
    return value
}

-- Unreachable - mark code that should never be reached
export function unreachable(message: string | nil): never {
    throw new StateError(message ?? "Reached unreachable code")
}
