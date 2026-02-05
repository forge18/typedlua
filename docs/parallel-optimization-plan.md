# Phase 3.3: Parallel Optimization Implementation Plan

**Status:** Planning phase - NOT YET IMPLEMENTED
**Complexity:** Moderate
**Expected Impact:** Additional 1.5-2x speedup on top of parallel codegen for large projects with O3 optimizations

---

## Current Architecture

### How Optimization Works Today

1. **Location:** Optimization runs **inside** `CodeGenerator::generate()` (codegen/mod.rs:217)
2. **Timing:** Happens per-module during the parallel codegen phase
3. **Limitation:** Each module only sees its own AST - no cross-module analysis

```rust
// crates/typedlua-core/src/codegen/mod.rs:212
pub fn generate(&mut self, program: &mut Program) -> String {
    if self.optimization_level != OptimizationLevel::O0 {
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut optimizer = Optimizer::new(
            self.optimization_level,
            handler,
            self.interner.clone()
        );
        let _ = optimizer.optimize(program);  // Per-module optimization
    }

    // ... code generation continues
}
```

### Optimizer Structure

The optimizer runs multiple passes in a fixed-point loop (up to 10 iterations):

**Pass Types:**
1. **Composite passes** (merged AST traversals):
   - Expression transforms (constant folding, algebraic simplification, operator inlining)
   - Statement elimination (dead code, dead store)
   - Function transforms (inlining, tail call optimization)
   - Data structure transforms (table preallocation, string concat, rich enums)

2. **Standalone passes** (whole-program analysis):
   - **Devirtualization** - needs ClassHierarchy across all modules
   - **Generic specialization** - needs type information across modules
   - **Global localization** - needs global usage analysis
   - **Loop optimization** - per-module (can parallelize)

---

## The Problem: Cross-Module Whole-Program Analysis

### Example: Devirtualization

```typescript
// module_a.tl
export class Base {
    method foo(): void { }
}

// module_b.tl
import { Base } from "./module_a"

export class Child extends Base {
    override method foo(): void { }
}

// module_c.tl
import { Child } from "./module_b"

function test(obj: Child) {
    obj.foo()  // Can this be devirtualized?
}
```

**Current behavior:** When optimizing `module_c.tl` in parallel, the optimizer only sees its own AST. It doesn't know:
- That `Child` extends `Base`
- Whether `Child.foo` is final or can be overridden further
- The complete class hierarchy

**Result:** Devirtualization opportunities are missed in multi-module projects.

---

## Proposed Architecture: Two-Phase Optimization

### Phase 1: Sequential Whole-Program Analysis (Before Parallel Codegen)

Extract analysis passes that need cross-module information:

```rust
// NEW: Whole-program analysis results (thread-safe, read-only)
pub struct WholeProgramAnalysis {
    /// Class hierarchy for devirtualization
    pub class_hierarchy: Arc<ClassHierarchy>,

    /// Generic specialization candidates
    pub generic_specializations: Arc<FxHashMap<TypeId, Vec<TypeId>>>,

    /// Global variable usage across all modules
    pub global_usage: Arc<FxHashMap<StringId, GlobalUsageInfo>>,
}

impl WholeProgramAnalysis {
    /// Build analysis by scanning all type-checked modules
    pub fn build(modules: &[CheckedModule]) -> Self {
        // Scan all modules to build ClassHierarchy
        let class_hierarchy = ClassHierarchy::build_multi_module(modules);

        // Analyze generic usage patterns
        let generic_specializations = analyze_generic_usage(modules);

        // Track global variable usage
        let global_usage = analyze_global_usage(modules);

        Self {
            class_hierarchy: Arc::new(class_hierarchy),
            generic_specializations: Arc::new(generic_specializations),
            global_usage: Arc::new(global_usage),
        }
    }
}
```

### Phase 2: Parallel Per-Module Optimization (During Codegen)

Each module gets optimized in parallel with shared read-only analysis:

