//! Mock implementations for testing

use std::sync::Arc;
use typedlua_core::diagnostics::{Diagnostic, DiagnosticHandler, DiagnosticLevel};

/// A mock diagnostic handler that collects diagnostics
#[derive(Debug, Default)]
pub struct MockDiagnosticHandler {
    diagnostics: std::sync::Mutex<Vec<Diagnostic>>,
}

impl MockDiagnosticHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl DiagnosticHandler for MockDiagnosticHandler {
    fn report(&self, diagnostic: Diagnostic) {
        self.diagnostics.lock().unwrap().push(diagnostic);
    }

    fn has_errors(&self) -> bool {
        self.diagnostics
            .lock()
            .unwrap()
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error)
    }

    fn error_count(&self) -> usize {
        self.diagnostics
            .lock()
            .unwrap()
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Error)
            .count()
    }

    fn warning_count(&self) -> usize {
        self.diagnostics
            .lock()
            .unwrap()
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Warning)
            .count()
    }

    fn get_diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics.lock().unwrap().clone()
    }
}

/// Simple test document for LSP testing
#[derive(Debug, Clone)]
pub struct TestDocument {
    pub text: String,
    pub version: i32,
}

impl TestDocument {
    pub fn new(text: impl Into<String>, version: i32) -> Self {
        Self {
            text: text.into(),
            version,
        }
    }

    pub fn simple_lua() -> Self {
        Self::new("local x = 10", 1)
    }

    pub fn with_function() -> Self {
        Self::new(
            r#"function add(a: number, b: number): number
    return a + b
end"#,
            1,
        )
    }
}

#[cfg(feature = "lsp")]
mod lsp_mocks {
    use super::*;
    use lsp_types::{
        CodeAction, CodeActionContext, CodeActionResponse, CompletionItem, Diagnostic,
        DocumentSymbolResponse, FoldingRange, FormattingOptions, GotoDefinitionResponse, Hover,
        InlayHint, Location, Position, Range, SelectionRange, SemanticToken, SemanticTokensDelta,
        SignatureHelp, Uri, WorkspaceEdit,
    };
    use typedlua_lsp::document::Document;
    use typedlua_lsp::traits::*;

    #[derive(Debug, Default)]
    pub struct MockCompletionProvider;

    impl MockCompletionProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl CompletionProviderTrait for MockCompletionProvider {
        fn provide(&self, _document: &Document, _position: Position) -> Vec<CompletionItem> {
            vec![]
        }

        fn resolve(&self, item: CompletionItem) -> CompletionItem {
            item
        }
    }

    #[derive(Debug, Default)]
    pub struct MockHoverProvider;

    impl MockHoverProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl HoverProviderTrait for MockHoverProvider {
        fn provide(&self, _document: &Document, _position: Position) -> Option<Hover> {
            None
        }
    }

    #[derive(Debug, Default)]
    pub struct MockDefinitionProvider;

    impl MockDefinitionProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl DefinitionProviderTrait for MockDefinitionProvider {
        fn provide(
            &self,
            _uri: &Uri,
            _document: &Document,
            _position: Position,
        ) -> Option<GotoDefinitionResponse> {
            None
        }
    }

    #[derive(Debug, Default)]
    pub struct MockReferencesProvider;

    impl MockReferencesProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl ReferencesProviderTrait for MockReferencesProvider {
        fn provide(
            &self,
            _uri: &Uri,
            _document: &Document,
            _position: Position,
            _include_declaration: bool,
        ) -> Vec<Location> {
            vec![]
        }
    }

    #[derive(Debug, Default)]
    pub struct MockRenameProvider;

    impl MockRenameProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl RenameProviderTrait for MockRenameProvider {
        fn prepare(&self, _document: &Document, _position: Position) -> Option<Range> {
            None
        }

        fn rename(
            &self,
            _uri: &Uri,
            _document: &Document,
            _position: Position,
            _new_name: &str,
        ) -> Option<WorkspaceEdit> {
            None
        }
    }

    #[derive(Debug, Default)]
    pub struct MockSymbolsProvider;

    impl MockSymbolsProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl SymbolsProviderTrait for MockSymbolsProvider {
        fn provide(&self, _document: &Document) -> DocumentSymbolResponse {
            DocumentSymbolResponse::Nested(vec![])
        }
    }

    #[derive(Debug, Default)]
    pub struct MockFormattingProvider;

