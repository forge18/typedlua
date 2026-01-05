# TypedLua Language Server Protocol (LSP) Design

**Document Version:** 0.1  
**Last Updated:** 2024-12-31

This document defines the Language Server Protocol implementation for TypedLua, providing full IDE support in editors like VS Code.

---

## Overview

TypedLua's LSP provides:
- **Syntax highlighting** via TextMate grammar
- **Real-time diagnostics** (errors, warnings)
- **IntelliSense** (autocomplete, signature help)
- **Navigation** (go to definition, find references)
- **Refactoring** (rename, code actions)
- **Formatting** and more

**Implementation:** Rust, integrated with the compiler to reuse type checker, AST, and other components.

---

## Architecture

### Component Structure

```
┌─────────────────────────────────────────────────────────┐
│                VS Code Extension                        │
│  - TextMate grammar (syntax highlighting)               │
│  - Extension commands                                   │
│  - LSP client                                           │
└────────────────────┬────────────────────────────────────┘
                     │ JSON-RPC over stdio
                     │
┌────────────────────▼────────────────────────────────────┐
│              TypedLua LSP Server (Rust)                 │
│  ┌──────────────────────────────────────────────────┐   │
│  │           Document Manager                       │   │
│  │  - Track open documents                          │   │
│  │  - Incremental updates                           │   │
│  │  - Version tracking                              │   │
│  └────────────┬─────────────────────────────────────┘   │
│               │                                          │
│  ┌────────────▼─────────────────────────────────────┐   │
│  │         Compiler Integration                     │   │
│  │  - Reuse Lexer, Parser, Type Checker             │   │
│  │  - AST caching                                   │   │
│  │  - Incremental compilation                       │   │
│  └────────────┬─────────────────────────────────────┘   │
│               │                                          │
│  ┌────────────▼─────────────────────────────────────┐   │
│  │         Feature Providers                        │   │
│  │  - Diagnostics, Completion, Hover                │   │
│  │  - Definition, References, Rename                │   │
│  │  - Formatting, Code Actions                      │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### Integration with Compiler

```rust
// lsp/server.rs

use crate::compiler::{Lexer, Parser, TypeChecker};
use crate::di::Container;

pub struct LanguageServer {
    container: Container,
    documents: DocumentManager,
    diagnostics_provider: DiagnosticsProvider,
    completion_provider: CompletionProvider,
    hover_provider: HoverProvider,
    definition_provider: DefinitionProvider,
}
```

---

## LSP Capabilities

When the LSP server initializes, it advertises these capabilities:

```json
{
  "capabilities": {
    "textDocumentSync": {
      "openClose": true,
      "change": 2,
      "save": { "includeText": false }
    },
    "completionProvider": {
      "triggerCharacters": [".", ":", "@", "<", "{", "("],
      "resolveProvider": true
    },
    "hoverProvider": true,
    "signatureHelpProvider": {
      "triggerCharacters": ["(", ","]
    },
    "definitionProvider": true,
    "referencesProvider": true,
    "documentHighlightProvider": true,
    "documentSymbolProvider": true,
    "workspaceSymbolProvider": true,
    "codeActionProvider": {
      "codeActionKinds": ["quickfix", "refactor", "source.organizeImports"]
    },
    "renameProvider": { "prepareProvider": true },
    "documentFormattingProvider": true,
    "documentRangeFormattingProvider": true,
    "foldingRangeProvider": true,
    "selectionRangeProvider": true,
    "semanticTokensProvider": {
      "legend": {
        "tokenTypes": ["class", "interface", "enum", "type", "parameter", "variable", "property", "function", "method", "keyword", "comment", "string", "number"],
        "tokenModifiers": ["declaration", "readonly", "static", "abstract", "deprecated", "modification"]
      },
      "range": true,
      "full": { "delta": true }
    },
    "inlayHintProvider": { "resolveProvider": false }
  }
}
```

---

## Feature Implementation

### 1. Diagnostics (Errors & Warnings)

```rust
// lsp/providers/diagnostics.rs

pub struct DiagnosticsProvider {
    container: Arc<Container>,
}

impl DiagnosticsProvider {
    pub fn provide(&self, document: &Document) -> Vec<Diagnostic> {
        let mut lexer = self.container.create_lexer(&document.text);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(_) => return self.collect_diagnostics(),
        };
        
