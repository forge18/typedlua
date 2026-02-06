# TypedLua TODO

## Quick Wins (Week 0)

### Universal FxHashMap

**Goal:** Replace all `std::HashMap` with `FxHashMap` across compiler

**Action Items:**

- [x] Search for all `use std::collections::HashMap` imports in `crates/typedlua-core/src/`
- [x] Replace imports with `use rustc_hash::FxHashMap`
- [x] Update type annotations from `HashMap` to `FxHashMap`
- [x] Run `cargo test` to verify no regressions (143 tests passed)
- [x] Run benchmarks to measure performance impact
- [x] Document performance improvement in commit message

**Expected:** Faster lookups in symbol tables and type registries

---

### Lazy Pass Evaluation

**Goal:** Skip optimization passes when AST doesn't contain relevant features

**Action Items:**

- [x] Define `AstFeatures` bitflags enum (HAS_LOOPS, HAS_CLASSES, HAS_ENUMS, etc.)
- [x] Create feature detection visitor in `optimizer/mod.rs`
- [x] Add `required_features()` method to all Pass traits (ExprVisitor, StmtVisitor, PreAnalysisPass, WholeProgramPass)
- [?] Hook up `should_run_pass()` in optimization loop - **INCOMPLETE**
  - ❌ `should_run_pass()` method exists but is never called!
  - Need to add feature check before each pass execution
- [x] Add debug logging to show which passes are skipped
- [x] Run `cargo test` to verify no regressions (143 tests passed)
- [x] Update individual passes with specific required features:
  - ✅ `LoopOptimizationPass` requires `HAS_LOOPS`
  - ✅ `RichEnumOptimizationPass` requires `HAS_ENUMS`
  - Other passes use default `EMPTY` (no specific requirements)
- [ ] Test with modules missing each feature type (e.g., loop-free code, class-free code, etc.)
- [ ] Run benchmarks to measure performance impact

**Status:** **NEEDS FIX** - Core infrastructure implemented but not connected. Need to update `Optimizer::optimize()` to actually call `should_run_pass()` before running each pass.

**Expected:** Skip unnecessary passes (e.g., no loops → skip loop optimization)

---

## Phase 3.5: Type Checker Optimization

**Problem:** Type checking is 67% of compilation time (4.65ms of 7ms)

### Strategy 1: Type Relation Caching (1-2 days)

**What:** LRU cache for `(Type, Type) → bool` subtype checks

**Action Items:**

- [x] Add `lru` crate dependency to `typedlua-typechecker/Cargo.toml`
- [x] Create `typedlua-typechecker/src/type_relations.rs`
- [x] Implement `TypeRelationCache` struct with `LruCache<(usize, usize), bool>` using type pointers
- [x] Add cache field to `TypeChecker` struct
- [x] Hook cache into subtype checking logic (5 call sites updated)
- [x] Add cache hit/miss metrics for debugging
- [x] Run `cargo test` in typedlua-typechecker to verify correctness (374 tests passed)
- [x] Run benchmarks to measure speedup
- [x] Benchmark results: **5-6% performance improvement** on synthetic tests
- [ ] Tune cache size based on profiling data (currently 1024 entries)

**Expected:** 10-30% faster type checking

**Status:** **IMPLEMENTED** - Core infrastructure complete. 5-6% speedup achieved on benchmarks. Cache tuning can be done as follow-up.

---

### Strategy 2: Declaration-Level Incremental (2-3 weeks)

