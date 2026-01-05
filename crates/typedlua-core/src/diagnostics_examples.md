# Structured Diagnostics Examples

This document shows how to use the enhanced diagnostic system in TypedLua.

## Basic Diagnostics

```rust
use typedlua_core::diagnostics::Diagnostic;
use typedlua_core::span::Span;

let span = Span::new(0, 5, 1, 1);

// Simple error
let error = Diagnostic::error(span, "Undefined variable 'x'");

// Warning
let warning = Diagnostic::warning(span, "Unused variable 'y'");

// Info
let info = Diagnostic::info(span, "Consider using 'const' instead");
```

## Diagnostics with Codes

```rust
use typedlua_core::diagnostics::{Diagnostic, DiagnosticCode};

// Define error codes
const E1001: DiagnosticCode = DiagnosticCode::new('E', 1001); // Syntax error
const E2004: DiagnosticCode = DiagnosticCode::new('E', 2004); // Type mismatch
const W3001: DiagnosticCode = DiagnosticCode::new('W', 3001); // Unused variable

// Use codes
let error = Diagnostic::error_with_code(
    span,
    E1001,
    "Expected ';' after statement"
);

// Codes format as "E1001", "E2004", "W3001", etc.
assert_eq!(E1001.as_str(), "E1001");
```

## Related Information (Multi-span Diagnostics)

Perfect for showing related locations like where a variable was previously declared:

```rust
let declaration_span = Span::new(10, 15, 2, 1);
let usage_span = Span::new(50, 55, 5, 10);

let error = Diagnostic::error(usage_span, "Variable 'x' already declared")
    .with_related(declaration_span, "Previously declared here");
```

Output:
```
error at 5:10: Variable 'x' already declared
  Note at 2:1: Previously declared here
```

## Suggestions (Quick Fixes)

Provide actionable suggestions to fix errors:

```rust
let error = Diagnostic::error(span, "Use 'const' for immutable bindings")
    .with_suggestion(
        span,
        "const".to_string(),
        "Replace 'local' with 'const'"
    );
```

Output:
```
error at 1:1: Use 'const' for immutable bindings
  Suggestion: Replace 'local' with 'const'
```

## Builder Pattern (Chaining)

Combine all features using the builder pattern:

```rust
let error = Diagnostic::error(span, "Type mismatch")
    .with_code(DiagnosticCode::new('E', 2004))
    .with_related(type_def_span, "Expected type defined here")
    .with_related(actual_type_span, "Actual type defined here")
    .with_suggestion(span, "number".to_string(), "Change to 'number' type");
```

## Performance Benefits

1. **Structured data**: No string concatenation during error reporting
2. **Lazy formatting**: Only format when displayed
3. **Compact representation**: Uses u16 for codes, saving memory
4. **Builder pattern**: Avoids intermediate allocations
5. **LSP ready**: Structured format maps directly to LSP diagnostics

## Migration from Old API

Old:
```rust
handler.error(span, "Error message");
```

New (same API still works):
```rust
handler.error(span, "Error message");

// Or with enhanced features:
handler.report(
    Diagnostic::error(span, "Error message")
        .with_code(ERROR_CODE)
        .with_suggestion(span, fix, "Apply this fix")
);
```

## LSP Integration

The structured format maps directly to LSP `Diagnostic`:

```rust
lsp_types::Diagnostic {
    range: span_to_range(diagnostic.span),
    severity: level_to_severity(diagnostic.level),
    code: diagnostic.code.map(|c| NumberOrString::String(c.as_str())),
    message: diagnostic.message,
    related_information: diagnostic.related_information
        .iter()
        .map(|r| DiagnosticRelatedInformation {
            location: Location { uri, range: span_to_range(r.span) },
            message: r.message.clone(),
        })
        .collect(),
    ..Default::default()
}
```
