# TypedLua Development Guide

Quick reference for developing TypedLua and its VS Code extension.

## Quick Start Scripts

### Full Rebuild (LSP + Extension)
Use this when you change Rust code (lexer, parser, LSP server):

```bash
scripts/rebuild-and-install-extension.sh
```

This will:
1. Build LSP server (`cargo build --release`)
2. Compile extension TypeScript
3. Package as VSIX
4. Install in VS Code

**Time:** ~30-60 seconds (depending on Rust changes)

### Extension Only Reload
Use this when you only change extension TypeScript code:

```bash
scripts/reload-extension.sh
```

This will:
1. Compile extension TypeScript
2. Package as VSIX
3. Install in VS Code

**Time:** ~5-10 seconds

### After Running Scripts
Always reload VS Code after installation:
- Press `Ctrl+Shift+P` (or `Cmd+Shift+P` on Mac)
- Type "Reload Window" and press Enter

## Development Workflow

### Working on LSP Server (Rust)

1. Make changes to Rust code in `crates/typedlua-lsp/`
2. Run full rebuild:
   ```bash
   scripts/rebuild-and-install-extension.sh
   ```
3. Reload VS Code window
4. Test with `.tl` files

### Working on VS Code Extension (TypeScript)

1. Make changes to TypeScript code in `editors/vscode/src/`
2. Run quick reload:
   ```bash
   scripts/reload-extension.sh
   ```
3. Reload VS Code window
4. Test with `.tl` files

### Working on Language Features (Parser/Lexer)

1. Make changes in `crates/typedlua-core/`
2. Run full rebuild:
   ```bash
   scripts/rebuild-and-install-extension.sh
   ```
3. Reload VS Code window
4. Test with `.tl` files

## Testing

### Quick Test
```bash
code editors/vscode/test-files/test-basic.tl
```

### Check LSP Server Logs
1. Open VS Code
2. View > Output
3. Select "TypedLua Language Server" from dropdown
4. Enable verbose logging:
   - Settings > search "typedlua trace"
   - Set to "verbose"

### Run All Tests
```bash
# Core tests
cargo test

# LSP tests
cargo test --package typedlua-lsp

# Extension tests (TODO)
cd editors/vscode
npm test
```

## Common Tasks

### Build Just the LSP Server
```bash
cargo build --release --package typedlua-lsp
# Binary at: target/release/typedlua-lsp
```

### Check Rust Code
```bash
cargo check
cargo clippy
```

### Compile Extension Only
```bash
cd editors/vscode
npm run compile
```

### Package Extension Without Installing
```bash
cd editors/vscode
npm run package
# Creates: typedlua-0.1.0.vsix
```

### Manually Install Extension
```bash
code --install-extension editors/vscode/typedlua-0.1.0.vsix
```

### Uninstall Extension
```bash
code --uninstall-extension typedlua.typedlua
```

## Debugging

### Debug Extension in VS Code
1. Open `editors/vscode/` in VS Code
2. Press F5
3. New "[Extension Development Host]" window opens
4. Create/open `.tl` file in that window
5. Set breakpoints in extension TypeScript code

### Debug LSP Server
Enable verbose logging in VS Code settings:
```json
{
  "typedlua.trace.server": "verbose"
}
```

Check Output panel for detailed LSP communication.

### Extension Not Activating
- Check file extension is `.tl`
- Check bottom-right corner shows "TypedLua" (not "Plain Text")
- Check Output panel for errors
- Try: Developer > Reload Window

### LSP Server Not Starting
- Check server is built: `ls -la target/release/typedlua-lsp`
- Check server path in settings
- Try absolute path:
  ```json
  {
    "typedlua.server.path": "/absolute/path/to/target/release/typedlua-lsp"
  }
  ```

### Extension Compiles But Features Don't Work
- LSP server might not be built
- Run full rebuild: `scripts/rebuild-and-install-extension.sh`
- Check Output panel for LSP errors

## File Structure

```
typed-lua/
├── crates/
│   ├── typedlua-cli/        # CLI tool
│   ├── typedlua-core/       # Lexer, parser, AST
│   └── typedlua-lsp/        # Language server
├── editors/
│   └── vscode/              # VS Code extension
│       ├── src/
│       │   └── extension.ts # Extension entry point
│       ├── syntaxes/        # TextMate grammar
│       ├── test-files/      # Sample .tl files
│       └── package.json     # Extension manifest
└── scripts/
    ├── rebuild-and-install-extension.sh  # Full rebuild
    ├── build-extension.sh                # Build without install
    └── reload-extension.sh               # Quick reload
```

## Common Errors

### "Cannot find module 'vscode-languageclient'"
```bash
cd editors/vscode
npm install
```

### "typedlua-lsp: command not found"
```bash
cargo build --release --package typedlua-lsp
# Then set path in VS Code settings
```

### "Extension 'typedlua' is not installed"
```bash
scripts/reload-extension.sh
# Then reload VS Code window
```

## Performance Tips

- Use `scripts/reload-extension.sh` for extension-only changes (much faster)
- Use `cargo check` instead of `cargo build` when just checking for errors
- Use `cargo build` (debug) instead of `cargo build --release` during development
- Only use `--release` for final testing/packaging

## Resources

- [TESTING.md](editors/vscode/TESTING.md) - Comprehensive testing guide
- [QUICKSTART.md](editors/vscode/QUICKSTART.md) - 5-minute setup guide
- [LSP-Design.md](docs/LSP-Design.md) - LSP architecture documentation
- [Grammar.md](docs/Grammar.md) - Language grammar specification

---

**Last Updated:** 2026-01-03
