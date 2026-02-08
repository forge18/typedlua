# TypedLua TODO

## Current Focus

### Phase 1: Test Suite Completion (Active)

**Goal:** Fix all failing tests across typedlua project.

**Completed:**

- [x] Fixed 20 destructuring tests (computed property keys, typed patterns, rest destructuring)
- [x] Fixed dead code/dead store elimination tests (created BlockVisitor trait for sibling-aware passes)
- [x] Fixed 7 string concatenation optimization tests (now correctly uses table.concat)
- [x] Fixed 3 access control tests (nonexistent member/class error reporting)
- [x] Fixed recursive type alias stack overflow (added cycle detection to type resolution)
- [x] Removed all `#[ignore]` attributes from typedlua-core (0 ignored tests)

**Status:** Complete. All tests pass: 421 typechecker unit tests, 169+ typedlua-core tests, 0 failures, 0 ignored in core.

---

## Completed Phases

### Phase 1.1: Arena Allocation Migration (Completed)

**Goal:** Migrate TypedLua parser AST from heap (`Box`, `Vec`) to arena allocation (`bumpalo`).

- [x] Added `Bump` arena to parser struct
- [x] Migrated all AST types to use `&'arena` references
- [x] Updated parser implementation (expression, statement, pattern, types)
- [x] Migrated all 168 AST node construction sites from `Box::new()` → `arena.alloc()`
- [x] Fixed arena lifetime propagation throughout codebase
- [x] All tests passing (431 parser tests, 468 typechecker tests)

---

### Phase 2: Benchmarking & Validation (COMPLETE)

**Goal:** Verify arena allocation performance improvements.

- [x] Profile allocation performance (profiling example created in `examples/profile_parser.rs`)
- [x] Benchmark compilation speed (criterion benchmarks updated and run)
- [x] Test with small programs (~100 nodes) - 6-16µs parse time
- [x] Test with medium programs (~1K nodes) - 70-130µs parse time
- [x] Test with stress tests (recursive types, deep nesting) - all pass
- [x] Document performance improvements in `docs/ARENA_PERFORMANCE.md`

**Results:**

- ✅ **20-30x faster** allocation for typical programs
- ✅ **150x faster** deallocation
- ✅ **10-20% reduced** memory footprint
- ✅ **100% test pass rate** maintained (1352+ tests)
- ✅ Linear O(n) scaling verified
- ✅ Parser: 6.57-130µs for programs up to 100 nodes
- ✅ Type checker: 258-399µs including stdlib loading

**Status:** Complete. See `docs/ARENA_PERFORMANCE.md` for full report.

---

## Future Work

### Phase 3: Reflection Metadata v2

**Goal:** Implement three coordinated enhancements:

1. Selective Generation via `@std/reflection` import detection
2. Bit Flags for field modifiers (~60% memory savings)
3. Method Signatures compact encoding (~50% memory savings)

**Status:** Not started. See docs/REFLECTION.md for specification.

**Subtasks:**

- Phase 3.1: Import Detection System
- Phase 3.2: Bit Flags for Field Modifiers
- Phase 3.3: Compact Method Signatures
- Phase 3.4: Runtime Support Updates
- Phase 3.5: Configuration & CLI Integration
- Phase 3.6: Breaking Changes Migration

---

### Phase 4: Runtime Validation from Types

**Goal:** Generate runtime validation code from type annotations.

**Key Features:**

- `Refined<Base, Constraints>` utility type
- Auto-validate at boundaries + @validate decorator
- Compiler intrinsics: `parse<T>()`, `safeParse<T>()`, `is<T>()`
- Fail-fast and collect error modes

**Status:** Not started. See docs/designs/Runtime-Validation.md for specification.

**Subtasks:**

- Phase 4.1: Refined<> Type System
- Phase 4.2: Validation Mode Configuration
- Phase 4.3: Validator Code Generation
- Phase 4.4: Compiler Intrinsics
- Phase 4.5: Inlining Optimization
- Phase 4.6: Advanced Optimizations
- Phase 4.7: Runtime Support Library
- Phase 4.8: Class Instance Validation
- Phase 4.9: Integration & Documentation

---

### Phase 5: File Extension Migration (.tl → .luax)

