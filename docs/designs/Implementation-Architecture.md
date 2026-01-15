# TypedLua Implementation Architecture

**Document Version:** 0.1  
**Last Updated:** 2024-12-31

This document outlines the architectural design for implementing the TypedLua compiler in Rust, with a focus on dependency injection, modularity, and testability.

---

## Core Principles

1. **Dependency Injection** - Components receive dependencies through constructors, not global state
2. **Single Responsibility** - Each module has one clear purpose
3. **Testability** - All components can be tested in isolation
4. **Immutability** - Prefer immutable data structures where possible
5. **Error Handling** - Use `Result<T, E>` for all fallible operations

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                          CLI Layer                          │
│  - Argument parsing                                         │
│  - Configuration loading                                    │
│  - Orchestration                                            │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                     Compiler Pipeline                       │
│  - Coordinates compilation phases                           │
│  - Manages dependency injection container                   │
└────────────────────┬────────────────────────────────────────┘
                     │
        ┌────────────┼────────────┐
        ▼            ▼            ▼
    ┌───────┐   ┌───────┐   ┌───────────┐
    │ Lexer │   │Parser │   │Type Checker│
    └───────┘   └───────┘   └───────────┘
        │            │            │
        └────────────┼────────────┘
                     ▼
              ┌─────────────┐
              │Code Generator│
              └─────────────┘
                     │
                     ▼
              ┌─────────────┐
              │   Emitter   │
              └─────────────┘
```

---

## Dependency Injection Container

### Container Design

```rust
// di/container.rs

pub struct Container {
    config: Arc<CompilerConfig>,
    diagnostic_handler: Arc<dyn DiagnosticHandler>,
    file_system: Arc<dyn FileSystem>,
    module_resolver: Arc<dyn ModuleResolver>,
}

impl Container {
    pub fn new(config: CompilerConfig) -> Self {
        let config = Arc::new(config);
        
        let diagnostic_handler = Arc::new(
            ConsoleDiagnosticHandler::new(config.clone())
        );
        
        let file_system = Arc::new(
            RealFileSystem::new()
        );
        
        let module_resolver = Arc::new(
            DefaultModuleResolver::new(
                config.clone(),
                file_system.clone()
            )
        );
        
        Container {
            config,
            diagnostic_handler,
            file_system,
            module_resolver,
        }
    }
    
    pub fn create_lexer(&self, source: &str) -> Lexer {
        Lexer::new(
            source,
            self.diagnostic_handler.clone()
        )
    }
    
    pub fn create_parser(&self, tokens: Vec<Token>) -> Parser {
        Parser::new(
            tokens,
            self.config.clone(),
            self.diagnostic_handler.clone()
        )
    }
    
    pub fn create_type_checker(&self) -> TypeChecker {
        TypeChecker::new(
            self.config.clone(),
            self.diagnostic_handler.clone(),
            self.module_resolver.clone()
        )
    }
    
    pub fn create_code_generator(&self) -> CodeGenerator {
        CodeGenerator::new(
            self.config.clone()
        )
    }
    
    // For testing: create container with mock dependencies
    pub fn with_mocks(
        config: CompilerConfig,
        diagnostic_handler: Arc<dyn DiagnosticHandler>,
        file_system: Arc<dyn FileSystem>,
    ) -> Self {
        let config = Arc::new(config);
        
        let module_resolver = Arc::new(
            DefaultModuleResolver::new(
                config.clone(),
                file_system.clone()
            )
        );
        
        Container {
            config,
            diagnostic_handler,
            file_system,
            module_resolver,
        }
    }
}
```

---

## Component Interfaces

### Diagnostic Handler

```rust
// diagnostics/mod.rs

pub trait DiagnosticHandler: Send + Sync {
    fn error(&self, span: Span, message: &str);
    fn warning(&self, span: Span, message: &str);
    fn info(&self, span: Span, message: &str);
    fn has_errors(&self) -> bool;
    fn error_count(&self) -> usize;
}

// Concrete implementations
pub struct ConsoleDiagnosticHandler {
    config: Arc<CompilerConfig>,
    errors: Mutex<Vec<Diagnostic>>,
}

pub struct CollectingDiagnosticHandler {
    diagnostics: Mutex<Vec<Diagnostic>>,
}