```rust
// In main.rs - NEW compilation pipeline:

// Phase 1: Sequential type checking (EXISTING)
let checked_modules: Vec<CheckedModule> = /* ... */;

// Phase 2: Sequential whole-program analysis (NEW)
let analysis = if optimization_level >= OptimizationLevel::O3 {
    Some(WholeProgramAnalysis::build(&checked_modules))
} else {
    None
};

// Phase 3: Parallel optimization + codegen (MODIFIED)
let results: Vec<CompilationResult> = checked_modules
    .into_par_iter()
    .map(|module| {
        let mut builder = CodeGeneratorBuilder::new(module.interner.clone())
            .target(target)
            .output_format(output_format)
            .optimization_level(optimization_level);

        // NEW: If we have whole-program analysis, pass it to the builder
        if let Some(ref analysis) = analysis {
            builder = builder.with_whole_program_analysis(analysis.clone());
        }

        let mut generator = builder.build();

        // Optimization now has access to cross-module analysis
        let lua_code = generator.generate(&mut module.ast)?;

        CompilationResult { /* ... */ }
    })
    .collect();
```

---

## Implementation Steps

### Step 1: Extract Whole-Program Analysis Logic

**Files to modify:**
- `crates/typedlua-core/src/optimizer/devirtualization.rs`
- `crates/typedlua-core/src/optimizer/generic_specialization.rs`
- `crates/typedlua-core/src/optimizer/global_localization.rs`

**Changes:**

```rust
// devirtualization.rs
impl ClassHierarchy {
    // EXISTING: Single-module builder
    pub fn build(program: &Program) -> Self { /* ... */ }

    // NEW: Multi-module builder
    pub fn build_multi_module(modules: &[CheckedModule]) -> Self {
        let mut hierarchy = ClassHierarchy::default();

        // Scan all modules to build complete hierarchy
        for module in modules {
            for stmt in &module.ast.statements {
                if let Statement::Class(class) = stmt {
                    // Register class, parent relationships, finality, etc.
                    hierarchy.register_class(class, &module.interner);
                }
            }
        }

        // Build children_of map from parent_of map
        hierarchy.finalize();
        hierarchy
    }
}
```

### Step 2: Create WholeProgramAnalysis Struct

**New file:** `crates/typedlua-core/src/optimizer/whole_program_analysis.rs`

```rust
use std::sync::Arc;
use rustc_hash::FxHashMap;
use crate::optimizer::devirtualization::ClassHierarchy;
use typedlua_parser::string_interner::StringId;

/// Thread-safe whole-program analysis results
///
/// This struct contains analysis that requires cross-module information.
/// It's built once sequentially, then shared (read-only) across parallel
/// optimization passes.
#[derive(Clone)]
pub struct WholeProgramAnalysis {
    pub class_hierarchy: Arc<ClassHierarchy>,
    pub global_usage: Arc<FxHashMap<StringId, GlobalUsageInfo>>,
    // Add more analysis results as needed
}

#[derive(Debug, Clone)]
pub struct GlobalUsageInfo {
    pub total_uses: usize,
    pub modules_using: Vec<PathBuf>,
}

impl WholeProgramAnalysis {
    pub fn build(
        modules: &[CheckedModule],
        optimization_level: OptimizationLevel,
    ) -> Self {
        // Only run expensive analysis if needed
        let class_hierarchy = if optimization_level >= OptimizationLevel::O3 {
            ClassHierarchy::build_multi_module(modules)
        } else {
            ClassHierarchy::default()
        };

        let global_usage = analyze_global_usage(modules);

        Self {
            class_hierarchy: Arc::new(class_hierarchy),
            global_usage: Arc::new(global_usage),
        }
    }
}

fn analyze_global_usage(modules: &[CheckedModule]) -> FxHashMap<StringId, GlobalUsageInfo> {
    let mut usage = FxHashMap::default();

    for module in modules {
        // Scan for global variable access
        for stmt in &module.ast.statements {
            // Track which globals are used in which modules
            // ...
        }
    }

    usage
}
```

### Step 3: Thread Through CodeGenerator

**Files to modify:**
- `crates/typedlua-core/src/codegen/mod.rs`
- `crates/typedlua-core/src/codegen/builder.rs`