**What:** Hash function signatures separately from bodies (Rust's approach). Re-check only changed declarations + dependents.

**Key insight:** Body-only changes don't invalidate callers

**Action Items:**

#### Phase 2.1: Signature Hashing (Week 1)

- [ ] Create `typedlua-typechecker/src/incremental.rs`
- [ ] Implement `DeclarationHash` for functions (name + params + return type)
- [ ] Implement `DeclarationHash` for classes (name + fields + method signatures)
- [ ] Implement `DeclarationHash` for interfaces
- [ ] Add `compute_signature_hash()` helper using stable hashing
- [ ] Add unit tests for hash stability

#### Phase 2.2: Dependency Tracking (Week 2)

- [ ] Implement `DependencyGraph` to track caller → callee relationships
- [ ] Add visitor to collect dependencies during type checking
- [ ] Implement `get_dependents(decl: &str) -> Vec<String>` query
- [ ] Add unit tests for dependency graph operations

#### Phase 2.3: Invalidation Logic (Week 2)

- [ ] Implement `compute_invalidated_decls()` logic:
  - Compare old vs new signature hashes
  - If signature changed → invalidate declaration + all dependents
  - If only body changed → only invalidate declaration itself
- [ ] Add debug logging for invalidation decisions
- [ ] Add unit tests for invalidation scenarios

#### Phase 2.4: Integration (Week 3)

- [ ] Add `check_program_incremental()` API to `typedlua-typechecker/src/lib.rs`
- [ ] Update `typedlua-cli/src/cache/manifest.rs` to store:
  - Declaration hashes from previous compilation
  - Dependency graph
- [ ] Update `typedlua-cli/src/main.rs` to call incremental API when cache exists
- [ ] Add `--force-full-check` CLI flag to bypass incremental for debugging

#### Phase 2.5: Testing & Validation (Week 3)

- [ ] Test: Signature-only change (params) → dependents re-checked
- [ ] Test: Body-only change → dependents NOT re-checked
- [ ] Test: New function → no invalidation
- [ ] Test: Deleted function → dependents invalidated
- [ ] Test: Transitive dependencies work correctly
- [ ] Run benchmarks to measure 50-90% speedup on incremental edits
- [ ] Test with real-world projects

**Expected:** 50-90% faster incremental edits (OOPSLA 2022 paper: 21-147× speedup)

---

### Strategy 3: RTA Devirtualization (3-5 days)

**What:** Track `new ClassName()` instantiation sites. Devirtualize when only one subclass instantiated.

**Action Items:**

- [ ] Add `InstantiationTracker` struct to `optimizer/mod.rs`
- [ ] Add `track_instantiation(class_name: &str, location: Span)` method
- [ ] Update AST visitor to detect `new ClassName()` expressions
- [ ] Build map of `ClassName → Set<SubclassName>` for all instantiations
- [ ] Update `devirtualization.rs` to accept RTA data from WholeProgramAnalysis
- [ ] Add devirtualization logic: if only one subclass instantiated → devirtualize
- [ ] Add debug logging showing:
  - Which classes have single instantiations
  - Which method calls are devirtualized via RTA
- [ ] Test with single-instantiation scenario (should devirtualize)
- [ ] Test with multi-instantiation scenario (should NOT devirtualize)
- [ ] Test with no instantiations (should fall back to existing CHA logic)
- [ ] Run benchmarks to measure devirtualization rate improvement

**Expected:** 30-50% more devirtualization opportunities

---

### Implementation Order

**Week 1:** Strategy 1 + Strategy 3 (quick wins)
**Weeks 2-4:** Strategy 2 (high impact, more complex)

---

### Bottom Line

- First compilation: 10-30% faster
- Incremental edits: 50-90% faster
- LSP: Sub-100ms responsiveness achieved

---

## Phase 4: Arena Allocation (Pre-1.0 Required)

**Rationale:** Best practice for production Rust compilers (rustc, swc). Required for unknown-scale production use.

**Problem:** Standard Box/Vec allocation doesn't scale to large codebases or LSP use cases.

### Arena-Allocated AST (2-3 weeks)

**What:** Replace `Box<T>`/`Vec<T>` with bump allocator. AST nodes allocated from arena with `'arena` lifetime.

**Why this is best practice:**

- **rustc** (Rust compiler): Uses `typed-arena`, all HIR/MIR has `'tcx` lifetime
- **swc** (Rust TypeScript/JS): Custom arena, 70x faster than Babel (arena is part of this)
- **Luau** (C++ Lua): Custom arena allocator
- **Pattern:** Every major systems-language compiler uses arenas for AST

**Benefits:**

- **10x faster allocation** - Bump pointer vs malloc overhead
- **Better cache locality** - Nodes allocated contiguously
- **Bulk deallocation** - O(1) free entire compilation unit
- **Scales to unknown use cases** - 500K line codebases, LSP, CI/CD

**Action Items:**

#### Phase 4.1: Arena Infrastructure (Days 1-2)

- [ ] Add `bumpalo` crate dependency to `typedlua-core/Cargo.toml`
- [ ] Create `crates/typedlua-core/src/arena.rs`
- [ ] Define `Arena` wrapper around `bumpalo::Bump`
- [ ] Add `alloc<T>(&self, value: T) -> &'arena T` helper
- [ ] Add `alloc_slice<T>(&self, slice: &[T]) -> &'arena [T]` helper
- [ ] Add unit tests for arena allocation/deallocation

#### Phase 4.2: AST Lifetime Migration (Days 3-7)

- [ ] Add `'arena` lifetime to `Program` struct
- [ ] Add `'arena` lifetime to `Statement` enum and all variants
- [ ] Add `'arena` lifetime to `Expression` enum and all variants
- [ ] Add `'arena` lifetime to `Type` enum and all variants
- [ ] Update all AST helper structs (FunctionDecl, ClassDecl, etc.)
- [ ] Replace `Box<Expr>` with `&'arena Expr`
- [ ] Replace `Vec<Stmt>` with `&'arena [Stmt]`
- [ ] Fix all compilation errors (expect 100+ across codebase)

#### Phase 4.3: Parser Integration (Days 8-10)

- [ ] Update `typedlua-parser` to accept `&'arena Arena` parameter
- [ ] Update `Parser::new()` signature to include arena
- [ ] Update all parser methods to allocate from arena
- [ ] Update `parse_single_file()` in CLI to create arena per file
- [ ] Ensure arena lifetime = parse lifetime (drop after parsing)
- [ ] Run parser tests to verify correctness

#### Phase 4.4: Type Checker Integration (Days 11-12)

- [ ] Update `TypeChecker` to accept AST with `'arena` lifetime
- [ ] Update all type checking functions with `'arena` parameter
- [ ] Ensure type checker doesn't extend AST lifetime beyond arena
- [ ] Run type checker tests to verify correctness

#### Phase 4.5: Optimizer Integration (Days 13-14)

- [ ] Update all optimization passes to accept `&'arena mut Program<'arena>`
- [ ] Update `Optimizer::optimize()` signature with arena lifetime
- [ ] Fix all pass implementations (18 passes)
- [ ] Ensure optimizations don't create dangling references
- [ ] Run optimizer tests to verify correctness

#### Phase 4.6: Code Generator Integration (Day 15)

- [ ] Update `CodeGenerator` to accept `&Program<'arena>`
- [ ] Update codegen to read from arena-allocated AST
- [ ] Ensure codegen doesn't extend AST lifetime
- [ ] Run codegen tests to verify correctness

#### Phase 4.7: Benchmarking & Validation (Days 16-18)

- [ ] Run full test suite (`cargo test`) - all tests must pass
- [ ] Profile allocation performance with flamegraph
- [ ] Benchmark compilation speed vs baseline (expect 2-5x allocation speedup)
- [ ] Test with small project (1K lines)
- [ ] Test with medium project (10K lines)
- [ ] Test with large project (100K lines) - verify no OOM
- [ ] Document performance improvements

**Expected:** 2-5x faster allocation, better memory predictability at scale

---

## Phase 5: Bundle Optimization (Pre-1.0 Required)

**Rationale:** Basic bundler features users expect (webpack, esbuild, Rollup all have these).

**Problem:** Current bundle mode includes all code, creates unnecessary module overhead.

### Tree Shaking (3-5 days)

**What:** Eliminate unused exports and modules from bundles.

**Why users expect this:**

- **webpack, esbuild, Rollup, SWC** all do this by default
- Including unused code is objectively wasteful
- Critical for library bundling (don't ship unused utilities)

**Action Items:**

#### Phase 5.1: Reachability Analysis (Days 1-2)

- [ ] Create `crates/typedlua-core/src/codegen/tree_shaking.rs`
- [ ] Implement `ReachabilityAnalysis` struct
- [ ] Add `analyze(entry: &Path, modules: &HashMap<Path, Program>) -> ReachableSet`
- [ ] Use BFS/DFS from entry point to collect reachable imports
- [ ] Track `(module_path, export_name)` pairs that are actually used
- [ ] Add `is_reachable(module: &Path, export: &str) -> bool` query
- [ ] Add unit tests for reachability with circular dependencies

#### Phase 5.2: Bundle Integration (Day 3)

- [ ] Update `CodeGenerator::generate_bundle()` to accept `ReachabilityAnalysis`
- [ ] Skip unreachable exports during bundle codegen
- [ ] Skip modules with no reachable exports
- [ ] Preserve entry point (always reachable)
- [ ] Add `--no-tree-shake` flag for debugging

#### Phase 5.3: Testing & Validation (Days 4-5)

- [ ] Test: Single module bundle (no shaking needed)
- [ ] Test: Unused function in module → removed
- [ ] Test: Unused entire module → removed
- [ ] Test: Transitive dependencies preserved
- [ ] Test: Entry point always included
- [ ] Benchmark bundle size reduction (expect 20-50% on typical projects)
- [ ] Test with real-world multi-module projects

**Expected:** 20-50% smaller bundles, faster load times

---

### Scope Hoisting (5-7 days)

**What:** Flatten module boundaries to reduce function call overhead.

**Current bundle structure:**

```lua
-- Module A
local module_a = {}
function module_a.foo() return 42 end
return module_a

-- Module B
local module_a = require("module_a")
local result = module_a.foo()  -- Indirect call through table
```

**After scope hoisting:**

```lua
-- Hoisted: modules flattened into shared scope
local function module_a_foo() return 42 end
local result = module_a_foo()  -- Direct call
```

**Benefits:**

- **Faster runtime** - Direct calls instead of table lookups
- **Smaller bundles** - No module wrapper boilerplate
- **Better minification** - More opportunities for name mangling

**Action Items:**

#### Phase 5.4: Escape Analysis (Days 1-2)

- [ ] Create `crates/typedlua-core/src/codegen/scope_hoisting.rs`
- [ ] Implement `can_hoist(decl: &Declaration, module: &Module) -> bool`
- [ ] Check: Does declaration escape module scope? (exported via `return`?)
- [ ] Check: Does declaration reference module-local state?
- [ ] Check: Safe to rename without conflicts?
- [ ] Build hoistable declaration set per module
- [ ] Add unit tests for escape analysis edge cases

#### Phase 5.5: Name Mangling (Days 3-4)

- [ ] Implement `mangle_name(module_path: &Path, name: &str) -> String`
- [ ] Generate unique names: `module_a.foo` → `module_a_foo`
- [ ] Handle name collisions across modules
- [ ] Preserve entry point names (user-facing API)
- [ ] Update all references to use mangled names

#### Phase 5.6: Hoisting Transform (Days 5-6)

- [ ] Update `generate_bundle()` to hoist declarations
- [ ] Move hoistable declarations to top-level scope
- [ ] Remove module wrapper boilerplate for hoisted modules
- [ ] Preserve module structure for non-hoistable code
- [ ] Add `--no-scope-hoist` flag for debugging

#### Phase 5.7: Testing & Validation (Day 7)

- [ ] Test: Simple module hoisting (single function)
- [ ] Test: Multiple modules hoisted into shared scope
- [ ] Test: Mixed hoisted + non-hoisted modules
- [ ] Test: Name collision handling
- [ ] Test: Circular dependencies still work
- [ ] Benchmark bundle size + runtime perf (expect 10-20% improvement)
- [ ] Test with real-world projects

**Expected:** 10-20% faster runtime, 10-15% smaller bundles

---

## Implementation Roadmap

**Week 0:** Quick Wins (FxHashMap, Lazy Pass Evaluation)

**Weeks 1-4:** Phase 3.5 Type Checker Optimization

- Week 1: Strategy 1 (Type Relation Caching) + Strategy 3 (RTA Devirtualization)
- Weeks 2-4: Strategy 2 (Declaration-Level Incremental)

**Weeks 5-7:** Phase 4 Arena Allocation (Pre-1.0 requirement)

**Weeks 8-9:** Phase 5 Bundle Optimization (Pre-1.0 requirement)

- Week 8: Tree Shaking (Days 1-5)
- Week 9: Scope Hoisting (Days 1-7)

**Target:** 1.0 release with production-grade performance architecture

**Week 0:** Quick Wins (FxHashMap, Lazy Pass Evaluation)

**Weeks 1-4:** Phase 3.5 Type Checker Optimization

- Week 1: Strategy 1 (Type Relation Caching) + Strategy 3 (RTA Devirtualization)
- Weeks 2-4: Strategy 2 (Declaration-Level Incremental)

**Weeks 5-7:** Phase 4 Arena Allocation (Pre-1.0 requirement)

**Target:** 1.0 release with production-grade performance architecture
