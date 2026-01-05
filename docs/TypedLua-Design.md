# TypedLua Design Document

## Project Overview

TypedLua is a typed superset of Lua, inspired by TypeScript's gradual typing approach and developer experience. It aims to provide robust type checking while maintaining Lua's simplicity and allowing gradual adoption.

**Implementation Language:** Rust

**Philosophy:**
- Stay as close to Lua syntax as possible
- Allow gradual adoption (like TypeScript)
- Avoid object-oriented programming initially (can be added later)
- Stricter than TypeScript in some areas (no `any` type)

---

## Type System Fundamentals

### Types vs Interfaces

**Clear, enforced distinction:**

#### `interface` - Table Shapes Only
- Used exclusively for describing table/object structures
- Can extend other interfaces
- Support generics
- Composition via `extends` keyword

```lua
interface User {
    id: number,
    name: string,
    email?: string
}

interface Admin extends User {
    role: string,
    permissions: string[]
}

interface Container<T> {
    value: T,
    get: () -> T
}
```

**Rules:**
- ✅ Can extend multiple interfaces: `interface Admin extends User, Auditable`
- ❌ Cannot use intersection types: `type X = Admin & User` is invalid
- ✅ Supports optional fields with `?`
- ✅ Supports generic type parameters

#### `type` - Everything Except Table Shapes
- Aliases for primitives
- Union types
- Function signatures  
- Tuple types
- References to interfaces
- **Cannot define table shapes**

```lua
type UserId = number
type Status = "active" | "inactive" | "banned"
type Result<T> = T | nil
type Handler = (x: number, y: string) -> boolean
type Coordinates = [number, number, number]

-- Can reference interfaces
type MaybeUser = User | nil
```

**Rules:**
- ❌ Cannot define table shapes: `type Point = {x: number, y: number}` is a **compiler error**
- ✅ Can create unions of interfaces
- ✅ Supports generic type parameters

### Inline vs Named Types

**All three forms are valid:**

```lua
-- 1. Named interface (reusable)
interface Point {
    x: number,
    y: number
}
local point: Point = {x = 1, y = 2}

-- 2. Inline anonymous table shape (one-off usage)
local point: {x: number, y: number} = {x = 1, y = 2}

-- 3. Generic table type (untyped)
local point: table = {x = 1, y = 2}
```

**Key distinction:**
- ✅ Inline table shapes allowed in variable/parameter annotations
- ✅ Named table shapes via `interface`
- ❌ Table shapes in `type` aliases (compiler error)

---

## Base Types

### Primitives

```lua
nil       -- Lua's null value
boolean   -- true/false
number    -- Lua's number type
integer   -- Subset of number (useful for Lua 5.3+, array indices)
string    -- Text
```

### Special Types

```lua
unknown   -- Type-safe unknown, must narrow before use (throws errors by default)
never     -- Bottom type (impossible values, exhaustiveness checking)
void      -- Functions that return nothing
table     -- Generic table type (base for all interfaces)
coroutine -- Lua coroutines (thread type)
```

**Note:** No `any` type - forces developers to be more explicit about types.

**Userdata:** Not explicitly typed; use `unknown` or define specific interfaces for C libraries.

### Composite Types

```lua
-- Arrays (both syntaxes supported)
number[]           -- Preferred
Array<number>      -- Also valid

-- Tuples
[string, number]   -- Ordered, fixed-length

-- Functions
(x: number) -> boolean
(input: string) -> [result: any, error: string | nil]  -- Multi-return with named elements
```

---

## Syntax

### Variable Declarations

TypedLua provides two types of variable declarations:

**`const` - Immutable Variables**
```lua
const status = "active"  -- Cannot be reassigned
const PI = 3.14159

status = "inactive"  -- ERROR: Cannot reassign const variable
```

**`local` - Mutable Variables**
```lua
local count = 0     -- Can be reassigned
local name = "Bob"

count = 5           -- OK
name = "Alice"      -- OK
```

**Compilation:**
Both `const` and `local` compile to Lua's `local` keyword - the immutability is enforced only at compile-time:

```lua
-- TypedLua:
const x = 5
local y = 10

-- Compiled Lua:
local x = 5
local y = 10
```

**Why both?**
- Immutability checking catches bugs at compile-time
- Clear developer intent
- Enables better type inference (see Type Inference section)
- Zero runtime overhead

### Type Annotations

Use `:` for type annotations (same as TypeScript):

```lua
local user: User = getUser()
local count: number = 0

function greet(name: string): string
    return "Hello, " .. name
end

-- Method calls still use : as in Lua (context distinguishes)
obj:method()
```

### Function Types

Use `->` for function type arrows (Lua-style):

```lua
type Handler = (x: number, y: string) -> boolean
type Parser = (input: string) -> [any, string | nil]
type Callback = () -> void
```

### Nullable/Optional

**Union syntax:**
```lua
type MaybeUser = User | nil
local user: User | nil = getUser()
```

**Shorthand syntax:**
```lua
type MaybeUser = User?  -- Expands to User | nil
local result: string?   -- Same as string | nil
```

**Optional table fields:**
```lua
interface User {
    id: number,
    name: string,
    email?: string  -- Optional field
}
```

### Literal Types

Support for string, number, and boolean literals as types:

```lua
type Status = "active" | "inactive" | "banned"
type Version = 1 | 2 | 3
type Enabled = true
type HttpMethod = "GET" | "POST" | "PUT" | "DELETE"
```

### Enums

TypedLua supports runtime enums that compile to Lua tables, providing both type safety and runtime value access.

**Three enum types:**

#### 1. Auto-increment Numeric Enums
Values automatically increment starting from 1 (Lua array convention):

```lua
enum Role {
  Guest,      -- 1
  User,       -- 2
  Moderator,  -- 3
  Admin       -- 4
}

-- Usage:
local userRole: Role = Role.User
if userRole == Role.Admin then
  -- ...
end
```

#### 2. Explicit Numeric Enums
All values must be explicitly defined:

```lua
enum Permission {
  None = 0,
  Read = 1,
  Write = 2,
  Execute = 4
}

-- Useful for bit flags:
local perms: Permission = Permission.Read | Permission.Write
```

#### 3. String Enums
All values must be explicitly defined as strings:

```lua
enum Status {
  Active = "active",
  Inactive = "inactive",
  Banned = "banned"
}

-- Usage:
local userStatus: Status = Status.Active
```

**Enum Rules:**
- ❌ No mixing types: cannot have both strings and numbers in one enum
- ❌ No mixing explicit/implicit: either all auto-increment OR all explicit values
- ✅ Auto-increment starts at 1 (following Lua array convention)
- ✅ Enums compile to runtime Lua tables for iteration and validation

**Compiled Output:**
```lua
-- TypedLua source:
enum Role {
  Guest,
  User,
  Admin
}

-- Compiled Lua:
local Role = {
  Guest = 1,
  User = 2,
  Admin = 3
}
```

**Type Usage:**
```lua
-- Enums can be used as types
function setUserRole(role: Role): void
  -- role must be a valid Role enum value
end

-- Valid
setUserRole(Role.Admin)

-- Type error
setUserRole(5)
```

### Generics

TypedLua supports generics on interfaces, types, and functions, enabling type-safe reusable code.

#### Generic Interfaces

```lua
interface Container<T> {
  value: T,
  get: () -> T,
  set: (value: T) -> void
}

local stringContainer: Container<string> = {
  value = "hello",
  get = function() return self.value end,
  set = function(value) self.value = value end
}
```

#### Generic Types

```lua
type Result<T> = T | nil
type Pair<T, U> = [T, U]
type Handler<T> = (data: T) -> void
type Dictionary<K, V> = table  -- More specific typing TBD
```

#### Generic Functions

```lua
function identity<T>(value: T): T
  return value
end

local num = identity(5)        -- T inferred as number
local str = identity("hello")  -- T inferred as string

function map<T, U>(arr: T[], fn: (item: T) -> U): U[]
  local result: U[] = {}
  for i, item in ipairs(arr) do
    result[i] = fn(item)
  end
  return result
end

local nums = {1, 2, 3}
local strs = map(nums, function(n) return tostring(n) end)
-- Infers: T = number, U = string, returns string[]
```

#### Generic Constraints

Use `extends` to constrain type parameters:

```lua
interface Lengthwise {
  length: number
}

function logLength<T extends Lengthwise>(item: T): void
  print(item.length)
end

logLength("hello")     -- OK: string has length
logLength({1, 2, 3})   -- OK: arrays have length
logLength(42)          -- ERROR: number doesn't have length
```

#### Multiple Constraints

```lua
interface Comparable<T> {
  compareTo: (other: T) -> number
}

interface Sortable<T extends Comparable<T> & Lengthwise> {
  items: T[],
  sort: () -> void
}
```

#### Default Type Parameters

```lua
interface Response<T = unknown, E = string> {
  data: T,
  error: E | nil,
  status: number
}

-- Can use with defaults
local response1: Response = getResponse()  -- Response<unknown, string>

-- Or specify types
local response2: Response<User> = getUser()  -- Response<User, string>

-- Or specify all
local response3: Response<User, Error> = getUser()  -- Response<User, Error>
```

#### Generic Rules

- ✅ No hard limit on number of type parameters
- ✅ Type parameters can have constraints via `extends`
- ✅ Multiple constraints combined with `&`
- ✅ Default type parameters supported
- ✅ Type inference for generic function calls
- ❌ Generics not supported on enums (enums are concrete value sets)

**Type Parameter Inference:**

The compiler infers generic type parameters from usage:

```lua
function first<T>(arr: T[]): T | nil
  return arr[1]
end

local nums = {1, 2, 3}
local result = first(nums)  -- Infers T = number, returns number | nil
```

Explicit type arguments can override inference:

```lua
local result = first<string>({"a", "b"})  -- Explicit: T = string
```

### Type Narrowing

TypedLua supports comprehensive type narrowing through control flow analysis, type guards, and assertion functions.

#### Built-in Type Guards

**Using `type()` checks:**
```lua
local value: string | number = getValue()

if type(value) == "string" then
  -- value is string here
  local len = value:len()
else
  -- value is number here
  local doubled = value * 2
end
```

**Truthiness narrowing:**
```lua
local str: string | nil = getOptionalString()

if str then
  -- str is string (nil eliminated)
  print(str:upper())
end
```

**Equality checks:**
```lua
local value: string | number | nil = getValue()

if value == nil then
  -- value is nil
elseif type(value) == "string" then
  -- value is string
else
  -- value is number
end
```

#### Discriminated Unions

Use literal types to create discriminated unions:

```lua
interface Circle {
  kind: "circle",
  radius: number
}

interface Rectangle {
  kind: "rectangle",
  width: number,
  height: number
}

type Shape = Circle | Rectangle

function area(shape: Shape): number
  if shape.kind == "circle" then
    -- shape is Circle here
    return 3.14 * shape.radius * shape.radius
  else
    -- shape is Rectangle here
    return shape.width * shape.height
  end
end
```

#### Property-based Narrowing

Narrowing based on property existence:

```lua
type Point2D = {x: number, y: number}
type Point3D = {x: number, y: number, z: number}

local point: Point2D | Point3D = getPoint()

if point.z ~= nil then
  -- point is Point3D here
  print(point.z)
else
  -- point is Point2D here
  print("2D point")
end
```

#### Custom Type Guards

Define custom type guard functions using `is` predicates:

```lua
function isString(value: unknown): value is string
  return type(value) == "string"
end

function isUser(value: unknown): value is User
  return type(value) == "table" 
    and value.id ~= nil 
    and value.name ~= nil
end

-- Usage:
local data: unknown = getData()

if isString(data) then
  -- data is string here
  print(data:upper())
end

if isUser(data) then
  -- data is User here
  print(data.name)
end
```

