mod expression;
mod pattern;
mod statement;
mod types;

#[cfg(test)]
mod tests;

use crate::ast::Program;
use crate::diagnostics::{Diagnostic, DiagnosticHandler, DiagnosticLevel};
use crate::lexer::{Token, TokenKind};
use crate::span::Span;
use std::sync::Arc;

pub use expression::ExpressionParser;
pub use pattern::PatternParser;
pub use statement::StatementParser;
pub use types::TypeParser;

#[derive(Debug, Clone)]
pub struct ParserError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at line {}", self.message, self.span.line)
    }
}

impl std::error::Error for ParserError {}

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    diagnostic_handler: Arc<dyn DiagnosticHandler>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, diagnostic_handler: Arc<dyn DiagnosticHandler>) -> Self {
        Parser {
            tokens,
            position: 0,
            diagnostic_handler,
        }
    }

    pub fn parse(&mut self) -> Result<Program, ParserError> {
        let start_span = self.current_span();
        let mut statements = Vec::new();

        while !self.is_at_end() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    self.report_error(&e.message, e.span);
                    // Error recovery: skip to next statement
                    self.synchronize();
                }
            }
        }

        let end_span = if !statements.is_empty() {
            statements.last().unwrap().span()
        } else {
            start_span
        };

        Ok(Program::new(statements, start_span.combine(&end_span)))
    }

    // Token stream management
    fn current(&self) -> &Token {
        self.tokens.get(self.position).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("Token stream should never be empty")
        })
    }

    fn peek(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.position + offset)
    }

    fn is_at_end(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.position += 1;
        }
        &self.tokens[self.position - 1]
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(kind)
    }

    fn match_token(&mut self, kinds: &[TokenKind]) -> bool {
        for kind in kinds {
            if self.check(kind) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn consume(&mut self, kind: TokenKind, message: &str) -> Result<&Token, ParserError> {
        if self.check(&kind) {
            return Ok(self.advance());
        }

        Err(ParserError {
            message: message.to_string(),
            span: self.current_span(),
        })
    }

    fn current_span(&self) -> Span {
        self.current().span
    }

    // Error reporting
    fn report_error(&self, message: &str, span: Span) {
        self.diagnostic_handler.report(Diagnostic {
            level: DiagnosticLevel::Error,
            span,
            message: message.to_string(),
        });
    }

    // Error recovery: skip to next statement boundary
    fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            // Check if we're at a statement boundary
            match &self.current().kind {
                TokenKind::Function
                | TokenKind::Local
                | TokenKind::Const
                | TokenKind::If
                | TokenKind::While
                | TokenKind::For
                | TokenKind::Return
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Interface
                | TokenKind::Type
                | TokenKind::Enum
                | TokenKind::Class
                | TokenKind::Import
                | TokenKind::Export => return,
                _ => {}
            }

            self.advance();
        }
    }
}

// Helper trait to get span from any statement/expression
trait Spannable {
    fn span(&self) -> Span;
}

impl Spannable for crate::ast::statement::Statement {
    fn span(&self) -> Span {
        use crate::ast::statement::Statement::*;
        match self {
            Variable(v) => v.span,
            Function(f) => f.span,
            Class(c) => c.span,
            Interface(i) => i.span,
            TypeAlias(t) => t.span,
            Enum(e) => e.span,
            Import(i) => i.span,
            Export(e) => e.span,
            If(i) => i.span,
            While(w) => w.span,
            For(f) => match f {
                crate::ast::statement::ForStatement::Numeric(n) => n.span,
                crate::ast::statement::ForStatement::Generic(g) => g.span,
            },
            Repeat(r) => r.span,
            Return(r) => r.span,
            Break(s) | Continue(s) => *s,
            Expression(e) => e.span,
            Block(b) => b.span,
        }
    }
}
