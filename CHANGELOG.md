# Changelog

All notable changes to TypedLua will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Cross-file LSP features: go-to-definition, find-all-references, and rename refactoring across files
- Module system with import/export statements
- Bundle mode code generation with custom module runtime
- Source maps for both require() mode and bundle mode
- Type-only imports that are erased during code generation
- Circular dependency detection with full cycle path reporting
- `.d.tl` declaration file support
- Hybrid module resolution (Node-style `./relative` + Lua-style `package.module`)
- `new` keyword for class instantiation
- Generic type inference for function calls
- Re-exports support (`export { foo } from './bar'`)
- Namespace imports (`import * as utils from './utils'`)
- Default exports and imports
- Named exports with multiple items
- 5-phase compilation pipeline (parse → build graph → topo sort → type check → codegen)

### Changed
- LSP server now uses dependency injection pattern for improved testability
- Type checker resolves both actual and expected types for better error messages
- Module imports can now override builtin names (no more stdlib shadowing)

### Fixed
- Interface method calls now work correctly with `this` keyword
- Generic type instantiation in function calls
- Type alias resolution in return statements
- Parser and type checker panic conditions
- Skipped computed properties in type checking
- Multiple silent failures in parser

## [0.1.0] - 2026-01-14

### Added
- Complete lexer with all Lua tokens plus TypeScript-inspired syntax
- Full parser for TypedLua language
- Comprehensive type checker with structural typing
- Code generator targeting Lua 5.1+
- CLI with compile, test, and bundle commands
- LSP server with 13 features (diagnostics, completion, hover, etc.)
- VS Code extension with syntax highlighting
- Object-oriented programming: classes, interfaces, inheritance
- Functional programming: first-class functions, closures, higher-order functions
- Decorators for classes and methods
- Generics with constraints
- Advanced types: union, intersection, literal, tuple, mapped types
- Template literal types
- Standard library type definitions
- 822 tests with 100% passing rate

[Unreleased]: https://github.com/yourusername/typed-lua/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/typed-lua/releases/tag/v0.1.0
