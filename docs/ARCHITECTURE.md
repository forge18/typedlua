# TypedLua Architecture

**Version:** 2.0
**Last Updated:** 2026-01-23

## Table of Contents

- [Executive Summary](#executive-summary)
- [Implementation Status](#implementation-status)
- [System Architecture](#system-architecture)
- [Component Design](#component-design)
- [Runtime Extensions Architecture](#runtime-extensions-architecture)
- [Data Flow](#data-flow)
- [Extension Points](#extension-points)
- [Known Limitations](#known-limitations)
- [Technology Stack](#technology-stack)
- [Architectural Decisions](#architectural-decisions)
- [Design Patterns](#design-patterns)
- [Future Considerations](#future-considerations)
- [Architectural Refactoring Plans](#architectural-refactoring-plans)

---

## Executive Summary

TypedLua is a typed superset of Lua implemented in Rust, providing static type checking with gradual typing inspired by TypeScript. The compiler transforms typed Lua code into plain Lua while ensuring type safety at compile time.

### Implementation Status at a Glance

| Component          | Status          | Details                                                                       |
|--------------------|-----------------|-------------------------------------------------------------------------------|
| **Lexer**          | ✅ Complete      | 854 lines, 40+ token types, template strings, string interning                |
| **Parser**         | ✅ Complete      | Trait-based, 22 statement types, 22 expression kinds, error recovery          |
| **Type Checker**   | ✅ Core Complete | 3,544 lines, generics, narrowing, 12 utility types; language features pending |
| **Code Generator** | ✅ Complete      | Lua 5.1-5.4 targets, bundling mode, source maps                               |
| **Optimizer**      | ✅ O1 Complete   | 15 passes registered; All 5 O1 passes now implement transformations           |
| **CLI**            | ✅ Complete      | Watch mode, config files, parallel compilation via rayon                      |
| **LSP**            | ⚠️ Partial       | Core features working; member completion stubbed                              |

### Language Features Status

| Feature                | Status         | Details                                             |
|------------------------|----------------|-----------------------------------------------------|
| Exception Handling     | ❌ Tokens only | try/catch/throw tokens exist, no AST/parser/codegen |
| Safe Navigation (`?.`) | ❌ Tokens only | Token exists; tests disabled; no AST/parser/codegen |
| Null Coalescing (`??`) | ❌ Tokens only | Token exists; tests ignored; no AST/parser/codegen  |
| Operator Overloading   | ❌ Tokens only  | Token exists, no implementation                     |
| Rich Enums             | ❌ Not started  | Basic enums only (name + value)                     |
| Interface Defaults     | ❌ Not started  | No DefaultMethod in InterfaceMember                 |
| File Namespaces        | ❌ Tokens only  | Only DeclareNamespace for .d.tl files               |

### Core Design Principles

1. **Dependency Injection** - All components receive dependencies through constructors, enabling testability
2. **Single Responsibility** - Each module has one clear, well-defined purpose
3. **Zero Runtime Overhead** - All type information is erased during compilation
4. **Gradual Typing** - Types are optional, allowing incremental adoption
5. **Trait-Based Abstraction** - Use traits for flexibility and mockability

---

## Implementation Status

### Lexer

| Feature            | Status | Notes                                                   |
|--------------------|--------|---------------------------------------------------------|
| Basic tokens       | ✅      | Keywords (40+), operators, literals                     |
| Template strings   | ✅      | Backtick syntax with `${}` interpolation                |
| String interning   | ✅      | Memory-efficient identifiers via `StringId`             |
| Number literals    | ✅      | Decimal, hex (`0x`), binary (`0b`), scientific notation |
| Multi-line strings | ✅      | `[[...]]` Lua-style long strings                        |
| Comments           | ✅      | Single-line (`--`) and multi-line (`--[[ ]]`)           |

### Parser

| Feature                | Status | Notes                                                                        |
|------------------------|--------|------------------------------------------------------------------------------|
| Statements (22 types)  | ✅      | Variable, function, class, interface, enum, control flow, declare statements |
| Expressions (22 kinds) | ✅      | Binary, unary, calls, member access, match, pipe, template, etc.             |
| Pattern matching       | ✅      | Match expressions with exhaustiveness checking                               |
| Destructuring          | ✅      | Array and object destructuring patterns                                      |
| Type annotations       | ✅      | Full TypeScript-style type syntax                                            |
| Error recovery         | ✅      | Synchronizes on statement boundaries                                         |
| Try/Catch              | ❌      | Tokens exist but no AST types or parsing                                     |
| Safe Navigation        | ❌      | No OptionalMember/OptionalIndex/OptionalCall in AST                          |
| Null Coalescing        | ❌      | No NullCoalesce in BinaryOp enum                                             |

### Type System

| Feature              | Status | Notes                                                                       |
|----------------------|--------|-----------------------------------------------------------------------------|
| Primitives           | ✅      | `nil`, `boolean`, `number`, `integer`, `string`, `unknown`, `never`, `void` |
| Unions               | ✅      | `T \| U` with proper narrowing                                              |
| Intersections        | ✅      | `T & U` for combining types                                                 |
| Arrays               | ✅      | `T[]` and `Array<T>` syntax                                                 |
| Tuples               | ✅      | `[T, U, V]` fixed-length arrays                                             |
| Functions            | ✅      | `(x: T) -> U` with overloads                                                |
| Generics             | ✅      | Constraints (`extends`), defaults, inference                                |
| Interfaces           | ✅      | Properties, methods, index signatures, inheritance                          |
| Classes              | ✅      | Access modifiers, inheritance, abstract, final, override                    |
| Primary Constructors | ✅      | `class Point(public x: number)` compact syntax                              |
| Type Guards          | ✅      | `param is Type` predicates with narrowing                                   |
| Type Narrowing       | ✅      | `typeof`, nil checks, truthiness, instanceof                                |
| Utility Types (12)   | ✅      | Partial, Required, Readonly, Pick, Omit, Exclude, Extract, etc.             |
| Decorators           | ✅      | Class, property, method decorators with runtime support                     |
| Enums (basic)        | ✅      | Numeric and string enums (name + value only)                                |
| Rich Enums           | ❌      | No fields, constructors, or methods in EnumDeclaration                      |
| Interface Defaults   | ❌      | No DefaultMethod variant in InterfaceMember enum                            |
| Conditional Types    | ⚠️      | Parsed and basic evaluation; not fully integrated                           |
| Mapped Types         | ⚠️      | Parsed and basic evaluation; limited recursion                              |

### Code Generation

| Feature            | Status | Notes                                      |
|--------------------|--------|--------------------------------------------|
| Lua 5.1 target     | ✅      | Bitwise ops emulated with library          |
| Lua 5.2 target     | ✅      | `bit32` library for bitwise                |
| Lua 5.3 target     | ✅      | Native bitwise, integer division           |
| Lua 5.4 target     | ✅      | `const`, to-be-closed variables            |
| Bundling mode      | ✅      | Single-file output with `__modules` system |
| Source maps        | ✅      | VLQ encoding, multi-source support         |
| Decorator runtime  | ✅      | Embedded when decorators detected          |
| Continue statement | ✅      | Emulated with labels/goto for older Lua    |

### LSP Features

| Feature               | Status | Notes                                      |
|-----------------------|--------|--------------------------------------------|
| Diagnostics           | ✅      | Full lex → parse → typecheck pipeline      |
| Hover                 | ✅      | Type info with markdown documentation      |
| Completion (keywords) | ✅      | Context-aware keyword suggestions          |
| Completion (members)  | ⚠️      | Infrastructure exists but not populated    |
| Go-to-definition      | ✅      | Local + cross-file via symbol index        |
| Find references       | ✅      | Workspace-wide with cross-file support     |
| Rename                | ✅      | Multi-file with export awareness           |
| Document symbols      | ✅      | Nested outline with all declaration types  |
| Workspace symbols     | ✅      | Case-insensitive search                    |
| Formatting            | ✅      | Full document, range, and on-type          |
| Signature help        | ✅      | Parameter hints with active parameter      |
| Inlay hints           | ✅      | Type hints and parameter name hints        |
| Code actions          | ✅      | Quick fixes, refactoring, organize imports |
| Semantic tokens       | ✅      | 13 token types, 6 modifiers                |
| Selection range       | ✅      | AST-based selection expansion              |
| Folding range         | ✅      | Block-based folding                        |

---

## System Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI Layer                               │
│  Entry point, argument parsing, configuration loading           │
│  Crates: typedlua-cli                                           │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Compiler Pipeline                            │
│  Orchestrates compilation phases via DI container               │
│                                                                 │
│    ┌───────────────────────────────────────────────┐           │
│    │  Dependency Injection Container               │           │
│    │  • Configuration (Arc<CompilerConfig>)        │           │
│    │  • Diagnostic Handler (Arc<dyn Trait>)        │           │
│    │  • File System (Arc<dyn Trait>)               │           │
│    │  • String Interner (Arena-based)              │           │
│    │  • AST Arena (Bump allocation)                │           │
│    └───────────────────────────────────────────────┘           │
│                                                                 │
└────────────────────────────┬────────────────────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        ▼                    ▼                    ▼
    ┌────────┐          ┌────────┐          ┌──────────┐
    │ Lexer  │─────────▶│ Parser │─────────▶│   Type   │
    │        │  Tokens  │        │   AST    │ Checker  │
    └────────┘          └────────┘          └──────────┘
                                                  │
                                                  │ Typed AST
                                                  ▼
                                           ┌──────────┐
                                           │   Code   │
                                           │Generator │
                                           └──────────┘
                                                  │
                                                  │ Lua Code + SourceMap
                                                  ▼
                                           ┌──────────┐
                                           │  Output  │
                                           └──────────┘
```

### Crate Structure

```text
typed-lua/
├── crates/
│   ├── typedlua-core/      # Compiler core (lexer, parser, type checker, codegen)
│   ├── typedlua-cli/       # Command-line interface
│   └── typedlua-lsp/       # Language Server Protocol implementation
├── editors/
│   └── vscode/             # VS Code extension
└── docs/                   # Documentation
```

---

## Component Design

### Core Module: `typedlua-core`

The core crate contains all compilation logic, organized into focused modules:

#### Module Organization

```rust
pub mod arena;              // Bump allocator for AST nodes
pub mod ast;                // Abstract Syntax Tree definitions
pub mod codegen;            // Lua code generation + source maps
pub mod config;             // Configuration management
pub mod di;                 // Dependency injection container
pub mod diagnostics;        // Error reporting system
pub mod errors;             // Error types
pub mod fs;                 // File system abstraction
pub mod lexer;              // Lexical analysis (tokenization)
pub mod parser;             // Syntax analysis (AST construction)
pub mod span;               // Source location tracking
pub mod stdlib;             // Standard library type definitions
pub mod string_interner;    // String deduplication
pub mod typechecker;        // Type checking and inference
```

### Dependency Injection Container

The DI container (`di.rs`) is the heart of the architecture, managing all shared dependencies:

```rust
pub struct Container {
    config: Arc<CompilerConfig>,
    diagnostic_handler: Arc<dyn DiagnosticHandler>,
    file_system: Arc<dyn FileSystem>,
}
```

**Key Features:**

- **Production Constructor**: `Container::new(config)` - Creates real implementations
- **Test Constructor**: `Container::with_dependencies(...)` - Injects mocks
- **Shared Ownership**: Uses `Arc` for efficient sharing across components
- **Trait-Based**: All dependencies are traits, enabling mockability

### Lexer

**Location:** `crates/typedlua-core/src/lexer/`

Converts source code into a stream of tokens.

**Key Components:**

- `Token`: Represents lexical units (keywords, identifiers, operators, literals)
- `TokenKind`: Enum of all possible token types (40+ variants)
- `Span`: Source location tracking (line, column, byte offset)
- `StringInterner`: Deduplicates identifiers for memory efficiency

**Features:**

- Context-aware tokenization (respects TypedLua vs Lua syntax)
- Comprehensive error reporting via diagnostic handler
- Support for multi-line strings, comments, and template literals
- String interning for fast identifier comparison

### Parser

**Location:** `crates/typedlua-core/src/parser/`

Builds an Abstract Syntax Tree (AST) from tokens using a trait-based design.

**Structure:**

- `mod.rs`: Core parser logic and orchestration
- `expression.rs`: Expression parsing via `ExpressionParser` trait (864 lines)
- `statement.rs`: Statement parsing via `StatementParser` trait (1,597 lines)
- `types.rs`: Type annotation parsing via `TypeParser` trait (375 lines)
- `pattern.rs`: Pattern matching and destructuring

**Features:**

- Recursive descent parser with operator precedence climbing
- Trait-based design for modularity and extensibility
- Feature flag support (OOP, FP, decorators)
- Comprehensive error recovery with `synchronize()` method

### Type Checker

**Location:** `crates/typedlua-core/src/typechecker/`

Performs static type analysis and inference.

**Structure:**

- `type_checker.rs`: Main type checking orchestration (3,544 lines)
- `type_environment.rs`: Type variable environments and scoping
- `symbol_table.rs`: Symbol resolution and storage (444 lines)
- `type_compat.rs`: Type compatibility and subtyping rules
- `generics.rs`: Generic type instantiation and constraints (605 lines)
- `narrowing.rs`: Control flow-based type narrowing (590 lines)
- `utility_types.rs`: Built-in utility types (1,360 lines)

**Features:**

- Bidirectional type inference
- Structural typing for tables
- Union/intersection type support
- Generic type instantiation with inference
- Control flow analysis for narrowing
- Multi-return function handling
- Access modifier enforcement (public, private, protected)

### Code Generator

**Location:** `crates/typedlua-core/src/codegen/`

Transforms typed AST into executable Lua code.

**Structure:**

- `mod.rs`: Core code generation logic (3,120 lines)
- `sourcemap.rs`: Source map generation for debugging

**Features:**

- Type erasure (zero runtime overhead)
- Target Lua version support (5.1-5.4) with version-specific adaptations
- Source map generation with VLQ encoding
- Bundle mode for single-file output
- Decorator runtime embedding when needed
- Readable output (preserves structure where possible)

### Optimizer

**Location:** `crates/typedlua-core/src/optimizer/`

The optimizer performs AST transformations to improve generated Lua code performance. It uses a configurable multi-level optimization system with 15 registered passes.

#### Structure

```
crates/typedlua-core/src/optimizer/
├── mod.rs                    # Optimizer orchestrator and pass registration
└── passes.rs                 # Individual optimization pass implementations
```

#### The `OptimizationPass` Trait

All optimization passes implement a common trait that defines the optimization interface:

```rust
pub trait OptimizationPass {
    fn name(&self) -> &'static str;
    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError>;
    fn min_level(&self) -> OptimizationLevel;
}
```

- `name()`: Returns a static identifier for debugging and logging
- `run()`: Performs the actual transformation; returns `true` if changes were made
- `min_level()`: Specifies the minimum optimization level required to run this pass

#### Optimization Levels

The optimizer supports four levels, controlled by the `OptimizationLevel` enum:

| Level | Description | Passes Run |
|-------|-------------|------------|
| **O0** | No optimizations | None |
| **O1** | Basic | 5 passes - constant folding, dead code elimination, algebraic simplification, table pre-allocation, global localization |
| **O2** | Standard | 5 additional passes - function inlining, loop optimization, string concatenation, dead store elimination, tail call optimization |
| **O3** | Aggressive | 5 additional passes - aggressive inlining, operator inlining, interface method inlining, devirtualization, generic specialization |

#### Pass Registration

The optimizer uses a fixed-point iteration strategy with a maximum of 10 iterations:

```rust
pub struct Optimizer {
    level: OptimizationLevel,
    handler: Arc<dyn DiagnosticHandler>,
    interner: Option<Arc<StringInterner>>,
    passes: Vec<Box<dyn OptimizationPass>>,
}

impl Optimizer {
    pub fn optimize(&mut self, program: &mut Program) -> Result<(), CompilationError> {
        if self.level == OptimizationLevel::O0 {
            return Ok(());
        }

        let mut iteration = 0;
        let max_iterations = 10;

        loop {
            let mut changed = false;
            iteration += 1;

            if iteration > max_iterations {
                break; // Safety limit
            }

            for pass in &mut self.passes {
                if pass.min_level() <= self.level {
                    let pass_changed = pass.run(program)?;
                    changed |= pass_changed;
                }
            }

            if !changed {
                break; // Fixed point reached
            }
        }

        Ok(())
    }
}
```

#### Implemented O1 Passes

**1. ConstantFoldingPass**
- Evaluates constant expressions at compile time
- Handles numeric literals, boolean expressions, simple arithmetic
- Example: `const x = 1 + 2 * 3` → `const x = 7`

**2. DeadCodeEliminationPass**
- Removes code after return/break/continue statements
- Truncates unreachable else blocks after early returns
- Eliminates empty branches in conditionals

**3. AlgebraicSimplificationPass**
- Simplifies expressions using algebraic identities
- Examples: `x + 0 → x`, `x * 1 → x`, `x * 0 → 0`, `true and x → x`

**4. TablePreallocationPass**
- Analyzes table constructors for size hints
- Generates `table.create(array_size, object_size)` for Lua 5.2+
- Helps Lua's memory allocator pre-size tables

**5. GlobalLocalizationPass** *(Recently Implemented)*
- Identifies frequently-used globals (>2 accesses)
- Creates local references to reduce repeated table lookups
- Example:
  ```lua
  -- Input
  local x = math.sin(1) + math.cos(2) + math.tan(3)

  -- After optimization
  local _math = math
  local x = _math.sin(1) + _math.cos(2) + _math.tan(3)
  ```
- Uses string interner to track global identifiers
- Respects local variable scope (doesn't localize already-declared locals)

#### O2/O3 Passes (Analysis-Only Placeholders)

The higher-level passes are scaffolded but currently perform analysis only:

- **FunctionInliningPass**: Would inline functions with ≤5 statements
- **LoopOptimizationPass**: Would convert `ipairs` to numeric loops
- **StringConcatOptimizationPass**: Would use `table.concat` for 3+ parts
- **DeadStoreEliminationPass**: Would remove redundant assignments
- **TailCallOptimizationPass**: Would detect and optimize tail calls
- **OperatorInliningPass**: Would inline simple operator overloads
- **InterfaceMethodInliningPass**: Would inline default interface methods
- **DevirtualizationPass**: Would resolve virtual method calls statically
- **GenericSpecializationPass**: Would specialize generic instantiations

#### Global Localization Implementation Details

The newly implemented global localization pass works as follows:

1. **Analysis Phase**: Traverse the AST, counting identifier usages
   - Track declared locals to avoid localizing variables already in scope
   - Build a usage map: `StringId → usage_count`

2. **Selection Phase**: Filter globals used more than 2 times
   - Threshold chosen empirically (local assignment overhead vs. lookup savings)
   - Results in a list of `(global_name, count)` pairs

3. **Transformation Phase**:
   - Create `local _name = name` declarations at block start
   - Replace remaining usages with the local reference
   - Handle all AST node types (expressions, statements, function bodies, control flow)

4. **Scope Awareness**: Properly handles:
   - Nested blocks and their local scopes
   - Loop variables (for numeric and generic loops)
   - Function parameters
   - Destructuring patterns

#### Integration with Code Generator

The optimizer runs between type checking and code generation:

```
Lexer → Parser → Type Checker → Optimizer → Code Generator → Lua Output
```

The optimizer receives a `Program` (typed AST) and returns a transformed `Program` that the code generator then emits as Lua.

#### Future Enhancements

| Enhancement | Description |
|-------------|-------------|
| **Profile-Guided Optimization** | Use runtime profiling data to guide optimization decisions |
| **Cross-Module Inlining** | Inline functions across module boundaries |
| **Escape Analysis** | Determine when allocations can be stack-allocated |
| **Constant Propagation** | Propagate known values through the program |

### Diagnostic System

**Location:** `crates/typedlua-core/src/diagnostics.rs`

Comprehensive error reporting with LSP compatibility.

**Components:**

- `DiagnosticHandler` trait: Interface for reporting errors/warnings
- `Diagnostic`: Error/warning with code, message, span, and suggestions
- `ConsoleDiagnosticHandler`: Pretty terminal output with colors
- `CollectingDiagnosticHandler`: Collects diagnostics for testing/LSP

**Features:**

- Structured error codes (e.g., `E0001`, `W0042`)
- Multi-level diagnostics (error, warning, info, hint)
- Related information (secondary spans)
- Quick-fix suggestions
- Pretty printing with color and source snippets

### LSP Server

**Location:** `crates/typedlua-lsp/src/`

Full-featured Language Server Protocol implementation.

**Structure:**

- `main.rs`: LSP server entry point and capability advertisement
- `message_handler.rs`: Request/notification router
- `document.rs`: Document management with AST caching
- `symbol_index.rs`: Cross-file symbol tracking
- `providers/`: Feature implementations (14 provider modules)

**Provider Pattern:**

Each LSP feature has a dedicated provider with stateless methods:

```rust
pub struct HoverProvider;

impl HoverProvider {
    pub fn provide(&self, document: &Document, position: Position) -> Option<Hover> {
        // Implementation
    }
}
```

---

## Runtime Extensions Architecture

TypedLua extends Lua with modern language features that don't exist in base Lua or its standard library. These extensions require **runtime support code** that gets embedded in the generated Lua output.

### What Qualifies as a Runtime Extension?

Any feature beyond base Lua 5.1-5.4 specification requires runtime support:

| Feature Category | Examples | Runtime Requirement |
|------------------|----------|---------------------|
| **Class System** | Classes, inheritance, `instanceof` | Constructor functions, prototype chains, type checking |
| **Decorators** | `@readonly`, `@deprecated`, custom decorators | Decorator runtime with metadata handling |
| **Advanced Operators** | Safe navigation (`?.`), null coalescing (`??`) | IIFE wrappers, nil-checking helpers |
| **Exception Handling** | `try/catch/finally`, error chaining (`!!`) | pcall/xpcall wrappers, error type checking |
| **Rich Enums** | Enums with fields, methods, constructors | Enum factories, lookup tables, metamethods |
| **Reflection** | `Runtime.typeof()`, `Runtime.getFields()` | Type registry, metadata tables |
| **Operator Overloading** | `operator +`, `operator ==` | Metatable setup, metamethod binding |
| **Interface Defaults** | Default method implementations | Method copying, inheritance chains |

### The `typedlua-runtime` Crate

**Status:** Planned (not yet created)

**Purpose:** Separate crate containing all Lua runtime support code for language extensions.

**Structure:**

```
crates/typedlua-runtime/
├── src/
│   ├── lib.rs                    # Re-exports all modules
│   ├── classes.rs                # Class system runtime
│   ├── decorators.rs             # Decorator runtime
│   ├── exceptions.rs             # Exception handling (pcall/xpcall wrappers)
│   ├── operators.rs              # Safe nav, null coalesce helpers
│   ├── enums.rs                  # Rich enum support
│   └── reflection.rs             # Reflection metadata
└── lua/                          # Lua source snippets
    ├── class_runtime.lua
    ├── decorator_runtime.lua
    ├── exception_runtime.lua
    ├── operator_helpers.lua
    ├── enum_runtime.lua
    └── reflection_runtime.lua
```

**Integration Pattern:**

```rust
// In typedlua-runtime/src/decorators.rs
pub const DECORATOR_RUNTIME: &str = include_str!("../lua/decorator_runtime.lua");

// In codegen/mod.rs
use typedlua_runtime::decorators::DECORATOR_RUNTIME;

impl CodeGenerator {
    fn emit_decorator_runtime(&mut self) {
        if self.uses_built_in_decorators {
            self.output.push_str(DECORATOR_RUNTIME);
        }
    }
}
```

**Benefits:**

1. **Modularity** - Each feature's runtime is isolated and independently testable
2. **Versioning** - Runtime snippets can be versioned per Lua target (5.1, 5.2, 5.3, 5.4)
3. **Zero-Cost** - Only include runtime code for features actually used
4. **Maintainability** - Lua code lives in `.lua` files, not Rust strings
5. **Testability** - Runtime snippets can be unit tested in isolation

**Conditional Inclusion:**

The code generator tracks which features are used and only embeds necessary runtime:

```rust
struct CodeGenerator {
    uses_decorators: bool,
    uses_safe_navigation: bool,
    uses_null_coalescing: bool,
    uses_exceptions: bool,
    uses_rich_enums: bool,
    // ... etc
}
```

Only features with `uses_*: true` have their runtime included in output.

---

## Data Flow

### Compilation Pipeline

1. **Source Input** → Raw TypedLua source code
2. **Lexer** → Tokenizes source into `Vec<Token>` with string interning
3. **Parser** → Builds `Program` (AST root) in arena
4. **Type Checker** → Validates types, performs inference, produces `TypeEnvironment`
5. **Code Generator** → Transforms typed AST to Lua string + source map
6. **Output** → Writes Lua file(s) and source maps

### Error Handling Flow

All phases report errors through the `DiagnosticHandler`:

```
Component → diagnostic_handler.error(span, message)
                      ↓
         Diagnostic stored in handler
                      ↓
         Container checks has_errors()
                      ↓
         Pipeline aborts if errors exist
                      ↓
         CLI displays errors and exits
```

---

## Extension Points

This section documents how to extend TypedLua with new features.

### Adding New Token Types

**Files to modify:**

1. [crates/typedlua-core/src/lexer/lexeme.rs](../crates/typedlua-core/src/lexer/lexeme.rs) - Add variant to `TokenKind` enum
2. [crates/typedlua-core/src/lexer/mod.rs](../crates/typedlua-core/src/lexer/mod.rs) - Add lexing logic in `scan_token()`

**Example:**

```rust
// In lexeme.rs
pub enum TokenKind {
    // ... existing variants
    NewKeyword,  // Add new variant
}

// In mod.rs, within scan_token()
"newkeyword" => TokenKind::NewKeyword,
```

### Adding New Statements

**Files to modify:**

1. [crates/typedlua-core/src/ast/statement.rs](../crates/typedlua-core/src/ast/statement.rs) - Add variant to `Statement` enum
2. [crates/typedlua-core/src/parser/statement.rs](../crates/typedlua-core/src/parser/statement.rs) - Implement `parse_*_statement()` method
3. [crates/typedlua-core/src/typechecker/type_checker.rs](../crates/typedlua-core/src/typechecker/type_checker.rs) - Add `check_*_statement()` method
4. [crates/typedlua-core/src/codegen/mod.rs](../crates/typedlua-core/src/codegen/mod.rs) - Add `generate_*_statement()` method

**Pattern:**

```rust
// 1. AST definition
pub enum Statement {
    NewStatement(NewStatementData),
}

// 2. Parser
fn parse_new_statement(&mut self) -> Result<Statement, ()> { ... }

// 3. Type checker
fn check_new_statement(&mut self, stmt: &NewStatementData) -> Result<(), ()> { ... }

// 4. Code generator
Statement::NewStatement(data) => self.generate_new_statement(data),
```

### Adding New Expressions

**Files to modify:**

1. [crates/typedlua-core/src/ast/expression.rs](../crates/typedlua-core/src/ast/expression.rs) - Add variant to `ExpressionKind` enum
2. [crates/typedlua-core/src/parser/expression.rs](../crates/typedlua-core/src/parser/expression.rs) - Add parsing logic
3. [crates/typedlua-core/src/typechecker/type_checker.rs](../crates/typedlua-core/src/typechecker/type_checker.rs) - Add type inference logic
4. [crates/typedlua-core/src/codegen/mod.rs](../crates/typedlua-core/src/codegen/mod.rs) - Add code generation

### Adding New Types

**Files to modify:**

1. [crates/typedlua-core/src/ast/types.rs](../crates/typedlua-core/src/ast/types.rs) - Add variant to `Type` enum
2. [crates/typedlua-core/src/parser/types.rs](../crates/typedlua-core/src/parser/types.rs) - Add parsing logic
3. [crates/typedlua-core/src/typechecker/type_compat.rs](../crates/typedlua-core/src/typechecker/type_compat.rs) - Add compatibility rules

### Adding New Utility Types

**File to modify:**

- [crates/typedlua-core/src/typechecker/utility_types.rs](../crates/typedlua-core/src/typechecker/utility_types.rs)

**Pattern:**

```rust
pub fn evaluate_utility_type(&self, name: &str, args: &[Type]) -> Option<Type> {
    match name {
        "NewUtility" => self.evaluate_new_utility(args),
        // ... existing cases
    }
}

fn evaluate_new_utility(&self, args: &[Type]) -> Option<Type> {
    // Implementation
}
```

### Adding LSP Features

**Files to modify:**

1. Create new provider in [crates/typedlua-lsp/src/providers/](../crates/typedlua-lsp/src/providers/)
2. Export from [crates/typedlua-lsp/src/providers/mod.rs](../crates/typedlua-lsp/src/providers/mod.rs)
3. Add field to `MessageHandler` in [crates/typedlua-lsp/src/message_handler.rs](../crates/typedlua-lsp/src/message_handler.rs)
4. Handle request in `MessageHandler::handle_request()`
5. Advertise capability in [crates/typedlua-lsp/src/main.rs](../crates/typedlua-lsp/src/main.rs)

**Pattern:**

```rust
// 1. New provider file
pub struct NewFeatureProvider;

impl NewFeatureProvider {
    pub fn new() -> Self { Self }

    pub fn provide(&self, document: &Document, params: Params) -> Option<Result> {
        // Implementation
    }
}

// 2. Export in mod.rs
pub mod new_feature;
pub use new_feature::NewFeatureProvider;

// 3. Add to MessageHandler
pub struct MessageHandler {
    new_feature_provider: NewFeatureProvider,
}

// 4. Handle request
match Self::cast_request::<NewRequest>(req.clone()) {
    Ok((id, params)) => {
        let result = self.new_feature_provider.provide(doc, params);
        connection.send_response(Response::new_ok(id, result))?;
    }
    Err(req) => req,
};

// 5. Advertise in main.rs
server_capabilities.new_feature_provider = Some(NewFeatureOptions { ... });
```

---

## Known Limitations

### Language Features Not Implemented

These features have lexer tokens but no AST, parser, type checker, or codegen support:

| Feature                   | AST Types | Parser | TypeChecker | Codegen | Notes                                         |
|---------------------------|-----------|--------|-------------|---------|-----------------------------------------------|
| Exception Handling        | ❌         | ❌      | ❌           | ❌       | try/catch/throw/finally tokens only           |
| Safe Navigation (`?.`)    | ❌         | ❌      | ❌           | ❌       | Tokens + tests exist; parser skips token      |
| Null Coalescing (`??`)    | ❌         | ❌      | ❌           | ❌       | Tokens + tests exist; parser skips token      |
| Operator Overloading      | ❌         | ❌      | ❌           | ❌       | `operator` token only                         |
| Rich Enums                | ❌         | ❌      | ❌           | ❌       | Enum has name+value only, no methods          |
| Interface Default Methods | ❌         | ❌      | ❌           | ❌       | No DefaultMethod in InterfaceMember           |
| File Namespaces           | ❌         | ❌      | ❌           | ❌       | Only DeclareNamespace for .d.tl               |

### Optimizer Limitations

| Level | Status | Details                                                           |
|-------|--------|-------------------------------------------------------------------|
| O1    | ✅      | All 5 passes working: constant folding, DCE, algebraic, table pre-allocation, global localization |
| O2    | ⚠️      | All 5 passes are analysis-only placeholders                       |
| O3    | ⚠️      | All 5 passes are analysis-only placeholders                       |

### LSP Limitations

| Feature           | Status | Details                                         |
|-------------------|--------|-------------------------------------------------|
| Member completion | ❌      | Infrastructure exists but returns empty results |
| Import completion | ❌      | Not implemented                                 |
| Method completion | ❌      | Not implemented                                 |

### TypeScript Features Not Supported

| Feature                         | Notes                                        |
|---------------------------------|----------------------------------------------|
| `any` type                      | By design - use `unknown` instead for safety |
| `as const` assertions           | Not implemented                              |
| `satisfies` operator            | Not implemented                              |
| Variance modifiers (`in`/`out`) | Not implemented                              |
| `keyof` (full)                  | Basic support only; no recursive resolution  |
| Branded/nominal types           | Not implemented                              |

### Infrastructure Not Implemented

| Feature                 | Status         | Notes                                          |
|-------------------------|----------------|------------------------------------------------|
| Incremental compilation | Not started    | Full recompile on every change                 |
| Arena allocation        | Not integrated | bumpalo exists but not used in parser pipeline |
| salsa integration       | Not started    | No fine-grained caching                        |

### Known Code Issues

1. **Type alias resolution**: Aliases don't resolve to underlying types for compatibility checking
2. **Panic in utility types**: Some edge cases use `panic!()` instead of `Result`
3. **Integration test failures**: error_classes_tests.rs has 6 failing tests (features not implemented)

---

## Technology Stack

### Language & Core Libraries

- **Rust 2021 Edition** - Implementation language
- **bumpalo** - Arena allocator for AST
- **rustc-hash** - Fast hash maps for symbol tables

### Serialization & Configuration

- **serde** - Serialization framework
- **serde_yaml** - Configuration file parsing
- **serde_json** - JSON handling for LSP

### Error Handling

- **thiserror** - Ergonomic error type derivation
- **anyhow** - Error propagation utilities

### CLI & Tooling

- **clap** - Command-line argument parsing
- **notify** - File system watching (for watch mode)
- **tracing** - Structured logging

### LSP Implementation

- **lsp-server** - LSP message transport
- **lsp-types** - LSP type definitions
- **crossbeam-channel** - Multi-threading for LSP

### Testing & Benchmarking

- **insta** - Snapshot testing
- **criterion** - Benchmarking framework

### Parallel Processing

- **rayon** - Data parallelism (used in CLI for parallel compilation)

---

## Architectural Decisions

### ADR-001: Use Rust for Implementation

**Decision:** Implement TypedLua in Rust rather than C++, OCaml, or Haskell.

**Rationale:**

- Memory safety without garbage collection
- Excellent error handling with `Result<T, E>`
- Strong type system prevents many bugs
- Great tooling (cargo, rustfmt, clippy)
- Growing ecosystem for compiler tooling
- Cross-platform without hassle

### ADR-002: Dependency Injection via Explicit Container

**Decision:** Use manual dependency injection with a container rather than global state or a DI framework.

**Rationale:**

- Explicit dependencies make code easier to understand
- Trivial to mock for testing
- No magic or reflection needed
- Full compile-time type safety
- Performance (no runtime lookup)

### ADR-003: Arena Allocation for AST

**Decision:** Use bump allocation (arena) for AST nodes instead of `Box`/`Rc`.

**Rationale:**

- 10-100x faster allocation than `Box::new()`
- Minimal memory fragmentation
- Better cache locality (nodes allocated sequentially)
- Single deallocation at end (drop entire arena)
- AST lifetime naturally scoped to compilation

### ADR-004: Structural Typing for Tables

**Decision:** Use structural (shape-based) typing for tables rather than nominal typing.

**Rationale:**

- Lua tables are inherently structural (duck typing)
- Matches TypeScript semantics (familiar for users)
- More flexible than nominal typing
- Natural fit for dynamic language

### ADR-005: No `any` Type

**Decision:** Do not provide an `any` type. Use `unknown` for dynamic values.

**Rationale:**

- `any` subverts type system (too permissive)
- `unknown` forces explicit narrowing (safer)
- Encourages better type design
- Prevents gradual erosion of type safety

### ADR-006: String Interning

**Decision:** Use string interning for all identifiers via `StringInterner`.

**Rationale:**

- Reduced memory usage (single copy per unique string)
- Fast equality comparison (compare IDs, not string content)
- Efficient symbol tables
- Common identifiers pre-interned for common cases

---

## Design Patterns

### Visitor Pattern (AST Traversal)

The type checker and code generator both traverse the AST using visitor-like patterns:

```rust
impl TypeChecker {
    fn check_expression(&mut self, expr: &Expression) -> Type {
        match expr {
            Expression::Literal(lit) => self.check_literal(lit),
            Expression::Binary(op, lhs, rhs) => self.check_binary(op, lhs, rhs),
            // ... other variants
        }
    }
}
```

### Strategy Pattern (Diagnostic Handlers)

Different diagnostic handlers implement the same trait:

```rust
trait DiagnosticHandler {
    fn error(&self, span: Span, message: &str);
    fn warning(&self, span: Span, message: &str);
}

// Console output
impl DiagnosticHandler for ConsoleDiagnosticHandler { ... }

// Collect for tests/LSP
impl DiagnosticHandler for CollectingDiagnosticHandler { ... }
```

### Facade Pattern (Container)

The DI container acts as a facade, hiding complexity of wiring:

```rust
let container = Container::new(config);
// Container internally creates and wires all dependencies
```

### Flyweight Pattern (String Interner)

String interner shares string storage:

```rust
let id1 = interner.intern("foo");
let id2 = interner.intern("foo");
assert_eq!(id1, id2); // Same ID, one copy
```

### Provider Pattern (LSP)

Each LSP feature is encapsulated in a stateless provider:

```rust
pub struct HoverProvider;

impl HoverProvider {
    pub fn provide(&self, document: &Document, position: Position) -> Option<Hover> {
        // Stateless - all context passed as parameters
    }
}
```

---

## Future Considerations

### Incremental Compilation

**Status:** Not implemented

**Plan:**

- Cache type-checked modules
- Track dependencies between files
- Only recompile changed files + dependents
- Persist cache to disk

**Challenges:**

- Invalidation strategy
- Serializing typed AST
- Cross-file type inference

### Enhanced Cross-File Support

**Status:** Foundation exists

**Current State:**

- Import/export statements parsed
- Module registry tracks exports
- Symbol index tracks cross-file references

**Remaining Work:**

- Full type resolution across files
- Type-only imports
- Re-exports
- Better circular dependency handling

### Member Completion

**Status:** Stubbed in LSP

**Plan:**

- Use type checker's resolved types for `.` completion
- Handle class members, interface properties, table fields
- Support method completion with `:` syntax

### Performance Optimization

**Opportunities:**

- Parallel type checking of independent modules
- Lazy type resolution
- Better caching in LSP
- Incremental parsing

---

## Architectural Refactoring Plans

### Lua Target Strategy Pattern

**Status:** Planned

**Current Approach:**

The code generator uses capability checks (`supports_bitwise_ops()`, `supports_goto()`) scattered throughout generation logic. This works but doesn't scale well as version-specific logic grows.

**Proposed Architecture:**

Implement the **Strategy Pattern** with a trait-based approach:

```rust
pub trait CodeGenStrategy {
    fn generate_bitwise_op(&self, op: &str, lhs: &str, rhs: &str) -> String;
    fn generate_integer_divide(&self, lhs: &str, rhs: &str) -> String;
    fn generate_continue(&self, label: &str) -> String;
    fn emit_preamble(&self) -> Option<String>; // For library includes
}

pub struct Lua51Strategy;
pub struct Lua52Strategy;
pub struct Lua53Strategy;
pub struct Lua54Strategy;

// CodeGenerator holds a strategy instance
pub struct CodeGenerator<'a> {
    strategy: Box<dyn CodeGenStrategy>,
    // ... other fields
}
```

**Benefits:**

- Each Lua version isolated and independently testable
- Adding Lua 5.5 or LuaJIT becomes trivial (just implement trait)
- Removes conditional logic from main codegen
- Clear contract for what differs between versions

**Migration Path:**

Incrementally extract version-specific logic from main codegen into strategy implementations.

---

### Code Generator Modularization

**Status:** Planned

**Current State:**

The [codegen/mod.rs](../crates/typedlua-core/src/codegen/mod.rs) file is 3,120 lines - too large for easy maintenance.

**Proposed Structure:**

```text
crates/typedlua-core/src/codegen/
├── mod.rs              # Orchestrator (300 lines)
├── strategies/         # Lua version strategies
│   ├── mod.rs
│   ├── lua51.rs
│   ├── lua52.rs
│   ├── lua53.rs
│   └── lua54.rs
├── emitters/           # AST → Lua emitters
│   ├── mod.rs
│   ├── expressions.rs
│   ├── statements.rs
│   └── types.rs
├── transforms/         # Pluggable transforms (pipeline pattern)
│   ├── mod.rs
│   ├── classes.rs      # Class → Lua table transformation
│   ├── decorators.rs   # Decorator runtime emission
│   ├── modules.rs      # Import/export handling
│   └── sourcemaps.rs   # Source map generation
└── sourcemap.rs        # Already exists
```

**Transform Pipeline Pattern:**

Apply the same pattern used in the optimizer to code generation:

```rust
pub trait CodeGenTransform {
    fn name(&self) -> &'static str;
    fn transform(&mut self, output: &mut String, context: &mut GenContext) -> Result<(), Error>;
    fn applies_to_target(&self, target: LuaTarget) -> bool;
}

// CodeGenerator becomes an orchestrator
pub struct CodeGenerator<'a> {
    transforms: Vec<Box<dyn CodeGenTransform>>,
    // ... existing fields
}
```

**Benefits:**

- Each module has single, clear responsibility
- Transforms are independently testable
- Easy to add/remove features
- Clear separation between orchestration and implementation

---

### Type Checker Visitor Pattern

**Status:** Planned

**Current State:**

The [type_checker.rs](../crates/typedlua-core/src/typechecker/type_checker.rs) file is 3,544 lines with multiple concerns mixed together.

**Proposed Architecture:**

Extract specialized visitors for different type checking concerns:

```rust
pub trait TypeCheckVisitor {
    fn visit_expression(&mut self, expr: &Expression) -> Type;
    fn visit_statement(&mut self, stmt: &Statement) -> Result<(), Error>;
}

// Specialized visitors:
pub struct NarrowingVisitor;     // Type narrowing logic
pub struct GenericVisitor;        // Generic instantiation
pub struct AccessControlVisitor;  // public/private checks
pub struct InferenceVisitor;      // Type inference rules
```

**Structure:**

```text
crates/typedlua-core/src/typechecker/
├── type_checker.rs           # Main orchestrator
├── visitors/
│   ├── mod.rs
│   ├── narrowing.rs          # Control flow narrowing
│   ├── generics.rs           # Generic type handling
│   ├── access_control.rs     # Visibility checks
│   └── inference.rs          # Bidirectional inference
├── type_environment.rs       # Already exists
├── symbol_table.rs           # Already exists
└── ...
```

**Benefits:**

- Clear separation of concerns
- Each visitor testable in isolation
- Easier to understand individual type system features
- Reduces cognitive load (smaller focused files)

---

### Builder Pattern for CodeGenerator

**Status:** Planned

**Current API:**

```rust
let generator = CodeGenerator::new(interner);
// Configuration happens via setters after construction
```

**Proposed API:**

```rust
let generator = CodeGenerator::builder()
    .interner(interner)
    .target(LuaTarget::Lua53)
    .strategy(Lua53Strategy::new())
    .enable_sourcemaps()
    .bundle_mode("my/module")
    .build();
```

**Benefits:**

- Self-documenting configuration
- Easier testing with partial configuration
- Clear required vs optional parameters
- Compile-time validation of configuration

---

## References

- [Implementation Architecture](Implementation-Architecture.md) - Original design document
- [TypedLua Design](TypedLua-Design.md) - Type system specification
- [Language Features](LANGUAGE_FEATURES.md) - Feature documentation

---

**Version:** 2.0
**Contributors:** TypedLua Team
**License:** MIT
