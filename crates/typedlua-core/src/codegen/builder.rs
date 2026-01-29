//! Builder pattern for CodeGenerator configuration
//!
//! Provides a fluent, self-documenting API for creating configured CodeGenerator instances.
//!
//! # Example
//!
//! ```rust
//! use std::rc::Rc;
//! use typedlua_parser::string_interner::StringInterner;
//! use typedlua_core::codegen::{CodeGeneratorBuilder, LuaTarget, CodeGenMode};
//!
//! let interner = Rc::new(StringInterner::new());
//! let generator = CodeGeneratorBuilder::new(interner)
//!     .target(LuaTarget::Lua54)
//!     .source_map("main.tl".to_string())
//!     .optimization_level(typedlua_core::config::OptimizationLevel::O2)
//!     .build();
//! ```

use std::rc::Rc;
use typedlua_parser::string_interner::StringInterner;

use super::{CodeGenMode, CodeGenerator, LuaTarget};
use crate::config::OptimizationLevel;

/// Builder for configuring and constructing a [`CodeGenerator`] instance.
///
/// This builder provides a fluent API for setting up code generation parameters
/// before creating the actual generator. It ensures all required fields are
/// provided and allows optional configuration of advanced features.
///
/// # Required Fields
///
/// - `interner`: A reference-counted [`StringInterner`] for identifier resolution
///
/// # Optional Configuration
///
/// - `target`: Lua version target (defaults to Lua 5.4)
/// - `source_map`: Enable source map generation with a source file name
/// - `mode`: Code generation mode - Require or Bundle (defaults to Require)
/// - `optimization_level`: Optimization level O0-O3 (defaults to O0)
///
/// # Example
///
/// ```rust
/// use std::rc::Rc;
/// use typedlua_parser::string_interner::StringInterner;
/// use typedlua_core::codegen::{CodeGeneratorBuilder, LuaTarget, CodeGenMode};
/// use typedlua_core::config::OptimizationLevel;
///
/// let interner = Rc::new(StringInterner::new());
/// let generator = CodeGeneratorBuilder::new(interner)
///     .target(LuaTarget::Lua53)
///     .source_map("input.tl".to_string())
///     .bundle_mode("main".to_string())
///     .optimization_level(OptimizationLevel::O2)
///     .build();
/// ```
pub struct CodeGeneratorBuilder {
    interner: Rc<StringInterner>,
    target: LuaTarget,
    source_map: Option<String>,
    mode: CodeGenMode,
    optimization_level: OptimizationLevel,
}

impl CodeGeneratorBuilder {
    /// Creates a new builder with the required string interner.
    ///
    /// # Arguments
    ///
    /// * `interner` - A reference-counted [`StringInterner`] for resolving StringIds
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::rc::Rc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    ///
    /// let interner = Rc::new(StringInterner::new());
    /// let builder = CodeGeneratorBuilder::new(interner);
    /// ```
    pub fn new(interner: Rc<StringInterner>) -> Self {
        Self {
            interner,
            target: LuaTarget::default(),
            source_map: None,
            mode: CodeGenMode::Require,
            optimization_level: OptimizationLevel::O0,
        }
    }

    /// Sets the Lua target version for code generation.
    ///
    /// # Arguments
    ///
    /// * `target` - The [`LuaTarget`] version (Lua51, Lua52, Lua53, or Lua54)
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::rc::Rc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::{CodeGeneratorBuilder, LuaTarget};
    ///
    /// let interner = Rc::new(StringInterner::new());
    /// let generator = CodeGeneratorBuilder::new(interner)
    ///     .target(LuaTarget::Lua53)
    ///     .build();
    /// ```
    pub fn target(mut self, target: LuaTarget) -> Self {
        self.target = target;
        self
    }