**Type guards in array operations:**
```lua
local items: (string | nil)[] = getItems()

-- Explicit type guard enables narrowing
local filtered = filter(items, function(item): item is string
  return item ~= nil
end)
-- filtered is string[]
```

#### Assertion Functions

Functions that assert a type and throw on failure:

```lua
function assertIsString(value: unknown): asserts value is string
  assert(type(value) == "string", "Value must be a string")
end

function assertIsDefined<T>(value: T | nil): asserts value is T
  assert(value ~= nil, "Value must be defined")
end

-- Usage:
local data: unknown = getData()
assertIsString(data)
-- After this point, data is narrowed to string
print(data:upper())

local user: User | nil = getUser()
assertIsDefined(user)
-- After this point, user is narrowed to User
print(user.name)
```

#### Exhaustiveness Checking

Use the `never` type to ensure all union cases are handled:

```lua
type Status = "active" | "inactive" | "banned"

function handleStatus(status: Status): void
  if status == "active" then
    -- handle active
  elseif status == "inactive" then
    -- handle inactive
  elseif status == "banned" then
    -- handle banned
  else
    -- Exhaustiveness check
    local _exhaustive: never = status
    -- If we missed a case, this line will error at compile time
  end
end
```

If a new status is added to the union, the compiler will error at the `never` assignment.

#### Narrowing Scope Rules

**Narrowing is conservative across function boundaries:**

```lua
local value: string | nil = getValue()

if value then
  -- value is string here
  someFunction()
  -- value is back to string | nil (function might have mutated it)
  if value then  -- Must re-check
    print(value:upper())
  end
end
```

**Why?** Lua allows mutation anywhere, so the type system assumes functions might modify variables.

**Narrowing persists within a block:**

```lua
local value: string | number = getValue()

if type(value) == "string" then
  -- value is string throughout this entire block
  local len = value:len()
  local upper = value:upper()
  -- Still string here
end
```

### Type Inference

TypedLua follows TypeScript's approach to type inference, inferring types aggressively while maintaining type safety.

#### Literal Type Widening

**`const` variables infer to literal types:**
```lua
const status = "active"  -- Infers: "active" (literal type)
const count = 5          -- Infers: 5 (literal type)
const flag = true        -- Infers: true (literal type)

-- Type error:
status = "inactive"      -- ERROR: Cannot reassign const
```

**`local` variables widen to general types:**
```lua
local status = "active"  -- Infers: string (widened)
local count = 5          -- Infers: number (widened)
local flag = true        -- Infers: boolean (widened)

-- OK - can reassign:
status = "inactive"      -- OK
count = 10               -- OK
flag = false             -- OK
```

**Why the difference?**
- `const` values can't change, so the literal type is always safe
- `local` values can be reassigned, so they need the wider type

#### Array Type Inference

**Homogeneous arrays:**
```lua
const numbers = {1, 2, 3}           -- Infers: number[]
const strings = {"a", "b", "c"}     -- Infers: string[]
```

**Heterogeneous arrays (automatic union):**
```lua
const mixed = {1, "hello", 3}       -- Infers: (number | string)[]
const items = {user, 42, "text"}   -- Infers: (User | number | string)[]
```

**Arrays with nil:**
```lua
const values = {1, nil, 3}          -- Infers: (number | nil)[]
const optional = {user, nil}        -- Infers: (User | nil)[]
```

#### Return Type Inference

**Return types are always inferred (never required):**
```lua
function add(a: number, b: number)
  return a + b  -- Infers return type: number
end

function getUser(id: number)
  if id > 0 then
    return {id = id, name = "User"}
  end
  return nil
end
-- Infers return type: {id: number, name: string} | nil
```

**Explicit return types are optional but recommended for public APIs:**
```lua
-- With explicit return type (clearer intent):
function divide(a: number, b: number): number | nil
  if b == 0 then
    return nil
  end
  return a / b
end
```

#### Contextual Typing

**Types are inferred from context:**

```lua
interface Handler {
  onClick: (event: ClickEvent) -> void,
  onHover: (event: HoverEvent) -> void
}

-- Parameter types inferred from Handler interface:
const handler: Handler = {
  onClick = function(event)  -- event is ClickEvent
    event.preventDefault()
  end,
  onHover = function(event)  -- event is HoverEvent
    event.getPosition()
  end
}
```

**Callback parameter inference:**
```lua
const users: User[] = getUsers()

-- user parameter type is inferred from users array:
const names = map(users, function(user)
  return user.name  -- user is User
end)
-- names inferred as string[]

-- Works with arrow-style callbacks too:
const ids = filter(users, function(u) return u.id > 100 end)
-- u is User, ids is User[]
```

#### Generic Type Inference

**Type parameters are inferred from arguments:**
```lua
function identity<T>(value: T): T
  return value
end

const num = identity(5)         -- T inferred as number
const str = identity("hello")   -- T inferred as string
const user = identity(getUser()) -- T inferred as User
```

**Multiple type parameters:**
```lua
function pair<T, U>(first: T, second: U): [T, U]
  return {first, second}
end

const p1 = pair(5, "hello")     -- T = number, U = string, returns [number, string]
const p2 = pair(user, 42)       -- T = User, U = number, returns [User, number]
```

**Inference with constraints:**
```lua
interface Lengthwise {
  length: number
}

function getLength<T extends Lengthwise>(item: T): number
  return item.length
end

const len1 = getLength("hello")    -- T inferred as string
const len2 = getLength({1, 2, 3})  -- T inferred as number[]
```

#### When Inference Falls Back to `unknown`

**Cannot infer complex types:**
```lua
const data = parseJSON(jsonString)  -- Infers: unknown
const result = complexFunction()    -- Infers: unknown (can't determine)
```

**With `noImplicitUnknown: true`:**
```lua
-- ERROR with noImplicitUnknown:
const data = parseJSON(jsonString)  -- Error: Cannot infer type

-- Must provide explicit type:
const data: User = parseJSON(jsonString)
```

#### Best Practices

**When to use explicit types:**

✅ **Do annotate:**
- Function parameters (always required)
- Public API return types (for clarity)
- When inference would give `unknown`
- Complex types that aren't obvious from context

❌ **Don't annotate:**
- Simple variable assignments (inference works)
- Return types for private helper functions
- Generic type arguments when inferable

**Examples:**
```lua
-- Good - let inference work:
const count = 5
const items = getItems()
const doubled = map(nums, function(n) return n * 2 end)

-- Good - explicit where helpful:
function processUser(user: User): ProcessedUser
  const result: ProcessedUser = transform(user)
  return result
end

-- Unnecessary - over-annotated:
const count: number = 5  -- Redundant, inference knows it's number
const name: string = getName()  -- Redundant if getName returns string
```

## Module System

TypedLua uses `import` and `export` keywords that compile to Lua's `require()` system, providing a familiar TypeScript-like syntax while maintaining compatibility with Lua's module system.

### Exporting

**Export interfaces, types, functions, and values:**

```lua
-- user.tl

-- Exported (public API)
export interface User {
  id: number,
  name: string,
  email?: string
}

export type UserId = number

export function createUser(name: string): User
  return {id = nextId(), name = name}
end

export function getUser(id: UserId): User | nil
  -- implementation
end

-- Not exported (module-private)
local nextIdCounter = 0

function nextId(): number
  nextIdCounter = nextIdCounter + 1
  return nextIdCounter
end
```

**Default exports:**

```lua
-- logger.tl
interface Logger {
  log: (message: string) -> void,
  error: (message: string) -> void
}

local logger: Logger = {
  log = function(message) print(message) end,
  error = function(message) print("ERROR: " .. message) end
}

export default logger
```

**Compiled output:**

```lua
-- user.lua (compiled from user.tl)
local nextIdCounter = 0

local function nextId()
  nextIdCounter = nextIdCounter + 1
  return nextIdCounter
end

local function createUser(name)
  return {id = nextId(), name = name}
end

local function getUser(id)
  -- implementation
end

-- Exported items bundled into module table
return {
  createUser = createUser,
  getUser = getUser
}
```

```lua
-- logger.lua (compiled from logger.tl with default export)
local logger = {
  log = function(message) print(message) end,
  error = function(message) print("ERROR: " .. message) end
}

return logger
```

### Importing

**Named imports:**
```lua
import { User, createUser } from "./user"

const u: User = createUser("Bob")
```

**Namespace import:**
```lua
import * as user from "./user"

const u: user.User = user.createUser("Bob")
```

**Default import:**
```lua
import logger from "./logger"

logger.log("Hello, world!")
```

**Mixed imports:**
```lua
import createUser, { User, UserId } from "./user"

const id: UserId = 1
const u: User = createUser("Bob")
```

**Type-only imports (compile-time only):**
```lua
import type { User } from "./user"  -- Type only, no runtime code

const u: User = {id = 1, name = "Bob"}  -- OK
const creator = User  -- ERROR: User is type-only, not a value
```

**Compiled imports:**

```lua
-- TypedLua source:
import { User, createUser } from "./user"
const u: User = createUser("Bob")

-- Compiled Lua:
local _user = require("./user")
local createUser = _user.createUser
local u = createUser("Bob")
```

```lua
-- TypedLua source:
import * as user from "./user"
const u: user.User = user.createUser("Bob")

-- Compiled Lua:
local user = require("./user")
local u = user.createUser("Bob")
```

```lua
-- TypedLua source:
import type { User } from "./user"
const u: User = {id = 1, name = "Bob"}

-- Compiled Lua (type-only import stripped):
local u = {id = 1, name = "Bob"}
```

### Type Definition Files

For external Lua libraries without TypedLua source, use `.d.tl` type definition files.

**Example - LuaSocket type definitions:**

```lua
-- luasocket.d.tl

interface TcpClient {
  send: (data: string) -> number | nil,
  receive: (pattern?: string) -> string | nil,
  close: () -> void,
  settimeout: (timeout: number) -> void
}

interface UdpSocket {
  send: (data: string) -> number | nil,
  receive: () -> string | nil,
  close: () -> void
}

declare module "socket" {
  export function tcp(): TcpClient
  export function udp(): UdpSocket
  export function gettime(): number
}
```

**Using typed external library:**

```lua
-- main.tl
import { tcp } from "socket"  -- Types from luasocket.d.tl

const client = tcp()
client.connect("example.com", 80)
client.send("GET / HTTP/1.0\r\n\r\n")
const response = client.receive()
client.close()
```

**Declaration file structure:**

```lua
-- mylib.d.tl

-- Declare types
interface Config {
  timeout: number,
  retries: number
}

-- Declare module exports
declare module "mylib" {
  export interface Config {  -- Re-export if needed
    timeout: number,
    retries: number
  }
  
  export function init(config: Config): void
  export function process(data: string): string
  export const VERSION: string
}
```

### Module Resolution

Module paths are resolved following Lua's `package.path` conventions, with additional support for path aliases from `typedlua.json`.

**Relative imports:**
```lua
import { User } from "./user"          -- Same directory
import { Config } from "../config"     -- Parent directory
import { Utils } from "./utils/index"  -- Subdirectory
```

**Absolute imports (with path aliases):**
```lua
// typedlua.json
{
  "compilerOptions": {
    "baseUrl": "./src",
    "paths": {
      "@/*": ["*"],
      "@/components/*": ["components/*"]
    }
  }
}
```

```lua
import { Button } from "@/components/button"
import { User } from "@/types/user"
```

**Finding type definitions:**

When importing a module, the compiler looks for types in this order:

1. **TypedLua source:** `module.tl`
2. **Type definition:** `module.d.tl`
3. **Alongside Lua file:** `module.lua` + `module.d.tl` in same directory
4. **Fallback:** If no types found, imported module type is `unknown`

**Example search for** `require("socket")`:

1. Look for `socket.tl` (TypedLua source)
2. Look for `socket.d.tl` (type definitions)
3. Look for `socket/init.tl` or `socket/init.d.tl`
4. If not found, `require("socket")` returns `unknown`

