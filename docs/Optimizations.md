# Transpiler Performance Analysis: TypedLua vs. The Field

## Transpilers Analyzed

| Transpiler           | Language          | Target      | Written In                       |
|----------------------|-------------------|-------------|----------------------------------|
| **TypedLua**         | Typed Lua         | Lua 5.1-5.4 | Rust                             |
| **TypeScript (tsc)** | TypeScript        | JavaScript  | TypeScript (Go port in progress) |
| **esbuild**          | JS/TS             | JavaScript  | Go                               |
| **SWC**              | JS/TS             | JavaScript  | Rust                             |
| **Gleam**            | Gleam             | Erlang/JS   | Rust                             |
| **Luau**             | Typed Lua variant | Bytecode    | C++                              |

---

## 1. Compiler Architecture Comparison

### Pipeline Design

| Technique               | TypedLua            | tsc                | esbuild                | SWC             | Gleam                | Luau |
|-------------------------|---------------------|--------------------|------------------------|-----------------|----------------------|------|
| Multi-pass AST          | Yes (18 passes)     | Yes                | **3 merged passes**    | Yes             | Yes                  | Yes  |
| Fixed-point iteration   | Yes (max 10)        | No                 | No                     | No              | No                   | No   |
| Parallel compilation    | **No**              | No (Go port: Yes)  | **Yes (goroutines)**   | **Yes (rayon)** | No                   | No   |
| Incremental compilation | Yes (bincode cache) | Yes (.tsbuildinfo) | No                     | Partial         | **Yes (beam cache)** | N/A  |
| Bundling mode           | Yes                 | No (needs bundler) | **Yes (core feature)** | Yes             | No                   | N/A  |

**Key Insight**: esbuild's biggest performance win comes from **merging passes**. Instead of separate lex, parse, bind, fold, mangle passes, esbuild does lexing+parsing+scope+symbols in one pass, then binding+folding+lowering+mangling in a second. This reduces AST traversals from ~5-6 down to 3. TypedLua does 18+ separate passes at O3 with fixed-point iteration (up to 10 rounds), meaning the AST could be traversed **up to 180 times** in worst case.

### Pass Merging Opportunity for TypedLua

TypedLua's 18 passes organized by what could be merged:

**Group A - Expression-level transforms (could merge into 1 pass):**

- Constant folding
- Algebraic simplification
- Operator inlining

**Group B - Statement-level elimination (could merge into 1 pass):**

- Dead code elimination
- Dead store elimination

**Group C - Function transforms (could merge into 1 pass):**

- Function inlining (O2)
- Aggressive inlining (O3)
- Interface method inlining
- Method-to-function conversion
- Tail call optimization

**Group D - Data structure transforms (could merge into 1 pass):**

- Table preallocation
- Rich enum optimization
- String concatenation optimization

**Group E - Whole-program analysis (must remain separate):**

- Devirtualization (needs class hierarchy)
- Generic specialization (needs type info)
- Global localization (needs scope analysis)
- Loop optimization (needs loop detection)

Merging Groups A-D could reduce 14 passes down to 4, cutting AST traversals significantly.

---

## 2. Parallelism

### Current State of the Art

**esbuild** (the gold standard for parallel transpilation):

- Parallel file scanning (dependency graph traversal)
- Parallel parsing (each file parsed on its own goroutine)
- Parallel printing (code generation)
- Parallel source map generation
- Shared memory model (Go goroutines + channels)

**SWC**:

- Uses Rust's `rayon` for data parallelism
- Parallel file processing
- 20x faster than Babel single-threaded, 70x on 4 cores

**TypeScript Go port (Project Corsa)**:

- 10x improvement from Go's goroutine-based parallelism
- Parallel type checking across modules
- Shared memory between threads

### TypedLua Status: Single-threaded pipeline

**Gap**: TypedLua processes everything sequentially. For multi-file projects, this means:

- Files are parsed one at a time
- Type checking is sequential
- Optimization passes run on each file sequentially
- Code generation is sequential

**Opportunity**: Rust's `rayon` crate could enable:

