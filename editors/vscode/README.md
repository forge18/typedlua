# TypedLua for Visual Studio Code

![TypedLua Logo](assets/typedlua-logo.png)

Official VS Code extension for [TypedLua](https://github.com/yourusername/typed-lua) - a statically typed dialect of Lua that brings TypeScript-like type safety to Lua development.

## Features

### 🎨 Rich Language Support

- **Syntax Highlighting**: Full syntax highlighting for TypedLua with TextMate grammar
- **Semantic Tokens**: Context-aware highlighting based on type information
- **IntelliSense**: Smart code completion with context awareness
  - Keyword completion
  - Type completion
  - Member access (`.` trigger)
  - Method calls (`:` trigger)
  - Decorator completion (`@` trigger)

### 🔍 Type Checking & Diagnostics

- **Real-time Type Checking**: Catch type errors as you type
- **Error Detection**: Comprehensive error messages with suggested fixes
- **Warning System**: Helpful warnings for potential issues
- **Diagnostic Panel**: All errors and warnings in one place

### 🚀 Code Navigation

- **Go to Definition** (`F12`): Jump to symbol definitions
- **Find References** (`Shift+F12`): Find all usages of a symbol
- **Document Symbols** (`Ctrl+Shift+O`): Quick navigation within files
- **Workspace Symbols** (`Ctrl+T`): Search symbols across your project
- **Document Highlights**: Highlight all occurrences of symbol under cursor

### ✏️ Refactoring

- **Rename Symbol** (`F2`): Rename across entire project
- **Code Actions**: Quick fixes and refactoring suggestions
- **Smart Rename**: Validates identifier names and checks for keywords

### 📝 Code Assistance

- **Hover Information**: View type information and documentation
- **Signature Help**: Parameter hints while typing function calls
- **Inlay Hints**: Inline type annotations and parameter names
- **Auto-Closing Pairs**: Automatic closing of brackets, quotes, and blocks
- **Smart Indentation**: Context-aware indentation

### 🎯 Formatting

- **Document Formatting** (`Shift+Alt+F`): Format entire file
- **Range Formatting**: Format selected code
- **On-Type Formatting**: Auto-format as you type (on newline, `end`, `}`, `]`)
- **Configurable**: Customize indent size and formatting style

### 📚 Code Folding

- **Function Folding**: Collapse function bodies
- **Block Folding**: Fold if/while/for blocks
- **Comment Folding**: Fold multi-line and consecutive comments
- **Region Markers**: Custom folding regions with `--#region`

### ⚙️ Smart Features

- **Selection Expansion**: Smart expand/shrink selection (`Alt+Shift+→/←`)
- **Bracket Matching**: Highlight matching brackets
- **Comment Toggle**: Quick line/block comments
- **Auto-Indentation**: Intelligent indentation rules

## Requirements

This extension requires the TypedLua language server (`typedlua-lsp`) to be installed.

### Installation

1. **Install the TypedLua compiler:**
   ```bash
   cargo install typedlua
   ```

2. **Verify installation:**
   ```bash
   typedlua-lsp --version
   ```

3. **Configure path (optional):**
   If the binary isn't in your PATH, set the absolute path in VS Code settings:
   ```json
   {
     "typedlua.server.path": "/absolute/path/to/typedlua-lsp"
   }
   ```

## Quick Start

1. **Create a new TypedLua file:**
   - Create a file with `.tl` extension
   - Example: `hello.tl`

2. **Write TypedLua code:**
   ```lua
   function greet(name: string): string
       return "Hello, " .. name
   end

   const message: string = greet("World")
   print(message)
   ```

3. **See it in action:**
   - Type checking happens automatically
   - Hover over variables to see types
   - Press `Ctrl+Space` for completions
   - Press `F12` on `greet` to go to definition

## Extension Settings

Configure TypedLua through VS Code settings (File > Preferences > Settings):

### Language Server

- **`typedlua.server.path`** (string, default: `"typedlua-lsp"`)
  Path to the TypedLua language server executable

- **`typedlua.trace.server`** (enum: "off" | "messages" | "verbose", default: `"off"`)
  Trace communication between VS Code and the language server (for debugging)

### Compiler

- **`typedlua.compiler.checkOnSave`** (boolean, default: `true`)
  Run type checking when saving files

- **`typedlua.compiler.strictNullChecks`** (boolean, default: `true`)
  Enable strict null checking

### Formatting

- **`typedlua.format.enable`** (boolean, default: `true`)
  Enable/disable code formatting

- **`typedlua.format.indentSize`** (number, default: `4`)
  Number of spaces for indentation

### Inlay Hints

- **`typedlua.inlayHints.typeHints`** (boolean, default: `true`)
  Show inlay hints for inferred types

- **`typedlua.inlayHints.parameterHints`** (boolean, default: `true`)
  Show inlay hints for parameter names

## Commands

Access commands via Command Palette (`Ctrl+Shift+P` or `Cmd+Shift+P`):

- **`TypedLua: Restart Language Server`**
  Restart the language server (useful if it crashes or becomes unresponsive)

- **`TypedLua: Show Output Channel`**
  Show the language server output channel for debugging

## Keyboard Shortcuts

### Navigation
- `F12` - Go to Definition
- `Shift+F12` - Find All References
- `Alt+F12` - Peek Definition
- `Ctrl+Shift+O` - Go to Symbol in File
- `Ctrl+T` - Go to Symbol in Workspace

### Editing
- `F2` - Rename Symbol
- `Ctrl+Space` - Trigger Suggestions
- `Ctrl+Shift+Space` - Trigger Parameter Hints
- `Shift+Alt+F` - Format Document
- `Ctrl+K Ctrl+F` - Format Selection

### Code Folding
- `Ctrl+Shift+[` - Fold Region
- `Ctrl+Shift+]` - Unfold Region
- `Ctrl+K Ctrl+0` - Fold All
- `Ctrl+K Ctrl+J` - Unfold All

## TypedLua Language Features

### Type Annotations
```lua
local name: string = "TypedLua"
local count: number = 42
local isActive: boolean = true
const PI: number = 3.14159
```

### Functions with Types
```lua
function add(a: number, b: number): number
    return a + b
end
```

### Type Aliases
```lua
type Point = {
    x: number,
    y: number
}
```

### Interfaces
```lua
interface Drawable {
    draw(): void
    move(dx: number, dy: number): void
}
```

### Classes
```lua
class Rectangle {
    width: number
    height: number

    function new(width: number, height: number)
        self.width = width
        self.height = height
    end

    function area(): number
        return self.width * self.height
    end
}
```

### Enums
```lua
enum Color {
    Red = "red",
    Green = "green",
    Blue = "blue"
}
```

### Generics
```lua
function identity<T>(value: T): T
    return value
end
```

### Union Types
```lua
type StringOrNumber = string | number

function process(value: StringOrNumber): string
    if type(value) == "string" then
        return value
    else
        return tostring(value)
    end
end
```

## Troubleshooting

### Extension doesn't activate
- Check that the file extension is `.tl`
- Look for errors in Developer Tools (Help > Toggle Developer Tools)
- Try reloading the window (Ctrl+Shift+P > "Developer: Reload Window")

### Language server doesn't start
- Verify `typedlua-lsp` is installed: `which typedlua-lsp`
- Check the Output panel (View > Output) for "TypedLua Language Server"
- Set absolute path in settings: `typedlua.server.path`
- Enable verbose logging: set `typedlua.trace.server` to `"verbose"`

### Features not working
- Check that the language server is running (look in Output panel)
- Try restarting the server: Ctrl+Shift+P > "TypedLua: Restart Language Server"
- Check for errors in the Output panel

### Performance issues
- Large files (>1000 lines) may be slower - consider splitting into modules
- Disable inlay hints if they cause lag: set `typedlua.inlayHints.typeHints` to `false`

## Known Issues

- This is an early release (v0.1.0)
- Some advanced type system features are still in development
- Semantic tokens require type checker integration (coming soon)

Please report issues at [GitHub Issues](https://github.com/yourusername/typed-lua/issues).

## Release Notes

### 0.1.0 (Initial Release)

**Features:**
- ✅ Full language server integration
- ✅ Syntax highlighting with TextMate grammar
- ✅ IntelliSense (completion, hover, signatures)
- ✅ Type checking and diagnostics
- ✅ Code navigation (go to definition, find references)
- ✅ Refactoring (rename symbol)
- ✅ Code actions and quick fixes
- ✅ Document formatting
- ✅ Inlay hints for types and parameters
- ✅ Code folding
- ✅ Smart selection

**Language Support:**
- ✅ Type annotations
- ✅ Type aliases and interfaces
- ✅ Classes and enums
- ✅ Generics
- ✅ Union types
- ✅ Decorators

See [CHANGELOG.md](CHANGELOG.md) for detailed release notes.

## Contributing

TypedLua is open source! Contributions are welcome:

- [GitHub Repository](https://github.com/yourusername/typed-lua)
- [Issue Tracker](https://github.com/yourusername/typed-lua/issues)
- [Documentation](https://github.com/yourusername/typed-lua/docs)

## Resources

- **Documentation**: [TypedLua Docs](https://github.com/yourusername/typed-lua/docs)
- **Grammar Specification**: [Grammar.md](https://github.com/yourusername/typed-lua/docs/Grammar.md)
- **LSP Design**: [LSP-Design.md](https://github.com/yourusername/typed-lua/docs/LSP-Design.md)
- **Examples**: See `test-files/` in the extension directory

## License

MIT License - see [LICENSE](../../LICENSE) for details.

---

**Enjoy coding with TypedLua!** 🚀