        let mut parser = self.container.create_parser(tokens);
        let ast = match parser.parse() {
            Ok(a) => a,
            Err(_) => return self.collect_diagnostics(),
        };
        
        let mut type_checker = self.container.create_type_checker();
        let _ = type_checker.check(ast);
        
        self.collect_diagnostics()
    }
}
```

**LSP Diagnostic Format:**
```json
{
  "range": {
    "start": { "line": 4, "character": 10 },
    "end": { "line": 4, "character": 18 }
  },
  "severity": 1,
  "code": "TL2322",
  "source": "typedlua",
  "message": "Type 'string' is not assignable to type 'number'."
}
```

### 2. Completion (IntelliSense)

```rust
// lsp/providers/completion.rs

pub struct CompletionProvider {
    container: Arc<Container>,
}

impl CompletionProvider {
    pub fn provide(&self, document: &Document, position: Position) -> Vec<CompletionItem> {
        let ast = self.parse_to_position(document, position);
        let context = self.get_completion_context(&ast, position);
        
        match context {
            CompletionContext::MemberAccess(base_type) => self.complete_members(base_type),
            CompletionContext::TypeAnnotation => self.complete_types(),
            CompletionContext::Import => self.complete_modules(),
            CompletionContext::Decorator => self.complete_decorators(),
            CompletionContext::Statement => self.complete_keywords_and_identifiers(),
        }
    }
}
```

**Completion Triggers:**
- `.` - Member access
- `:` - Method call
- `@` - Decorators
- `<` - Generic type arguments

### 3. Hover Information

```rust
// lsp/providers/hover.rs

pub struct HoverProvider {
    container: Arc<Container>,
}

impl HoverProvider {
    pub fn provide(&self, document: &Document, position: Position) -> Option<Hover> {
        let ast = self.parse_document(document);
        let node = self.find_node_at_position(&ast, position)?;
        let type_info = self.get_type_info(&node)?;
        
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: self.format_hover_content(&node, &type_info),
            }),
            range: Some(self.get_node_range(&node)),
        })
    }
}
```

### 4. Go to Definition

```rust
// lsp/providers/definition.rs

pub struct DefinitionProvider {
    container: Arc<Container>,
}

impl DefinitionProvider {
    pub fn provide(&self, document: &Document, position: Position) -> Option<Vec<Location>> {
        let ast = self.parse_document(document);
        let identifier = self.find_identifier_at_position(&ast, position)?;
        let symbol = self.resolve_symbol(&identifier)?;
        
        let location = match symbol.kind {
            SymbolKind::Variable => self.get_variable_definition_location(&symbol),
            SymbolKind::Function => self.get_function_definition_location(&symbol),
            SymbolKind::Class => self.get_class_definition_location(&symbol),
            SymbolKind::Import => self.get_imported_symbol_location(&symbol),
            _ => return None,
        };
        
        Some(vec![location])
    }
}
```

### 5. Find References

```rust
// lsp/providers/references.rs

pub struct ReferencesProvider {
    container: Arc<Container>,
}

impl ReferencesProvider {
    pub fn provide(&self, document: &Document, position: Position, include_declaration: bool) -> Vec<Location> {
        let ast = self.parse_document(document);
        let symbol = self.find_symbol_at_position(&ast, position);
        let mut references = Vec::new();
        
        // Search current file
        self.find_references_in_ast(&ast, &symbol, &mut references);
        
        // Search project files
        for file in self.get_project_files() {
            let file_ast = self.parse_file(&file);
            self.find_references_in_ast(&file_ast, &symbol, &mut references);
        }
        
        if !include_declaration {
            references.retain(|loc| !self.is_declaration(loc, &symbol));
        }
        
        references
    }
}
```

### 6. Rename Symbol

```rust
// lsp/providers/rename.rs

pub struct RenameProvider {
    container: Arc<Container>,
}

