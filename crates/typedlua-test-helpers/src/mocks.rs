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
