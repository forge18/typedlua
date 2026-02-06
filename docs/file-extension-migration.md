Claude’s Plan
File Extension Migration: .tl → .luax
Status: Planning

Reason: Renaming project from TypedLua to LuaNext. The .luax extension (Lua + neXt) provides clear semantic meaning and follows language naming conventions.

Migration Overview
Scope: Replace all .tl file extension references with .luax throughout the codebase.

Impact Areas:

CLI file handling & glob patterns (3 files)
VSCode extension configuration (2 files)
Test fixtures & stdlib files (12 actual .tl files to rename)
Test code with hardcoded paths (20+ files)
Documentation (13+ files)
Cache/manifest logic (3 files)
Source maps (2 files)
LSP symbol indexing (1 file)
Total: 45+ files to modify, 12 files to rename

Critical Files (Priority Order)
Phase 1: Core Runtime Logic
1. CLI File Extension Validation

File: crates/typedlua-cli/src/main.rs
Lines: 422, 428, 445
Change: e == "tl" → e == "luax"
Lines: 239, 240, 243
Change: main.tl → main.luax (init command)
2. VSCode Extension Configuration

File: editors/vscode/package.json
Line: 35
Change: "extensions": [".tl"] → "extensions": [".luax"]
3. VSCode File Watcher

File: editors/vscode/src/extension.ts
Line: 61
Change: **/*.tl → **/*.luax
Phase 2: Test Infrastructure
4. Cache & Manifest Test Fixtures

crates/typedlua-core/src/cache/manifest.rs (lines 243, 247, 279, 286)
crates/typedlua-core/src/cache/module.rs (line 78)
crates/typedlua-core/src/cache/invalidation.rs (lines 79, 81, 101-137)
Change: All /test/*.tl paths → /test/*.luax
5. Integration & CLI Tests

crates/typedlua-cli/tests/integration_tests.rs (14 occurrences)
crates/typedlua-cli/tests/cli_features_tests.rs (18 occurrences)
crates/typedlua-cli/tests/watch_mode_tests.rs (11 occurrences)
crates/typedlua-cli/tests/cli_edge_cases_tests.rs (22 occurrences)
crates/typedlua-cli/tests/lua_file_copy_tests.rs (2 occurrences + .d.tl)
Change: temp_dir.path().join("test.tl") → temp_dir.path().join("test.luax")
Special: .d.tl → .d.luax (declaration files)
6. Source Map Tests

crates/typedlua-core/src/codegen/sourcemap.rs (lines 306, 317, 335, 358, 373, 380, 444)
crates/typedlua-core/src/codegen/builder.rs (lines 15, 82, 115, 138)
Change: input.tl → input.luax
7. LSP Symbol Index Tests

crates/typedlua-lsp/src/symbol_index.rs (lines 712-713)
Change: /test/module.tl → /test/module.luax
8. Benchmark Tests

crates/typedlua-cli/benches/parallel_optimization.rs
crates/typedlua-core/tests/performance_benchmarks.rs
crates/typedlua-parser/benches/full.rs
Change: All .tl test file references
Phase 3: Actual File Renames
9. Rename Actual .tl Files

VSCode test files:

editors/vscode/test-files/test-basic.tl → test-basic.luax
editors/vscode/test-files/test-types.tl → test-types.luax
editors/vscode/test-files/test-errors.tl → test-errors.luax
editors/vscode/test-files/test-features.tl → test-features.luax
Stdlib declaration files:

crates/typedlua-core/src/stdlib/lua54.d.tl → lua54.d.luax
crates/typedlua-parser/test_data/sample.tl → sample.luax
crates/typedlua-typechecker/src/stdlib/builtins.d.tl → builtins.d.luax
crates/typedlua-typechecker/src/stdlib/lua51.d.tl → lua51.d.luax
crates/typedlua-typechecker/src/stdlib/lua52.d.tl → lua52.d.luax
crates/typedlua-typechecker/src/stdlib/lua53.d.tl → lua53.d.luax
crates/typedlua-typechecker/src/stdlib/lua54.d.tl → lua54.d.luax
crates/typedlua-typechecker/src/stdlib/reflection.d.tl → reflection.d.luax
Note: Declaration files use .d.luax pattern (similar to TypeScript's .d.ts)

Phase 4: Documentation Updates
10. Documentation Files

Main docs:

README.md - Update glob pattern examples (src/**/*.tl → src/**/*.luax)
CONTRIBUTING.md - Update test file structure examples
docs/README.md
docs/ARCHITECTURE.md
docs/REFLECTION.md
Design docs (in docs/designs/):