**Goal:** Rename project file extension for LuaNext rebrand.

**Status:** Not started. See docs/file-extension-migration.md (45+ files, 12 renames).

**Subtasks:**

- Phase 5.1: Core Runtime Logic
- Phase 5.2: Test Infrastructure
- Phase 5.3: Actual File Renames (git mv)
- Phase 5.4: Documentation Updates
- Phase 5.5: Module Resolution & Build Scripts
- Phase 5.6: Verification & Post-Migration

---

### Phase 6: Project Rename (TypedLua → LuaNext)

**Goal:** Rename all project references from TypedLua to LuaNext.

**Subtasks:**

#### Phase 6.1: Cargo Workspace & Crate Names

- [ ] Update `Cargo.toml` workspace name and `members` paths
- [ ] Update `crates/typedlua-core/Cargo.toml` package name → `luanext-core`
- [ ] Update `crates/typedlua-cli/Cargo.toml` package name → `luanext-cli`
- [ ] Update `crates/typedlua-runtime/Cargo.toml` package name → `luanext-runtime`
- [ ] Update `crates/typedlua-typechecker/Cargo.toml` package name → `luanext-typechecker`
- [ ] Update `crates/typedlua-parser/Cargo.toml` package name → `luanext-parser`
- [ ] Update `crates/typedlua-lsp/Cargo.toml` package name → `luanext-lsp`
- [ ] Run `cargo metadata` to verify workspace resolves

#### Phase 6.2: Rust Module & Crate Names

- [ ] Rename `typedlua_core` → `luanext_core` in lib.rs of each crate
- [ ] Rename `typedlua_cli` → `luanext_cli` in lib.rs
- [ ] Rename `typedlua_runtime` → `luanext_runtime` in lib.rs
- [ ] Rename `typedlua_typechecker` → `luanext_typechecker` in lib.rs
- [ ] Rename `typedlua_parser` → `luanext_parser` in lib.rs
- [ ] Rename `typedlua_lsp` → `luanext_lsp` in lib.rs
- [ ] Update all `use` statements across codebase (grep for `typedlua::`)
- [ ] Update `cargo.toml` dependencies to use new crate names
- [ ] Run `cargo check --workspace` to find all remaining references

#### Phase 6.3: CLI Binary Name

- [ ] Rename binary in `crates/typedlua-cli/Cargo.toml`: `[[bin]]` name `typedlua` → `luanext`
- [ ] Update scripts that invoke `typedlua` command
- [ ] Update VSCode extension to spawn `luanext` instead of `typedlua`
- [ ] Update CI/CD workflows that use CLI

#### Phase 6.4: VSCode Extension Rename

- [ ] Rename extension in `editors/vscode/package.json` name/displayName
- [ ] Update extension ID from `typedlua` to `luanext`
- [ ] Update README.md in editors/vscode
- [ ] Update marketplace descriptions

#### Phase 6.5: Documentation Updates

- [ ] Update main README.md title and references
- [ ] Update `docs/README.md`
- [ ] Update `docs/ARCHITECTURE.md`
- [ ] Rename `docs/designs/TypedLua-Design.md` → `LuaNext-Design.md`
- [ ] Update all design docs with new name
- [ ] Update `CONTRIBUTING.md`
- [ ] Update `CHANGELOG.md` header

#### Phase 6.6: Source Code References

- [ ] Search for string literals "TypedLua" in source code
- [ ] Update welcome messages, error messages, --help output
- [ ] Update internal constants/enum variants if any
- [ ] Update comments that reference TypedLua

#### Phase 6.7: GitHub & Publishing

- [ ] Rename GitHub repository from `typedlua` to `luanext`
- [ ] Yank old crates.io packages, publish new ones
- [ ] Update GitHub Actions workflows
- [ ] Update any badges in README.md
- [ ] Update package.json version for VSCode extension

#### Phase 6.8: Verification

- [ ] Run `cargo build --release` for all crates
- [ ] Run `cargo test --workspace`
- [ ] Test CLI: `luanext --version`, `luanext --help`
- [ ] Test VSCode extension loads correctly
- [ ] Test LSP functionality
- [ ] Update any local development instructions

**Status:** Not started. Requires coordination with crates.io publishing and GitHub repo rename.
