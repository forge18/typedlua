# Contributing to TypedLua

Thank you for your interest in contributing to TypedLua!

## Code of Conduct

- Be respectful and constructive
- Focus on what is best for the project
- Show empathy towards other contributors
- Accept constructive criticism gracefully

---

## Getting Started

### Prerequisites

- Rust 1.70+ (`rustup update`)
- Cargo (comes with Rust)
- Node.js 16+ (for VS Code extension)
- Git

### Setup

1. Fork and clone:
   ```bash
   git clone https://github.com/yourusername/typed-lua.git
   cd typed-lua
   ```

2. Build the project:
   ```bash
   cargo build
   cargo test
   ```

3. For VS Code extension development:
   ```bash
   cd editors/vscode
   npm install
   ```

---

## Development Workflows

### Working on Core (Rust)

**Lexer, Parser, Type Checker, Code Generator:**

1. Make changes in `crates/typedlua-core/`
2. Run checks:
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   ```
3. Commit changes

### Working on LSP Server

1. Make changes in `crates/typedlua-lsp/`
2. Run checks (same as above)
3. For integration testing with VS Code:
   ```bash
   scripts/rebuild-and-install-extension.sh
   # Then reload VS Code window (Cmd/Ctrl+Shift+P > "Reload Window")
   ```

### Working on VS Code Extension

**Quick reload (TypeScript changes only):**
```bash
scripts/reload-extension.sh
# Then reload VS Code window
```

**Full rebuild (includes LSP server):**
```bash
scripts/rebuild-and-install-extension.sh
# Then reload VS Code window
```

### Creating a Feature Branch

```bash
git checkout -b feature/your-feature-name
# Make changes
git add .
git commit -m "Brief description"
git push origin feature/your-feature-name
# Create pull request
```

---

## Coding Standards

### Rust Conventions

**Required:**
- `cargo fmt` before committing (enforced by pre-commit hook)
- `cargo clippy -- -D warnings` (enforced by pre-commit hook)
- `Result<T, E>` over panicking
- Trait-based dependency injection for testability
- Doc comments for public APIs

**Forbidden:**
- `#[allow(clippy::...)]` / `#[allow(dead_code)]` (except `#[cfg(test)]` items)
- Fix issues, don't suppress them

### Testing

- Unit: `#[cfg(test)]` in same file
- Integration: `tests/` directory
- Target: 70%+ coverage (`cargo tarpaulin`)
- Use DI pattern (see [message_handler.rs](crates/typedlua-lsp/src/message_handler.rs))

### Code Philosophy

- Simplicity over cleverness - no premature abstraction
- No backward compatibility unless requested
- Never delete failing tests - fix code or update test
- Test behavior, not implementation
- Use realistic test data

---

## Testing & Debugging

### Run Tests

```bash
# All tests
cargo test

# Specific crate
cargo test --package typedlua-lsp

# With coverage
cargo tarpaulin --package typedlua-core --out Stdout

# Specific test
cargo test test_name
```

### Rust Debugging

```bash
# Debug logging
RUST_LOG=debug cargo run
RUST_LOG=typedlua_core=debug cargo run

# With backtrace
RUST_BACKTRACE=1 cargo test
```

### VS Code Extension Debugging

**Check LSP server logs:**
1. View > Output
2. Select "TypedLua Language Server"
3. Enable verbose: Settings > "typedlua trace" > "verbose"

**Debug extension:**
1. Open `editors/vscode/` in VS Code
2. Press F5
3. New "[Extension Development Host]" window opens
4. Set breakpoints in TypeScript code

**Common issues:**
- Extension not activating? Check file extension is `.tl`
- LSP not starting? Check `target/release/typedlua-lsp` exists
- Features not working? Run full rebuild script

---

## Project Structure

```
typed-lua/
├── crates/
│   ├── typedlua-cli/        # CLI tool
│   ├── typedlua-core/       # Lexer, parser, type checker, codegen
│   └── typedlua-lsp/        # Language server
├── editors/vscode/          # VS Code extension
│   ├── src/extension.ts     # Extension entry point
│   ├── syntaxes/            # TextMate grammar
│   └── test-files/          # Sample .tl files
├── scripts/
│   ├── rebuild-and-install-extension.sh  # Full rebuild
│   └── reload-extension.sh               # Quick reload
└── tests/                   # Integration tests
```

---

## Common Tasks

### Adding a Feature

1. Update parser (if syntax changes) - `crates/typedlua-core/src/parser/`
2. Update type checker (if types change) - `crates/typedlua-core/src/typechecker/`
3. Update codegen (if output changes) - `crates/typedlua-core/src/codegen/`
4. Add tests
5. Update docs

### Fixing a Bug

1. Write failing test
2. Fix bug
3. Verify test passes
4. Add regression test

### Adding LSP Feature

1. Create provider in `crates/typedlua-lsp/src/providers/`
2. Register in `message_handler.rs`
3. Add tests in `tests/message_handler_tests.rs`
4. Update capabilities in `main.rs`

### Manual Commands

```bash
# Build LSP server only
cargo build --release --package typedlua-lsp

# Check code
cargo check
cargo clippy

# Compile extension only
cd editors/vscode
npm run compile

# Package extension
npm run package  # Creates typedlua-0.1.0.vsix

# Install extension manually
code --install-extension editors/vscode/typedlua-0.1.0.vsix

# Uninstall
code --uninstall-extension typedlua.typedlua
```

---

## Pull Request Guidelines

### PR Title Format

Use conventional commits:
- `feat: Add cross-file rename`
- `fix: Resolve parser panic`
- `docs: Update README`
- `test: Add LSP tests`
- `refactor: Simplify type checker`
- `perf: Optimize module resolution`

### PR Description

Include:
1. **What**: Brief description
2. **Why**: Motivation/context
3. **How**: Implementation approach
4. **Testing**: How you tested
5. **Checklist**:
   - [ ] Tests pass
   - [ ] Clippy passes
   - [ ] Formatted
   - [ ] Docs updated

### Review Process

1. Automated checks must pass
2. Maintainer approval required
3. Address feedback
4. Squash commits (if requested)

---

## Documentation

- Update relevant READMEs
- Add doc comments for public APIs
- Update [TODO.md](TODO.md) for tasks
- Update [CHANGELOG.md](CHANGELOG.md) for changes

---

## Resources

- [TODO.md](TODO.md) - Available tasks
- [CHANGELOG.md](CHANGELOG.md) - Version history
- [README.md](README.md) - Project overview

---

## Recognition

Contributors recognized in:
- Git history
- Release notes
- Future CONTRIBUTORS.md

Thank you for contributing to TypedLua! 🚀
