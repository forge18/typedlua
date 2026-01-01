use crate::config::CompilerConfig;
use crate::diagnostics::{ConsoleDiagnosticHandler, DiagnosticHandler};
use crate::fs::{FileSystem, RealFileSystem};
use std::sync::Arc;

/// Dependency injection container
/// Manages all shared dependencies and creates instances with proper wiring
pub struct Container {
    config: Arc<CompilerConfig>,
    diagnostic_handler: Arc<dyn DiagnosticHandler>,
    file_system: Arc<dyn FileSystem>,
}

impl Container {
    /// Create a new container with production dependencies
    pub fn new(config: CompilerConfig) -> Self {
        let config = Arc::new(config);

        let diagnostic_handler = Arc::new(ConsoleDiagnosticHandler::new(
            config.compiler_options.pretty,
        ));

        let file_system = Arc::new(RealFileSystem::new());

        Container {
            config,
            diagnostic_handler,
            file_system,
        }
    }

    /// Create a container with custom dependencies (for testing)
    pub fn with_dependencies(
        config: CompilerConfig,
        diagnostic_handler: Arc<dyn DiagnosticHandler>,
        file_system: Arc<dyn FileSystem>,
    ) -> Self {
        let config = Arc::new(config);

        Container {
            config,
            diagnostic_handler,
            file_system,
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &Arc<CompilerConfig> {
        &self.config
    }

    /// Get the diagnostic handler
    pub fn diagnostic_handler(&self) -> &Arc<dyn DiagnosticHandler> {
        &self.diagnostic_handler
    }

    /// Get the file system
    pub fn file_system(&self) -> &Arc<dyn FileSystem> {
        &self.file_system
    }

    /// Check if any errors have been reported
    pub fn has_errors(&self) -> bool {
        self.diagnostic_handler.has_errors()
    }

    /// Get the error count
    pub fn error_count(&self) -> usize {
        self.diagnostic_handler.error_count()
    }

    /// Get the warning count
    pub fn warning_count(&self) -> usize {
        self.diagnostic_handler.warning_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::CollectingDiagnosticHandler;
    use crate::fs::MockFileSystem;
    use crate::span::Span;

    #[test]
    fn test_container_creation() {
        let config = CompilerConfig::default();
        let container = Container::new(config);

        assert_eq!(container.error_count(), 0);
        assert!(!container.has_errors());
    }

    #[test]
    fn test_container_with_mock_dependencies() {
        let config = CompilerConfig::default();
        let diagnostics = Arc::new(CollectingDiagnosticHandler::new());
        let fs = Arc::new(MockFileSystem::new());

        let container = Container::with_dependencies(config, diagnostics.clone(), fs);

        // Report an error
        container
            .diagnostic_handler()
            .error(Span::dummy(), "Test error");

        assert!(container.has_errors());
        assert_eq!(container.error_count(), 1);
    }

    #[test]
    fn test_container_config_access() {
        let mut config = CompilerConfig::default();
        config.compiler_options.enable_oop = false;

        let container = Container::new(config);

        assert!(!container.config().compiler_options.enable_oop);
    }
}