impl DiagnosticHandler for ConsoleDiagnosticHandler {
    fn error(&self, span: Span, message: &str) {
        eprintln!("Error at {}:{}: {}", span.line, span.column, message);
        self.errors.lock().unwrap().push(Diagnostic {
            level: DiagnosticLevel::Error,
            span,
            message: message.to_string(),
        });
    }
    
    // ...
}
```

### File System Abstraction

```rust
// fs/mod.rs

pub trait FileSystem: Send + Sync {
    fn read_file(&self, path: &Path) -> Result<String, std::io::Error>;
    fn write_file(&self, path: &Path, content: &str) -> Result<(), std::io::Error>;
    fn exists(&self, path: &Path) -> bool;
    fn resolve_path(&self, base: &Path, relative: &str) -> PathBuf;
}

// Real implementation
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn read_file(&self, path: &Path) -> Result<String, std::io::Error> {
        std::fs::read_to_string(path)
    }
    
    fn write_file(&self, path: &Path, content: &str) -> Result<(), std::io::Error> {
        std::fs::write(path, content)
    }
    
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    
    fn resolve_path(&self, base: &Path, relative: &str) -> PathBuf {
        base.join(relative)
    }
}

// Mock for testing
pub struct MockFileSystem {
    files: HashMap<PathBuf, String>,
}

impl FileSystem for MockFileSystem {
    fn read_file(&self, path: &Path) -> Result<String, std::io::Error> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File not found"
            ))
    }
    
    // ...
}
```

### Module Resolver

```rust
// modules/resolver.rs

pub trait ModuleResolver: Send + Sync {
    fn resolve(&self, from: &Path, import_path: &str) -> Result<ResolvedModule, ResolutionError>;
    fn load_type_definitions(&self, module_path: &Path) -> Result<Option<TypeDefinitions>, std::io::Error>;
}

pub struct ResolvedModule {
    pub path: PathBuf,
    pub source: String,
    pub has_type_definitions: bool,
}

pub struct DefaultModuleResolver {
    config: Arc<CompilerConfig>,
    file_system: Arc<dyn FileSystem>,
}

impl ModuleResolver for DefaultModuleResolver {
    fn resolve(&self, from: &Path, import_path: &str) -> Result<ResolvedModule, ResolutionError> {
        // 1. Try .tl file
        // 2. Try .lua file (if allowNonTypedLua)
        // 3. Check for .d.tl definition file
        // 4. Apply path aliases from config
        // ...
    }
}
```

---

## Compiler Pipeline

```rust
// compiler/pipeline.rs

pub struct CompilerPipeline {
    container: Container,
}

impl CompilerPipeline {
    pub fn new(config: CompilerConfig) -> Self {
        CompilerPipeline {
            container: Container::new(config),
        }
    }
    
    pub fn compile_file(&self, path: &Path) -> Result<CompiledOutput, CompilationError> {
        // Read source
        let source = self.container.file_system
            .read_file(path)
            .map_err(CompilationError::IoError)?;
        
        // Lexical analysis
        let mut lexer = self.container.create_lexer(&source);
        let tokens = lexer.tokenize()?;
        
        if self.container.diagnostic_handler.has_errors() {
            return Err(CompilationError::LexicalErrors);
        }
        
        // Parse
        let mut parser = self.container.create_parser(tokens);
        let ast = parser.parse()?;
        
        if self.container.diagnostic_handler.has_errors() {
            return Err(CompilationError::ParseErrors);
        }
        
        // Type check
        let mut type_checker = self.container.create_type_checker();
        let typed_ast = type_checker.check(ast)?;
        
        if self.container.diagnostic_handler.has_errors() {
            return Err(CompilationError::TypeErrors);
        }
        
        // Code generation
        let mut code_gen = self.container.create_code_generator();
        let lua_code = code_gen.generate(typed_ast)?;
        
        Ok(CompiledOutput {
            lua_code,
            source_map: code_gen.source_map(),
        })
    }
    
    pub fn compile_project(&self, root: &Path) -> Result<Vec<CompiledOutput>, CompilationError> {
        // Find all files matching include/exclude patterns
        // Compile each file
        // Handle cross-file type checking
        // ...
    }
}
```

---

## Component Design

### Lexer

```rust
// lexer/mod.rs

pub struct Lexer {
    source: String,
    position: usize,
    line: usize,
    column: usize,
    diagnostic_handler: Arc<dyn DiagnosticHandler>,
}

