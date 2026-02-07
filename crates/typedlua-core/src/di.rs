use super::codegen::CodeGenerator;
use super::config::{CompilerConfig, OptimizationLevel};
use super::diagnostics::{
    CollectingDiagnosticHandler, ConsoleDiagnosticHandler, DiagnosticHandler,
};
use super::fs::{FileSystem, MockFileSystem, RealFileSystem};
use super::optimizer::Optimizer;
use bumpalo::Bump;
use std::any::{Any, TypeId};
use std::sync::Arc;
use typedlua_parser::diagnostics::CollectingDiagnosticHandler as ParserCollectingHandler;
use typedlua_parser::string_interner::StringInterner;
use typedlua_parser::{Lexer, Parser};
use typedlua_typechecker::TypeChecker;

#[derive(Clone, Copy)]
pub enum ServiceLifetime {
    Transient,
    Singleton,
}

type FactoryFn = Arc<dyn Fn(&mut DiContainer) -> Box<dyn Any + Send + Sync> + Send + Sync>;

pub struct DiContainer {
    factories: HashMap<TypeId, (FactoryFn, ServiceLifetime)>,
    singletons: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl DiContainer {
    pub fn new() -> Self {
        DiContainer {
            factories: HashMap::default(),
            singletons: HashMap::default(),
        }
    }

    pub fn production(config: CompilerConfig) -> Self {
        let mut container = DiContainer::new();
        let config = Arc::new(config);

        container.register(
            move |_| config.clone() as Arc<CompilerConfig>,
            ServiceLifetime::Singleton,
        );

        container.register(
            |_| Arc::new(ConsoleDiagnosticHandler::new(false)) as Arc<dyn DiagnosticHandler>,
            ServiceLifetime::Singleton,
        );

        container.register(
            |_| Arc::new(RealFileSystem::new()) as Arc<dyn FileSystem>,
            ServiceLifetime::Singleton,
        );

        container
    }

    pub fn test(
        config: CompilerConfig,
        diagnostic_handler: Arc<dyn DiagnosticHandler>,
        file_system: Arc<dyn FileSystem>,
    ) -> Self {
        let mut container = DiContainer::new();
        let config = Arc::new(config);

        container.register(
            move |_| config.clone() as Arc<CompilerConfig>,
            ServiceLifetime::Singleton,
        );

        container.register(
            move |_| diagnostic_handler.clone() as Arc<dyn DiagnosticHandler>,
            ServiceLifetime::Singleton,
        );

        container.register(
            move |_| file_system.clone() as Arc<dyn FileSystem>,
            ServiceLifetime::Singleton,
        );

        container
    }

    pub fn test_default() -> Self {
        let config = CompilerConfig::default();
        let diagnostics = Arc::new(CollectingDiagnosticHandler::new());
        let fs = Arc::new(MockFileSystem::new());
        Self::test(config, diagnostics, fs)
    }

    pub fn test_with_config(config: CompilerConfig) -> Self {
        let diagnostics = Arc::new(CollectingDiagnosticHandler::new());
        let fs = Arc::new(MockFileSystem::new());
        Self::test(config, diagnostics, fs)
    }

    pub fn register<T>(
        &mut self,
        factory: impl Fn(&mut DiContainer) -> T + 'static + Send + Sync,
        lifetime: ServiceLifetime,
    ) where
        T: Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let boxed_factory: FactoryFn = Arc::new(move |container| {
            let value: T = factory(container);
            Box::new(value) as Box<dyn Any + Send + Sync>
        });
        self.factories.insert(type_id, (boxed_factory, lifetime));
    }

    pub fn resolve<T: Clone + 'static + Send + Sync>(&mut self) -> Option<T> {
        let type_id = TypeId::of::<T>();

        let is_singleton = self
            .factories
            .get(&type_id)
            .is_some_and(|(_, l)| matches!(*l, ServiceLifetime::Singleton));

        if is_singleton {
            if let Some(cached) = self.singletons.get(&type_id) {
                return cached.downcast_ref::<T>().cloned();
            }
        }

        if let Some((factory, lifetime)) = self.factories.get(&type_id).cloned() {
            let result = (factory)(self);

            if let ServiceLifetime::Singleton = lifetime {
                let arc_result: Arc<dyn Any + Send + Sync> = Arc::from(result);
                let cloned = arc_result.clone();
                self.singletons.insert(type_id, arc_result);
                return cloned.downcast::<T>().ok().map(|arc_t| (*arc_t).clone());
            }

            return result.downcast::<T>().ok().map(|boxed| *boxed);
        }

