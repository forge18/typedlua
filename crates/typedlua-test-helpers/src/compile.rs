//! Test compilation helpers for TypedLua
//!
//! Provides convenient functions for compiling TypedLua source code
//! in tests, using proper DI through the Container.

use std::sync::Arc;
use typedlua_core::config::{CompilerConfig, OptimizationLevel};
use typedlua_core::di::DiContainer;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::fs::MockFileSystem;
use typedlua_core::TypeChecker;
use typedlua_parser::string_interner::StringInterner;
use typedlua_parser::{Lexer, Parser};

/// Compile TypedLua source code without stdlib
///
/// # Arguments
/// * `source` - The TypedLua source code to compile
///
/// # Returns
/// The generated Lua code or an error message
pub fn compile(source: &str) -> Result<String, String> {
    let config = CompilerConfig::default();
    let mut container = DiContainer::production(config);
    container.compile(source)
}

/// Compile TypedLua source code without stdlib and with optimization
///
/// # Arguments
/// * `source` - The TypedLua source code to compile
/// * `level` - The optimization level to apply
///
/// # Returns
/// The generated Lua code or an error message
pub fn compile_with_optimization(source: &str, level: OptimizationLevel) -> Result<String, String> {
    let config = CompilerConfig::default();
    let mut container = DiContainer::production(config);
    container.compile_with_optimization(source, level)
}

/// Compile TypedLua source code with stdlib loaded
///
/// Use this for tests that need standard library features
/// like debug.traceback(), print(), etc.
///
/// # Arguments
/// * `source` - The TypedLua source code to compile
///
/// # Returns
/// The generated Lua code or an error message
pub fn compile_with_stdlib(source: &str) -> Result<String, String> {
    let config = CompilerConfig::default();
    let mut container = DiContainer::production(config);
    container.compile_with_stdlib(source)
}

/// Compile TypedLua source code with stdlib loaded and optimization
///
/// Use this for tests that need both standard library features and optimization.
///
/// # Arguments
/// * `source` - The TypedLua source code to compile
/// * `level` - The optimization level to apply
///
/// # Returns
/// The generated Lua code or an error message
pub fn compile_with_stdlib_and_optimization(
    source: &str,
    level: OptimizationLevel,
) -> Result<String, String> {
    let config = CompilerConfig::default();
    let mut container = DiContainer::production(config);
    container.compile_with_stdlib_and_optimization(source, level)
}

/// Type check TypedLua source code
///
/// Returns the symbol table for further inspection, or an error message.
///
/// # Arguments
/// * `source` - The TypedLua source code to type check
///
/// # Returns
/// Ok(()) if type checking succeeds, or an error message
pub fn type_check(source: &str) -> Result<(), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = std::rc::Rc::new(interner);

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let mut type_checker = TypeChecker::new(handler, &interner, &common_ids);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    Ok(())
}

/// Create a test container with mock file system
///
/// Useful for tests that need to test file system interactions.
///
/// # Arguments
/// * `config` - Compiler configuration to use
///
/// # Returns
/// A DiContainer with mock file system
pub fn create_test_container(config: CompilerConfig) -> DiContainer {
    let diagnostics = Arc::new(CollectingDiagnosticHandler::new());
    let fs = Arc::new(MockFileSystem::new());
    DiContainer::test(config, diagnostics, fs)
}
