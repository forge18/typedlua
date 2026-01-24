# TypedLua TODO

**Last Updated:** 2026-01-17

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
- [ ] Codegen: O2 optimization - skip nil check for guaranteed non-nil expressions (literals, objects, arrays, new expressions) - Deferred to O2 implementation
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

**Status:** PARTIALLY IMPLEMENTED | **Model:** Sonnet

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

- [ ] Fix remaining test failures:
  - [ ] test_operator_unary_minus - unary minus parsing
  - [ ] test_multiple_operators - syntax error in test

**Test file:** operator_overload_tests.rs

---

### 2.1 Exception Handling

**Status:** Lexer keywords exist, implementation missing | **Model:** Opus (complex feature)

Lexer keywords `Throw`, `Try`, `Catch`, `Finally`, `Rethrow`, `Throws`, `BangBang` exist but no AST/parser/type checker/codegen.

- [ ] Create `TryStatement` struct
- [ ] Create `CatchClause` struct
- [ ] Create `CatchPattern` enum (Untyped, Typed, MultiTyped, Destructured)
- [ ] Create `ThrowStatement` struct
- [ ] Create `TryExpression` struct
- [ ] Create `ErrorChainExpression` struct for `!!`
- [ ] Add `throws: Option<Vec<Type>>` to `FunctionDeclaration`
- [ ] Parser: Parse `throw` statement
- [ ] Parser: Parse `try`/`catch`/`finally` blocks
- [ ] Parser: Parse catch patterns (simple, typed, multi-typed, destructured)
- [ ] Parser: Parse `rethrow` statement
- [ ] Parser: Parse `try ... catch ...` as expression
- [ ] Parser: Parse `!!` operator
- [ ] Parser: Parse `throws` clause on functions
- [ ] Type checker: Type `throw` expression (any type)
- [ ] Type checker: Type catch blocks with declared types
- [ ] Type checker: Type try expression as union of try and catch results
- [ ] Type checker: Validate `rethrow` only in catch blocks
- [ ] Codegen: Automatic pcall vs xpcall selection based on complexity
- [ ] Codegen: Simple catch → pcall (30% faster)
- [ ] Codegen: Typed/multi-catch → xpcall (full-featured)
- [ ] Codegen: Finally blocks with guaranteed execution
- [ ] Codegen: Try expressions → inline pcall
- [ ] Codegen: Error chaining `!!` operator
- [ ] Fix test compilation: exception_handling_tests.rs, exception_optimization_tests.rs, error_classes_tests.rs, bang_operator_tests.rs

**Test files:** exception_handling_tests.rs, exception_optimization_tests.rs, error_classes_tests.rs, bang_operator_tests.rs

---

### 2.2 Rich Enums (Java-style)

**Status:** Not implemented | **Model:** Sonnet

- [ ] Extend `EnumDeclaration` with fields, constructor, methods
- [ ] Update `EnumMember` to include constructor arguments
- [ ] Create `EnumField` struct
- [ ] Parser: Parse enum members with constructor arguments syntax
- [ ] Parser: Parse field declarations inside enum
- [ ] Parser: Parse constructor inside enum
- [ ] Parser: Parse methods inside enum
- [ ] Type checker: Validate constructor parameters match field declarations
- [ ] Type checker: Validate enum member arguments match constructor signature
- [ ] Type checker: Type check methods with `self` bound to enum type
- [ ] Type checker: Auto-generate signatures for `name()`, `ordinal()`, `values()`, `valueOf()`
- [ ] Codegen: Generate enum constructor function
- [ ] Codegen: Generate enum instances as constants
- [ ] Codegen: Generate `name()` and `ordinal()` methods
- [ ] Codegen: Generate `values()` static method
- [ ] Codegen: Generate `valueOf()` with O(1) hash lookup
- [ ] Codegen: Generate static `__byName` lookup table
- [ ] Codegen: Prevent instantiation via metatable
- [ ] Fix test compilation: rich_enum_tests.rs

**Test file:** rich_enum_tests.rs

---

### 2.3 Interfaces with Default Implementations

**Status:** Not implemented | **Model:** Sonnet

