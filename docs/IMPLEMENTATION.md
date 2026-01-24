# TypedLua Implementation Guide

**Version:** 1.0
**Last Updated:** 2026-01-13

## Table of Contents

- [Overview](#overview)
- [Code Organization](#code-organization)
- [Implementation Patterns](#implementation-patterns)
- [Key Algorithms](#key-algorithms)
- [Type System Implementation](#type-system-implementation)
- [Testing Strategy](#testing-strategy)
- [Performance Optimizations](#performance-optimizations)
- [Common Tasks](#common-tasks)

---

## Overview

This document provides practical guidance for working with the TypedLua codebase, covering implementation patterns, algorithms, and best practices.

### Prerequisites

- Rust 1.70+
- Familiarity with compiler concepts (lexing, parsing, type checking)
- Understanding of Lua semantics

---

## Code Organization

### Project Structure

```text
typed-lua/
├── crates/
│   ├── typedlua-core/          # Core compiler logic
│   │   ├── src/
│   │   │   ├── lib.rs          # Public API exports
│   │   │   ├── di.rs           # Dependency injection
│   │   │   ├── config.rs       # Configuration
│   │   │   ├── diagnostics.rs  # Error reporting
│   │   │   ├── errors.rs       # Error types
│   │   │   ├── span.rs         # Source locations
│   │   │   ├── arena.rs        # Memory arena
│   │   │   ├── string_interner.rs
│   │   │   ├── lexer/
│   │   │   │   ├── mod.rs      # Lexer implementation
│   │   │   │   └── token.rs    # Token types
│   │   │   ├── parser/
│   │   │   │   ├── mod.rs      # Parser core
│   │   │   │   ├── expression.rs
│   │   │   │   ├── statement.rs
│   │   │   │   ├── types.rs    # Type annotations
│   │   │   │   ├── pattern.rs  # Pattern matching
│   │   │   │   └── tests.rs
│   │   │   ├── ast/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── expression.rs
│   │   │   │   ├── statement.rs
│   │   │   │   ├── types.rs
│   │   │   │   └── pattern.rs
│   │   │   ├── typechecker/
│   │   │   │   ├── mod.rs      # Type checker exports
│   │   │   │   ├── type_checker.rs
│   │   │   │   ├── type_environment.rs
│   │   │   │   ├── symbol_table.rs
│   │   │   │   ├── type_compat.rs
│   │   │   │   ├── generics.rs
│   │   │   │   ├── narrowing.rs
│   │   │   │   ├── utility_types.rs
│   │   │   │   └── tests.rs
│   │   │   ├── codegen/
│   │   │   │   ├── mod.rs      # Code generation
│   │   │   │   └── sourcemap.rs
│   │   │   ├── fs/
│   │   │   │   └── mod.rs      # File system abstraction
│   │   │   └── stdlib/
│   │   │       └── mod.rs      # Standard library types
│   │   └── tests/              # Integration tests
│   │       ├── oop_tests.rs
│   │       ├── pattern_matching_tests.rs
│   │       ├── type_narrowing_tests.rs
│   │       └── ...
│   ├── typedlua-cli/           # Command-line interface
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   └── ...
│   │   └── tests/
│   └── typedlua-lsp/           # Language server
│       ├── src/
│       │   ├── main.rs
│       │   ├── document.rs
│       │   └── providers/
│       │       ├── completion.rs
│       │       ├── diagnostics.rs
│       │       ├── hover.rs
│       │       └── ...
│       └── tests/
└── docs/
    ├── ARCHITECTURE.md
    ├── IMPLEMENTATION.md (this file)
    ├── SECURITY.md
    ├── TypedLua-Design.md
    └── ...
```

### Module Responsibilities

| Module            | Purpose                      | Key Types                                |
|-------------------|------------------------------|------------------------------------------|
| `arena`           | Bump allocator for AST nodes | `Arena<'ast>`                            |
| `ast`             | AST node definitions         | `Expression`, `Statement`, `Type`        |
| `codegen`         | Lua code emission            | `CodeGenerator`                          |
| `config`          | Configuration management     | `CompilerConfig`, `CompilerOptions`      |
| `di`              | Dependency injection         | `Container`                              |
| `diagnostics`     | Error/warning reporting      | `DiagnosticHandler`, `Diagnostic`        |
| `fs`              | File system abstraction      | `FileSystem` trait                       |
| `lexer`           | Tokenization                 | `Lexer`, `Token`, `TokenKind`            |
| `parser`          | AST construction             | `Parser`, `Program`                      |
| `span`            | Source location tracking     | `Span`                                   |
| `string_interner` | String deduplication         | `StringInterner`, `StringId`             |
| `typechecker`     | Type analysis                | `TypeChecker`, `Type`, `TypeEnvironment` |

---

## Implementation Patterns

### Pattern 1: Dependency Injection

**All major components receive dependencies via constructor:**

```rust
// Good: Explicit dependencies
pub struct Parser<'ast> {
    arena: &'ast Arena<'ast>,
    tokens: Vec<Token>,
    current: usize,
    diagnostics: Arc<dyn DiagnosticHandler>,
    config: Arc<CompilerConfig>,
}

impl<'ast> Parser<'ast> {
    pub fn new(
        arena: &'ast Arena<'ast>,
        tokens: Vec<Token>,
        diagnostics: Arc<dyn DiagnosticHandler>,
        config: Arc<CompilerConfig>,
    ) -> Self {
        Parser {
            arena,
            tokens,
            current: 0,
            diagnostics,
            config,
        }
    }
}

// Bad: Global state
static mut DIAGNOSTICS: Option<DiagnosticHandler> = None;
```

### Pattern 2: Trait-Based Abstraction

**Use traits for swappable implementations:**

```rust
// File system abstraction
pub trait FileSystem: Send + Sync {
    fn read_file(&self, path: &Path) -> Result<String, std::io::Error>;
    fn write_file(&self, path: &Path, content: &str) -> Result<(), std::io::Error>;
}

// Production implementation
pub struct RealFileSystem;
impl FileSystem for RealFileSystem { ... }

// Test implementation
pub struct MockFileSystem {
    files: HashMap<PathBuf, String>,
}
impl FileSystem for MockFileSystem { ... }
```

### Pattern 3: Arena Allocation

**AST nodes are allocated in an arena:**

```rust
// Allocate an expression
let expr = arena.alloc(Expression::Literal(Literal::Number(42.0)));

// Return &'ast references
fn parse_expression(&mut self) -> &'ast Expression<'ast> {
    let expr = /* ... */;
    self.arena.alloc(expr)
}
```

**Benefits:**

- Fast allocation (bump pointer)
- Automatic cleanup (drop entire arena)
- Cache-friendly (sequential allocation)

**Drawbacks:**

- Cannot free individual nodes
- Lifetime annotations required

### Pattern 4: Visitor Pattern for AST Traversal

**Use `match` to traverse AST nodes:**

```rust
impl<'ast> TypeChecker<'ast> {
    fn check_expression(&mut self, expr: &'ast Expression<'ast>) -> Type {
        match expr {
            Expression::Literal(lit) => self.check_literal(lit),
            Expression::Identifier(name) => self.lookup_variable(name),
            Expression::Binary { op, lhs, rhs } => {
                let lhs_ty = self.check_expression(lhs);
                let rhs_ty = self.check_expression(rhs);
                self.check_binary_op(op, lhs_ty, rhs_ty)
            }
            Expression::Call { callee, args } => {
                self.check_call(callee, args)
            }
            // ... handle all variants
        }
    }
}
```

### Pattern 5: Result-Based Error Handling

**Use `Result<T, E>` for recoverable errors:**

```rust
pub fn compile_file(&self, path: &Path) -> Result<String, CompilationError> {
    let source = self.fs.read_file(path)?;
    let tokens = self.lex(&source)?;
    let ast = self.parse(tokens)?;
    let typed_ast = self.type_check(ast)?;
    let lua_code = self.codegen(typed_ast)?;
    Ok(lua_code)
}
```

**Use diagnostics for user-facing errors:**

```rust
// Report to user
self.diagnostics.error(span, "Type mismatch: expected number, found string");

// Check for errors
if self.diagnostics.has_errors() {
    return Err(CompilationError::TypeCheckFailed);
}
```

### Pattern 6: Flyweight (String Interner)

**Intern strings to save memory and enable fast comparison:**

```rust
let mut interner = StringInterner::new();

let id1 = interner.intern("foo");
let id2 = interner.intern("foo");

assert_eq!(id1, id2); // Same ID
assert_eq!(interner.resolve(id1), "foo");

// Use StringId instead of String in AST
struct Identifier {
    name: StringId, // Not String!
    span: Span,
}
```

**Important: Always use a single shared StringInterner across all components:**

```rust
// WRONG: Each component creates its own interner
let (mut interner1, _) = StringInterner::new_with_common_identifiers();
let (mut interner2, _) = StringInterner::new_with_common_identifiers();
// AST contains StringIds from interner1, but TypeChecker uses interner2
// This will cause "index out of bounds" panics!

// CORRECT: Single shared interner
let (mut interner, common_ids) = StringInterner::new_with_common_identifiers();

let mut lexer = Lexer::new(source, handler.clone(), &mut interner);
let mut parser = Parser::new(tokens, handler.clone(), &mut interner, &common_ids);
let program = parser.parse().unwrap();

// TypeChecker receives a reference to the SAME interner
let mut type_checker = TypeChecker::new(handler.clone(), &interner);
```

**Why StringId is important:**

- `StringId` is a `u32` under the hood - tiny compared to `String`
- Comparison is O(1) instead of O(n) for string comparison
- Enables efficient hashing in HashMap/FxHashMap

**Pattern for identifier comparison:**

```rust
// Get string from AST (already a StringId)
if let ExpressionKind::Identifier(name_id) = &expr.kind {
    // Resolve to &str for display/debugging
    let name = interner.resolve(*name_id);

    // Compare StringIds directly (O(1))
    if *name_id == some_known_id {
        // ...
    }
}
```

---

## Key Algorithms

### Lexical Analysis

**Algorithm:** Single-pass character scanner with lookahead

```rust
pub fn tokenize(&mut self) -> Result<Vec<Token>, LexerError> {
    let mut tokens = Vec::new();

    while !self.is_at_end() {
        self.skip_whitespace();
        self.skip_comments();

        if self.is_at_end() {
            break;
        }

        let start = self.position;
        let token = self.scan_token()?;
        tokens.push(token);
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: self.current_span(),
    });

    Ok(tokens)
}

fn scan_token(&mut self) -> Result<Token, LexerError> {
    let ch = self.advance();

    match ch {
        '(' => self.make_token(TokenKind::LeftParen),
        ')' => self.make_token(TokenKind::RightParen),
        '+' => self.make_token(TokenKind::Plus),
        '-' => {
            if self.peek() == '>' {
                self.advance();
                self.make_token(TokenKind::Arrow)
            } else {
                self.make_token(TokenKind::Minus)
            }
        }
        '"' => self.scan_string(),
        '0'..='9' => self.scan_number(),
        'a'..='z' | 'A'..='Z' | '_' => self.scan_identifier_or_keyword(),
        _ => Err(LexerError::UnexpectedCharacter(ch)),
    }
}
```

### Parsing - Recursive Descent

**Algorithm:** Top-down recursive descent with precedence climbing for expressions

```rust
// Recursive descent for statements
fn parse_statement(&mut self) -> Result<&'ast Statement<'ast>, ParseError> {
    match self.current().kind {
        TokenKind::Local => self.parse_local_declaration(),
        TokenKind::Const => self.parse_const_declaration(),
        TokenKind::If => self.parse_if_statement(),
        TokenKind::While => self.parse_while_statement(),
        TokenKind::Function => self.parse_function_declaration(),
        _ => self.parse_expression_statement(),
    }
}

// Precedence climbing for expressions
fn parse_expression_with_precedence(&mut self, min_prec: u8) -> &'ast Expression<'ast> {
    let mut lhs = self.parse_primary_expression();

    while let Some(op) = self.current_binary_operator() {
        let prec = op.precedence();
        if prec < min_prec {
            break;
        }

        self.advance(); // Consume operator

        let rhs = self.parse_expression_with_precedence(prec + 1);

        lhs = self.arena.alloc(Expression::Binary {
            op,
            lhs,
            rhs,
        });
    }

    lhs
}
```

### Type Checking - Bidirectional

**Algorithm:** Bidirectional type checking with inference

```rust
// Check expression against expected type (checking mode)
fn check_expression(&mut self, expr: &'ast Expression, expected: &Type) -> Type {
    match expr {
        // Can infer type
        Expression::Literal(_) | Expression::Identifier(_) => {
            let inferred = self.infer_expression(expr);
            self.check_compatible(&inferred, expected);
            inferred
        }
        // Need expected type
        Expression::Lambda { params, body } => {
            if let Type::Function(expected_fn) = expected {
                self.check_lambda(params, body, expected_fn)
            } else {
                self.error("Expected function type for lambda");
                Type::Error
            }
        }
        _ => {
            let inferred = self.infer_expression(expr);
            self.check_compatible(&inferred, expected);
            inferred
        }
    }
}

// Infer expression type (inference mode)
fn infer_expression(&mut self, expr: &'ast Expression) -> Type {
    match expr {
        Expression::Literal(Literal::Number(_)) => Type::Number,
        Expression::Literal(Literal::String(_)) => Type::String,
        Expression::Literal(Literal::Boolean(_)) => Type::Boolean,
        Expression::Identifier(name) => {
            self.symbol_table.lookup(name)
                .unwrap_or(Type::Unknown)
        }
        Expression::Binary { op, lhs, rhs } => {
            let lhs_ty = self.infer_expression(lhs);
            let rhs_ty = self.infer_expression(rhs);
            self.infer_binary_op(op, lhs_ty, rhs_ty)
        }
        // ... other cases
    }
}
```

### Type Compatibility

**Algorithm:** Structural subtyping with caching

```rust
pub fn is_assignable(&mut self, source: &Type, target: &Type) -> bool {
    // Check cache
    if let Some(&result) = self.compat_cache.get(&(source.clone(), target.clone())) {
        return result;
    }

    let result = match (source, target) {
        // Any type is assignable to unknown
        (_, Type::Unknown) => true,

        // Nothing is assignable to never
        (Type::Never, _) => true,
        (_, Type::Never) => false,

        // Structural typing for tables
        (Type::Table(source_fields), Type::Table(target_fields)) => {
            self.check_table_compatibility(source_fields, target_fields)
        }

        // Union types
        (source, Type::Union(targets)) => {
            targets.iter().any(|t| self.is_assignable(source, t))
        }
        (Type::Union(sources), target) => {
            sources.iter().all(|s| self.is_assignable(s, target))
        }

        // Exact match
        _ => source == target,
    };

    // Cache result
    self.compat_cache.insert((source.clone(), target.clone()), result);
    result
}
```

### Control Flow Narrowing

**Algorithm:** Track type refinements through control flow

```rust
fn check_if_statement(&mut self, condition: &Expression, then_block: &Block, else_block: Option<&Block>) {
    // Check condition
    let cond_ty = self.infer_expression(condition);

    // Extract type guards
    let guards = self.extract_type_guards(condition);

    // Then branch: apply positive guards
    self.symbol_table.push_scope();
    for (var, refined_type) in &guards {
        self.symbol_table.refine_type(var, refined_type);
    }
    self.check_block(then_block);
    self.symbol_table.pop_scope();

    // Else branch: apply negative guards
    if let Some(else_block) = else_block {
        self.symbol_table.push_scope();
        for (var, excluded_type) in &guards {
            let current_type = self.symbol_table.lookup(var).unwrap();
            let narrowed = self.subtract_type(&current_type, excluded_type);
            self.symbol_table.refine_type(var, &narrowed);
        }
        self.check_block(else_block);
        self.symbol_table.pop_scope();
    }
}
```

---

## Type System Implementation

### Type Representation

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // Primitives
    Nil,
    Boolean,
    Number,
    Integer,
    String,

    // Special types
    Unknown,  // Top type (anything assignable to unknown)
    Never,    // Bottom type (never returns)
    Void,     // No return value

    // Composite types
    Table {
        fields: HashMap<String, Type>,
        index_signature: Option<Box<(Type, Type)>>,
    },
    Array(Box<Type>),
    Tuple(Vec<Type>),
    Function {
        params: Vec<FunctionParam>,
        returns: Vec<Type>,
    },
    Union(Vec<Type>),

    // Generics
    TypeParameter {
        name: String,
        constraint: Option<Box<Type>>,
    },
    Generic {
        base: Box<Type>,
        type_args: Vec<Type>,
    },

    // References
    TypeRef(String), // Named type reference
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParam {
    pub name: String,
    pub ty: Type,
    pub optional: bool,
    pub rest: bool,
}
```

### Generic Instantiation

```rust
pub fn instantiate_generic(&mut self, generic_type: &Type, type_args: &[Type]) -> Type {
    match generic_type {
        Type::Generic { base, .. } => {
            let param_names = self.extract_type_parameters(base);

            if param_names.len() != type_args.len() {
                self.error("Wrong number of type arguments");
                return Type::Error;
            }

            // Build substitution map
            let mut substitutions = HashMap::new();
            for (param, arg) in param_names.iter().zip(type_args) {
                substitutions.insert(param.clone(), arg.clone());
            }

            // Substitute type parameters
            self.substitute_type(base, &substitutions)
        }
        _ => generic_type.clone(),
    }
}

fn substitute_type(&self, ty: &Type, subs: &HashMap<String, Type>) -> Type {
    match ty {
        Type::TypeParameter { name, .. } => {
            subs.get(name).cloned().unwrap_or_else(|| ty.clone())
        }
        Type::Function { params, returns } => {
            Type::Function {
                params: params.iter().map(|p| FunctionParam {
                    name: p.name.clone(),
                    ty: self.substitute_type(&p.ty, subs),
                    optional: p.optional,
                    rest: p.rest,
                }).collect(),
                returns: returns.iter().map(|r| self.substitute_type(r, subs)).collect(),
            }
        }
        Type::Union(types) => {
            Type::Union(types.iter().map(|t| self.substitute_type(t, subs)).collect())
        }
        // ... recursively substitute in all composite types
        _ => ty.clone(),
    }
}
```

---

## Testing Strategy

### Unit Tests

**Located:** `#[cfg(test)] mod tests` in each module

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_numbers() {
        let diagnostics = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new("42 3.14 0xFF", diagnostics);

        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens.len(), 4); // 3 numbers + EOF
        assert!(matches!(tokens[0].kind, TokenKind::Number(_)));
    }
}
```

### Integration Tests

**Located:** `crates/typedlua-core/tests/*.rs`

```rust
// tests/type_narrowing_tests.rs

use typedlua_core::*;

#[test]
fn test_type_narrowing_with_typeof() {
    let source = r#"
        function check(x: number | string): number
            if typeof(x) == "number" then
                return x  -- x is narrowed to number
            else
                return 0
            end
        end
    "#;

    let result = compile_source(source);
    assert!(result.is_ok());
    assert_eq!(result.diagnostics().len(), 0);
}
```

### Snapshot Testing

**Using `insta` for AST and output testing:**

```rust
use insta::assert_debug_snapshot;

#[test]
fn test_parse_function_declaration() {
    let source = "function add(a: number, b: number): number return a + b end";
    let ast = parse(source).unwrap();

    assert_debug_snapshot!(ast);
}
```

### Property-Based Testing

**Using `quickcheck` or custom fuzzing:**

```rust
#[test]
fn fuzz_lexer() {
    for _ in 0..10000 {
        let input = generate_random_source();
        let _ = Lexer::new(&input, diagnostics.clone()).tokenize();
        // Should not panic
    }
}
```

### Test Helpers

```rust
// Helper to quickly set up test infrastructure
fn setup_test_compiler() -> (Container, Arena<'static>) {
    let config = CompilerConfig::default();
    let diagnostics = Arc::new(CollectingDiagnosticHandler::new());
    let fs = Arc::new(MockFileSystem::new());
    let container = Container::with_dependencies(config, diagnostics, fs);
    let arena = Arena::new();
    (container, arena)
}
```

---

## Performance Optimizations

### 1. Arena Allocation

**Benchmark:** 10-100x faster than `Box::new()`

```rust
// Fast: Bump allocation
let node = arena.alloc(Expression::Literal(...));

// Slow: Heap allocation
let node = Box::new(Expression::Literal(...));
```

### 2. String Interning

**Benchmark:** 10x faster string comparison, 50% less memory

```rust
// Fast: Compare integers
if id1 == id2 { /* strings are equal */ }

// Slow: Compare string contents
if str1 == str2 { /* expensive */ }
```

### 3. Type Compatibility Caching

**Prevents exponential blowup in complex types:**

```rust
// Cache results
self.compat_cache.insert((source, target), result);

// Check cache first
if let Some(&cached) = self.compat_cache.get(&(source, target)) {
    return cached;
}
```

### 4. Parallel Compilation (Future)

**Use `rayon` for parallel file compilation:**

```rust
use rayon::prelude::*;

files.par_iter().map(|file| {
    compile_file(file)
}).collect()
```

---

## Common Tasks

### Adding a New AST Node

1. **Define the node in `ast/<category>.rs`:**

```rust
// ast/expression.rs
pub enum Expression<'ast> {
    // ... existing variants

    /// New node: spread operator
    Spread {
        expr: &'ast Expression<'ast>,
        span: Span,
    },
}
```

1. **Update the parser:**

```rust
// parser/expression.rs
fn parse_primary_expression(&mut self) -> &'ast Expression<'ast> {
    match self.current().kind {
        TokenKind::DotDotDot => {
            let start = self.current().span;
            self.advance();
            let expr = self.parse_expression();
            self.arena.alloc(Expression::Spread { expr, span: start })
        }
        // ... other cases
    }
}
```

1. **Update the type checker:**

```rust
// typechecker/type_checker.rs
fn check_expression(&mut self, expr: &Expression) -> Type {
    match expr {
        Expression::Spread { expr, span } => {
            self.check_spread(expr, *span)
        }
        // ... other cases
    }
}
```

1. **Update the code generator:**

```rust
// codegen/mod.rs
fn generate_expression(&mut self, expr: &Expression) {
    match expr {
        Expression::Spread { expr, .. } => {
            self.output.push_str("...");
            self.generate_expression(expr);
        }
        // ... other cases
    }
}
```

1. **Add tests:**

```rust
#[test]
fn test_spread_in_function_call() {
    let source = "foo(...args)";
    let result = compile_source(source);
    assert!(result.is_ok());
}
```

### Adding a New Compiler Option

1. **Add to `CompilerOptions` in `config.rs`:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilerOptions {
    // ... existing options

    /// Enable experimental async/await syntax
    #[serde(default)]
    pub enable_async: bool,
}
```

1. **Use in relevant component:**

```rust
if self.config.compiler_options.enable_async {
    // Parse async syntax
} else {
    self.diagnostics.error(span, "async/await requires enableAsync: true");
}
```

1. **Document in README and config schema**

### Adding a New Diagnostic Error

1. **Add error code in `diagnostics.rs`:**

```rust
pub mod error_codes {
    pub const TYPE_MISMATCH: &str = "E0001";
    pub const UNDEFINED_VARIABLE: &str = "E0002";
    pub const NEW_ERROR: &str = "E0042"; // New error
}
```

1. **Report the error:**

```rust
self.diagnostics.error_with_code(
    span,
    error_codes::NEW_ERROR,
    "Description of the error",
);
```

1. **Add to documentation in `docs/ERROR_CODES.md`**

---

## Best Practices

### DO

- ✅ Use dependency injection
- ✅ Allocate AST nodes in arena
- ✅ Intern strings for identifiers
- ✅ Cache expensive computations
- ✅ Write unit tests for each module
- ✅ Use `Result` for errors, diagnostics for user messages
- ✅ Document public APIs with doc comments
- ✅ Run `cargo fmt` and `cargo clippy` before committing

### DON'T

- ❌ Use global mutable state
- ❌ Heap-allocate AST nodes with `Box`
- ❌ Store `String` in AST (use `StringId`)
- ❌ Create separate StringInterners for different components
- ❌ Panic in library code (use `Result`)
- ❌ Ignore clippy warnings
- ❌ Skip tests for new features
- ❌ Commit code with `dbg!()` statements

---

## Troubleshooting

### Compilation is Slow

- Check if you're heap-allocating AST nodes (use arena)
- Profile with `cargo flamegraph`
- Consider parallelizing file compilation

### Memory Usage is High

- Ensure strings are interned
- Check for type compatibility cache unbounded growth
- Use `cargo bloat` to find large types

### Tests are Flaky

- Ensure tests don't depend on filesystem state
- Use mocks for all external dependencies
- Avoid global state

### "index out of bounds" panic when resolving StringId

**Cause:** You're trying to use a `StringId` from one `StringInterner` with a different `StringInterner`.

**Solution:** Use a single shared `StringInterner`:

```rust
// Create one interner at the top level
let (mut interner, common_ids) = StringInterner::new_with_common_identifiers();

// Pass reference to all components
let mut lexer = Lexer::new(source, handler.clone(), &mut interner);
let mut parser = Parser::new(tokens, handler.clone(), &mut interner, &common_ids);
let program = parser.parse().unwrap();

// TypeChecker receives a reference
let mut type_checker = TypeChecker::new(handler.clone(), &interner);
```

---

## References

- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture overview
- [TypedLua Design](TypedLua-Design.md) - Type system specification
- [AST Structure](AST-Structure.md) - AST node definitions
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

---

**Version:** 1.0
**Contributors:** TypedLua Team
**License:** MIT
