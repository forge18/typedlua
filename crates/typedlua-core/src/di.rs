use super::codegen::CodeGenerator;
use super::config::{CompilerConfig, OptimizationLevel};
use super::diagnostics::{ConsoleDiagnosticHandler, DiagnosticHandler};
use super::fs::{FileSystem, RealFileSystem};
use super::optimizer::Optimizer;
use std::rc::Rc;
use std::sync::Arc;
use typedlua_parser::diagnostics::CollectingDiagnosticHandler as ParserCollectingHandler;
use typedlua_parser::string_interner::StringInterner;
use typedlua_parser::{Lexer, Parser};
use typedlua_typechecker::TypeChecker;

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

    /// Compile source code using the container's dependencies (without stdlib)
    ///
    /// # Arguments
    /// * `source` - The TypedLua source code to compile
    ///
    /// # Returns
    /// The generated Lua code or an error message
    pub fn compile(&self, source: &str) -> Result<String, String> {
        self.compile_with_optimization(source, OptimizationLevel::O0)
    }

    /// Compile source code using the container's dependencies (without stdlib) with optimization
    ///
    /// # Arguments
    /// * `source` - The TypedLua source code to compile
    /// * `level` - The optimization level to apply
    ///
    /// # Returns
    /// The generated Lua code or an error message
    pub fn compile_with_optimization(
        &self,
        source: &str,
        level: OptimizationLevel,
    ) -> Result<String, String> {
        let parser_handler =
            Arc::new(ParserCollectingHandler::new()) as Arc<dyn typedlua_parser::DiagnosticHandler>;
        let typecheck_handler = self.diagnostic_handler.clone();
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);

        let mut lexer = Lexer::new(source, parser_handler.clone(), &interner);
        let tokens = lexer
            .tokenize()
            .map_err(|e| format!("Lexing failed: {:?}", e))?;

        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids);
        let mut program = parser
            .parse()
            .map_err(|e| format!("Parsing failed: {:?}", e))?;

        let mut type_checker = TypeChecker::new(typecheck_handler.clone(), &interner, &common_ids);
        type_checker
            .check_program(&mut program)
            .map_err(|e| e.message)?;

        let mut optimizer = Optimizer::new(level, typecheck_handler.clone(), interner.clone());
        let _ = optimizer.optimize(&mut program);

        let mut codegen = CodeGenerator::new(interner.clone());
        let output = codegen.generate(&mut program);

        Ok(output)
    }

    /// Compile source code with stdlib loaded (for tests that need standard library)
    ///
    /// # Arguments
    /// * `source` - The TypedLua source code to compile
    ///
    /// # Returns
    /// The generated Lua code or an error message
    pub fn compile_with_stdlib(&self, source: &str) -> Result<String, String> {
        self.compile_with_stdlib_and_optimization(source, OptimizationLevel::O0)
    }

    /// Compile source code with stdlib loaded and optimization (for tests that need both)
    ///
    /// # Arguments
    /// * `source` - The TypedLua source code to compile
    /// * `level` - The optimization level to apply
    ///
    /// # Returns
    /// The generated Lua code or an error message
    pub fn compile_with_stdlib_and_optimization(
        &self,
        source: &str,
        level: OptimizationLevel,
    ) -> Result<String, String> {
        let parser_handler =
            Arc::new(ParserCollectingHandler::new()) as Arc<dyn typedlua_parser::DiagnosticHandler>;
        let typecheck_handler = self.diagnostic_handler.clone();
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);

        let mut lexer = Lexer::new(source, parser_handler.clone(), &interner);
        let tokens = lexer
            .tokenize()
            .map_err(|e| format!("Lexing failed: {:?}", e))?;

        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids);
        let mut program = parser
            .parse()
            .map_err(|e| format!("Parsing failed: {:?}", e))?;

        let mut type_checker =
            TypeChecker::new_with_stdlib(typecheck_handler.clone(), &interner, &common_ids)
                .map_err(|e| format!("Failed to load stdlib: {:?}", e))?;
        type_checker
            .check_program(&mut program)
            .map_err(|e| e.message)?;

        let mut optimizer = Optimizer::new(level, typecheck_handler.clone(), interner.clone());
        let _ = optimizer.optimize(&mut program);

        let mut codegen = CodeGenerator::new(interner.clone());
        let output = codegen.generate(&mut program);

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::CollectingDiagnosticHandler;
    use crate::fs::MockFileSystem;
    use typedlua_parser::span::Span;

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
        config.compiler_options.strict_null_checks = false;

        let container = Container::new(config);

        assert!(!container.config().compiler_options.strict_null_checks);
    }
}