- [ ] Add `DefaultMethod(MethodDeclaration)` to `InterfaceMember` enum
- [ ] Parser: Parse interface methods with `{` after signature as default methods
- [ ] Parser: Parse interface methods without `{` as abstract methods
- [ ] Type checker: Track which methods are abstract vs default
- [ ] Type checker: Error if abstract method not implemented
- [ ] Type checker: Allow default methods to be optional (use default if not overridden)
- [ ] Type checker: Type `self` in default methods as implementing class
- [ ] Codegen: Generate interface table with default methods
- [ ] Codegen: Copy default implementations to implementing class: `User.print = User.print or Printable.print`
- [ ] Fix test compilation: interface_default_methods_tests.rs

**Test file:** interface_default_methods_tests.rs

---

### 2.4 File-Based Namespaces

**Status:** Lexer keyword exists, implementation missing | **Model:** Sonnet

Lexer keyword `Namespace` exists (only `DeclareNamespaceStatement` for .d.tl files). File-scoped namespaces not implemented.

- [ ] Add `NamespaceDeclaration` to `Statement` enum with path: `Vec<String>`
- [ ] Parser: Parse `namespace Math.Vector;` at file start
- [ ] Parser: Error if namespace appears after other statements
- [ ] Parser: Only allow semicolon syntax (no block `{}` syntax)
- [ ] Parser: Store namespace path in module metadata
- [ ] Type checker: Track namespace for each module
- [ ] Type checker: Include namespace prefix when resolving imports
- [ ] Type checker: If `enforceNamespacePath: true`, verify namespace matches file path
- [ ] Type checker: Make namespace types accessible via dot notation
- [ ] Codegen: Generate nested table structure for namespace
- [ ] Codegen: Export namespace root table
- [ ] Config: Add `enforceNamespacePath` boolean option (default: false)
- [ ] Fix test compilation: namespace_tests.rs

**Test file:** namespace_tests.rs

---

### 2.5 Template Literal Auto-Dedenting

**Status:** Not implemented | **Model:** Haiku (algorithmic task)

- [ ] Lexer: Track indentation of each line when parsing template literals
- [ ] Lexer: Store raw string with indentation metadata
- [ ] Codegen: Implement dedenting algorithm
- [ ] Codegen: Find first/last non-empty lines
- [ ] Codegen: Find minimum indentation
- [ ] Codegen: Remove common indentation
- [ ] Codegen: Trim first/last blank lines
- [ ] Codegen: Join with `\n`
- [ ] Codegen: Apply dedenting during codegen
- [ ] Codegen: Handle edge cases: tabs vs spaces, first-line content, explicit `\n`
- [ ] Fix test compilation: template_dedent_tests.rs

**Test file:** template_dedent_tests.rs

---

### 2.6 Reflection System

**Status:** Not implemented | **Model:** Opus (multi-crate, FFI, complex)

**Rust Native Module:**

- [ ] Create `crates/typedlua-reflect-native/` cargo project with mlua dependency
- [ ] Implement type registry with compile-time metadata
- [ ] Implement `is_instance()` with O(1) ancestor bitmask checks
- [ ] Implement `typeof()` returning type info
- [ ] Implement `get_fields()` with lazy building
- [ ] Implement `get_methods()` with lazy building
- [ ] Implement field/method lookup with HashMap (O(1))
- [ ] String interning for type/field/method names
- [ ] Compact binary metadata with bitflags

**LuaRocks Distribution:**

- [ ] Create `.rockspec` file
- [ ] Set up cargo build command
- [ ] Pre-compile binaries for Linux (x64, ARM), macOS (x64, ARM), Windows (x64)
- [ ] Publish to LuaRocks
- [ ] Publish to GitHub releases

**Runtime Integration:**

- [ ] Create Lua runtime wrapper for native module
- [ ] Implement `Runtime.isInstance()`
- [ ] Implement `Runtime.typeof()`
- [ ] Implement `Runtime.getFields()`

**Codegen:**

