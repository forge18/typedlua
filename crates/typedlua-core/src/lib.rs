// Keep core-specific modules
pub mod arena;
pub mod cache;
pub mod codegen;
pub mod di;
pub mod optimizer;
pub mod type_checker;

// Re-export arena for convenience
pub use arena::Arena;

// Re-export shared utilities and types used by typedlua-core
pub use typedlua_typechecker::{
    // Generic specialization optimizer functions
    build_substitutions,
    instantiate_function_declaration,
    // Module resolver
    module_resolver,
    // Symbol table for caching
    SerializableSymbolTable,
    Symbol,
    SymbolTable,
    // Main type checker
    TypeChecker,
};

// Re-export CLI modules - shared utilities
pub use typedlua_typechecker::cli::{config, diagnostics, errors, fs};

// Re-export common diagnostics for backward compatibility
pub use typedlua_typechecker::cli::diagnostics::{
    CollectingDiagnosticHandler, Diagnostic, DiagnosticHandler, DiagnosticLevel,
};

// Re-export parser types
pub use typedlua_parser::{
    ast::Program,
    string_interner::{CommonIdentifiers, StringId, StringInterner},
};

use std::path::PathBuf;
use typedlua_parser::ast::statement::Statement;
use typedlua_parser::span::Span;

/// A mutable program representation for post-type-checking phases (optimizer, codegen).
///
/// After parsing and type checking (which use arena-allocated immutable AST),
/// statements are cloned into a `Vec` so the optimizer can mutate them in-place.
/// This follows the pattern of having separate representations for different phases,
/// similar to how rustc has HIR (immutable) and MIR (mutable for optimization).
#[derive(Debug, Clone)]
pub struct MutableProgram<'arena> {
    pub statements: Vec<Statement<'arena>>,
    pub span: Span,
}

impl<'arena> MutableProgram<'arena> {
    /// Convert an arena-allocated Program into a mutable representation.
    pub fn from_program(program: &Program<'arena>) -> Self {
        MutableProgram {
            statements: program.statements.to_vec(),
            span: program.span,
        }
    }
}

/// A module after parsing, before type checking.
/// Foundation for parallel parsing infrastructure.
pub struct ParsedModule<'arena> {
    pub path: PathBuf,
    pub ast: Program<'arena>,
    pub interner: StringInterner,
    pub common_ids: CommonIdentifiers,
    pub diagnostics: Vec<Diagnostic>,
}
