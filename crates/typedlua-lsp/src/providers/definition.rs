use crate::document::Document;
use std::sync::Arc;
use lsp_types::{*, Uri};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::ast::statement::Statement;
use typedlua_core::{Lexer, Parser, Span};

/// Provides go-to-definition functionality
pub struct DefinitionProvider;

impl DefinitionProvider {
    pub fn new() -> Self {
        Self
    }

    /// Provide definition location for symbol at position
    pub fn provide(&self, uri: &Uri, document: &Document, position: Position) -> Option<GotoDefinitionResponse> {
        // Get the word at the current position
        let word = self.get_word_at_position(document, position)?;

        // Parse the document to find declarations
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(&document.text, handler.clone());
        let tokens = lexer.tokenize().ok()?;

        let mut parser = Parser::new(tokens, handler);
        let ast = parser.parse().ok()?;

        // Search for the declaration of this symbol
        let def_span = self.find_declaration(&ast.statements, &word)?;

        // Convert span to LSP Location
        let location = Location {
            uri: uri.clone(),
            range: span_to_range(&def_span),
        };

        Some(GotoDefinitionResponse::Scalar(location))
    }

    /// Find the declaration span for a given symbol name
    fn find_declaration(&self, statements: &[Statement], name: &str) -> Option<Span> {
        use typedlua_core::ast::pattern::Pattern;

        for stmt in statements {
            match stmt {
                Statement::Variable(var_decl) => {
                    // Check if the pattern contains this identifier
                    if let Pattern::Identifier(ident) = &var_decl.pattern {
                        if ident.node == name {
                            return Some(ident.span);
                        }
                    }
                }
                Statement::Function(func_decl) => {
                    if func_decl.name.node == name {
                        return Some(func_decl.name.span);
                    }
                }
                Statement::Class(class_decl) => {
                    if class_decl.name.node == name {
                        return Some(class_decl.name.span);
                    }
                }
                Statement::Interface(interface_decl) => {
                    if interface_decl.name.node == name {
                        return Some(interface_decl.name.span);
                    }
                }
                Statement::TypeAlias(type_decl) => {
                    if type_decl.name.node == name {
                        return Some(type_decl.name.span);
                    }
                }
                Statement::Enum(enum_decl) => {
                    if enum_decl.name.node == name {
                        return Some(enum_decl.name.span);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Get the word at the cursor position
    fn get_word_at_position(&self, document: &Document, position: Position) -> Option<String> {
        let lines: Vec<&str> = document.text.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }

        let line = lines[position.line as usize];
        let char_pos = position.character as usize;
        if char_pos > line.len() {
            return None;
        }

        // Find word boundaries
        let chars: Vec<char> = line.chars().collect();
        if char_pos >= chars.len() {
            return None;
        }

        // Check if we're on a word character
        if !chars[char_pos].is_alphanumeric() && chars[char_pos] != '_' {
            return None;
        }

        // Find start of word
        let mut start = char_pos;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }

        // Find end of word
        let mut end = char_pos;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }

        Some(chars[start..end].iter().collect())
    }
}

/// Convert a Span to an LSP Range
fn span_to_range(span: &Span) -> Range {
    Range {
        start: Position {
            line: (span.line.saturating_sub(1)) as u32,
            character: (span.column.saturating_sub(1)) as u32,
        },
        end: Position {
            line: (span.line.saturating_sub(1)) as u32,
            character: ((span.column + span.len()).saturating_sub(1)) as u32,
        },
    }
}