- [ ] Assign unique `__typeId` to each class
- [ ] Generate `__ancestorMask` bitset for inheritance
- [ ] Generate lazy `_buildFields()` function
- [ ] Generate lazy `_buildMethods()` function
- [ ] Generate lazy `_resolveType()` functions
- [ ] Use bitflags for field modifiers (readonly, optional)
- [ ] Use string interning for names
- [ ] Fix test compilation: reflection_tests.rs

**Test file:** reflection_tests.rs

---

### 3.1-3.4 Compiler Optimizations

**Status:** O1 passes implemented and tested, O2/O3 passes scaffolded (analysis-only) | **Model:** Opus

All 15 optimization passes are registered. O1 passes (constant folding, dead code elimination, algebraic simplification) are fully functional. O2/O3 passes are analysis-only placeholders awaiting full implementation.

**3.1 Optimization Infrastructure:**

- [x] Create `crates/typedlua-core/src/optimizer/mod.rs` module
- [x] Create `Optimizer` struct with optimization passes
- [x] Implement `OptimizationPass` trait
- [x] Add `OptimizationLevel` enum to config.rs (O0, O1, O2, O3)
- [x] Add `optimization_level: OptimizationLevel` to `CompilerOptions`
- [x] Add `with_optimization_level()` method to `CodeGenerator`
- [x] Integrate optimizer into compilation pipeline
- [x] Fixed-point iteration (runs passes until no changes)
- [x] Level-based pass filtering (only runs passes <= current level)

**3.2 O1 Optimizations - Basic (COMPLETE):**

- [x] Constant folding (numeric + boolean expressions)
- [x] Dead code elimination (after return/break/continue)
- [x] Algebraic simplification (x+0=x, x*1=x, x*0=0, etc.)
- [x] Table pre-allocation (analysis pass - scaffolded)
- [x] Global localization (analysis pass - scaffolded)

**3.3 O2 Optimizations - Standard (SCAFFOLDED - analysis only):**

- [x] Function inlining (threshold: 5 statements) - analysis only
- [x] Loop optimization - analysis only
- [ ] Null coalescing optimization (inline vs IIFE) - needs null coalescing feature
- [ ] Safe navigation optimization - needs safe navigation feature
- [ ] Exception handling optimization - needs exception handling feature
- [x] String concatenation optimization - analysis only
- [x] Dead store elimination - analysis only
- [ ] Method to function call conversion
- [x] Tail call optimization - analysis only (Lua handles TCO automatically)
- [ ] Rich enum optimization - needs rich enum feature

**3.4 O3 Optimizations - Aggressive (SCAFFOLDED - analysis only):**

- [x] Devirtualization - analysis only
- [x] Generic specialization - analysis only
- [x] Operator inlining - analysis only
- [x] Interface method inlining - analysis only
- [x] Aggressive inlining (threshold: 15 statements) - analysis only

**Test files:** optimizer_integration_tests.rs, o1_combined_tests.rs, o3_combined_tests.rs

---

## P1: Core Infrastructure

### Create typedlua-runtime Crate

**Status:** Not Started | **Expected:** Better modularity, testability, versioning | **Model:** Sonnet

TypedLua extends Lua with many features not in base Lua (classes, decorators, exceptions, rich enums, etc.). Each requires runtime support code embedded in generated Lua. Currently scattered in codegen - needs dedicated crate.

**Crate Setup:**

- [ ] Create `crates/typedlua-runtime/` with lib.rs
- [ ] Add modules: classes.rs, decorators.rs, exceptions.rs, operators.rs, enums.rs, reflection.rs
- [ ] Create `lua/` directory for Lua source snippets

**Lua Runtime Snippets:**

- [ ] Extract decorator runtime to `lua/decorator_runtime.lua`
- [ ] Create class system runtime in `lua/class_runtime.lua`
- [ ] Create exception helpers in `lua/exception_runtime.lua` (pcall/xpcall wrappers)
- [ ] Create operator helpers in `lua/operator_helpers.lua` (safe nav, null coalesce)
- [ ] Create enum runtime in `lua/enum_runtime.lua`
- [ ] Create reflection runtime in `lua/reflection_runtime.lua`

**Const String Exports:**

- [ ] Use `include_str!` to embed Lua snippets as const strings
- [ ] Export one const per feature (e.g., `pub const DECORATOR_RUNTIME: &str`)
- [ ] Version snippets per Lua target (5.1, 5.2, 5.3, 5.4) where needed

