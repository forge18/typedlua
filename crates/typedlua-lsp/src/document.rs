use std::collections::HashMap;
use std::sync::Arc;
use lsp_types::{*, Uri};
use typedlua_core::ast::Program;

/// Manages open documents and their cached analysis results
#[derive(Debug, Default)]
pub struct DocumentManager {
    documents: HashMap<Uri, Document>,
}

/// Represents a single document with cached analysis
#[derive(Debug, Clone)]
pub struct Document {
    pub text: String,
    pub version: i32,
    /// Cached parsed AST (invalidated on change)
    pub ast: Option<Arc<Program>>,
}

impl DocumentManager {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    /// Open a new document
    pub fn open(&mut self, params: DidOpenTextDocumentParams) {
        let document = Document {
            text: params.text_document.text,
            version: params.text_document.version,
            ast: None,
        };
        self.documents.insert(params.text_document.uri, document);
    }

    /// Handle document changes (incremental)
    pub fn change(&mut self, params: DidChangeTextDocumentParams) {
        if let Some(doc) = self.documents.get_mut(&params.text_document.uri) {
            doc.version = params.text_document.version;

            for change in params.content_changes {
                if let Some(range) = change.range {
                    // Apply incremental change
                    let start_offset = Self::position_to_offset(&doc.text, range.start);
                    let end_offset = Self::position_to_offset(&doc.text, range.end);

                    let mut new_text = String::new();
                    new_text.push_str(&doc.text[..start_offset]);
                    new_text.push_str(&change.text);
                    new_text.push_str(&doc.text[end_offset..]);

                    doc.text = new_text;
                } else {
                    // Full document sync
                    doc.text = change.text;
                }
            }

            // Invalidate cached AST on change
            doc.ast = None;
        }
    }

    /// Handle document save
    pub fn save(&mut self, _params: DidSaveTextDocumentParams) {
        // Nothing special to do on save for now
        // The document is already up to date from didChange events
    }

    /// Close a document
    pub fn close(&mut self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }

    /// Get a document by URI
    pub fn get(&self, uri: &Uri) -> Option<&Document> {
        self.documents.get(uri)
    }

    /// Convert LSP Position to byte offset in text
    fn position_to_offset(text: &str, position: Position) -> usize {
        let mut offset = 0;
        let mut current_line = 0;
        let mut current_char = 0;

        for ch in text.chars() {
            if current_line == position.line && current_char == position.character {
                return offset;
            }

            if ch == '\n' {
                current_line += 1;
                current_char = 0;
            } else {
                current_char += 1;
            }

            offset += ch.len_utf8();
        }

        offset
    }

    /// Convert byte offset to LSP Position
    #[allow(dead_code)]
    pub fn offset_to_position(text: &str, offset: usize) -> Position {
        let mut current_line = 0;
        let mut current_char = 0;
        let mut current_offset = 0;

        for ch in text.chars() {
            if current_offset >= offset {
                break;
            }

            if ch == '\n' {
                current_line += 1;
                current_char = 0;
            } else {
                current_char += 1;
            }

            current_offset += ch.len_utf8();
        }

        Position {
            line: current_line,
            character: current_char,
        }
    }
}