impl RenameProvider {
    pub fn rename(&self, document: &Document, position: Position, new_name: &str) -> Option<WorkspaceEdit> {
        if !self.is_valid_identifier(new_name) {
            return None;
        }
        
        let symbol = self.find_symbol_at_position(document, position)?;
        let references = self.find_all_references(&symbol);
        
        let mut changes = HashMap::new();
        for reference in references {
            let edits = changes.entry(reference.uri.clone()).or_insert_with(Vec::new);
            edits.push(TextEdit {
                range: reference.range,
                new_text: new_name.to_string(),
            });
        }
        
        Some(WorkspaceEdit { changes: Some(changes), ..Default::default() })
    }
}
```

### 7. Document Formatting

```rust
// lsp/providers/formatting.rs

pub struct FormattingProvider {
    config: FormattingConfig,
}

impl FormattingProvider {
    pub fn format_document(&self, document: &Document) -> Vec<TextEdit> {
        let ast = self.parse_document(document);
        let formatted = self.format_ast(&ast);
        
        vec![TextEdit {
            range: Range {
                start: Position { line: 0, character: 0 },
                end: self.get_document_end(document),
            },
            new_text: formatted,
        }]
    }
}

pub struct FormattingConfig {
    pub indent_size: usize,
    pub use_tabs: bool,
    pub max_line_length: usize,
    pub insert_final_newline: bool,
    pub trim_trailing_whitespace: bool,
}
```

### 8. Code Actions (Quick Fixes)

```rust
// lsp/providers/code_actions.rs

pub struct CodeActionProvider {
    container: Arc<Container>,
}

impl CodeActionProvider {
    pub fn provide(&self, document: &Document, range: Range, context: &CodeActionContext) -> Vec<CodeAction> {
        let mut actions = Vec::new();
        
        // Quick fixes for diagnostics
        for diagnostic in &context.diagnostics {
            if let Some(fix) = self.get_quick_fix(diagnostic, document) {
                actions.push(fix);
            }
        }
        
        // Refactoring actions
        actions.extend(self.get_refactoring_actions(document, range));
        
        // Source actions
        actions.extend(self.get_source_actions(document));
        
        actions
    }
    
    fn get_quick_fix(&self, diagnostic: &Diagnostic, document: &Document) -> Option<CodeAction> {
        match diagnostic.code.as_deref() {
            Some("TL2304") => self.suggest_imports(document, diagnostic),
            Some("TL2322") => self.suggest_type_assertion(document, diagnostic),
            Some("TL2551") => self.suggest_similar_properties(document, diagnostic),
            _ => None
        }
    }
}
```

### 9. Signature Help

```rust
// lsp/providers/signature_help.rs

pub struct SignatureHelpProvider {
    container: Arc<Container>,
}

impl SignatureHelpProvider {
    pub fn provide(&self, document: &Document, position: Position) -> Option<SignatureHelp> {
        let call_info = self.find_enclosing_call(document, position)?;
        let func_type = self.get_function_type(&call_info.function)?;
        let active_parameter = self.get_active_parameter(&call_info, position);
        
        Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: self.format_signature(&func_type),
                documentation: func_type.documentation.clone(),
                parameters: Some(
                    func_type.parameters.iter()
                        .map(|p| ParameterInformation {
                            label: ParameterLabel::Simple(p.name.clone()),
                            documentation: p.documentation.clone(),
                        })
                        .collect()
                ),
                active_parameter: Some(active_parameter),
            }],
            active_signature: Some(0),
            active_parameter: Some(active_parameter),
        })
    }
}
```

### 10. Inlay Hints

```rust
// lsp/providers/inlay_hints.rs

pub struct InlayHintProvider {
    container: Arc<Container>,
}

impl InlayHintProvider {
    pub fn provide(&self, document: &Document, range: Range) -> Vec<InlayHint> {
        let mut hints = Vec::new();
        let ast = self.parse_document(document);
        let mut visitor = InlayHintVisitor::new(range);
        visitor.visit(&ast);
        
        for node in visitor.nodes {
            if let AstNode::Variable(var) = node {
                if var.type_annotation.is_none() {
                    let inferred_type = self.infer_type(var);
                    hints.push(InlayHint {
                        position: self.get_identifier_end(&var.name),
                        label: InlayHintLabel::String(format!(": {}", inferred_type)),
                        kind: Some(InlayHintKind::Type),
                        padding_left: Some(false),
                        padding_right: Some(false),
                    });
                }
            }
        }
        
        hints
    }
}
```

### 11. Folding Ranges

```rust
// lsp/providers/folding_range.rs

