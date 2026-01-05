mod symbol_table;
mod type_environment;
mod type_checker;
mod type_compat;
mod generics;
mod utility_types;
mod narrowing;
mod narrowing_integration;

#[cfg(test)]
mod tests;

pub use symbol_table::{Symbol, SymbolKind, SymbolTable, Scope};
pub use type_environment::TypeEnvironment;
pub use type_checker::TypeChecker;
pub use type_compat::TypeCompatibility;
pub use generics::{instantiate_type, check_type_constraints, infer_type_arguments};
pub use utility_types::{apply_utility_type, evaluate_mapped_type, evaluate_keyof, evaluate_conditional_type, evaluate_template_literal_type};
pub use narrowing::{NarrowingContext, narrow_type_from_condition};

use crate::span::Span;

/// Type checker error
#[derive(Debug, Clone)]
pub struct TypeCheckError {
    pub message: String,
    pub span: Span,
}

impl TypeCheckError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl std::fmt::Display for TypeCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}:{}", self.message, self.span.line, self.span.column)
    }
}

impl std::error::Error for TypeCheckError {}