    impl MockFormattingProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl FormattingProviderTrait for MockFormattingProvider {
        fn format_document(
            &self,
            _document: &Document,
            _options: FormattingOptions,
        ) -> Option<String> {
            None
        }

        fn format_range(
            &self,
            _document: &Document,
            _range: Range,
            _options: FormattingOptions,
        ) -> Option<String> {
            None
        }

        fn format_on_type(
            &self,
            _document: &Document,
            _position: Position,
            _ch: &str,
            _options: FormattingOptions,
        ) -> Option<String> {
            None
        }
    }

    #[derive(Debug, Default)]
    pub struct MockCodeActionsProvider;

    impl MockCodeActionsProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl CodeActionsProviderTrait for MockCodeActionsProvider {
        fn provide(
            &self,
            _uri: &Uri,
            _document: &Document,
            _range: Range,
            _context: CodeActionContext,
        ) -> Option<CodeActionResponse> {
            None
        }

        fn resolve(&self, _item: CodeAction) -> Option<CodeAction> {
            None
        }
    }

    #[derive(Debug, Default)]
    pub struct MockSignatureHelpProvider;

    impl MockSignatureHelpProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl SignatureHelpProviderTrait for MockSignatureHelpProvider {
        fn provide(&self, _document: &Document, _position: Position) -> Option<SignatureHelp> {
            None
        }
    }

    #[derive(Debug, Default)]
    pub struct MockInlayHintsProvider;

    impl MockInlayHintsProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl InlayHintsProviderTrait for MockInlayHintsProvider {
        fn provide(&self, _document: &Document, _range: Range) -> Option<Vec<InlayHint>> {
            None
        }

        fn resolve(&self, _item: InlayHint) -> Option<InlayHint> {
            None
        }
    }

    #[derive(Debug, Default)]
    pub struct MockSelectionRangeProvider;

    impl MockSelectionRangeProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl SelectionRangeProviderTrait for MockSelectionRangeProvider {
        fn provide(&self, _document: &Document, _positions: Vec<Position>) -> Vec<SelectionRange> {
            vec![]
        }
    }

    #[derive(Debug, Default)]
    pub struct MockFoldingRangeProvider;

    impl MockFoldingRangeProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl FoldingRangeProviderTrait for MockFoldingRangeProvider {
        fn provide(&self, _document: &Document) -> Vec<FoldingRange> {
            vec![]
        }
    }

    #[derive(Debug, Default)]
    pub struct MockSemanticTokensProvider;

    impl MockSemanticTokensProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl SemanticTokensProviderTrait for MockSemanticTokensProvider {
        fn provide_full(&self, _document: &Document) -> Option<Vec<SemanticToken>> {
            None
        }

        fn provide_range(&self, _document: &Document, _range: Range) -> Option<Vec<SemanticToken>> {
            None
        }

        fn provide_full_delta(
            &self,
            _document: &Document,
            _previous_result_id: Option<String>,
        ) -> Option<SemanticTokensDelta> {
            None
        }
    }

    #[derive(Debug, Default)]
    pub struct MockDiagnosticsProvider;

    impl MockDiagnosticsProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl DiagnosticsProviderTrait for MockDiagnosticsProvider {
        fn provide(&self, _document: &Document) -> Vec<Diagnostic> {
            vec![]
        }
    }

    pub fn create_mock_providers() -> (
        MockCompletionProvider,
        MockHoverProvider,
        MockDefinitionProvider,
        MockReferencesProvider,
        MockRenameProvider,
        MockSymbolsProvider,
        MockFormattingProvider,
        MockCodeActionsProvider,
        MockSignatureHelpProvider,
        MockInlayHintsProvider,
        MockSelectionRangeProvider,
        MockFoldingRangeProvider,
        MockSemanticTokensProvider,
        MockDiagnosticsProvider,
    ) {
        (
            MockCompletionProvider::new(),
            MockHoverProvider::new(),
            MockDefinitionProvider::new(),
            MockReferencesProvider::new(),
            MockRenameProvider::new(),
            MockSymbolsProvider::new(),
            MockFormattingProvider::new(),
            MockCodeActionsProvider::new(),
            MockSignatureHelpProvider::new(),
            MockInlayHintsProvider::new(),
            MockSelectionRangeProvider::new(),
            MockFoldingRangeProvider::new(),
            MockSemanticTokensProvider::new(),
            MockDiagnosticsProvider::new(),
        )
    }
}