pub struct FoldingRangeProvider;

impl FoldingRangeProvider {
    pub fn provide(&self, document: &Document) -> Vec<FoldingRange> {
        let mut ranges = Vec::new();

        // Find foldable regions:
        // - Function bodies (function...end)
        // - Control flow blocks (if...end, while...end, for...end)
        // - Table literals ({...})
        // - Array literals ([...])
        // - Multi-line comments (--[[ ... ]])
        // - Consecutive single-line comments (3+ lines)

        self.find_block_ranges(document, &mut ranges);
        self.find_comment_ranges(document, &mut ranges);

        ranges
    }
}
```

**Use Cases:**
- Collapse function implementations to see overview
- Hide implementation details in large files
- Focus on specific sections of code
- Collapse multi-line comments

### 12. Selection Ranges (Smart Selection)

```rust
// lsp/providers/selection_range.rs

pub struct SelectionRangeProvider;

impl SelectionRangeProvider {
    pub fn provide(&self, document: &Document, positions: Vec<Position>) -> Vec<SelectionRange> {
        // For each position, build a hierarchy of selections from innermost to outermost:
        // 1. Current word/identifier
        // 2. Expression
        // 3. Statement
        // 4. Block
        // 5. Function/class body
        // 6. Entire document

        positions.iter()
            .filter_map(|pos| self.get_selection_range_at_position(document, *pos))
            .collect()
    }
}
```

**Selection Hierarchy Example:**
```lua
local result = calculateSum(getValue(10), 20)
                                    ^cursor
```
1. `10` (literal)
2. `getValue(10)` (call expression)
3. `calculateSum(getValue(10), 20)` (outer call)
4. `result = calculateSum(getValue(10), 20)` (assignment)
5. Entire line

### 13. Semantic Tokens

```rust
// lsp/providers/semantic_tokens.rs

pub struct SemanticTokensProvider {
    token_types: Vec<SemanticTokenType>,
    token_modifiers: Vec<SemanticTokenModifier>,
}

impl SemanticTokensProvider {
    pub fn provide_full(&self, document: &Document) -> SemanticTokens {
        // Parse document and extract semantic information
        // For each token:
        // - Determine type: class, function, variable, parameter, property, etc.
        // - Determine modifiers: declaration, readonly, static, abstract, deprecated
        // - Encode in delta format for efficient transmission

        // Token types: class, interface, enum, type, parameter, variable, property,
        //              function, method, keyword, comment, string, number
        // Modifiers: declaration, readonly, static, abstract, deprecated, modification

        SemanticTokens { result_id: None, data: encoded_tokens }
    }

    pub fn provide_range(&self, document: &Document, range: Range) -> SemanticTokens {
        // Same as provide_full but only for the visible range (optimization)
    }

    pub fn provide_full_delta(&self, document: &Document, previous_result_id: String) -> SemanticTokensDelta {
        // Return only changes from previous state (incremental update)
    }
}
```

**Semantic vs Textual Highlighting:**
- **Textual**: Based on regex patterns, doesn't understand code semantics
- **Semantic**: Based on type information and symbol resolution
  - Distinguish between local variables and constants
  - Highlight deprecated symbols differently
  - Show readonly vs mutable properties
  - Accurate even in complex contexts

**Example:**
```lua
const PI: number = 3.14159  -- PI highlighted as readonly variable
local radius: number = 5     -- radius highlighted as mutable variable

@deprecated
function oldFunction() end   -- oldFunction highlighted with strikethrough
```

---

## Document Management

```rust
// lsp/document_manager.rs

pub struct DocumentManager {
    documents: HashMap<Url, Document>,
}

#[derive(Clone)]
pub struct Document {
    pub uri: Url,
    pub text: String,
    pub version: i32,
    pub language_id: String,
    pub ast: Option<Arc<AST>>,
    pub type_info: Option<Arc<TypeInfo>>,
    pub diagnostics: Vec<Diagnostic>,
}

