pub mod builder;
pub mod emitter;
pub mod sourcemap;
pub mod strategies;
pub mod traits;

pub mod classes;
pub mod decorators;
pub mod enums;
pub mod expressions;
pub mod modules;
pub mod patterns;
pub mod scope_hoisting;
pub mod statements;
pub mod tree_shaking;

pub use emitter::Emitter;

pub use builder::CodeGeneratorBuilder;
pub use sourcemap::{SourceMap, SourceMapBuilder};

// Re-export types needed for builder API
pub use super::config::OptimizationLevel;
use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::*;
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::{StringId, StringInterner};
use typedlua_runtime::module;
use typedlua_runtime::reflection;

/// Target Lua version for code generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LuaTarget {
    /// Lua 5.1 (widespread compatibility)
    Lua51,
    /// Lua 5.2 (added goto, bit operators via library)
    Lua52,
    /// Lua 5.3 (added integers, bitwise operators)
    Lua53,
    /// Lua 5.4 (added const, to-be-closed)
    #[default]
    Lua54,
}

/// Dedent a multi-line template literal string.
/// Removes common leading whitespace from non-empty lines, trims leading/trailing blank lines.
pub fn dedent(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();

    if lines.is_empty() {
        return String::new();
    }

    let non_empty_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, &line)| !line.trim().is_empty())
        .map(|(i, _)| i)
        .collect();

    if non_empty_indices.is_empty() {
        return String::new();
    }

    let min_indent = non_empty_indices
        .iter()
        .map(|&i| {
            let line = lines[i];
            let stripped = line.trim_start();
            line.len() - stripped.len()
        })
        .min()
        .unwrap_or(0);

    let result_lines: Vec<String> = lines
        .iter()
        .map(|&line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                String::new()
            } else if line.len() >= min_indent {
                line[min_indent..].to_string()
            } else {
                line.to_string()
            }
        })
        .collect();

    result_lines.join("\n")
}

/// Code generation mode for modules
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeGenMode {
    /// Generate separate files with require() calls
    Require,
    /// Bundle all modules into a single file
    Bundle {
        /// The canonical path of this module in the bundle
        module_id: String,
    },
}

/// Code generator for TypedLua to Lua
pub struct CodeGenerator {
    emitter: Emitter,
    target: LuaTarget,
    current_class_parent: Option<StringId>,
    uses_built_in_decorators: bool,
    /// Module generation mode
    mode: CodeGenMode,
    /// Track exported symbols for module mode
    exports: Vec<String>,
    /// Track if there's a default export
    has_default_export: bool,
    /// Import source to module ID mapping (for bundle mode)
    import_map: std::collections::HashMap<String, String>,
    /// Current source index for multi-source source maps (bundle mode)
    current_source_index: usize,
    /// String interner for resolving identifiers (shared with optimizer)
    interner: Arc<StringInterner>,
    /// Optimization level for code generation
    optimization_level: crate::config::OptimizationLevel,
    /// Track interface default methods: (interface_name, method_name) -> default_function_name
    interface_default_methods: std::collections::HashMap<(String, String), String>,
    /// Current namespace path for the file (if any)
    current_namespace: Option<Vec<String>>,
    /// Namespace exports to attach: (local_name, namespace_path_string)
    namespace_exports: Vec<(String, String)>,
    /// Reflection: counter for assigning unique type IDs
    next_type_id: u32,
    /// Reflection: track registered types for __TypeRegistry
    registered_types: std::collections::HashMap<String, u32>,
    /// Code generation strategy for Lua version-specific logic
    strategy: Box<dyn strategies::CodeGenStrategy>,
    /// Enforce access modifiers (private/protected/public) at runtime
    enforce_access_modifiers: bool,
    /// Whole-program analysis for O3+ cross-module optimizations
    whole_program_analysis: Option<crate::optimizer::WholeProgramAnalysis>,
    /// Tree shaking: reachable exports for bundle mode
    reachable_exports: Option<std::collections::HashSet<String>>,
    /// Tree shaking: whether tree shaking is enabled
    tree_shaking_enabled: bool,
    /// Scope hoisting: whether scope hoisting is enabled for bundles
    scope_hoisting_enabled: bool,
}

impl CodeGenerator {
    pub fn new(interner: Arc<StringInterner>) -> Self {
        let target = LuaTarget::default();
        Self {
            emitter: Emitter::new(),
            target,
            current_class_parent: None,
            uses_built_in_decorators: false,
            mode: CodeGenMode::Require,
            exports: Vec::new(),
            has_default_export: false,
            import_map: Default::default(),
            current_source_index: 0,
            interner,
            optimization_level: crate::config::OptimizationLevel::O0,
            interface_default_methods: Default::default(),
            current_namespace: None,
            namespace_exports: Vec::new(),
            next_type_id: 1,
            registered_types: Default::default(),
            strategy: Self::create_strategy(target),
            enforce_access_modifiers: false,
            whole_program_analysis: None,
            reachable_exports: None,
            tree_shaking_enabled: false,
            scope_hoisting_enabled: true,
        }
    }

    pub fn with_output_format(mut self, format: crate::config::OutputFormat) -> Self {
        self.emitter = self.emitter.with_output_format(format);
        self
    }

    pub fn with_enforce_access_modifiers(mut self, enforce: bool) -> Self {
        self.enforce_access_modifiers = enforce;
        self
    }

    /// Create a strategy for the given Lua target
    fn create_strategy(target: LuaTarget) -> Box<dyn strategies::CodeGenStrategy> {
        match target {
            LuaTarget::Lua51 => Box::new(strategies::lua51::Lua51Strategy),
            LuaTarget::Lua52 => Box::new(strategies::lua52::Lua52Strategy),
            LuaTarget::Lua53 => Box::new(strategies::lua53::Lua53Strategy),
            LuaTarget::Lua54 => Box::new(strategies::lua54::Lua54Strategy),
        }
    }

