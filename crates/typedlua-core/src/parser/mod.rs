mod expression;
mod pattern;
mod statement;
mod types;

#[cfg(test)]
mod tests;

use crate::ast::Program;
use crate::diagnostics::{Diagnostic, DiagnosticHandler};
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

    // Error reporting with automatic error code assignment and helpful suggestions
    fn report_error(&self, message: &str, span: Span) {
        use crate::error_codes;

        // Assign error codes and add helpful suggestions based on message patterns
        let diagnostic = if message.contains("Classes are disabled") {
            Diagnostic::error_with_code(span, error_codes::CLASSES_DISABLED, message.to_string())
                .with_suggestion(span, "".to_string(), "Enable OOP features in tlconfig.yaml by setting enableOOP: true")
        } else if message.contains("Decorators are disabled") {
            Diagnostic::error_with_code(span, error_codes::DECORATORS_DISABLED, message.to_string())
                .with_suggestion(span, "".to_string(), "Enable decorators in tlconfig.yaml by setting enableDecorators: true")
        } else if message.contains("Functional programming features are disabled") {
            Diagnostic::error_with_code(span, error_codes::FP_DISABLED, message.to_string())
                .with_suggestion(span, "".to_string(), "Enable FP features in tlconfig.yaml by setting enableFP: true")
        } else if message.contains("break") && message.contains("outside") {
            Diagnostic::error_with_code(span, error_codes::BREAK_OUTSIDE_LOOP, message.to_string())
        } else if message.contains("continue") && message.contains("outside") {
            Diagnostic::error_with_code(span, error_codes::CONTINUE_OUTSIDE_LOOP, message.to_string())
        } else if message.contains("end") {
            Diagnostic::error_with_code(span, error_codes::MISSING_END, message.to_string())
                .with_suggestion(span, "end".to_string(), "Add 'end' keyword to close the block")
        } else if message.contains("then") {
            Diagnostic::error_with_code(span, error_codes::MISSING_THEN, message.to_string())
                .with_suggestion(span, "then".to_string(), "Add 'then' keyword after the condition")
        } else if message.contains("do") && (message.contains("while") || message.contains("for")) {
            Diagnostic::error_with_code(span, error_codes::MISSING_DO, message.to_string())
                .with_suggestion(span, "do".to_string(), "Add 'do' keyword to start the loop body")
        } else if message.contains("Expected ')'") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TOKEN, message.to_string())
                .with_suggestion(span, ")".to_string(), "Add closing parenthesis ')'")
        } else if message.contains("Expected ']'") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TOKEN, message.to_string())
                .with_suggestion(span, "]".to_string(), "Add closing bracket ']'")
        } else if message.contains("Expected '}'") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TOKEN, message.to_string())
                .with_suggestion(span, "}".to_string(), "Add closing brace '}'")
        } else if message.contains("Expected ':'") && message.contains("type") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TYPE, message.to_string())
                .with_suggestion(span, ": type".to_string(), "Add type annotation with ':'")
        } else if message.contains("Expected '=>'") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TOKEN, message.to_string())
                .with_suggestion(span, "=>".to_string(), "Add arrow '=>' for arrow function")
        } else if message.contains("Expected '->'") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TOKEN, message.to_string())
                .with_suggestion(span, "->".to_string(), "Add arrow '->' for function type")
        } else if message.contains("Expected ','") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TOKEN, message.to_string())
                .with_suggestion(span, ",".to_string(), "Add comma ',' to separate items")
        } else if message.contains("Expected '='") && message.contains("variable") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TOKEN, message.to_string())
                .with_suggestion(span, "= value".to_string(), "Add '=' followed by an initial value")
        } else if message.contains("identifier") && message.contains("Expected") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_IDENTIFIER, message.to_string())
                .with_suggestion(span, "name".to_string(), "Provide a valid identifier name")
        } else if message.contains("expression") && message.contains("Expected") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_EXPRESSION, message.to_string())
        } else if message.contains("Invalid number") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TOKEN, message.to_string())
                .with_suggestion(span, "0".to_string(), "Use a valid number literal")
        } else if message.contains("Enum value must be") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TOKEN, message.to_string())
                .with_suggestion(span, "".to_string(), "Enum values must be string or number literals")
        } else if message.contains("Index signature key must be") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TOKEN, message.to_string())
                .with_suggestion(span, "".to_string(), "Index signature keys must be 'string' or 'number'")
        } else if message.contains("Unexpected token in expression") {
            let token_name = self.format_token_name(&self.current().kind);
            let msg = format!("Unexpected {} in expression", token_name);
            Diagnostic::error_with_code(span, error_codes::UNEXPECTED_TOKEN, msg)
                .with_suggestion(span, "".to_string(), "This token cannot appear in an expression context")
        } else if message.contains("Unexpected token in pattern") {
            let token_name = self.format_token_name(&self.current().kind);
            let msg = format!("Unexpected {} in destructuring pattern", token_name);
            Diagnostic::error_with_code(span, error_codes::EXPECTED_PATTERN, msg)
                .with_suggestion(span, "".to_string(), "Expected a valid destructuring pattern (identifier, array, or object)")
        } else if message.contains("Unexpected token in type") {
            let token_name = self.format_token_name(&self.current().kind);
            let msg = format!("Unexpected {} in type annotation", token_name);
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TYPE, msg)
                .with_suggestion(span, "".to_string(), "Expected a valid type (string, number, boolean, etc.)")
        } else if message.contains("Unexpected") || message.contains("unexpected") {
            Diagnostic::error_with_code(span, error_codes::UNEXPECTED_TOKEN, message.to_string())
        } else if message.contains("Expected") || message.contains("expected") {
            Diagnostic::error_with_code(span, error_codes::EXPECTED_TOKEN, message.to_string())
        } else {
            // Generic parser error
            Diagnostic::error_with_code(span, error_codes::UNEXPECTED_TOKEN, message.to_string())
        };

        self.diagnostic_handler.report(diagnostic);
    }

    // Helper to format token names in a user-friendly way
    fn format_token_name(&self, kind: &TokenKind) -> String {
        match kind {
            TokenKind::Identifier(name) => format!("identifier '{}'", name),
            TokenKind::Number(n) => format!("number '{}'", n),
            TokenKind::String(s) => {
                // Truncate long strings
                if s.len() > 20 {
                    format!("string \"{}...\"", &s[..20])
                } else {
                    format!("string \"{}\"", s)
                }
            },
            TokenKind::TemplateString(_) => "template string".to_string(),
            TokenKind::True => "keyword 'true'".to_string(),
            TokenKind::False => "keyword 'false'".to_string(),
            TokenKind::Nil => "keyword 'nil'".to_string(),
            TokenKind::And => "keyword 'and'".to_string(),
            TokenKind::Or => "keyword 'or'".to_string(),
            TokenKind::Not => "keyword 'not'".to_string(),
            TokenKind::If => "keyword 'if'".to_string(),
            TokenKind::Then => "keyword 'then'".to_string(),
            TokenKind::Else => "keyword 'else'".to_string(),
            TokenKind::Elseif => "keyword 'elseif'".to_string(),
            TokenKind::End => "keyword 'end'".to_string(),
            TokenKind::While => "keyword 'while'".to_string(),
            TokenKind::Do => "keyword 'do'".to_string(),
            TokenKind::For => "keyword 'for'".to_string(),
            TokenKind::In => "keyword 'in'".to_string(),
            TokenKind::Repeat => "keyword 'repeat'".to_string(),
            TokenKind::Until => "keyword 'until'".to_string(),
            TokenKind::Function => "keyword 'function'".to_string(),
            TokenKind::Return => "keyword 'return'".to_string(),
            TokenKind::Break => "keyword 'break'".to_string(),
            TokenKind::Continue => "keyword 'continue'".to_string(),
            TokenKind::Local => "keyword 'local'".to_string(),
            TokenKind::Const => "keyword 'const'".to_string(),
            TokenKind::Interface => "keyword 'interface'".to_string(),
            TokenKind::Type => "keyword 'type'".to_string(),
            TokenKind::Enum => "keyword 'enum'".to_string(),
            TokenKind::Class => "keyword 'class'".to_string(),
            TokenKind::Extends => "keyword 'extends'".to_string(),
            TokenKind::Implements => "keyword 'implements'".to_string(),
            TokenKind::Match => "keyword 'match'".to_string(),
            TokenKind::Import => "keyword 'import'".to_string(),
            TokenKind::Export => "keyword 'export'".to_string(),
            TokenKind::Declare => "keyword 'declare'".to_string(),
            TokenKind::Namespace => "keyword 'namespace'".to_string(),
            TokenKind::LeftParen => "'('".to_string(),
            TokenKind::RightParen => "')'".to_string(),
            TokenKind::LeftBrace => "'{'".to_string(),
            TokenKind::RightBrace => "'}'".to_string(),
            TokenKind::LeftBracket => "'['".to_string(),
            TokenKind::RightBracket => "']'".to_string(),
            TokenKind::Comma => "','".to_string(),
            TokenKind::Semicolon => "';'".to_string(),
            TokenKind::Colon => "':'".to_string(),
            TokenKind::ColonColon => "'::'".to_string(),
            TokenKind::Dot => "'.'".to_string(),
            TokenKind::DotDot => "'..'".to_string(),
            TokenKind::DotDotDot => "'...'".to_string(),
            TokenKind::Equal => "'='".to_string(),
            TokenKind::EqualEqual => "'=='".to_string(),
            TokenKind::BangEqual => "'!='".to_string(),
            TokenKind::TildeEqual => "'~='".to_string(),
            TokenKind::LessThan => "'<'".to_string(),
            TokenKind::GreaterThan => "'>'".to_string(),
            TokenKind::LessEqual => "'<='".to_string(),
            TokenKind::GreaterEqual => "'>='".to_string(),
            TokenKind::Plus => "'+'".to_string(),
            TokenKind::Minus => "'-'".to_string(),
            TokenKind::Star => "'*'".to_string(),
            TokenKind::Slash => "'/'".to_string(),
            TokenKind::SlashSlash => "'//'".to_string(),
            TokenKind::Percent => "'%'".to_string(),
            TokenKind::Caret => "'^'".to_string(),
            TokenKind::Hash => "'#'".to_string(),
            TokenKind::Ampersand => "'&'".to_string(),
            TokenKind::Pipe => "'|'".to_string(),
            TokenKind::Tilde => "'~'".to_string(),
            TokenKind::LessLess => "'<<'".to_string(),
            TokenKind::GreaterGreater => "'>>'".to_string(),
            TokenKind::Arrow => "'->'".to_string(),
            TokenKind::FatArrow => "'=>'".to_string(),
            TokenKind::Question => "'?'".to_string(),
            TokenKind::Bang => "'!'".to_string(),
            TokenKind::At => "'@'".to_string(),
            TokenKind::PipeOp => "'|>'".to_string(),
            TokenKind::Eof => "end of file".to_string(),
            TokenKind::Unknown(ch) => format!("unknown character '{}'", ch),
            _ => format!("{:?}", kind),
        }
    }

    // Error recovery: skip to next statement boundary
    // This allows the parser to continue after an error and find more errors in one pass
    fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            // Check if we're at a statement boundary (keyword that starts a new statement)
            match &self.current().kind {
                TokenKind::Function
                | TokenKind::Local
                | TokenKind::Const
                | TokenKind::If
                | TokenKind::While
                | TokenKind::For
                | TokenKind::Repeat
                | TokenKind::Return
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Interface
                | TokenKind::Type
                | TokenKind::Enum
                | TokenKind::Class
                | TokenKind::Import
                | TokenKind::Export
                | TokenKind::Declare
                | TokenKind::Namespace
                | TokenKind::Semicolon => return,
                // Also stop at block terminators
                TokenKind::End | TokenKind::Elseif | TokenKind::Else | TokenKind::Until => return,
                _ => {}
            }

            self.advance();
        }
    }

    // Enhanced consume with better error messages showing what was found
    fn consume_with_context(&mut self, kind: TokenKind, expected_desc: &str) -> Result<&Token, ParserError> {
        if self.check(&kind) {
            return Ok(self.advance());
        }

        let found = self.format_token_name(&self.current().kind);
        let message = format!("Expected {}, but found {}", expected_desc, found);

        Err(ParserError {
            message,
            span: self.current_span(),
        })
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
            DeclareFunction(f) => f.span,
            DeclareNamespace(n) => n.span,
            DeclareType(t) => t.span,
            DeclareInterface(i) => i.span,
            DeclareConst(c) => c.span,
        }
    }
}