1. Parallel file parsing (each file is independent until type checking)
2. Parallel code generation (after type checking, each module's codegen is independent)
3. Parallel optimization passes (per-module, after whole-program analysis)

**Estimated impact**: 2-4x improvement on multi-file projects (based on SWC/esbuild data).

---

## 3. Memory Management

### Arena Allocation

| Transpiler | Arena Alloc | Strategy                    |
|------------|-------------|-----------------------------|
| TypedLua   | **No**      | Standard `Box<T>`, `Vec<T>` |
| tsc        | No          | GC (JavaScript/Go)          |
| esbuild    | No          | Go GC (but compact structs) |
| SWC        | **Yes**     | Custom arena for AST nodes  |
| Gleam      | No          | Standard Rust allocations   |
| Luau       | **Yes**     | Custom arena allocator      |

**Why arena allocation matters for compilers:**

- AST nodes have uniform lifetime (created during parse, freed after codegen)
- Bump allocation is ~10x faster than malloc for small objects
- Better cache locality (nodes allocated contiguously)
- Bulk deallocation (free entire arena at once)

**TypedLua**: Not pursuing arena allocation. Standard `Box<T>`/`Vec<T>` allocation is sufficient for the current scale.

### String Interning

| Transpiler | String Interning     | Implementation                   |
|------------|----------------------|----------------------------------|
| TypedLua   | **Yes**              | `StringInterner` with `StringId` |
| tsc        | No (Go port may add) | Standard strings                 |
| esbuild    | No                   | Compact string representation    |
| SWC        | **Yes**              | `swc_atoms` crate                |
| Gleam      | Partial              | EcoString                        |
| Luau       | **Yes**              | Internal string pool             |

TypedLua is on par with the best here. String interning is used across all 27+ files in the compiler.

### Hash Map Performance

| Transpiler | Hash Map                  | Notes                    |
|------------|---------------------------|--------------------------|
| TypedLua   | **FxHashMap** (partial)   | Only in devirtualization |
| SWC        | **FxHashMap** (universal) | All internal maps        |
| esbuild    | Go built-in maps          | Optimized by Go runtime  |

**Gap**: TypedLua uses `FxHashMap` only in the devirtualization pass. Switching all internal `HashMap` usage to `FxHashMap` (or `ahash`) would improve lookup performance across the entire compiler, especially in symbol tables and type registries.

---

## 4. Output Optimization (Generated Code Quality)

### Type Erasure Strategy

| Transpiler | Type Erasure                    | Runtime Types                  |
|------------|---------------------------------|--------------------------------|
| TypedLua   | Complete erasure                | Optional reflection registry   |
| TypeScript | Complete erasure                | None                           |
| SWC        | Complete erasure (strips types) | None                           |
| Gleam      | Complete erasure                | Erlang tagged tuples           |
| Luau       | **Types preserved**             | VM uses types for optimization |

**Luau's unique advantage**: Unlike all other transpilers, Luau preserves type information into bytecode. The VM can use types to optimize arithmetic operations, skip type guards, and enable native codegen. This is possible because Luau controls both the compiler AND the runtime.

**TypedLua consideration**: Since TypedLua targets standard Lua VMs (which it doesn't control), complete type erasure is the correct strategy. The optional reflection system is a reasonable trade-off.

### Generated Code Patterns

**TypedLua strengths:**

- Lua version-specific strategies (5.1-5.4)
- String concat optimization (`..` chains to `table.concat`)
- Global localization (frequently used globals become locals)
- Table preallocation (reduces rehashing)
- Tail call preservation
- Three output formats (readable, compact, minified)

**Patterns used by other transpilers that TypedLua could adopt:**

1. **Scope hoisting** (esbuild, Rollup): Flatten module scopes to reduce function call overhead. In Lua terms, instead of each module returning a table, hoist declarations into a shared scope when bundling.

2. **Tree shaking** (esbuild, SWC, TypeScript): Eliminate unused exports. TypedLua's bundle mode includes all modules - it could skip modules/exports that are never imported.

3. **Constant propagation across modules**: If module A exports `const X = 42` and module B imports X, inline `42` directly instead of the `require` call.

---

## 5. Caching & Incremental Compilation

| Feature               | TypedLua                | tsc                 | esbuild | Gleam                 |
|-----------------------|-------------------------|---------------------|---------|-----------------------|
| Cache format          | bincode (binary)        | JSON (.tsbuildinfo) | None    | .cache + .beam        |
| Integrity check       | **BLAKE3 hash**         | File timestamps     | N/A     | File timestamps       |
| Cache hit rate target | 95%+                    | ~90%                | N/A     | ~95%                  |
| Incremental speedup   | 2x+                     | 2-5x                | N/A     | **128x** (Gleam 0.26) |
| Stale detection       | mtime + transitive deps | Dependency graph    | N/A     | Module graph          |

**TypedLua is strong here.** BLAKE3 hashing is more robust than timestamp-based invalidation (which can miss changes from `git checkout`). The binary cache format (bincode) is faster to serialize/deserialize than JSON.

**Gleam's lesson**: Their 128x improvement (18s to 140ms) came from aggressive caching where unchanged modules load pre-compiled bytecode AND cached type information. TypedLua could similarly cache type-checked ASTs to skip both parsing AND type checking for unchanged modules.

---

## 6. Diagnostic & Compilation Performance

### Compilation Speed Targets

| Transpiler | Small Project     | Medium Project | Large Project |
|------------|-------------------|----------------|---------------|
| TypedLua   | <100ms (1K lines) | <500ms (10K)   | <5s (100K)    |
| tsc        | ~1-3s (any size)  | 5-15s          | 30-90s        |
| esbuild    | <50ms             | <200ms         | <1s           |
| SWC        | <50ms             | <150ms         | <500ms        |

TypedLua's targets are reasonable but not best-in-class. esbuild and SWC achieve better numbers through parallelism and fewer passes.

---

## 7. Implementation Plan

### Phase 1: Quick Wins

#### 1.1 Universal FxHashMap

- Replace all `std::HashMap` with `FxHashMap` across the compiler
- Currently only used in devirtualization pass
- Files to modify: all files in `crates/typedlua-core/src/` using `std::collections::HashMap`
- Add `rustc-hash` to `Cargo.toml` dependencies (already a transitive dep)
- Status: **Not started**

```rust
// Cargo.toml
[dependencies]
rustc-hash = "2"

// In each file, replace:
use std::collections::HashMap;
// With:
use rustc_hash::FxHashMap;

// Then replace HashMap<K, V> with FxHashMap<K, V> throughout
// Also replace HashSet<K> with FxHashSet<K> where applicable
```

#### 1.2 Lazy Pass Evaluation

- Add a bitflag pre-scan before each optimization pass
- During parsing or as a cheap pre-pass, set flags for AST features present in the module
- Skip passes whose required features aren't present
- Files to modify:
  - `crates/typedlua-core/src/optimizer/mod.rs` (pass dispatch logic)
  - Each pass file (add `required_features() -> AstFeatureFlags` method)
- Status: **Not started**

```rust
// crates/typedlua-core/src/optimizer/mod.rs

bitflags::bitflags! {
    pub struct AstFeatureFlags: u32 {
        const HAS_STRING_CONCAT   = 0b0000_0001;
        const HAS_FUNCTION_DEF    = 0b0000_0010;
        const HAS_LOOP            = 0b0000_0100;
        const HAS_ENUM            = 0b0000_1000;
        const HAS_CLASS           = 0b0001_0000;
        const HAS_GENERIC         = 0b0010_0000;
        const HAS_MATCH           = 0b0100_0000;
        const HAS_ARITHMETIC      = 0b1000_0000;
        const HAS_GLOBAL_ACCESS   = 0b0001_0000_0000;
        const HAS_METHOD_CALL     = 0b0010_0000_0000;
        const HAS_INTERFACE       = 0b0100_0000_0000;
    }
}

/// Quick AST scan to detect which features are present.
/// Runs once before optimization, avoids full traversals for inapplicable passes.
fn scan_features(program: &Program) -> AstFeatureFlags {
    let mut flags = AstFeatureFlags::empty();
    for stmt in &program.body {
        scan_stmt(stmt, &mut flags);
    }
    flags
}

trait OptimizationPass {
    fn name(&self) -> &'static str;
    fn min_level(&self) -> OptLevel;
    /// Which AST features must be present for this pass to have any effect.
    fn required_features(&self) -> AstFeatureFlags;
    fn run(&mut self, program: &mut Program, interner: &StringInterner) -> Result<bool, Error>;
}

// In the optimization loop:
let features = scan_features(&program);
for pass in &mut passes {
    if pass.min_level() <= level
        && features.contains(pass.required_features())
    {
        changed |= pass.run(&mut program, &interner)?;
    }
}
```

### Phase 2: Composite Visitor

#### 2.1 OptimizationVisitor Trait

- Define a shared visitor trait; each pass implements it in its own file
- Dispatcher walks the AST once, calling all eligible visitors at each node
- Group compatible passes into composite traversals:
  - **Expression group**: constant folding + algebraic simplification + operator inlining
  - **Elimination group**: dead code elimination + dead store elimination
  - **Function group**: function inlining + aggressive inlining + interface inlining + method-to-function + tail call optimization
  - **Data structure group**: table preallocation + rich enum optimization + string concat optimization
- Whole-program passes (devirtualization, generic specialization, global localization, loop optimization) remain separate
- Reduces 14 passes to 4 composite traversals + 4 standalone = 8 total
- Files to modify:
  - `crates/typedlua-core/src/optimizer/mod.rs` (new trait + dispatcher)
  - All pass files in `crates/typedlua-core/src/optimizer/passes/` (implement trait)
- Status: **Not started**

```rust
// crates/typedlua-core/src/optimizer/mod.rs

/// Trait for passes that can participate in composite traversals.
/// Each pass stays in its own file but implements this shared interface.
trait ExprVisitor {
    fn visit_expr(&mut self, expr: &mut Expr, interner: &StringInterner) -> bool;
}

trait StmtVisitor {
    fn visit_stmt(&mut self, stmt: &mut Stmt, interner: &StringInterner) -> bool;
}

/// Runs multiple ExprVisitors in a single AST traversal.
fn run_composite_expr_pass(
    program: &mut Program,
    visitors: &mut [&mut dyn ExprVisitor],
    interner: &StringInterner,
) -> bool {
    let mut changed = false;
    for stmt in &mut program.body {
        for expr in stmt.expressions_mut() {
            for visitor in visitors.iter_mut() {
                changed |= visitor.visit_expr(expr, interner);
            }
        }
    }
    changed
}

// Usage in optimization loop:
let mut const_fold = ConstantFolding::new();
let mut algebraic = AlgebraicSimplification::new();
let mut op_inline = OperatorInlining::new();

let changed = run_composite_expr_pass(
    &mut program,
    &mut [&mut const_fold, &mut algebraic, &mut op_inline],
    &interner,
);
```

### Phase 3: Parallel File Processing

#### 3.1 Parallel Parsing (easy)

- Files parse independently with no shared mutable state
- Use per-thread `StringInterner` instances, merge after parsing
- Wrap file parsing loop in `par_iter`
- Files to modify:
  - `crates/typedlua-core/Cargo.toml`
  - `crates/typedlua-cli/src/main.rs` (multi-file compilation loop)
- Status: **Not started**

```rust
// Cargo.toml
[dependencies]
rayon = "1"

// crates/typedlua-cli/src/main.rs
use rayon::prelude::*;

// Per-thread parsing with local interners, merged after
let parsed_modules: Vec<ParsedModule> = source_files
    .par_iter()
    .map(|file| {
        let (mut interner, common) = StringInterner::new_with_common_identifiers();
        let tokens = Lexer::new(&file.content, &mut interner).tokenize()?;
        let ast = Parser::new(tokens, &mut interner, &common).parse()?;
        Ok(ParsedModule {
            path: file.path.clone(),
            ast,
            interner,
        })
    })
    .collect::<Result<Vec<_>, Error>>()?;

// Merge interners into a single shared interner for type checking
let mut master_interner = StringInterner::new();
let remap_tables: Vec<_> = parsed_modules.iter()
    .map(|m| master_interner.merge_from(&m.interner))
    .collect();
// Remap StringIds in each AST using remap_tables
```

#### 3.2 Parallel Codegen (easy)

- After type checking, each module's code generation is independent
- Same per-thread pattern as 3.1
- Files to modify:
  - `crates/typedlua-cli/src/main.rs` (codegen loop)
- Status: **Not started**

```rust
// After sequential type checking completes:
let outputs: Vec<(PathBuf, String)> = checked_modules
    .par_iter()
    .map(|module| {
        let mut codegen = CodeGenerator::new(&module.ast, &interner, &options);
        let lua_code = codegen.generate()?;
        Ok((module.output_path.clone(), lua_code))
    })
    .collect::<Result<Vec<_>, Error>>()?;
```

#### 3.3 Parallel Optimization (moderate)

- Per-module optimization at O1-O2 can parallelize after type checking
- O3 passes (devirtualization) need whole-program analysis first, then per-module transforms parallelize
- Files to modify:
  - `crates/typedlua-core/src/optimizer/mod.rs`
  - `crates/typedlua-cli/src/main.rs`
- Status: **Not started**

```rust
// Two-phase approach for O3:
// Phase 1: Sequential whole-program analysis
let class_hierarchy = build_class_hierarchy(&all_modules);
let devirt_info = analyze_devirtualization(&class_hierarchy);

// Phase 2: Parallel per-module optimization using shared analysis
let optimized: Vec<_> = modules
    .par_iter_mut()
    .map(|module| {
        let mut optimizer = Optimizer::new(level, &interner);
        optimizer.set_devirt_info(&devirt_info); // read-only shared data
        optimizer.optimize(&mut module.ast)?;
        Ok(module)
    })
    .collect::<Result<Vec<_>, Error>>()?;
```

#### 3.4 Parallel Type Checking (hard, deferred)

- Cross-module dependencies require dependency-ordered scheduling
- Independent modules (no import relationship) can check in parallel
- Dependent modules must wait for their dependencies to complete
- Status: **Deferred**

```rust
// Conceptual approach (not implementing yet):
// Use topological sort to identify independent "levels"
let levels = dependency_graph.parallel_levels();
// levels[0] = modules with no deps (can all check in parallel)
// levels[1] = modules depending only on level 0 (parallel after level 0 done)
// etc.
for level in levels {
    let results: Vec<_> = level.par_iter()
        .map(|module| type_check(module, &registry))
        .collect();
    // Register results in shared registry before next level
    for result in results {
        registry.register_module(result);
    }
}
```

### Phase 4: LSP Performance

#### 4.1 LSP Cache Integration

- The LSP bridge creates fresh state on every `check_document()` call
- Integrate the existing `CacheManager` and `ModuleRegistry` into LSP server state
- Only re-parse and re-check files that have actually changed
- Files to modify:
  - `crates/typedlua-lsp/src/impls/compiler_bridge.rs`
  - `crates/typedlua-lsp/src/impls/` (server state management)
- Status: **Not started**

```rust
// crates/typedlua-lsp/src/impls/compiler_bridge.rs

/// Long-lived compilation state persisted across LSP requests.
pub struct CompilationState {
    interner: StringInterner,
    common_ids: CommonIdentifiers,
    module_registry: Arc<ModuleRegistry>,
    cache_manager: CacheManager,
    /// Cached per-file results: AST + diagnostics + symbol table
    file_cache: FxHashMap<PathBuf, CachedFileState>,
    /// Track file versions to detect changes
    file_versions: FxHashMap<PathBuf, i32>,
}

struct CachedFileState {
    ast: Program,
    diagnostics: Vec<Diagnostic>,
    symbol_table: SymbolTable,
    version: i32,
}

impl CompilationState {
    fn check_document(&mut self, uri: &Url, text: &str, version: i32) -> TypeCheckResult {
        let path = uri.to_file_path().unwrap();

        // Skip if unchanged
        if let Some(cached) = self.file_cache.get(&path) {
            if cached.version == version {
                return TypeCheckResult::from_cache(cached);
            }
        }

        // Only re-parse and re-check the changed file
        let tokens = Lexer::new(text, &mut self.interner).tokenize()?;
        let ast = Parser::new(tokens, &mut self.interner, &self.common_ids).parse()?;
        let checker = TypeChecker::new_with_module_support(
            handler, &self.interner, &self.common_ids,
            &self.module_registry, module_id, resolver,
        );
        let result = checker.check(&ast);

        // Cache the result
        self.file_cache.insert(path, CachedFileState {
            ast, diagnostics: result.diagnostics.clone(),
            symbol_table: result.symbols.clone(), version,
        });

        result
    }
}
```

#### 4.2 Type Pooling (requires typechecker crate changes)

- Pool common type instances to avoid repeated allocation
- Similar pattern to `CommonIdentifiers` for string interning
- Worth pursuing if profiling shows type allocation as a bottleneck
- Files to modify: `typedlua-typechecker` crate
- Status: **Not started**

```rust
// typedlua-typechecker crate

pub struct TypePool {
    pub any: Arc<Type>,
    pub unknown: Arc<Type>,
    pub never: Arc<Type>,
    pub void: Arc<Type>,
    pub string: Arc<Type>,
    pub number: Arc<Type>,
    pub boolean: Arc<Type>,
    pub nil: Arc<Type>,
    /// Cache for generic instantiations: (base_type, args) -> specialized
    generic_cache: FxHashMap<(TypeId, Vec<TypeId>), Arc<Type>>,
}

impl TypePool {
    pub fn new() -> Self {
        Self {
            any: Arc::new(Type::Any),
            unknown: Arc::new(Type::Unknown),
            // ...
            generic_cache: FxHashMap::default(),
        }
    }

    pub fn get_or_specialize(&mut self, base: TypeId, args: &[TypeId]) -> Arc<Type> {
        let key = (base, args.to_vec());
        self.generic_cache.entry(key)
            .or_insert_with(|| Arc::new(specialize(base, args)))
            .clone()
    }
}
```

#### 4.3 Type Relationship Cache (requires typechecker crate changes)

- Cache subtype/assignability check results
- Worth pursuing if profiling shows repeated subtype checks
- Files to modify: `typedlua-typechecker` crate
- Status: **Not started**

```rust
// typedlua-typechecker crate

pub struct TypeRelationCache {
    assignable: FxHashMap<(TypeId, TypeId), bool>,
    subtype: FxHashMap<(TypeId, TypeId), bool>,
}

impl TypeRelationCache {
    pub fn is_assignable_to(&mut self, from: TypeId, to: TypeId, checker: &TypeChecker) -> bool {
        if let Some(&result) = self.assignable.get(&(from, to)) {
            return result;
        }
        let result = checker.compute_assignability(from, to);
        self.assignable.insert((from, to), result);
        result
    }
}
```

### Phase 5: Output Quality

#### 5.1 Tree Shaking in Bundle Mode

- Eliminate unused exports/modules when bundling
- Previous implementation caused cascading issues
- New approach: mark-and-sweep from entry point, separate from codegen
- Files to modify:
  - `crates/typedlua-core/src/codegen/mod.rs` (bundle generation)
  - `crates/typedlua-cli/src/main.rs` (dependency analysis)
- Status: **Not started**

```rust
// crates/typedlua-core/src/codegen/mod.rs

/// Reachability analysis for tree shaking.
/// Runs before codegen, produces a set of reachable exports.
pub struct ReachabilityAnalysis {
    /// Set of (module_path, export_name) pairs that are actually used
    reachable: FxHashSet<(PathBuf, String)>,
}

impl ReachabilityAnalysis {
    pub fn analyze(entry: &PathBuf, modules: &FxHashMap<PathBuf, Program>) -> Self {
        let mut analysis = Self { reachable: FxHashSet::default() };
        let mut worklist = vec![entry.clone()];
        let mut visited = FxHashSet::default();

        while let Some(module_path) = worklist.pop() {
            if !visited.insert(module_path.clone()) {
                continue;
            }
            if let Some(program) = modules.get(&module_path) {
                for import in &program.imports {
                    // Mark imported symbols as reachable
                    for symbol in &import.symbols {
                        analysis.reachable.insert((import.source.clone(), symbol.name.clone()));
                    }
                    worklist.push(import.source.clone());
                }
            }
        }
        analysis
    }

    pub fn is_reachable(&self, module: &Path, export: &str) -> bool {
        self.reachable.contains(&(module.to_path_buf(), export.to_string()))
    }
}

// During bundle codegen, skip unreachable exports:
if !reachability.is_reachable(&module_path, &export_name) {
    continue; // Don't emit this export
}
```

#### 5.2 Cross-module Constant Propagation

- If module A exports `const X = 42` and module B imports X, inline `42` directly
- Requires tracking which exports are compile-time constants
- Files to modify:
  - `crates/typedlua-core/src/codegen/mod.rs`
  - `crates/typedlua-core/src/optimizer/` (new cross-module pass)
- Status: **Not started**

```rust
// Constant export tracking during type checking / codegen:
pub struct ConstantExports {
    /// module_path -> (export_name -> constant_value)
    constants: FxHashMap<PathBuf, FxHashMap<String, LiteralValue>>,
}

enum LiteralValue {
    Number(f64),
    String(String),
    Boolean(bool),
    Nil,
}

// During codegen of an import:
// Instead of: local X = require("module_a").X
// Emit:       local X = 42
fn resolve_import(&self, import: &Import, constants: &ConstantExports) -> Option<Expr> {
    if let Some(module_consts) = constants.constants.get(&import.source) {
        if let Some(value) = module_consts.get(&import.symbol) {
            return Some(value.to_expr());
        }
    }
    None // Fall back to normal require()
}
```

#### 5.3 Optimization Pass Statistics

- Track which passes make changes per-module across fixed-point iterations
- If a pass consistently does nothing for a module, skip it in subsequent iterations
- Files to modify:
  - `crates/typedlua-core/src/optimizer/mod.rs`
- Status: **Not started**

```rust
// crates/typedlua-core/src/optimizer/mod.rs

struct PassStats {
    /// Number of times this pass made changes, indexed by pass name
    change_count: FxHashMap<&'static str, u32>,
    /// Number of times this pass was run without making changes
    no_change_count: FxHashMap<&'static str, u32>,
}

impl PassStats {
    /// After 3 consecutive no-ops, skip this pass for the rest of the
    /// fixed-point loop on this module.
    fn should_skip(&self, pass_name: &str) -> bool {
        let no_changes = self.no_change_count.get(pass_name).copied().unwrap_or(0);
        no_changes >= 3
    }

    fn record(&mut self, pass_name: &'static str, changed: bool) {
        if changed {
            *self.change_count.entry(pass_name).or_default() += 1;
            self.no_change_count.insert(pass_name, 0); // Reset streak
        } else {
            *self.no_change_count.entry(pass_name).or_default() += 1;
        }
    }
}
```

---

## 8. Existing Infrastructure (Already Implemented)

| Feature                 | Status   | Implementation                                                                         |
|-------------------------|----------|----------------------------------------------------------------------------------------|
| AST persistence layer   | **Done** | `CachedModule` stores full AST, exports, symbol table, interner via bincode            |
| Symbol table caching    | **Done** | `SerializableSymbolTable` stored per-module in cache                                   |
| Dependency graph        | **Done** | `CacheManifest.dependencies` with transitive invalidation via `InvalidationEngine` BFS |
| Import resolution       | **Done** | `ImportScanner` (lightweight lexer, no full parse) + `ModuleResolver`                  |
| Avoid double parsing    | **Done** | Discovery uses lightweight lexer scanner, not full parser                              |
| File-level invalidation | **Done** | BLAKE3 source hash + config hash + transitive dep tracking                             |
| String interning        | **Done** | `StringInterner` with `StringId` across 27+ files                                      |
| Incremental compilation | **Done** | Cache hit path skips lex/parse/typecheck entirely                                      |
| Multi-file compilation  | **Done** | Topological sort + sequential compile with module registry                             |

## 9. Evaluated & Deferred

| Proposed                           | Assessment                                                                                             |
|------------------------------------|--------------------------------------------------------------------------------------------------------|
| Declaration-level dependency graph | TypeScript doesn't do this either; file-level is sufficient                                            |
| Deferred type checking             | Only benefits LSP; requires modifying external type checker crate; CLI needs full check before codegen |
| Partial rechecking                 | Massive complexity; current file-level invalidation is adequate                                        |
| Hot module detection               | Pattern for long-running build servers, not CLI compilers                                              |

---

## 10. What TypedLua Already Does Well

- **18 optimization passes** across 3 tiers - more comprehensive than most transpilers
- **Fixed-point iteration** - ensures passes compose correctly (most transpilers don't do this)
- **String interning** - universal across the compiler
- **BLAKE3 cache validation** - more robust than timestamp-based approaches
- **Lua version strategies** - unique among transpilers, handles 5.1-5.4 differences
- **Devirtualization** - advanced optimization that most transpilers skip entirely
- **Generic specialization (monomorphization)** - Rust/C++-level optimization
- **Incremental compilation** with binary caching
- **Multiple output formats** (readable, compact, minified)

---

## Sources

- [TypeScript Performance Wiki](https://github.com/microsoft/Typescript/wiki/Performance)
- [TypeScript Native Port (Go)](https://devblogs.microsoft.com/typescript/typescript-native-port/)
- [esbuild FAQ - Why Fast](https://esbuild.github.io/faq/)
- [esbuild Architecture](https://github.com/evanw/esbuild/blob/main/docs/architecture.md)
- [SWC - Rust-based Web Compiler](https://swc.rs/)
- [Gleam Incremental Compilation](https://gleam.run/news/v0.26-incremental-compilation-and-deno/)
- [Luau Performance](https://luau.org/performance/)
- [Arena Allocation in Compilers](http://www.inferara.com/blog/arena-based-allocation-in-compilers/)
- [Gleam Performance Improvements](https://gleam.run/news/improved-performance-and-publishing/)
