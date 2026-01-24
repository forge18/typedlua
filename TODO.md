# TypedLua TODO

**Last Updated:** 2026-01-17

---

## P0: Language Features (Partially Implemented - Need Completion)

### 1.4 Null Coalescing Operator (`??`)

**Status:** Complete (including O2 optimization) | **Model:** Sonnet

- [x] Add `NullCoalesce` variant to `BinaryOp` enum in ast/expression.rs (already existed)
- [x] Lexer: Parse `??` token (and `?.` for safe navigation)
- [x] Parser: Parse `??` with correct precedence (lower than comparison, higher than `or`)
- [x] Type checker: Type left operand as any type, right operand compatible with non-nil version of left
- [x] Codegen: Simple form `(a ~= nil and a or b)` for identifiers and simple member access
- [x] Codegen: IIFE form for complex expressions (avoid double evaluation)
- [x] Codegen: O2 optimization - skip nil check for guaranteed non-nil expressions (literals, objects, arrays, new expressions)
- [x] Tests: null_coalescing_tests.rs (24/24 pass)
- [x] Tests: null_coalescing_iife_tests.rs (15/15 pass)

---

### 1.5 Safe Navigation Operator (`?.`)

**Status:** Implementation completed but blocked by StringId migration | **Model:** Sonnet

- [x] Add `OptionalMember`, `OptionalIndex`, `OptionalCall`, `OptionalMethodCall` expression kinds to AST
- [x] Parser: Parse `?.` as optional member access
- [x] Parser: Parse `?.[` as optional index access  
- [x] Parser: Parse `?.method()` as optional method call
- [x] Parser: Parse `?.()` as optional function call
- [x] Type checker: If receiver is `T | nil`, result is `PropertyType | nil`
- [x] Type checker: Implement `make_optional_type()` helper for creating `T | nil` union types
- [x] Type checker: Implement `infer_method_type()` for method call type inference
- [x] Type checker: Implement `check_call_arguments()` for argument compatibility checking
- [x] Codegen: IIFE form for long chains (3+ levels)
- [x] Codegen: Simple `and` chaining for short chains (optimization)
- [x] Codegen: Implement `is_simple_expression()` to determine optimization strategy
- [x] Codegen: Generate optimized code for all optional access patterns
- [x] Fix test compilation: safe_navigation_tests.rs

**Test file:** safe_navigation_tests.rs

---

### 1.6 Operator Overloading

**Status:** Lexer keyword exists, implementation missing | **Model:** Sonnet

Lexer keyword `Operator` exists but no AST/parser/type checker/codegen.

- [ ] Create `OperatorDeclaration` struct in AST
- [ ] Create `OperatorKind` enum (Add, Sub, Mul, Div, Mod, Pow, Eq, Lt, Le, Concat, Len, Index, NewIndex, Call, Unm)
- [ ] Parser: Parse `operator` followed by operator symbol in class body
- [ ] Type checker: Validate operator signatures (e.g., `operator ==` must return boolean)
- [ ] Type checker: Binary operators take one parameter (right operand)
- [ ] Type checker: Unary operators take no parameters
- [ ] Codegen: Map to Lua metamethods (`__add`, `__sub`, etc.)
- [ ] Fix test compilation: operator_overload_tests.rs

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
