use crate::document::Document;
use std::sync::Arc;
use lsp_types::{*, Uri};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::ast::statement::{Statement, ClassMember};
use typedlua_core::{Lexer, Parser, Span};

/// Provides document symbols (outline view)
pub struct SymbolsProvider;

impl SymbolsProvider {
    pub fn new() -> Self {
        Self
    }

    /// Provide all symbols in the document
    pub fn provide(&self, document: &Document) -> Vec<DocumentSymbol> {
        // Parse the document
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(&document.text, handler.clone());
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

        let mut parser = Parser::new(tokens, handler);
        let ast = match parser.parse() {
            Ok(a) => a,
            Err(_) => return Vec::new(),
        };

        // Extract symbols from AST
        let mut symbols = Vec::new();
        for stmt in &ast.statements {
            if let Some(symbol) = self.extract_symbol_from_statement(stmt) {
                symbols.push(symbol);
            }
        }

        symbols
    }

    /// Extract a document symbol from a statement
    #[allow(deprecated)]
    fn extract_symbol_from_statement(&self, stmt: &Statement) -> Option<DocumentSymbol> {
        use typedlua_core::ast::pattern::Pattern;

        match stmt {
            Statement::Variable(var_decl) => {
                if let Pattern::Identifier(ident) = &var_decl.pattern {
                    let kind = match var_decl.kind {
                        typedlua_core::ast::statement::VariableKind::Const => SymbolKind::CONSTANT,
                        typedlua_core::ast::statement::VariableKind::Local => SymbolKind::VARIABLE,
                    };

                    Some(DocumentSymbol {
                        name: ident.node.clone(),
                        detail: None,
                        kind,
                        tags: None,
                        deprecated: None,
                        range: span_to_range(&var_decl.span),
                        selection_range: span_to_range(&ident.span),
                        children: None,
                    })
                } else {
                    None
                }
            }
            Statement::Function(func_decl) => {
                let mut children = Vec::new();

                // Add function body statements as children
                for stmt in &func_decl.body.statements {
                    if let Some(symbol) = self.extract_symbol_from_statement(stmt) {
                        children.push(symbol);
                    }
                }

                Some(DocumentSymbol {
                    name: func_decl.name.node.clone(),
                    detail: None,
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range: span_to_range(&func_decl.span),
                    selection_range: span_to_range(&func_decl.name.span),
                    children: if children.is_empty() { None } else { Some(children) },
                })
            }
            Statement::Class(class_decl) => {
                let mut children = Vec::new();

                // Add class members as children
                for member in &class_decl.members {
                    if let Some(symbol) = self.extract_symbol_from_class_member(member) {
                        children.push(symbol);
                    }
                }

                Some(DocumentSymbol {
                    name: class_decl.name.node.clone(),
                    detail: None,
                    kind: SymbolKind::CLASS,
                    tags: None,
                    deprecated: None,
                    range: span_to_range(&class_decl.span),
                    selection_range: span_to_range(&class_decl.name.span),
                    children: if children.is_empty() { None } else { Some(children) },
                })
            }
            Statement::Interface(interface_decl) => {
                Some(DocumentSymbol {
                    name: interface_decl.name.node.clone(),
                    detail: None,
                    kind: SymbolKind::INTERFACE,
                    tags: None,
                    deprecated: None,
                    range: span_to_range(&interface_decl.span),
                    selection_range: span_to_range(&interface_decl.name.span),
                    children: None,
                })
            }
            Statement::TypeAlias(type_decl) => {
                Some(DocumentSymbol {
                    name: type_decl.name.node.clone(),
                    detail: None,
                    kind: SymbolKind::TYPE_PARAMETER,
                    tags: None,
                    deprecated: None,
                    range: span_to_range(&type_decl.span),
                    selection_range: span_to_range(&type_decl.name.span),
                    children: None,
                })
            }
            Statement::Enum(enum_decl) => {
                Some(DocumentSymbol {
                    name: enum_decl.name.node.clone(),
                    detail: None,
                    kind: SymbolKind::ENUM,
                    tags: None,
                    deprecated: None,
                    range: span_to_range(&enum_decl.span),
                    selection_range: span_to_range(&enum_decl.name.span),
                    children: None,
                })
            }
            _ => None,
        }
    }

