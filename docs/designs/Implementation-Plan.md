# TypedLua Implementation Plan

**Document Version:** 0.1  
**Last Updated:** 2024-12-31

Complete implementation roadmap for TypedLua compiler and tooling.

---

## Timeline: 7-10 Months (31-44 weeks)

---

## Phase 0: Foundation (2-3 weeks)

**Setup:**
- Cargo workspace with 3 crates: core, cli, lsp
- Core types: Span, Diagnostic, Error types
- DI Container implementation
- Configuration system (typedlua.json parsing)
- CI/CD pipeline (GitHub Actions)

**Deliverables:**
- ✓ Project structure
- ✓ Core types defined
- ✓ Config parsing works
- ✓ Tests passing

---

## Phase 1: Lexer & Parser (3-4 weeks)

**Lexer (Week 1-2):**
- Tokenize all TypedLua syntax
- Handle keywords, operators, literals, identifiers
- Span tracking for error reporting
- Test all token types

**Parser (Week 3-4):**
- Recursive descent parser
- Generate complete AST
- Expression parsing with Pratt parser
- Error recovery
- Snapshot tests

**Milestone:** `tl parse file.tl --print-ast`

---

## Phase 2: Type System (4-5 weeks)

**Week 1:** Type representation
**Week 2-3:** Type checker core, symbol table
**Week 4:** Type inference  
**Week 5:** Interfaces, structural typing

**Features:**
- Primitives, unions, intersections, objects
- Function types, tuples, arrays
- Type inference (const = literal, local = widened)
- Structural compatibility checking

**Milestone:** `tl check file.tl` reports errors

---

## Phase 3: Code Generation (2-3 weeks)

**Week 1:** Basic codegen (statements, expressions)
**Week 2:** Source maps, target versions (5.1-5.4)
**Week 3:** Testing, refinement

**Features:**
- Generate clean Lua code
- Type erasure
- Source map support
- Target-specific output

**Milestone:** `tl compile file.tl && lua file.lua`

---

## Phase 4: CLI (1-2 weeks)

**Features:**
- TypeScript-style `tl` command
- All flags: --project, --watch, --noEmit, --outDir, etc.
- Watch mode with file monitoring
- Pretty error messages with colors
- Config file loading

**Milestone:** `tl --watch src/**/*.tl`

---

## Phase 5: Advanced Types (3-4 weeks)

**Week 1:** Generics + constraints
**Week 2:** Utility types (Partial, Required, Pick, Omit, Record, etc.)
**Week 3:** Mapped types, conditional types
**Week 4:** Type narrowing, template literal types

---

## Phase 6: OOP Features (3-4 weeks)

**Week 1:** Basic classes
**Week 2:** Inheritance
**Week 3-4:** Access modifiers, abstract classes, getters/setters

**Code Generation:**
- Classes → Lua metatables
- Inheritance → prototype chain
- Access modifiers → compile-time checking

**Prerequisite:** `enableOOP` config flag

---

## Phase 7: FP Features (2-3 weeks)

**Week 1:** Pattern matching + exhaustiveness
**Week 2:** Destructuring, spread operator
**Week 3:** Pipe operator

**Prerequisite:** `enableFP` config flag

---

## Phase 8: Decorators (2-3 weeks)

**Features:**
- TC39 Stage 3 decorator syntax
- Class, method, field, accessor decorators
- Built-in: @readonly, @sealed, @deprecated

**Prerequisite:** `enableDecorators` config flag

---

## Phase 9: Language Server (4-5 weeks)

**Week 1:** LSP infrastructure (tower-lsp)
**Week 2:** Diagnostics, hover, completion
**Week 3:** Go-to-def, find references, symbols
**Week 4:** Rename, code actions, signature help
**Week 5:** VS Code extension + TextMate grammar

**Features:**
- Real-time diagnostics
- IntelliSense autocomplete
- Navigation (definition, references)
- Refactoring (rename, quick fixes)
- Syntax highlighting

**Deliverables:**
- LSP server binary
- VS Code extension published

---

## Phase 10: Standard Library (2-3 weeks)

**Week 1:** Core libraries (string, table, math, globals)
**Week 2:** I/O, OS, coroutine + version differences

**Format:** `.d.tl` type definition files with TypeScript-style overloads

---

## Phase 11: Polish & Optimization (3-4 weeks)

**Week 1:** Performance profiling + optimization
**Week 2:** Improved error messages
**Week 3:** Documentation (user guide, API docs)
**Week 4:** Examples, stress testing

---

## Phase 12: Release (2-3 weeks)

**Week 1:** Beta testing, bug fixes
**Week 2:** Website, release notes, install guides
**Week 3:** Launch v1.0.0

**Distribution:** cargo, homebrew, npm (optional)

---

## Development Workflow

```bash
# Daily
cargo test                    # Run tests
cargo fmt && cargo clippy     # Format + lint
cargo tarpaulin              # Coverage

# Milestones
tl parse file.tl             # Phase 1
tl check file.tl             # Phase 2
tl compile file.tl           # Phase 3
tl --watch src/              # Phase 4
code .                       # Phase 9 (IDE support)
```

---

## Testing Strategy

**Unit Tests:** All components (lexer, parser, type checker, codegen)
**Integration Tests:** End-to-end compilation
**Snapshot Tests:** AST and output verification (insta crate)
**Fuzzing:** Parser robustness
**Target:** >90% coverage

---

## Key Decisions

1. **Rust** for implementation (performance, safety)
2. **Tower-LSP** for language server
3. **Clap** for CLI
4. **TypeScript-style** CLI and errors
5. **Feature flags** for OOP, FP, decorators
6. **Lua 5.1-5.4** target support (no LuaJIT)

---

## Risk Mitigation

**Technical:** Start simple, iterate on complexity
**Performance:** Profile early, optimize incrementally  
**Schedule:** Buffer time for unexpected issues
**Quality:** >90% test coverage requirement

---

## Post-1.0 Features

- Incremental compilation
- Project references (monorepos)
- REPL mode
- Debugger integration
- Plugin system
- Package manager

---

**Next Step:** Begin Phase 0 - Foundation

**Document Version:** 0.1  
**Last Updated:** 2024-12-31
