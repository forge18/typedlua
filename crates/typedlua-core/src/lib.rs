pub mod arena;
pub mod ast;
pub mod codegen;
pub mod config;
pub mod di;
pub mod diagnostics;
pub mod errors;
pub mod fs;
pub mod lexer;
pub mod parser;
pub mod span;
pub mod stdlib;
pub mod string_interner;
pub mod typechecker;

pub use arena::Arena;
pub use ast::{Program, Spanned};
pub use codegen::CodeGenerator;
pub use config::{CliOverrides, CompilerConfig};
pub use di::Container;
pub use diagnostics::{
    error_codes, Diagnostic, DiagnosticCode, DiagnosticHandler, DiagnosticLevel,
    DiagnosticRelatedInformation, DiagnosticSuggestion,
};
pub use errors::CompilationError;
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::Parser;
pub use span::Span;
pub use string_interner::{StringId, StringInterner};
pub use typechecker::{SymbolTable, TypeChecker, TypeEnvironment};
