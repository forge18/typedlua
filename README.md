# TypedLua

[![Alpha](https://img.shields.io/badge/status-alpha-orange.svg)](https://github.com/forge18/typed-lua)
[![codecov](https://codecov.io/gh/forge18/typed-lua/branch/main/graph/badge.svg)](https://codecov.io/gh/forge18/typed-lua)
[![CI](https://github.com/forge18/typed-lua/actions/workflows/ci.yml/badge.svg)](https://github.com/forge18/typed-lua/actions/workflows/ci.yml)

A typed superset of Lua with gradual typing, inspired by TypeScript's approach to JavaScript.

## Overview

TypedLua brings static type checking to Lua while maintaining its simplicity and allowing gradual adoption. Write type-safe Lua code that compiles to plain Lua, with zero runtime overhead.

## Features

- **Gradual Typing** - Add types at your own pace, from none to full coverage
- **TypeScript-Inspired** - Familiar syntax for developers coming from TypeScript
- **Zero Runtime Cost** - Types are erased at compile time
- **Lua Compatibility** - Compiles to clean, readable Lua (5.1-5.4)
- **Rich Type System** - Interfaces, unions, generics, and more
- **Optional Features** - Enable OOP, functional programming, or decorators as needed
- **LSP Support** - Full language server with autocomplete, diagnostics, and more

## Project Status

**Phase 0: Foundation - Complete ✅**

The compiler foundation is built and ready:
- ✅ Cargo workspace with 3 crates (core, cli, lsp)
- ✅ Dependency injection architecture
- ✅ Configuration system (tlconfig.yaml)
- ✅ Diagnostic and error handling
- ✅ CI/CD pipeline with tests, formatting, and linting
- ✅ 21 passing tests with full code coverage

**Next: Phase 1 - Lexer & Parser** (In Progress)

## Installation

*Coming soon - the compiler is under active development*

```bash
# Install via cargo (when released)
cargo install typedlua

# Or build from source
git clone https://github.com/forge18/typed-lua
cd typed-lua
cargo build --release
```

## Quick Start

### Example TypedLua Code

```lua
-- Variable declarations with types
const PI: number = 3.14159
local radius: number = 5

-- Interfaces for table shapes
interface Point {
    x: number,
    y: number
}

-- Functions with type signatures
function calculateArea(r: number): number
    return PI * r * r
end

-- Type inference
const area = calculateArea(radius)  -- inferred as number

print("Area:", area)
```

### Compiles to Clean Lua

```lua
local PI = 3.14159
local radius = 5

local function calculateArea(r)
    return PI * r * r
end

local area = calculateArea(radius)

print("Area:", area)
```

## Configuration

Create a `tlconfig.yaml` in your project root:

```yaml
compilerOptions:
  target: "5.4"           # Lua version: 5.1, 5.2, 5.3, 5.4
  strictNullChecks: true  # Enforce null safety
  enableOop: true         # Enable class syntax
  enableFp: true          # Enable pattern matching, pipe operators
  enableDecorators: true  # Enable decorator syntax
  outDir: "dist"          # Output directory
  sourceMap: true         # Generate source maps

include:
  - "src/**/*.tl"

exclude:
  - "**/node_modules/**"
  - "**/dist/**"
```

## Architecture

TypedLua is built in Rust with a focus on modularity and testability:

```
typedlua/
├── crates/
│   ├── typedlua-core/    # Compiler core (lexer, parser, type checker, codegen)
│   ├── typedlua-cli/     # Command-line interface
│   └── typedlua-lsp/     # Language Server Protocol implementation
```

**Design Principles:**
- Dependency injection for testability
- Trait-based abstractions for flexibility
- Comprehensive error handling with detailed diagnostics
- Zero runtime overhead - all types erased at compile time

## Development

### Prerequisites

- Rust 1.70+
- Cargo

### Building

```bash
# Build all crates
cargo build

# Run tests
cargo test

# Run linter
cargo clippy

# Format code
cargo fmt
```

### Running Tests

```bash
# Run all tests
cargo test --all

# Run tests for specific crate
cargo test -p typedlua-core

# Run with coverage
cargo tarpaulin --all-features --workspace
```

## Type System

TypedLua provides a rich type system inspired by TypeScript:

### Primitive Types
- `nil`, `boolean`, `number`, `integer`, `string`
- `unknown` (type-safe, must narrow before use)
- `never` (for exhaustiveness checking)
- `void` (for functions with no return)

### Composite Types
- Arrays: `number[]` or `Array<number>`
- Tuples: `[string, number]`
- Functions: `(x: number) -> boolean`
- Unions: `string | number`
- Interfaces: table shapes only
- Type aliases: everything except table shapes

See [docs/designs/TypedLua-Design.md](docs/designs/TypedLua-Design.md) for complete type system documentation.

## Language Features

TypedLua includes powerful OOP features for building robust applications:

- **`override` keyword** - Explicit method overriding with compile-time validation
- **`final` keyword** - Prevent inheritance and method overriding
- Classes, interfaces, and inheritance
- Access modifiers (public, private, protected)
- Decorators and metadata
- Pattern matching and destructuring

See [docs/LANGUAGE_FEATURES.md](docs/LANGUAGE_FEATURES.md) for detailed documentation and examples.

## Roadmap

- [x] **Phase 0: Foundation** - Project setup, DI, configuration, CI/CD
- [ ] **Phase 1: Lexer & Parser** - Tokenization and AST construction
- [ ] **Phase 2: Type System** - Type checking and inference
- [ ] **Phase 3: Code Generation** - Lua output with source maps
- [ ] **Phase 4: CLI** - Command-line interface and watch mode
- [ ] **Phase 5: Advanced Features** - Generics, utility types, narrowing
- [ ] **Phase 6: OOP** - Classes, inheritance, access modifiers
- [ ] **Phase 7: FP** - Pattern matching, destructuring, pipe operators
- [ ] **Phase 8: Decorators** - Decorator syntax and built-ins
- [ ] **Phase 9: LSP** - Full language server with VS Code extension
- [ ] **Phase 10: Standard Library** - Type definitions for Lua stdlib
- [ ] **Phase 11: Polish** - Performance optimization, error messages
- [ ] **Phase 12: Release** - v1.0.0 launch

See [TODO.md](TODO.md) for detailed progress tracking.

## Contributing

TypedLua is under active development. Contributions are welcome!

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

Please ensure:
- All tests pass (`cargo test`)
- Code is formatted (`cargo fmt`)
- No clippy warnings (`cargo clippy`)

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Inspired by [TypeScript](https://www.typescriptlang.org/) and [Teal](https://github.com/teal-language/tl)
- Built with [Rust](https://www.rust-lang.org/) for performance and safety
- Uses [Tower LSP](https://github.com/ebkalderon/tower-lsp) for language server implementation

---

**Status:** 🚧 Under Active Development - Phase 0 Complete

Built with ❤️ by the TypedLua team