    /// Enables source map generation with the given source file name.
    ///
    /// # Arguments
    ///
    /// * `source_file` - The name of the source file for source map generation
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::rc::Rc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    ///
    /// let interner = Rc::new(StringInterner::new());
    /// let generator = CodeGeneratorBuilder::new(interner)
    ///     .source_map("main.tl".to_string())
    ///     .build();
    /// ```
    pub fn source_map(mut self, source_file: String) -> Self {
        self.source_map = Some(source_file);
        self
    }

    /// Sets the code generation mode to "Require" (default).
    ///
    /// In Require mode, modules are generated as separate files that use
    /// Lua's `require()` function for imports.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::rc::Rc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    ///
    /// let interner = Rc::new(StringInterner::new());
    /// let generator = CodeGeneratorBuilder::new(interner)
    ///     .require_mode()
    ///     .build();
    /// ```
    pub fn require_mode(mut self) -> Self {
        self.mode = CodeGenMode::Require;
        self
    }

    /// Sets the code generation mode to "Bundle" with the given module ID.
    ///
    /// In Bundle mode, all modules are combined into a single output file
    /// with a custom module loader.
    ///
    /// # Arguments
    ///
    /// * `module_id` - The canonical path/ID of this module in the bundle
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::rc::Rc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    ///
    /// let interner = Rc::new(StringInterner::new());
    /// let generator = CodeGeneratorBuilder::new(interner)
    ///     .bundle_mode("src/main".to_string())
    ///     .build();
    /// ```
    pub fn bundle_mode(mut self, module_id: String) -> Self {
        self.mode = CodeGenMode::Bundle { module_id };
        self
    }

    /// Sets the optimization level for code generation.
    ///
    /// # Arguments
    ///
    /// * `level` - The [`OptimizationLevel`] (O0, O1, O2, O3, or Auto)
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::rc::Rc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    /// use typedlua_core::config::OptimizationLevel;
    ///
    /// let interner = Rc::new(StringInterner::new());
    /// let generator = CodeGeneratorBuilder::new(interner)
    ///     .optimization_level(OptimizationLevel::O2)
    ///     .build();
    /// ```
    pub fn optimization_level(mut self, level: OptimizationLevel) -> Self {
        self.optimization_level = level;
        self
    }