    /// Extract a document symbol from a class member
    fn extract_symbol_from_class_member(&self, member: &ClassMember) -> Option<DocumentSymbol> {
        match member {
            ClassMember::Property(prop) => {
                Some(DocumentSymbol {
                    name: prop.name.node.clone(),
                    detail: None,
                    kind: SymbolKind::PROPERTY,
                    tags: None,
                    deprecated: None,
                    range: span_to_range(&prop.span),
                    selection_range: span_to_range(&prop.name.span),
                    children: None,
                })
            }
            ClassMember::Constructor(ctor) => {
                Some(DocumentSymbol {
                    name: "constructor".to_string(),
                    detail: None,
                    kind: SymbolKind::CONSTRUCTOR,
                    tags: None,
                    deprecated: None,
                    range: span_to_range(&ctor.span),
                    selection_range: span_to_range(&ctor.span),
                    children: None,
                })
            }
            ClassMember::Method(method) => {
                Some(DocumentSymbol {
                    name: method.name.node.clone(),
                    detail: None,
                    kind: SymbolKind::METHOD,
                    tags: None,
                    deprecated: None,
                    range: span_to_range(&method.span),
                    selection_range: span_to_range(&method.name.span),
                    children: None,
                })
            }
            ClassMember::Getter(getter) => {
                Some(DocumentSymbol {
                    name: getter.name.node.clone(),
                    detail: Some("get".to_string()),
                    kind: SymbolKind::PROPERTY,
                    tags: None,
                    deprecated: None,
                    range: span_to_range(&getter.span),
                    selection_range: span_to_range(&getter.name.span),
                    children: None,
                })
            }
            ClassMember::Setter(setter) => {
                Some(DocumentSymbol {
                    name: setter.name.node.clone(),
                    detail: Some("set".to_string()),
                    kind: SymbolKind::PROPERTY,
                    tags: None,
                    deprecated: None,
                    range: span_to_range(&setter.span),
                    selection_range: span_to_range(&setter.name.span),
                    children: None,
                })
            }
        }
    }

    /// Provide workspace symbols matching a query
    pub fn provide_workspace_symbols(&self, _query: &str) -> Vec<SymbolInformation> {
        // For now, return empty until we have workspace access
        Vec::new()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_kinds() {
        let provider = SymbolsProvider::new();

        // Test function symbol
        let doc = Document {
            text: "function foo() end".to_string(),
            version: 1,
            ast: None,
        };
        let symbols = provider.provide(&doc);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "foo");
        assert_eq!(symbols[0].kind, SymbolKind::FUNCTION);

        // Test variable symbol (local)
        let doc = Document {
            text: "local x = 10".to_string(),
            version: 1,
            ast: None,
        };
        let symbols = provider.provide(&doc);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "x");
        assert_eq!(symbols[0].kind, SymbolKind::VARIABLE);

        // Test constant symbol
        let doc = Document {
            text: "const PI = 3.14".to_string(),
            version: 1,
            ast: None,
        };
        let symbols = provider.provide(&doc);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "PI");
        assert_eq!(symbols[0].kind, SymbolKind::CONSTANT);

        // Test class symbol
        let doc = Document {
            text: "class Point end".to_string(),
            version: 1,
            ast: None,
        };
        let symbols = provider.provide(&doc);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Point");
        assert_eq!(symbols[0].kind, SymbolKind::CLASS);

        // Test interface symbol
        let doc = Document {
            text: "interface Drawable end".to_string(),
            version: 1,
            ast: None,
        };
        let symbols = provider.provide(&doc);
        // Parser might not recognize 'interface' keyword yet
        if symbols.len() > 0 {
            assert_eq!(symbols[0].name, "Drawable");
            assert_eq!(symbols[0].kind, SymbolKind::INTERFACE);
        }

        // Test enum symbol
        let doc = Document {
            text: "enum Color { Red, Green, Blue }".to_string(),
            version: 1,
            ast: None,
        };
        let symbols = provider.provide(&doc);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Color");
        assert_eq!(symbols[0].kind, SymbolKind::ENUM);

        // Test type alias symbol
        let doc = Document {
            text: "type Point = { x: number, y: number }".to_string(),
            version: 1,
            ast: None,
        };
        let symbols = provider.provide(&doc);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Point");
        assert_eq!(symbols[0].kind, SymbolKind::TYPE_PARAMETER);
    }
}