impl Lexer {
    pub fn new(source: &str, diagnostic_handler: Arc<dyn DiagnosticHandler>) -> Self {
        Lexer {
            source: source.to_string(),
            position: 0,
            line: 1,
            column: 1,
            diagnostic_handler,
        }
    }
    
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexerError> {
        let mut tokens = Vec::new();
        
        while !self.is_at_end() {
            self.skip_whitespace();
            if self.is_at_end() {
                break;
            }
            
            let token = self.next_token()?;
            tokens.push(token);
        }
        
        Ok(tokens)
    }
    
    fn next_token(&mut self) -> Result<Token, LexerError> {
        // Tokenization logic
        // Reports errors via diagnostic_handler
    }
}
```

### Parser

```rust
// parser/mod.rs

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    config: Arc<CompilerConfig>,
    diagnostic_handler: Arc<dyn DiagnosticHandler>,
}

impl Parser {
    pub fn new(
        tokens: Vec<Token>,
        config: Arc<CompilerConfig>,
        diagnostic_handler: Arc<dyn DiagnosticHandler>
    ) -> Self {
        Parser {
            tokens,
            position: 0,
            config,
            diagnostic_handler,
        }
    }
    
    pub fn parse(&mut self) -> Result<AST, ParserError> {
        // Check if OOP features are enabled
        if !self.config.enable_oop && self.check_keyword("class") {
            self.diagnostic_handler.error(
                self.current_span(),
                "Classes are disabled. Set enableOOP: true"
            );
            return Err(ParserError::DisabledFeature);
        }
        
        // Parse AST
        // ...
    }
}
```

### Type Checker

```rust
// typechecker/mod.rs

pub struct TypeChecker {
    config: Arc<CompilerConfig>,
    diagnostic_handler: Arc<dyn DiagnosticHandler>,
    module_resolver: Arc<dyn ModuleResolver>,
    symbol_table: SymbolTable,
    type_environment: TypeEnvironment,
}

impl TypeChecker {
    pub fn new(
        config: Arc<CompilerConfig>,
        diagnostic_handler: Arc<dyn DiagnosticHandler>,
        module_resolver: Arc<dyn ModuleResolver>
    ) -> Self {
        TypeChecker {
            config,
            diagnostic_handler,
            module_resolver,
            symbol_table: SymbolTable::new(),
            type_environment: TypeEnvironment::new(),
        }
    }
    
    pub fn check(&mut self, ast: AST) -> Result<TypedAST, TypeCheckError> {
        // Type checking logic
        // Uses module_resolver to load external type definitions
        // Reports errors via diagnostic_handler
    }
}
```

---

## Testing Strategy

### Unit Testing with Mocks

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lexer_with_mock_diagnostics() {
        let diagnostics = Arc::new(CollectingDiagnosticHandler::new());
        let lexer = Lexer::new("const x = 5", diagnostics.clone());
        
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens.len(), 4);
        assert!(!diagnostics.has_errors());
    }
    
    #[test]
    fn test_module_resolver_with_mock_fs() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs.add_file(
            Path::new("/project/utils.tl"),
            "export function add(a: number, b: number): number"
        );
        
        let config = Arc::new(CompilerConfig::default());
        let resolver = DefaultModuleResolver::new(
            config,
            Arc::new(mock_fs)
        );
        
        let result = resolver.resolve(
            Path::new("/project"),
            "./utils"
        ).unwrap();
        
        assert_eq!(result.path, Path::new("/project/utils.tl"));
    }
}
```

### Integration Testing

```rust
#[cfg(test)]
mod integration_tests {
    #[test]
    fn test_compile_simple_program() {
        let config = CompilerConfig {
            target: LuaVersion::Lua54,
            enable_oop: true,
            ..Default::default()
        };
        
        let pipeline = CompilerPipeline::new(config);
        
        let source = r#"
            const x: number = 5
            const y: number = 10
            const sum = x + y
        "#;
        
        let output = pipeline.compile_string(source).unwrap();
        
        assert!(output.lua_code.contains("local x = 5"));
    }
}
```

---

## Error Handling

### Error Types

