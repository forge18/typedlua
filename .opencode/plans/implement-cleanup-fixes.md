# Implementation Plan: Repository Cleanup Fixes

## Summary
Fix 3 categories of issues in the TypedLua repository:
1. Remove unnecessary cfg attributes from LSP tests
2. Fix unused variable warnings in core tests
3. Verify git dependency configuration

## Changes Overview

### Change 1: Remove cfg attributes from LSP tests
**Rationale:** The `#![cfg(feature = "compiler")]` attributes reference a non-existent feature, causing compiler warnings. Since the user confirmed to remove them, these tests will always run (which is fine).

**Files to modify (3):**

#### File 1: `crates/typedlua-lsp/tests/lsp_tests.rs`
**Current:**
```rust
#![cfg(feature = "compiler")]
```
**Change:** Delete line 1 entirely

#### File 2: `crates/typedlua-lsp/tests/message_handler_tests.rs`
**Current:**
```rust
#![cfg(feature = "compiler")]
```
**Change:** Delete line 1 entirely

#### File 3: `crates/typedlua-lsp/tests/lsp_integration_tests.rs`
**Current:**
```rust
#![allow(dead_code)]
#![cfg(feature = "compiler")]
```
**Change:** Delete line 4 (`#![cfg(feature = "compiler")]`), keep line 3

---

### Change 2: Fix unused variables in core tests
**Rationale:** Three unused items causing compiler warnings. Prefix with underscore to indicate intentional non-use.

**Files to modify (2):**

#### File 4: `crates/typedlua-core/tests/generic_specialization_tests.rs`

**Location 1 - Line 74:**
**Current:**
```rust
fn string_type(span: Span) -> Type {
```
**Change:**
```rust
fn _string_type(span: Span) -> Type {
```

**Location 2 - Line 173:**
**Current:**
```rust
    let has_specialized = program.statements.iter().any(|s| {
```
**Change:**
```rust
    let _has_specialized = program.statements.iter().any(|s| {
```

#### File 5: `crates/typedlua-core/tests/optimization_effectiveness_tests.rs`

**Location - Line 67:**
**Current:**
```rust
    let o0 = compile(src, OptimizationLevel::O0).unwrap();
```
**Change:**
```rust
    let _o0 = compile(src, OptimizationLevel::O0).unwrap();
```

**Note:** Other `o0` variables later in the file (lines 77, 88, 102, 110, 118, 126, 137) are actually used, so only line 67 needs the underscore prefix.

---

### Change 3: Verify git dependency configuration (NO CHANGES NEEDED)
**Rationale:** User confirmed to keep typedlua-typechecker as git-only dependency. Current configuration is correct.

**Current state:**
- `typedlua-lsp/Cargo.toml` uses git dependency for typedlua-typechecker
- No local crate exists in `crates/typedlua-typechecker`
- This is the intended configuration

**Action:** No changes required. The git dependency will continue to work as-is.

---

## Verification Steps

After all changes, run:

```bash
# Check for compilation errors
cargo check --workspace

# Run all tests
cargo test --workspace

# Check for warnings
cargo clippy --workspace

# Verify formatting
cargo fmt --all -- --check
```

## Expected Results

- ✅ All workspace crates compile without errors
- ✅ All 700+ tests pass
- ✅ No compiler warnings about unused variables
- ✅ No warnings about non-existent "compiler" feature
- ✅ Code formatting passes

## Risk Assessment

**Low Risk:**
- Changes are minimal and focused on test files
- No production code modifications
- Only affects warnings, not functionality
- All changes are reversible

## Rollback Plan

If issues arise, revert changes with git:
```bash
git checkout -- crates/typedlua-lsp/tests/lsp_tests.rs
git checkout -- crates/typedlua-lsp/tests/message_handler_tests.rs
git checkout -- crates/typedlua-lsp/tests/lsp_integration_tests.rs
git checkout -- crates/typedlua-core/tests/generic_specialization_tests.rs
git checkout -- crates/typedlua-core/tests/optimization_effectiveness_tests.rs
```

## Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/typedlua-lsp/tests/lsp_tests.rs` | 1 deleted | cfg removal |
| `crates/typedlua-lsp/tests/message_handler_tests.rs` | 1 deleted | cfg removal |
| `crates/typedlua-lsp/tests/lsp_integration_tests.rs` | 1 deleted | cfg removal |
| `crates/typedlua-core/tests/generic_specialization_tests.rs` | 2 modified | underscore prefix |
| `crates/typedlua-core/tests/optimization_effectiveness_tests.rs` | 1 modified | underscore prefix |

**Total:** 5 files, 6 line changes

---

## Pre-Implementation Checklist

- [ ] User has reviewed and approved this plan
- [ ] No other pending changes that could conflict
- [ ] Ready to execute

## Post-Implementation Checklist

- [ ] All 5 files modified as specified
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes (700+ tests)
- [ ] `cargo clippy --workspace` shows no new warnings
- [ ] `cargo fmt --all -- --check` passes
- [ ] Changes committed (if requested)
