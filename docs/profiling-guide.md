# TypedLua Optimizer Profiling Guide

This guide explains how to profile and benchmark the TypedLua optimizer to measure performance and determine optimal parallelization strategies.

## Quick Start

### 1. Quick Profiling (Single Compilation)

To profile a single compilation with detailed pass-by-pass timing:

```bash
# Build the release binary first
cargo build --release -p typedlua-cli

# Profile with debug logging
RUST_LOG=typedlua_core::optimizer=debug \
  cargo run --release -p typedlua-cli -- compile <your-project-path> \
  --optimize \
  --no-cache \
  --no-emit
```

**Example output:**
```
[INFO] Optimization level: O3
[DEBUG] [Iter 1] ExpressionCompositePass: 4.25µs (changed: true)
[DEBUG] [Iter 1] EliminationCompositePass: 10.25µs (changed: false)
[DEBUG] [Iter 1] FunctionCompositePass: 4.958µs (changed: false)
[DEBUG] [Iter 1] DataStructureCompositePass: 666ns (changed: false)
[DEBUG] [Iter 1] loop-optimization: 334ns (changed: false)
[DEBUG] [Iter 1] rich-enum-optimization: 42ns (changed: false)
[DEBUG] [Iter 1] devirtualization: 667ns (changed: false)
[DEBUG] [Iter 1] generic-specialization: 250ns (changed: false)
[DEBUG] [Iter 1] global-localization: 4.25µs (changed: false)
[INFO] Optimization complete: 2 iterations, 100.458µs total
```

### 2. Statistical Benchmarking

For rigorous statistical benchmarking with multiple runs:

```bash
# Run all benchmark groups
cargo bench -p typedlua-cli

# Run specific benchmark group
cargo bench -p typedlua-cli module_parallelism
cargo bench -p typedlua-cli visitor_parallelism
cargo bench -p typedlua-cli optimization_levels
```

### 3. Compare Performance Changes

To measure the impact of code changes:

```bash
# Establish baseline before making changes
git checkout main
cargo bench -p typedlua-cli -- --save-baseline before

# Make your changes, then compare
git checkout your-feature-branch
cargo bench -p typedlua-cli -- --baseline before
```

## Benchmark Suites

### Module Parallelism (`module_parallelism`)

Tests how module-level parallelization scales with project size.

**What it measures:**
- Sequential vs parallel optimization with varying module counts (1, 2, 4, 8, 16)
- Small modules (~5 functions each) to isolate parallelization overhead

**Use case:** Determine threshold for enabling module-level parallelization
- How many modules needed before parallel optimization pays off?
- What's the speedup factor with N modules?

### Visitor Parallelism (`visitor_parallelism`)

Tests how statement-level parallelization scales within a single module.

**What it measures:**
- Optimization time for single modules with varying statement counts (10, 50, 100, 200)
- Each "statement" is a function with optimizable code

**Use case:** Determine threshold for enabling visitor-level parallelization
- How many statements needed before parallel AST traversal pays off?
- Is statement-level parallelization worth the complexity?

### Optimization Levels (`optimization_levels`)

Compares sequential vs parallel optimization at different levels.

**What it measures:**
- O3 optimization with 8 modules, 10 functions each
- Sequential vs parallel execution time

**Use case:** Verify parallel optimization provides benefit at production workloads

## Understanding Benchmark Output

### Criterion Output Format

```
module_parallelism/sequential_1
                        time:   [1.2345 ms 1.2567 ms 1.2789 ms]
                        change: [-5.2341% -3.1234% -1.0127%] (p = 0.02 < 0.05)
                        Performance has improved.

module_parallelism/parallel_8
                        time:   [456.78 µs 467.89 µs 478.90 µs]
                        change: [-45.234% -42.123% -39.012%] (p = 0.00 < 0.05)
                        Performance has improved.
```

**Reading the output:**
- **Time range:** [min median max] from multiple samples
- **Change:** Percentage change vs previous run (or baseline)
- **p-value:** Statistical significance (< 0.05 = significant)

### Interpreting Speedup

**Module parallelism example:**
- Sequential (8 modules): 1.25ms
- Parallel (8 modules): 467µs
- Speedup: 1.25ms / 467µs ≈ 2.67×

**Expected speedup:**
- 2 modules: ~1.5-1.8×
- 4 modules: ~2.0-2.5×
- 8 modules: ~2.5-3.5×
- 16 modules: ~3.0-4.0× (depends on CPU cores)

**Note:** Speedup is limited by:
- Number of CPU cores (Rayon defaults to logical cores)
- Amdahl's Law (sequential portions like type checking)
- Synchronization overhead (collecting results, barriers)

## HTML Reports

Criterion generates detailed HTML reports with graphs:

```bash
# After running benchmarks
open target/criterion/report/index.html
```

**Report features:**
- Line charts showing performance over time
- Violin plots showing distribution
- Comparison tables
- Statistical analysis

## CLI Flags for Profiling

TypedLua CLI provides flags specifically for profiling and benchmarking:

### `--optimize` / `--no-optimize`

Enable/disable optimization (default: enabled at O1 level).

```bash
# Enable O3 optimizations
cargo run --release -p typedlua-cli -- compile project/ --optimize

# Disable all optimizations
cargo run --release -p typedlua-cli -- compile project/ --no-optimize
```

### `--no-parallel-optimization`

Disable parallel optimization for benchmarking sequential performance.