```rust
// errors/mod.rs

#[derive(Debug, thiserror::Error)]
pub enum CompilationError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Lexical analysis failed")]
    LexicalErrors,
    
    #[error("Parsing failed")]
    ParseErrors,
    
    #[error("Type checking failed")]
    TypeErrors,
    
    #[error("Code generation failed: {0}")]
    CodeGenError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ResolutionError {
    #[error("Module not found: {0}")]
    ModuleNotFound(String),
    
    #[error("Circular dependency detected")]
    CircularDependency,
    
    #[error("Non-typed Lua file without type definitions: {0}")]
    MissingTypeDefinitions(String),
}
```

---

## Configuration Management

```rust
// config/mod.rs

#[derive(Debug, Clone, Deserialize)]
pub struct CompilerConfig {
    #[serde(default)]
    pub compiler_options: CompilerOptions,
    
    #[serde(default)]
    pub include: Vec<String>,
    
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompilerOptions {
    #[serde(default = "default_true")]
    pub strict_null_checks: bool,
    
    #[serde(default = "default_error")]
    pub strict_naming: StrictLevel,
    
    #[serde(default)]
    pub no_implicit_unknown: bool,
    
    #[serde(default)]
    pub no_explicit_unknown: bool,
    
    #[serde(default = "default_lua54")]
    pub target: LuaVersion,
    
    #[serde(default = "default_true")]
    pub enable_oop: bool,
    
    #[serde(default = "default_true")]
    pub enable_fp: bool,
    
    #[serde(default = "default_true")]
    pub enable_decorators: bool,
    
    #[serde(default = "default_true")]
    pub allow_non_typed_lua: bool,
    
    // ... other options
}

impl CompilerConfig {
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: CompilerConfig = serde_json::from_str(&content)?;
        Ok(config)
    }
}
```

---

## Module Structure

```
typedlua/
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── lib.rs                  # Library entry point
│   │
│   ├── di/                     # Dependency injection
│   │   ├── mod.rs
│   │   └── container.rs
│   │
│   ├── config/                 # Configuration
│   │   ├── mod.rs
│   │   └── loader.rs
│   │
│   ├── diagnostics/            # Error reporting
│   │   ├── mod.rs
│   │   ├── handler.rs
│   │   └── diagnostic.rs
│   │
│   ├── fs/                     # File system abstraction
│   │   ├── mod.rs
│   │   ├── real.rs
│   │   └── mock.rs
│   │
│   ├── lexer/                  # Lexical analysis
│   │   ├── mod.rs
│   │   ├── token.rs
│   │   └── span.rs
│   │
│   ├── parser/                 # Syntax analysis
│   │   ├── mod.rs
│   │   ├── ast.rs
│   │   └── precedence.rs
│   │
│   ├── typechecker/            # Type checking
│   │   ├── mod.rs
│   │   ├── types.rs
│   │   ├── environment.rs
│   │   └── inference.rs
│   │
│   ├── codegen/                # Code generation
│   │   ├── mod.rs
│   │   ├── lua_emitter.rs
│   │   └── source_map.rs
│   │
│   ├── modules/                # Module resolution
│   │   ├── mod.rs
│   │   └── resolver.rs
│   │
│   ├── compiler/               # Compiler pipeline
│   │   ├── mod.rs
│   │   └── pipeline.rs
│   │
│   └── errors/                 # Error types
│       └── mod.rs
│
└── tests/
    ├── integration/
    └── fixtures/
```

---

## Benefits of This Architecture

1. **Testability**
   - Each component can be tested in isolation with mock dependencies
   - Integration tests verify end-to-end functionality
   
2. **Maintainability**
   - Clear separation of concerns
   - Easy to locate and modify specific functionality
   
3. **Flexibility**
   - Easy to swap implementations (e.g., different file systems, diagnostic handlers)
   - Feature flags are configuration-driven
   
4. **Performance**
   - `Arc` allows efficient sharing of immutable data
   - Parallel compilation of multiple files possible
   
5. **Error Handling**
   - Consistent error reporting through diagnostic handler
   - Rich error types with context

---

## Next Steps

1. Implement base Container and DI infrastructure
2. Create trait definitions for all abstractions
3. Implement Lexer with diagnostic reporting
4. Build Parser with feature flag support
5. Develop Type Checker with module resolution
6. Create Code Generator
7. Write comprehensive tests for each component
8. Build CLI that orchestrates the pipeline

---

**Document Version:** 0.1  
**Last Updated:** 2024-12-31
