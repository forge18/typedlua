# TypedLua TODO

**Last Updated:** 2026-01-15 (Primary Constructors completed)

This file tracks implementation tasks for TypedLua. Tasks are organized by priority based on the [Additional Features Design](docs/designs/Additional-Features-Design.md) document.

---

## Phase 1: Core Language Features (Quick Wins)

### 1.1 Override Keyword
**Effort:** 2-4 hours | **Priority:** P0 | **Status:** ✅ Completed

- [x] Add `Override` to `TokenKind` enum in lexer
- [x] Add `is_override: bool` to `MethodDeclaration` AST
- [x] Parse `override` keyword before method declarations
- [x] Type checker: Verify parent class exists and has method
- [x] Type checker: Verify method signatures are compatible
- [x] Add error for override without parent method
- [x] Add warning for overriding without `override` keyword (optional)
- [x] Write unit tests for override validation
- [x] Update documentation

**Dependencies:** None

**Implementation Notes:**
- Lexer: Added `Override` token at [token.rs:48](crates/typedlua-core/src/lexer/token.rs#L48)
- AST: Added `is_override` field at [statement.rs:104](crates/typedlua-core/src/ast/statement.rs#L104)
- Parser: Parses override keyword at [statement.rs:977](crates/typedlua-core/src/parser/statement.rs#L977)
- Type Checker: Validates override semantics at [type_checker.rs:1489-1619](crates/typedlua-core/src/typechecker/type_checker.rs#L1489-L1619)
- Type Checker: Emits warning for missing `override` keyword at [type_checker.rs:1395-1411](crates/typedlua-core/src/typechecker/type_checker.rs#L1395-L1411)
- Tests: Comprehensive tests at [override_tests.rs](crates/typedlua-core/tests/override_tests.rs) (5 passing tests including warning test)
- Documentation: Complete feature documentation at [docs/LANGUAGE_FEATURES.md](docs/LANGUAGE_FEATURES.md)
- Fixed `extract_exports` function that was broken during implementation

---

### 1.2 Final Keyword
**Effort:** 1-2 days | **Priority:** P0 | **Status:** ✅ Completed

- [x] Add `Final` to `TokenKind` enum
- [x] Add `is_final: bool` to `ClassDeclaration` AST
- [x] Add `is_final: bool` to `MethodDeclaration` AST
- [x] Parse `final` before class keyword
- [x] Parse `final` before method declarations
- [x] Type checker: Error when extending final class
- [x] Type checker: Error when overriding final method
- [x] Write unit tests for final validation
- [x] Update documentation

**Dependencies:** None

**Implementation Notes:**
- Lexer: Added `Final` token at [token.rs:49](crates/typedlua-core/src/lexer/token.rs#L49) (length-5 keyword mapping)
- AST: Added `is_final` field to [ClassDeclaration](crates/typedlua-core/src/ast/statement.rs#L61) and [MethodDeclaration](crates/typedlua-core/src/ast/statement.rs#L105)
- Parser: Parses `final` and `abstract` modifiers in any order using loop
- Type Checker: Validates final class extension at [type_checker.rs:964-976](crates/typedlua-core/src/typechecker/type_checker.rs#L964-L976)
- Type Checker: Validates final method override at [type_checker.rs:1570-1579](crates/typedlua-core/src/typechecker/type_checker.rs#L1570-L1579)
- Tests: Comprehensive tests at [final_tests.rs](crates/typedlua-core/tests/final_tests.rs) (8 passing tests)
- Documentation: Complete feature documentation at [docs/LANGUAGE_FEATURES.md](docs/LANGUAGE_FEATURES.md)
- Note: Currently only checks immediate parent for final methods (not full inheritance chain - can be enhanced later)

---

### 1.3 Primary Constructors
**Effort:** 1 week | **Priority:** P0 | **Status:** ✅ Completed

**AST Changes:**
- [x] Add `primary_constructor: Option<Vec<ConstructorParameter>>` to `ClassDeclaration`
- [x] Create `ConstructorParameter` struct with name, type, access modifier, readonly flag
- [x] Add `parent_constructor_args: Option<Vec<Expression>>` to `ClassDeclaration` for forwarding

**Parser:**
- [x] Parse `class Name(params)` syntax
- [x] Parse access modifiers on constructor parameters (`public`, `private`, `protected`)
- [x] Parse `readonly` modifier on parameters
- [x] Parse `extends Parent(arg1, arg2)` for constructor forwarding
- [x] Error if both primary constructor and parameterized constructor exist

**Type Checker:**
- [x] Create property declarations from primary constructor parameters
- [x] Validate access modifiers on parameters
- [x] Check parent constructor argument types
- [x] Ensure constructor body doesn't redeclare parameter properties

**Codegen:**
- [x] Generate properties from primary constructor parameters
- [x] Apply access modifiers (private → `_name` prefix)
- [x] Generate parent constructor forwarding
- [x] Support optional constructor body for validation

**Implementation Notes:**
- Modified `generate_class_declaration` at [mod.rs:771-805](crates/typedlua-core/src/codegen/mod.rs#L771-L805) to detect primary constructors
- Created `generate_primary_constructor` function at [mod.rs:953-1062](crates/typedlua-core/src/codegen/mod.rs#L953-L1062)
- Generates both `._init(self, params)` for initialization and `.new(params)` for instance creation
- Private properties prefixed with `_`, public/protected use normal naming
- Parent constructor forwarding calls `ParentClass._init(self, args)` before property initialization
- Follows existing Lua metatable pattern for consistency with regular class generation

**Testing:**
- [x] Test basic primary constructor
- [x] Test with access modifiers
- [x] Test with inheritance
- [x] Test with additional properties
- [x] Test with constructor body
- [x] Test readonly parameters
- [x] Test error cases (mixing patterns)

**Implementation Notes:**
- Comprehensive test suite at [primary_constructor_tests.rs](crates/typedlua-core/tests/primary_constructor_tests.rs) (22 passing tests)
- Parser tests: Basic syntax, access modifiers, readonly, inheritance, error cases
- Type checker tests: Property creation, duplicate detection, parent constructor validation
- Codegen tests: Verify generated Lua code for constructors, access modifiers, inheritance, metatable setup
- All tests pass successfully

**Documentation:**
- [x] Update design docs with examples
- [x] Add to language reference

**Implementation Notes:**
- Comprehensive documentation added to [LANGUAGE_FEATURES.md](docs/LANGUAGE_FEATURES.md)
- Covers syntax, usage patterns, access modifiers, inheritance, examples, and compilation details
- Design examples already exist in [Additional-Features-Design.md](docs/designs/Additional-Features-Design.md)
- Documentation explains generated Lua code patterns and type checking behavior

**Dependencies:** Class system must be implemented

---

### 1.4 Null Coalescing Operator (`??`)
**Effort:** 2-3 days | **Priority:** P1 | **Status:** Not Started

**Lexer:**
- [ ] Add `QuestionQuestion` to `TokenKind`
- [ ] Parse `??` as single token (not two `?`)

**AST:**
- [ ] Add `NullCoalesce` to `BinaryOp` enum

**Parser:**
- [ ] Parse `??` with correct precedence (lower than comparison, higher than `or`)

**Type Checker:**
- [ ] Type left operand as any type
- [ ] Type right operand compatible with non-nil version of left
- [ ] Result type: non-nil union of both sides

**Codegen:**
- [ ] Simple form: `(a ~= nil and a or b)` for identifiers
- [ ] IIFE form for complex expressions (avoid double evaluation)
- [ ] Optimization: Detect when to use each form

**Optimizer (O2):**
- [ ] Eliminate nil checks when left operand proven non-nil by type analysis

**Testing:**
- [ ] Test with simple identifiers
- [ ] Test with complex expressions
- [ ] Test with `false` values (vs `or` operator)
- [ ] Test type inference
- [ ] Test optimization elimination

**Documentation:**
- [ ] Add to language reference
- [ ] Add examples comparing to `or` operator

**Dependencies:** None

---

### 1.5 Safe Navigation Operator (`?.`)
**Effort:** 3-4 days | **Priority:** P1 | **Status:** Not Started

**Lexer:**
- [ ] Add `QuestionDot` to `TokenKind` for `?.`
- [ ] Add `QuestionLeftBracket` to `TokenKind` for `?.[`

**AST:**
- [ ] Add `is_optional: bool` to member access expressions
- [ ] Or create `OptionalMember`, `OptionalIndex`, `OptionalCall` expression kinds

**Parser:**
- [ ] Parse `?.` as optional member access
- [ ] Parse `?.[` as optional index access
- [ ] Parse `?.method()` as optional method call

**Type Checker:**
- [ ] If receiver is `T | nil`, result is `PropertyType | nil`
- [ ] Chain of `?.` accumulates nil possibility

**Codegen:**
- [ ] IIFE form for long chains (3+ levels)
- [ ] Simple `and` chaining for short chains (optimization)

**Optimizer (O2):**
- [ ] Skip nil checks when receiver proven non-nil by type analysis
- [ ] Especially valuable in hot loops

**Testing:**
- [ ] Test property access chains
- [ ] Test method calls
- [ ] Test indexed access
- [ ] Test mixed chains
- [ ] Test type inference through chains
- [ ] Test optimization elimination

**Documentation:**
- [ ] Add to language reference
- [ ] Show chaining examples

**Dependencies:** None

---

### 1.6 Operator Overloading
**Effort:** 1-2 weeks | **Priority:** P1 | **Status:** Not Started

**Lexer:**
- [ ] Add `Operator` keyword to `TokenKind`

**AST:**
- [ ] Create `OperatorDeclaration` struct
- [ ] Create `OperatorKind` enum (Add, Sub, Mul, Div, Mod, Pow, Eq, Lt, Le, Concat, Len, Index, NewIndex, Call, Unm)

**Parser:**
- [ ] Parse `operator` followed by operator symbol in class body
- [ ] Parse parameter list and body

**Type Checker:**
- [ ] Validate operator signatures (e.g., `operator ==` must return boolean)
- [ ] Binary operators take one parameter (right operand)
- [ ] Unary operators take no parameters

**Codegen:**
- [ ] Map to Lua metamethods (`__add`, `__sub`, etc.)
- [ ] Cache operator functions as named locals for performance
- [ ] Store as direct methods for O3 devirtualization

**Testing:**
- [ ] Test all supported operators (+, -, *, /, %, ^, ==, <, <=, .., #, [], []=, ())
- [ ] Test type checking of operators
- [ ] Test usage in expressions
- [ ] Test operator chaining

**Documentation:**
- [ ] Document all supported operators and their metamethod mappings
- [ ] Add Vector example

**Dependencies:** None

---

## Phase 2: Advanced Features

### 2.1 Exception Handling
**Effort:** 2-3 weeks | **Priority:** P1 | **Status:** Not Started

**Lexer:**
- [ ] Add `Throw`, `Try`, `Catch`, `Finally`, `Rethrow` to `TokenKind`
- [ ] Add `BangBang` for `!!` operator

**AST:**
- [ ] Create `TryStatement` struct
- [ ] Create `CatchClause` struct
- [ ] Create `CatchPattern` enum (Untyped, Typed, MultiTyped, Destructured)
- [ ] Create `ThrowStatement` struct
- [ ] Create `TryExpression` struct
- [ ] Create `ErrorChainExpression` struct for `!!`
- [ ] Add `throws: Option<Vec<Type>>` to `FunctionDeclaration`

**Parser:**
- [ ] Parse `throw` statement
- [ ] Parse `try`/`catch`/`finally` blocks
- [ ] Parse catch patterns (simple, typed, multi-typed, destructured)
- [ ] Parse `rethrow` statement
- [ ] Parse `try ... catch ...` as expression
- [ ] Parse `!!` operator
- [ ] Parse `throws` clause on functions

**Type Checker:**
- [ ] Type `throw` expression (any type)
- [ ] Type catch blocks with declared types
- [ ] Type try expression as union of try and catch results
- [ ] Validate `rethrow` only in catch blocks
- [ ] `throws` annotation is informational only

**Codegen:**
- [ ] Automatic pcall vs xpcall selection based on complexity
- [ ] Simple catch → pcall (30% faster)
- [ ] Typed/multi-catch → xpcall (full-featured)
- [ ] Finally blocks with guaranteed execution
- [ ] Try expressions → inline pcall
- [ ] Error chaining `!!` operator

**Built-in Error Classes:**
- [ ] Implement `Error` base class
- [ ] Implement `ArgumentError`
- [ ] Implement `StateError`
- [ ] Implement `IOError`
- [ ] Implement `ParseError`
- [ ] Add debug info capture (file, line, stack)

**Helper Functions:**
- [ ] Implement `require()` function
- [ ] Implement `check()` function
- [ ] Implement `unreachable()` function

**Optimizer (O2):**
- [ ] Inline simple try-catch blocks
- [ ] Choose optimal pcall/xpcall based on AST analysis

**Testing:**
- [ ] Test throw/catch
- [ ] Test typed catches
- [ ] Test multi-type catches
- [ ] Test finally blocks
- [ ] Test try expressions
- [ ] Test rethrow
- [ ] Test error chaining `!!`
- [ ] Test pattern matching catch
- [ ] Test pcall vs xpcall selection

**Documentation:**
- [ ] Document exception handling syntax
- [ ] Document built-in error classes
- [ ] Document helper functions
- [ ] Show examples of all patterns

**Dependencies:** None

---

### 2.2 Rich Enums (Java-style)
**Effort:** 2-3 weeks | **Priority:** P1 | **Status:** Not Started

**AST:**
- [ ] Extend `EnumDeclaration` with fields, constructor, methods
- [ ] Update `EnumMember` to include constructor arguments
- [ ] Create `EnumField` struct

**Parser:**
- [ ] Parse enum members with constructor arguments syntax
- [ ] Parse field declarations inside enum
- [ ] Parse constructor inside enum
- [ ] Parse methods inside enum

**Type Checker:**
- [ ] Validate constructor parameters match field declarations
- [ ] Validate enum member arguments match constructor signature
- [ ] Type check methods with `self` bound to enum type
- [ ] Auto-generate signatures for `name()`, `ordinal()`, `values()`, `valueOf()`

**Codegen:**
- [ ] Generate enum constructor function
- [ ] Generate enum instances as constants
- [ ] Generate `name()` and `ordinal()` methods
- [ ] Generate `values()` static method
- [ ] Generate `valueOf()` with O(1) hash lookup (not O(n) iteration)
- [ ] Generate static `__byName` lookup table
- [ ] Prevent instantiation via metatable

**Optimizer (O2):**
- [ ] Pre-compute enum instances at compile time
- [ ] Mark small methods as inlinable (O3)

**Testing:**
- [ ] Test basic rich enums
- [ ] Test with constructors
- [ ] Test with instance methods
- [ ] Test `name()`, `ordinal()` built-ins
- [ ] Test `values()`, `valueOf()` static methods
- [ ] Test Planet example from design doc

**Documentation:**
- [ ] Document rich enum syntax
- [ ] Show Planet example
- [ ] Document built-in methods

**Dependencies:** None

---

### 2.3 Interfaces with Default Implementations
**Effort:** 1-2 weeks | **Priority:** P1 | **Status:** Not Started

**AST:**
- [ ] Add `DefaultMethod(MethodDeclaration)` to `InterfaceMember` enum

**Parser:**
- [ ] Parse interface methods with `{` after signature as default methods
- [ ] Parse interface methods without `{` as abstract methods

**Type Checker:**
- [ ] Track which methods are abstract vs default
- [ ] Error if abstract method not implemented
- [ ] Allow default methods to be optional (use default if not overridden)
- [ ] Type `self` in default methods as implementing class

**Codegen:**
- [ ] Generate interface table with default methods
- [ ] Copy default implementations to implementing class: `User.print = User.print or Printable.print`

**Optimizer (O3):**
- [ ] Inline default methods when implementing class is known
- [ ] Add memoization hints for pure methods (marked with `@pure` decorator)

**Testing:**
- [ ] Test abstract methods (must implement)
- [ ] Test default methods (optional override)
- [ ] Test multiple interfaces
- [ ] Test overriding default methods
- [ ] Test `self` typing in default methods

**Documentation:**
- [ ] Document interface default implementation syntax
- [ ] Show Printable/Serializable example

**Dependencies:** None

---

### 2.4 File-Based Namespaces
**Effort:** 1-2 weeks | **Priority:** P2 | **Status:** Not Started

**Lexer:**
- [ ] Add `Namespace` to `TokenKind`

**AST:**
- [ ] Add `NamespaceDeclaration` to `Statement` enum with path: `Vec<String>`

**Parser:**
- [ ] Parse `namespace Math.Vector;` at file start
- [ ] Error if namespace appears after other statements
- [ ] Only allow semicolon syntax (no block `{}` syntax)
- [ ] Store namespace path in module metadata

**Type Checker:**
- [ ] Track namespace for each module
- [ ] Include namespace prefix when resolving imports
- [ ] If `enforceNamespacePath: true`, verify namespace matches file path
- [ ] Make namespace types accessible via dot notation

**Codegen:**
- [ ] Generate nested table structure for namespace
- [ ] Export namespace root table

**Config:**
- [ ] Add `enforceNamespacePath` boolean option (default: false)

**Testing:**
- [ ] Test basic namespace declaration
- [ ] Test namespace imports (full path, aliasing, specific exports)
- [ ] Test namespace path enforcement
- [ ] Test with declaration files (.d.tl)
- [ ] Test Godot example from design doc

**Documentation:**
- [ ] Document namespace syntax
- [ ] Show modules vs namespaces comparison
- [ ] Show declaration file use case
- [ ] Document config option

**Dependencies:** Module system must exist

---

### 2.5 Template Literal Enhancements (Auto-Dedenting)
**Effort:** 3-5 days | **Priority:** P2 | **Status:** Not Started

**Lexer:**
- [ ] Track indentation of each line when parsing template literals
- [ ] Store raw string with indentation metadata

**Codegen:**
- [ ] Implement dedenting algorithm:
  - Find first/last non-empty lines
  - Find minimum indentation
  - Remove common indentation
  - Trim first/last blank lines
  - Join with `\n`
- [ ] Apply dedenting during codegen
- [ ] Handle edge cases: tabs vs spaces, first-line content, explicit `\n`

**Testing:**
- [ ] Test basic multi-line dedenting
- [ ] Test preserving relative indentation
- [ ] Test trimming leading/trailing blank lines
- [ ] Test single-line templates (no dedenting)
- [ ] Test tabs vs spaces error
- [ ] Test first line on same line as backtick
- [ ] Test SQL/HTML/JSON examples

**Documentation:**
- [ ] Document auto-dedenting behavior
- [ ] Show SQL, HTML examples
- [ ] Document edge cases

**Dependencies:** Template literals must already exist

---

### 2.6 Reflection System
**Effort:** 2-3 weeks | **Priority:** P2 | **Status:** Not Started

**Rust Native Module (`crates/typedlua-reflect-native/`):**
- [ ] Set up cargo project with mlua dependency
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

**Testing:**
- [ ] Test `isInstance()` with inheritance
- [ ] Test `typeof()` with various types
- [ ] Test `getFields()` with lazy loading
- [ ] Test `getMethods()` with lazy loading
- [ ] Test field info (name, isOptional, isReadonly, getType)
- [ ] Test method info (name, isStatic, isAbstract, getSignature)
- [ ] Performance benchmarks vs pure Lua

**Documentation:**
- [ ] Document reflection API
- [ ] Document installation via LuaRocks
- [ ] Show usage examples
- [ ] Document performance characteristics

**Dependencies:** Rust toolchain, mlua library, LuaRocks infrastructure

---

## Phase 3: Compiler Optimizations

### 3.1 Optimization Infrastructure
**Effort:** 1 week | **Priority:** P2 | **Status:** Not Started

- [ ] Create `Optimizer` struct with optimization passes
- [ ] Implement `OptimizationPass` trait
- [ ] Add optimization level config option (0-3)
- [ ] Integrate optimizer into compilation pipeline

**Dependencies:** Type checker must provide type information

---

### 3.2 O1 Optimizations (Basic)
**Effort:** 1-2 weeks | **Priority:** P2 | **Status:** Not Started

- [ ] **Constant Folding:** Evaluate constant expressions at compile time
- [ ] **Dead Code Elimination:** Remove unreachable code
- [ ] **Table Pre-allocation:** Pre-size tables when size known
- [ ] **Global Localization:** Cache frequently-used globals as locals
- [ ] **Algebraic Simplification:** Simplify identities and strength reduction

**Testing:**
- [ ] Test each optimization individually
- [ ] Test combined effect
- [ ] Benchmark performance improvement

**Dependencies:** Optimization infrastructure

---

### 3.3 O2 Optimizations (Standard)
**Effort:** 2-3 weeks | **Priority:** P2 | **Status:** Not Started

- [ ] **Function Inlining:** Replace small function calls with body (threshold: 5 statements)
- [ ] **Loop Optimization:** Convert ipairs to numeric for (3-5x faster)
- [ ] **Null Coalescing Optimization:** Choose inline vs IIFE based on complexity
- [ ] **Safe Navigation Optimization:** Simple and vs long chain early exit
- [ ] **Exception Handling Optimization:** Auto-select pcall vs xpcall
- [ ] **String Concatenation Optimization:** Use table.concat for 3+ parts
- [ ] **Dead Store Elimination:** Remove assignments to never-read variables
- [ ] **Method to Function Call:** Convert `:` to `.` when type known
- [ ] **Tail Call Optimization:** Convert tail recursion to loops
- [ ] **Rich Enum Optimization:** Pre-compute enum instances

**Testing:**
- [ ] Test each optimization individually
- [ ] Test combined effect
- [ ] Benchmark performance improvement (target: 2-5x vs naive)

**Dependencies:** O1 optimizations

---

### 3.4 O3 Optimizations (Aggressive)
**Effort:** 1-2 weeks | **Priority:** P3 | **Status:** Not Started

- [ ] **Devirtualization:** Direct calls instead of metatable lookups when type known
- [ ] **Generic Specialization:** Generate type-specific versions of generic functions
- [ ] **Operator Inlining:** Inline operator methods when type known
- [ ] **Interface Method Inlining:** Inline default interface methods when class known
- [ ] **Aggressive Inlining:** Increase threshold to 15 statements

**Testing:**
- [ ] Test each optimization individually
- [ ] Test combined effect
- [ ] Benchmark performance improvement (target: 3-7x vs naive)

**Dependencies:** O2 optimizations

---

## Phase 4: Testing & Documentation

### 4.1 Integration Tests
**Effort:** 1-2 weeks | **Priority:** P1 | **Status:** Not Started

- [ ] Write integration tests for all features combined
- [ ] Test feature interactions (e.g., primary constructors + operator overloading)
- [ ] Test edge cases and error conditions
- [ ] Test with different optimization levels
- [ ] Performance regression tests

**Dependencies:** All features implemented

---

### 4.2 Documentation
**Effort:** 1 week | **Priority:** P1 | **Status:** Not Started

- [ ] Update language reference with all new features
- [ ] Create tutorial/guide for each major feature
- [ ] Document optimization levels and what they do
- [ ] Document reflection API
- [ ] Create migration guide from plain Lua
- [ ] Update README with feature showcase

**Dependencies:** All features implemented

---

## Summary

**Total Features:** 13
**Total Estimated Effort:** 22-31 weeks (5.5-7.75 months)

**Priority Breakdown:**
- **P0 (Critical):** ~~Override~~ ✅, ~~Final~~ ✅, ~~Primary Constructors~~ ✅ (All P0 features complete!)
- **P1 (High):** Null coalescing, Safe navigation, Operators, Exceptions, Rich Enums, Interfaces (9-15 weeks)
- **P2 (Medium):** Namespaces, Template dedenting, Reflection, Optimizations (10-13 weeks)
- **P3 (Low):** O3 optimizations (1-2 weeks)

**See:** [docs/designs/Additional-Features-Design.md](docs/designs/Additional-Features-Design.md) for complete specifications.

---

## Active Work

All P0 (Critical) features are now complete! Ready to move to P1 (High Priority) features.

---

## Known Issues

None.

---

## Future Enhancements

### Library Adoption & Tooling (Infrastructure)

**Status:** Dependencies added to Cargo.toml, infrastructure setup in progress

#### Rust Library Ecosystem Integration
**Effort:** 1-2 weeks total | **Priority:** P0-P2 | **Impact:** Better testing, profiling, and development experience

**Decision Summary:** Adopt 9 libraries to improve code quality, testing, and performance measurement.

---

#### id-arena - Arena with Stable IDs
**Effort:** Integrated during salsa | **Priority:** P1 | **Impact:** Graph structure management, eliminates lifetime issues

**Decision:** ✅ **ADOPT during salsa integration**

**Why:**
- Perfect for graph structures (type checker, module graph)
- `Id<T>` is `Copy` with no lifetime constraints (eliminates borrowck fights)
- Serialization-friendly (IDs are u32)
- salsa loves index-based structures
- AST redesign from `Box<T>` → `Id<T>` is already required for salsa

**Implementation:**
- [ ] Use id-arena for type checker graph during salsa Phase 3-4
- [ ] Use id-arena for module graph during salsa Phase 3-4
- [ ] Replace `Box<Expression>` / `Box<Statement>` with `ExpressionId` / `StatementId`
- [ ] Thread arena context through type checker
- [ ] Update serialization to use IDs instead of pointers

**Expected Results:**
- No lifetime hell in graph structures
- Safer incremental compilation (stable IDs across recompilation)
- Cleaner code for complex dependency tracking

**Dependencies:** salsa integration (Phase 3-4)

---

#### indexmap - Ordered HashMap
**Effort:** 2-3 hours | **Priority:** P2 | **Impact:** Deterministic ordering for LSP and diagnostics

**Decision:** ✅ **ADOPT SELECTIVELY** (use alongside FxHashMap)

**Strategy:**
- **Use indexmap for:** LSP symbol tables, diagnostic collection, export tables (where ordering matters)
- **Keep FxHashMap for:** Performance-critical internal structures (where ordering doesn't matter)

**Why:**
- Deterministic iteration order (better debugging, stable error messages)
- Stable snapshot testing with insta
- Consistent LSP completion order (better UX)
- Only 5-10% slower than FxHashMap (acceptable for UI-facing structures)

**Implementation:**
- [ ] Replace symbol tables in LSP with `IndexMap` at [symbol_index.rs](crates/typedlua-lsp/src/symbol_index.rs)
- [ ] Use `IndexMap` for diagnostic collection in [diagnostics.rs](crates/typedlua-core/src/diagnostics.rs)
- [ ] Use `IndexMap` for export tables in module system
- [ ] Keep `FxHashMap` for internal type checker structures

**Expected Results:**
- Deterministic diagnostic ordering
- Stable LSP completion results
- Better snapshot testing with insta

**Dependencies:** None

---

#### indoc - Indented String Literals
**Effort:** 5 minutes | **Priority:** P0 | **Impact:** Dramatically cleaner test code

**Decision:** ✅ **ADOPT IMMEDIATELY**

**Why:**
- Zero runtime cost (compile-time macro)
- Tiny, widely-used dependency
- Pure quality-of-life win for test maintainability
- Hundreds of existing test strings would benefit
- No downside whatsoever

**Implementation:**
- [x] Add `indoc = "2.0"` to workspace dependencies (DONE)
- [ ] Convert existing test strings to use `indoc!` macro opportunistically
- [ ] Use `indoc!` for all new multi-line test strings

**Example:**
```rust
// Before
let source = "\
    function add(a: number, b: number): number\n\
        return a + b\n\
    end\n\
";

// After
let source = indoc! {"
    function add(a: number, b: number): number
        return a + b
    end
"};
```

**Dependencies:** None

---

#### cargo-fuzz - Coverage-Guided Fuzzing
**Effort:** 2-4 hours | **Priority:** P1 | **Impact:** Find parser edge cases and security issues

**Decision:** ✅ **ADOPT for lexer and parser**

**Why:**
- Parsers are PRIME fuzzing targets (industry standard practice)
- WILL find edge cases that unit tests miss (malformed input, stack overflow, panics)
- Continuous security testing
- Catches DoS vulnerabilities before production

**Implementation:**
- [x] Add `cargo-fuzz = "0.12"` to workspace dependencies (DONE)
- [ ] Initialize fuzz directory: `cargo fuzz init` (requires nightly)
- [ ] Create fuzz target for lexer at `fuzz/fuzz_targets/lexer.rs`
- [ ] Create fuzz target for parser at `fuzz/fuzz_targets/parser.rs`
- [ ] Add CI job for continuous fuzzing (OSS-Fuzz integration)
- [ ] Document fuzzing setup in CONTRIBUTING.md

**Example Fuzz Target:**
```rust
// fuzz/fuzz_targets/lexer.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use typedlua_core::Lexer;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = Lexer::new(s).collect::<Vec<_>>();
    }
});
```

**Expected Results:**
- Discovery of edge cases in lexer/parser
- Prevention of panic-based DoS attacks
- Improved robustness for malformed input

**Dependencies:** None (nightly Rust required for fuzzing)

---

#### proptest - Property-Based Testing
**Effort:** 1-2 weeks (ongoing) | **Priority:** P2 | **Impact:** Find invariant violations

**Decision:** ✅ **ADOPT for critical paths only**

**Why:**
- Finds edge cases humans don't think of
- Shrinking finds minimal failing examples (excellent debugging)
- Perfect for testing invariants (parse roundtrips, type soundness)
- Complements existing unit tests and snapshot tests

**Use Cases:**
- Parser round-trip testing: `parse(print(ast)) == ast`
- Type checker soundness: Type operations preserve soundness
- Code generator correctness: Generated Lua is valid

**Implementation:**
- [x] Add `proptest = "1.5"` to workspace dependencies (DONE)
- [ ] Add proptest to parser tests for round-trip property
- [ ] Add proptest to type checker for soundness properties
- [ ] Add proptest to codegen for correctness properties

**Example:**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn parse_print_roundtrip(code in "function.*end") {
        let ast = parse(&code)?;
        let printed = print(&ast);
        let reparsed = parse(&printed)?;
        prop_assert_eq!(ast, reparsed);
    }
}
```

**Expected Results:**
- Discovery of parser bugs through roundtrip testing
- Type checker invariant validation
- Better confidence in compiler correctness

**Dependencies:** None

---

#### criterion - Statistical Benchmarking
**Effort:** 30 minutes | **Priority:** P0 | **Impact:** Baseline measurements before optimization

**Decision:** ✅ **CREATE BENCHMARKS NOW** (already in Cargo.toml but unused)

**Why:**
- **CRITICAL:** Cannot validate optimization work without baselines
- Already configured in Cargo.toml but NO benchmarks exist
- Need measurements before arena allocation integration
- Need measurements before string interning integration
- Statistical analysis detects regressions

**Implementation:**
- [x] `criterion = "0.5"` already in workspace dependencies
- [x] Create `crates/typedlua-core/benches/` directory (DONE)
- [x] Create `lexer_bench.rs` for lexer baseline (DONE)
- [x] Create `parser_bench.rs` for parser baseline (DONE)
- [x] Create `type_checker_bench.rs` for type checker baseline (DONE)
- [ ] Run benchmarks to establish baseline: `cargo bench`
- [ ] Document baseline results in TODO.md or separate BENCHMARKS.md
- [ ] Add benchmark CI job for regression detection

**Expected Results:**
- Baseline lexer performance (tokens/sec)
- Baseline parser performance (AST nodes/sec)
- Baseline type checker performance (lines/sec)
- Ability to validate 15-20% speedup claims for arena allocation
- Ability to validate 30-50% memory savings claims for string interning

**Dependencies:** None - URGENT (needed before optimization work)

---

#### dhat - Heap Profiler
**Effort:** 1 hour | **Priority:** P0 | **Impact:** Measure actual allocation patterns before optimization

**Decision:** ✅ **ADOPT for optimization validation**

**Why:**
- **CRITICAL:** Need to measure ACTUAL allocation hotspots (not guess)
- Validate arena allocation actually reduces allocations
- Validate string interning actually saves memory
- Find unexpected allocations in hot loops
- Zero production cost (dev-only tool)

**Implementation:**
- [x] Add `dhat = "0.3"` to workspace dependencies (DONE)
- [ ] Create profiling harness at `crates/typedlua-core/benches/profile_allocations.rs`
- [ ] Profile current baseline allocations (before optimization)
- [ ] Document baseline allocation patterns
- [ ] Re-profile after arena integration to measure improvement
- [ ] Re-profile after string interning to measure improvement

**Example:**
```rust
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    // Run compiler on large file
    let source = include_str!("large_test.tl");
    let mut parser = Parser::new(source);
    parser.parse().unwrap();
}
```

**Expected Results:**
- Identify allocation hotspots (lexer, parser, type checker)
- Measure reduction in allocations after arena integration (target: 80%+ reduction)
- Measure memory savings after string interning (target: 30-50% reduction)
- Validate optimization claims with data

**Dependencies:** None - URGENT (needed before optimization work)

---

#### insta - Snapshot Testing
**Effort:** Ongoing | **Priority:** P1 | **Impact:** Better test maintainability

**Decision:** ✅ **EXPAND USAGE** (already in Cargo.toml, underutilized)

**Why:**
- Already configured but only 10 uses (massive underutilization)
- Perfect for parser AST snapshots
- Perfect for type checker error messages
- Perfect for code generator output
- Review diffs instead of manually updating assertions

**Implementation:**
- [x] `insta = "1.40"` already in workspace dependencies
- [ ] Convert parser tests to use `insta::assert_snapshot!` for AST output
- [ ] Convert type checker tests to use snapshots for error messages
- [ ] Convert codegen tests to use snapshots for Lua output
- [ ] Make snapshot testing the DEFAULT for all new tests
- [ ] Run `cargo insta review` after test changes

**Example:**
```rust
#[test]
fn test_class_parsing() {
    let source = "class User { name: string }";
    let ast = parse(source);
    insta::assert_snapshot!(format!("{:#?}", ast));
}
```

**Expected Results:**
- Easier test maintenance (review diffs vs manual updates)
- Better visibility of AST/error message changes
- Catch unintended changes during refactoring

**Dependencies:** None

---

### Performance Optimizations (Memory & Speed)

**Status:** High-impact improvements with existing infrastructure ready

#### String Interning Integration
**Effort:** 1-2 days | **Priority:** P0 | **Impact:** 30-50% memory reduction, faster equality checks

Infrastructure already exists at [string_interner.rs](crates/typedlua-core/src/string_interner.rs) but is unused in production code. Integration will deduplicate repeated strings (identifiers, keywords, type names) and replace string comparisons with u32 comparisons.

**Implementation Tasks:**
- [ ] Thread `&mut StringInterner` from compilation entry point through lexer/parser/type checker
- [ ] Change `Token::Identifier(String)` → `Token::Identifier(StringId)` in [lexer/mod.rs](crates/typedlua-core/src/lexer/mod.rs)
- [ ] Change `TypeKind::Named(String)` → `TypeKind::Named(StringId)` in [ast/types.rs](crates/typedlua-core/src/ast/types.rs)
- [ ] Change `Symbol.name: String` → `Symbol.name: StringId` in [typechecker/symbol_table.rs](crates/typedlua-core/src/typechecker/symbol_table.rs)
- [ ] Intern common keywords and builtin type names at startup
- [ ] Add resolver methods where string display is needed (diagnostics, codegen)
- [ ] Update all Symbol/Type-related code to work with StringId
- [ ] Write tests for interning correctness and memory savings

**Expected Results:**
- Every repeated identifier (`local`, `function`, `User`, `string`, etc.) stored once as u32
- Symbol lookups use integer hash instead of string hash (faster)
- 30-50% memory reduction for identifier-heavy code

**Dependencies:** None - infrastructure complete

---

#### Arena Allocation Integration
**Effort:** 2-3 days | **Priority:** P0 | **Impact:** 15-20% parsing speedup, better cache locality

Infrastructure already exists at [arena.rs](crates/typedlua-core/src/arena.rs) with bumpalo but is only used in tests. Integration will replace individual `Box<T>` allocations with bulk arena allocation for AST nodes.

**Implementation Tasks:**
- [ ] Thread `&'arena Arena` lifetime parameter through parser
- [ ] Change AST node types from `Box<Statement>` → `&'arena Statement`
- [ ] Change AST node types from `Box<Expression>` → `&'arena Expression`
- [ ] Change AST node types from `Box<Type>` → `&'arena Type`
- [ ] Replace all `Box::new(...)` calls with `arena.alloc(...)`
- [ ] Create arena at compilation entry point, pass through pipeline
- [ ] Update parser methods to accept `&'arena Arena` parameter
- [ ] Update type checker to work with arena-allocated AST
- [ ] Consider arena for temporary types during type checking
- [ ] Write benchmarks comparing Box vs Arena performance

**Expected Results:**
- Single bulk allocation instead of thousands of individual heap allocations
- Better CPU cache locality (nodes allocated contiguously)
- 15-20% parsing speedup per existing benchmarks
- Faster deallocation (entire arena dropped at once)

**Dependencies:** None - infrastructure complete

---

#### Aggressive Inline Annotations
**Effort:** 2-4 hours | **Priority:** P1 | **Impact:** 5-10% speedup on hot paths

Add `#[inline]` annotations to small, frequently-called functions that cross crate boundaries. Compiler can still choose to ignore hints, but enables cross-crate inlining with LTO.

**Implementation Tasks:**
- [ ] Add `#[inline]` to [span.rs](crates/typedlua-core/src/span.rs) methods: `len()`, `is_empty()`, `merge()`, `combine()`
- [ ] Add `#[inline]` to [lexer/token.rs](crates/typedlua-core/src/lexer/token.rs) predicates (if any exist)
- [ ] Add `#[inline]` to parser helper methods: `check()`, `match_token()`, `is_at_end()`, `peek()`
- [ ] Add `#[inline]` to small type comparison helpers in type checker
- [ ] Add `#[inline]` to frequently-called accessor methods (≤5 lines, hot path)
- [ ] Avoid inlining large functions (>10 lines) to prevent binary bloat
- [ ] Profile with `cargo flamegraph` to identify additional hot paths

**Rule of Thumb:**
- Function ≤5 lines + called frequently + crosses crate boundary → `#[inline]`
- Compiler can ignore hints if it would hurt performance

**Expected Results:**
- Cross-crate inlining on hot paths (especially with LTO enabled)
- 5-10% speedup on tight loops (lexing, type comparison)

**Dependencies:** None

---

#### Supply Chain Security (cargo-deny)
**Effort:** 5 minutes | **Priority:** P1 | **Impact:** Continuous dependency auditing

Add cargo-deny for automated scanning of dependencies for security vulnerabilities, license violations, and unmaintained crates.

**Implementation Tasks:**
- [ ] Create `deny.toml` configuration file in repo root
- [ ] Configure advisory checks (RustSec vulnerability database)
- [ ] Configure license checks (ensure all deps are MIT/Apache-2.0 compatible)
- [ ] Configure ban checks (prevent known-bad crates)
- [ ] Configure source checks (only allow crates.io, github.com)
- [ ] Add `cargo deny check` to CI pipeline
- [ ] Document in README how to run locally

**Expected Results:**
- Automatic alerting on vulnerable dependencies
- License compliance verification
- Protection against typosquatting attacks

**Dependencies:** None

---

#### Undefined Behavior Detection (miri)
**Effort:** 10 minutes | **Priority:** P1 | **Impact:** Safety insurance, no production overhead

Add miri to CI for detecting undefined behavior in test suite. Runs tests in slow interpreter that catches UB the compiler misses.

**Implementation Tasks:**
- [ ] Add CI job for `cargo +nightly miri test`
- [ ] Configure to run on nightly schedule (not every commit, since it's slow)
- [ ] Document in CONTRIBUTING.md how to run locally
- [ ] Fix any UB issues discovered (likely none, minimal unsafe code)

**Expected Results:**
- Detection of use-after-free, uninitialized memory, data races
- Zero production performance impact (test-time only)
- Insurance against future unsafe code bugs

**Dependencies:** None

---

#### Cow for Error Messages (Optional)
**Effort:** 1 day | **Priority:** P2 | **Impact:** Minor memory optimization

Use `Cow<'static, str>` for error messages to avoid allocating static strings.

**Implementation Tasks:**
- [ ] Change diagnostic messages to use `Cow::Borrowed` for static strings
- [ ] Change diagnostic messages to use `Cow::Owned(format!(...))` for dynamic messages
- [ ] Apply to parser error messages
- [ ] Apply to type checker error messages
- [ ] Apply to type display/formatting

**Expected Results:**
- Avoid allocations for common error messages
- Minor memory savings (less impactful than string interning)

**Dependencies:** None

---

#### Index-Based Module Graph (Optional)
**Effort:** 2-3 days | **Priority:** P2 | **Impact:** Cleaner dependency tracking

Replace PathBuf-based `ModuleId` with numeric indices stored in a central `Vec<Module>`. Makes mutation easier and avoids borrow checker issues with graph-like structures.

**Implementation Tasks:**
- [ ] Create `ModuleId` as `usize` or newtype wrapper
- [ ] Store all modules in `Vec<Module>` in registry
- [ ] Change dependencies from `Vec<PathBuf>` → `Vec<ModuleId>`
- [ ] Update module resolver to work with indices
- [ ] Update cache manifest to serialize/deserialize module graph

**Expected Results:**
- Easier mutation of module graph (no lifetime issues)
- Cleaner dependency tracking
- Easier serialization for incremental compilation

**Dependencies:** None

---

#### salsa Framework Integration
**Effort:** 2-3 weeks | **Priority:** P1 | **Impact:** Fine-grained incremental compilation for CLI + LSP

**Decision:** APPROVED - TypedLua LSP is sophisticated enough (10+ IDE features) to benefit from fine-grained incremental recomputation. Manual file-level caching helps CLI but not LSP keystroke responsiveness.

**Why salsa:**
- ✅ LSP has hover, completion, diagnostics, rename, references, etc. (IDE-first toolchain)
- ✅ Manual caching: CLI optimization (switching files), LSP bottleneck remains (editing within file)
- ✅ salsa: Function-level granularity - editing line 50 doesn't invalidate entire 5000-line file
- ✅ Battle-tested by rust-analyzer (100K+ line crates with instant responses)
- ✅ Automatic dependency tracking (no manual invalidation logic)

**Implementation Plan:**

**Phase 1: Add salsa Dependency & Core Database (Week 1)**
- [ ] Add `salsa = "0.17"` to `Cargo.toml`
- [ ] Create `crates/typedlua-core/src/db/mod.rs` - Database trait
- [ ] Create `crates/typedlua-core/src/db/inputs.rs` - Input queries (source text)
- [ ] Create `crates/typedlua-core/src/db/queries.rs` - Query group definitions
- [ ] Define `#[salsa::input]` for source files: `source_text(FileId) -> Arc<String>`
- [ ] Define `#[salsa::tracked]` for parsing: `parse(FileId) -> Arc<Program>`
- [ ] Define `#[salsa::tracked]` for type checking: `type_check(FileId) -> Arc<TypeCheckResult>`
- [ ] Create database struct implementing all query groups

**Phase 2: Integrate with Parser & Type Checker (Week 1-2)**
- [ ] Modify lexer to work with salsa inputs
- [ ] Modify parser to be pure function: `fn parse(db: &dyn Db, file: FileId) -> Arc<Program>`
- [ ] Modify type checker to be pure function: `fn type_check(db: &dyn Db, file: FileId) -> Arc<TypeCheckResult>`
- [ ] Handle imports/dependencies: `#[salsa::tracked] fn dependencies(FileId) -> Vec<FileId>`
- [ ] Ensure all queries are deterministic (no global state)

**Phase 3: CLI Integration (Week 2)**
- [ ] Create `RootDatabase` in CLI entry point
- [ ] Set input queries from file system
- [ ] Call `db.type_check(file)` instead of direct calls
- [ ] salsa automatically handles caching/invalidation
- [ ] Test: Modify one file → only recompiles dependents

**Phase 4: LSP Integration (Week 2-3)**
- [ ] Integrate `RootDatabase` into LSP server state
- [ ] On file change: `db.set_source_text(file, new_text)`
- [ ] Diagnostics: `db.type_check(file)` (auto-cached)
- [ ] Hover: `db.type_at_position(file, pos)` (add granular query)
- [ ] Completion: `db.symbols_at_position(file, pos)` (add granular query)
- [ ] Test: Type character → only re-analyzes changed function

**Phase 5: Fine-Grained Queries (Week 3+)**
- [ ] `#[salsa::tracked] fn symbol_at_position(FileId, Position) -> Option<Symbol>`
- [ ] `#[salsa::tracked] fn type_of_symbol(FileId, SymbolId) -> Type`
- [ ] `#[salsa::tracked] fn references_to_symbol(SymbolId) -> Vec<(FileId, Span)>`
- [ ] Enable sub-file invalidation (editing function foo doesn't invalidate bar)

**Testing:**
- [ ] Unit tests: Verify salsa invalidation works correctly
- [ ] Integration tests: Modify file, verify only dependents recompiled
- [ ] LSP tests: Edit file, verify hover on unrelated symbol is instant (cached)
- [ ] Performance benchmarks: Measure LSP keystroke latency on large files

**Expected Results:**
- CLI: 5-10x speedup on single-file changes (file-level caching)
- LSP: 10-50x speedup on keystroke response (function-level caching)
- Large files (5000+ lines): No noticeable lag when editing single function
- Hover/completion: Instant even during active typing (cached queries)

**Migration Strategy:**
- Keep existing incremental cache plan as reference (good design ideas)
- salsa replaces manual manifest/invalidation with framework
- Existing cache infrastructure (hashing, serialization) may still be useful for persistent caching

**Dependencies:** None - can start immediately

---

### Code Quality Improvements (Low Priority - Polish)

**Status:** Optional refactoring for consistency - current code is production-ready

#### Iterator and Functional Style Consistency
**Effort:** 1-2 days | **Priority:** P3 | **Impact:** Code consistency, no functional changes

The codebase currently uses a pragmatic mix of functional (iterator chains) and imperative (for loops with push) patterns. This is acceptable but could be more consistent.

**Current State (Good - 70/100):**
- ✅ Strong iterator adoption: ~173 `.iter()` calls, ~194 closures
- ✅ Good use of `.map()` (66), `.collect()` (82), `.any()` (40), `.filter()` (14)
- ✅ Appropriate functional patterns in utility_types.rs, diagnostics, cache
- ⚠️ Mixed patterns in parser and type checker (some functional, some imperative)
- ⚠️ Underutilized: `.fold()` (0 uses), `.flat_map()` (1 use), `.find()` (3 uses)

**Potential Improvements (Not Required):**
- [ ] Replace imperative Vec building with `.iter().map().collect()` patterns
  - Locations: `parser/statement.rs`, `typechecker/type_checker.rs`, `utility_types.rs`
  - Example: `let mut v = Vec::new(); for x in items { v.push(f(x)); }` → `items.iter().map(f).collect()`
- [ ] Use `.fold()` for accumulation patterns instead of mutable accumulators
- [ ] Use `.flat_map()` for nested iteration instead of nested for-loops
- [ ] Replace index-based loops with `.enumerate()` or `.windows()` where appropriate

**Note:** Do NOT over-refactor:
- Parser needs imperative loops for error recovery complexity ✅
- Type checker has complex control flow with early returns ✅
- Imperative code with `push()` is NOT inherently bad ✅
- Readability > dogmatic functional style ✅
- Performance difference is negligible ✅

**Guideline for New Code:**
- Simple transformations → `.iter().map().collect()`
- Boolean checks → `.any()`, `.all()`
- Finding items → `.find()`, `.find_map()`
- Accumulation → `.fold()` instead of `let mut`
- Complex state management → Imperative for-loops are fine

### Publishing
- [ ] Publish VS Code extension to marketplace (see `editors/vscode/PUBLISHING.md`)

---