```rust
// builder.rs
pub struct CodeGeneratorBuilder {
    interner: Arc<StringInterner>,
    target: LuaTarget,
    output_format: OutputFormat,
    optimization_level: OptimizationLevel,
    whole_program_analysis: Option<WholeProgramAnalysis>,  // NEW
    // ...
}

impl CodeGeneratorBuilder {
    // NEW
    pub fn with_whole_program_analysis(
        mut self,
        analysis: WholeProgramAnalysis
    ) -> Self {
        self.whole_program_analysis = Some(analysis);
        self
    }
}

// mod.rs
pub struct CodeGenerator {
    interner: Arc<StringInterner>,
    optimization_level: OptimizationLevel,
    whole_program_analysis: Option<WholeProgramAnalysis>,  // NEW
    // ...
}

impl CodeGenerator {
    pub fn generate(&mut self, program: &mut Program) -> String {
        if self.optimization_level != OptimizationLevel::O0 {
            let handler = Arc::new(CollectingDiagnosticHandler::new());
            let mut optimizer = Optimizer::new(
                self.optimization_level,
                handler,
                self.interner.clone()
            );

            // NEW: Pass whole-program analysis to optimizer
            if let Some(ref analysis) = self.whole_program_analysis {
                optimizer.set_whole_program_analysis(analysis.clone());
            }

            let _ = optimizer.optimize(program);
        }

        // ... code generation continues
    }
}
```

### Step 4: Update Optimizer to Use Analysis

**Files to modify:**
- `crates/typedlua-core/src/optimizer/mod.rs`

```rust
pub struct Optimizer {
    level: OptimizationLevel,
    handler: Arc<dyn DiagnosticHandler>,
    interner: Arc<StringInterner>,

    // NEW: Whole-program analysis (optional, for O3+)
    whole_program_analysis: Option<WholeProgramAnalysis>,

    // Composite passes
    expr_pass: Option<ExpressionCompositePass>,
    elim_pass: Option<StatementCompositePass>,
    func_pass: Option<AnalysisCompositePass>,
    data_pass: Option<ExpressionCompositePass>,

    // Standalone passes
    standalone_passes: Vec<Box<dyn WholeProgramPass>>,
}

impl Optimizer {
    pub fn new(
        level: OptimizationLevel,
        handler: Arc<dyn DiagnosticHandler>,
        interner: Arc<StringInterner>,
    ) -> Self {
        // ... existing initialization
    }

    // NEW
    pub fn set_whole_program_analysis(&mut self, analysis: WholeProgramAnalysis) {
        self.whole_program_analysis = Some(analysis);

        // Update passes that need cross-module information
        for pass in &mut self.standalone_passes {
            if let Some(devirt) = pass.as_any_mut().downcast_mut::<DevirtualizationPass>() {
                devirt.set_class_hierarchy(analysis.class_hierarchy.clone());
            }
            // ... update other passes
        }
    }
}
```

### Step 5: Update main.rs Compilation Pipeline

**File to modify:**
- `crates/typedlua-cli/src/main.rs`

```rust
// Around line 1280 - after type checking, before parallel codegen

// NEW: Build whole-program analysis for O3+ optimizations
let whole_program_analysis = if optimization_level >= OptimizationLevel::O3 {
    info!("Building whole-program analysis for O3 optimizations...");
    Some(WholeProgramAnalysis::build(&checked_modules, optimization_level))
} else {
    None
};

// Parallel code generation (MODIFIED - add analysis)
let results: Vec<CompilationResult> = checked_modules
    .into_par_iter()
    .map(|module| {
        let output_format = parse_output_format(&cli.format);
        let mut builder = CodeGeneratorBuilder::new(module.interner.clone())
            .target(target)
            .output_format(output_format)
            .optimization_level(optimization_level);

        // NEW: Pass whole-program analysis if available
        if let Some(ref analysis) = whole_program_analysis {
            builder = builder.with_whole_program_analysis(analysis.clone());
        }

        let mut generator = builder.build();
        let mut ast = module.ast;

        let lua_code = generator.generate(&mut ast)
            .map_err(|e| anyhow::anyhow!("Code generation failed: {}", e))?;

        Ok(CompilationResult {
            file_path: module.file_path,
            output_path: module.output_path,
            lua_code,
            source_map: generator.source_map(),
            cache_entry: module.cache_entry,
        })
    })
    .collect::<Result<Vec<_>, _>>()?;
```

---

## Benefits

### Performance Improvements

