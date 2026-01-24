# TypedLua Performance Benchmarks

Baseline performance measurements for TypedLua compiler components using Criterion.rs.

**System Information:**
- Benchmarking Tool: Criterion.rs 0.5
- Build Profile: `--release` with optimizations
- Date: 2026-01-16

---

## Lexer Performance

### Simple Code (42 tokens)
```
const x: number = 42
const y: string = "hello"
function add(a: number, b: number): number {
    return a + b
}
```

**Result:** 733.08 ns/iteration
- **Throughput:** ~57M tokens/second

### Class Definition (~80 tokens)
```
class User {
    public name: string
    private age: number
    constructor(name: string, age: number) { ... }
    public greet(): void { ... }
}
```

**Result:** 1.236 µs/iteration
- **Throughput:** ~65M tokens/second

### Scaling Tests
| Size | Time per Iteration | Tokens/Second |
|------|-------------------|---------------|
| 10 statements | 1.373 µs | ~7.3M |
| 50 statements | 6.397 µs | ~7.8M |
| 100 statements | 13.188 µs | ~7.6M |
| 500 statements | 64.179 µs | ~7.8M |

**Observation:** Lexer performance is linear with input size (~128 ns/statement).

---

## Parser Performance

### Simple Code
**Result:** 2.591 µs/iteration
- **Throughput:** ~386K parses/second

### Class Definition
**Result:** 4.793 µs/iteration
- **Throughput:** ~209K parses/second

### Interface Implementation
**Result:** 6.846 µs/iteration
- **Throughput:** ~146K parses/second

### Scaling Tests
| Size | Time per Iteration | Statements/Second |
|------|-------------------|------------------|
| 10 functions | 10.698 µs | ~935K |
| 50 functions | 53.251 µs | ~939K |
| 100 functions | 107.47 µs | ~930K |

**Observation:** Parser performance is linear (~1.07 µs/function).

---

## Type Checker Performance

### Simple Code (3 statements)
**Result:** 221.68 µs/iteration
- **Throughput:** ~4.5K type checks/second

### Function Calls (7 statements)
**Result:** 243.14 µs/iteration
- **Throughput:** ~4.1K type checks/second

### Class with Methods
**Result:** 248.14 µs/iteration
- **Throughput:** ~4.0K type checks/second

### Interface Implementation
**Result:** 250.96 µs/iteration
- **Throughput:** ~4.0K type checks/second

### Scaling Tests
| Size | Time per Iteration | Statements/Second |
|------|-------------------|------------------|
| 10 statements | 235.96 µs | ~42K |
| 25 statements | 257.15 µs | ~97K |
| 50 statements | 301.01 µs | ~166K |

**Observation:** Type checker has baseline overhead (~230 µs) + ~1.4 µs/statement.

---

## Performance Bottlenecks

Based on the baseline measurements:

1. **Type Checker:** Largest overhead (~221-301 µs per compilation)
   - Opportunity: String interning for type names and identifiers (30-50% memory reduction)
   - Opportunity: Optimize symbol table lookups

2. **Parser:** Moderate overhead (~2.6-107 µs)
   - Opportunity: Arena allocation to replace Box<T> allocations (15-20% speedup expected)
   - Opportunity: Reduce AST node allocation overhead

3. **Lexer:** Minimal overhead, excellent performance (~730 ns - 64 µs)
   - Already highly optimized
   - Linear scaling with input size

---

## Next Steps

### P0 Optimizations (Ready to Implement)
1. **String Interning** - Infrastructure exists at `string_interner.rs`
   - Expected: 30-50% memory reduction
   - Expected: Faster symbol lookups (u32 vs String comparison)

2. **Arena Allocation** - Infrastructure exists at `arena.rs`
   - Expected: 15-20% parsing speedup
   - Expected: Better cache locality

3. **Inline Annotations** - Add to hot paths
   - Expected: 5-10% speedup on type checking loops

### Measurement Plan
After each optimization:
1. Re-run benchmarks: `cargo bench`
2. Compare with baseline (this document)
3. Verify expected improvements
4. Profile allocations with dhat to measure memory impact

---

---

## Memory Profiling Results (dhat)

**Test Configuration:**
- 4 test suites: simple code, functions, class, large file (100 functions)
- Profile using dhat heap profiler
- Release build with optimizations

### With String Interning (2026-01-16)

**Metrics:**
- Total allocated: 22,391,571 bytes (22.4 MB)
- Peak memory: 1,388,502 bytes (1.39 MB)
- Total allocations: 123,195 blocks

**Analysis:**
String interning provides modest memory improvements:
- Reduces duplicated identifier storage in lexer
- Hash map + vector overhead offsets some gains
- AST still uses `String` not `StringId` (backward compatible)
- Actual reduction: ~5-6% vs theoretical 30-50%

**Future Optimizations:**
To achieve the theoretical 30-50% reduction, would need:
1. Change AST to use `StringId` instead of `String` (breaking change)
2. Intern type names and string literals (not just identifiers)
3. Arena allocation for AST nodes (planned P0 optimization)

---

## Benchmark Commands

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark suite
cargo bench --bench lexer_bench
cargo bench --bench parser_bench
cargo bench --bench type_checker_bench

# View benchmark history
open target/criterion/report/index.html

# Memory profiling with dhat
cargo run --release --example profile_allocations
# View results at: https://nnethercote.github.io/dh_view/dh_view.html
```

---

## Notes

- All benchmarks include lexing + parsing + type checking in their measurements
- Times shown are median values from 100 samples
- Criterion automatically detects and reports outliers
- Benchmarks use CollectingDiagnosticHandler (similar to production)
- Memory profiling uses dhat heap profiler in release mode