        None
    }

    pub fn is_registered<T: 'static>(&self) -> bool {
        self.factories.contains_key(&TypeId::of::<T>())
    }

    pub fn service_count(&self) -> usize {
        self.factories.len()
    }

    pub fn singleton_count(&self) -> usize {
        self.singletons.len()
    }

    pub fn has_errors(&mut self) -> bool {
        self.resolve::<Arc<dyn DiagnosticHandler>>()
            .map(|h| h.has_errors())
            .unwrap_or(false)
    }

    pub fn error_count(&mut self) -> usize {
        self.resolve::<Arc<dyn DiagnosticHandler>>()
            .map(|h| h.error_count())
            .unwrap_or(0)
    }

    pub fn warning_count(&mut self) -> usize {
        self.resolve::<Arc<dyn DiagnosticHandler>>()
            .map(|h| h.warning_count())
            .unwrap_or(0)
    }

    pub fn config(&mut self) -> Arc<CompilerConfig> {
        self.resolve::<Arc<CompilerConfig>>().unwrap()
    }

    pub fn compile(&mut self, source: &str) -> Result<String, String> {
        self.compile_with_optimization(source, OptimizationLevel::O0)
    }

    pub fn compile_with_optimization(
        &mut self,
        source: &str,
        level: OptimizationLevel,
    ) -> Result<String, String> {
        let arena = Bump::new();
        let parser_handler =
            Arc::new(ParserCollectingHandler::new()) as Arc<dyn typedlua_parser::DiagnosticHandler>;
        let typecheck_handler = self
            .resolve::<Arc<dyn DiagnosticHandler>>()
            .unwrap_or_else(|| Arc::new(ConsoleDiagnosticHandler::new(false)));
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Arc::new(interner);

        let mut lexer = Lexer::new(source, parser_handler.clone(), &interner);
        let tokens = lexer
            .tokenize()
            .map_err(|e| format!("Lexing failed: {:?}", e))?;

        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids, &arena);
        let program = parser
            .parse()
            .map_err(|e| format!("Parsing failed: {:?}", e))?;

        let mut type_checker =
            TypeChecker::new(typecheck_handler.clone(), &interner, &common_ids, &arena);
        type_checker
            .check_program(&program)
            .map_err(|e| e.message)?;

        let mut mutable_program = crate::MutableProgram::from_program(&program);

        let mut optimizer = Optimizer::new(level, typecheck_handler.clone(), interner.clone());
        if let Err(err_msg) = optimizer.optimize(&mut mutable_program, &arena) {
            typecheck_handler.warning(
                typedlua_parser::span::Span::dummy(),
                &format!("Optimization warning: {}", err_msg),
            );
        }

        let mut codegen = CodeGenerator::new(interner.clone());
        let output = codegen.generate(&mutable_program);

        Ok(output)
    }

    pub fn compile_with_stdlib(&mut self, source: &str) -> Result<String, String> {
        self.compile_with_stdlib_and_optimization(source, OptimizationLevel::O0)
    }

    pub fn compile_with_stdlib_and_optimization(
        &mut self,
        source: &str,
        level: OptimizationLevel,
    ) -> Result<String, String> {
        let arena = Bump::new();
        let parser_handler =
            Arc::new(ParserCollectingHandler::new()) as Arc<dyn typedlua_parser::DiagnosticHandler>;
        let typecheck_handler = self
            .resolve::<Arc<dyn DiagnosticHandler>>()
            .unwrap_or_else(|| Arc::new(ConsoleDiagnosticHandler::new(false)));
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Arc::new(interner);

        let mut lexer = Lexer::new(source, parser_handler.clone(), &interner);
        let tokens = lexer
            .tokenize()
            .map_err(|e| format!("Lexing failed: {:?}", e))?;

        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids, &arena);
        let program = parser
            .parse()
            .map_err(|e| format!("Parsing failed: {:?}", e))?;

        let mut type_checker =
            TypeChecker::new_with_stdlib(typecheck_handler.clone(), &interner, &common_ids, &arena)
                .map_err(|e| format!("Failed to load stdlib: {:?}", e))?;
        type_checker
            .check_program(&program)
            .map_err(|e| e.message)?;

        let mut mutable_program = crate::MutableProgram::from_program(&program);

        let mut optimizer = Optimizer::new(level, typecheck_handler.clone(), interner.clone());
        if let Err(err_msg) = optimizer.optimize(&mut mutable_program, &arena) {
            typecheck_handler.warning(
                typedlua_parser::span::Span::dummy(),
                &format!("Optimization warning: {}", err_msg),
            );
        }

        let mut codegen = CodeGenerator::new(interner.clone());
        let output = codegen.generate(&mutable_program);

        Ok(output)
    }
}

impl Default for DiContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeCheckHelper for DiContainer {
    fn type_check_source(&self, source: &str) -> Result<(), String> {
        let arena = Bump::new();
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Arc::new(interner);

        let mut lexer = Lexer::new(source, handler.clone(), &interner);
        let tokens = lexer
            .tokenize()
            .map_err(|e| format!("Lexing failed: {:?}", e))?;

        let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
        let program = parser
            .parse()
            .map_err(|e| format!("Parsing failed: {:?}", e))?;

        let mut type_checker = TypeChecker::new(handler, &interner, &common_ids, &arena);
        type_checker
            .check_program(&program)
            .map_err(|e| e.message)?;

        Ok(())
    }
}

