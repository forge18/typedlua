# Error Codes Usage Examples

This document demonstrates how to use TypedLua's structured error codes system.

## Basic Error with Code

```rust
use typedlua_core::{Diagnostic, error_codes, Span};

let span = Span::new(10, 15, 2, 5);
let diag = Diagnostic::error_with_code(
    span,
    error_codes::TYPE_MISMATCH,
    "Type 'string' is not assignable to type 'number'"
);

// Output: [E3001] at 2:5: Type 'string' is not assignable to type 'number'
```

## Error with Related Information

```rust
use typedlua_core::{Diagnostic, error_codes, Span};

let error_span = Span::new(10, 15, 2, 1);
let decl_span = Span::new(0, 5, 1, 1);

let diag = Diagnostic::error_with_code(
    error_span,
    error_codes::DUPLICATE_DECLARATION,
    "Duplicate declaration of variable 'x'"
).with_related(
    decl_span,
    "Previously declared here"
);

// Output:
// [E3003] at 2:1: Duplicate declaration of variable 'x'
//   Note at 1:1: Previously declared here
```

## Error with Suggestion

```rust
use typedlua_core::{Diagnostic, error_codes, Span};

let span = Span::new(10, 15, 2, 5);
let diag = Diagnostic::error_with_code(
    span,
    error_codes::TYPE_MISMATCH,
    "Type 'string' is not assignable to type 'number'"
).with_suggestion(
    span,
    "tonumber(value)".to_string(),
    "Convert to number using tonumber()"
);

// Output:
// [E3001] at 2:5: Type 'string' is not assignable to type 'number'
//   Suggestion: Convert to number using tonumber()
```

## Complex Error with Code, Related Info, and Suggestion

```rust
use typedlua_core::{Diagnostic, error_codes, Span};

let call_span = Span::new(20, 30, 3, 1);
let param_span = Span::new(5, 10, 1, 15);

let diag = Diagnostic::error_with_code(
    call_span,
    error_codes::TYPE_MISMATCH,
    "Argument of type 'string' is not assignable to parameter of type 'number'"
).with_related(
    param_span,
    "Parameter 'count' expects type 'number'"
).with_suggestion(
    call_span,
    "tonumber(value)".to_string(),
    "Wrap the argument with tonumber()"
);

// Output:
// [E3001] at 3:1: Argument of type 'string' is not assignable to parameter of type 'number'
//   Note at 1:15: Parameter 'count' expects type 'number'
//   Suggestion: Wrap the argument with tonumber()
```

## Lexer Error Example

```rust
use typedlua_core::{Diagnostic, error_codes, Span};

let span = Span::new(10, 15, 1, 10);
let diag = Diagnostic::error_with_code(
    span,
    error_codes::UNTERMINATED_STRING,
    "Unterminated string literal"
).with_suggestion(
    span,
    "\"".to_string(),
    "Add closing quote"
);

// Output:
// [E1001] at 1:10: Unterminated string literal
//   Suggestion: Add closing quote
```

## Parser Error Example

```rust
use typedlua_core::{Diagnostic, error_codes, Span};

let span = Span::new(20, 25, 2, 10);
let diag = Diagnostic::error_with_code(
    span,
    error_codes::MISSING_END,
    "Expected 'end' to close function block"
).with_suggestion(
    span,
    "end".to_string(),
    "Add 'end' keyword"
);

// Output:
// [E2008] at 2:10: Expected 'end' to close function block
//   Suggestion: Add 'end' keyword
```

## Type Checker Error Example

```rust
use typedlua_core::{Diagnostic, error_codes, Span};

let use_span = Span::new(15, 20, 3, 5);
let similar_span = Span::new(5, 10, 1, 1);

let diag = Diagnostic::error_with_code(
    use_span,
    error_codes::UNDEFINED_VARIABLE,
    "Variable 'conter' is not defined"
).with_related(
    similar_span,
    "Did you mean 'counter'?"
).with_suggestion(
    use_span,
    "counter".to_string(),
    "Replace with 'counter'"
);

// Output:
// [E3002] at 3:5: Variable 'conter' is not defined
//   Note at 1:1: Did you mean 'counter'?
//   Suggestion: Replace with 'counter'
```

## Warning Example

```rust
use typedlua_core::{Diagnostic, error_codes, Span};

let span = Span::new(10, 15, 2, 1);
let diag = Diagnostic::warning(span, "Variable 'x' is declared but never used")
    .with_code(error_codes::UNUSED_VARIABLE)
    .with_suggestion(
        span,
        "_x".to_string(),
        "Prefix with underscore if intentionally unused"
    );

// Output:
// [W1001] at 2:1: Variable 'x' is declared but never used
//   Suggestion: Prefix with underscore if intentionally unused
```

## Using in Parser

```rust
fn parse_statement(&mut self) -> Result<Statement, ParserError> {
    use typedlua_core::error_codes;

    if !self.check(&TokenKind::End) {
        let span = self.current_token().span;
        self.diagnostic_handler.report(
            Diagnostic::error_with_code(
                span,
                error_codes::MISSING_END,
                "Expected 'end' to close block"
            ).with_suggestion(
                span,
                "end".to_string(),
                "Add 'end' keyword to close the block"
            )
        );
        return Err(ParserError::UnexpectedToken);
    }

    // ... rest of parsing
}
```

## Using in Type Checker

```rust
fn check_assignment(&mut self, lhs_type: &Type, rhs_type: &Type, span: Span) {
    use typedlua_core::error_codes;

    if !self.is_assignable(rhs_type, lhs_type) {
        let mut diag = Diagnostic::error_with_code(
            span,
            error_codes::TYPE_MISMATCH,
            format!("Type '{}' is not assignable to type '{}'", rhs_type, lhs_type)
        );

        // Add helpful suggestion based on types
        if matches!(lhs_type, Type::Primitive(PrimitiveType::Number))
            && matches!(rhs_type, Type::Primitive(PrimitiveType::String)) {
            diag = diag.with_suggestion(
                span,
                "tonumber(value)".to_string(),
                "Convert string to number using tonumber()"
            );
        }

        self.diagnostic_handler.report(diag);
    }
}
```

## Error Code Ranges

- **E1000-E1999**: Lexer errors (unterminated strings, invalid characters, etc.)
- **E2000-E2999**: Parser errors (syntax errors, unexpected tokens, etc.)
- **E3000-E3999**: Type checker errors (type mismatches, undefined variables, etc.)
- **E4000-E4999**: Code generator errors (unsupported features, etc.)
- **E5000-E5999**: Configuration errors (invalid config files, etc.)
- **W1000-W9999**: Warnings (unused variables, deprecated features, etc.)

## Benefits

1. **Searchability**: Users can look up error codes in documentation
2. **Tooling**: IDEs can provide quick fixes based on error codes
3. **Consistency**: Same error always has the same code
4. **Categorization**: Easy to filter errors by component
5. **Documentation**: Each code can have detailed explanations
