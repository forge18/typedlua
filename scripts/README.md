# Development Scripts

Quick reference for TypedLua development scripts. All scripts should be run from the project root.

## Scripts Overview

| Script | What it does | When to use |
|--------|--------------|-------------|
| `scripts/rebuild-and-install-extension.sh` | Build everything + install | After Rust changes |
| `scripts/build-extension.sh` | Build everything, no install | For distribution |
| `scripts/reload-extension.sh` | Quick extension rebuild + install | After TypeScript changes |

## Detailed Guide

### `scripts/rebuild-and-install-extension.sh`
**Full rebuild with automatic installation**

```bash
scripts/rebuild-and-install-extension.sh
```

**Does:**
1. ✅ Builds LSP server (Rust)
2. ✅ Compiles extension (TypeScript)
3. ✅ Creates VSIX in `editors/vscode/`
4. ✅ Installs extension in VS Code
5. ℹ️ You must reload VS Code window after

**Use when:**
- You changed any Rust code (lexer, parser, LSP)
- You want to test everything together
- First time setup

**Time:** 30-60 seconds

---

### `scripts/build-extension.sh`
**Build package without installing**

```bash
scripts/build-extension.sh
```

**Does:**
1. ✅ Builds LSP server (Rust)
2. ✅ Compiles extension (TypeScript)
3. ✅ Creates VSIX in `editors/vscode/`
4. ❌ Does NOT install

**Use when:**
- You want to create a package for distribution
- You want to manually install later
- You're testing the build process

**Output:**
- `editors/vscode/dist/typedlua-0.1.0.vsix`

**Manual install:**
```bash
code --install-extension editors/vscode/dist/typedlua-0.1.0.vsix
```

**Time:** 30-60 seconds

---

### `scripts/reload-extension.sh`
**Quick extension-only rebuild**

```bash
scripts/reload-extension.sh
```

**Does:**
1. ❌ Skips LSP server build
2. ✅ Compiles extension (TypeScript)
3. ✅ Creates VSIX in `editors/vscode/`
4. ✅ Installs extension in VS Code
5. ℹ️ You must reload VS Code window after

**Use when:**
- You ONLY changed TypeScript code in `editors/vscode/src/`
- You want fast iteration
- LSP server is already built

**Time:** 5-10 seconds

---

## After Running Any Install Script

**Always reload VS Code:**
1. Press `Ctrl+Shift+P` (or `Cmd+Shift+P` on Mac)
2. Type "Reload Window"
3. Press Enter

OR just close and reopen VS Code.

## Testing

After installing, test with:

```bash
code editors/vscode/test-files/test-basic.tl
```

Check logs:
1. View > Output
2. Select "TypedLua Language Server" from dropdown

## Workflow Examples

### Scenario 1: Working on Parser

```bash
# 1. Edit code in crates/typedlua-core/src/parser/
vim crates/typedlua-core/src/parser/expression.rs

# 2. Rebuild everything
scripts/rebuild-and-install-extension.sh

# 3. Reload VS Code
# Ctrl+Shift+P > "Reload Window"

# 4. Test
code editors/vscode/test-files/test-basic.tl
```

### Scenario 2: Working on Extension UI

```bash
# 1. Edit extension code
vim editors/vscode/src/extension.ts

# 2. Quick reload
scripts/reload-extension.sh

# 3. Reload VS Code
# Ctrl+Shift+P > "Reload Window"

# 4. Test
# Extension changes take effect immediately
```

### Scenario 3: Preparing Release

```bash
# 1. Build package
scripts/build-extension.sh

# 2. Test the VSIX
code --install-extension editors/vscode/dist/typedlua-0.1.0.vsix

# 3. Distribute
# Upload editors/vscode/dist/typedlua-0.1.0.vsix to GitHub releases
# Or publish to VS Code Marketplace
```

## File Locations

```
typed-lua/
├── scripts/
│   ├── rebuild-and-install-extension.sh   (full rebuild + install)
│   ├── build-extension.sh                 (build without install)
│   └── reload-extension.sh                (quick extension reload)
│
├── target/
│   └── release/
│       └── typedlua-lsp                   (LSP server binary)
│
└── editors/
    └── vscode/
        ├── dist/
        │   └── typedlua-0.1.0.vsix        (generated VSIX package)
        ├── out/
        │   └── extension.js               (compiled TypeScript)
        └── src/
            └── extension.ts               (source TypeScript)
```

## Troubleshooting

### "Permission denied"
```bash
chmod +x scripts/*.sh
```

### "npm: command not found"
```bash
cd editors/vscode
npm install
```

### "cargo: command not found"
Install Rust toolchain: https://rustup.rs/

### Extension not working after install
1. Check you reloaded VS Code window
2. Check file extension is `.tl`
3. Check Output panel for errors
4. Try full rebuild: `scripts/rebuild-and-install-extension.sh`

---

**See also:** [DEVELOPMENT.md](../DEVELOPMENT.md) for detailed development guide
