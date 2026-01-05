use crate::document::Document;
use std::sync::Arc;
use lsp_types::{*, Uri};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::ast::{statement::Statement, expression::{Expression, ExpressionKind}};
use typedlua_core::{Lexer, Parser, Span};

/// Provides find-references functionality
pub struct ReferencesProvider;

impl ReferencesProvider {
    pub fn new() -> Self {
        Self
    }

    /// Find all references to the symbol at the given position
    pub fn provide(
        &self,
        uri: &Uri,
        document: &Document,
        position: Position,
        include_declaration: bool,
    ) -> Option<Vec<Location>> {
        // Get the word at the current position
        let word = self.get_word_at_position(document, position)?;

        // Parse the document
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(&document.text, handler.clone());
        let tokens = lexer.tokenize().ok()?;

        let mut parser = Parser::new(tokens, handler);
        let ast = parser.parse().ok()?;

        // Find all references to this symbol
        let mut references = Vec::new();
        self.find_references_in_statements(&ast.statements, &word, &mut references);

        // Optionally include the declaration
        if include_declaration {
            if let Some(decl_span) = self.find_declaration(&ast.statements, &word) {
                references.push(decl_span);
            }
        }

        // Convert spans to locations
        let locations: Vec<Location> = references
            .into_iter()
            .map(|span| Location {
                uri: uri.clone(),
                range: span_to_range(&span),
            })
            .collect();

        Some(locations)
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

    /// Find all references to a symbol by traversing the AST
    fn find_references_in_statements(&self, statements: &[Statement], name: &str, refs: &mut Vec<Span>) {
        for stmt in statements {
            self.find_references_in_statement(stmt, name, refs);
        }
    }

    fn find_references_in_statement(&self, stmt: &Statement, name: &str, refs: &mut Vec<Span>) {
        match stmt {
            Statement::Expression(expr) => {
                self.find_references_in_expression(expr, name, refs);
            }
            Statement::Variable(var_decl) => {
                self.find_references_in_expression(&var_decl.initializer, name, refs);
            }
            Statement::Function(func_decl) => {
                for stmt in &func_decl.body.statements {
                    self.find_references_in_statement(stmt, name, refs);
                }
            }
            Statement::If(if_stmt) => {
                self.find_references_in_expression(&if_stmt.condition, name, refs);
                self.find_references_in_statements(&if_stmt.then_block.statements, name, refs);
                for else_if in &if_stmt.else_ifs {
                    self.find_references_in_expression(&else_if.condition, name, refs);
                    self.find_references_in_statements(&else_if.block.statements, name, refs);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.find_references_in_statements(&else_block.statements, name, refs);
                }
            }
            Statement::While(while_stmt) => {
                self.find_references_in_expression(&while_stmt.condition, name, refs);
                self.find_references_in_statements(&while_stmt.body.statements, name, refs);
            }
            Statement::Return(ret) => {
                for expr in &ret.values {
                    self.find_references_in_expression(expr, name, refs);
                }
            }
            Statement::Block(block) => {
                self.find_references_in_statements(&block.statements, name, refs);
            }
            _ => {}
        }
    }

    fn find_references_in_expression(&self, expr: &Expression, name: &str, refs: &mut Vec<Span>) {
        match &expr.kind {
            ExpressionKind::Identifier(ident) => {
                if ident == name {
                    refs.push(expr.span);
                }
            }
            ExpressionKind::Binary(_, left, right) => {
                self.find_references_in_expression(left, name, refs);
                self.find_references_in_expression(right, name, refs);
            }
            ExpressionKind::Unary(_, operand) => {
                self.find_references_in_expression(operand, name, refs);
            }
            ExpressionKind::Call(callee, args) => {
                self.find_references_in_expression(callee, name, refs);
                for arg in args {
                    self.find_references_in_expression(&arg.value, name, refs);
                }
            }
            ExpressionKind::Member(object, _) => {
                self.find_references_in_expression(object, name, refs);
            }
            ExpressionKind::Index(object, index) => {
                self.find_references_in_expression(object, name, refs);
                self.find_references_in_expression(index, name, refs);
            }
            ExpressionKind::Assignment(target, _, value) => {
                self.find_references_in_expression(target, name, refs);
                self.find_references_in_expression(value, name, refs);
            }
            ExpressionKind::Array(elements) => {
                for elem in elements {
                    match elem {
                        typedlua_core::ast::expression::ArrayElement::Expression(e) => {
                            self.find_references_in_expression(e, name, refs);
                        }
                        typedlua_core::ast::expression::ArrayElement::Spread(e) => {
                            self.find_references_in_expression(e, name, refs);
                        }
                    }
                }
            }
            ExpressionKind::Object(properties) => {
                use typedlua_core::ast::expression::ObjectProperty;
                for prop in properties {
                    match prop {
                        ObjectProperty::Property { value, .. } => {
                            self.find_references_in_expression(value, name, refs);
                        }
                        ObjectProperty::Computed { key, value, .. } => {
                            self.find_references_in_expression(key, name, refs);
                            self.find_references_in_expression(value, name, refs);
                        }
                        ObjectProperty::Spread { value, .. } => {
                            self.find_references_in_expression(value, name, refs);
                        }
                    }
                }
            }
            ExpressionKind::Conditional(condition, then_expr, else_expr) => {
                self.find_references_in_expression(condition, name, refs);
                self.find_references_in_expression(then_expr, name, refs);
                self.find_references_in_expression(else_expr, name, refs);
            }
            ExpressionKind::Parenthesized(inner) => {
                self.find_references_in_expression(inner, name, refs);
            }
            _ => {}
        }
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

    /// Provide document highlights for the symbol at the given position
    pub fn provide_highlights(
        &self,
        document: &Document,
        position: Position,
    ) -> Option<Vec<DocumentHighlight>> {
        // Get the word at the current position
        let word = self.get_word_at_position(document, position)?;

        // Parse the document
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(&document.text, handler.clone());
        let tokens = lexer.tokenize().ok()?;

        let mut parser = Parser::new(tokens, handler);
        let ast = parser.parse().ok()?;

        // Find all references to this symbol (including declaration)
        let mut references = Vec::new();
        self.find_references_in_statements(&ast.statements, &word, &mut references);

        // Include the declaration
        if let Some(decl_span) = self.find_declaration(&ast.statements, &word) {
            references.push(decl_span);
        }

        // Convert spans to document highlights
        let highlights: Vec<DocumentHighlight> = references
            .into_iter()
            .map(|span| DocumentHighlight {
                range: span_to_range(&span),
                kind: Some(DocumentHighlightKind::TEXT),
            })
            .collect();

        if highlights.is_empty() {
            None
        } else {
            Some(highlights)
        }
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