    /// Builds and returns a configured [`CodeGenerator`] instance.
    ///
    /// This consumes the builder and creates the generator with all
    /// the configured settings.
    ///
    /// # Returns
    ///
    /// A fully configured [`CodeGenerator`] ready for code generation.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::rc::Rc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::{CodeGeneratorBuilder, LuaTarget};
    /// use typedlua_core::config::OptimizationLevel;
    ///
    /// let interner = Rc::new(StringInterner::new());
    /// let generator = CodeGeneratorBuilder::new(interner)
    ///     .target(LuaTarget::Lua54)
    ///     .optimization_level(OptimizationLevel::O1)
    ///     .build();
    /// ```
    pub fn build(self) -> CodeGenerator {
        let mut generator = CodeGenerator::new(self.interner);
        generator = generator.with_target(self.target);
        generator = generator.with_mode(self.mode);
        generator = generator.with_optimization_level(self.optimization_level);

        if let Some(source_file) = self.source_map {
            generator = generator.with_source_map(source_file);
        }

        generator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_interner() -> Rc<StringInterner> {
        Rc::new(StringInterner::new())
    }

    #[test]
    fn test_builder_default_configuration() {
        let interner = create_test_interner();
        let generator = CodeGeneratorBuilder::new(interner).build();

        // Verify defaults through behavior (generator is opaque)
        // The generator should be created successfully with defaults
        assert_eq!(generator.target, LuaTarget::Lua54);
        assert!(matches!(generator.mode, CodeGenMode::Require));
        assert_eq!(generator.optimization_level, OptimizationLevel::O0);
    }

    #[test]
    fn test_builder_target_configuration() {
        let interner = create_test_interner();

        let generator = CodeGeneratorBuilder::new(interner.clone())
            .target(LuaTarget::Lua51)
            .build();
        assert_eq!(generator.target, LuaTarget::Lua51);

        let generator = CodeGeneratorBuilder::new(interner.clone())
            .target(LuaTarget::Lua52)
            .build();
        assert_eq!(generator.target, LuaTarget::Lua52);

        let generator = CodeGeneratorBuilder::new(interner.clone())
            .target(LuaTarget::Lua53)
            .build();
        assert_eq!(generator.target, LuaTarget::Lua53);

        let generator = CodeGeneratorBuilder::new(interner)
            .target(LuaTarget::Lua54)
            .build();
        assert_eq!(generator.target, LuaTarget::Lua54);
    }

    #[test]
    fn test_builder_source_map_configuration() {
        let interner = create_test_interner();

        // Without source map
        let generator = CodeGeneratorBuilder::new(interner.clone()).build();
        assert!(generator.source_map.is_none());

        // With source map
        let generator = CodeGeneratorBuilder::new(interner)
            .source_map("test.tl".to_string())
            .build();
        assert!(generator.source_map.is_some());
    }

    #[test]
    fn test_builder_mode_configuration() {
        let interner = create_test_interner();

        // Require mode (default)
        let generator = CodeGeneratorBuilder::new(interner.clone())
            .require_mode()
            .build();
        assert!(matches!(generator.mode, CodeGenMode::Require));

        // Bundle mode
        let generator = CodeGeneratorBuilder::new(interner)
            .bundle_mode("main".to_string())
            .build();
        assert!(matches!(
            generator.mode,
            CodeGenMode::Bundle { module_id } if module_id == "main"
        ));
    }

    #[test]
    fn test_builder_optimization_level_configuration() {
        let interner = create_test_interner();

        let generator = CodeGeneratorBuilder::new(interner.clone())
            .optimization_level(OptimizationLevel::O0)
            .build();
        assert_eq!(generator.optimization_level, OptimizationLevel::O0);

        let generator = CodeGeneratorBuilder::new(interner.clone())
            .optimization_level(OptimizationLevel::O1)
            .build();
        assert_eq!(generator.optimization_level, OptimizationLevel::O1);

        let generator = CodeGeneratorBuilder::new(interner.clone())
            .optimization_level(OptimizationLevel::O2)
            .build();
        assert_eq!(generator.optimization_level, OptimizationLevel::O2);

        let generator = CodeGeneratorBuilder::new(interner.clone())
            .optimization_level(OptimizationLevel::O3)
            .build();
        assert_eq!(generator.optimization_level, OptimizationLevel::O3);

        let generator = CodeGeneratorBuilder::new(interner)
            .optimization_level(OptimizationLevel::Auto)
            .build();
        assert_eq!(generator.optimization_level, OptimizationLevel::Auto);
    }

    #[test]
    fn test_builder_chained_configuration() {
        let interner = create_test_interner();
        let generator = CodeGeneratorBuilder::new(interner)
            .target(LuaTarget::Lua53)
            .source_map("main.tl".to_string())
            .bundle_mode("src/main".to_string())
            .optimization_level(OptimizationLevel::O2)
            .build();

        assert_eq!(generator.target, LuaTarget::Lua53);
        assert!(generator.source_map.is_some());
        assert!(matches!(
            generator.mode,
            CodeGenMode::Bundle { module_id } if module_id == "src/main"
        ));
        assert_eq!(generator.optimization_level, OptimizationLevel::O2);
    }

    #[test]
    fn test_builder_fluent_api() {
        let interner = create_test_interner();

        // Test that all methods return Self for chaining
        let _generator = CodeGeneratorBuilder::new(interner)
            .target(LuaTarget::Lua54)
            .source_map("file.tl".to_string())
            .require_mode()
            .bundle_mode("bundle".to_string())
            .optimization_level(OptimizationLevel::O1)
            .build();

        // If this compiles, the fluent API works correctly
    }
}