**Integration:**

- [ ] Add `typedlua-runtime` dependency to `typedlua-core`
- [ ] Update codegen to import runtime constants
- [ ] Track which features are used (uses_decorators, uses_exceptions, etc.)
- [ ] Only embed runtime for features actually used in compiled code

**Testing:**

- [ ] Unit test each Lua snippet independently
- [ ] Integration tests with codegen

---

### Lua Target Strategy Pattern

**Status:** Not Started | **Expected:** Better maintainability, easier to add versions | **Model:** Sonnet

Current approach: capability checks scattered in codegen (`supports_bitwise_ops()`, `supports_goto()`). Doesn't scale well.

**Trait Definition:**

- [ ] Create `crates/typedlua-core/src/codegen/strategies/mod.rs`
- [ ] Define `CodeGenStrategy` trait with methods:
  - `generate_bitwise_op(&self, op, lhs, rhs) -> String`
  - `generate_integer_divide(&self, lhs, rhs) -> String`
  - `generate_continue(&self, label) -> String`
  - `emit_preamble(&self) -> Option<String>` (for library includes)

**Strategy Implementations:**

- [ ] Create `strategies/lua51.rs` implementing `CodeGenStrategy`
- [ ] Create `strategies/lua52.rs` implementing `CodeGenStrategy`
- [ ] Create `strategies/lua53.rs` implementing `CodeGenStrategy`
- [ ] Create `strategies/lua54.rs` implementing `CodeGenStrategy`

**Integration:**

- [ ] Add `strategy: Box<dyn CodeGenStrategy>` field to `CodeGenerator`
- [ ] Select strategy based on `LuaTarget` during initialization
- [ ] Replace conditional logic in codegen with strategy method calls
- [ ] Remove `supports_*` methods from `LuaTarget` (logic now in strategies)

**Testing:**

- [ ] Unit test each strategy independently
- [ ] Regression tests for version-specific output

---

### Code Generator Modularization

**Status:** Not Started | **Expected:** 50%+ maintainability improvement | **Model:** Sonnet

CodeGenerator is 3,120 lines - too large. Break into focused modules.

**Directory Structure:**

- [ ] Create `crates/typedlua-core/src/codegen/strategies/` (Lua version strategies)
- [ ] Create `crates/typedlua-core/src/codegen/emitters/` (AST → Lua emitters)
- [ ] Create `crates/typedlua-core/src/codegen/transforms/` (pluggable transforms)

**Emitters (AST → Lua):**

- [ ] Extract expression generation to `emitters/expressions.rs`
- [ ] Extract statement generation to `emitters/statements.rs`
- [ ] Extract type erasure to `emitters/types.rs`
- [ ] Main codegen becomes orchestrator (~300 lines)

**Transforms (Pipeline Pattern):**

- [ ] Define `CodeGenTransform` trait (like `OptimizationPass`)
- [ ] Create `transforms/classes.rs` for class → table transformation
- [ ] Create `transforms/decorators.rs` for decorator emission
- [ ] Create `transforms/modules.rs` for import/export handling
- [ ] Move sourcemap logic to `transforms/sourcemaps.rs`

**Integration:**

- [ ] Register transforms in CodeGenerator::new()
- [ ] Run transforms in pipeline during generation
- [ ] Each transform testable in isolation

---

### Type Checker Visitor Pattern

**Status:** Not Started | **Expected:** Better separation of concerns | **Model:** Sonnet

Type checker is 3,544 lines. Extract specialized visitors for different concerns.

**Visitor Trait:**

- [ ] Create `crates/typedlua-core/src/typechecker/visitors/mod.rs`
- [ ] Define `TypeCheckVisitor` trait with visit methods

**Specialized Visitors:**

- [ ] Create `visitors/narrowing.rs` - Type narrowing logic
- [ ] Create `visitors/generics.rs` - Generic instantiation and constraints
- [ ] Create `visitors/access_control.rs` - public/private/protected checks
- [ ] Create `visitors/inference.rs` - Type inference rules

**Integration:**

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
