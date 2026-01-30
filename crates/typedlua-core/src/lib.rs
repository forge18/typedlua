pub mod cache;
pub mod codegen;
pub mod config;
pub mod di;
pub mod diagnostics;
pub mod errors;
pub mod fs;
pub mod module_resolver;
pub mod optimizer;
pub mod stdlib;
pub mod typechecker;

pub use codegen::{CodeGenMode, CodeGenerator, LuaTarget};
pub use config::{CliOverrides, CompilerConfig};
pub use di::Container;
pub use diagnostics::{
    error_codes, CollectingDiagnosticHandler, Diagnostic, DiagnosticCode, DiagnosticHandler,
    DiagnosticLevel, DiagnosticRelatedInformation, DiagnosticSuggestion,
};
pub use errors::CompilationError;
pub use typechecker::{
    SerializableSymbol, SerializableSymbolTable, SymbolTable, TypeChecker, TypeEnvironment,
};
