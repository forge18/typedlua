pub mod ast;
pub mod config;
pub mod di;
pub mod diagnostics;
pub mod errors;
pub mod fs;
pub mod lexer;
pub mod parser;
pub mod span;

pub use ast::{Program, Spanned};
pub use config::{CliOverrides, CompilerConfig};
pub use di::Container;
pub use diagnostics::{Diagnostic, DiagnosticHandler, DiagnosticLevel};
pub use errors::CompilationError;
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::Parser;
pub use span::Span;
