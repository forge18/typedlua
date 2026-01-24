# String Interner Architecture

## Overview

The TypedLua compiler uses a string interner to deduplicate string storage and enable efficient identifier comparison. This document describes the correct architecture and usage patterns.

## The Problem

A common mistake is creating multiple `StringInterner` instances. When TypeChecker has its own separate interner from the Lexer/Parser, the AST contains `StringId`s from one interner while TypeChecker tries to resolve them in a different (empty) interner. This causes "index out of bounds" panics.

**Wrong**:

```rust
// Lexer/Parser uses one interner
let (mut interner, common_ids) = StringInterner::new_with_common_identifiers();
let mut lexer = Lexer::new(source, handler.clone(), &mut interner);
let mut parser = Parser::new(tokens, handler.clone(), &mut interner, &common_ids);
let program = parser.parse().unwrap();

// TypeChecker creates its OWN empty interner
let mut tc = TypeChecker::new(handler.clone()); // <-- NEW empty interner
let _ = tc.check_program(&program); // PANIC: StringIds from different interner
```

## The Solution: Single Shared Interner

**Correct**:

```rust
// One interner, shared by all components
let (mut interner, common_ids) = StringInterner::new_with_common_identifiers();
let interner_ref = &interner; // Reference for TypeChecker

let mut lexer = Lexer::new(source, handler.clone(), &mut interner);
// ... parse ...

// All components use the same interner reference
let mut tc = TypeChecker::new(handler.clone(), interner_ref);
```

## API Reference

### StringInterner

```rust
pub struct StringInterner {
    string_to_id: FxHashMap<String, StringId>,
    id_to_string: Vec<String>,
}

impl StringInterner {
    /// Create a new empty interner
    pub fn new() -> Self

    /// Create with common Lua keywords pre-registered
    pub fn new_with_common_identifiers() -> (Self, CommonIdentifiers)

    /// Intern a string, returning its ID
    pub fn intern(&mut self, s: &str) -> StringId

    /// Resolve a StringId back to &str (read-only operation)
    pub fn resolve(&self, id: StringId) -> &str
}
```

### TypeChecker

```rust
pub struct TypeChecker<'a> {
    // ... fields ...
    interner: &'a StringInterner,
}

impl<'a> TypeChecker<'a> {
    pub fn new(
        diagnostic_handler: Arc<dyn DiagnosticHandler>,
        interner: &'a StringInterner,
    ) -> Self
}
```

## Usage Pattern for Tests

```rust
#[test]
fn test_example() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (mut interner, common_ids) = StringInterner::new_with_common_identifiers();

    // Lex and parse using the interner
    let mut lexer = Lexer::new(source, handler.clone(), &mut interner);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens, handler.clone(), &mut interner, &common_ids);
    let program = parser.parse().unwrap();

    // TypeCheck with reference to the SAME interner
    let interner_ref = &interner; // Keep reference alive
    let mut tc = TypeChecker::new(handler.clone(), interner_ref);
    tc.check_program(&program).unwrap();
}
```

## Usage Pattern for CLI/LSP

```rust
// At the top level, create one interner
let (mut interner, common_ids) = StringInterner::new_with_common_identifiers();

// Pass reference through the pipeline
let mut lexer = Lexer::new(source, handler.clone(), &mut interner);
let tokens = lexer.tokenize()?;
let mut parser = Parser::new(tokens, handler.clone(), &mut interner, &common_ids);
let program = parser.parse()?;

// TypeChecker receives reference (read-only access)
let mut type_checker = TypeChecker::new(handler.clone(), &interner);
type_checker.check_program(&program)?;

// CodeGenerator also receives the interner
let mut codegen = CodeGenerator::new(&interner);
let output = codegen.generate(&program);
```

## CommonIdentifiers

For efficiency, common Lua keywords are pre-interned:

```rust
pub struct CommonIdentifiers {
    pub nil: StringId,
    pub true_: StringId,
    pub false_: StringId,
    pub and: StringId,
    pub or: StringId,
    pub not: StringId,
    pub function: StringId,
    pub local: StringId,
    pub const_: StringId,
    pub return_: StringId,
    pub if_: StringId,
    // ... more keywords
}
```

Use `new_with_common_identifiers()` to create an interner with these pre-registered:

```rust
let (mut interner, common_ids) = StringInterner::new_with_common_identifiers();
// common_ids.nil is already available as a StringId
```

## Why This Architecture?

1. **Memory efficiency**: Strings are stored once, referenced by `StringId` (u32)
2. **Fast comparison**: `StringId` comparison is O(1) vs O(n) for string comparison
3. **Single source of truth**: All components agree on what each `StringId` means

## Anti-Patterns to Avoid

1. **Don't create separate interners for different components**
2. **Don't pass owned `StringInterner` to TypeChecker** (causes lifetime issues)
3. **Don't try to intern new strings in TypeChecker** (it only has read access)

## Performance Notes

- The interner is thread-safe for read operations via `&StringInterner`
- Pre-registering common identifiers saves memory and lookup time
- For multi-threaded scenarios, use `Arc<StringInterner>` and clone for write access