TypedLua-Design.md
CLI-Design.md
LSP-Design.md (line 665 - file extension config)
Additional-Features-Design.md
Implementation-Plan.md
Implementation-Architecture.md
old_TODO.md
VSCode extension docs:

editors/vscode/README.md
editors/vscode/TESTING.md
editors/vscode/QUICKSTART.md (test-basic.tl example)
editors/vscode/CHANGELOG.md
Script docs:

scripts/README.md
Phase 5: Module Resolution Logic
11. TypeChecker Module Resolver

crates/typedlua-typechecker/src/module_resolver/mod.rs
crates/typedlua-typechecker/src/module_resolver/error.rs
crates/typedlua-typechecker/src/stdlib/mod.rs
Change: Any .tl extension logic in module path resolution
Phase 6: Build Scripts
12. Build/Deployment Scripts

scripts/rebuild-and-install-extension.sh (line 57)
scripts/build-extension.sh
scripts/reload-extension.sh
Implementation Strategy
Order of Operations:

Phase 1 first - Core runtime logic (CLI + VSCode) ensures new files work immediately
Phase 2 next - Test infrastructure updates (no actual file changes yet)
Phase 3 - Rename actual .tl files (requires Git mv for history preservation)
Phase 4 - Documentation (low risk, can be done anytime)
Phase 5 - Module resolver (likely minimal changes)
Phase 6 - Scripts (verify after core changes)
Git Strategy:

Use git mv for file renames to preserve history
Single commit for core logic changes (Phases 1-2)
Single commit for file renames (Phase 3)
Single commit for documentation (Phase 4)
Phases 5-6 as needed
Verification Plan
After Phase 1-2 (Core + Tests):


# Verify CLI accepts .luax files
cargo build --release
./target/release/typedlua init test-project
cd test-project
# Verify src/main.luax was created
ls -la src/

# Verify file extension validation
echo "print('hello')" > test.luax
../target/release/typedlua check test.luax  # Should work
../target/release/typedlua check test.tl   # Should fail
After Phase 3 (File Renames):


# Verify all tests still pass
cargo test --workspace

# Verify VSCode extension recognizes .luax
cd editors/vscode
code test-files/test-basic.luax  # Should have syntax highlighting
After Phase 4-6 (Docs + Scripts):


# Verify documentation examples work
# Follow README.md examples with .luax files

# Verify build scripts work
./scripts/rebuild-and-install-extension.sh
Full Integration Test:


# Create a new project from scratch
cargo run --bin typedlua -- init my-luanext-project
cd my-luanext-project

# Write some code to src/main.luax
echo 'function greet(name: string): string
    return "Hello, " .. name
end

print(greet("LuaNext"))' > src/main.luax

# Compile and run
cargo run --bin typedlua -- build
lua output/main.lua  # Should print "Hello, LuaNext"
Edge Cases to Handle
Declaration files: .d.tl → .d.luax (similar to TypeScript's .d.ts)
Backup test files: integration_tests.rs.bak2, .bak3 - likely can be deleted
Module resolver: May have hardcoded .tl extension logic for file discovery
Error messages: Search for any error messages that mention ".tl" specifically
File type detection: Ensure MIME types/language IDs are updated if applicable
Potential Risks
External dependencies: If typedlua-parser is external Git repo, may need updates there first
Cached compilation artifacts: Users may have old .tl files in cache - will be incompatible
VSCode extension users: Need clear migration guide in changelog
Breaking change: This is a breaking change requiring major version bump
Note: No backward compatibility - clean break from .tl to .luax

Post-Migration Tasks
Update CHANGELOG.md with migration instructions (breaking change)
Update GitHub repository description and topics
Update package.json version (major bump required)
Write migration guide for existing users (manual file rename required)
Update any CI/CD pipelines that may reference .tl files
Clear any cached .tl compilation artifacts in development