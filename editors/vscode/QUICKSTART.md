# Quick Start: Testing TypedLua Extension

Get the TypedLua VS Code extension running in under 5 minutes.

## Prerequisites

- VS Code 1.75.0 or higher
- Node.js and npm installed
- Rust toolchain (for building LSP server)

## Step 1: Build the LSP Server

```bash
cd /path/to/typed-lua
cargo build --release --package typedlua-lsp
```

The binary will be at: `target/release/typedlua-lsp`

## Step 2: Set up the Extension

```bash
cd editors/vscode
npm install
npm run compile
```

## Step 3: Run the Extension

### Option A: Debug in VS Code (Recommended)

1. Open the extension folder in VS Code:
   ```bash
   code editors/vscode
   ```

2. Press `F5` to start debugging

3. A new "[Extension Development Host]" window opens

4. In the new window, open the test files:
   ```
   File > Open Folder > editors/vscode/test-files
   ```

5. Open `test-basic.tl` - the extension should activate!

### Option B: Install as VSIX

1. Package the extension:
   ```bash
   cd editors/vscode
   npm run package
   ```

2. Install it:
   ```bash
   code --install-extension typedlua-0.1.0.vsix
   ```

3. Open any `.tl` file to activate the extension

## Step 4: Verify It's Working

### Check Extension Activation

1. Open `test-basic.tl`
2. Open Output panel: View > Output
3. Select "TypedLua Language Server" from dropdown
4. You should see initialization messages

### Test Basic Features

**Syntax Highlighting:**
- Keywords like `function`, `local`, `const` should be colored
- Strings and comments should be colored

**Auto-Closing:**
- Type `{` → should auto-close with `}`
- Type `"` → should auto-close with `"`

**Indentation:**
- Press Enter after `function foo()` → should auto-indent
- Type `end` → should auto-outdent

**Commands:**
- Press `Ctrl+Shift+P` (Cmd+Shift+P on Mac)
- Type "TypedLua" → should see commands:
  - "TypedLua: Restart Language Server"
  - "TypedLua: Show Output Channel"

## Troubleshooting

### "Failed to start TypedLua Language Server"

The extension can't find the LSP server binary.

**Fix:**
1. Make sure you built it: `cargo build --release --package typedlua-lsp`
2. Add to PATH or set absolute path in settings:
   ```json
   {
     "typedlua.server.path": "/absolute/path/to/target/release/typedlua-lsp"
   }
   ```

### Extension doesn't activate

**Check:**
- File extension is `.tl`
- Open Developer Tools: Help > Toggle Developer Tools
- Look for JavaScript errors in Console tab
- Try reloading: Ctrl+Shift+P > "Developer: Reload Window"

### No syntax highlighting

**Check:**
- File is recognized as TypedLua (bottom-right of VS Code should show "TypedLua")
- If it says "Plain Text", click it and select "TypedLua"
- Reopen the file

### Features not working (completion, hover, etc.)

**Check Output channel:**
1. View > Output
2. Select "TypedLua Language Server"
3. Look for errors

**Enable verbose logging:**
1. Open Settings (Ctrl+,)
2. Search for "typedlua trace"
3. Set to "verbose"
4. Restart language server
5. Check Output channel again

## Next Steps

- Read [TESTING.md](./TESTING.md) for comprehensive test checklist
- Try the sample files in `test-files/`
- Report issues at https://github.com/yourusername/typed-lua/issues

## Common Test Scenarios

### Test Completion

1. Open `test-basic.tl`
2. Type `function` and press Space
3. Type `my` then Ctrl+Space
4. Should see keyword/identifier suggestions

### Test Hover

1. Open `test-basic.tl`
2. Hover over the `function` keyword
3. Should see documentation popup

### Test Go to Definition

1. Open `test-basic.tl`
2. Find the line: `local message = greet("World")`
3. Ctrl+Click on `greet` (or press F12)
4. Should jump to the function definition

### Test Diagnostics

1. Open `test-errors.tl`
2. Should see red squiggles on type errors
3. Hover over them to see error messages
4. Check Problems panel (View > Problems)

## VS Code Keyboard Shortcuts

- `F5` - Start debugging extension
- `Ctrl+Shift+P` - Command palette
- `Ctrl+Space` - Trigger completion
- `F12` - Go to definition
- `Shift+F12` - Find references
- `F2` - Rename symbol
- `Shift+Alt+F` - Format document
- `Ctrl+,` - Open settings

---

**Happy testing!**
