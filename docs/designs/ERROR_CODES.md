# TypedLua Error Codes Reference

This document provides a comprehensive reference for all TypedLua compiler error codes.

## Error Code Format

Error codes follow the format: `[PREFIX][NUMBER]`
- Prefix: `E` for errors, `W` for warnings
- Number: 4-digit code indicating the error category and specific error

## Quick Reference

| Range | Category | Description |
|-------|----------|-------------|
| E1000-E1999 | Lexer | Tokenization errors |
| E2000-E2999 | Parser | Syntax errors |
| E3000-E3999 | Type Checker | Type errors |
| E4000-E4999 | Code Generator | Code generation errors |
| E5000-E5999 | Configuration | Configuration errors |
| W1000-W9999 | Warnings | Non-fatal issues |

---

## Lexer Errors (E1000-E1999)

### E1001: Unterminated String Literal
**Cause**: String literal not properly closed with a quote.

**Example**:
```lua
const message = "Hello, world
```

**Fix**: Add the closing quote:
```lua
const message = "Hello, world"
```

---

### E1002: Unterminated Multi-line Comment
**Cause**: Multi-line comment not properly closed.

**Example**:
```lua
--[[ This comment is not closed
const x = 5
```

**Fix**: Add the closing delimiter:
```lua
--[[ This comment is properly closed ]]--
const x = 5
```

---

### E1003: Invalid Number Literal
**Cause**: Number format is invalid.

**Example**:
```lua
const x = 123.456.789  -- Multiple decimal points
```

**Fix**: Use valid number format:
```lua
const x = 123.456
```

---

### E1004: Unexpected Character
**Cause**: Character not valid in TypedLua syntax.

**Example**:
```lua
const x = 5 @ 10  -- @ not valid operator
```

**Fix**: Use valid syntax:
```lua
const x = 5 + 10
```

---

### E1005: Invalid Escape Sequence
**Cause**: Unknown or malformed escape sequence in string.

**Example**:
```lua
const path = "C:\new\folder"  -- \n and \f are escape sequences
```

**Fix**: Use valid escape sequences or raw strings:
```lua
const path = "C:\\new\\folder"
```

---

### E1006: Unterminated Template Literal
**Cause**: Template literal not properly closed.

**Example**:
```lua
const greeting = `Hello, ${name}
```

**Fix**: Add the closing backtick:
```lua
const greeting = `Hello, ${name}`
```

---

### E1007: Invalid Hexadecimal Number
**Cause**: Hex number format is invalid.

**Example**:
```lua
const x = 0xGHIJ  -- G-J not valid hex digits
```

**Fix**: Use valid hex digits (0-9, A-F):
```lua
const x = 0x1234
```

---

### E1008: Invalid Binary Number
**Cause**: Binary number format is invalid.

**Example**:
```lua
const x = 0b1012  -- 2 not valid in binary
```

**Fix**: Use only 0 and 1:
```lua
const x = 0b1010
```

---

## Parser Errors (E2000-E2999)

### E2001: Expected Token
**Cause**: Expected a specific token but found something else.

**Example**:
```lua
if x > 5
    print(x)  -- Missing 'then'
end
```

**Fix**: Add the expected token:
```lua
if x > 5 then
    print(x)
end
```

---

### E2002: Unexpected Token
**Cause**: Token appeared where it shouldn't.

**Example**:
```lua
const x = 5 end  -- 'end' unexpected here
```

**Fix**: Remove or relocate the token:
```lua
const x = 5
```

---

### E2003: Expected Identifier
**Cause**: Expected a variable/function name.

**Example**:
```lua
function 123() end  -- Number instead of identifier
```

**Fix**: Use a valid identifier:
```lua
function myFunc() end
```

---

### E2008: Missing 'end'
**Cause**: Block not closed with 'end' keyword.

**Example**:
```lua
function greet()
    print("Hello")
-- Missing 'end'
```

**Fix**: Add 'end':
```lua
function greet()
    print("Hello")
end
```

---

### E2009: Missing 'then'
**Cause**: If statement missing 'then' after condition.

**Example**:
```lua
if x > 5
    print(x)
end
```

**Fix**: Add 'then':
```lua
if x > 5 then
    print(x)
end
```

---

### E2013: Break Outside Loop
**Cause**: Break statement used outside a loop.

**Example**:
```lua
function test()
    break  -- Not in a loop
end
```

**Fix**: Only use break inside loops:
```lua
function test()
    while true do
        break
    end
end
```

---

### E2020: Classes Disabled
**Cause**: Class syntax used but OOP features disabled in config.

**Example**:
```lua
class Point {
    x: number
}
```

**Fix**: Enable OOP in `tlconfig.yaml`:
```yaml
compilerOptions:
  enableOOP: true
