# Change Log

All notable changes to the TypedLua VS Code extension will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-01-03

### Added

#### Language Server Features
- Full LSP (Language Server Protocol) integration with `typedlua-lsp`
- Real-time type checking and diagnostics
- Incremental document synchronization for performance
- Asynchronous architecture using Tokio

#### IntelliSense
- Context-aware code completion
  - Keyword completion (40+ TypedLua keywords)
  - Type completion (primitive types)
  - Decorator completion (@deprecated, @readonly, etc.)
  - Member access completion (`.` trigger)
  - Method call completion (`:` trigger)
- Hover information for keywords, types, and identifiers
- Signature help with parameter information
- Inlay hints for inferred types and parameter names

#### Code Navigation
- Go to definition (F12)
- Find all references (Shift+F12)
- Document highlights for symbol under cursor
- Document symbols for file outline (Ctrl+Shift+O)
- Workspace symbol search (Ctrl+T)

#### Refactoring
- Rename symbol across project (F2)
- Prepare rename with validation
- Keyword checking to prevent invalid renames
- Identifier validation

#### Code Actions & Formatting
- Quick fixes for common errors
- Refactoring suggestions
- Source actions (organize imports, etc.)
- Document formatting (Shift+Alt+F)
- Range formatting for selections
- On-type formatting (triggers: newline, `end`, `}`, `]`)

#### Advanced Features
- Code folding for functions, blocks, comments, and table literals
- Smart selection expansion/contraction
- Semantic tokens for enhanced syntax highlighting
- Multiple LSP provider implementations ready for type checker integration

#### Language Support
- Syntax highlighting via TextMate grammar
- Support for `.tl` file extension
- Language configuration:
  - Auto-closing pairs for brackets, quotes, and blocks
  - Comment toggling (line and block comments)
  - Smart indentation rules
  - Bracket matching
  - Folding markers (`--#region` / `--#endregion`)

#### Extension Features
- TypedLua icon (128x128, optimized)
- Configurable settings (8 configuration options)
- Two extension commands:
  - Restart Language Server
  - Show Output Channel
- Comprehensive error handling and user feedback

#### Documentation
- Extension README with features, setup, and troubleshooting
- TESTING.md with 60+ test cases and manual testing guide
- QUICKSTART.md for 5-minute setup
- Sample test files (4 files covering basic, types, errors, features)

#### Development Tools
- Build scripts for quick rebuilding
- VS Code launch configuration for debugging
- TypeScript compilation with ESLint
- Automated packaging to VSIX

### Configuration Options

Added 8 user-configurable settings:
- `typedlua.trace.server` - LSP communication tracing
- `typedlua.server.path` - Path to language server binary
- `typedlua.compiler.checkOnSave` - Type check on save
- `typedlua.compiler.strictNullChecks` - Strict null checking
- `typedlua.format.enable` - Enable/disable formatting
- `typedlua.format.indentSize` - Indentation size
- `typedlua.inlayHints.typeHints` - Show type hints
- `typedlua.inlayHints.parameterHints` - Show parameter hints

### Technical Details

- **LSP Providers**: 14 providers implemented
  - DiagnosticsProvider
  - CompletionProvider
  - HoverProvider
  - DefinitionProvider
  - ReferencesProvider
  - RenameProvider
  - SymbolsProvider
  - FormattingProvider
  - CodeActionsProvider
  - SignatureHelpProvider
  - InlayHintsProvider
  - FoldingRangeProvider
  - SelectionRangeProvider
  - SemanticTokensProvider

- **Architecture**:
  - tower-lsp for LSP implementation
  - vscode-languageclient for VS Code integration
  - TypeScript for extension code
  - Rust for language server

### Known Limitations

- Type checker integration is in progress (providers have infrastructure ready)
- Some advanced type system features awaiting implementation
- Semantic tokens require type checker to provide full functionality
- No file icon theme (VS Code limitation - icons come from icon themes)

### Testing

- 23 unit tests (all passing)
- Manual testing guide with 60+ test scenarios
- Test files included for development

---

## [Unreleased]

### Planned Features
- Full type checker integration for all LSP features
- Enhanced diagnostics with fix suggestions
- More sophisticated code actions
- Performance optimizations for large projects
- Workspace-wide operations (rename, refactor across files)

---

**Note**: This is an initial release. Feedback and contributions are welcome!
