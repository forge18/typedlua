# TypedLua Architecture

**Version:** 1.0
**Last Updated:** 2026-01-13

## Table of Contents

- [Overview](#overview)
- [System Architecture](#system-architecture)
- [Component Design](#component-design)
- [Data Flow](#data-flow)
- [Technology Stack](#technology-stack)
- [Architectural Decisions](#architectural-decisions)
- [Design Patterns](#design-patterns)

---

## Overview

TypedLua is a typed superset of Lua implemented in Rust, providing static type checking with gradual typing inspired by TypeScript. The compiler transforms typed Lua code into plain Lua while ensuring type safety at compile time.

### Core Design Principles

1. **Dependency Injection** - All components receive dependencies through constructors, enabling testability
2. **Single Responsibility** - Each module has one clear, well-defined purpose
3. **Zero Runtime Overhead** - All type information is erased during compilation
4. **Gradual Typing** - Types are optional, allowing incremental adoption
5. **Trait-Based Abstraction** - Use traits for flexibility and mockability

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

```
typed-lua/
├── crates/
│   ├── typedlua-core/      # Compiler core (lexer, parser, type checker, codegen)
│   ├── typedlua-cli/       # Command-line interface
│   └── typedlua-lsp/       # Language Server Protocol implementation
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
- `TokenKind`: Enum of all possible token types
- `Span`: Source location tracking (line, column, byte offset)

**Features:**
- Context-aware tokenization (respects TypedLua vs Lua syntax)
- Comprehensive error reporting via diagnostic handler
- Support for multi-line strings, comments, and template literals

### Parser

**Location:** `crates/typedlua-core/src/parser/`

Builds an Abstract Syntax Tree (AST) from tokens.

**Structure:**
- `mod.rs`: Core parser logic and orchestration
- `expression.rs`: Expression parsing (binary ops, calls, indexing, etc.)
- `statement.rs`: Statement parsing (assignments, loops, functions, etc.)
- `types.rs`: Type annotation parsing
- `pattern.rs`: Pattern matching and destructuring

**Features:**
- Recursive descent parser
- Operator precedence climbing
- Feature flag support (OOP, FP, decorators)
- Arena-based AST allocation (bump allocator)
- Comprehensive error recovery

### Type Checker

**Location:** `crates/typedlua-core/src/typechecker/`

Performs static type analysis and inference.

**Structure:**
- `type_checker.rs`: Main type checking orchestration
- `type_environment.rs`: Type variable environments and scoping
- `symbol_table.rs`: Symbol resolution and storage
- `type_compat.rs`: Type compatibility and subtyping rules
- `generics.rs`: Generic type instantiation and constraints
- `narrowing.rs`: Control flow-based type narrowing
- `utility_types.rs`: Built-in utility types (Partial, Required, etc.)

**Features:**
- Bidirectional type inference
- Structural typing for tables
- Union/intersection type support
- Generic type instantiation
- Control flow analysis for narrowing
- Multi-return function handling

### Code Generator

**Location:** `crates/typedlua-core/src/codegen/`

Transforms typed AST into executable Lua code.

**Structure:**
- `mod.rs`: Core code generation logic
- `sourcemap.rs`: Source map generation for debugging

**Features:**
- Type erasure (zero runtime overhead)
- Target Lua version support (5.1-5.4)
- Source map generation
- Readable output (preserves structure where possible)
- Feature flag handling (OOP → metatable patterns, etc.)

### Diagnostic System

**Location:** `crates/typedlua-core/src/diagnostics.rs`

Comprehensive error reporting with LSP compatibility.

**Components:**
- `DiagnosticHandler` trait: Interface for reporting errors/warnings
- `Diagnostic`: Error/warning with code, message, span, and suggestions
- `ConsoleDiagnosticHandler`: Pretty terminal output
- `CollectingDiagnosticHandler`: Collects diagnostics for testing/LSP

**Features:**
- Structured error codes (e.g., `E0001`, `W0042`)
- Multi-level diagnostics (error, warning, info, hint)
- Related information (secondary spans)
- Quick-fix suggestions
- Pretty printing with color and source snippets

### Memory Management

#### Arena Allocator

**Location:** `crates/typedlua-core/src/arena.rs`

Uses `bumpalo` for fast AST node allocation.

**Benefits:**
- Extremely fast allocation (bump pointer)
- Minimal fragmentation
- Single deallocation (entire arena at once)
- Better cache locality

**Usage:**
```rust
let arena = Arena::new();
let node = arena.alloc(Expression::Literal(...));
```

#### String Interner

**Location:** `crates/typedlua-core/src/string_interner.rs`

Deduplicates string literals and identifiers.

**Benefits:**
- Reduced memory usage (single copy per unique string)
- Fast equality comparison (compare IDs, not string content)
- Efficient symbol tables

---

## Data Flow

### Compilation Pipeline

1. **Source Input** → Raw TypedLua source code
2. **Lexer** → Tokenizes source into `Vec<Token>`
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

### Configuration Flow

```
1. CLI parses arguments
2. Loads tlconfig.yaml (if exists)
3. Merges CLI overrides
4. Creates CompilerConfig
5. Injects into Container
6. All components access via Arc<CompilerConfig>
```

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

- **rayon** - Data parallelism (project-wide compilation)

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

**Trade-offs:**
- Steeper learning curve than Go
- Longer compile times than interpreted languages
- Less mature compiler libraries than OCaml

---

### ADR-002: Dependency Injection via Explicit Container

**Decision:** Use manual dependency injection with a container rather than global state or a DI framework.

**Rationale:**
- Explicit dependencies make code easier to understand
- Trivial to mock for testing
- No magic or reflection needed
- Full compile-time type safety
- Performance (no runtime lookup)

**Trade-offs:**
- More boilerplate than global state
- Manual wiring in container
- Must pass container or dependencies through call chains

---

### ADR-003: Arena Allocation for AST

**Decision:** Use bump allocation (arena) for AST nodes instead of `Box`/`Rc`.

**Rationale:**
- 10-100x faster allocation than `Box::new()`
- Minimal memory fragmentation
- Better cache locality (nodes allocated sequentially)
- Single deallocation at end (drop entire arena)
- AST lifetime naturally scoped to compilation

**Trade-offs:**
- Cannot deallocate individual nodes
- Lifetime annotations required (`'ast`)
- Arena must outlive all AST references

---

### ADR-004: Structural Typing for Tables

**Decision:** Use structural (shape-based) typing for tables rather than nominal typing.

**Rationale:**
- Lua tables are inherently structural (duck typing)
- Matches TypeScript semantics (familiar for users)
- More flexible than nominal typing
- Natural fit for dynamic language

**Trade-offs:**
- More complex type checking algorithm
- Harder to generate good error messages
- No branded types (can be added later)

---

### ADR-005: Separate `interface` and `type` Keywords

**Decision:** Enforce that `interface` is only for table shapes, `type` is for everything else.

**Rationale:**
- Clear mental model for users
- Prevents confusion between structural and alias types
- Easier to generate better error messages
- Matches TypeScript convention

**Trade-offs:**
- More restrictive than TypeScript
- Users must remember distinction
- Cannot use intersection types with interfaces

---

### ADR-006: No `any` Type

**Decision:** Do not provide an `any` type. Use `unknown` for dynamic values.

**Rationale:**
- `any` subverts type system (too permissive)
- `unknown` forces explicit narrowing (safer)
- Encourages better type design
- Prevents gradual erosion of type safety

**Trade-offs:**
- More friction when migrating untyped code
- Requires more annotations initially
- Less familiar to TypeScript users

---

### ADR-007: Feature Flags for Optional Syntax

**Decision:** Gate OOP, FP, and decorator features behind configuration flags.

**Rationale:**
- Users can opt-in to complexity
- Smaller learning surface for beginners
- Better error messages (can explain flag requirement)
- Allows evolution without breaking existing code

**Trade-offs:**
- More configuration complexity
- Feature interaction edge cases
- Parser must handle multiple syntaxes

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

### Builder Pattern (Configuration)

Configuration uses builder-like construction with sensible defaults:

```rust
CompilerConfig {
    compiler_options: CompilerOptions::default(),
    include: vec!["src/**/*.tl".to_string()],
    exclude: vec!["**/node_modules/**".to_string()],
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

---

## Future Architecture Considerations

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

### Parallel Compilation

**Status:** Rayon available but not used

**Plan:**
- Topologically sort modules by dependencies
- Compile independent modules in parallel
- Use work-stealing scheduler (rayon)

**Challenges:**
- Shared diagnostic handler (needs thread-safe collection)
- Progress reporting
- Error ordering for user experience

### Module System

**Status:** Not implemented

**Plan:**
- Support `import`/`export` statements
- Type-only imports
- Re-exports
- Circular dependency detection

**Challenges:**
- Resolution strategy (Node, browser, Lua-specific?)
- Type definition files (`.d.tl`)
- Interaction with Lua `require`

---

## References

- [Implementation Architecture](Implementation-Architecture.md) - Original design document
- [TypedLua Design](TypedLua-Design.md) - Type system specification
- [AST Structure](AST-Structure.md) - AST node definitions
- [LSP Design](LSP-Design.md) - Language server architecture

---

**Version:** 1.0
**Contributors:** TypedLua Team
**License:** MIT