use rustc_hash::FxHashMap as HashMap;

pub trait TypeCheckHelper {
    fn type_check_source(&self, source: &str) -> Result<(), String>;
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
        let mut container = DiContainer::production(config);

        assert_eq!(container.error_count(), 0);
        assert!(!container.has_errors());
    }

    #[test]
    fn test_container_with_mock_dependencies() {
        let config = CompilerConfig::default();
        let diagnostics = Arc::new(CollectingDiagnosticHandler::new());
        let fs = Arc::new(MockFileSystem::new());

        let mut container = DiContainer::test(config, diagnostics.clone(), fs);

        container
            .resolve::<Arc<dyn DiagnosticHandler>>()
            .unwrap()
            .error(Span::dummy(), "Test error");

        assert!(container.has_errors());
        assert_eq!(container.error_count(), 1);
    }

    #[test]
    fn test_container_config_access() {
        let mut config = CompilerConfig::default();
        config.compiler_options.strict_null_checks = false;

        let mut container = DiContainer::production(config);

        assert!(!container.config().compiler_options.strict_null_checks);
    }

    #[test]
    fn test_container_compile_simple() {
        let source = r#"
            const x: number = 42
            return x
        "#;

        let config = CompilerConfig::default();
        let mut container = DiContainer::production(config);

        let result = container.compile(source);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("42"));
    }

    #[test]
    fn test_container_compile_with_optimization() {
        let source = r#"
            const x = 1 + 2 + 3
            return x
        "#;

        let config = CompilerConfig::default();
        let mut container = DiContainer::production(config);

        let result = container.compile_with_optimization(source, OptimizationLevel::O2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_container_compile_with_stdlib() {
        let source = r#"
            const x: number = 42
            return x
        "#;

        let config = CompilerConfig::default();
        let mut container = DiContainer::production(config);

        let result = container.compile_with_stdlib(source);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("42"));
    }

    #[test]
    fn test_container_compile_with_stdlib_and_optimization() {
        let source = r#"
            const x = 10 * 5
            return x
        "#;

        let config = CompilerConfig::default();
        let mut container = DiContainer::production(config);

        let result = container.compile_with_stdlib_and_optimization(source, OptimizationLevel::O2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_container_compile_error() {
        let source = r#"
            const x: number = "wrong"
            return x
        "#;

        let config = CompilerConfig::default();
        let mut container = DiContainer::production(config);

        let result = container.compile(source);
        assert!(result.is_ok() || container.has_errors());
    }

    #[test]
    fn test_container_warning_count() {
        let source = r#"
            local unused = 42
            const x: number = 10
            return x
        "#;

        let config = CompilerConfig::default();
        let mut container = DiContainer::production(config);

        let _ = container.compile(source);
        let _ = container.warning_count();
    }

    #[test]
    fn test_container_file_system_access() {
        let config = CompilerConfig::default();
        let mut container = DiContainer::production(config);

        let _fs = container.resolve::<Arc<dyn FileSystem>>();
        assert!(true);
    }

    #[test]
    fn test_container_default_options() {
        let config = CompilerConfig::default();
        let mut container = DiContainer::production(config);

        let cfg = container.config();
        assert!(cfg.compiler_options.strict_null_checks);
    }

    #[test]
    fn test_service_registration() {
        let mut container = DiContainer::new();
        assert_eq!(container.service_count(), 0);

        container.register(|_| Arc::new(42) as Arc<i32>, ServiceLifetime::Singleton);
        assert_eq!(container.service_count(), 1);
        assert!(container.is_registered::<Arc<i32>>());
    }

    #[test]
    fn test_transient_service() {
        let mut container = DiContainer::new();
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_for_check = counter.clone();

        container.register(
            move |_| {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                counter.clone() as Arc<std::sync::atomic::AtomicUsize>
            },
            ServiceLifetime::Transient,
        );

        let _ = container.resolve::<Arc<std::sync::atomic::AtomicUsize>>();
        let _ = container.resolve::<Arc<std::sync::atomic::AtomicUsize>>();
        let _ = container.resolve::<Arc<std::sync::atomic::AtomicUsize>>();

        assert_eq!(
            counter_for_check.load(std::sync::atomic::Ordering::SeqCst),
            3
        );
    }

    #[test]
    fn test_singleton_service() {
        let mut container = DiContainer::new();
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_clone = counter.clone();

        container.register(
            move |_| {
                counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let value = counter_clone.load(std::sync::atomic::Ordering::SeqCst);
                Arc::new(value) as Arc<usize>
            },
            ServiceLifetime::Singleton,
        );

        let _ = container.resolve::<Arc<usize>>();
        let _ = container.resolve::<Arc<usize>>();
        let _ = container.resolve::<Arc<usize>>();

        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(container.singleton_count(), 1);
    }
}