### Interoperability with Lua

**TypedLua modules can be used from plain Lua:**

```lua
-- user.tl (TypedLua)
export interface User {
  id: number,
  name: string
}

export function createUser(name: string): User
  return {id = 1, name = name}
end
```

Compiles to standard Lua that works anywhere:

```lua
-- main.lua (plain Lua)
local user = require("user")
local u = user.createUser("Bob")
print(u.name)  -- Works fine
```

**Plain Lua modules can be used from TypedLua with type definitions:**

```lua
-- math_utils.lua (existing Lua code)
local M = {}
function M.square(x) return x * x end
return M
```

```lua
-- math_utils.d.tl (add types)
declare module "math_utils" {
  export function square(x: number): number
}
```

```lua
-- main.tl (TypedLua with types)
import { square } from "./math_utils"
const result = square(5)  -- Fully typed!
```

---

## Standard Library Type Definitions

TypedLua includes built-in type definitions for Lua's standard library. These types are always available without imports and provide full type safety for Lua's core functionality.

**Supported Lua versions:** 5.1, 5.2, 5.3, 5.4 (LuaJIT not officially supported)

### Design Principles

**Function overloads** - Multiple signatures for functions with different parameter combinations:
```lua
declare function string.find(s: string, pattern: string): number | nil
declare function string.find(s: string, pattern: string, init: number): number | nil  
declare function string.find(s: string, pattern: string, init: number, plain: boolean): number | nil
```

**Variadic functions** - Rest parameters for functions accepting variable arguments:
```lua
declare function print(...args: unknown[]): void
declare function string.format(formatstring: string, ...args: unknown[]): string
```

**Built-in availability** - No imports required, all standard library types available globally:
```lua
// No import needed:
const upper = string.upper("hello")  // Compiler knows the type
const floored = math.floor(3.7)      // Returns integer
```

### Global Functions

```lua
// Type checking and conversion
declare function type(v: unknown): string
declare function tonumber(e: string | number): number | nil
declare function tonumber(e: string | number, base: integer): number | nil
declare function tostring(v: unknown): string

// Assertions
declare function assert<T>(v: T | nil, message?: string): T

// Output
declare function print(...args: unknown[]): void

// Error handling
declare function error(message: string, level?: integer): never
declare function pcall<T, A>(f: (args: A) -> T, args: A): [boolean, T | string]
declare function xpcall<T, A>(f: (args: A) -> T, msgh: (err: string) -> string, args: A): [boolean, T | string]

// Iteration
declare function ipairs<T>(t: T[]): ((t: T[], i: integer) -> [integer, T] | nil, T[], integer)
declare function pairs<K, V>(t: table): ((t: table, k: K | nil) -> [K, V] | nil, table, nil)

// Metatables
declare function setmetatable<T>(table: T, metatable: table | nil): T
declare function getmetatable(object: unknown): table | nil

// Selection
declare function select<T>(index: integer | "#", ...args: T[]): T | integer

// Module loading
declare function require(modname: string): unknown

// Garbage collection
declare function collectgarbage(opt?: string, arg?: number): number | boolean
```

### String Library

```lua
declare module string {
  // Case conversion
  export function upper(s: string): string
  export function lower(s: string): string
  
  // Length
  export function len(s: string): integer
  
  // Substrings
  export function sub(s: string, i: integer): string
  export function sub(s: string, i: integer, j: integer): string
  
  // Pattern matching
  export function find(s: string, pattern: string): number | nil
  export function find(s: string, pattern: string, init: integer): number | nil
  export function find(s: string, pattern: string, init: integer, plain: boolean): number | nil
  
  export function match(s: string, pattern: string): string | nil
  export function match(s: string, pattern: string, init: integer): string | nil
  
  export function gmatch(s: string, pattern: string): () -> string | nil
  
  // String manipulation
  export function gsub(s: string, pattern: string, repl: string): [string, integer]
  export function gsub(s: string, pattern: string, repl: table): [string, integer]
  export function gsub(s: string, pattern: string, repl: (match: string) -> string): [string, integer]
  export function gsub(s: string, pattern: string, repl: string, n: integer): [string, integer]
  
  export function reverse(s: string): string
  export function rep(s: string, n: integer): string
  export function rep(s: string, n: integer, sep: string): string
  
  // Formatting
  export function format(formatstring: string, ...args: unknown[]): string
  
  // Character codes
  export function byte(s: string): integer
  export function byte(s: string, i: integer): integer
  export function byte(s: string, i: integer, j: integer): ...integer[]
  
  export function char(...args: integer[]): string
}
```

### Table Library

```lua
declare module table {
  // Array manipulation
  export function insert<T>(list: T[], value: T): void
  export function insert<T>(list: T[], pos: integer, value: T): void
  
  export function remove<T>(list: T[]): T | nil
  export function remove<T>(list: T[], pos: integer): T | nil
  
  export function concat(list: string[], sep?: string): string
  export function concat(list: string[], sep: string, i: integer): string
  export function concat(list: string[], sep: string, i: integer, j: integer): string
  
  // Sorting
  export function sort<T>(list: T[]): void
  export function sort<T>(list: T[], comp: (a: T, b: T) -> boolean): void
  
  // Lua 5.2+ functions
  export function pack(...args: unknown[]): table  // Lua 5.2+
  export function unpack<T>(list: T[]): ...T[]     // Lua 5.2+
  export function unpack<T>(list: T[], i: integer): ...T[]  // Lua 5.2+
  export function unpack<T>(list: T[], i: integer, j: integer): ...T[]  // Lua 5.2+
}
```

### Math Library

```lua
declare module math {
  // Constants
  export const pi: number
  export const huge: number
  export const mininteger: integer  // Lua 5.3+
  export const maxinteger: integer  // Lua 5.3+
  
  // Rounding
  export function floor(x: number): integer
  export function ceil(x: number): integer
  
  // Absolute value and sign
  export function abs(x: number): number
  
  // Min/Max
  export function max(...args: number[]): number
  export function min(...args: number[]): number
  
  // Exponents and logarithms
  export function exp(x: number): number
  export function log(x: number): number
  export function log(x: number, base: number): number
  export function log10(x: number): number  // Deprecated in 5.2+
  
  export function sqrt(x: number): number
  export function pow(x: number, y: number): number
  
  // Trigonometry
  export function sin(x: number): number
  export function cos(x: number): number
  export function tan(x: number): number
  export function asin(x: number): number
  export function acos(x: number): number
  export function atan(x: number): number
  export function atan2(y: number, x: number): number
  
  export function deg(x: number): number
  export function rad(x: number): number
  
  // Random
  export function random(): number
  export function random(m: integer): integer
  export function random(m: integer, n: integer): integer
  export function randomseed(x: integer): void
  
  // Other
  export function modf(x: number): [integer, number]
  export function fmod(x: number, y: number): number
  
  // Lua 5.3+ integer operations
  export function tointeger(x: number): integer | nil  // Lua 5.3+
  export function type(x: number): "integer" | "float"  // Lua 5.3+
  export function ult(m: integer, n: integer): boolean  // Lua 5.3+
}
```

### IO Library

```lua
interface File {
  read: (format?: string | number) -> string | number | nil,
  write: (...args: string[]) -> File | nil,
  lines: () -> () -> string | nil,
  close: () -> boolean | nil,
  flush: () -> boolean,
  seek: (whence?: "set" | "cur" | "end", offset?: integer) -> integer | nil,
  setvbuf: (mode: "no" | "full" | "line", size?: integer) -> boolean
}

declare module io {
  export function open(filename: string, mode?: string): File | nil
  export function close(file?: File): boolean
  export function input(file?: File | string): File
  export function output(file?: File | string): File
  export function read(format?: string | number): string | number | nil
  export function write(...args: string[]): File | nil
  export function flush(): boolean
  export function lines(filename?: string): () -> string | nil
  export function popen(prog: string, mode?: string): File | nil
  export function tmpfile(): File
  export function type(obj: unknown): "file" | "closed file" | nil
  
  export const stdin: File
  export const stdout: File
  export const stderr: File
}
```

### OS Library

```lua
interface DateTable {
  year: integer,
  month: integer,
  day: integer,
  hour: integer,
  min: integer,
  sec: integer,
  wday: integer,
  yday: integer,
  isdst: boolean
}

declare module os {
  export function clock(): number
  export function date(format?: string, time?: integer): string | DateTable
  export function difftime(t2: integer, t1: integer): number
  export function execute(command?: string): boolean | nil
  export function exit(code?: integer | boolean, close?: boolean): never
  export function getenv(varname: string): string | nil
  export function remove(filename: string): boolean | nil
  export function rename(oldname: string, newname: string): boolean | nil
  export function setlocale(locale: string, category?: string): string | nil
  export function time(table?: DateTable): integer
  export function tmpname(): string
}
```

### Coroutine Library

```lua
declare module coroutine {
  export function create<A, R>(f: (args: A) -> R): coroutine
  export function resume<A, R>(co: coroutine, args?: A): [boolean, R | string]
  export function running(): coroutine | nil
  export function status(co: coroutine): "running" | "suspended" | "normal" | "dead"
  export function wrap<A, R>(f: (args: A) -> R): (args: A) -> R
  export function yield<T>(...args: T[]): ...unknown[]
  
  // Lua 5.3+ additions
  export function isyieldable(): boolean  // Lua 5.3+
}
```

### Version-Specific Features

The compiler selects the appropriate standard library definitions based on the `target` setting in `typedlua.json`:

```json
{
  "compilerOptions": {
    "target": "lua5.4"  // Or "lua5.1", "lua5.2", "lua5.3"
  }
}
```

**Version differences handled:**

