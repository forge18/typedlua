use crate::document::Document;
use std::sync::Arc;
use std::collections::HashMap;
use lsp_types::{*, Uri};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::ast::statement::Statement;
use typedlua_core::ast::expression::{Expression, ExpressionKind};
use typedlua_core::{Lexer, Parser, Span};

/// Provides rename functionality
pub struct RenameProvider;

impl RenameProvider {
    pub fn new() -> Self {
        Self
    }

    /// Prepare a rename operation (validate rename position and provide placeholder)
    pub fn prepare(&self, document: &Document, position: Position) -> Option<PrepareRenameResponse> {
        // Get the word at the current position
        let word = self.get_word_at_position(document, position)?;

        // Get the range of the word
        let range = self.get_word_range(document, position)?;

        
        

        // Return the range and current name as placeholder
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range,
            placeholder: word,
        })
    }

    /// Perform the rename operation
    pub fn rename(
        &self,
        uri: &Uri,
        document: &Document,
        position: Position,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        // Get the word at the current position
        let word = self.get_word_at_position(document, position)?;

        // Validate the new name
        if !self.is_valid_identifier(new_name) {
            return None;
        }

        // Parse the document
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(&document.text, handler.clone());
        let tokens = lexer.tokenize().ok()?;

        let mut parser = Parser::new(tokens, handler);
        let ast = parser.parse().ok()?;

        // Find all occurrences of the symbol (including declaration)
        let mut occurrences = Vec::new();
        self.find_all_occurrences(&ast.statements, &word, &mut occurrences);

        // Find the declaration to include it
        if let Some(decl_span) = self.find_declaration(&ast.statements, &word) {
            occurrences.push(decl_span);
        }

        // Convert spans to text edits
        let edits: Vec<TextEdit> = occurrences
            .into_iter()
            .map(|span| TextEdit {
                range: span_to_range(&span),
                new_text: new_name.to_string(),
            })
            .collect();

        // Create workspace edit
        let mut changes = HashMap::new();
        changes.insert(uri.clone(), edits);

        Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    }

    /// Validate that a name is a valid identifier
    fn is_valid_identifier(&self, name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        // Check first character (must be letter or underscore)
        let mut chars = name.chars();
        if let Some(first) = chars.next() {
            if !first.is_alphabetic() && first != '_' {
                return false;
            }
        } else {
            return false;
        }

        // Check remaining characters (letter, digit, or underscore)
        for ch in chars {
            if !ch.is_alphanumeric() && ch != '_' {
                return false;
            }
        }

        // Check if it's a reserved keyword
        if self.is_keyword(name) {
            return false;
        }

        true
    }

    /// Check if a name is a reserved keyword
    fn is_keyword(&self, name: &str) -> bool {
        matches!(
            name,
            "const"
                | "local"
                | "function"
                | "if"
                | "then"
                | "else"
                | "elseif"
                | "end"
                | "while"
                | "for"
                | "in"
                | "do"
                | "repeat"
                | "until"
                | "return"
                | "break"
                | "continue"
                | "and"
                | "or"
                | "not"
                | "true"
                | "false"
                | "nil"
                | "type"
                | "interface"
                | "enum"
                | "class"
                | "extends"
                | "implements"
                | "public"
                | "private"
                | "protected"
                | "static"
                | "abstract"
                | "readonly"
                | "match"
                | "when"
                | "import"
                | "from"
                | "export"
        )
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

    /// Get the range of the word at the cursor position
    fn get_word_range(&self, document: &Document, position: Position) -> Option<Range> {
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

        Some(Range {
            start: Position {
                line: position.line,
                character: start as u32,
            },
            end: Position {
                line: position.line,
                character: end as u32,
            },
        })
    }

    /// Find the declaration span for a given symbol name
    fn find_declaration(&self, statements: &[Statement], name: &str) -> Option<Span> {
        use typedlua_core::ast::pattern::Pattern;

        for stmt in statements {
            match stmt {
                Statement::Variable(var_decl) => {
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

    /// Find all occurrences of a symbol
    fn find_all_occurrences(&self, statements: &[Statement], name: &str, refs: &mut Vec<Span>) {
        for stmt in statements {
            self.find_occurrences_in_statement(stmt, name, refs);
        }
    }

    fn find_occurrences_in_statement(&self, stmt: &Statement, name: &str, refs: &mut Vec<Span>) {
        match stmt {
            Statement::Expression(expr) => {
                self.find_occurrences_in_expression(expr, name, refs);
            }
            Statement::Variable(var_decl) => {
                self.find_occurrences_in_expression(&var_decl.initializer, name, refs);
            }
            Statement::Function(func_decl) => {
                for stmt in &func_decl.body.statements {
                    self.find_occurrences_in_statement(stmt, name, refs);
                }
            }
            Statement::If(if_stmt) => {
                self.find_occurrences_in_expression(&if_stmt.condition, name, refs);
                self.find_all_occurrences(&if_stmt.then_block.statements, name, refs);
                for else_if in &if_stmt.else_ifs {
                    self.find_occurrences_in_expression(&else_if.condition, name, refs);
                    self.find_all_occurrences(&else_if.block.statements, name, refs);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.find_all_occurrences(&else_block.statements, name, refs);
                }
            }
            Statement::While(while_stmt) => {
                self.find_occurrences_in_expression(&while_stmt.condition, name, refs);
                self.find_all_occurrences(&while_stmt.body.statements, name, refs);
            }
            Statement::Return(ret) => {
                for expr in &ret.values {
                    self.find_occurrences_in_expression(expr, name, refs);
                }
            }
            Statement::Block(block) => {
                self.find_all_occurrences(&block.statements, name, refs);
            }
            _ => {}
        }
    }

    fn find_occurrences_in_expression(&self, expr: &Expression, name: &str, refs: &mut Vec<Span>) {
        match &expr.kind {
            ExpressionKind::Identifier(ident) => {
                if ident == name {
                    refs.push(expr.span);
                }
            }
            ExpressionKind::Binary(_, left, right) => {
                self.find_occurrences_in_expression(left, name, refs);
                self.find_occurrences_in_expression(right, name, refs);
            }
            ExpressionKind::Unary(_, operand) => {
                self.find_occurrences_in_expression(operand, name, refs);
            }
            ExpressionKind::Call(callee, args) => {
                self.find_occurrences_in_expression(callee, name, refs);
                for arg in args {
                    self.find_occurrences_in_expression(&arg.value, name, refs);
                }
            }
            ExpressionKind::Member(object, _) => {
                self.find_occurrences_in_expression(object, name, refs);
            }
            ExpressionKind::Index(object, index) => {
                self.find_occurrences_in_expression(object, name, refs);
                self.find_occurrences_in_expression(index, name, refs);
            }
            ExpressionKind::Assignment(target, _, value) => {
                self.find_occurrences_in_expression(target, name, refs);
                self.find_occurrences_in_expression(value, name, refs);
            }
            ExpressionKind::Conditional(condition, then_expr, else_expr) => {
                self.find_occurrences_in_expression(condition, name, refs);
                self.find_occurrences_in_expression(then_expr, name, refs);
                self.find_occurrences_in_expression(else_expr, name, refs);
            }
            ExpressionKind::Parenthesized(inner) => {
                self.find_occurrences_in_expression(inner, name, refs);
            }
            _ => {}
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
