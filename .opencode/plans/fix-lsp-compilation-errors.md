# TypedLua LSP Compilation Errors - Fix Plan

## Summary
The `typedlua-lsp` crate has **38 compilation errors** due to:
1. Invalid import paths (`typedlua_core::typechecker` doesn't exist)
2. Invalid SymbolKind variants (`TypeAlias` should be `Type`)
3. Missing `full_completion` feature in Cargo.toml

## Root Cause
- The `typechecker` types are re-exported from `typedlua_core` root (not from a `typechecker` submodule)
- The external `typedlua_typechecker` crate exports `SymbolKind::Type` not `SymbolKind::TypeAlias`

## Files to Fix

### 1. `crates/typedlua-lsp/src/impls/compiler_bridge.rs`
**Line 16:** Change import path
```rust
// BEFORE:
use typedlua_core::typechecker::{Symbol, SymbolTable, TypeChecker as CoreTypeCheckerType};

// AFTER:
use typedlua_core::{Symbol, SymbolTable, TypeChecker as CoreTypeCheckerType};
```

**Lines 363-376:** Fix SymbolKind conversion function
```rust
// BEFORE:
fn convert_symbol_kind(kind: &typedlua_core::typechecker::SymbolKind) -> SymbolKind {
    use typedlua_core::typechecker::SymbolKind as CoreKind;
    match kind {
        CoreKind::Variable => SymbolKind::Variable,
        CoreKind::Const => SymbolKind::Const,
        CoreKind::Function => SymbolKind::Function,
        CoreKind::Class => SymbolKind::Class,
        CoreKind::Interface => SymbolKind::Interface,
        CoreKind::TypeAlias => SymbolKind::Type,  // ERROR: TypeAlias doesn't exist
        CoreKind::Enum => SymbolKind::Enum,
        CoreKind::Parameter => SymbolKind::Parameter,
        CoreKind::Namespace => SymbolKind::Namespace,
    }
}

// AFTER:
fn convert_symbol_kind(kind: &typedlua_core::SymbolKind) -> SymbolKind {
    use typedlua_core::SymbolKind as CoreKind;
    match kind {
        CoreKind::Variable => SymbolKind::Variable,
        CoreKind::Const => SymbolKind::Const,
        CoreKind::Function => SymbolKind::Function,
        CoreKind::Class => SymbolKind::Class,
        CoreKind::Interface => SymbolKind::Interface,
        CoreKind::Type => SymbolKind::Type,  // FIXED: Type not TypeAlias
        CoreKind::Enum => SymbolKind::Enum,
        CoreKind::Parameter => SymbolKind::Parameter,
        CoreKind::Namespace => SymbolKind::Namespace,
        _ => SymbolKind::Variable,  // Handle other variants
    }
}
```

### 2. `crates/typedlua-lsp/src/providers/completion.rs`
**Line 4:** Change import path
```rust
// BEFORE:
use typedlua_core::typechecker::{Symbol, SymbolKind, TypeChecker};

// AFTER:
use typedlua_core::{Symbol, SymbolKind, TypeChecker};
```

**Lines 218-227:** Fix SymbolKind match arms
```rust
// BEFORE:
let kind = match symbol.kind {
    SymbolKind::Const | SymbolKind::Variable => CompletionItemKind::VARIABLE,
    SymbolKind::Function => CompletionItemKind::FUNCTION,
    SymbolKind::Class => CompletionItemKind::CLASS,
    SymbolKind::Interface => CompletionItemKind::INTERFACE,
    SymbolKind::TypeAlias => CompletionItemKind::STRUCT,  // ERROR
    SymbolKind::Enum => CompletionItemKind::ENUM,
    SymbolKind::Parameter => CompletionItemKind::VARIABLE,
    SymbolKind::Namespace => CompletionItemKind::MODULE,
};

// AFTER:
let kind = match symbol.kind {
    SymbolKind::Const | SymbolKind::Variable => CompletionItemKind::VARIABLE,
    SymbolKind::Function => CompletionItemKind::FUNCTION,
    SymbolKind::Class => CompletionItemKind::CLASS,
    SymbolKind::Interface => CompletionItemKind::INTERFACE,
    SymbolKind::Type => CompletionItemKind::STRUCT,  // FIXED
    SymbolKind::Enum => CompletionItemKind::ENUM,
    SymbolKind::Parameter => CompletionItemKind::VARIABLE,
    SymbolKind::Namespace => CompletionItemKind::MODULE,
    _ => CompletionItemKind::VARIABLE,  // Handle other variants
};
```

**Lines 246-255:** Fix SymbolKind match in format_symbol_detail
```rust
// BEFORE:
let kind_str = match symbol.kind {
    SymbolKind::Const => "const",
    SymbolKind::Variable => "let",
    SymbolKind::Function => "function",
    SymbolKind::Class => "class",
    SymbolKind::Interface => "interface",
    SymbolKind::TypeAlias => "type",  // ERROR
    SymbolKind::Enum => "enum",
    SymbolKind::Parameter => "param",
    SymbolKind::Namespace => "namespace",
};

// AFTER:
let kind_str = match symbol.kind {
    SymbolKind::Const => "const",
    SymbolKind::Variable => "let",
    SymbolKind::Function => "function",
    SymbolKind::Class => "class",
    SymbolKind::Interface => "interface",
    SymbolKind::Type => "type",  // FIXED
    SymbolKind::Enum => "enum",
    SymbolKind::Parameter => "param",
    SymbolKind::Namespace => "namespace",
    _ => "unknown",
};
```

### 3. `crates/typedlua-lsp/src/providers/diagnostics.rs`
**Line 6:** Change import path
```rust
// BEFORE:
use typedlua_core::typechecker::TypeChecker;

// AFTER:
use typedlua_core::TypeChecker;
```

### 4. `crates/typedlua-lsp/src/providers/hover.rs`
**Line 5:** Change import path
```rust
// BEFORE:
use typedlua_core::typechecker::{SymbolKind, TypeChecker};

// AFTER:
use typedlua_core::{SymbolKind, TypeChecker};
```

**Lines 58-67:** Fix SymbolKind match
```rust
// BEFORE:
let kind_str = match symbol.kind {
    SymbolKind::Const => "const",
    SymbolKind::Variable => "let",
    SymbolKind::Function => "function",
    SymbolKind::Class => "class",
    SymbolKind::Interface => "interface",
    SymbolKind::TypeAlias => "type",  // ERROR
    SymbolKind::Enum => "enum",
    SymbolKind::Parameter => "parameter",
    SymbolKind::Namespace => "namespace",
};

// AFTER:
let kind_str = match symbol.kind {
    SymbolKind::Const => "const",
    SymbolKind::Variable => "let",
    SymbolKind::Function => "function",
    SymbolKind::Class => "class",
    SymbolKind::Interface => "interface",
    SymbolKind::Type => "type",  // FIXED
    SymbolKind::Enum => "enum",
    SymbolKind::Parameter => "parameter",
    SymbolKind::Namespace => "namespace",
    _ => "unknown",
};
```

### 5. `crates/typedlua-lsp/src/providers/inlay_hints.rs`
**Line 4:** Change import path
```rust
// BEFORE:
use typedlua_core::typechecker::TypeChecker;

// AFTER:
use typedlua_core::TypeChecker;
```

### 6. `crates/typedlua-lsp/src/providers/signature_help.rs`
**Line 6:** Change import path
```rust
// BEFORE:
use typedlua_core::typechecker::TypeChecker;

// AFTER:
use typedlua_core::TypeChecker;
```

**Line 137:** Fix type annotation
```rust
// BEFORE:
symbol: &typedlua_core::typechecker::Symbol,

// AFTER:
symbol: &typedlua_core::Symbol,
```

### 7. `crates/typedlua-lsp/Cargo.toml`
**Add feature:** After line 45, add:
```toml
# Full completion feature for enhanced IntelliSense
full_completion = []
```

## Verification Steps

After making all changes, run:
```bash
cargo check
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## SymbolKind Mapping Reference

| Local (traits/type_analysis.rs) | typedlua_core (re-exported) |
|--------------------------------|---------------------------|
| Variable | Variable |
| Const | Const |
| Function | Function |
| Class | Class |
| Interface | Interface |
| Type | Type (not TypeAlias) |
| Enum | Enum |
| Property | Property |
| Method | Method |
| Parameter | Parameter |
| Namespace | Namespace |

## Notes
- The wildcard pattern `_ => ...` is added to handle any additional variants from the external crate
- All imports from `typedlua_core::typechecker` should be changed to `typedlua_core`
- The `SymbolKind` type from `typedlua_core` is different from `lsp_types::SymbolKind` - don't confuse them