1. **Parallel per-module optimization:** Each module's fixed-point iteration (up to 10 rounds × 18 passes) runs in parallel
2. **Better devirtualization:** Cross-module class hierarchy enables more aggressive optimization
3. **Estimated speedup:** Additional 1.5-2x on top of parallel codegen for large multi-module projects with O3

### Correctness Improvements

1. **Accurate whole-program analysis:** Cross-module class hierarchies, generic usage patterns
2. **Safe devirtualization:** Can properly determine when methods are final across module boundaries
3. **Better global localization:** Track global usage across entire project

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_cross_module_devirtualization() {
    // Create two modules: base class in module A, child class in module B
    let module_a = parse_module("class Base { method foo() { } }");
    let module_b = parse_module("import { Base } from 'A'; class Child extends Base { }");

    // Build whole-program analysis
    let analysis = WholeProgramAnalysis::build(&[module_a, module_b], OptimizationLevel::O3);

    // Verify class hierarchy is correct
    assert!(analysis.class_hierarchy.is_child_of("Child", "Base"));

    // Optimize module B with analysis
    let mut optimizer = Optimizer::new(OptimizationLevel::O3, handler, interner);
    optimizer.set_whole_program_analysis(analysis);
    optimizer.optimize(&mut module_b.ast).unwrap();

    // Verify devirtualization happened
}
```

### Integration Tests

1. **Multi-module compilation:** Compile a project with 10+ modules, verify O3 optimizations work
2. **Benchmark:** Compare before/after on a large codebase (100+ modules)
3. **Correctness:** Verify optimized code produces same output as unoptimized

---

## Challenges & Mitigations

### Challenge 1: StringId Mismatch Across Modules

**Problem:** Each module has its own StringInterner during parsing. When building ClassHierarchy, StringIds from different modules may conflict.

**Solution:**
- During type checking (sequential phase), modules already get integrated into a shared interner
- By the time we build WholeProgramAnalysis, all CheckedModules use Arc<StringInterner> pointing to consistent string IDs
- Verify this is working correctly in testing

### Challenge 2: Memory Usage

**Problem:** WholeProgramAnalysis is cloned for each parallel worker (Arc clones are cheap but still overhead).

**Mitigation:**
- Arc cloning is O(1) and only increments a reference count
- The actual data (ClassHierarchy, usage maps) is shared read-only
- Profile memory usage; if problematic, can use a single Arc passed by reference

### Challenge 3: Analysis Build Time

**Problem:** Building WholeProgramAnalysis adds sequential overhead before parallel codegen.

**Mitigation:**
- Only run for O3+ (skip for O0-O2)
- Analysis is much faster than full optimization (single scan vs fixed-point iteration)
- Expected overhead: < 5% of total compile time for large projects

---

## Future Enhancements

1. **Parallel analysis building:** If analysis becomes a bottleneck, some parts could run in parallel:
   - Class hierarchy building per module → merge
   - Global usage tracking per module → merge

2. **Incremental analysis:** Cache whole-program analysis results across compilations

3. **More analysis types:**
   - Pure function detection (no side effects)
   - Escape analysis (can a value escape the module?)
   - Type refinement across module boundaries

---

## Summary

**Phase 3.3 Architecture:**

```
┌─────────────────────────────────────────────────────────────┐
│ Phase 1: Sequential Type Checking                            │
│ • Parse results → Type check → CheckedModules               │
│ • Maintains dependency order                                 │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 2: Sequential Whole-Program Analysis (O3 only)        │
│ • Build ClassHierarchy across all modules                   │
│ • Analyze generic specialization opportunities              │
│ • Track global variable usage                               │
│ • Result: Arc<WholeProgramAnalysis> (thread-safe)           │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 3: Parallel Optimization + Codegen                    │
│ • Each module gets:                                          │
│   - Its own AST (mutable)                                    │
│   - Shared WholeProgramAnalysis (read-only via Arc)         │
│   - Optimizer runs fixed-point iteration per-module         │
│   - CodeGenerator emits Lua code                            │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Phase 4: Sequential Cache + Output Writing                  │
└─────────────────────────────────────────────────────────────┘
```

**Key Principles:**
- Sequential where dependencies exist (type checking, whole-program analysis)
- Parallel where independent (per-module optimization + codegen)
- Share analysis via Arc (cheap, thread-safe)
- Maintain correctness by building accurate cross-module views

Agent is calibrated...