- **Lua 5.1**: No `table.pack`, `table.unpack`, `bit32` operations
- **Lua 5.2**: Added `table.pack`, `table.unpack`, `bit32` module, removed `module()`, `setfenv()`, `getfenv()`
- **Lua 5.3**: Added integer type, integer division `//`, bitwise operators, `utf8` module, `math.tointeger`, `math.type`, `math.ult`
- **Lua 5.4**: Added const variables (different from TypedLua's `const`), `warn()` function, `<close>` attribute

**Using version-specific features:**

```lua
// This code requires Lua 5.3+
const x: integer = math.tointeger(5.0)  // OK with target: "lua5.3" or "lua5.4"
                                        // ERROR with target: "lua5.1" or "lua5.2"
```

### Extending Standard Library Types

You can augment built-in types with additional methods:

```lua
// Extend string library with custom function
declare module string {
  export function trim(s: string): string
}

// Implementation
function string.trim(s: string): string
  return s:match("^%s*(.-)%s*$")
end

// Usage
const trimmed = string.trim("  hello  ")  // Fully typed
```

---

## Utility Types

TypedLua provides built-in utility types for common type transformations, following TypeScript's conventions with Lua-specific additions.

### Readonly Properties

Before exploring utility types, TypedLua supports `readonly` property modifiers:

```lua
interface User {
  readonly id: number,      -- Cannot be modified after creation
  name: string,             -- Can be modified
  readonly created: number  -- Cannot be modified
}

const user: User = {id = 1, name = "Bob", created = os.time()}
user.name = "Alice"  -- OK
user.id = 2          -- ERROR: Cannot assign to readonly property
user.created = 123   -- ERROR: Cannot assign to readonly property
```

**Readonly is compile-time only:**
```lua
-- TypedLua enforces readonly at compile-time
-- Compiled Lua has no runtime enforcement:
local user = {id = 1, name = "Bob", created = 1234567890}
user.name = "Alice"  -- Works
user.id = 2          -- Also works (no runtime error)
```

### Object Transformation Types

#### `Partial<T>`

Makes all properties in `T` optional:

```lua
interface User {
  id: number,
  name: string,
  email: string
}

type PartialUser = Partial<User>
// Equivalent to:
// {
//   id?: number,
//   name?: string,
//   email?: string
// }

function updateUser(id: number, updates: PartialUser): void
  -- Can update any subset of User properties
end

updateUser(1, {name = "Alice"})           -- OK
updateUser(2, {email = "bob@example.com"}) -- OK
updateUser(3, {})                          -- OK (empty updates)
```

#### `Required<T>`

Makes all properties in `T` required (removes optional modifiers):

```lua
interface Config {
  host?: string,
  port?: number,
  timeout?: number
}

type RequiredConfig = Required<Config>
// Equivalent to:
// {
//   host: string,
//   port: number,
//   timeout: number
// }

function initialize(config: RequiredConfig): void
  -- All properties must be provided
end

initialize({host = "localhost", port = 8080, timeout = 30})  -- OK
initialize({host = "localhost"})  -- ERROR: Missing port and timeout
```

#### `Readonly<T>`

Makes all properties in `T` readonly:

```lua
interface User {
  id: number,
  name: string
}

type ReadonlyUser = Readonly<User>
// Equivalent to:
// {
//   readonly id: number,
//   readonly name: string
// }

const user: ReadonlyUser = {id = 1, name = "Bob"}
user.name = "Alice"  -- ERROR: Cannot assign to readonly property
```

#### `Pick<T, Keys>`

Selects specific properties from `T`:

```lua
interface User {
  id: number,
  name: string,
  email: string,
  password: string,
  created: number
}

type UserPreview = Pick<User, "id" | "name">
// Equivalent to:
// {
//   id: number,
//   name: string
// }

function displayUser(user: UserPreview): void
  print(user.id, user.name)
end
```

#### `Omit<T, Keys>`

Removes specific properties from `T`:

```lua
interface User {
  id: number,
  name: string,
  email: string,
  password: string
}

type PublicUser = Omit<User, "password">
// Equivalent to:
// {
//   id: number,
//   name: string,
//   email: string
// }

function sendToClient(user: PublicUser): void
  -- Password field not accessible
end
```

#### `Record<Keys, Type>`

Creates an object type with specific keys and value type:

```lua
type UserRole = "admin" | "user" | "guest"
type Permissions = Record<UserRole, boolean>
// Equivalent to:
// {
//   admin: boolean,
//   user: boolean,
//   guest: boolean
// }

const permissions: Permissions = {
  admin = true,
  user = true,
  guest = false
}

// Numeric keys
type Cache = Record<integer, string>
const cache: Cache = {
  [1] = "value1",
  [2] = "value2"
}
```

### Union Manipulation Types

#### `Exclude<T, U>`

Removes types from a union:

```lua
type Status = "active" | "inactive" | "banned" | "pending"

type ActiveStatus = Exclude<Status, "banned" | "pending">
// Equivalent to: "active" | "inactive"

type NumberOrString = number | string | boolean
type OnlyNumberOrString = Exclude<NumberOrString, boolean>
// Equivalent to: number | string
```

#### `Extract<T, U>`

Extracts types from a union:

```lua
type Status = "active" | "inactive" | "banned" | "pending"

type InactiveStates = Extract<Status, "inactive" | "banned">
// Equivalent to: "inactive" | "banned"

type Mixed = number | string | User | boolean
type OnlyPrimitives = Extract<Mixed, number | string | boolean>
// Equivalent to: number | string | boolean
```

#### `NonNilable<T>`

Removes `nil` from a type (Lua-specific naming):

```lua
type MaybeUser = User | nil
type DefiniteUser = NonNilable<MaybeUser>
// Equivalent to: User

type MaybeValues = string | number | nil
type Values = NonNilable<MaybeValues>
// Equivalent to: string | number

function processUser(user: NonNilable<User | nil>): void
  -- user is guaranteed to be User, not nil
  print(user.name)
end
```

#### `Nullable<T>` (Lua-specific)

Shorthand for `T | nil`:

```lua
type MaybeUser = Nullable<User>
// Equivalent to: User | nil

function getUser(id: number): Nullable<User>
  if id > 0 then
    return {id = id, name = "User"}
  end
  return nil
end

// More concise than:
function getUser(id: number): User | nil
  -- ...
end
```

### Function Utility Types

#### `Parameters<F>`

Extracts parameter types from a function as a tuple:

```lua
function createUser(name: string, age: number, admin: boolean): User
  return {id = 1, name = name, age = age, admin = admin}
end

type CreateUserParams = Parameters<typeof createUser>
// Equivalent to: [string, number, boolean]

function logAndCreate(...args: CreateUserParams): User
  print("Creating user with:", args)
  return createUser(...args)
end
```

#### `ReturnType<F>`

Extracts the return type from a function:

```lua
function getUser(id: number): User
  return {id = id, name = "User"}
end

type UserType = ReturnType<typeof getUser>
// Equivalent to: User

function processData(): [string, number, boolean]
  return {"data", 42, true}
end

type ProcessResult = ReturnType<typeof processData>
// Equivalent to: [string, number, boolean]
```

### Combining Utility Types

Utility types can be composed for complex transformations:

```lua
interface User {
  id: number,
  name: string,
  email: string,
  password: string,
  role: "admin" | "user",
  created: number
}

// Partial public user (no password, optional fields)
type PartialPublicUser = Partial<Omit<User, "password">>
// Equivalent to:
// {
//   id?: number,
//   name?: string,
//   email?: string,
//   role?: "admin" | "user",
//   created?: number
// }

// Readonly subset
type UserCredentials = Readonly<Pick<User, "email" | "password">>
// Equivalent to:
// {
//   readonly email: string,
//   readonly password: string
// }

// Required fields for update
type UserUpdate = Required<Pick<User, "id">> & Partial<Omit<User, "id" | "created">>
// Equivalent to:
// {
//   id: number,           // Required
//   name?: string,        // Optional
//   email?: string,       // Optional
//   password?: string,    // Optional
//   role?: "admin" | "user" // Optional
// }
```

### Custom Utility Type Examples

You can define your own utility types using generics:

```lua
// Make specific properties optional
type PartialBy<T, K> = Omit<T, K> & Partial<Pick<T, K>>

interface User {
  id: number,
  name: string,
  email: string
}

type UserWithOptionalEmail = PartialBy<User, "email">
// Equivalent to:
// {
//   id: number,
//   name: string,
//   email?: string
// }

// Make specific properties required
type RequireBy<T, K> = Omit<T, K> & Required<Pick<T, K>>

// Deep readonly (for nested objects)
type DeepReadonly<T> = {
  readonly [K in keyof T]: T[K] extends table ? DeepReadonly<T[K]> : T[K]
}
```

### Built-in Utility Types Summary

**Object Transformation:**
- `Partial<T>` - All properties optional
- `Required<T>` - All properties required
- `Readonly<T>` - All properties readonly
- `Pick<T, Keys>` - Select specific properties
- `Omit<T, Keys>` - Remove specific properties
- `Record<Keys, Type>` - Create object from key/value types

**Union Manipulation:**
- `Exclude<T, U>` - Remove types from union
- `Extract<T, U>` - Extract types from union
- `NonNilable<T>` - Remove `nil` from type
- `Nilable<T>` - Shorthand for `T | nil` (Lua-specific)

**Function Utilities:**
- `Parameters<F>` - Extract parameter types as tuple
- `ReturnType<F>` - Extract return type

---

## Advanced Type Features

TypedLua supports advanced type-level programming features that enable powerful type transformations and generic abstractions.

### `keyof` Operator

Extracts all property names from a type as a string literal union:

```lua
interface User {
  id: number,
  name: string,
  email: string
}

type UserKeys = keyof User  // "id" | "name" | "email"

// Use with generics for type-safe property access:
function getProperty<T, K extends keyof T>(obj: T, key: K): T[K]
  return obj[key]
end

const user: User = {id = 1, name = "Bob", email = "bob@example.com"}
const name = getProperty(user, "name")  // Type is string
const id = getProperty(user, "id")      // Type is number
const invalid = getProperty(user, "age") // ERROR: "age" not in keyof User
```

**With arrays:**
```lua
type ArrayKeys = keyof string[]  // number | "length" | "push" | "pop" | ...
```

### Index Access Types

Access the type of a specific property:

```lua
interface User {
  id: number,
  name: string,
  tags: string[],
  metadata: {
    created: number,
    updated: number
  }
}

type UserId = User["id"]           // number
type UserName = User["name"]       // string
type UserTags = User["tags"]       // string[]

// Access nested properties:
type Created = User["metadata"]["created"]  // number

// Multiple properties (creates union):
type UserInfo = User["id" | "name"]  // number | string

// Array element type:
type Tag = User["tags"][number]  // string
```

**Practical example:**
```lua
function pluck<T, K extends keyof T>(array: T[], key: K): T[K][]
  const result: T[K][] = {}
  for i, item in ipairs(array) do
    table.insert(result, item[key])
  end
  return result
end

const users: User[] = getUsers()
const names = pluck(users, "name")  // string[]
const ids = pluck(users, "id")      // number[]
```

### Mapped Types

Transform each property in a type:

```lua
// Basic mapped type syntax:
type Mapped<T> = {
  [K in keyof T]: T[K]
}

// Add readonly modifier:
type Readonly<T> = {
  readonly [K in keyof T]: T[K]
}

interface User {
  id: number,
  name: string
}

type ReadonlyUser = Readonly<User>
// Result: { readonly id: number, readonly name: string }
```

**Add optional modifier:**
```lua
type Partial<T> = {
  [K in keyof T]?: T[K]
}

type PartialUser = Partial<User>
// Result: { id?: number, name?: string }
```

**Remove modifiers:**
```lua
// Remove optional (-? removes optional modifier)
type Required<T> = {
  [K in keyof T]-?: T[K]
}

// Remove readonly (-readonly removes readonly modifier)
type Mutable<T> = {
  -readonly [K in keyof T]: T[K]
}
```

**Transform property types:**
```lua
// Make all properties nullable:
type Nullable<T> = {
  [K in keyof T]: T[K] | nil
}

// Wrap all properties in arrays:
type Arrayify<T> = {
  [K in keyof T]: T[K][]
}

interface Point {
  x: number,
  y: number
}

type NullablePoint = Nullable<Point>
// Result: { x: number | nil, y: number | nil }

type ArrayPoint = Arrayify<Point>
// Result: { x: number[], y: number[] }
```

### Conditional Types

Types that change based on conditions:

```lua
// Syntax: T extends U ? X : Y
// If T is assignable to U, result is X, otherwise Y

type IsString<T> = T extends string ? true : false

type A = IsString<string>  // true
type B = IsString<number>  // false

type IsArray<T> = T extends unknown[] ? true : false

type C = IsArray<string[]>  // true
type D = IsArray<number>    // false
```

**Practical examples:**
```lua
// Flatten array type:
type Flatten<T> = T extends unknown[] ? T[number] : T

type Nums = Flatten<number[]>  // number
type Str = Flatten<string>     // string

// Extract function return type:
type ReturnTypeBasic<T> = T extends (...args: unknown[]) -> infer R ? R : never
```

**Distributive conditional types:**

When applied to union types, conditional types distribute:

```lua
type ToArray<T> = T extends unknown ? T[] : never

type Result = ToArray<string | number>
// Distributes to: ToArray<string> | ToArray<number>
// Result: string[] | number[]
```

### `infer` Keyword

Extract types within conditional types:

```lua
// Extract return type from function:
type ReturnType<T> = T extends (...args: unknown[]) -> infer R ? R : never

function getUser(): User
  return {id = 1, name = "User"}
end

type UserType = ReturnType<typeof getUser>  // User

// Extract parameter types:
type Parameters<T> = T extends (...args: infer P) -> unknown ? P : never

function createUser(name: string, age: number): User
  return {id = 1, name = name, age = age}
end

type Params = Parameters<typeof createUser>  // [string, number]
```

**Extract array element type:**
```lua
type ElementType<T> = T extends Array<infer E> ? E : T

type Num = ElementType<number[]>  // number
type Str = ElementType<string>    // string
```

**Extract from Promise-like (if we add promises):**
```lua
type Awaited<T> = T extends { then: (onfulfilled: (value: infer V) -> unknown) -> unknown } ? V : T
```

**Multiple infer locations:**
```lua
type FirstArg<T> = T extends (first: infer F, ...rest: unknown[]) -> unknown ? F : never

function doSomething(name: string, count: number): void
  -- ...
end

type First = FirstArg<typeof doSomething>  // string
```

### `typeof` Operator

Get the type of a value:

```lua
const config = {
  host = "localhost",
  port = 8080,
  ssl = true,
  options = {
    timeout = 30,
    retries = 3
  }
}

type Config = typeof config
// Result: {
//   host: string,
//   port: number,
//   ssl: boolean,
//   options: {
//     timeout: number,
//     retries: number
//   }
// }
```

**With functions:**
```lua
function createUser(name: string, age: number): User
  return {id = 1, name = name, age = age}
end

type CreateUserFn = typeof createUser
// Result: (name: string, age: number) -> User

type CreateUserReturn = ReturnType<typeof createUser>  // User
type CreateUserParams = Parameters<typeof createUser>  // [string, number]
```

**With enums:**
```lua
enum Status {
  Active = "active",
  Inactive = "inactive"
}

type StatusType = typeof Status
// Result: { Active: "active", Inactive: "inactive" }

type StatusValue = typeof Status.Active  // "active"
```

### Template Literal Types

String types constructed from other types:

```lua
type Greeting = `Hello ${string}`

const g1: Greeting = "Hello World"  // OK
const g2: Greeting = "Hi there"     // ERROR
```

**Combining literal types:**
```lua
type Color = "red" | "blue" | "green"
type Size = "small" | "large"

type Style = `${Color}-${Size}`
// Result: "red-small" | "red-large" | "blue-small" | 
//         "blue-large" | "green-small" | "green-large"
```

**Event handler pattern:**
```lua
type EventName = "click" | "focus" | "blur" | "change"

// Capitalize first letter:
type Capitalize<S extends string> = /* intrinsic */

type EventHandler = `on${Capitalize<EventName>}`
// Result: "onClick" | "onFocus" | "onBlur" | "onChange"

interface Props {
  onClick: (e: ClickEvent) -> void,
  onFocus: (e: FocusEvent) -> void,
  onBlur: (e: BlurEvent) -> void,
  onChange: (e: ChangeEvent) -> void
}
```

**Built-in string manipulation types:**
```lua
type Uppercase<S extends string> = /* intrinsic */
type Lowercase<S extends string> = /* intrinsic */
type Capitalize<S extends string> = /* intrinsic */
type Uncapitalize<S extends string> = /* intrinsic */

type A = Uppercase<"hello">       // "HELLO"
type B = Lowercase<"WORLD">       // "world"
type C = Capitalize<"typedLua">   // "TypedLua"
type D = Uncapitalize<"TypedLua"> // "typedLua"
```

### Advanced Patterns

#### Deep Partial

Make all properties optional recursively:

```lua
type DeepPartial<T> = {
  [K in keyof T]?: T[K] extends table ? DeepPartial<T[K]> : T[K]
}

interface Config {
  database: {
    host: string,
    port: number,
    credentials: {
      user: string,
      password: string
    }
  },
  cache: {
    enabled: boolean,
    ttl: number
  }
}

type PartialConfig = DeepPartial<Config>
// All properties at all levels are optional
```

#### Type-safe Event System

```lua
interface EventMap {
  click: { x: number, y: number },
  keypress: { key: string, ctrl: boolean },
  load: void,
  error: { message: string, code: number }
}

type EventHandler<K extends keyof EventMap> = 
  EventMap[K] extends void 
    ? () -> void 
    : (data: EventMap[K]) -> void

function on<K extends keyof EventMap>(event: K, handler: EventHandler<K>): void
  -- implementation
end

// Type-safe usage:
on("click", function(data)   -- data is { x: number, y: number }
  print(data.x, data.y)
end)

on("load", function()        -- No parameters for void events
  print("Loaded")
end)

on("error", function(data)   -- data is { message: string, code: number }
  print(data.message)
end)
```

#### Builder Pattern

```lua
interface User {
  id: number,
  name: string,
  email: string,
  age: number
}

type Builder<T, Set extends keyof T = never> = {
  [K in Exclude<keyof T, Set>]: (value: T[K]) -> Builder<T, Set | K>
} & {
  build: () -> Set extends keyof T ? T : never
}

// Usage ensures all required fields are set:
const user = userBuilder()
  .id(1)
  .name("Bob")
  .email("bob@example.com")
  .age(30)
  .build()  // Only callable when all fields are set
```

### Lua-Specific: Metatable Typing

Typing Lua metatables:

```lua
interface Vector {
  x: number,
  y: number
}

interface VectorMetatable {
  __add: (a: Vector, b: Vector) -> Vector,
  __mul: (a: Vector, b: number) -> Vector,
  __tostring: (v: Vector) -> string,
  __index: Vector
}

type VectorWithMeta = Vector & { __metatable: VectorMetatable }

const vectorMT: VectorMetatable = {
  __add = function(a, b)
    return {x = a.x + b.x, y = a.y + b.y}
  end,
  __mul = function(a, b)
    return {x = a.x * b, y = a.y * b}
  end,
  __tostring = function(v)
    return `Vector(${v.x}, ${v.y})`
  end
}

vectorMT.__index = vectorMT

function createVector(x: number, y: number): VectorWithMeta
  return setmetatable({x = x, y = y}, vectorMT)
end

const v1 = createVector(1, 2)
const v2 = createVector(3, 4)
const v3 = v1 + v2        // Type checker knows this returns Vector
const v4 = v1 * 2         // Type checker knows this returns Vector
print(tostring(v1))       // Type checker knows __tostring exists
```

---

## Object-Oriented Programming

TypedLua supports a complete object-oriented programming system that compiles to idiomatic Lua metatable patterns. OOP features can be disabled via configuration.

### Configuration

```json
{
  "compilerOptions": {
    "enableOOP": true  // Default: true
  }
}
```

**When `enableOOP: false`, the following are syntax errors:**
- `class`, `extends`, `implements`, `abstract`
- `public`, `private`, `protected`
- `static`, `super`
- `get`, `set`

### OOP Feature Set

1. **Classes** - Define object blueprints with fields and methods
2. **Inheritance** - `extends` keyword for class hierarchies
3. **Interface Implementation** - `implements` keyword for contracts
4. **Access Modifiers** - `public`, `private`, `protected` (compile-time enforcement)
5. **Abstract Classes** - `abstract` keyword for base classes
6. **Static Members** - Class-level fields and methods
7. **Getters/Setters** - Property accessors
8. **Super Calls** - Access parent class constructors and methods
9. **Property Initialization** - Inline field initialization

### Basic Classes

```lua
class User {
  // Properties
  id: number
  name: string
  email: string
  
  // Property with initialization
  status: string = "active"
  
  // Constructor
  constructor(id: number, name: string, email: string) {
    self.id = id
    self.name = name
    self.email = email
  }
  
  // Instance method
  greet(): string {
    return "Hello, " .. self.name
  }
  
  // Instance method with no return
  updateEmail(newEmail: string): void {
    self.email = newEmail
  }
}

// Usage:
const user = User.new(1, "Bob", "bob@example.com")
user:greet()  // "Hello, Bob"
user:updateEmail("newemail@example.com")
```

**Compiles to:**
```lua
local User = {}
User.__index = User

function User.new(id, name, email)
  local self = setmetatable({}, User)
  self.id = id
  self.name = name
  self.email = email
  self.status = "active"  -- Initialized property
  return self
end

function User:greet()
  return "Hello, " .. self.name
end

function User:updateEmail(newEmail)
  self.email = newEmail
end
```

### Inheritance

**Extending classes:**
```lua
class Admin extends User {
  role: string
  permissions: string[] = {}
  
  constructor(id: number, name: string, email: string, role: string) {
    super(id, name, email)  // Call parent constructor
    self.role = role
  }
  
  // Override parent method
  greet(): string {
    return "Admin " .. self.name
  }
  
  // New method
  addPermission(permission: string): void {
    table.insert(self.permissions, permission)
  }
}

const admin = Admin.new(1, "Alice", "alice@example.com", "superadmin")
admin:greet()  // "Admin Alice"
admin:addPermission("delete_users")
```

**Compiles to:**
```lua
local Admin = setmetatable({}, {__index = User})
Admin.__index = Admin

function Admin.new(id, name, email, role)
  local self = setmetatable({}, Admin)
  -- Call parent constructor
  User.new(self, id, name, email)
  self.role = role
  self.permissions = {}
  return self
end

function Admin:greet()
  return "Admin " .. self.name
end

function Admin:addPermission(permission)
  table.insert(self.permissions, permission)
end
```

**Super method calls:**
```lua
class Manager extends User {
  department: string
  
  constructor(id: number, name: string, email: string, department: string) {
    super(id, name, email)
    self.department = department
  }
  
  greet(): string {
    const base = super.greet()  // Call parent method
    return base .. " from " .. self.department
  }
}
```

**Compiles to:**
```lua
function Manager:greet()
  local base = User.greet(self)  -- Super method call
  return base .. " from " .. self.department
end
```

### Implementing Interfaces

```lua
interface Drawable {
  draw(): void
  getColor(): string
}

interface Resizable {
  resize(width: number, height: number): void
}

class Rectangle implements Drawable, Resizable {
  width: number
  height: number
  color: string
  
  constructor(width: number, height: number, color: string) {
    self.width = width
    self.height = height
    self.color = color
  }
  
  // Implement Drawable
  draw(): void {
    print("Drawing " .. self.color .. " rectangle")
  }
  
  getColor(): string {
    return self.color
  }
  
  // Implement Resizable
  resize(width: number, height: number): void {
    self.width = width
    self.height = height
  }
}
```

**Type checking:**
- Compiler verifies `Rectangle` implements all methods from `Drawable` and `Resizable`
- Interfaces are erased at compile time (no runtime cost)
- `Rectangle` is assignable to `Drawable` and `Resizable` types

### Access Modifiers

**Compile-time enforcement** (no runtime protection):

```lua
class BankAccount {
  public balance: number
  private pin: string
  protected accountNumber: string
  
  constructor(balance: number, pin: string) {
    self.balance = balance
    self.pin = pin
    self.accountNumber = generateAccountNumber()
  }
  
  public deposit(amount: number): void {
    self.balance = self.balance + amount
  }
  
  public withdraw(amount: number, inputPin: string): boolean {
    if self:validatePin(inputPin) then
      if self.balance >= amount then
        self.balance = self.balance - amount
        return true
      end
    end
    return false
  }
  
  private validatePin(inputPin: string): boolean {
    return self.pin == inputPin
  }
  
  protected getAccountNumber(): string {
    return self.accountNumber
  }
}

const account = BankAccount.new(1000, "1234")
account:deposit(500)         // OK - public
account.balance = 2000       // OK - public field
account.pin = "5678"         // ERROR - private field
account:validatePin("1234")  // ERROR - private method
```

**Access rules:**
- `public` - Accessible everywhere (default if not specified)
- `private` - Only accessible within the class
- `protected` - Accessible within the class and subclasses

**Inheritance and access:**
```lua
class SavingsAccount extends BankAccount {
  interestRate: number
  
  constructor(balance: number, pin: string, rate: number) {
    super(balance, pin)
    self.interestRate = rate
  }
  
  calculateInterest(): number {
    return self.balance * self.interestRate  // OK - public field
    // self.pin would be ERROR - private to parent
    const num = self:getAccountNumber()  // OK - protected method
  }
}
```

### Abstract Classes

```lua
abstract class Shape {
  color: string
  
  constructor(color: string) {
    self.color = color
  }
  
  // Abstract method - must be implemented by subclasses
  abstract area(): number
  abstract perimeter(): number
  
  // Concrete method - inherited by subclasses
  describe(): void {
    print(self.color .. " shape with area: " .. self:area())
  }
}

class Circle extends Shape {
  radius: number
  
  constructor(color: string, radius: number) {
    super(color)
    self.radius = radius
  }
  
  area(): number {
    return 3.14159 * self.radius * self.radius
  }
  
  perimeter(): number {
    return 2 * 3.14159 * self.radius
  }
}

class Rectangle extends Shape {
  width: number
  height: number
  
  constructor(color: string, width: number, height: number) {
    super(color)
    self.width = width
    self.height = height
  }
  
  area(): number {
    return self.width * self.height
  }
  
  perimeter(): number {
    return 2 * (self.width + self.height)
  }
}

// const shape = Shape.new("red")  // ERROR: Cannot instantiate abstract class
const circle = Circle.new("blue", 5)
circle:describe()  // "blue shape with area: 78.53975"
```

**Abstract class rules:**
- Cannot be instantiated directly
- Can have abstract methods (no implementation)
- Can have concrete methods (with implementation)
- Subclasses must implement all abstract methods

### Static Members

```lua
class MathUtils {
  static PI: number = 3.14159
  static readonly E: number = 2.71828
  
  static square(x: number): number {
    return x * x
  }
  
  static circleArea(radius: number): number {
    return MathUtils.PI * radius * radius
  }
}

// Usage - no instance needed:
const area = MathUtils.circleArea(5)
const sq = MathUtils.square(10)
print(MathUtils.PI)  // 3.14159
MathUtils.E = 3.0    // ERROR: Cannot assign to readonly
```

**Compiles to:**
```lua
local MathUtils = {}
MathUtils.PI = 3.14159
MathUtils.E = 2.71828

function MathUtils.square(x)  -- Uses . not :
  return x * x
end

function MathUtils.circleArea(radius)
  return MathUtils.PI * radius * radius
end
```

**Static and instance members together:**
```lua
class Counter {
  static totalCount: number = 0
  
  value: number
  
  constructor() {
    self.value = 0
    Counter.totalCount = Counter.totalCount + 1
  }
  
  increment(): void {
    self.value = self.value + 1
  }
  
  static getTotalCount(): number {
    return Counter.totalCount
  }
}

const c1 = Counter.new()
const c2 = Counter.new()
c1:increment()
print(Counter.getTotalCount())  // 2
```

### Getters and Setters

```lua
class Temperature {
  private celsius: number
  
  constructor(celsius: number) {
    self.celsius = celsius
  }
  
  get fahrenheit(): number {
    return self.celsius * 9/5 + 32
  }
  
  set fahrenheit(value: number) {
    self.celsius = (value - 32) * 5/9
  }
  
  get kelvin(): number {
    return self.celsius + 273.15
  }
  
  set kelvin(value: number) {
    self.celsius = value - 273.15
  }
}

const temp = Temperature.new(0)
print(temp.fahrenheit)  // 32 (calls getter)
temp.fahrenheit = 212   // Calls setter, celsius becomes 100
print(temp.celsius)     // 100
print(temp.kelvin)      // 373.15
```

**Compiles to:**
```lua
local Temperature = {}
Temperature.__index = Temperature

function Temperature.new(celsius)
  local self = setmetatable({}, Temperature)
  self.celsius = celsius
  return self
end

function Temperature:get_fahrenheit()
  return self.celsius * 9/5 + 32
end

function Temperature:set_fahrenheit(value)
  self.celsius = (value - 32) * 5/9
end

function Temperature:get_kelvin()
  return self.celsius + 273.15
end

function Temperature:set_kelvin(value)
  self.celsius = value - 273.15
end

-- Property access becomes method calls:
-- temp.fahrenheit     ->  temp:get_fahrenheit()
-- temp.fahrenheit = x ->  temp:set_fahrenheit(x)
```

**Readonly properties (getter only):**
```lua
class Circle {
  radius: number
  
  constructor(radius: number) {
    self.radius = radius
  }
  
  get area(): number {
    return 3.14159 * self.radius * self.radius
  }
  
  get diameter(): number {
    return self.radius * 2
  }
}

const circle = Circle.new(5)
print(circle.area)       // 78.53975 (getter)
circle.area = 100        // ERROR: No setter defined
```

### Method Syntax and Compilation

**Instance methods** compile to colon `:` syntax:
```lua
class User {
  greet(): void {
    print("Hello")
  }
}

// Compiles to:
function User:greet()
  print("Hello")
end

// Called as:
user:greet()
```

**Static methods** compile to dot `.` syntax:
```lua
class User {
  static create(): User {
    return User.new()
  }
}

// Compiles to:
function User.create()
  return User.new()
end

// Called as:
User.create()
```

### OOP Type System Integration

**Classes as types:**
```lua
class User {
  name: string
  constructor(name: string) {
    self.name = name
  }
}

function processUser(user: User): void {
  print(user.name)
}

const user = User.new("Bob")
processUser(user)  // OK
```

**Classes with generics:**
```lua
class Container<T> {
  private value: T
  
  constructor(value: T) {
    self.value = value
  }
  
  get(): T {
    return self.value
  }
  
  set(value: T): void {
    self.value = value
  }
}

const stringContainer = Container<string>.new("hello")
const num: number = stringContainer:get()  // ERROR: Type mismatch
```

**Structural typing still applies:**
```lua
interface HasName {
  name: string
}

class User {
  name: string
  age: number
  constructor(name: string, age: number) {
    self.name = name
    self.age = age
  }
}

function greet(obj: HasName): void {
  print("Hello, " .. obj.name)
}

const user = User.new("Bob", 30)
greet(user)  // OK - User structurally matches HasName
```

---

## Functional Programming

TypedLua supports functional programming features that enable expressive, composable code patterns. FP features can be disabled via configuration.

### Configuration

```json
{
  "compilerOptions": {
    "enableFP": true  // Default: true
  }
}
```

**When `enableFP: false`, the following are syntax errors:**
- `match` expressions
- Pipe operator `|>`
- Destructuring syntax `const [a, b] = arr` and `const {x, y} = obj`
- Spread operator `...`

### FP Feature Set

1. **Pattern Matching** - Match expressions with exhaustiveness checking
2. **Pipe Operator** - Chain function calls with `|>`
3. **Destructuring** - Array and object destructuring
4. **Spread Operator** - Spread arrays and objects
5. **Rest Parameters** - Collect remaining elements

### Pattern Matching

Match expressions for discriminated unions and value matching:

**Basic match:**
```lua
type Status = "active" | "inactive" | "pending" | "banned"

function getStatusMessage(status: Status): string
  return match status {
    "active" => "User is active",
    "inactive" => "User is inactive",
    "pending" => "Awaiting approval",
    "banned" => "User is banned"
  }
end
```

**Compiles to:**
```lua
function getStatusMessage(status)
  if status == "active" then
    return "User is active"
  elseif status == "inactive" then
    return "User is inactive"
  elseif status == "pending" then
    return "Awaiting approval"
  elseif status == "banned" then
    return "User is banned"
  end
end
```

**Discriminated unions:**
```lua
type Result<T> = 
  | { kind: "success", value: T }
  | { kind: "error", message: string }

function handleResult<T>(result: Result<T>): void
  match result {
    { kind: "success", value } => {
      print("Success: " .. tostring(value))
    },
    { kind: "error", message } => {
      print("Error: " .. message)
    }
  }
end

// Usage:
const result: Result<number> = {kind = "success", value = 42}
handleResult(result)  // "Success: 42"
```

**Compiles to:**
```lua
function handleResult(result)
  if result.kind == "success" then
    local value = result.value
    print("Success: " .. tostring(value))
  elseif result.kind == "error" then
    local message = result.message
    print("Error: " .. message)
  end
end
```

**Guards with `when`:**
```lua
function classifyNumber(n: number): string
  return match n {
    0 => "zero",
    n when n < 0 => "negative",
    n when n > 0 and n < 10 => "small positive",
    n when n >= 10 => "large positive",
    _ => "unknown"  // Wildcard/default
  }
end
```

**Compiles to:**
```lua
function classifyNumber(n)
  if n == 0 then
    return "zero"
  elseif n < 0 then
    return "negative"
  elseif n > 0 and n < 10 then
    return "small positive"
  elseif n >= 10 then
    return "large positive"
  else
    return "unknown"
  end
end
```

**Nested patterns:**
```lua
type Shape = 
  | { kind: "circle", radius: number }
  | { kind: "rectangle", width: number, height: number }
  | { kind: "point", x: number, y: number }

function describeShape(shape: Shape): string
  return match shape {
    { kind: "circle", radius } => `Circle with radius ${radius}`,
    { kind: "rectangle", width, height } => `Rectangle ${width}x${height}`,
    { kind: "point", x: 0, y: 0 } => "Origin point",
    { kind: "point", x, y } => `Point at (${x}, ${y})`,
    _ => "Unknown shape"
  }
end
```

**Exhaustiveness checking:**
```lua
type Color = "red" | "green" | "blue"

function getColorCode(color: Color): string
  return match color {
    "red" => "#FF0000",
    "green" => "#00FF00"
    // ERROR: Missing case for "blue"
  }
end
```

**Match expressions (not statements):**
```lua
// Can be used anywhere an expression is valid:
const message = match status {
  "active" => "Welcome!",
  _ => "Please wait"
}

const result = process(match value {
  0 => "zero",
  n => tostring(n)
})
```

### Pipe Operator

Chain function calls left-to-right with `|>`:

**Basic usage:**
```lua
const result = value
  |> validate
  |> transform
  |> save

// Equivalent to:
const result = save(transform(validate(value)))
```

**With arguments:**
```lua
const result = numbers
  |> filter((n) => n > 0)
  |> map((n) => n * 2)
  |> reduce((a, b) => a + b, 0)
```

**Compiles to:**
```lua
local result = reduce(
  map(
    filter(numbers, function(n) return n > 0 end),
    function(n) return n * 2 end
  ),
  function(a, b) return a + b end,
  0
)
```

**Type inference through pipes:**
```lua
function getLength(s: string): number
  return #s
end

function double(n: number): number
  return n * 2
end

const result = "hello"
  |> getLength   // number (5)
  |> double      // number (10)
  
// Type errors:
const bad = "hello"
  |> double  // ERROR: string is not assignable to number
```

**Method calls:**
```lua
const result = user
  |> .getName()      // Call method on piped value
  |> string.upper    // Lua standard library
  |> print           // Side effect
```

**Compiles to:**
```lua
local result = print(string.upper(user:getName()))
```

### Destructuring

#### Array Destructuring

**Basic array destructuring:**
```lua
const [x, y, z] = getCoordinates()

// With type annotation:
const [first, second]: [string, number] = getTuple()

// Skip elements:
const [first, , third] = {1, 2, 3}
```

**Compiles to:**
```lua
local _tmp = getCoordinates()
local x = _tmp[1]
local y = _tmp[2]
local z = _tmp[3]

local _tmp2 = getTuple()
local first = _tmp2[1]
local second = _tmp2[2]

local _tmp3 = {1, 2, 3}
local first = _tmp3[1]
local third = _tmp3[3]
```

**Rest elements:**
```lua
const [first, ...rest] = {1, 2, 3, 4, 5}
// first = 1
// rest = {2, 3, 4, 5}

const [head, ...tail] = items
```

**Compiles to:**
```lua
local _tmp = {1, 2, 3, 4, 5}
local first = _tmp[1]
local rest = {}
for i = 2, #_tmp do
  rest[#rest + 1] = _tmp[i]
end
```

#### Object Destructuring

**Basic object destructuring:**
```lua
const {name, age} = user

// With type annotation:
const {x, y}: Point = getPoint()

// Rename properties:
const {name: userName, age: userAge} = user
```

**Compiles to:**
```lua
local _tmp = user
local name = _tmp.name
local age = _tmp.age

local _tmp2 = getPoint()
local x = _tmp2.x
local y = _tmp2.y

local _tmp3 = user
local userName = _tmp3.name
local userAge = _tmp3.age
```

**Default values:**
```lua
const {name, age = 18} = user
// If user.age is nil, use 18

const [x = 0, y = 0] = getCoordinates()
```

**Compiles to:**
```lua
local _tmp = user
local name = _tmp.name
local age = _tmp.age ~= nil and _tmp.age or 18
```

**Nested destructuring:**
```lua
const {user: {name, age}, status} = response

const {metadata: {created, updated}} = document
```

**Compiles to:**
```lua
local _tmp = response
local _user = _tmp.user
local name = _user.name
local age = _user.age
local status = _tmp.status
```

#### Function Parameter Destructuring

**Array parameters:**
```lua
function distance([x1, y1]: [number, number], [x2, y2]: [number, number]): number
  return math.sqrt((x2 - x1)^2 + (y2 - y1)^2)
end

distance({0, 0}, {3, 4})  // 5.0
```

**Object parameters:**
```lua
function greet({name, age}: User): void
  print(`Hello ${name}, you are ${age} years old`)
end

interface Options {
  width: number,
  height: number,
  color?: string
}

function createBox({width, height, color = "red"}: Options): Box
  return {width = width, height = height, color = color}
end
```

**Compiles to:**
```lua
function greet(_arg1)
  local name = _arg1.name
  local age = _arg1.age
  print("Hello " .. name .. ", you are " .. tostring(age) .. " years old")
end

function createBox(_arg1)
  local width = _arg1.width
  local height = _arg1.height
  local color = _arg1.color ~= nil and _arg1.color or "red"
  return {width = width, height = height, color = color}
end
```

### Spread Operator

#### Array Spread

**Spread in array literals:**
```lua
const arr1 = {1, 2, 3}
const arr2 = {0, ...arr1, 4, 5}
// Result: {0, 1, 2, 3, 4, 5}

const combined = {...first, ...second, ...third}
```

**Compiles to:**
```lua
local arr1 = {1, 2, 3}
local arr2 = {0}
for i = 1, #arr1 do
  arr2[#arr2 + 1] = arr1[i]
end
arr2[#arr2 + 1] = 4
arr2[#arr2 + 1] = 5
```

#### Object Spread

**Spread in object literals:**
```lua
const point1 = {x = 1, y = 2}
const point2 = {...point1, z = 3}
// Result: {x = 1, y = 2, z = 3}

const updated = {...user, name = "NewName"}  // Override name

const merged = {...defaults, ...options}
```

**Compiles to:**
```lua
local point1 = {x = 1, y = 2}
local point2 = {}
for k, v in pairs(point1) do
  point2[k] = v
end
point2.z = 3

local updated = {}
for k, v in pairs(user) do
  updated[k] = v
end
updated.name = "NewName"
```

**Type safety:**
```lua
interface Point2D {
  x: number,
  y: number
}

interface Point3D extends Point2D {
  z: number
}

const point2d: Point2D = {x = 1, y = 2}
const point3d: Point3D = {...point2d, z = 3}  // OK
```

#### Function Spread

**Spread in function calls:**
```lua
function sum(a: number, b: number, c: number): number
  return a + b + c
end

const numbers = {1, 2, 3}
const result = sum(...numbers)  // 6
```

**Compiles to:**
```lua
function sum(a, b, c)
  return a + b + c
end

local numbers = {1, 2, 3}
local result = sum(table.unpack(numbers))  -- Lua 5.2+
-- Or: sum(unpack(numbers))  -- Lua 5.1
```

### Rest Parameters

Collect remaining function arguments:

```lua
function sum(...numbers: number[]): number
  local total = 0
  for i, n in ipairs(numbers) do
    total = total + n
  end
  return total
end

sum(1, 2, 3, 4, 5)  // 15

// With leading parameters:
function createLogger(prefix: string, ...messages: string[]): void
  for i, msg in ipairs(messages) do
    print(prefix .. ": " .. msg)
  end
end

createLogger("INFO", "Starting", "Processing", "Done")
```

**Compiles to:**
```lua
function sum(...)
  local numbers = {...}
  local total = 0
  for i, n in ipairs(numbers) do
    total = total + n
  end
  return total
end

function createLogger(prefix, ...)
  local messages = {...}
  for i, msg in ipairs(messages) do
    print(prefix .. ": " .. msg)
  end
end
```

### Combining FP Features

**Real-world example:**
```lua
type Result<T> = 
  | { kind: "success", value: T }
  | { kind: "error", message: string }

function processUsers(users: User[]): Result<ProcessedUser[]>
  const validated = users
    |> filter(isValid)
    |> map(normalize)
  
  if #validated == 0 then
    return {kind = "error", message = "No valid users"}
  end
  
  const processed = validated
    |> map((user) => match user.role {
      "admin" => {...user, permissions = adminPermissions},
      "user" => {...user, permissions = userPermissions},
      _ => user
    })
    |> sortBy((u) => u.name)
  
  return {kind = "success", value = processed}
end

// Usage:
const result = processUsers(allUsers)

match result {
  { kind: "success", value: users } => {
    for i, user in ipairs(users) do
      print(user.name)
    end
  },
  { kind: "error", message } => {
    print("Error: " .. message)
  }
}
```

### FP and Type System Integration

**Pattern matching with exhaustiveness:**
```lua
type Option<T> = T | nil

function getOrDefault<T>(option: Option<T>, defaultValue: T): T
  return match option {
    nil => defaultValue,
    value => value
  }
end
```

**Pipe operator preserves types:**
```lua
function toUpper(s: string): string
  return string.upper(s)
end

function getLength(s: string): number
  return #s
end

const length: number = "hello"
  |> toUpper    // string
  |> getLength  // number
```

**Destructuring with type inference:**
```lua
function getUserData(): [string, number, boolean]
  return {"Alice", 30, true}
end

const [name, age, active] = getUserData()
// name: string, age: number, active: boolean (all inferred)
```

---

## Decorators

TypedLua supports TC39 Stage 3 decorators for classes, methods, fields, and accessors. Decorators enable metaprogramming patterns like validation, logging, and aspect-oriented programming.

### Configuration

```json
{
  "compilerOptions": {
    "enableDecorators": true  // Default: true
  }
}
```

**When `enableDecorators: false`, the `@` decorator syntax is a syntax error.**

### Decorator Basics

Decorators are functions that modify or replace class elements. They follow the TC39 Stage 3 specification.

**Decorator signature:**
```lua
type DecoratorContext = {
  kind: "class" | "method" | "getter" | "setter" | "field" | "accessor",
  name: string | symbol,
  access: { get?: () -> unknown, set?: (value: unknown) -> void },
  static: boolean,
  private: boolean,
  addInitializer: (initializer: () -> void) -> void,
  metadata: table
}

type ClassDecorator<T> = (value: T, context: DecoratorContext) -> T | void
type MethodDecorator = (value: callable, context: DecoratorContext) -> callable | void
type FieldDecorator = (value: undefined, context: DecoratorContext) -> (initialValue: unknown) -> unknown | void
type AccessorDecorator = ({ get: callable, set: callable }, context: DecoratorContext) -> { get: callable, set: callable } | void
```

### Class Decorators

Decorate entire classes:

```lua
function logged<T>(value: T, context: DecoratorContext): T
  if context.kind == "class" then
    print("Class created: " .. context.name)
  end
  return value
end

@logged
class User {
  name: string
  constructor(name: string) {
    self.name = name
  }
}

// Output: "Class created: User"
```

**Replace class constructor:**
```lua
function withTimestamp<T>(value: T, context: DecoratorContext): T
  if context.kind ~= "class" then return value end
  
  // Return wrapper class
  return class extends value {
    created: number
    
    constructor(...args: unknown[]) {
      super(...args)
      self.created = os.time()
    }
  }
end

@withTimestamp
class User {
  name: string
  constructor(name: string) {
    self.name = name
  }
}

const user = User.new("Alice")
print(user.created)  // Timestamp
```

### Method Decorators

Decorate class methods:

```lua
function log(value: callable, context: DecoratorContext): callable
  const methodName = context.name
  return function(...args: unknown[]): unknown
    print(`Calling ${methodName} with args:`, args)
    const result = value(...args)
    print(`${methodName} returned:`, result)
    return result
  end
end

class Calculator {
  @log
  add(a: number, b: number): number {
    return a + b
  }
}

const calc = Calculator.new()
calc:add(2, 3)
// Output:
// Calling add with args: {2, 3}
// add returned: 5
```

**Memoization decorator:**
```lua
function memoize(value: callable, context: DecoratorContext): callable
  const cache: table = {}

  return function(...args: unknown[]): unknown
    const key = table.concat(args, ",")
    if cache[key] ~= nil then
      return cache[key]
    end

    const result = value(...args)
    cache[key] = result
    return result
  end
end

class Fibonacci {
  @memoize
  calculate(n: number): number {
    if n <= 1 then return n end
    return self:calculate(n - 1) + self:calculate(n - 2)
  }
}
```

**Validation decorator:**
```lua
function validate(schema: table) {
  return function(value: callable, context: DecoratorContext): callable
    return function(...args: unknown[]): unknown
      // Validate args against schema
      for i, arg in ipairs(args) do
        const expectedType = schema[i]
        if type(arg) ~= expectedType then
          error(`Argument ${i} must be ${expectedType}, got ${type(arg)}`)
        end
      end
      return value(...args)
    end
  end
}

class User {
  @validate({"string", "number"})
  create(name: string, age: number): User {
    return {name = name, age = age}
  }
}
```

### Field Decorators

Decorate class fields:

```lua
function default(defaultValue: unknown) {
  return function(value: undefined, context: DecoratorContext) {
    return function(initialValue: unknown): unknown
      return initialValue ~= nil and initialValue or defaultValue
    end
  end
}

class Config {
  @default("localhost")
  host: string
  
  @default(8080)
  port: number
  
  constructor(host?: string, port?: number) {
    self.host = host
    self.port = port
  }
}

const config = Config.new()
print(config.host)  // "localhost"
print(config.port)  // 8080
```

**Observable fields:**
```lua
function observable(value: undefined, context: DecoratorContext) {
  const fieldName = context.name
  const privateKey = `_${fieldName}`
  
  context.addInitializer(function()
    self.listeners = self.listeners or {}
    self.listeners[fieldName] = {}
  end)
  
  return function(initialValue: unknown): unknown
    // Store in private field
    self[privateKey] = initialValue
    
    // Create getter/setter
    const mt = getmetatable(self)
    const originalIndex = mt.__index
    
    mt.__index = function(t, k)
      if k == fieldName then
        return rawget(t, privateKey)
      end
      return originalIndex[k]
    end
    
    mt.__newindex = function(t, k, v)
      if k == fieldName then
        const oldValue = rawget(t, privateKey)
        rawset(t, privateKey, v)
        // Notify listeners
        for i, listener in ipairs(t.listeners[fieldName]) do
          listener(oldValue, v)
        end
      else
        rawset(t, k, v)
      end
    end
  end
}

class Model {
  @observable
  value: number
  
  constructor(value: number) {
    self.value = value
  }
  
  onChange(callback: (old: number, new: number) -> void): void {
    table.insert(self.listeners.value, callback)
  }
}

const model = Model.new(0)
model:onChange(function(old, new)
  print(`Changed from ${old} to ${new}`)
end)
model.value = 42  // Output: "Changed from 0 to 42"
```

### Accessor Decorators

Decorate getters and setters:

```lua
function validate(predicate: (value: unknown) -> boolean, message: string) {
  return function(value: { get: callable, set: callable }, context: DecoratorContext) {
    return {
      get = value.get,
      set = function(newValue: unknown): void
        if not predicate(newValue) then
          error(message)
        end
        value.set(newValue)
      end
    }
  end
}

class Person {
  private _age: number
  
  constructor(age: number) {
    self._age = age
  }
  
  @validate((v) => v >= 0 and v <= 150, "Age must be between 0 and 150")
  get age(): number {
    return self._age
  }
  
  set age(value: number) {
    self._age = value
  }
}

const person = Person.new(30)
person.age = 200  // ERROR: Age must be between 0 and 150
```

### Decorator Composition

Multiple decorators are applied bottom-to-top:

```lua
class User {
  @log
  @validate({"string"})
  @memoize
  processName(name: string): string {
    return string.upper(name)
  }
}

// Application order:
// 1. memoize (innermost)
// 2. validate
// 3. log (outermost)
```

### Decorator Factories

Decorators that accept parameters:

```lua
function deprecated(message: string) {
  return function(value: callable, context: DecoratorContext): callable
    return function(...args: unknown[]): unknown
      print(`WARNING: ${context.name} is deprecated. ${message}`)
      return value(...args)
    end
  end
}

function throttle(ms: number) {
  return function(value: callable, context: DecoratorContext): callable
    local lastCall = 0
    return function(...args: unknown[]): unknown
      const now = os.clock() * 1000
      if now - lastCall < ms then
        return nil
      end
      lastCall = now
      return value(...args)
    end
  end
}

class API {
  @deprecated("Use fetchUserV2 instead")
  fetchUser(id: number): User {
    // ...
  }
  
  @throttle(1000)  // Max once per second
  saveData(data: table): void {
    // ...
  }
}
```

### Metadata

Decorators can attach metadata:

```lua
function route(path: string) {
  return function(value: callable, context: DecoratorContext): callable
    context.metadata.routes = context.metadata.routes or {}
    table.insert(context.metadata.routes, {
      path = path,
      handler = context.name
    })
    return value
  end
}

class Controller {
  @route("/users")
  getUsers(): User[] {
    // ...
  }
  
  @route("/users/:id")
  getUser(id: number): User {
    // ...
  }
}

// Access metadata:
const routes = Controller[Symbol.metadata].routes
for i, route in ipairs(routes) do
  print(`${route.path} -> ${route.handler}`)
end
```

### Built-in Decorators

TypedLua provides some built-in decorators:

#### `@readonly`

Makes a field immutable after initialization:

```lua
class Config {
  @readonly
  apiKey: string
  
  constructor(apiKey: string) {
    self.apiKey = apiKey
  }
}

const config = Config.new("secret")
config.apiKey = "new"  // ERROR: Cannot assign to readonly field
```

#### `@sealed`

Prevents a class from being extended:

```lua
@sealed
class FinalClass {
  // ...
}

class Extended extends FinalClass {  // ERROR: Cannot extend sealed class
  // ...
}
```

#### `@deprecated`

Marks an element as deprecated:

```lua
class OldAPI {
  @deprecated("Use newMethod instead")
  oldMethod(): void {
    // ...
  }
}

api:oldMethod()  // WARNING: oldMethod is deprecated. Use newMethod instead
```

### Decorator Compilation

**TypedLua source:**
```lua
function log(value: callable, context: DecoratorContext): callable
  return function(...args: unknown[]): unknown
    print("Calling " .. context.name)
    return value(...args)
  end
end

class User {
  @log
  greet(name: string): string {
    return "Hello, " .. name
  }
}
```

**Compiles to:**
```lua
local function log(value, context)
  return function(...)
    print("Calling " .. context.name)
    return value(...)
  end
end

local User = {}
User.__index = User

function User.new()
  local self = setmetatable({}, User)
  return self
end

-- Apply decorator
local _greet = function(self, name)
  return "Hello, " .. name
end

User.greet = log(_greet, {
  kind = "method",
  name = "greet",
  static = false,
  private = false
})
```

### Type Safety

Decorators are fully typed:

```lua
// Decorator with type constraints
function validate<T extends (...args: unknown[]) -> unknown>(
  value: T,
  context: DecoratorContext
): T {
  // T preserves the original function signature
  return value
}

class Calculator {
  @validate
  add(a: number, b: number): number {
    return a + b
  }
}

const calc = Calculator.new()
const result: number = calc:add(1, 2)  // Type preserved
```

### Decorators and OOP

Decorators work seamlessly with OOP features:

```lua
abstract class Entity {
  @readonly
  id: number
  
  constructor(id: number) {
    self.id = id
  }
  
  @log
  abstract save(): void
}

class User extends Entity {
  name: string
  
  constructor(id: number, name: string) {
    super(id)
    self.name = name
  }
  
  @validate({"string"})
  save(): void {
    // Save to database
  }
}
```

### Limitations

**No parameter decorators** (removed in TC39 Stage 3):
```lua
class User {
  // Parameter decorators are NOT supported
  create(@validate name: string): User {  // ERROR
    // ...
  }
}
```

**Use method decorators instead:**
```lua
class User {
  @validate({"string"})
  create(name: string): User {
    // ...
  }
}
```

---

## Naming Conventions

### Built-in Types (lowercase)
```lua
nil, boolean, number, integer, string
unknown, never, void, table, coroutine
```

### User-defined Types/Interfaces (PascalCase)
```lua
interface User { ... }
type UserId = number
type Result = User | nil
```

**Enforcement:** Configurable via `strictNaming` option
- `"error"` - Compiler error if violated
- `"warn"` - Warning only
- `"off"` - No enforcement

---

## Compiler Configuration

Configuration file: `typedlua.json`

```json
{
  "compilerOptions": {
    "strictNullChecks": true,
    "strictNaming": "error",
    "noImplicitUnknown": true,
    "noExplicitUnknown": false,
    
    "outDir": "./dist",
    "removeComments": false,
    "sourceMap": true,
    
    "target": "lua5.4",
    
    "baseUrl": "./src",
    "paths": {
      "@/*": ["*"],
      "@/components/*": ["components/*"],
      "@/utils/*": ["utils/*"]
    },
    
    "enableOOP": true,
    "enableFP": true,
    "enableDecorators": true,
    
    "allowNonTypedLua": true
  },
  "include": ["src/**/*"],
  "exclude": ["dist"]
}
```

### Compiler Options

#### Type Checking Strictness

- **`strictNullChecks`** (boolean)
  - When `true`, `nil` is treated as a distinct type
  - Variables must explicitly allow `nil` via union types

- **`strictNaming`** (string: `"error"` | `"warn"` | `"off"`)
  - Enforces PascalCase for user-defined types/interfaces
  - Enforces lowercase for built-in types

- **`noImplicitUnknown`** (boolean)
  - When `true`, all variables/parameters must have explicit types
  - Prevents defaulting to `unknown`
  ```lua
  -- ERROR with noImplicitUnknown:
  local data = getValue()
  
  -- Must write:
  local data: User = getValue()
  ```

- **`noExplicitUnknown`** (boolean)
  - When `true`, cannot use `unknown` type in code
  - Forces proper typing with no escape hatches
  ```lua
  -- ERROR with noExplicitUnknown:
  local data: unknown = getValue()
  ```

#### Output Options

- **`outDir`** (string)
  - Directory where compiled `.lua` files are written
  - Default: same directory as source files

- **`removeComments`** (boolean)
  - Strip comments from compiled output
  - Default: `false`

- **`sourceMap`** (boolean)
  - Generate source maps for debugging
  - Maps compiled Lua back to TypedLua source
  - Default: `true`

#### Target

- **`target`** (string)
  - Lua version to target: `"lua5.1"`, `"lua5.2"`, `"lua5.3"`, `"lua5.4"`, `"luajit"`
  - Affects integer semantics and available features

#### Path Resolution

- **`baseUrl`** (string)
  - Base directory for resolving non-relative module names
  - Typically `"./src"`

- **`paths`** (object)
  - Path aliases to avoid relative path hell
  - Compiler resolves aliases during compilation
  ```lua
  -- With paths configured as above:
  local Button = require("@/components/button")
  
  -- Instead of:
  local Button = require("../../../../components/button")
  ```

#### Project Files

- **`include`** (array of strings)
  - Glob patterns for files to include in compilation
  - Example: `["src/**/*"]`

- **`exclude`** (array of strings)
  - Glob patterns for files to exclude
  - Example: `["node_modules", "dist", "**/*.test.tl"]`

#### Feature Toggles

- **`enableOOP`** (boolean)
  - Enable object-oriented programming features
  - Default: `true`
  - When `false`, `class`, `extends`, `implements`, etc. are syntax errors

- **`enableFP`** (boolean)
  - Enable functional programming features
  - Default: `true`
  - When `false`, `match`, `|>`, destructuring, spread are syntax errors

- **`enableDecorators`** (boolean)
  - Enable decorator syntax
  - Default: `true`
  - When `false`, `@decorator` syntax is a syntax error

#### Interoperability

- **`allowNonTypedLua`** (boolean)
  - Allow importing plain `.lua` files without type definitions
  - Default: `true`
  - Enables gradual migration from Lua to TypedLua

**When `true`:**
```lua
// my-module.lua (plain Lua, no types)
local M = {}

function M.hello()
  return "Hello"
end

return M

// main.tl (TypedLua)
import myModule from "./my-module"  // OK - myModule has type 'unknown'

const result = myModule.hello()  // Type is 'unknown'
```

**When `false`:**
```lua
// main.tl
import myModule from "./my-module"  // ERROR: No type definitions found
                                     // Create a .d.tl file or set allowNonTypedLua: true
```

**Best practice with `allowNonTypedLua: true`:**

Create `.d.tl` declaration files for type safety:

```lua
// my-module.d.tl
declare module "./my-module" {
  export function hello(): string
}

// main.tl
import myModule from "./my-module"  // Now fully typed
const result = myModule.hello()  // Type is 'string'
```

**Gradual migration strategy:**
1. Enable `allowNonTypedLua: true`
2. Import existing Lua files (typed as `unknown`)
3. Gradually add `.d.tl` files for critical modules
4. Rewrite modules to TypedLua when convenient
5. Once migration complete, set `allowNonTypedLua: false` for strictness

---

## Design Decisions Summary

### What Makes TypedLua Different from TypeScript

1. **No `any` type** - Forces explicit typing or use of `unknown`
2. **Clear type/interface distinction** - Eliminates confusion about when to use which
3. **Lua-flavored syntax** - Uses `->` for functions, stays close to Lua conventions
4. **Configurable strictness** - Each strict option is independent and explicit
5. **No OOP initially** - Focusing on functional/structural patterns first

### What Makes TypedLua Different from Existing Lua Type Systems

1. **TypeScript-inspired workflow** - Familiar for developers coming from TypeScript
2. **Gradual adoption** - Can mix typed and untyped code
3. **Strong tooling focus** - Source maps, path aliases, clear error messages
4. **Enforced conventions** - Configurable naming rules, strict type checking options

---

## Open Questions / Future Considerations

- Literal types implementation details
- Module system and external library type definitions
- Editor integration (LSP)
- Package manager integration (LuaRocks)
- Advanced type features (mapped types, conditional types, template literal types)
- Class/OOP support (Phase 2)
- Variance annotations for generics
- Type narrowing strategies (type guards, discriminated unions)

---

## Next Steps

1. Define complete grammar for TypedLua syntax
2. Design AST structure
3. Implement lexer/parser in Rust
4. Build type checker
5. Implement code generator (TypedLua → Lua)
6. Create CLI tool
7. Develop LSP for editor integration
8. Build standard library type definitions

---

**Document Version:** 0.1 (Initial Design)  
**Last Updated:** 2024-12-31
