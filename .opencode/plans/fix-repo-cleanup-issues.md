# Fix Plan: Repository Cleanup Issues

## Overview
This plan addresses 4 specific issues identified in the repository analysis:
1. Git dependency version mismatch in typedlua-lsp
2. Unused variables in test files  
3. Missing "compiler" feature in typedlua-lsp

## Issue 1: Git Dependency Version Mismatch (HIGH PRIORITY)

### Problem
`typedlua-lsp/Cargo.toml` uses git dependencies:
```toml
typedlua-parser = { git = "https://github.com/forge18/typed-lua.git" }
typedlua-typechecker = { git = "https://github.com/forge18/typedlua-typechecker.git" }
```

While other crates use local paths. The workspace has patches that should redirect these, but:
- typedlua-parser patch uses wrong URL pattern
- typedlua-typechecker has no patch configured
- This creates risk of version mismatches

### Solution
Option A: Change typedlua-lsp to use local paths (RECOMMENDED)
- Change dependencies to use `path = "..."` like other crates
- Ensures all crates use same code
- Eliminates network dependency for builds

Option B: Fix patch configuration
- Add proper patches for both dependencies in workspace Cargo.toml
- Keep git dependencies but redirect to local paths

**Decision needed:** Which approach to take?

## Issue 2: Unused Variables in Tests (LOW PRIORITY)

### Problem
Three unused variables/functions in test files:
1. `crates/typedlua-core/tests/generic_specialization_tests.rs:173`
   - `let has_specialized = ...` (unused)
2. `crates/typedlua-core/tests/optimization_effectiveness_tests.rs:67`
   - `let o0 = ...` (unused, though other `o0` vars are used later)
3. `crates/typedlua-core/tests/generic_specialization_tests.rs:74`
   - `fn string_type(...)` (unused)

### Solution
Prefix unused items with underscore:
- `let _has_specialized = ...`
- `let _o0 = ...` (first occurrence only)
- `fn _string_type(...)` OR remove entirely

## Issue 3: Missing "compiler" Feature (MEDIUM PRIORITY)

### Problem
Three test files use `#![cfg(feature = "compiler")]`:
- `tests/lsp_tests.rs`
- `tests/message_handler_tests.rs`
- `tests/lsp_integration_tests.rs`

But the feature doesn't exist in `typedlua-lsp/Cargo.toml`, causing warnings.

### Solution
Add to `typedlua-lsp/Cargo.toml`:
```toml
[features]
compiler = []  # Add this line
```

Or remove the `#![cfg(...)]` attributes if not needed.

## Issue 4: Duplicate Dependencies (INFO ONLY)

### Problem
Multiple versions of dependencies exist:
- bitflags: v1.3.2 and v2.10.0
- rustc-hash: v1.1.0 and v2.1.1
- These are transitive dependencies, not directly controlled

### Solution
**No action needed** - These are brought in by external crates (lsp-types, dhat, notify, etc.) and cannot be easily deduplicated without forking those crates.

## Implementation Steps

### Step 1: Fix Git Dependencies (User Decision Required)
**Option A - Local Paths:**
```toml
# In typedlua-lsp/Cargo.toml
typedlua-parser = { path = "../typedlua-parser" }
typedlua-typechecker = { path = "../typedlua-typechecker" }  # If exists locally
```

**Option B - Fix Patches:**
```toml
# In workspace Cargo.toml
[patch."https://github.com/forge18/typedlua-typechecker.git"]
typedlua-typechecker = { path = "crates/typedlua-typechecker" }
```

### Step 2: Fix Unused Variables
Edit 3 test files to prefix unused items with underscore.

### Step 3: Add Missing Feature
Add `compiler = []` feature to typedlua-lsp/Cargo.toml

### Step 4: Verify
Run:
```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace
```

## Files to Modify

1. `crates/typedlua-lsp/Cargo.toml` - Fix dependencies, add feature
2. `crates/typedlua-core/tests/generic_specialization_tests.rs` - Fix unused items
3. `crates/typedlua-core/tests/optimization_effectiveness_tests.rs` - Fix unused variable
4. `Cargo.toml` (workspace) - Optional: fix patches

## Questions for User

1. **Git Dependencies:** Should typedlua-lsp use local paths or git dependencies with patches?
   - Local paths: More reliable, no network needed
   - Git dependencies: Allows independent versioning but needs patches

2. **"compiler" Feature:** Should we add the feature or remove the cfg attributes?
   - Add feature: Keeps conditional compilation ability
   - Remove cfg: Simpler, tests always run

3. **Priority:** Should I proceed with all fixes or focus on specific ones?