    /// Resolve a StringId to a String
    fn resolve(&self, id: typedlua_parser::string_interner::StringId) -> String {
        self.interner.resolve(id).to_string()
    }

    pub fn with_target(mut self, target: LuaTarget) -> Self {
        self.target = target;
        self.strategy = Self::create_strategy(target);
        self
    }

    pub fn with_source_map(mut self, source_file: String) -> Self {
        self.emitter = self.emitter.with_source_map(source_file);
        self
    }

    pub fn with_mode(mut self, mode: CodeGenMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_optimization_level(mut self, level: crate::config::OptimizationLevel) -> Self {
        self.optimization_level = level;
        self
    }

    pub fn with_whole_program_analysis(
        mut self,
        analysis: crate::optimizer::WholeProgramAnalysis,
    ) -> Self {
        self.whole_program_analysis = Some(analysis);
        self
    }

    /// Enable tree shaking with the given set of reachable exports
    pub fn with_tree_shaking(
        mut self,
        reachable_exports: std::collections::HashSet<String>,
    ) -> Self {
        self.reachable_exports = Some(reachable_exports);
        self.tree_shaking_enabled = true;
        self
    }

    /// Enable or disable tree shaking
    pub fn set_tree_shaking_enabled(&mut self, enabled: bool) {
        self.tree_shaking_enabled = enabled;
    }

    /// Enable or disable scope hoisting for bundles
    pub fn set_scope_hoisting_enabled(&mut self, enabled: bool) {
        self.scope_hoisting_enabled = enabled;
    }

    /// Check if tree shaking is enabled and an export is reachable
    fn is_export_reachable(&self, export_name: &str) -> bool {
        if !self.tree_shaking_enabled {
            return true;
        }
        if let Some(ref exports) = self.reachable_exports {
            exports.contains(export_name)
        } else {
            true
        }
    }

    pub fn generate(&mut self, program: &crate::MutableProgram<'_>) -> String {
        // Emit strategy-specific preamble (e.g., library includes)
        if let Some(preamble) = self.strategy.emit_preamble() {
            self.writeln(&preamble);
            self.writeln("");
        }

        // First pass: check if any decorators are used
        self.detect_decorators_from_statements(&program.statements);

        // Embed runtime library if decorators are used (provides built-in decorators)
        if self.uses_built_in_decorators {
            self.embed_runtime_library();
        }

        for statement in &program.statements {
            self.generate_statement(statement);
        }

        // Finalize module exports if any
        self.finalize_module();

        // Finalize namespace exports if any
        self.finalize_namespace();

        // Generate __TypeRegistry if any types were registered
        if !self.registered_types.is_empty() {
            self.writeln("");
            self.writeln("-- ============================================================");
            self.writeln("-- Type Registry for Reflection");
            self.writeln("-- ============================================================");

            // Generate registry tables (name -> id and id -> class)
            self.writeln("__TypeRegistry = {}");
            self.writeln("__TypeIdToClass = {}");
            self.writeln("");

            // Collect into a Vec to avoid borrow checker issues
            let type_entries: Vec<(String, u32)> = self
                .registered_types
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();

            // Populate __TypeRegistry (name -> id mapping)
            for (type_name, type_id) in &type_entries {
                self.writeln(&format!("__TypeRegistry[\"{}\"] = {}", type_name, type_id));
            }
            self.writeln("");

            // Populate __TypeIdToClass (id -> class constructor mapping)
            for (type_name, type_id) in &type_entries {
                self.writeln(&format!("__TypeIdToClass[{}] = {}", type_id, type_name));
            }
            self.writeln("");

            // Generate Reflect module from runtime
            self.writeln(reflection::REFLECTION_MODULE);
        }

        self.emitter.clone_output()
    }

    /// Generate a bundle from multiple modules
    ///
    /// # Arguments
    /// * `modules` - Vector of (module_id, program, import_map) tuples
    /// * `entry_module_id` - The ID of the entry point module
    /// * `target` - Lua target version
    /// * `with_source_map` - Whether to generate source map
    /// * `output_file` - Optional output file name for source map reference
    /// * `interner` - The string interner used during parsing (required for resolving StringIds)
    /// * `reachable_set` - Optional reachability analysis for tree shaking
    ///
    /// # Returns
    /// Returns a tuple of (generated_code, optional_source_map)
    pub fn generate_bundle<'arena>(
        modules: &[(String, &Program<'arena>, std::collections::HashMap<String, String>)],
        entry_module_id: &str,
        target: LuaTarget,
        with_source_map: bool,
        output_file: Option<String>,
        interner: Option<Arc<StringInterner>>,
        reachable_set: Option<&tree_shaking::ReachableSet>,
    ) -> (String, Option<SourceMap>) {
        Self::generate_bundle_with_options(
            modules,
            entry_module_id,
            target,
            with_source_map,
            output_file,
            interner,
            reachable_set,
            true, // scope_hoisting_enabled by default
        )
    }

    /// Generate a bundle from multiple modules with full options
    ///
    /// # Arguments
    /// * `modules` - Vector of (module_id, program, import_map) tuples
    /// * `entry_module_id` - The ID of the entry point module
    /// * `target` - Lua target version
    /// * `with_source_map` - Whether to generate source map
    /// * `output_file` - Optional output file name for source map reference
    /// * `interner` - The string interner used during parsing (required for resolving StringIds)
    /// * `reachable_set` - Optional reachability analysis for tree shaking
    /// * `scope_hoisting_enabled` - Whether to hoist declarations to top-level scope
    ///
    /// # Returns
    /// Returns a tuple of (generated_code, optional_source_map)
    #[allow(clippy::too_many_arguments)]
    pub fn generate_bundle_with_options<'arena>(
        modules: &[(String, &Program<'arena>, std::collections::HashMap<String, String>)],
        entry_module_id: &str,
        target: LuaTarget,
        with_source_map: bool,
        output_file: Option<String>,
        interner: Option<Arc<StringInterner>>,
        reachable_set: Option<&tree_shaking::ReachableSet>,
        scope_hoisting_enabled: bool,
    ) -> (String, Option<SourceMap>) {
        let mut output = String::new();

        // Initialize source map builder if requested
        let mut source_map_builder = if with_source_map {
            let source_files: Vec<String> = modules.iter().map(|(id, _, _)| id.clone()).collect();
            let mut builder = SourceMapBuilder::new_multi_source(source_files);
            if let Some(ref file) = output_file {
                builder.set_file(file.clone());
            }
            Some(builder)
        } else {
            None
        };

        // Helper macro to advance output and source map
        macro_rules! advance {
            ($text:expr) => {
                output.push_str($text);
                if let Some(ref mut b) = source_map_builder {
                    b.advance($text);
                }
            };
        }

        // Runtime header (no source mappings for runtime code)
        advance!("-- TypedLua Bundle\n");
        advance!("-- Generated by TypedLua compiler\n");
        advance!("\n");
        advance!(module::MODULE_PRELUDE);
        advance!("\n");

        // Build hoisting context if scope hoisting is enabled
        let interner_for_hoisting = interner
            .clone()
            .unwrap_or_else(|| Arc::new(StringInterner::new()));
        let modules_for_analysis: Vec<(String, &Program)> = modules
            .iter()
            .map(|(id, program, _)| (id.clone(), *program))
            .collect();
        let hoisting_context = scope_hoisting::HoistingContext::analyze_modules(
            &modules_for_analysis,
            &interner_for_hoisting,
            entry_module_id,
            scope_hoisting_enabled,
        );

        // Generate hoisted declarations at the top level
        if scope_hoisting_enabled && !hoisting_context.hoistable_by_module.is_empty() {
            advance!("-- Hoisted declarations (scope hoisting)\n");

            // Generate hoisted declarations from each module
            for (module_id, program, _) in modules.iter() {
                if let Some(hoistable) = hoisting_context.get_hoistable_declarations(module_id) {
                    // Generate hoisted functions
                    for stmt in program.statements.iter() {
                        Self::generate_hoisted_declaration_if_needed(
                            stmt,
                            module_id,
                            hoistable,
                            &hoisting_context,
                            &interner_for_hoisting,
                            target,
                            &mut output,
                            &mut source_map_builder,
                        );
                    }
                }
            }

            advance!("\n");
        }

        // Generate each module as a function
        for (source_index, module_id, program, import_map) in modules
            .iter()
            .enumerate()
            .filter(|(_idx, (module_id, _, _))| {
                // Always include the entry module
                if module_id == entry_module_id {
                    return true;
                }

                // If no reachable set provided, include all modules
                if let Some(reachable) = reachable_set {
                    reachable.is_module_reachable(module_id)
                } else {
                    true
                }
            })
            .map(|(idx, item)| (idx, item.0.clone(), item.1, item.2.clone()))
        {
            // Check if this module should be skipped (no reachable exports and not entry)
            if let Some(reachable) = reachable_set {
                if module_id != entry_module_id {
                    if let Some(exports) = reachable.get_reachable_exports(&module_id) {
                        if exports.is_empty() {
                            advance!(&format!(
                                "-- Module: {} (skipped - no reachable exports)\n",
                                module_id
                            ));
                            continue;
                        }
                    }
                }
            }

            advance!(&format!("-- Module: {}\n", module_id));
            advance!(&format!("__modules[\"{}\"] = function()\n", module_id));

            // Record the starting position of this module in the bundle for source map merging
            // Add a mapping for the start of this module (line 0, column 0 of source)
            if let Some(ref mut builder) = source_map_builder {
                builder.add_mapping_with_source(
                    typedlua_parser::span::Span::new(0, 0, 0, 0),
                    source_index,
                    None,
                );
            }
            let module_start_position = source_map_builder.as_ref().map(|b| b.current_position());

            // Generate module code with source map support
            // Use the provided interner or create a new one if not provided
            // Note: Rc used for shared ownership with CodeGenerator; threading not used here
            let interner = interner
                .clone()
                .unwrap_or_else(|| Arc::new(StringInterner::new()));
            let mut generator =
                CodeGenerator::new(interner)
                    .with_target(target)
                    .with_mode(CodeGenMode::Bundle {
                        module_id: module_id.clone(),
                    });

            // Set the import map so imports can be resolved to module IDs
            generator.import_map = import_map.clone();

            // Set source index for this module
            generator.current_source_index = source_index;

            // If we're building a source map, enable it for the generator
            if with_source_map {
                generator.emitter = generator.emitter.with_source_map(module_id.clone());
            }

            let mutable_program = crate::MutableProgram::from_program(program);
            let module_code = generator.generate(&mutable_program);

            // Clone the source map builder from the generator for merging
            let module_source_map_builder = generator.emitter.clone_source_map();

            // Indent the module code and add mappings
            for line in module_code.lines() {
                if !line.is_empty() {
                    advance!("    ");
                }
                advance!(line);
                advance!("\n");
            }

            // Merge module source map mappings into the bundle source map
            if let (Some(ref mut builder), Some(module_builder)) = (
                source_map_builder.as_mut(),
                module_source_map_builder.as_ref(),
            ) {
                if let Some((start_line, start_col)) = module_start_position {
                    // Create source index mapping: module's source index 0 -> bundle's source_index
                    let mut source_index_map = HashMap::default();
                    source_index_map.insert(0, source_index);

                    // Merge the module's mappings with proper offsets
                    // The module code is indented by 4 spaces, so we add 4 to the column offset
                    builder.merge_mappings_from(
                        module_builder,
                        start_line,
                        start_col + 4,
                        &source_index_map,
                    );
                }
            }

            advance!("end\n");
            advance!("\n");
        }

        // Execute entry point
        advance!("-- Execute entry point\n");
        advance!(&format!("__require(\"{}\")\n", entry_module_id));

        let source_map = source_map_builder.map(|builder| builder.build());

        (output, source_map)
    }

    /// Generate a hoisted declaration if it's hoistable
    #[allow(clippy::too_many_arguments)]
    fn generate_hoisted_declaration_if_needed(
        stmt: &Statement,
        module_id: &str,
        hoistable: &scope_hoisting::HoistableDeclarations,
        hoisting_context: &scope_hoisting::HoistingContext,
        interner: &StringInterner,
        target: LuaTarget,
        output: &mut String,
        source_map_builder: &mut Option<SourceMapBuilder>,
    ) {
        let advance = |text: &str, output: &mut String, builder: &mut Option<SourceMapBuilder>| {
            output.push_str(text);
            if let Some(ref mut b) = builder {
                b.advance(text);
            }
        };

        match stmt {
            Statement::Function(func_decl) => {
                let name = interner.resolve(func_decl.name.node).to_string();
                if hoistable.functions.contains(&name) {
                    // Get the mangled name
                    if let Some(mangled_name) = hoisting_context.get_mangled_name(module_id, &name) {
                        // Generate the function with mangled name
                        let mut temp_gen = CodeGenerator::new(Arc::new(interner.clone()))
                            .with_target(target);

                        // Generate function signature with mangled name
                        temp_gen.write("local function ");
                        temp_gen.write(mangled_name);
                        temp_gen.write("(");

                        // Generate parameters
                        let mut has_rest = false;
                        for (i, param) in func_decl.parameters.iter().enumerate() {
                            if param.is_rest {
                                if i > 0 {
                                    temp_gen.write(", ");
                                }
                                temp_gen.write("...");
                                has_rest = true;
                            } else {
                                if i > 0 {
                                    temp_gen.write(", ");
                                }
                                temp_gen.generate_pattern(&param.pattern);
                            }
                        }
                        let _ = has_rest; // silence unused variable warning
                        temp_gen.writeln(")");

                        // Generate body
                        temp_gen.indent();
                        for body_stmt in func_decl.body.statements.iter() {
                            temp_gen.generate_statement(body_stmt);
                        }
                        temp_gen.dedent();
                        temp_gen.writeln("end");

                        let func_code = temp_gen.emitter.clone_output();
                        advance(&func_code, output, source_map_builder);
                    }
                }
            }
            Statement::Variable(var_decl) => {
                if let Pattern::Identifier(ident) = &var_decl.pattern {
                    let name = interner.resolve(ident.node).to_string();
                    if hoistable.variables.contains(&name) {
                        if let Some(mangled_name) =
                            hoisting_context.get_mangled_name(module_id, &name)
                        {
                            // Generate variable with mangled name
                            let mut temp_gen = CodeGenerator::new(Arc::new(interner.clone()))
                                .with_target(target);
                            temp_gen.write("local ");
                            temp_gen.write(mangled_name);
                            temp_gen.write(" = ");
                            temp_gen.generate_expression(&var_decl.initializer);
                            temp_gen.writeln("");

                            let var_code = temp_gen.emitter.clone_output();
                            advance(&var_code, output, source_map_builder);
                        }
                    }
                }
            }
            Statement::Class(class_decl) => {
                let name = interner.resolve(class_decl.name.node).to_string();
                if hoistable.classes.contains(&name) {
                    if let Some(mangled_name) = hoisting_context.get_mangled_name(module_id, &name) {
                        // Generate class with mangled name
                        // For now, generate a simplified class stub
                        // Full class generation would need to handle methods, constructor, etc.
                        let mut temp_gen = CodeGenerator::new(Arc::new(interner.clone()))
                            .with_target(target);

                        // Generate class table
                        temp_gen.write("local ");
                        temp_gen.write(mangled_name);
                        temp_gen.writeln(" = {}");

                        temp_gen.write(mangled_name);
                        temp_gen.write(".__index = ");
                        temp_gen.writeln(mangled_name);

                        // Generate constructor
                        temp_gen.write("function ");
                        temp_gen.write(mangled_name);
                        temp_gen.writeln(".new()");
                        temp_gen.indent();
                        temp_gen.write("local self = setmetatable({}, ");
                        temp_gen.write(mangled_name);
                        temp_gen.writeln(")");
                        temp_gen.writeln("return self");
                        temp_gen.dedent();
                        temp_gen.writeln("end");

                        let class_code = temp_gen.emitter.clone_output();
                        advance(&class_code, output, source_map_builder);
                    }
                }
            }
            Statement::Enum(enum_decl) => {
                let name = interner.resolve(enum_decl.name.node).to_string();
                if hoistable.enums.contains(&name) {
                    if let Some(mangled_name) = hoisting_context.get_mangled_name(module_id, &name) {
                        // Generate enum with mangled name
                        let mut temp_gen = CodeGenerator::new(Arc::new(interner.clone()))
                            .with_target(target);
                        temp_gen.write("local ");
                        temp_gen.write(mangled_name);
                        temp_gen.writeln(" = {");
                        temp_gen.indent();

                        for (i, member) in enum_decl.members.iter().enumerate() {
                            let member_name = temp_gen.resolve(member.name.node);
                            temp_gen.write(&member_name);
                            temp_gen.write(" = ");

                            if let Some(ref value) = member.value {
                                // EnumValue is an enum with Number and String variants
                                match value {
                                    typedlua_parser::ast::statement::EnumValue::Number(n) => {
                                        temp_gen.write(&n.to_string());
                                    }
                                    typedlua_parser::ast::statement::EnumValue::String(s) => {
                                        temp_gen.write("\"");
                                        temp_gen.write(s);
                                        temp_gen.write("\"");
                                    }
                                }
                            } else {
                                temp_gen.write(&format!("{}", i));
                            }

                            if i < enum_decl.members.len() - 1 {
                                temp_gen.write(",");
                            }
                            temp_gen.writeln("");
                        }

                        temp_gen.dedent();
                        temp_gen.writeln("}");

                        let enum_code = temp_gen.emitter.clone_output();
                        advance(&enum_code, output, source_map_builder);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn take_source_map(&mut self) -> Option<SourceMap> {
        self.emitter.take_source_map()
    }

    fn write(&mut self, s: &str) {
        self.emitter.write(s);
    }

    fn writeln(&mut self, s: &str) {
        self.emitter.writeln(s);
    }

    fn indent(&mut self) {
        self.emitter.indent();
    }

    fn dedent(&mut self) {
        self.emitter.dedent();
    }

    fn write_indent(&mut self) {
        self.emitter.write_indent();
    }

    fn finalize_module(&mut self) {
        if self.current_namespace.is_some() {
            return;
        }

        if !self.exports.is_empty() || self.has_default_export {
            self.writeln("");
            self.writeln("local M = {}");

            let exports = self.exports.clone();
            for name in &exports {
                self.write("M.");
                self.write(name);
                self.write(" = ");
                self.writeln(name);
            }

            if self.has_default_export {
                self.writeln("M.default = _default");
            }

            self.writeln("return M");
        }
    }

    fn finalize_namespace(&mut self) {
        // Clone namespace path to avoid borrow checker issues
        let ns_path_clone = self.current_namespace.clone();

        if let Some(ns_path) = ns_path_clone {
            if !ns_path.is_empty() {
                let ns_full_path = ns_path.join(".");
                let ns_root = ns_path[0].clone();

                // Assign exports to namespace
                if !self.exports.is_empty() || self.has_default_export {
                    self.writeln("");

                    let exports = self.exports.clone();
                    for name in &exports {
                        self.write_indent();
                        self.write(&ns_full_path);
                        self.write(".");
                        self.write(name);
                        self.write(" = ");
                        self.writeln(name);
                    }

                    if self.has_default_export {
                        self.write_indent();
                        self.write(&ns_full_path);
                        self.writeln(".default = _default");
                    }
                }

                self.writeln("");
                self.write("return ");
                self.writeln(&ns_root);
            }
        }
    }

    fn get_declaration_name(
        &self,
        stmt: &Statement,
    ) -> Option<typedlua_parser::string_interner::StringId> {
        match stmt {
            Statement::Variable(decl) => {
                if let Pattern::Identifier(ident) = &decl.pattern {
                    Some(ident.node)
                } else {
                    None
                }
            }
            Statement::Function(decl) => Some(decl.name.node),
            Statement::Class(decl) => Some(decl.name.node),
            Statement::Interface(decl) => Some(decl.name.node),
            Statement::TypeAlias(decl) => Some(decl.name.node),
            Statement::Enum(decl) => Some(decl.name.node),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CodeGenerator;
    use super::LuaTarget;
    use crate::codegen::strategies::CodeGenStrategy;
    use crate::codegen::strategies::{
        lua51::Lua51Strategy, lua52::Lua52Strategy, lua53::Lua53Strategy,
    };
    use crate::diagnostics::CollectingDiagnosticHandler;
    use crate::MutableProgram;
    use bumpalo::Bump;
    use std::sync::Arc;
    use typedlua_parser::ast::expression::BinaryOp;
    use typedlua_parser::lexer::Lexer;
    use typedlua_parser::parser::Parser;
    use typedlua_parser::string_interner::StringInterner;

    fn generate_code(source: &str) -> String {
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let interner = Arc::new(interner);
        let arena = Bump::new();
        let mut lexer = Lexer::new(source, handler.clone(), &interner);
        let tokens = lexer.tokenize().expect("Lexing failed");
        let mut parser = Parser::new(tokens, handler, &interner, &common, &arena);
        let program = parser.parse().expect("Parsing failed");
        let mutable = MutableProgram::from_program(&program);

        let mut generator = CodeGenerator::new(interner.clone());
        generator.generate(&mutable)
    }

    #[test]
    fn test_generate_variable_declaration() {
        let source = "const x = 42";
        let output = generate_code(source);
        assert!(output.contains("local x = 42"));
    }

    #[test]
    fn test_generate_function_declaration() {
        let source = "function add(a, b) return a + b end";
        let output = generate_code(source);
        assert!(output.contains("local function add(a, b)"));
        assert!(output.contains("return (a + b)"));
    }

    #[test]
    fn test_generate_if_statement() {
        let source = "if x > 0 then print(x) end";
        let output = generate_code(source);
        assert!(output.contains("if (x > 0) then"));
        assert!(output.contains("print(x)"));
        assert!(output.contains("end"));
    }

    #[test]
    fn test_generate_while_loop() {
        let source = "while x < 10 do x = x + 1 end";
        let output = generate_code(source);
        assert!(output.contains("while (x < 10) do"));
        // Assignment is parsed and will be in the output
        assert!(!output.trim().is_empty());
    }

    #[test]
    fn test_generate_for_loop() {
        let source = "for i = 1, 10 do print(i) end";
        let output = generate_code(source);
        assert!(output.contains("for i = 1, 10 do"));
        assert!(output.contains("print(i)"));
    }

    fn generate_code_with_target(source: &str, target: LuaTarget) -> String {
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let interner = Arc::new(interner);
        let arena = Bump::new();
        let mut lexer = Lexer::new(source, handler.clone(), &interner);
        let tokens = lexer.tokenize().expect("Lexing failed");
        let mut parser = Parser::new(tokens, handler, &interner, &common, &arena);
        let program = parser.parse().expect("Parsing failed");
        let mutable = MutableProgram::from_program(&program);

        let mut generator = CodeGenerator::new(interner.clone()).with_target(target);
        generator.generate(&mutable)
    }

    #[test]
    fn test_bitwise_lua53() {
        let source = "const x = a & b";
        let output = generate_code_with_target(source, LuaTarget::Lua53);
        assert!(output.contains("local x = (a & b)"));
    }

    #[test]
    fn test_bitwise_lua52() {
        let source = "const x = a & b";
        let output = generate_code_with_target(source, LuaTarget::Lua52);
        assert!(output.contains("local x = bit32.band(a, b)"));
    }

    #[test]
    fn test_bitwise_lua51() {
        let source = "const x = a | b";
        let output = generate_code_with_target(source, LuaTarget::Lua51);
        // Should use helper function for Lua 5.1
        assert!(output.contains("local x = _bit_bor(a, b)"));
    }

    #[test]
    fn test_shift_operators_lua53() {
        let source = "const x = a << 2";
        let output = generate_code_with_target(source, LuaTarget::Lua53);
        assert!(output.contains("local x = (a << 2)"));

        let source = "const y = a >> 2";
        let output = generate_code_with_target(source, LuaTarget::Lua53);
        assert!(output.contains("local y = (a >> 2)"));
    }

    #[test]
    fn test_shift_operators_lua52() {
        let source = "const x = a << 2";
        let output = generate_code_with_target(source, LuaTarget::Lua52);
        assert!(output.contains("local x = bit32.lshift(a, 2)"));

        let source = "const y = a >> 2";
        let output = generate_code_with_target(source, LuaTarget::Lua52);
        assert!(output.contains("local y = bit32.rshift(a, 2)"));
    }

    #[test]
    fn test_integer_divide_lua53() {
        let source = "const x = a // b";
        let output = generate_code_with_target(source, LuaTarget::Lua53);
        assert!(output.contains("local x = (a // b)"));
    }

    #[test]
    fn test_integer_divide_lua52() {
        let source = "const x = a // b";
        let output = generate_code_with_target(source, LuaTarget::Lua52);
        assert!(output.contains("local x = math.floor(a / b)"));
    }

    #[test]
    fn test_integer_divide_lua51() {
        let source = "const x = a // b";
        let output = generate_code_with_target(source, LuaTarget::Lua51);
        assert!(output.contains("local x = math.floor(a / b)"));
    }

    // Test that target selection works with currently supported operators
    #[test]
    fn test_target_selection() {
        let source = "const x = a + b";

        // Should work with any target
        let output_54 = generate_code_with_target(source, LuaTarget::Lua54);
        assert!(output_54.contains("local x = (a + b)"));

        let output_53 = generate_code_with_target(source, LuaTarget::Lua53);
        assert!(output_53.contains("local x = (a + b)"));

        let output_52 = generate_code_with_target(source, LuaTarget::Lua52);
        assert!(output_52.contains("local x = (a + b)"));

        let output_51 = generate_code_with_target(source, LuaTarget::Lua51);
        assert!(output_51.contains("local x = (a + b)"));
    }

    // Snapshot tests for generated code
    #[test]
    fn test_snapshot_variable_declarations() {
        let source = r#"
const x = 42
let y = "hello"
const z = true
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_function_declaration() {
        let source = r#"
function greet(name: string): string
    return "Hello, " .. name
end
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_arrow_function() {
        let source = r#"
const add = (a, b) => a + b
const multiply = (x, y) => {
    return x * y
}
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_control_flow() {
        let source = r#"
if x > 0 then
    print("positive")
elseif x < 0 then
    print("negative")
else
    print("zero")
end

while count < 10 do
    count = count + 1
end

for i = 1, 10 do
    print(i)
end
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_bitwise_lua53() {
        let source = r#"
const a = x & y
const b = x | y
const c = x ~ y
const d = x << 2
const e = x >> 3
const f = x // y
"#;
        let output = generate_code_with_target(source, LuaTarget::Lua53);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_bitwise_lua52() {
        let source = r#"
const a = x & y
const b = x | y
const c = x ~ y
const d = x << 2
const e = x >> 3
const f = x // y
"#;
        let output = generate_code_with_target(source, LuaTarget::Lua52);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_bitwise_lua51() {
        let source = r#"
const a = x & y
const b = x | y
const c = x ~ y
const d = x << 2
const e = x >> 3
const f = x // y
"#;
        let output = generate_code_with_target(source, LuaTarget::Lua51);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_complex_expressions() {
        let source = r#"
const result = (a + b) * (c - d) / e
const comparison = x >= y and z < w
const ternary = condition ? value1 : value2
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_arrays_and_objects() {
        let source = r#"
const arr = [1, 2, 3, 4, 5]
const obj = { name = "John", age = 30, active = true }
const nested = { data = [1, 2], meta = { version = 1 } }
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_string_literals() {
        let source = r#"
const simple = "hello"
const withEscape = "hello \"world\""
const withBackslash = "path\\to\\file"
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    // Roundtrip tests: parse → generate → parse
    #[test]
    fn test_roundtrip_variable_declaration() {
        let source = "const x = 42";
        let generated = generate_code(source);

        // Parse the generated code again - should not panic
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let arena = Bump::new();
        let mut lexer = Lexer::new(&generated, handler.clone(), &interner);
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler, &interner, &common, &arena);
        let _program = parser.parse().expect("Roundtrip parsing failed");
    }

    fn parse_roundtrip(source: &str) {
        let generated = generate_code(source);

        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let arena = Bump::new();
        let mut lexer = Lexer::new(&generated, handler.clone(), &interner);
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler, &interner, &common, &arena);
        let _program = parser.parse().expect("Roundtrip parsing failed");
    }

    #[test]
    fn test_roundtrip_function_declaration() {
        let source = "function add(a, b) return a + b end";
        parse_roundtrip(source);
    }

    #[test]
    fn test_roundtrip_control_flow() {
        let source = r#"
if x > 0 then
    print("positive")
end

while count < 10 do
    count = count + 1
end

for i = 1, 10 do
    print(i)
end
"#;
        parse_roundtrip(source);
    }

    #[test]
    fn test_roundtrip_binary_expressions() {
        let source = "const x = (a + b) * (c - d) / e";
        parse_roundtrip(source);
    }

    #[test]
    fn test_roundtrip_arrays_and_objects() {
        let source = r#"
const arr = [1, 2, 3, 4, 5]
const obj = { name = "John", age = 30 }
"#;
        parse_roundtrip(source);
    }

    #[test]
    fn test_roundtrip_bitwise_lua53() {
        let source = "const x = a & b | c ^ d << 2 >> 1";
        let output = generate_code_with_target(source, LuaTarget::Lua53);

        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let arena = Bump::new();
        let mut lexer = Lexer::new(&output, handler.clone(), &interner);
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler, &interner, &common, &arena);
        let _program = parser.parse().expect("Roundtrip parsing failed");
    }

    #[test]
    fn test_roundtrip_parent_class() {
        let source = r#"
class Animal {
    var name: string

    function constructor(name: string)
        self.name = name
    end

    function speak()
        print("...")
    end
end

class Dog < Animal
    function speak()
        print("woof!")
    end
end
"#;
        parse_roundtrip(source);
    }

    // ========== Class Code Generation Tests ==========

    #[test]
    fn test_simple_class() {
        let source = r#"
            class Person {
            }
        "#;
        let output = generate_code(source);

        assert!(output.contains("local Person = {}"));
        assert!(output.contains("Person.__index = Person"));
        assert!(output.contains("function Person.new()"));
        assert!(output.contains("local self = setmetatable({}, Person)"));
        assert!(output.contains("return self"));
    }

    #[test]
    fn test_class_with_constructor() {
        let source = r#"
            class Person {
                constructor(name: string, age: number) {
                    const x: number = 5
                }
            }
        "#;
        let output = generate_code(source);

        assert!(output.contains("local Person = {}"));
        assert!(output.contains("Person.__index = Person"));
        assert!(output.contains("function Person.new(name, age)"));
        assert!(output.contains("local self = setmetatable({}, Person)"));
        assert!(output.contains("return self"));
    }

    #[test]
    fn test_class_with_method() {
        let source = r#"
            class Calculator {
                add(a: number, b: number): number {
                    return a + b
                }
            }
        "#;
        let output = generate_code(source);

        assert!(output.contains("local Calculator = {}"));
        assert!(output.contains("Calculator.__index = Calculator"));
        assert!(output.contains("function Calculator:add(a, b)"));
        assert!(output.contains("return (a + b)"));
    }

    #[test]
    fn test_class_with_static_method() {
        let source = r#"
            class Math {
                static abs(x: number): number {
                    return x
                }
            }
        "#;
        let output = generate_code(source);

        assert!(output.contains("local Math = {}"));
        assert!(output.contains("Math.__index = Math"));
        // Static methods use . instead of :
        assert!(output.contains("function Math.abs(x)"));
        assert!(output.contains("return x"));
    }

    #[test]
    fn test_class_with_constructor_and_methods() {
        let source = r#"
            class Counter {
                constructor(initial: number) {
                    const x: number = initial
                }

                increment(): void {
                    const y: number = 1
                }

                getValue(): number {
                    return 0
                }
            }
        "#;
        let output = generate_code(source);

        assert!(output.contains("local Counter = {}"));
        assert!(output.contains("Counter.__index = Counter"));
        assert!(output.contains("function Counter.new(initial)"));
        assert!(output.contains("function Counter:increment()"));
        assert!(output.contains("function Counter:getValue()"));
    }

    #[test]
    fn test_abstract_class() {
        let source = r#"
            abstract class Animal {
                abstract makeSound(): string;

                move(): void {
                    const x: number = 5
                }
            }
        "#;
        let output = generate_code(source);

        assert!(output.contains("local Animal = {}"));
        assert!(output.contains("Animal.__index = Animal"));
        // Abstract methods should not generate function bodies
        assert!(!output.contains("function Animal:makeSound"));
        // Concrete methods should be generated
        assert!(output.contains("function Animal:move()"));
    }

    #[test]
    fn test_class_with_getter_setter() {
        let source = r#"
            class Person {
                get fullName(): string {
                    return "John Doe"
                }

                set age(value: number) {
                    const x: number = value
                }
            }
        "#;
        let output = generate_code(source);

        assert!(output.contains("local Person = {}"));
        assert!(output.contains("function Person:get_fullName()"));
        assert!(output.contains("function Person:set_age(value)"));
    }

    #[test]
    fn test_class_inheritance() {
        let source = r#"
            class Animal {
                name: string

                constructor(name: string) {
                    self.name = name
                }

                speak(): string {
                    return "Animal sound"
                }
            }

            class Dog extends Animal {
                breed: string

                constructor(name: string, breed: string) {
                    self.name = name
                    self.breed = breed
                }

                speak(): string {
                    return "Woof!"
                }
            }
        "#;
        let output = generate_code(source);

        // Check Animal class
        assert!(output.contains("local Animal = {}"));
        assert!(output.contains("Animal.__index = Animal"));
        assert!(output.contains("function Animal.new(name)"));
        assert!(output.contains("function Animal:speak()"));

        // Check Dog class with inheritance
        assert!(output.contains("local Dog = {}"));
        assert!(output.contains("Dog.__index = Dog"));
        assert!(output.contains("setmetatable(Dog, { __index = Animal })"));
        assert!(output.contains("function Dog.new(name, breed)"));
        assert!(output.contains("function Dog:speak()"));
    }

    #[test]
    fn test_super_calls() {
        let source = r#"
            class Animal {
                name: string

                constructor(name: string) {
                    self.name = name
                }

                speak(): string {
                    return "Animal sound"
                }

                move(): void {
                    const x: number = 1
                }
            }

            class Dog extends Animal {
                breed: string

                constructor(name: string, breed: string) {
                    self.name = name
                    self.breed = breed
                }

                speak(): string {
                    const baseSound: string = super.speak()
                    return baseSound .. " and Woof!"
                }

                wagTail(): void {
                    super.move()
                    const y: number = 2
                }
            }
        "#;
        let output = generate_code(source);

        // Check that super.speak() is translated to Animal.speak(self)
        assert!(output.contains("Animal.speak(self)"));
        // Check that super.move() is translated to Animal.move(self)
        assert!(output.contains("Animal.move(self)"));
        // Check inheritance setup
        assert!(output.contains("setmetatable(Dog, { __index = Animal })"));
    }

    #[test]
    fn test_super_constructor_chaining() {
        let source = r#"
            class Animal {
                name: string

                constructor(name: string) {
                    self.name = name
                }
            }

            class Dog extends Animal {
                breed: string

                constructor(name: string, breed: string) {
                    super(name)
                    self.breed = breed
                }
            }
        "#;
        let output = generate_code(source);

        // Check that Animal has _init method
        assert!(output.contains("function Animal._init(self, name)"));
        // Check that Animal.new calls Animal._init
        assert!(output.contains("Animal._init(self, name)"));
        // Check that Dog has _init method
        assert!(output.contains("function Dog._init(self, name, breed)"));
        // Check that super(name) is translated to Animal._init(self, name)
        assert!(output.contains("Animal._init(self, name)"));
        // Check that Dog.new calls Dog._init
        assert!(output.contains("Dog._init(self, name, breed)"));
    }

    // Strategy Pattern Tests
    #[test]
    fn test_lua51_strategy_name() {
        let strategy = Lua51Strategy;
        assert_eq!(strategy.name(), "Lua 5.1");
    }

    #[test]
    fn test_lua51_bitwise_operator_generation() {
        let strategy = Lua51Strategy;
        let x = "x";
        let y = "y";

        let result = strategy.generate_bitwise_op(BinaryOp::BitwiseAnd, x, y);
        assert_eq!(result, "_bit_band(x, y)");

        let result = strategy.generate_bitwise_op(BinaryOp::BitwiseOr, x, y);
        assert_eq!(result, "_bit_bor(x, y)");

        let result = strategy.generate_bitwise_op(BinaryOp::BitwiseXor, x, y);
        assert_eq!(result, "_bit_bxor(x, y)");

        let result = strategy.generate_bitwise_op(BinaryOp::ShiftLeft, x, "2");
        assert_eq!(result, "_bit_lshift(x, 2)");

        let result = strategy.generate_bitwise_op(BinaryOp::ShiftRight, x, "3");
        assert_eq!(result, "_bit_rshift(x, 3)");
    }

    #[test]
    fn test_lua51_integer_division() {
        let strategy = Lua51Strategy;
        let result = strategy.generate_integer_divide("x", "y");
        assert_eq!(result, "math.floor(x / y)");
    }

    #[test]
    fn test_lua51_unary_bitwise_not() {
        let strategy = Lua51Strategy;
        let result = strategy.generate_unary_bitwise_not("x");
        assert_eq!(result, "_bit_bnot(x)");
    }

    #[test]
    fn test_lua51_supports_native_features() {
        let strategy = Lua51Strategy;
        assert!(!strategy.supports_native_bitwise());
        assert!(!strategy.supports_native_integer_divide());
    }

    #[test]
    fn test_lua51_emits_preamble() {
        let strategy = Lua51Strategy;
        let preamble = strategy.emit_preamble();
        assert!(preamble.is_some());
        let preamble_text = preamble.unwrap();
        assert!(preamble_text.contains("local function _bit_band"));
        assert!(preamble_text.contains("local function _bit_bor"));
        assert!(preamble_text.contains("local function _bit_bnot"));
    }

    #[test]
    fn test_lua52_bitwise_operators() {
        let strategy = Lua52Strategy;
        let x = "x";
        let y = "y";

        let result = strategy.generate_bitwise_op(BinaryOp::BitwiseAnd, x, y);
        assert_eq!(result, "bit32.band(x, y)");

        let result = strategy.generate_bitwise_op(BinaryOp::BitwiseOr, x, y);
        assert_eq!(result, "bit32.bor(x, y)");

        let result = strategy.generate_bitwise_op(BinaryOp::BitwiseXor, x, y);
        assert_eq!(result, "bit32.bxor(x, y)");
    }

    #[test]
    fn test_lua53_native_bitwise_operators() {
        let strategy = Lua53Strategy;
        let x = "x";
        let y = "y";

        let result = strategy.generate_bitwise_op(BinaryOp::BitwiseAnd, x, y);
        assert_eq!(result, "(x & y)");

        let result = strategy.generate_bitwise_op(BinaryOp::BitwiseOr, x, y);
        assert_eq!(result, "(x | y)");

        let result = strategy.generate_bitwise_op(BinaryOp::BitwiseXor, x, y);
        assert_eq!(result, "(x ~ y)");

        let result = strategy.generate_bitwise_op(BinaryOp::ShiftLeft, x, "2");
        assert_eq!(result, "(x << 2)");

        let result = strategy.generate_bitwise_op(BinaryOp::ShiftRight, x, "3");
        assert_eq!(result, "(x >> 3)");
    }

    #[test]
    fn test_lua53_native_integer_division() {
        let strategy = Lua53Strategy;
        let result = strategy.generate_integer_divide("x", "y");
        assert_eq!(result, "(x // y)");
    }

    #[test]
    fn test_lua53_unary_bitwise_not() {
        let strategy = Lua53Strategy;
        let result = strategy.generate_unary_bitwise_not("x");
        assert_eq!(result, "~x");
    }

    #[test]
    fn test_lua53_supports_native_features() {
        let strategy = Lua53Strategy;
        assert!(strategy.supports_native_bitwise());
        assert!(strategy.supports_native_integer_divide());
    }
}