```

---

### E2021: Decorators Disabled
**Cause**: Decorator syntax used but decorators disabled in config.

**Example**:
```lua
@readonly
class Point { }
```

**Fix**: Enable decorators in `tlconfig.yaml`:
```yaml
compilerOptions:
  enableDecorators: true
```

---

### E2022: Functional Programming Features Disabled
**Cause**: FP features (match, destructuring, etc.) used but disabled.

**Example**:
```lua
const [a, b] = arr  -- Destructuring disabled
```

**Fix**: Enable FP features in `tlconfig.yaml`:
```yaml
compilerOptions:
  enableFP: true
```

---

## Type Checker Errors (E3000-E3999)

### E3001: Type Mismatch
**Cause**: Value type doesn't match expected type.

**Example**:
```lua
const x: number = "hello"  -- String assigned to number
```

**Fix**: Match the types:
```lua
const x: number = 42
-- or
const x: string = "hello"
```

---

### E3002: Undefined Variable
**Cause**: Variable used before declaration.

**Example**:
```lua
print(x)  -- x not defined
```

**Fix**: Declare the variable first:
```lua
const x = 5
print(x)
```

---

### E3003: Duplicate Declaration
**Cause**: Variable declared twice in same scope.

**Example**:
```lua
const x = 5
const x = 10  -- Duplicate
```

**Fix**: Use different names or reassign:
```lua
const x = 5
const y = 10
```

---

### E3004: Cannot Assign to Constant
**Cause**: Trying to reassign a constant.

**Example**:
```lua
const x = 5
x = 10  -- Cannot reassign const
```

**Fix**: Use 'local' for mutable variables:
```lua
local x = 5
x = 10  -- OK
```

---

### E3005: Type Not Found
**Cause**: Referenced type doesn't exist.

**Example**:
```lua
const x: MyType = {}  -- MyType not defined
```

**Fix**: Define the type or import it:
```lua
type MyType = { name: string }
const x: MyType = { name: "test" }
```

---

### E3007: Wrong Number of Arguments
**Cause**: Function called with incorrect argument count.

**Example**:
```lua
function add(a: number, b: number): number {
    return a + b
}
add(5)  -- Missing second argument
```

**Fix**: Provide correct number of arguments:
```lua
add(5, 10)
```

---

### E3020: Pattern Match Not Exhaustive
**Cause**: Match expression doesn't handle all cases.

**Example**:
```lua
const result = match value {
    1 => "one"
    2 => "two"
    -- Missing default case
}
```

**Fix**: Add all cases or a default:
```lua
const result = match value {
    1 => "one"
    2 => "two"
    _ => "other"
}
```

---

## Code Generator Errors (E4000-E4999)

### E4001: Unsupported Feature
**Cause**: Feature not supported in target Lua version.

**Example**:
```lua
-- Using bitwise operators with Lua 5.1 target
const x = 5 << 2
```

**Fix**: Change target version or use alternative:
```yaml
# In tlconfig.yaml
compilerOptions:
  target: "5.3"
```

---

## Configuration Errors (E5000-E5999)

### E5001: Invalid Configuration
**Cause**: Configuration file has syntax or validation errors.

**Fix**: Check `tlconfig.yaml` syntax and values.

---

### E5003: Invalid Lua Target
**Cause**: Specified Lua target version is invalid.

**Example**:
```yaml
compilerOptions:
  target: "6.0"  # Invalid version
```

**Fix**: Use valid version (5.1, 5.2, 5.3, or 5.4):
```yaml
compilerOptions:
  target: "5.4"
```

---

## Warnings (W1000-W9999)

### W1001: Unused Variable
**Cause**: Variable declared but never used.

**Example**:
```lua
const x = 5  -- Never used
print("Hello")
```

**Fix**: Remove variable or prefix with underscore if intentional:
```lua
const _x = 5  -- Indicates intentionally unused
```

---

### W1003: Deprecated Feature
**Cause**: Using a deprecated language feature.

**Fix**: Use the recommended alternative shown in the diagnostic.

---

### W1004: Unreachable Code
**Cause**: Code after return/break/continue that will never execute.

**Example**:
```lua
function test() {
    return 5
    print("Never runs")  -- Unreachable
}
```

**Fix**: Remove unreachable code:
```lua
function test() {
    return 5
}
```

---

### W1006: Possible Nil Value
**Cause**: Variable might be nil when used.

**Example**:
```lua
local x: string?
print(x.length)  -- x might be nil
```

**Fix**: Check for nil first:
```lua
local x: string?
if x != nil then
    print(x.length)
end
```

---

## Getting More Help

For detailed explanations of any error code, visit:
- Online Documentation: https://typedlua.dev/errors/[CODE]
- CLI Help: `typedlua --explain E3001`

## Contributing

If you encounter an error without a code or believe an error message could be improved, please file an issue at:
https://github.com/yourusername/typedlua/issues