impl DocumentManager {
    pub fn open(&mut self, params: DidOpenTextDocumentParams) {
        let document = Document {
            uri: params.text_document.uri.clone(),
            text: params.text_document.text,
            version: params.text_document.version,
            language_id: params.text_document.language_id,
            ast: None,
            type_info: None,
            diagnostics: Vec::new(),
        };
        self.documents.insert(params.text_document.uri, document);
    }
    
    pub fn change(&mut self, params: DidChangeTextDocumentParams) {
        if let Some(doc) = self.documents.get_mut(&params.text_document.uri) {
            doc.version = params.text_document.version;
            for change in params.content_changes {
                if let Some(range) = change.range {
                    self.apply_incremental_change(doc, range, &change.text);
                } else {
                    doc.text = change.text;
                }
            }
            doc.ast = None;
            doc.type_info = None;
        }
    }
}
```

---

## VS Code Extension

### package.json

```json
{
  "name": "typedlua",
  "displayName": "TypedLua",
  "description": "TypedLua language support",
  "version": "0.1.0",
  "engines": { "vscode": "^1.75.0" },
  "categories": ["Programming Languages"],
  "activationEvents": ["onLanguage:typedlua"],
  "main": "./out/extension.js",
  "contributes": {
    "languages": [{
      "id": "typedlua",
      "aliases": ["TypedLua", "typedlua"],
      "extensions": [".tl"],
      "configuration": "./language-configuration.json"
    }],
    "grammars": [{
      "language": "typedlua",
      "scopeName": "source.typedlua",
      "path": "./syntaxes/typedlua.tmLanguage.json"
    }],
    "configuration": {
      "title": "TypedLua",
      "properties": {
        "typedlua.trace.server": {
          "type": "string",
          "enum": ["off", "messages", "verbose"],
          "default": "off"
        },
        "typedlua.compiler.path": {
          "type": "string",
          "default": "tl"
        }
      }
    }
  }
}
```

### TextMate Grammar

```json
{
  "name": "TypedLua",
  "scopeName": "source.typedlua",
  "patterns": [
    {"include": "#comments"},
    {"include": "#keywords"},
    {"include": "#types"},
    {"include": "#decorators"},
    {"include": "#strings"},
    {"include": "#numbers"}
  ],
  "repository": {
    "keywords": {
      "patterns": [{
        "name": "keyword.control.typedlua",
        "match": "\\b(if|then|else|while|for|return|break|continue|match|when)\\b"
      }]
    },
    "types": {
      "patterns": [{
        "name": "support.type.primitive.typedlua",
        "match": "\\b(nil|boolean|number|string|unknown|never)\\b"
      }]
    },
    "decorators": {
      "name": "entity.name.function.decorator.typedlua",
      "match": "@[A-Za-z_][A-Za-z0-9_]*"
    }
  }
}
```

### Extension Client

```typescript
// src/extension.ts

import * as vscode from 'vscode';
import { LanguageClient, ServerOptions, LanguageClientOptions } from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
  const config = vscode.workspace.getConfiguration('typedlua');
  const compilerPath = config.get<string>('compiler.path', 'tl');
  
  const serverOptions: ServerOptions = {
    command: compilerPath,
    args: ['--lsp'],
    transport: TransportKind.stdio
  };
  
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'typedlua' }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher('**/*.tl')
    }
  };
  
  client = new LanguageClient('typedlua', 'TypedLua Language Server', serverOptions, clientOptions);
  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) return undefined;
  return client.stop();
}
```

---

## Performance Optimization

### Incremental Parsing

```rust
pub struct IncrementalParser {
    previous_ast: Option<Arc<AST>>,
    previous_tokens: Option<Vec<Token>>,
}

impl IncrementalParser {
    pub fn parse_incremental(&mut self, text: &str, changes: &[TextDocumentContentChangeEvent]) -> AST {
        if self.can_use_incremental(changes) {
            self.apply_incremental_changes(changes)
        } else {
            self.parse_full(text)
        }
    }
}
```

### Caching Strategy

```rust
pub struct AnalysisCache {
    asts: HashMap<(Url, i32), Arc<AST>>,
    type_info: HashMap<(Url, i32), Arc<TypeInfo>>,
    symbols: HashMap<Url, Arc<SymbolTable>>,
}
```

---

**Document Version:** 0.1  
**Last Updated:** 2024-12-31
