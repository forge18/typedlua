//! Builder pattern for CodeGenerator configuration
//!
//! Provides a fluent, self-documenting API for creating configured CodeGenerator instances.
//!
//! # Example
//!
//! ```rust
//! use std::sync::Arc;
//! use typedlua_parser::string_interner::StringInterner;
//! use typedlua_core::codegen::{CodeGeneratorBuilder, LuaTarget, CodeGenMode};
//!
//! let interner = Arc::new(StringInterner::new());
//! let generator = CodeGeneratorBuilder::new(interner)
//!     .target(LuaTarget::Lua54)
//!     .source_map("main.tl".to_string())
//!     .optimization_level(typedlua_core::config::OptimizationLevel::O2)
//!     .build();
//! ```

use std::sync::Arc;
use typedlua_parser::string_interner::StringInterner;

use super::{CodeGenMode, CodeGenerator, LuaTarget, ReflectionMode};
use crate::config::{OptimizationLevel, OutputFormat};
use crate::optimizer::WholeProgramAnalysis;

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
/// use std::sync::Arc;
/// use typedlua_parser::string_interner::StringInterner;
/// use typedlua_core::codegen::CodeGeneratorBuilder;
///
/// let interner = Arc::new(StringInterner::new());
/// let builder = CodeGeneratorBuilder::new(interner);
/// ```
pub struct CodeGeneratorBuilder {
    interner: Arc<StringInterner>,
    target: LuaTarget,
    source_map: Option<String>,
    mode: CodeGenMode,
    optimization_level: OptimizationLevel,
    output_format: OutputFormat,
    whole_program_analysis: Option<WholeProgramAnalysis>,
    reachable_exports: Option<std::collections::HashSet<String>>,
    reflection_mode: ReflectionMode,
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
    /// use std::sync::Arc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::{CodeGeneratorBuilder, LuaTarget, CodeGenMode};
    /// use typedlua_core::config::OptimizationLevel;
    ///
    /// let interner = Arc::new(StringInterner::new());
    /// let generator = CodeGeneratorBuilder::new(interner)
    ///     .target(LuaTarget::Lua53)
    ///     .source_map("input.tl".to_string())
    ///     .bundle_mode("main".to_string())
    ///     .optimization_level(OptimizationLevel::O2)
    ///     .build();
    /// ```
    pub fn new(interner: Arc<StringInterner>) -> Self {
        Self {
            interner,
            target: LuaTarget::default(),
            source_map: None,
            mode: CodeGenMode::Require,
            optimization_level: OptimizationLevel::O0,
            output_format: OutputFormat::Readable,
            whole_program_analysis: None,
            reachable_exports: None,
            reflection_mode: ReflectionMode::default(),
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
    /// use std::sync::Arc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::{CodeGeneratorBuilder, LuaTarget, CodeGenMode};
    ///
    /// let interner = Arc::new(StringInterner::new());
    /// let generator = CodeGeneratorBuilder::new(interner)
    ///     .target(LuaTarget::Lua54)
    ///     .source_map("main.tl".to_string())
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
    /// use std::sync::Arc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    ///
    /// let interner = Arc::new(StringInterner::new());
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
    /// use std::sync::Arc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    ///
    /// let interner = Arc::new(StringInterner::new());
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
    /// use std::sync::Arc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    ///
    /// let interner = Arc::new(StringInterner::new());
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
    /// use std::sync::Arc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    /// use typedlua_core::config::OptimizationLevel;
    ///
    /// let interner = Arc::new(StringInterner::new());
    /// let generator = CodeGeneratorBuilder::new(interner)
    ///     .optimization_level(OptimizationLevel::O2)
    ///     .build();
    /// ```
    pub fn optimization_level(mut self, level: OptimizationLevel) -> Self {
        self.optimization_level = level;
        self
    }

    /// Sets the output format for code generation.
    ///
    /// # Arguments
    ///
    /// * `format` - The [`OutputFormat`] (Readable, Compact, or Minified)
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::sync::Arc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    /// use typedlua_core::config::OutputFormat;
    ///
    /// let interner = Arc::new(StringInterner::new());
    /// let generator = CodeGeneratorBuilder::new(interner)
    ///     .output_format(OutputFormat::Minified)
    ///     .build();
    /// ```
    pub fn output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
        self
    }

    /// Sets the whole-program analysis for cross-module optimizations.
    ///
    /// This is optional and only needed for O3+ optimizations that benefit
    /// from cross-module analysis (e.g., devirtualization).
    ///
    /// # Arguments
    ///
    /// * `analysis` - The [`WholeProgramAnalysis`] results from scanning all modules
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::sync::Arc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    /// use typedlua_core::optimizer::WholeProgramAnalysis;
    /// use typedlua_core::config::OptimizationLevel;
    ///
    /// let interner = Arc::new(StringInterner::new());
    /// // Assume analysis was built from all checked modules
    /// // let analysis = WholeProgramAnalysis::build(&programs, OptimizationLevel::O3);
    /// // let generator = CodeGeneratorBuilder::new(interner)
    /// //     .optimization_level(OptimizationLevel::O3)
    /// //     .with_whole_program_analysis(analysis)
    /// //     .build();
    /// ```
    pub fn with_whole_program_analysis(mut self, analysis: WholeProgramAnalysis) -> Self {
        self.whole_program_analysis = Some(analysis);
        self
    }

    /// Sets the reflection metadata generation mode.
    pub fn reflection_mode(mut self, mode: ReflectionMode) -> Self {
        self.reflection_mode = mode;
        self
    }

    /// Sets the reachable exports for tree shaking in bundle mode.
    ///
    /// When tree shaking is enabled, exports not in this set will be skipped
    /// during code generation, resulting in smaller bundles.
    ///
    /// # Arguments
    ///
    /// * `reachable_exports` - A set of export names that should be included
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::sync::Arc;
    /// use std::collections::HashSet;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::CodeGeneratorBuilder;
    ///
    /// let interner = Arc::new(StringInterner::new());
    /// let reachable: HashSet<String> = ["add", "subtract"].into_iter().map(|s| s.to_string()).collect();
    /// let generator = CodeGeneratorBuilder::new(interner)
    ///     .with_tree_shaking(reachable)
    ///     .build();
    /// ```
    pub fn with_tree_shaking(
        mut self,
        reachable_exports: std::collections::HashSet<String>,
    ) -> Self {
        self.reachable_exports = Some(reachable_exports);
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
    /// use std::sync::Arc;
    /// use typedlua_parser::string_interner::StringInterner;
    /// use typedlua_core::codegen::{CodeGeneratorBuilder, LuaTarget};
    /// use typedlua_core::config::OptimizationLevel;
    ///
    /// let interner = Arc::new(StringInterner::new());
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
        generator = generator.with_output_format(self.output_format);
        generator = generator.with_reflection_mode(self.reflection_mode);

        if let Some(source_file) = self.source_map {
            generator = generator.with_source_map(source_file);
        }

        if let Some(analysis) = self.whole_program_analysis {
            generator = generator.with_whole_program_analysis(analysis);
        }

        if let Some(ref exports) = self.reachable_exports {
            generator = generator.with_tree_shaking(exports.clone());
        }

        generator
    }
}
