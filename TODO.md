# TypedLua TODO

**Last Updated:** 2026-01-24 (Dead Store Elimination complete)

---

## P0: Language Features (Partially Implemented - Need Completion)

### 1.4 Null Coalescing Operator (`??`)

**Status:** IMPLEMENTED | **Model:** Sonnet

- [x] Add `NullCoalesce` variant to `BinaryOp` enum in ast/expression.rs
- [x] Lexer: Ensure `??` token exists (TokenKind::QuestionQuestion)
- [x] Parser: Parse `??` with correct precedence (lower than comparison, higher than `or`)
- [x] Parser: Map `TokenKind::QuestionQuestion` to `BinaryOp::NullCoalesce` in binary expression parsing
- [x] Type checker: Type left operand as any type, right operand compatible with non-nil version of left
- [x] Type checker: Result type is union of left (without nil) and right type
- [x] Codegen: Simple form `(a ~= nil and a or b)` for identifiers and simple member access
- [x] Codegen: IIFE form for complex expressions (avoid double evaluation)
- [x] Codegen: Handle member access correctly in simple form
- [x] Codegen: O2 optimization - skip nil check for guaranteed non-nil expressions (literals, objects, arrays, new expressions)
- [x] Remove `#[ignore]` from tests in null_coalescing_tests.rs
- [x] Remove `#[ignore]` from tests in null_coalescing_iife_tests.rs (removed cfg flag, O2 tests marked with #[ignore])
- [x] Fix and enable all tests

---

### 1.5 Safe Navigation Operator (`?.`)

**Status:** IMPLEMENTED | **Model:** Sonnet

**AST Changes:**

- [x] Add `OptionalMember` variant to `ExpressionKind` enum (object, property_name, span)
- [x] Add `OptionalIndex` variant to `ExpressionKind` enum (object, index, span)
- [x] Add `OptionalCall` variant to `ExpressionKind` enum (callee, arguments, span)
- [x] Add `OptionalMethodCall` variant to `ExpressionKind` enum (object, method_name, arguments, span)

**Lexer:**

- [x] Verify `TokenKind::QuestionDot` exists for `?.` token

**Parser:**

- [x] Parse `?.` as optional member access in postfix expression handling
- [x] Parse `?.[` as optional index access (check for `[` after `?.`)
- [x] Parse `?.identifier()` as optional method call
- [x] Parse `?.()` as optional function call
- [x] Handle precedence correctly (same as regular member access)

**Type Checker:**

- [x] Type `OptionalMember`: If receiver is `T | nil`, result is `PropertyType | nil`
- [x] Type `OptionalIndex`: If receiver is `T | nil`, result is `IndexedType | nil`
- [x] Type `OptionalCall`: If callee is `T | nil`, result is `ReturnType | nil`
- [x] Type `OptionalMethodCall`: Combine method lookup with optional receiver
- [x] Implement `make_optional_type()` helper for creating `T | nil` union types
- [x] Implement `infer_method_type()` for method call type inference

**Code Generation:**

- [x] Implement `is_simple_expression()` to determine if IIFE needed
- [x] Codegen for `OptionalMember`: Simple `and` chaining for short chains (1-2 levels)
- [x] Codegen for `OptionalMember`: IIFE form for long chains (3+ levels)
- [x] Codegen for `OptionalIndex`: Similar strategy (simple vs IIFE)
- [x] Codegen for `OptionalCall`: Handle nil-safe function calls
- [x] Codegen for `OptionalMethodCall`: Combine member access + call
- [x] Generate optimized code for all optional access patterns

**Testing:**

- [x] Remove `#![cfg(feature = "unimplemented")]` from safe_navigation_tests.rs
- [x] Fix and enable all tests (26 pass, 2 ignored for O2 optimization)

**Test file:** safe_navigation_tests.rs

**O2 Optimizations Deferred:**

- [ ] Codegen: O2 optimization - skip nil check for guaranteed non-nil expressions (literals, objects, arrays, new expressions)

---

### 1.6 Operator Overloading

**Status:** IMPLEMENTED | **Model:** Sonnet

Lexer keyword `Operator` exists. AST and parser complete, type checker and codegen in progress.

**AST Changes:**

- [x] Create `OperatorDeclaration` struct in AST (class_id, operator, parameters, body, span)
- [x] Create `OperatorKind` enum (Add, Sub, Mul, Div, Mod, Pow, Eq, Lt, Le, Concat, Len, Index, NewIndex, Call, Unm, Ne, Ge)
- [x] Add `Operator` variant to `ClassMember` enum

**Parser:**

- [x] Parse `operator` keyword in class body
- [x] Parse operator symbol after `operator`
- [x] Validate operator symbol against allowed set
- [x] Parse parameters for binary (1 param) vs unary (0 params) operators
- [x] Parse function body after parameters

**Type Checker - Signature Validation:**

- [x] Binary operators require exactly 1 parameter (right operand)
- [x] Unary operators require 0 parameters
- [x] `operator ==` and `operator ~=` must return `boolean`
- [x] `operator <`, `operator <=`, `operator >`, `operator >=` must return `boolean`
- [x] `operator and`, `operator or` disallowed (short-circuit semantics)

**Codegen - Arithmetic Operators:**

- [x] `__add` for `+` (number, string, custom)
- [x] `__sub` for `-`
- [x] `__mul` for `*`
- [x] `__div` for `/`
- [x] `__mod` for `%`
- [x] `__pow` for `^`

**Codegen - Comparison Operators:**

- [x] `__eq` for `==`
- [x] `__lt` for `<`
- [x] `__le` for `<=`

**Codegen - Index Operators:**

- [x] `__index` for `[]` access
- [x] `__newindex` for index assignment

**Codegen - Special Operators:**

- [x] `__concat` for `..`
- [x] `__unm` for unary `-`
- [x] `__len` for `#`
- [x] `__call` for `()` invocation

**Codegen - Integration:**

- [x] Generate metamethod table for class
- [x] Wire operators to metatable `__metatable` slot

**Testing:**

- [x] Fix remaining test failures:
  - [x] test_operator_unary_minus - Fixed type checker to recognize UnaryMinus in zero-parameter cases
  - [x] test_multiple_operators - Fixed parser's unary minus detection to use lookahead instead of consuming tokens prematurely

**Test file:** operator_overload_tests.rs (all 13 tests pass)

---

### 2.1 Exception Handling

**Status:** IMPLEMENTED | **Model:** Opus (complex feature)

Lexer keywords `Throw`, `Try`, `Catch`, `Finally`, `Rethrow`, `Throws`, `BangBang` exist and are now fully implemented.

#### 2.1.1 Exception AST Structures

- [x] Create `ThrowStatement` struct
- [x] Create `TryStatement` struct
- [x] Create `CatchClause` struct
- [x] Create `CatchPattern` enum (Untyped, Typed, MultiTyped, Destructured)
- [x] Create `TryExpression` struct
- [x] Create `ErrorChainExpression` struct for `!!`
- [x] Add `throws: Option<Vec<Type>>` to `FunctionDeclaration`
- [x] Add `throws: Option<Vec<Type>>` to `FunctionType`
- [x] Add `throws: Option<Vec<Type>>` to `DeclareFunctionStatement`

#### 2.1.2 Exception Parser

- [x] Parse `throw` statement
- [x] Parse `try`/`catch`/`finally` blocks
- [x] Parse catch patterns (simple, typed, multi-typed, destructured)
- [x] Parse `rethrow` statement
- [x] Parse `try ... catch ...` as expression
- [x] Parse `!!` operator
- [x] Parse `throws` clause on functions (with or without parens)

#### 2.1.3 Exception Type Checker

- [x] Type `throw` expression (any type)
- [x] Type catch blocks with declared types
- [x] Type try expression as union of try and catch results
- [x] Validate `rethrow` only in catch blocks
- [x] Track catch block nesting for rethrow validation

#### 2.1.4 Exception Codegen

- [x] Automatic pcall vs xpcall selection based on catch complexity
- [x] Simple catch → pcall (faster)
- [x] Typed/multi-catch → xpcall (full-featured)
- [x] Finally blocks with guaranteed execution
- [x] Try expressions → inline pcall
- [x] Error chaining `!!` operator

#### 2.1.5 Exception Tests

- [x] Fix exception_handling_tests.rs compilation (15 tests pass)
- [x] Fix exception_optimization_tests.rs compilation (8 tests pass)
- [x] Fix error_classes_tests.rs compilation (9 tests pass - all previously ignored tests now work)
- [x] Fix bang_operator_tests.rs compilation (7 tests pass)

**Test files:** exception_handling_tests.rs, exception_optimization_tests.rs, error_classes_tests.rs, bang_operator_tests.rs

---

### 2.2 Rich Enums (Java-style)

**Status:** IMPLEMENTED | **Model:** Sonnet

#### 2.2.1 Rich Enum AST Extensions

- [x] Create `EnumField` struct
- [x] Extend `EnumDeclaration` with fields, constructor, methods
- [x] Update `EnumMember` to include constructor arguments

#### 2.2.2 Rich Enum Parser

- [x] Parse enum members with constructor arguments syntax
- [x] Parse field declarations inside enum
- [x] Parse constructor inside enum
- [x] Parse methods inside enum

#### 2.2.3 Rich Enum Type Checker

- [x] Validate constructor parameters match field declarations
- [x] Validate enum member arguments match constructor signature
- [x] Type check methods with `self` bound to enum type
- [x] Auto-generate signatures for `name()`, `ordinal()`, `values()`, `valueOf()`

#### 2.2.4 Rich Enum Codegen

- [x] Generate enum constructor function
- [x] Generate enum instances as constants
- [x] Generate `name()` and `ordinal()` methods
- [x] Generate `values()` static method
- [x] Generate `valueOf()` with O(1) hash lookup
- [x] Generate static `__byName` lookup table
- [x] Generate custom enum methods
- [x] Prevent instantiation via metatable

#### 2.2.5 Rich Enum Tests

- [x] Fix rich_enum_tests.rs compilation (4 pass, 2 ignored for O2/O3 optimizations)

**Test file:** rich_enum_tests.rs

**Known Issues:**

- [ ] O2 optimization - precompute instances as literal tables (deferred)
- [ ] O3 optimization - add inline hints (deferred)

---

### 2.3 Interfaces with Default Implementations

**Status:** IMPLEMENTED | **Model:** Sonnet

#### 2.3.1 Interface Default Method AST

- [x] Add `body: Option<Block>` to `MethodSignature` struct (reuses existing struct rather than new enum variant)

#### 2.3.2 Interface Default Method Parser

- [x] Parse interface methods with `{` after signature as default methods
- [x] Parse interface methods without `{` as abstract methods
- [x] Properly consume `{` and `}` braces around method body

#### 2.3.3 Interface Default Method Type Checker

- [x] Track which methods are abstract vs default (via `body.is_some()`)
- [x] Error if abstract method not implemented
- [x] Allow default methods to be optional (use default if not overridden)
- [x] Type `self` in default methods as interface type
- [x] Resolve StringId values in error messages for readable output

#### 2.3.4 Interface Default Method Codegen

- [x] Generate interface default methods as `Interface__method(self, ...)` functions
- [x] Copy default implementations to implementing class: `User:method = User:method or Interface__method`

#### 2.3.5 Interface Default Method Tests

- [x] Fix interface_default_methods_tests.rs compilation (all 6 tests pass)

**Test file:** interface_default_methods_tests.rs

---

### 2.4 File-Based Namespaces

**Status:** IMPLEMENTED | **Model:** Sonnet

Lexer keyword `Namespace` exists (only `DeclareNamespaceStatement` for .d.tl files). File-scoped namespaces now fully implemented.

#### 2.4.1 Namespace AST & Parser

- [x] Add `NamespaceDeclaration` to `Statement` enum with path: `Vec<String>`
- [x] Parse `namespace Math.Vector;` at file start
- [x] Error if namespace appears after other statements
- [x] Only allow semicolon syntax (no block `{}` syntax)
- [x] Store namespace path in module metadata

#### 2.4.2 Namespace Type Checker

- [x] Track namespace for each module
- [x] Include namespace prefix when resolving imports
- [x] If `enforceNamespacePath: true`, verify namespace matches file path
- [x] Make namespace types accessible via dot notation

#### 2.4.3 Namespace Codegen

- [x] Generate nested table structure for namespace
- [x] Export namespace root table

#### 2.4.4 Namespace Config & Tests

- [x] Add `enforceNamespacePath` boolean option (default: false)
- [x] Fix namespace_tests.rs compilation (all 17 tests pass)

**Test file:** namespace_tests.rs

---

### 2.5 Template Literal Auto-Dedenting

**Status:** IMPLEMENTED | **Model:** Haiku

#### Template Dedenting Algorithm

- [x] Implement dedenting algorithm in codegen/mod.rs
- [x] Find minimum indentation across non-empty lines
- [x] Remove common indentation from each line
- [x] Preserve relative indentation within content
- [x] Handle edge cases: tabs vs spaces, first-line content, mixed indentation
- [x] Apply dedenting during codegen for template literal strings

#### Template Tests

- [x] Remove `#[cfg(feature = "unimplemented")]` from template_dedent_tests.rs
- [x] Fix template_dedent_tests.rs compilation (all 11 tests pass)

**Test file:** template_dedent_tests.rs

**Examples:**

- `const sql =`\n    SELECT *\n    FROM users\n`` → `"SELECT *\nFROM users"`
- Relative indentation preserved for nested content
- Empty/whitespace-only templates become empty strings

---

### 2.6 Reflection System

**Status:** IMPLEMENTED | **Model:** Sonnet (pure Lua, codegen-focused)

Pure Lua reflection via compile-time metadata generation. No native code or FFI required.

#### 2.6.1 Reflection Metadata Codegen

- [x] Assign unique `__typeId` (integer) to each class/interface/enum
- [x] Generate `__typeName` string on metatable
- [x] Generate `__ancestors` as hash set for O(1) lookup: `{ [ParentId] = true }`
- [x] Generate lazy `_buildAllFields()` - builds field table on first access, caches result
- [x] Generate lazy `_buildAllMethods()` - builds method table on first access, caches result
- [x] Field format: `{ name, type, modifiers }` (modifiers: `"readonly"`, `"optional"`, etc.)
- [x] Method format: `{ name, params, returnType }`
- [x] Generate `__ownFields` and `__ownMethods` arrays for own members only
- [x] Generate `__parent` reference for reflective parent access

#### 2.6.2 Reflection Runtime Module

- [x] `Reflect.isInstance(obj, Type)` - O(1) lookup: `obj.__ancestors[Type.__typeId]`
- [x] `Reflect.typeof(obj)` - returns `{ id, name, kind }` from metatable
- [x] `Reflect.getFields(obj)` - lazy: calls `_buildAllFields()` once, caches in `_allFieldsCache`
- [x] `Reflect.getMethods(obj)` - lazy: calls `_buildAllMethods()` once, caches in `_allMethodsCache`
- [x] `Reflect.forName(name)` - O(1) lookup in `__TypeRegistry`

#### 2.6.3 Reflection Integration

- [x] Track registered types in codegen context
- [x] Generate `__TypeRegistry` table: `{ ["MyClass"] = typeId, ... }`
- [x] Embed Reflect module at end of generated code

#### 2.6.5 Reflection Tests

- [x] Remove `#[cfg(feature = "unimplemented")]` from reflection_tests.rs
- [x] Test `isInstance()` with class hierarchies
- [x] Test `typeof()` returns correct metadata
- [x] Test `getFields()` and `getMethods()` accuracy
- [x] Test `forName()` lookup
- [x] Test ancestor chain merging in multi-level inheritance
- [x] Test caching behavior for lazy building functions

**Test file:** reflection_tests.rs (11 tests pass)

---

### 3.1-3.4 Compiler Optimizations

**Status:** O1 passes complete, O2 passes complete (8/8), O3 passes scaffolded | **Model:** Opus

All 16 optimization passes are registered. O1 passes (constant folding, dead code elimination, algebraic simplification) are fully functional. O2 passes: function inlining, loop optimization, null coalescing, safe navigation, exception handling, string concatenation, dead store elimination, tail call optimization, and rich enum optimization are complete. O3 passes are analysis-only placeholders.

**3.1 Optimization Infrastructure:**

- [x] Create `crates/typedlua-core/src/optimizer/mod.rs` module
- [x] Create `Optimizer` struct with optimization passes
- [x] Implement `OptimizationPass` trait
- [x] Add `OptimizationLevel` enum to config.rs (O0, O1, O2, O3, Auto)
- [x] Add `optimization_level: OptimizationLevel` to `CompilerOptions`
- [x] Add `with_optimization_level()` method to `CodeGenerator`
- [x] Integrate optimizer into compilation pipeline
- [x] Fixed-point iteration (runs passes until no changes)
- [x] Level-based pass filtering (only runs passes <= current level)
- [x] Auto optimization level support (O1 in debug, O2 in release)

**3.2 O1 Optimizations - Basic (COMPLETE):**

- [x] Constant folding (numeric + boolean expressions)
- [x] Dead code elimination (after return/break/continue)
- [x] Algebraic simplification (x+0=x, x*1=x, x*0=0, etc.)
- [x] Table pre-allocation (adds table.create() hints for Lua 5.2+)
- [x] Global localization - caches frequently-used globals in local variables

**3.3 O2 Optimizations - Standard (COMPLETE - 6/6 passes):**

### 3.3 O2 Optimizations - Standard (COMPLETE - 6/6 passes)

- [x] Function inlining
  - [x] Define inlining policy (size thresholds: 5, 10 statements; recursion safety rules)
  - [x] Implement candidate discovery pass (scan call graph, record call‑site info)
  - [x] Create transformation that clones function body into caller (handling locals, return)
  - [x] Handle inlining of functions with upvalues / closures (skip or special case)
  - [x] Register new `FunctionInliningPass` in optimizer infrastructure
  - [x] **BLOCKING:** Fix StringInterner sharing between CodeGenerator and Optimizer (IN PROGRESS)
    - [x] Add `use std::sync::Arc` and `use crate::string_interner::StringId` imports to codegen/mod.rs
    - [x] Remove lifetime `'a` from `impl<'a> CodeGenerator<'a>` → `impl CodeGenerator`
    - [x] Change `CodeGenerator::new()` to accept `Arc<StringInterner>` instead of `&StringInterner`
    - [x] Add `optimization_level: OptimizationLevel` field and `with_optimization_level()` method
    - [x] Integrate `Optimizer::new()` in `generate()` using `self.interner.clone()`
    - [x] Update `crates/typedlua-core/src/codegen/mod.rs` - internal test helpers
    - [x] Update `crates/typedlua-cli/src/main.rs` - CLI entry point
    - [x] Update remaining test files
  - [x] Fix borrow checker error in `generate_bundle()` - Program parameter comes as `&Program` but `generate()` now requires `&mut Program`
  - [x] Write unit tests: simple pure function, function with parameters, recursive guard, closure edge case

- [x] Loop optimization
  - [x] Detect loop‑invariant expressions (constant folding inside loops)
  - [x] Add pass to hoist invariant statements before loop header (conservative: locals with invariant initializers)
  - [x] Implement optional loop unrolling for `for` loops with known small iteration count - DEFERRED (LuaJIT handles this)
  - [x] Add pass to simplify loop conditions (dead loop body clearing at O2)
  - [x] Handle repeat...until loops (previously missing in optimizer)
  - [x] Write tests covering invariant detection, dead loop removal, and repeat support

- [x] Null coalescing optimization (IMPLEMENTED)
  - [x] Add `is_guaranteed_non_nil()` helper in codegen
  - [x] O2 optimization: skip nil check for literals (number, string, boolean)
  - [x] O2 optimization: skip nil check for object/array literals
  - [x] O2 optimization: skip nil check for new expressions and function expressions
  - [x] Enable all 6 O2 tests in null_coalescing_iife_tests.rs

- [x] Safe navigation optimization
  - [x] Identify optional access chains in AST (`OptionalMember`, `OptionalIndex`, `OptionalCall`, `OptionalMethodCall`)
  - [x] Determine chain length and side‑effect complexity
  - [x] Emit chained `and` checks for short chains (1‑2 levels)
  - [x] Generate IIFE for longer or side‑effecting chains
  - [x] Add tests for various optional navigation patterns
  - [x] O2 optimization - skip nil check for guaranteed non-nil expressions (literals, objects, arrays, new expressions)

- [x] Exception handling optimization
  - [x] Benchmark typical `try/catch` patterns using `pcall` vs `xpcall`
  - [x] Add analysis to select `pcall` when catch block is a single simple handler (O0/O1)
  - [x] Keep `xpcall` for multi‑catch or rethrow scenarios
  - [x] Update codegen to emit chosen wrapper
  - [x] Write tests for simple try/catch (pcall) and complex (xpcall) cases
  - [x] O2/O3 optimization: Use `xpcall` with `debug.traceback` for better stack traces
  - [x] O2/O3 optimization: Skip type checking handler for untyped catches (use debug.traceback directly)

- [x] String concatenation optimization
  - [x] Detect consecutive `..` operations
  - [x] Transform to `table.concat({a, b, c})` for 3+ parts
  - [x] Handle nested concatenations and parentheses
  - [ ] Loop-based concatenation optimization (DEFERRED - requires block transformation)

- [x] Dead store elimination
  - [x] Perform liveness analysis on local variables within basic blocks
  - [x] Flag assignments whose values are never read before being overwritten or out of scope
  - [x] Remove flagged store instructions in a dedicated pass
  - [x] Verify correctness with tests ensuring no observable side‑effects removed
  - [x] Handle nested function bodies and arrow functions recursively
  - [x] Preserve variables captured by closures

- [ ] Method to function call conversion (O2)
  - [ ] PHASE 1: Add type annotation storage to AST
    - [ ] Add `annotated_type: Option<Type>` field to `Expression` struct in ast/expression.rs
    - [ ] Update `Expression::new()` to initialize `annotated_type: None`
    - [ ] Update all 65 Expression struct construction sites across 4 files:
      - [ ] parser/expression.rs (~42 sites)
      - [ ] parser/statement.rs (~5 sites)
      - [ ] typechecker/narrowing_integration.rs (~12 sites)
      - [ ] optimizer/passes.rs (~6 sites)
    - [ ] Verify all tests pass after AST change

  - [ ] PHASE 2: Populate type annotations in type checker
    - [ ] Store type inference result in `Expression::annotated_type` for all expressions
    - [ ] Handle OptionalMethodCall to use `infer_method_type()` instead of Unknown
    - [ ] Add handling for regular MethodCall to use `infer_method_type()`

  - [ ] PHASE 3: Create MethodToFunctionConversionPass
    - [ ] Create pass struct in optimizer/passes.rs
    - [ ] Implement `OptimizationPass` trait (name, min_level, run)
    - [ ] Implement visitor that scans for MethodCall with known receiver type
    - [ ] Transform MethodCall → Call with direct class.method invocation
    - [ ] Handle receiver expressions:
      - [ ] New expressions (always known type)
      - [ ] Identifier known to be a class name
      - [ ] Member access on known class type
      - [ ] Optional chaining (preserve optional semantics)

  - [ ] PHASE 4: Register and test
    - [ ] Register pass in optimizer/mod.rs for O2 level
    - [ ] Add tests in tests/method_to_function_tests.rs:
      - [ ] Test new expression conversion
      - [ ] Test class identifier conversion
      - [ ] Test chained method calls
      - [ ] Test optional method calls (should not convert)
      - [ ] Test static method calls (should convert)
      - [ ] Test preservation of argument evaluation order
    - [ ] Run full test suite to verify no regressions

  - [x] Tail call optimization
    - [x] Review Lua runtime tail‑call behavior for generated functions
    - [x] Ensure optimizer does not insert statements that break tail‑position
    - [x] Add a pass that verifies tail‑call positions remain unchanged after other optimizations
  - [x] Write tests for tail‑recursive functions and non‑tail calls
    - [x] Test file: tail_call_optimization_tests.rs (21 tests pass)

  - [x] Rich enum optimization (COMPLETE)
    - [x] PHASE 1: Create RichEnumOptimizationPass
      - [x] Create `crates/typedlua-core/src/optimizer/rich_enum_optimization.rs`
      - [x] Implement `OptimizationPass` trait (name, min_level=O2, run)
      - [x] Define pass struct `RichEnumOptimizationPass`
      - [x] Register pass in optimizer/mod.rs after FunctionInliningPass

    - [x] PHASE 2: Instance Table Precomputation (O2)
      - [x] Transform enum member declarations from constructor calls to literal tables
      - [x] Before: `Planet.Mercury = Planet__new("Mercury", 0, mass, radius)`
      - [x] After: `Planet.Mercury = setmetatable({ __name = "Mercury", __ordinal = 0, mass = mass, radius = radius }, Planet)`
      - [x] Keep Planet__new function for potential runtime instantiation
      - [x] Populate __values array with pre-created instances

    - [x] PHASE 3: Inline Hints for Simple Methods (O2)
      - [x] Implement `is_simple_method()` helper to detect single-return methods
      - [x] Add `-- @inline` comment before qualifying methods (deferred - see note)
      - [x] Simple method criteria: single return statement, no function calls, no control flow

    - [x] PHASE 4: Override Rule Preservation
      - [x] Track which methods are safe to inline (not overridable)
      - [x] Skip inlining for methods that access mutable state modified by overrides
      - [x] Preserve method table lookup semantics for potentially overridden methods

    - [x] PHASE 5: Enable Tests
      - [x] Remove `#[ignore]` from `test_o2_optimization_precomputes_instances`
      - [x] Remove `#[ignore]` from `test_o3_optimization_adds_inline_hints`
      - [x] Verify all 6 rich_enum_tests.rs tests pass

    - [x] Files Modified:
      - [x] `optimizer/rich_enum_optimization.rs` (NEW)
      - [x] `optimizer/mod.rs` (register pass)
      - [x] `codegen/mod.rs` (add O2 instance precomputation)
      - [x] `tests/rich_enum_tests.rs` (enable tests)

**Note:** Inline hints (`-- @inline` comments) are deferred as Lua interpreters don't standardly support them. The O2 optimization focuses on precomputing instances as literal tables.

### 3.4 O3 Optimizations - Aggressive (SCAFFOLDED)

- [ ] Devirtualization
  - [ ] Identify virtual method call sites (method tables)
  - [ ] Analyze concrete type information at call sites
  - [ ] Replace virtual dispatch with direct function call where type is known
  - [ ] Add tests for class hierarchy method calls

- [ ] Generic specialization
  - [ ] Detect generic function instantiations with concrete type arguments
  - [ ] Generate specialized monomorphic versions of generic functions
  - [ ] Inline specialized versions where beneficial
  - [ ] Write tests for generic function performance and correctness

- [ ] Operator inlining
  - [ ] Identify frequently used operator overloads
  - [ ] Inline operator body at call sites when small
  - [ ] Ensure correct handling of metamethod lookups
  - [ ] Add tests for operator inlining correctness

- [ ] Interface method inlining
  - [ ] Analyze interface method calls with known implementing class
  - [ ] Inline method body when implementation is known and small
  - [ ] Preserve semantics for dynamic dispatch fallback
  - [ ] Add tests for interface method inlining scenarios
  
- [ ] Aggressive inlining (threshold: 15 statements)
  - [ ] Extend inlining criteria to larger functions up to 15 statements
  - [ ] Implement heuristics to avoid code bloat
  - [ ] Add benchmarks to evaluate impact
  - [ ] Write tests for large function inlining and recursion safety

**Test files:** optimizer_integration_tests.rs, o1_combined_tests.rs, o3_combined_tests.rs, dead_store_elimination_tests.rs (19 tests)

---

## P1: Core Infrastructure

### Create typedlua-runtime Crate

**Status:** Not Started | **Expected:** Better modularity, testability, versioning | **Model:** Sonnet

TypedLua extends Lua with many features not in base Lua (classes, decorators, exceptions, rich enums, etc.). Each requires runtime support code embedded in generated Lua. Currently scattered in codegen - needs dedicated crate.

#### Runtime Crate Setup

- [ ] Create `crates/typedlua-runtime/` with lib.rs
- [ ] Add modules: classes.rs, decorators.rs, exceptions.rs, operators.rs, enums.rs, reflection.rs
- [ ] Create `lua/` directory for Lua source snippets

#### Runtime Lua Snippets

- [ ] Extract decorator runtime to `lua/decorator_runtime.lua`
- [ ] Create class system runtime in `lua/class_runtime.lua`
- [ ] Create exception helpers in `lua/exception_runtime.lua` (pcall/xpcall wrappers)
- [ ] Create operator helpers in `lua/operator_helpers.lua` (safe nav, null coalesce)
- [ ] Create enum runtime in `lua/enum_runtime.lua`
- [ ] Create reflection runtime in `lua/reflection_runtime.lua`

#### Runtime Const String Exports

- [ ] Use `include_str!` to embed Lua snippets as const strings
- [ ] Export one const per feature (e.g., `pub const DECORATOR_RUNTIME: &str`)
- [ ] Version snippets per Lua target (5.1, 5.2, 5.3, 5.4) where needed

#### Runtime Integration

- [ ] Add `typedlua-runtime` dependency to `typedlua-core`
- [ ] Update codegen to import runtime constants
- [ ] Track which features are used (uses_decorators, uses_exceptions, etc.)
- [ ] Only embed runtime for features actually used in compiled code

#### Runtime Testing

- [ ] Unit test each Lua snippet independently
- [ ] Integration tests with codegen

---

### Lua Target Strategy Pattern

**Status:** Not Started | **Expected:** Better maintainability, easier to add versions | **Model:** Sonnet

Current approach: capability checks scattered in codegen (`supports_bitwise_ops()`, `supports_goto()`). Doesn't scale well.

#### Strategy Trait Definition

- [ ] Create `crates/typedlua-core/src/codegen/strategies/mod.rs`
- [ ] Define `CodeGenStrategy` trait with methods:
  - `generate_bitwise_op(&self, op, lhs, rhs) -> String`
  - `generate_integer_divide(&self, lhs, rhs) -> String`
  - `generate_continue(&self, label) -> String`
  - `emit_preamble(&self) -> Option<String>` (for library includes)

#### Strategy Implementations

- [ ] Create `strategies/lua51.rs` implementing `CodeGenStrategy`
- [ ] Create `strategies/lua52.rs` implementing `CodeGenStrategy`
- [ ] Create `strategies/lua53.rs` implementing `CodeGenStrategy`
- [ ] Create `strategies/lua54.rs` implementing `CodeGenStrategy`

#### Strategy Integration

- [ ] Add `strategy: Box<dyn CodeGenStrategy>` field to `CodeGenerator`
- [ ] Select strategy based on `LuaTarget` during initialization
- [ ] Replace conditional logic in codegen with strategy method calls
- [ ] Remove `supports_*` methods from `LuaTarget` (logic now in strategies)

#### Strategy Testing

- [ ] Unit test each strategy independently
- [ ] Regression tests for version-specific output

---

### Code Generator Modularization

**Status:** Not Started | **Expected:** 50%+ maintainability improvement | **Model:** Sonnet

CodeGenerator is 3,120 lines - too large. Break into focused modules.

#### Codegen Directory Structure

- [ ] Create `crates/typedlua-core/src/codegen/strategies/` (Lua version strategies)
- [ ] Create `crates/typedlua-core/src/codegen/emitters/` (AST → Lua emitters)
- [ ] Create `crates/typedlua-core/src/codegen/transforms/` (pluggable transforms)

#### Codegen Emitters

- [ ] Extract expression generation to `emitters/expressions.rs`
- [ ] Extract statement generation to `emitters/statements.rs`
- [ ] Extract type erasure to `emitters/types.rs`
- [ ] Main codegen becomes orchestrator (~300 lines)

#### Codegen Transforms

- [ ] Define `CodeGenTransform` trait (like `OptimizationPass`)
- [ ] Create `transforms/classes.rs` for class → table transformation
- [ ] Create `transforms/decorators.rs` for decorator emission
- [ ] Create `transforms/modules.rs` for import/export handling
- [ ] Move sourcemap logic to `transforms/sourcemaps.rs`

#### Codegen Integration

- [ ] Register transforms in CodeGenerator::new()
- [ ] Run transforms in pipeline during generation
- [ ] Each transform testable in isolation

---

### Type Checker Visitor Pattern

**Status:** Not Started | **Expected:** Better separation of concerns | **Model:** Sonnet

Type checker is 3,544 lines. Extract specialized visitors for different concerns.

#### Visitor Trait Definition

- [ ] Create `crates/typedlua-core/src/typechecker/visitors/mod.rs`
- [ ] Define `TypeCheckVisitor` trait with visit methods

#### Specialized Visitors

- [ ] Create `visitors/narrowing.rs` - Type narrowing logic
- [ ] Create `visitors/generics.rs` - Generic instantiation and constraints
- [ ] Create `visitors/access_control.rs` - public/private/protected checks
- [ ] Create `visitors/inference.rs` - Type inference rules

#### Visitor Integration

- [ ] Main TypeChecker orchestrates visitors
- [ ] Each visitor testable independently
- [ ] Clear separation of type system concerns

---

### Builder Pattern for CodeGenerator

**Status:** Not Started | **Expected:** Better testability, clearer API | **Model:** Haiku

Current constructor only takes `StringInterner`. Builder pattern for complex setup.

**Implementation:**

- [ ] Create `CodeGeneratorBuilder` struct
- [ ] Methods: `interner()`, `target()`, `strategy()`, `enable_sourcemaps()`, `bundle_mode()`, etc.
- [ ] `build()` returns configured `CodeGenerator`

**Benefits:**

- [ ] Clear configuration interface
- [ ] Easier partial configuration in tests
- [ ] Self-documenting API

---

### Arena Allocation Integration

**Status:** Not Started | **Expected:** 15-20% parsing speedup | **Model:** Sonnet

Infrastructure exists at `arena.rs` (bumpalo). Currently only used in tests.

- [ ] Thread `&'arena Arena` lifetime through parser
- [ ] Change `Box<Statement>` → `&'arena Statement`
- [ ] Change `Box<Expression>` → `&'arena Expression`
- [ ] Change `Box<Type>` → `&'arena Type`
- [ ] Replace `Box::new(...)` with `arena.alloc(...)`
- [ ] Create arena at compilation entry, pass through pipeline
- [ ] Update type checker for arena-allocated AST
- [ ] Benchmark before/after

---

### salsa Framework Integration

**Status:** Not Started | **Expected:** 10-50x LSP speedup | **Model:** Opus (complex framework integration)

Fine-grained incremental compilation. Replaces manual caching.

**Phase 1: Database Setup**

- [ ] Add `salsa = "0.17"` to Cargo.toml
- [ ] Create db module with inputs and queries
- [ ] Define `#[salsa::input]` for source files
- [ ] Define `#[salsa::tracked]` for parse/type_check

**Phase 2: Integration**

- [ ] Modify lexer/parser/checker for salsa
- [ ] Integrate with CLI
- [ ] Integrate with LSP

**Phase 3: Fine-Grained Queries**

- [ ] symbol_at_position, type_of_symbol, references_to_symbol
- [ ] Sub-file invalidation

---

### id-arena Integration

**Status:** Not Started | **Expected:** Cleaner graph structures | **Model:** Sonnet

Integrate during salsa work. Eliminates lifetime issues in type checker and module graph.

- [ ] Use id-arena for type checker graph
- [ ] Use id-arena for module graph
- [ ] Replace `Box<Expression>` / `Box<Statement>` with `ExpressionId` / `StatementId`
- [ ] Update serialization to use IDs

---

### Inline Annotations

**Status:** Not Started | **Expected:** 5-10% speedup | **Model:** Haiku (simple annotations)

- [ ] Add `#[inline]` to span.rs methods
- [ ] Add `#[inline]` to parser helpers (`check()`, `match_token()`, `peek()`)
- [ ] Add `#[inline]` to type checker hot paths
- [ ] Profile with cargo flamegraph

---

### Security & CI

**Model:** Haiku (configuration tasks)

**cargo-deny:**

- [ ] Create deny.toml
- [ ] Add `cargo deny check` to CI

**miri:**

- [ ] Add miri CI job (nightly schedule)

**Fuzzing:**

- [ ] Initialize fuzz directory
- [ ] Create lexer fuzz target
- [ ] Create parser fuzz target
- [ ] Add CI job for continuous fuzzing

**Benchmarks CI:**

- [ ] Add benchmark regression detection to CI

---

## P2: Quality of Life

### indexmap for Deterministic Ordering

**Model:** Haiku (simple replacements)

- [ ] Replace LSP symbol tables with IndexMap
- [ ] Use IndexMap for diagnostic collection
- [ ] Use IndexMap for export tables
- [ ] Keep FxHashMap for internal structures

---

### Cow for Error Messages

**Model:** Haiku (simple optimization)

- [ ] Change diagnostic messages to use `Cow<'static, str>`
- [ ] Apply to parser, type checker, type display

---

### Index-Based Module Graph

**Model:** Sonnet (refactoring)

- [ ] Create ModuleId as usize wrapper
- [ ] Store modules in `Vec<Module>`
- [ ] Change dependencies to `Vec<ModuleId>`

---

### insta Snapshot Testing Expansion

**Model:** Haiku (test conversions)

- [ ] Convert parser tests to snapshots
- [ ] Convert type checker tests to snapshots
- [ ] Convert codegen tests to snapshots

---

### proptest Property Testing

**Model:** Sonnet (property design)

- [ ] Parser round-trip property
- [ ] Type checker soundness properties
- [ ] Codegen correctness properties

---

## P3: Polish

### Output Format Options

- [ ] Add output.format config (readable | compact | minified)
- [ ] Implement compact mode
- [ ] Implement minified mode with sourcemaps
- [ ] Document bytecode compilation with `luajit -b`

---

### Code Style Consistency

- [ ] Replace imperative Vec building with iterators where appropriate
- [ ] Use `.fold()` / `.flat_map()` patterns

---

## P4: Testing & Documentation

### Integration Tests

- [ ] Test all features combined
- [ ] Test feature interactions
- [ ] Test edge cases and error conditions
- [ ] Performance regression tests

---

### Documentation

- [ ] Update language reference
- [ ] Create tutorial for each major feature
- [ ] Document optimization levels
- [ ] Create migration guide from plain Lua
- [ ] Update README with feature showcase

---

### Publishing

- [ ] Publish VS Code extension to marketplace

---

## Completed

### Performance Measurement Baseline ✓

**Criterion benchmarks:** Lexer 7.8M tokens/sec, Parser 930K statements/sec, Type checker ~1.4µs/statement

**dhat profiling:** 23.5 MB total, 1.38 MB peak, 131k allocations

See `BENCHMARKS.md` for details.

### Dependencies Added ✓

indoc, criterion, dhat, proptest, cargo-fuzz, insta — all in Cargo.toml

---

## Implementation Details

### Function Inlining (Section 3.3) - IN PROGRESS

**Status:** Core implementation complete, blocked by StringInterner sharing issue

**Root Cause:**
The optimizer's FunctionInliningPass creates new StringIds via `interner.get_or_intern()` for temp variables (`_inline_result_0`, etc.). CodeGenerator must resolve these IDs during code generation. Currently, CodeGenerator and Optimizer use separate interner instances, so IDs created by the optimizer are not resolvable by codegen.

**Solution:** Share a single `Arc<StringInterner>` between CodeGenerator and Optimizer.

---

#### Phase 1: Fix CodeGenerator to use `Arc<StringInterner>`

**File:** `crates/typedlua-core/src/codegen/mod.rs`

The struct field was already changed to `Arc<StringInterner>`, but the impl block is inconsistent:

- [ ] Add missing imports at top of file:

```rust
use std::sync::Arc;
use crate::string_interner::StringId;
```

- [ ] Remove lifetime from impl block (line ~152):

```rust
// Before: impl<'a> CodeGenerator<'a>
// After:  impl CodeGenerator
```

- [ ] Update `new()` signature to accept Arc:

```rust
pub fn new(interner: Arc<StringInterner>) -> Self
```

- [ ] Add `optimization_level` field to struct:

```rust
optimization_level: OptimizationLevel,
```

- [ ] Add `with_optimization_level()` builder method:

```rust
pub fn with_optimization_level(mut self, level: OptimizationLevel) -> Self {
    self.optimization_level = level;
    self
}
```

- [ ] Integrate optimizer into `generate()` method:

```rust
pub fn generate(&mut self, program: &mut Program) -> String {
    // Run optimizer before code generation
    if self.optimization_level != OptimizationLevel::O0 {
        let handler = Arc::new(crate::diagnostics::CollectingDiagnosticHandler::new());
        let mut optimizer = Optimizer::new(
            self.optimization_level,
            handler,
            self.interner.clone(),  // Same Arc!
        );
        let _ = optimizer.optimize(program);
    }
    // ... existing codegen logic
}
```

- [ ] Update all other impl blocks that use lifetime `'a`

---

#### Phase 2: Update Call Sites (~40 files)

**Pattern change:**

```rust
// Before:
let interner = StringInterner::new();
let mut codegen = CodeGenerator::new(&interner);

// After:
let interner = Arc::new(StringInterner::new());
let mut codegen = CodeGenerator::new(interner.clone());
// Note: Lexer/Parser/TypeChecker still use &interner (Arc<T> derefs to &T)
```

**Files to update:**

- [ ] `crates/typedlua-cli/src/main.rs` - CLI entry point
- [ ] `crates/typedlua-core/src/codegen/mod.rs` - internal tests (~line 3698, 3752)
- [ ] Test files (use `&interner` for lexer/parser, `interner.clone()` for codegen):
  - [ ] `tests/bang_operator_tests.rs`
  - [ ] `tests/builtin_decorator_tests.rs`
  - [ ] `tests/decorator_tests.rs`
  - [ ] `tests/destructuring_tests.rs`
  - [ ] `tests/error_classes_tests.rs`
  - [ ] `tests/error_path_tests.rs`
  - [ ] `tests/exception_handling_tests.rs`
  - [ ] `tests/exception_optimization_tests.rs`
  - [ ] `tests/function_inlining_tests.rs` (already correct)
  - [ ] `tests/interface_default_methods_tests.rs`
  - [ ] `tests/namespace_tests.rs`
  - [ ] `tests/null_coalescing_iife_tests.rs`
  - [ ] `tests/null_coalescing_tests.rs`
  - [ ] `tests/o1_combined_tests.rs`
  - [ ] `tests/o3_combined_tests.rs`
  - [ ] `tests/oop_tests.rs`
  - [ ] `tests/operator_overload_tests.rs`
  - [ ] `tests/optimizer_integration_tests.rs`
  - [ ] `tests/pattern_matching_tests.rs`
  - [ ] `tests/pipe_tests.rs`
  - [ ] `tests/primary_constructor_tests.rs`
  - [ ] `tests/reflection_tests.rs`
  - [ ] `tests/rest_params_tests.rs`
  - [ ] `tests/rich_enum_tests.rs`
  - [ ] `tests/safe_navigation_tests.rs`
  - [ ] `tests/spread_tests.rs`
  - [ ] `tests/table_preallocation_tests.rs`
  - [ ] `tests/template_dedent_tests.rs`
- [ ] Benchmark files:
  - [ ] `benches/reflection_bench.rs`
- [ ] Example files:
  - [ ] `examples/profile_allocations.rs`

---

#### Phase 3: Verify and Test

- [ ] Run `cargo check --lib -p typedlua-core` - should compile without errors
- [ ] Run `cargo test -p typedlua-core` - all existing tests should pass
- [ ] Run function inlining tests specifically:

```bash
cargo test -p typedlua-core function_inlining
```

- [ ] Verify inlined code generates correctly (temp variables resolve properly)

---

#### Implementation Notes

**Why `Arc<StringInterner>`?**

- `Arc` allows shared ownership between CodeGenerator and Optimizer
- `&Arc<T>` automatically derefs to `&T`, so Lexer/Parser/TypeChecker don't need changes
- Thread-safe (though currently single-threaded, future-proofs for parallel compilation)

**What stays the same:**

- Lexer, Parser, TypeChecker signatures (`&StringInterner`)
- Optimizer already uses `Arc<StringInterner>`
- FunctionInliningPass already uses `get_or_intern()` correctly

**Completed work:**

- [x] FunctionInliningPass implementation (~900 lines in passes.rs)
- [x] Inlining policy (5 statement threshold, recursion/closure guards)
- [x] AST transformation (inline_statement, inline_expression)
- [x] Optimizer integration (pass registered, set_interner() called)
