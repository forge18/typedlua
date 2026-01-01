use crate::span::Span;
use std::sync::Mutex;

/// Diagnostic severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Info,
}

/// A diagnostic message with location and severity
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            span,
            message: message.into(),
        }
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Warning,
            span,
            message: message.into(),
        }
    }

    pub fn info(span: Span, message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Info,
            span,
            message: message.into(),
        }
    }
}

/// Trait for handling diagnostics
/// This allows for dependency injection and testing with mock handlers
pub trait DiagnosticHandler: Send + Sync {
    fn report(&self, diagnostic: Diagnostic);

    fn error(&self, span: Span, message: &str) {
        self.report(Diagnostic::error(span, message));
    }

    fn warning(&self, span: Span, message: &str) {
        self.report(Diagnostic::warning(span, message));
    }

    fn info(&self, span: Span, message: &str) {
        self.report(Diagnostic::info(span, message));
    }

    fn has_errors(&self) -> bool;
    fn error_count(&self) -> usize;
    fn warning_count(&self) -> usize;
    fn get_diagnostics(&self) -> Vec<Diagnostic>;
}

/// Console-based diagnostic handler that prints to stderr
pub struct ConsoleDiagnosticHandler {
    diagnostics: Mutex<Vec<Diagnostic>>,
    pretty: bool,
}

impl ConsoleDiagnosticHandler {
    pub fn new(pretty: bool) -> Self {
        Self {
            diagnostics: Mutex::new(Vec::new()),
            pretty,
        }
    }
}

impl DiagnosticHandler for ConsoleDiagnosticHandler {
    fn report(&self, diagnostic: Diagnostic) {
        let level_str = match diagnostic.level {
            DiagnosticLevel::Error => "error",
            DiagnosticLevel::Warning => "warning",
            DiagnosticLevel::Info => "info",
        };

        if self.pretty {
            eprintln!(
                "\x1b[1m{}\x1b[0m at {}: {}",
                level_str, diagnostic.span, diagnostic.message
            );
        } else {
            eprintln!("{} at {}: {}", level_str, diagnostic.span, diagnostic.message);
        }

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

/// Collecting diagnostic handler for testing
/// Collects all diagnostics without printing
pub struct CollectingDiagnosticHandler {
    diagnostics: Mutex<Vec<Diagnostic>>,
}

impl CollectingDiagnosticHandler {
    pub fn new() -> Self {
        Self {
            diagnostics: Mutex::new(Vec::new()),
        }
    }
}

impl Default for CollectingDiagnosticHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticHandler for CollectingDiagnosticHandler {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_creation() {
        let span = Span::new(0, 5, 1, 1);
        let diag = Diagnostic::error(span, "Test error");

        assert_eq!(diag.level, DiagnosticLevel::Error);
        assert_eq!(diag.message, "Test error");
    }

    #[test]
    fn test_collecting_handler() {
        let handler = CollectingDiagnosticHandler::new();
        let span = Span::new(0, 5, 1, 1);

        handler.error(span, "Error 1");
        handler.warning(span, "Warning 1");
        handler.error(span, "Error 2");

        assert_eq!(handler.error_count(), 2);
        assert_eq!(handler.warning_count(), 1);
        assert!(handler.has_errors());
        assert_eq!(handler.get_diagnostics().len(), 3);
    }

    #[test]
    fn test_no_errors() {
        let handler = CollectingDiagnosticHandler::new();
        let span = Span::new(0, 5, 1, 1);

        handler.warning(span, "Warning 1");
        handler.info(span, "Info 1");

        assert!(!handler.has_errors());
        assert_eq!(handler.error_count(), 0);
    }
}
