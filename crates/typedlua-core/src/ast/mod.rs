pub mod expression;
pub mod pattern;
pub mod statement;
pub mod types;

use crate::span::Span;

/// Wrapper for AST nodes with span information
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Spanned { node, span }
    }
}

/// Identifier
pub type Ident = Spanned<String>;

/// Top-level program
#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<statement::Statement>,
    pub span: Span,
}

impl Program {
    pub fn new(statements: Vec<statement::Statement>, span: Span) -> Self {
        Program { statements, span }
    }
}