```bash
# Sequential optimization (for baseline comparison)
cargo run --release -p typedlua-cli -- compile project/ \
  --optimize \
  --no-parallel-optimization
```

### `--no-cache`

Disable compilation cache to force fresh compilation.

```bash
# Always useful for profiling to avoid cache hits
cargo run --release -p typedlua-cli -- compile project/ \
  --optimize \
  --no-cache
```

### `--no-emit`

Skip writing output files (speeds up profiling by isolating compilation phases).

```bash
# Profile just parsing + type checking + optimization
cargo run --release -p typedlua-cli -- compile project/ \
  --optimize \
  --no-cache \
  --no-emit
```

## Profiling Workflow

### Step 1: Establish Baseline

```bash
# Profile current implementation
RUST_LOG=typedlua_core::optimizer=debug \
  cargo run --release -p typedlua-cli -- compile test-project/ \
  --optimize --no-cache --no-emit \
  2>&1 | tee baseline-profile.log

# Run statistical benchmarks
cargo bench -p typedlua-cli -- --save-baseline baseline
```

### Step 2: Identify Bottlenecks

```bash
# Examine per-pass timing
grep "Iter 1" baseline-profile.log

# Look for:
# - Which passes take longest?
# - How many iterations needed?
# - Which passes trigger re-runs (changed: true)?
```

### Step 3: Make Changes

Implement optimizations or parallelization based on bottlenecks.

### Step 4: Measure Impact

```bash
# Profile after changes
RUST_LOG=typedlua_core::optimizer=debug \
  cargo run --release -p typedlua-cli -- compile test-project/ \
  --optimize --no-cache --no-emit \
  2>&1 | tee optimized-profile.log

# Compare benchmarks
cargo bench -p typedlua-cli -- --baseline baseline

# Look for:
# - Reduced iteration count?
# - Faster individual passes?
# - Better overall speedup?
```

### Step 5: Verify Correctness

```bash
# Run full test suite
cargo test

# Compile real projects and verify output matches
cargo run --release -p typedlua-cli -- compile project/ -o output/
diff output/ expected-output/
```

## Profiling Best Practices

### 1. Build in Release Mode

Always profile with `--release` builds:

```bash
cargo build --release -p typedlua-cli
cargo bench -p typedlua-cli  # automatically uses release
```

Debug builds are 10-100× slower and have different performance characteristics.

### 2. Warm Up the System

Run a few iterations before measuring:

```bash
# Warm up caches and JIT
for i in {1..3}; do
  cargo run --release -p typedlua-cli -- compile project/ --no-emit
done

# Now measure
RUST_LOG=debug cargo run --release -p typedlua-cli -- compile project/ --no-emit
```

Criterion handles this automatically with warmup iterations.

### 3. Minimize System Noise

For consistent results:

```bash
# Close unnecessary applications
# Disable background tasks (Spotlight indexing, Time Machine, etc.)
# Plug in laptop (don't run on battery)

# Consider using `nice` for priority
nice -n -20 cargo bench -p typedlua-cli
```

### 4. Use Representative Test Cases

Profile with realistic code:

```bash
# Small project (1-5 modules)
# Medium project (10-50 modules)
# Large project (100+ modules)

# Measure all three to see scaling behavior
```

### 5. Isolate Optimization Phase

Use flags to measure just optimization, not I/O:

```bash
# Measure everything
cargo run --release -p typedlua-cli -- compile project/

# Measure just compilation (no output writing)
cargo run --release -p typedlua-cli -- compile project/ --no-emit

# Measure just optimization (skip cache, output)
cargo run --release -p typedlua-cli -- compile project/ --no-cache --no-emit
```

## Troubleshooting

### Benchmarks fail with "No such file or directory"

The benchmark generates temporary test projects. If you see errors:

```bash
# Clean and rebuild
cargo clean -p typedlua-cli
cargo bench -p typedlua-cli
```

### No profiling output

Ensure `RUST_LOG` is set correctly:

```bash
# Too verbose (shows everything)
RUST_LOG=debug cargo run ...

# Just optimizer (recommended)
RUST_LOG=typedlua_core::optimizer=debug cargo run ...

# Multiple modules
RUST_LOG=typedlua_core::optimizer=debug,typedlua_cli=info cargo run ...
```

### Benchmarks take too long

Reduce sample size for faster iteration:

Edit `benches/parallel_optimization.rs`:

```rust
let mut group = c.benchmark_group("module_parallelism");
group.sample_size(10);  // Default is 100
```

Or run specific benchmarks:

```bash
# Just one benchmark function
cargo bench -p typedlua-cli benchmark_module_count
```

### Results show high variance

Increase sample size or measurement time:

```rust
group.sample_size(100);  // More samples
group.measurement_time(Duration::from_secs(10));  // Longer measurement
```

## Next Steps

After profiling, use the data to:

1. **Determine Thresholds:**
   - At what module count does parallelization pay off?
   - What statement count justifies visitor-level parallelism?

2. **Identify Bottlenecks:**
   - Which passes are slowest?
   - Can they be optimized or parallelized?

3. **Validate Approaches:**
   - Does module-level parallelization help?
   - Is visitor-level parallelization worth the complexity?

4. **Make Informed Decisions:**
   - Should we implement parallel optimization?
   - Which approach (module-level, pass-level, visitor-level)?

See the [Parallel Optimization Plan](../.claude/plans/wobbly-tinkering-rabin.md) for detailed analysis of parallelization approaches.
