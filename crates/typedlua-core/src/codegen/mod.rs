pub mod sourcemap;

use crate::config::OptimizationLevel;
use crate::optimizer::Optimizer;
pub use sourcemap::{SourceMap, SourceMapBuilder};
use std::sync::Arc;
use typedlua_parser::ast::expression::*;
use typedlua_parser::ast::pattern::{ArrayPattern, ArrayPatternElement, ObjectPattern, Pattern};
use typedlua_parser::ast::statement::*;
use typedlua_parser::ast::Program;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::{StringId, StringInterner};
use typedlua_runtime::bitwise;
use typedlua_runtime::class;
use typedlua_runtime::decorator;
use typedlua_runtime::enum_rt;
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

impl LuaTarget {
    /// Check if this target supports bitwise operators
    pub fn supports_bitwise_ops(self) -> bool {
        matches!(self, LuaTarget::Lua53 | LuaTarget::Lua54)
    }

    /// Check if this target supports integer division (//)
    pub fn supports_integer_divide(self) -> bool {
        matches!(self, LuaTarget::Lua53 | LuaTarget::Lua54)
    }

    /// Check if this target supports goto/labels
    pub fn supports_goto(self) -> bool {
        !matches!(self, LuaTarget::Lua51)
    }

    /// Check if this target supports the continue statement
    pub fn supports_continue(self) -> bool {
        // None of the standard Lua versions support continue
        // Would need to be emulated
        false
    }

    /// Get the bitwise operator library name for Lua 5.2
    pub fn bit_library_name(self) -> Option<&'static str> {
        match self {
            LuaTarget::Lua52 => Some("bit32"),
            _ => None,
        }
    }
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
    output: String,
    indent_level: usize,
    indent_str: String,
    source_map: Option<SourceMapBuilder>,
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
}

impl CodeGenerator {
    pub fn new(interner: Arc<StringInterner>) -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            indent_str: "    ".to_string(),
            source_map: None,
            target: LuaTarget::default(),
            current_class_parent: None,
            uses_built_in_decorators: false,
            mode: CodeGenMode::Require,
            exports: Vec::new(),
            has_default_export: false,
            import_map: std::collections::HashMap::new(),
            current_source_index: 0,
            interner,
            optimization_level: crate::config::OptimizationLevel::O0,
            interface_default_methods: std::collections::HashMap::new(),
            current_namespace: None,
            namespace_exports: Vec::new(),
            next_type_id: 1,
            registered_types: std::collections::HashMap::new(),
        }
    }

    /// Resolve a StringId to a String
    fn resolve(&self, id: typedlua_parser::string_interner::StringId) -> String {
        self.interner.resolve(id).to_string()
    }

    pub fn with_target(mut self, target: LuaTarget) -> Self {
        self.target = target;
        self
    }

    pub fn with_source_map(mut self, source_file: String) -> Self {
        self.source_map = Some(SourceMapBuilder::new(source_file));
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

    pub fn generate(&mut self, program: &mut Program) -> String {
        // Run optimizer before code generation if optimization level > O0
        if self.optimization_level != crate::config::OptimizationLevel::O0 {
            let handler = Arc::new(crate::diagnostics::CollectingDiagnosticHandler::new());
            let mut optimizer =
                Optimizer::new(self.optimization_level, handler, self.interner.clone());
            let _ = optimizer.optimize(program);
        }

        // First pass: check if any decorators are used
        self.detect_decorators(program);

        // Embed runtime library if decorators are used (provides built-in decorators)
        if self.uses_built_in_decorators {
            self.embed_runtime_library();
        }

        // Always embed bitwise helpers for Lua 5.1
        // (They are small and only defined if used due to local function scope)
        if self.target == LuaTarget::Lua51 {
            self.embed_bitwise_helpers();
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
            self.writeln("__TypeRegistry = {");
            self.indent();
            // Collect into a Vec to avoid borrow checker issues
            let type_entries: Vec<(String, u32)> = self
                .registered_types
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            for (type_name, type_id) in type_entries {
                self.write_indent();
                self.writeln(&format!("[\"{}\"] = {},", type_name, type_id));
            }
            self.dedent();
            self.writeln("}");
            self.writeln("");

            // Generate Reflect module from runtime
            self.writeln(reflection::REFLECTION_MODULE);
        }

        self.output.clone()
    }

    /// Generate a bundle from multiple modules
    ///
    /// This creates a single Lua file with a custom module system that:
    /// - Stores all modules as functions in __modules table
    /// - Implements __require() with caching
    /// - Executes the entry point module
    ///
    /// # Parameters
    /// * `modules` - List of (module_id, program, import_map) tuples
    ///   - `module_id` - Canonical path identifier for the module
    ///   - `program` - The parsed AST
    ///   - `import_map` - Map from import source strings to resolved module IDs
    /// * `entry_module_id` - The module ID to execute as entry point
    /// * `target` - Target Lua version
    /// * `with_source_map` - Whether to generate source map
    /// * `output_file` - Optional output file name for source map reference
    ///
    /// # Returns
    /// Returns a tuple of (generated_code, optional_source_map)
    pub fn generate_bundle(
        modules: &[(String, &Program, std::collections::HashMap<String, String>)],
        entry_module_id: &str,
        target: LuaTarget,
        with_source_map: bool,
        output_file: Option<String>,
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

        // Helper to advance source map
        let mut advance = |text: &str, builder: &mut Option<SourceMapBuilder>| {
            output.push_str(text);
            if let Some(ref mut b) = builder {
                b.advance(text);
            }
        };

        // Runtime header (no source mappings for runtime code)
        advance("-- TypedLua Bundle\n", &mut source_map_builder);
        advance(
            "-- Generated by TypedLua compiler\n",
            &mut source_map_builder,
        );
        advance("\n", &mut source_map_builder);
        advance(module::MODULE_PRELUDE, &mut source_map_builder);
        advance("\n", &mut source_map_builder);

        // Generate each module as a function
        for (source_index, (module_id_ref, program_ref, import_map_ref)) in
            modules.iter().enumerate()
        {
            let mut program: Program = (*program_ref).clone();
            let import_map = import_map_ref.clone();
            let module_id = module_id_ref.clone();
            advance(
                &format!("-- Module: {}\n", module_id),
                &mut source_map_builder,
            );
            advance(
                &format!("__modules[\"{}\"] = function()\n", module_id),
                &mut source_map_builder,
            );

            // Add a mapping for the start of this module (line 0, column 0 of source)
            if let Some(ref mut builder) = source_map_builder {
                builder.add_mapping_with_source(
                    typedlua_parser::span::Span::new(0, 0, 0, 0),
                    source_index,
                    None,
                );
            }

            // Generate module code with source map support
            // Create a new interner for this module (bundles can have multiple independent modules)
            // Note: Arc used for shared ownership with CodeGenerator; threading not used here
            #[allow(clippy::arc_with_non_send_sync)]
            let interner = Arc::new(StringInterner::new());
            let mut generator = CodeGenerator::new(interner.clone())
                .with_target(target)
                .with_mode(CodeGenMode::Bundle {
                    module_id: module_id.clone(),
                });

            // Set the import map so imports can be resolved to module IDs
            generator.import_map = import_map;

            // Set source index for this module
            generator.current_source_index = source_index;

            // If we're building a source map, enable it for the generator
            if with_source_map {
                // Create a temporary source map builder for this module
                generator.source_map = Some(SourceMapBuilder::new(module_id.clone()));
            }

            let (module_code, _module_source_map) = {
                let mut gen = generator;
                let code = gen.generate(&mut program);
                let source_map = if with_source_map {
                    gen.take_source_map()
                } else {
                    None
                };
                (code, source_map)
            };

            // Indent the module code and add mappings
            for line in module_code.lines() {
                if !line.is_empty() {
                    advance("    ", &mut source_map_builder);
                }
                advance(line, &mut source_map_builder);
                advance("\n", &mut source_map_builder);
            }

            // TODO: Merge module_mappings into source_map_builder with proper source_index
            // This is complex because we need to offset the generated line/column positions
            // and update the source_index for each mapping
            //
            // KNOWN LIMITATION (Low Priority):
            // Bundle mode source maps currently don't accurately map back to individual module
            // source files. The source_map_builder tracks positions in the bundled output, but
            // the module_mappings (which map back to original files) are not merged in.
            // This means debugging bundled code will show the bundle structure rather than
            // the original file locations. Single-file compilation works correctly.
            //
            // To fix: Need to transform each mapping in module_mappings by:
            //   1. Adding the line offset of where the module appears in the bundle
            //   2. Adjusting column positions for the module wrapper indentation
            //   3. Setting the correct source_index for each module's original file
            //   4. Inserting these transformed mappings into source_map_builder

            advance("end\n", &mut source_map_builder);
            advance("\n", &mut source_map_builder);
        }

        // Execute entry point
        advance("-- Execute entry point\n", &mut source_map_builder);
        advance(
            &format!("__require(\"{}\")\n", entry_module_id),
            &mut source_map_builder,
        );

        let source_map = source_map_builder.map(|builder| builder.build());

        (output, source_map)
    }

    pub fn take_source_map(&mut self) -> Option<SourceMap> {
        self.source_map.take().map(|builder| builder.build())
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
        if let Some(source_map) = &mut self.source_map {
            source_map.advance(s);
        }
    }

    fn writeln(&mut self, s: &str) {
        self.output.push_str(s);
        self.output.push('\n');
        if let Some(source_map) = &mut self.source_map {
            source_map.advance(s);
            source_map.advance("\n");
        }
    }

    fn indent(&mut self) {
        self.indent_level += 1;
    }

    fn dedent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.write(&self.indent_str.clone());
        }
    }

    fn generate_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Variable(decl) => self.generate_variable_declaration(decl),
            Statement::Function(decl) => self.generate_function_declaration(decl),
            Statement::If(if_stmt) => self.generate_if_statement(if_stmt),
            Statement::While(while_stmt) => self.generate_while_statement(while_stmt),
            Statement::For(for_stmt) => self.generate_for_statement(for_stmt.as_ref()),
            Statement::Repeat(repeat_stmt) => self.generate_repeat_statement(repeat_stmt),
            Statement::Return(return_stmt) => self.generate_return_statement(return_stmt),
            Statement::Break(_) => {
                self.write_indent();
                self.writeln("break");
            }
            Statement::Continue(_) => {
                self.write_indent();
                self.writeln("continue");
            }
            Statement::Expression(expr) => {
                self.write_indent();
                self.generate_expression(expr);
                self.writeln("");
            }
            Statement::Block(block) => self.generate_block(block),
            Statement::Interface(iface_decl) => self.generate_interface_declaration(iface_decl),
            Statement::TypeAlias(_) => {
                // Type-only declarations are erased
            }
            Statement::Enum(decl) => self.generate_enum_declaration(decl),
            Statement::Class(class_decl) => self.generate_class_declaration(class_decl),
            Statement::Import(import) => self.generate_import(import),
            Statement::Export(export) => self.generate_export(export),
            // Declaration file statements - these are type-only and erased
            Statement::DeclareFunction(_)
            | Statement::DeclareNamespace(_)
            | Statement::DeclareType(_)
            | Statement::DeclareInterface(_)
            | Statement::DeclareConst(_) => {
                // Declaration file statements are erased during code generation
            }
            Statement::Throw(throw_stmt) => self.generate_throw_statement(throw_stmt),
            Statement::Try(try_stmt) => self.generate_try_statement(try_stmt),
            Statement::Rethrow(span) => self.generate_rethrow_statement(*span),
            Statement::Namespace(ns) => self.generate_namespace_declaration(ns),
        }
    }

    fn generate_variable_declaration(&mut self, decl: &VariableDeclaration) {
        match &decl.pattern {
            Pattern::Identifier(_) | Pattern::Wildcard(_) => {
                // Simple case: local name = value
                self.write_indent();
                self.write("local ");
                self.generate_pattern(&decl.pattern);
                self.write(" = ");
                self.generate_expression(&decl.initializer);
                self.writeln("");
            }
            Pattern::Array(array_pattern) => {
                // Generate temporary variable and destructuring assignments
                self.write_indent();
                self.write("local __temp = ");
                self.generate_expression(&decl.initializer);
                self.writeln("");
                self.generate_array_destructuring(array_pattern, "__temp");
            }
            Pattern::Object(obj_pattern) => {
                // Generate temporary variable and destructuring assignments
                self.write_indent();
                self.write("local __temp = ");
                self.generate_expression(&decl.initializer);
                self.writeln("");
                self.generate_object_destructuring(obj_pattern, "__temp");
            }
            Pattern::Literal(_, _) => {
                // Literals in patterns don't bind variables - just evaluate the initializer
                self.write_indent();
                self.write("local _ = ");
                self.generate_expression(&decl.initializer);
                self.writeln("");
            }
        }
    }

    fn generate_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Identifier(ident) => {
                let resolved = self.resolve(ident.node);
                self.write(&resolved);
            }
            Pattern::Wildcard(_) => {
                self.write("_");
            }
            Pattern::Array(_) | Pattern::Object(_) | Pattern::Literal(_, _) => {
                // These should be handled by generate_variable_declaration or other contexts
                self.write("_destructure");
            }
        }
    }

    /// Generate array destructuring assignments
    fn generate_array_destructuring(&mut self, pattern: &ArrayPattern, source: &str) {
        let mut index = 1; // Lua arrays are 1-indexed

        for elem in &pattern.elements {
            match elem {
                ArrayPatternElement::Pattern(pat) => {
                    match pat {
                        Pattern::Identifier(ident) => {
                            self.write_indent();
                            self.write("local ");
                            let resolved = self.resolve(ident.node);
                            self.write(&resolved);
                            self.write(&format!(" = {}[{}]", source, index));
                            self.writeln("");
                        }
                        Pattern::Array(nested_array) => {
                            // Nested array destructuring
                            let temp_var = format!("__temp_{}", index);
                            self.write_indent();
                            self.write("local ");
                            self.write(&temp_var);
                            self.write(&format!(" = {}[{}]", source, index));
                            self.writeln("");
                            self.generate_array_destructuring(nested_array, &temp_var);
                        }
                        Pattern::Object(nested_obj) => {
                            // Nested object destructuring
                            let temp_var = format!("__temp_{}", index);
                            self.write_indent();
                            self.write("local ");
                            self.write(&temp_var);
                            self.write(&format!(" = {}[{}]", source, index));
                            self.writeln("");
                            self.generate_object_destructuring(nested_obj, &temp_var);
                        }
                        Pattern::Wildcard(_) | Pattern::Literal(_, _) => {
                            // Skip - don't bind anything
                        }
                    }
                    index += 1;
                }
                ArrayPatternElement::Rest(ident) => {
                    // Rest element: collect remaining elements
                    self.write_indent();
                    self.write("local ");
                    let resolved = self.resolve(ident.node);
                    self.write(&resolved);
                    self.write(" = {");
                    self.write(&format!("table.unpack({}, {})", source, index));
                    self.write("}");
                    self.writeln("");
                    break; // Rest must be last
                }
                ArrayPatternElement::Hole => {
                    // Skip this element
                    index += 1;
                }
            }
        }
    }

    /// Generate object destructuring assignments
    fn generate_object_destructuring(&mut self, pattern: &ObjectPattern, source: &str) {
        for prop in &pattern.properties {
            let key_str = self.resolve(prop.key.node);

            if let Some(value_pattern) = &prop.value {
                // { key: pattern }
                match value_pattern {
                    Pattern::Identifier(ident) => {
                        self.write_indent();
                        self.write("local ");
                        let resolved = self.resolve(ident.node);
                        self.write(&resolved);
                        self.write(&format!(" = {}.{}", source, key_str));
                        self.writeln("");
                    }
                    Pattern::Array(nested_array) => {
                        // Nested array destructuring
                        let temp_var = format!("__temp_{}", key_str);
                        self.write_indent();
                        self.write("local ");
                        self.write(&temp_var);
                        self.write(&format!(" = {}.{}", source, key_str));
                        self.writeln("");
                        self.generate_array_destructuring(nested_array, &temp_var);
                    }
                    Pattern::Object(nested_obj) => {
                        // Nested object destructuring
                        let temp_var = format!("__temp_{}", key_str);
                        self.write_indent();
                        self.write("local ");
                        self.write(&temp_var);
                        self.write(&format!(" = {}.{}", source, key_str));
                        self.writeln("");
                        self.generate_object_destructuring(nested_obj, &temp_var);
                    }
                    Pattern::Wildcard(_) | Pattern::Literal(_, _) => {
                        // Skip - don't bind anything
                    }
                }
            } else {
                // Shorthand: { key } means { key: key }
                self.write_indent();
                self.write("local ");
                self.write(&key_str);
                self.write(&format!(" = {}.{}", source, key_str));
                self.writeln("");
            }
        }
    }

    fn generate_function_declaration(&mut self, decl: &FunctionDeclaration) {
        self.write_indent();
        self.write("local function ");
        let fn_name = self.resolve(decl.name.node);
        self.write(&fn_name);
        self.write("(");

        let mut rest_param_name: Option<typedlua_parser::string_interner::StringId> = None;

        for (i, param) in decl.parameters.iter().enumerate() {
            if param.is_rest {
                // For rest parameters, just write ... in the parameter list
                if i > 0 {
                    self.write(", ");
                }
                self.write("...");
                // Save the parameter name to initialize it in the function body
                if let Pattern::Identifier(ident) = &param.pattern {
                    rest_param_name = Some(ident.node);
                }
            } else {
                if i > 0 {
                    self.write(", ");
                }
                self.generate_pattern(&param.pattern);
            }
        }

        self.writeln(")");
        self.indent();

        // If there's a rest parameter, initialize it from ...
        if let Some(rest_name) = rest_param_name {
            self.write_indent();
            self.write("local ");
            let rest_name_str = self.resolve(rest_name);
            self.write(&rest_name_str);
            self.writeln(" = {...}");
        }

        self.generate_block(&decl.body);
        self.dedent();
        self.write_indent();
        self.writeln("end");

        // If in a namespace, attach the function to the namespace
        if let Some(ns_path) = &self.current_namespace {
            let ns_full_path = ns_path.join(".");
            self.namespace_exports
                .push((fn_name.clone(), ns_full_path.clone()));

            self.write_indent();
            self.writeln(&format!("{}.{} = {}", ns_full_path, fn_name, fn_name));
        }
    }

    fn generate_if_statement(&mut self, if_stmt: &IfStatement) {
        self.write_indent();
        self.write("if ");
        self.generate_expression(&if_stmt.condition);
        self.writeln(" then");
        self.indent();
        self.generate_block(&if_stmt.then_block);
        self.dedent();

        for else_if in &if_stmt.else_ifs {
            self.write_indent();
            self.write("elseif ");
            self.generate_expression(&else_if.condition);
            self.writeln(" then");
            self.indent();
            self.generate_block(&else_if.block);
            self.dedent();
        }

        if let Some(else_block) = &if_stmt.else_block {
            self.write_indent();
            self.writeln("else");
            self.indent();
            self.generate_block(else_block);
            self.dedent();
        }

        self.write_indent();
        self.writeln("end");
    }

    fn generate_while_statement(&mut self, while_stmt: &WhileStatement) {
        self.write_indent();
        self.write("while ");
        self.generate_expression(&while_stmt.condition);
        self.writeln(" do");
        self.indent();
        self.generate_block(&while_stmt.body);
        self.dedent();
        self.write_indent();
        self.writeln("end");
    }

    fn generate_for_statement(&mut self, for_stmt: &ForStatement) {
        match for_stmt {
            ForStatement::Numeric(numeric) => {
                self.write_indent();
                self.write("for ");
                let var_name = self.resolve(numeric.variable.node);
                self.write(&var_name);
                self.write(" = ");
                self.generate_expression(&numeric.start);
                self.write(", ");
                self.generate_expression(&numeric.end);
                if let Some(step) = &numeric.step {
                    self.write(", ");
                    self.generate_expression(step);
                }
                self.writeln(" do");
                self.indent();
                self.generate_block(&numeric.body);
                self.dedent();
                self.write_indent();
                self.writeln("end");
            }
            ForStatement::Generic(generic) => {
                self.write_indent();
                self.write("for ");
                for (i, var) in generic.variables.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    let var_name = self.resolve(var.node);
                    self.write(&var_name);
                }
                self.write(" in ");
                for (i, iter) in generic.iterators.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_expression(iter);
                }
                self.writeln(" do");
                self.indent();
                self.generate_block(&generic.body);
                self.dedent();
                self.write_indent();
                self.writeln("end");
            }
        }
    }

    fn generate_repeat_statement(&mut self, repeat_stmt: &RepeatStatement) {
        self.write_indent();
        self.writeln("repeat");
        self.indent();
        self.generate_block(&repeat_stmt.body);
        self.dedent();
        self.write_indent();
        self.write("until ");
        self.generate_expression(&repeat_stmt.until);
        self.writeln("");
    }

    fn generate_return_statement(&mut self, return_stmt: &ReturnStatement) {
        self.write_indent();
        self.write("return");
        if !return_stmt.values.is_empty() {
            self.write(" ");
            for (i, value) in return_stmt.values.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.generate_expression(value);
            }
        }
        self.writeln("");
    }

    fn generate_block(&mut self, block: &Block) {
        for statement in &block.statements {
            self.generate_statement(statement);
        }
    }

    fn generate_class_declaration(&mut self, class_decl: &ClassDeclaration) {
        let class_name = self.resolve(class_decl.name.node).to_string();

        // Save previous parent class context and set new one
        let prev_parent = self.current_class_parent.take();

        // Handle inheritance
        let base_class_name = if let Some(extends) = &class_decl.extends {
            // Extract base class name from Type
            if let typedlua_parser::ast::types::TypeKind::Reference(type_ref) = &extends.kind {
                Some(type_ref.name.node)
            } else {
                None
            }
        } else {
            None
        };

        // Set current parent for super calls
        self.current_class_parent = base_class_name;

        // Create the class table
        self.write_indent();
        self.write("local ");
        self.write(&class_name);
        self.writeln(" = {}");

        // Set up __index for method lookup
        self.write_indent();
        self.write(&class_name);
        self.write(".__index = ");
        self.write(&class_name);
        self.writeln("");

        if let Some(base_name) = &base_class_name {
            // Set up prototype chain for inheritance
            self.writeln("");
            self.write_indent();
            self.write("setmetatable(");
            self.write(&class_name);
            self.write(", { __index = ");
            let base_name_str = self.resolve(*base_name);
            self.write(&base_name_str);
            self.writeln(" })");
        }

        // Generate constructor
        let has_constructor = class_decl
            .members
            .iter()
            .any(|m| matches!(m, ClassMember::Constructor(_)));

        let has_primary_constructor = class_decl.primary_constructor.is_some();

        if has_primary_constructor {
            // Generate constructor from primary constructor parameters
            self.generate_primary_constructor(class_decl, &class_name, base_class_name);
        } else if has_constructor {
            for member in &class_decl.members {
                if let ClassMember::Constructor(ctor) = member {
                    self.generate_class_constructor(&class_name, ctor);
                }
            }
        } else {
            // Generate default constructor
            self.writeln("");
            self.write_indent();
            self.write("function ");
            self.write(&class_name);
            self.writeln(".new()");
            self.indent();
            self.write_indent();
            self.write("local self = setmetatable({}, ");
            self.write(&class_name);
            self.writeln(")");
            self.write_indent();
            self.writeln("return self");
            self.dedent();
            self.write_indent();
            self.writeln("end");
        }

        // Generate properties (as part of constructor or as defaults)
        // Properties will be initialized in the constructor

        // Generate methods
        for member in &class_decl.members {
            match member {
                ClassMember::Method(method) => {
                    self.generate_class_method(&class_name, method);
                }
                ClassMember::Getter(getter) => {
                    self.generate_class_getter(&class_name, getter);
                }
                ClassMember::Setter(setter) => {
                    self.generate_class_setter(&class_name, setter);
                }
                ClassMember::Operator(op) => {
                    self.generate_operator_declaration(&class_name, op);
                }
                ClassMember::Property(_) | ClassMember::Constructor(_) => {
                    // Already handled
                }
            }
        }

        // Generate metamethods table for operator overloading
        let mut has_operators = false;

        for member in &class_decl.members {
            if let ClassMember::Operator(_) = member {
                has_operators = true;
                break;
            }
        }

        if has_operators {
            self.writeln("");
            self.write_indent();
            self.write(&class_name);
            self.writeln(".__metatable = {");
            self.indent();

            let mut first = true;
            for member in &class_decl.members {
                if let ClassMember::Operator(op) = member {
                    let metamethod_name = self.operator_kind_name(&op.operator);
                    self.write_indent();
                    if !first {
                        self.writeln(",");
                    }
                    first = false;
                    self.write(&format!(
                        "{} = {}.{}",
                        metamethod_name, class_name, metamethod_name
                    ));
                }
            }
            self.writeln("");
            self.dedent();
            self.write_indent();
            self.writeln("}");
        }

        // Apply decorators to the class
        if !class_decl.decorators.is_empty() {
            self.writeln("");
            for decorator in &class_decl.decorators {
                self.write_indent();
                self.write(&class_name);
                self.write(" = ");
                self.generate_decorator_call(decorator, &class_name);
                self.writeln("");
            }
        }

        // Copy default methods from implemented interfaces
        self.writeln("");
        for impl_type in &class_decl.implements {
            if let typedlua_parser::ast::types::TypeKind::Reference(type_ref) = &impl_type.kind {
                let interface_name = self.resolve(type_ref.name.node).to_string();

                // Get all method names that the class already has
                let class_methods: std::collections::HashSet<String> = class_decl
                    .members
                    .iter()
                    .filter_map(|member| {
                        if let ClassMember::Method(method) = member {
                            Some(self.resolve(method.name.node).to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                // For each interface method with a default, copy it if not overridden
                let default_methods: Vec<(String, String)> = self
                    .interface_default_methods
                    .iter()
                    .filter(|((iface_name, _), _)| iface_name == &interface_name)
                    .filter(|((_, method_name), _)| !class_methods.contains(method_name))
                    .map(|((_, method_name), default_fn_name)| {
                        (method_name.clone(), default_fn_name.clone())
                    })
                    .collect();

                for (method_name, default_fn_name) in default_methods {
                    self.write_indent();
                    self.write(&class_name);
                    self.write(":");
                    self.write(&method_name);
                    self.write(" = ");
                    self.write(&class_name);
                    self.write(":");
                    self.write(&method_name);
                    self.write(" or ");
                    self.writeln(&default_fn_name);
                }
            }
        }

        self.writeln("");

        // ============================================================
        // Reflection Metadata Generation
        // ============================================================

        // Assign a unique type ID
        let type_id = self.next_type_id;
        self.next_type_id += 1;

        // Register this type for __TypeRegistry
        self.registered_types.insert(class_name.clone(), type_id);

        // Generate __typeName
        self.write_indent();
        self.write(&class_name);
        self.write(".__typeName = \"");
        self.write(&class_name);
        self.writeln("\"");

        // Generate __typeId
        self.write_indent();
        self.write(&class_name);
        self.writeln(&format!(".__typeId = {}", type_id));

        // Generate __ownFields - array of field metadata
        self.write_indent();
        self.write(&class_name);
        self.writeln(".__ownFields = {");

        self.indent();
        for member in &class_decl.members {
            if let ClassMember::Property(prop) = member {
                let prop_name = self.resolve(prop.name.node).to_string();
                self.write_indent();
                self.write(&format!(
                    "{{ name = \"{}\", type = \"\", modifiers = {{}}",
                    prop_name
                ));
                self.writeln(" },");
            }
        }
        self.dedent();
        self.write_indent();
        self.writeln("}");

        // Generate __ownMethods - array of method metadata
        self.write_indent();
        self.write(&class_name);
        self.writeln(".__ownMethods = {");

        self.indent();
        for member in &class_decl.members {
            if let ClassMember::Method(method) = member {
                let method_name = self.resolve(method.name.node).to_string();
                self.write_indent();
                self.write(&format!(
                    "{{ name = \"{}\", params = {{}}, returnType = \"\" }}",
                    method_name
                ));
                self.writeln(",");
            }
        }
        self.dedent();
        self.write_indent();
        self.writeln("}");

        // Generate __ancestors table for O(1) instanceof checks
        self.write_indent();
        self.write(&class_name);
        self.writeln(".__ancestors = {");

        self.indent();
        self.write_indent();
        self.write(&format!("[{}] = true", type_id));
        self.writeln(",");

        self.dedent();
        self.write_indent();
        self.writeln("}");

        // Merge parent ancestors (done after table is closed)
        if let Some(base_name) = &base_class_name {
            let base_name_str = self.resolve(*base_name);
            self.write_indent();
            self.writeln(&format!(
                "if {} and {}.__ancestors then",
                base_name_str, base_name_str
            ));
            self.indent();
            self.write_indent();
            self.writeln(&format!(
                "for k, v in pairs({}.__ancestors) do",
                base_name_str
            ));
            self.indent();
            self.write_indent();
            self.writeln(&format!("{}.__ancestors[k] = v", class_name));
            self.dedent();
            self.write_indent();
            self.writeln("end");
            self.dedent();
            self.write_indent();
            self.writeln("end");
        }

        // Generate __parent reference for reflective access
        if let Some(base_name) = &base_class_name {
            let base_name_str = self.resolve(*base_name);
            self.write_indent();
            self.write(&class_name);
            self.writeln(&format!(".__parent = {}", base_name_str));
        }

        // Generate lazy _buildAllFields() and _buildAllMethods() functions
        self.writeln("");
        self.writeln(
            &class::BUILD_ALL_FIELDS
                .replace("{}", &class_name)
                .replace("{}", &class_name)
                .replace("{}", &class_name)
                .replace("{}", &class_name)
                .replace("{}", &class_name)
                .replace("{}", &class_name)
                .replace("{}", &class_name),
        );
        self.writeln("");
        self.writeln(
            &class::BUILD_ALL_METHODS
                .replace("{}", &class_name)
                .replace("{}", &class_name)
                .replace("{}", &class_name)
                .replace("{}", &class_name)
                .replace("{}", &class_name)
                .replace("{}", &class_name)
                .replace("{}", &class_name),
        );
        self.writeln("");

        // Restore previous parent class context
        self.current_class_parent = prev_parent;
    }

    fn generate_interface_declaration(&mut self, iface_decl: &InterfaceDeclaration) {
        let interface_name = self.resolve(iface_decl.name.node).to_string();

        for member in &iface_decl.members {
            if let InterfaceMember::Method(method) = member {
                if let Some(body) = &method.body {
                    let method_name = self.resolve(method.name.node).to_string();
                    let default_fn_name = format!("{}__{}", interface_name, method_name);

                    self.writeln("");
                    self.write_indent();
                    self.write("function ");
                    self.write(&default_fn_name);
                    self.write("(self");

                    for param in &method.parameters {
                        self.write(", ");
                        self.generate_pattern(&param.pattern);
                    }
                    self.writeln(")");
                    self.indent();

                    self.generate_block(body);

                    self.dedent();
                    self.write_indent();
                    self.writeln("end");

                    self.interface_default_methods
                        .insert((interface_name.clone(), method_name), default_fn_name);
                }
            }
        }
    }

    fn generate_class_constructor(&mut self, class_name: &str, ctor: &ConstructorDeclaration) {
        // Always generate _init for consistency - this allows super() to work
        let always_use_init = true;

        if always_use_init {
            // Generate _init method for initialization logic (when super() is called)
            self.writeln("");
            self.write_indent();
            self.write("function ");
            self.write(class_name);
            self.write("._init(self");

            // Generate parameters
            for param in &ctor.parameters {
                self.write(", ");
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            self.indent();

            // Generate constructor body (which includes super() calls)
            self.generate_block(&ctor.body);

            self.dedent();
            self.write_indent();
            self.writeln("end");

            // Generate .new constructor that creates instance and calls _init
            self.writeln("");
            self.write_indent();
            self.write("function ");
            self.write(class_name);
            self.write(".new(");

            // Generate parameters
            for (i, param) in ctor.parameters.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            self.indent();

            // Create instance with metatable
            self.write_indent();
            self.write("local self = setmetatable({}, ");
            self.write(class_name);
            self.writeln(")");

            // Call _init
            self.write_indent();
            self.write(class_name);
            self.write("._init(self");
            for param in &ctor.parameters {
                self.write(", ");
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            // Return the instance
            self.write_indent();
            self.writeln("return self");

            self.dedent();
            self.write_indent();
            self.writeln("end");
        } else {
            // Simple constructor without super() - inline the body
            self.writeln("");
            self.write_indent();
            self.write("function ");
            self.write(class_name);
            self.write(".new(");

            // Generate parameters
            for (i, param) in ctor.parameters.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            self.indent();

            // Create instance with metatable
            self.write_indent();
            self.write("local self = setmetatable({}, ");
            self.write(class_name);
            self.writeln(")");

            // Generate constructor body directly
            self.generate_block(&ctor.body);

            // Return the instance
            self.write_indent();
            self.writeln("return self");

            self.dedent();
            self.write_indent();
            self.writeln("end");
        }
    }

    fn generate_primary_constructor(
        &mut self,
        class_decl: &ClassDeclaration,
        class_name: &str,
        base_class_name: Option<typedlua_parser::string_interner::StringId>,
    ) {
        let primary_params = class_decl.primary_constructor.as_ref().unwrap();

        // Generate _init method for use by child classes
        self.writeln("");
        self.write_indent();
        self.write("function ");
        self.write(class_name);
        self.write("._init(self");

        // Generate parameters
        for param in primary_params {
            self.write(", ");
            let param_name = self.resolve(param.name.node);
            self.write(&param_name);
        }
        self.writeln(")");

        self.indent();

        // Call parent constructor if parent_constructor_args is provided
        if let Some(parent_args) = &class_decl.parent_constructor_args {
            if let Some(parent_name) = base_class_name {
                self.write_indent();
                let parent_name_str = self.resolve(parent_name);
                self.write(&parent_name_str);
                self.write("._init(self");
                for arg in parent_args {
                    self.write(", ");
                    self.generate_expression(arg);
                }
                self.writeln(")");
            }
        }

        // Initialize properties from primary constructor parameters
        for param in primary_params {
            self.write_indent();

            // Apply access modifier naming (private → _name)
            if param.access.as_ref()
                == Some(&typedlua_parser::ast::statement::AccessModifier::Private)
            {
                self.write("self._");
            } else {
                self.write("self.");
            }
            let param_name = self.resolve(param.name.node);
            self.write(&param_name);

            self.write(" = ");
            let param_name = self.resolve(param.name.node);
            self.write(&param_name);
            self.writeln("");
        }

        self.dedent();
        self.write_indent();
        self.writeln("end");

        // Generate .new constructor that creates instance and calls _init
        self.writeln("");
        self.write_indent();
        self.write("function ");
        self.write(class_name);
        self.write(".new(");

        // Generate parameters
        for (i, param) in primary_params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            let param_name = self.resolve(param.name.node);
            self.write(&param_name);
        }
        self.writeln(")");

        self.indent();

        // Create instance with metatable
        self.write_indent();
        self.write("local self = setmetatable({}, ");
        self.write(class_name);
        self.writeln(")");

        // Call _init
        self.write_indent();
        self.write(class_name);
        self.write("._init(self");
        for param in primary_params {
            self.write(", ");
            let param_name = self.resolve(param.name.node);
            self.write(&param_name);
        }
        self.writeln(")");

        // Return the instance
        self.write_indent();
        self.writeln("return self");

        self.dedent();
        self.write_indent();
        self.writeln("end");
    }

    fn generate_class_method(&mut self, class_name: &str, method: &MethodDeclaration) {
        // Skip abstract methods - they have no implementation
        if method.is_abstract {
            return;
        }

        self.writeln("");
        self.write_indent();
        self.write("function ");
        self.write(class_name);

        if method.is_static {
            self.write(".");
        } else {
            self.write(":");
        }

        let method_name = self.resolve(method.name.node);
        self.write(&method_name);
        self.write("(");

        // Generate parameters (for instance methods, 'self' is implicit with :)
        for (i, param) in method.parameters.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.generate_pattern(&param.pattern);
        }
        self.writeln(")");

        // Generate method body
        if let Some(body) = &method.body {
            self.indent();
            self.generate_block(body);
            self.dedent();
        }

        self.write_indent();
        self.writeln("end");

        // Apply decorators to the method
        if !method.decorators.is_empty() {
            for decorator in &method.decorators {
                self.write_indent();
                self.write(class_name);
                self.write(".");
                let method_name = self.resolve(method.name.node);
                self.write(&method_name);
                self.write(" = ");

                let method_ref = format!("{}.{}", class_name, method_name);
                self.generate_decorator_call(decorator, &method_ref);
                self.writeln("");
            }
        }
    }

    fn generate_class_getter(&mut self, class_name: &str, getter: &GetterDeclaration) {
        self.writeln("");
        self.write_indent();
        self.write("function ");
        self.write(class_name);

        if getter.is_static {
            self.write(".");
        } else {
            self.write(":");
        }

        self.write("get_");
        let getter_name = self.resolve(getter.name.node);
        self.write(&getter_name);
        self.writeln("()");

        self.indent();
        self.generate_block(&getter.body);
        self.dedent();

        self.write_indent();
        self.writeln("end");
    }

    fn generate_class_setter(&mut self, class_name: &str, setter: &SetterDeclaration) {
        self.writeln("");
        self.write_indent();
        self.write("function ");
        self.write(class_name);

        if setter.is_static {
            self.write(".");
        } else {
            self.write(":");
        }

        self.write("set_");
        let setter_name = self.resolve(setter.name.node);
        self.write(&setter_name);
        self.write("(");
        self.generate_pattern(&setter.parameter.pattern);
        self.writeln(")");

        self.indent();
        self.generate_block(&setter.body);
        self.dedent();

        self.write_indent();
        self.writeln("end");
    }

    fn generate_operator_declaration(&mut self, class_name: &str, op: &OperatorDeclaration) {
        self.writeln("");
        self.write_indent();
        self.write("function ");
        self.write(class_name);
        self.write(".");
        self.write(&self.operator_kind_name(&op.operator));
        self.write("(self");

        let is_unary = op.parameters.is_empty();

        if !is_unary {
            for param in op.parameters.iter() {
                self.write(", ");
                self.generate_pattern(&param.pattern);
            }
        }
        self.writeln(")");

        self.indent();
        self.generate_block(&op.body);
        self.dedent();

        self.write_indent();
        self.writeln("end");

        for decorator in &op.decorators {
            self.write_indent();
            self.write(class_name);
            self.write(".");
            self.write(&self.operator_kind_name(&op.operator));
            self.write(" = ");

            let method_ref = format!("{}.{}", class_name, self.operator_kind_name(&op.operator));
            self.generate_decorator_call(decorator, &method_ref);
            self.writeln("");
        }
    }

    fn operator_kind_name(&self, op: &typedlua_parser::ast::statement::OperatorKind) -> String {
        match op {
            typedlua_parser::ast::statement::OperatorKind::Add => "__add".to_string(),
            typedlua_parser::ast::statement::OperatorKind::Subtract => "__sub".to_string(),
            typedlua_parser::ast::statement::OperatorKind::Multiply => "__mul".to_string(),
            typedlua_parser::ast::statement::OperatorKind::Divide => "__div".to_string(),
            typedlua_parser::ast::statement::OperatorKind::Modulo => "__mod".to_string(),
            typedlua_parser::ast::statement::OperatorKind::Power => "__pow".to_string(),
            typedlua_parser::ast::statement::OperatorKind::Concatenate => "__concat".to_string(),
            typedlua_parser::ast::statement::OperatorKind::FloorDivide => "__idiv".to_string(),
            typedlua_parser::ast::statement::OperatorKind::Equal => "__eq".to_string(),
            typedlua_parser::ast::statement::OperatorKind::NotEqual => "__eq".to_string(),
            typedlua_parser::ast::statement::OperatorKind::LessThan => "__lt".to_string(),
            typedlua_parser::ast::statement::OperatorKind::LessThanOrEqual => "__le".to_string(),
            typedlua_parser::ast::statement::OperatorKind::GreaterThan => "__lt".to_string(),
            typedlua_parser::ast::statement::OperatorKind::GreaterThanOrEqual => "__le".to_string(),
            typedlua_parser::ast::statement::OperatorKind::BitwiseAnd => "__band".to_string(),
            typedlua_parser::ast::statement::OperatorKind::BitwiseOr => "__bor".to_string(),
            typedlua_parser::ast::statement::OperatorKind::BitwiseXor => "__bxor".to_string(),
            typedlua_parser::ast::statement::OperatorKind::ShiftLeft => "__shl".to_string(),
            typedlua_parser::ast::statement::OperatorKind::ShiftRight => "__shr".to_string(),
            typedlua_parser::ast::statement::OperatorKind::Index => "__index".to_string(),
            typedlua_parser::ast::statement::OperatorKind::NewIndex => "__newindex".to_string(),
            typedlua_parser::ast::statement::OperatorKind::Call => "__call".to_string(),
            typedlua_parser::ast::statement::OperatorKind::UnaryMinus => "__unm".to_string(),
            typedlua_parser::ast::statement::OperatorKind::Length => "__len".to_string(),
        }
    }

    /// Generate a decorator call
    /// Decorators in Lua are applied as function calls that wrap/modify the target
    /// Example: @log becomes target = log(target)
    fn generate_decorator_call(
        &mut self,
        decorator: &typedlua_parser::ast::statement::Decorator,
        target: &str,
    ) {
        use typedlua_parser::ast::statement::DecoratorExpression;

        match &decorator.expression {
            DecoratorExpression::Identifier(name) => {
                // Simple decorator: @decorator -> target = decorator(target)
                let decorator_name = self.resolve(name.node);
                self.write(&decorator_name);
                self.write("(");
                self.write(target);
                self.write(")");
            }
            DecoratorExpression::Call {
                callee, arguments, ..
            } => {
                // Decorator with arguments: @decorator(arg1, arg2) -> target = decorator(arg1, arg2)(target)
                self.generate_decorator_expression(callee);
                self.write("(");
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_expression(arg);
                }
                self.write(")(");
                self.write(target);
                self.write(")");
            }
            DecoratorExpression::Member {
                object, property, ..
            } => {
                // Member decorator: @namespace.decorator -> target = namespace.decorator(target)
                self.generate_decorator_expression(object);
                self.write(".");
                let prop_name = self.resolve(property.node);
                self.write(&prop_name);
                self.write("(");
                self.write(target);
                self.write(")");
            }
        }
    }

    /// Generate decorator expression (helper for nested decorator expressions)
    fn generate_decorator_expression(
        &mut self,
        expr: &typedlua_parser::ast::statement::DecoratorExpression,
    ) {
        use typedlua_parser::ast::statement::DecoratorExpression;

        match expr {
            DecoratorExpression::Identifier(name) => {
                let name_str = self.resolve(name.node);
                self.write(&name_str);
            }
            DecoratorExpression::Call {
                callee, arguments, ..
            } => {
                self.generate_decorator_expression(callee);
                self.write("(");
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_expression(arg);
                }
                self.write(")");
            }
            DecoratorExpression::Member {
                object, property, ..
            } => {
                self.generate_decorator_expression(object);
                self.write(".");
                let prop_str = self.resolve(property.node);
                self.write(&prop_str);
            }
        }
    }

    /// Check if a decorator name is a built-in decorator
    fn is_built_in_decorator(&self, name: &str) -> bool {
        matches!(name, "readonly" | "sealed" | "deprecated")
    }

    /// Detect if the program uses built-in decorators
    fn detect_decorators(&mut self, program: &Program) {
        for statement in &program.statements {
            if self.statement_uses_built_in_decorators(statement) {
                self.uses_built_in_decorators = true;
                return;
            }
        }
    }

    /// Check if a statement uses built-in decorators
    fn statement_uses_built_in_decorators(&self, stmt: &Statement) -> bool {
        match stmt {
            Statement::Class(class_decl) => {
                // Check class decorators
                for decorator in &class_decl.decorators {
                    if self.is_decorator_built_in(&decorator.expression) {
                        return true;
                    }
                }

                // Check member decorators
                for member in &class_decl.members {
                    let decorators = match member {
                        ClassMember::Method(method) => &method.decorators,
                        ClassMember::Property(prop) => &prop.decorators,
                        ClassMember::Getter(getter) => &getter.decorators,
                        ClassMember::Setter(setter) => &setter.decorators,
                        _ => continue,
                    };

                    for decorator in decorators {
                        if self.is_decorator_built_in(&decorator.expression) {
                            return true;
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Check if a decorator expression references a built-in decorator
    fn is_decorator_built_in(
        &self,
        expr: &typedlua_parser::ast::statement::DecoratorExpression,
    ) -> bool {
        use typedlua_parser::ast::statement::DecoratorExpression;

        match expr {
            DecoratorExpression::Identifier(name) => {
                let name_str = self.resolve(name.node);
                self.is_built_in_decorator(&name_str)
            }
            DecoratorExpression::Call { callee, .. } => {
                if let DecoratorExpression::Identifier(name) = &**callee {
                    let name_str = self.resolve(name.node);
                    self.is_built_in_decorator(&name_str)
                } else {
                    false
                }
            }
            DecoratorExpression::Member {
                object, property, ..
            } => {
                // Check if it's TypedLua.readonly, TypedLua.sealed, or TypedLua.deprecated
                if let DecoratorExpression::Identifier(obj_name) = &**object {
                    let obj_str = self.resolve(obj_name.node);
                    let prop_str = self.resolve(property.node);
                    obj_str == "TypedLua" && self.is_built_in_decorator(&prop_str)
                } else {
                    false
                }
            }
        }
    }

    /// Embed the TypedLua runtime library at the beginning of the generated code
    fn embed_runtime_library(&mut self) {
        self.writeln(decorator::DECORATOR_RUNTIME);
        self.writeln("");
    }

    /// Embed bitwise helper functions for Lua 5.1
    /// These are always included when targeting Lua 5.1 since they are small
    /// and defined as local functions (no global namespace pollution)
    fn embed_bitwise_helpers(&mut self) {
        self.writeln(bitwise::for_lua51());
    }

    fn generate_expression(&mut self, expr: &Expression) {
        match &expr.kind {
            ExpressionKind::Literal(lit) => self.generate_literal(lit),
            ExpressionKind::Identifier(name) => {
                let name_str = self.resolve(*name);
                self.write(&name_str);
            }
            ExpressionKind::Binary(op, left, right) => {
                self.generate_binary_expression(*op, left, right);
            }
            ExpressionKind::Unary(op, operand) => {
                self.write(self.unary_op_to_string(*op));
                self.generate_expression(operand);
            }
            ExpressionKind::Assignment(target, op, value) => {
                // For compound assignments, desugar to: target = target op value
                // For plain assignment, just do: target = value
                match op {
                    AssignmentOp::Assign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::AddAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" + ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::SubtractAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" - ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::MultiplyAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" * ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::DivideAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" / ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::ModuloAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" % ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::PowerAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" ^ ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::ConcatenateAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" .. ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::BitwiseAndAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" & ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::BitwiseOrAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" | ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::FloorDivideAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" // ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::LeftShiftAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" << ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::RightShiftAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" >> ");
                        self.generate_expression(value);
                    }
                }
            }
            ExpressionKind::Call(callee, args, _) => {
                // Check for super() constructor call
                if matches!(&callee.kind, ExpressionKind::SuperKeyword) {
                    // super() in constructor - call Parent._init(self, args)
                    if let Some(parent) = self.current_class_parent {
                        let parent_str = self.resolve(parent);
                        self.write(&parent_str);
                        self.write("._init(self");
                        if !args.is_empty() {
                            self.write(", ");
                        }
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                self.write(", ");
                            }
                            self.generate_argument(arg);
                        }
                        self.write(")");
                    } else {
                        self.write("nil -- super() without parent class");
                    }
                    return;
                }

                // Check if this is a super method call: super.method(args)
                let is_super_method_call = matches!(&callee.kind,
                    ExpressionKind::Member(obj, _) if matches!(obj.kind, ExpressionKind::SuperKeyword)
                );

                self.generate_expression(callee);
                self.write("(");

                // For super method calls, inject 'self' as first argument
                if is_super_method_call {
                    self.write("self");
                    if !args.is_empty() {
                        self.write(", ");
                    }
                }

                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_argument(arg);
                }
                self.write(")");
            }
            ExpressionKind::New(constructor, args) => {
                // Generate: ClassName.new(args)
                // In Lua, classes have a .new() method that creates instances
                self.generate_expression(constructor);
                self.write(".new(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_argument(arg);
                }
                self.write(")");
            }
            ExpressionKind::Member(object, member) => {
                // Check if this is super.method - translate to ParentClass.method
                if matches!(object.kind, ExpressionKind::SuperKeyword) {
                    if let Some(parent) = self.current_class_parent {
                        let parent_str = self.resolve(parent);
                        self.write(&parent_str);
                        self.write(".");
                        let member_str = self.resolve(member.node);
                        self.write(&member_str);
                    } else {
                        // No parent class - this is an error, but generate something
                        self.write("nil -- super used without parent class");
                    }
                } else {
                    self.generate_expression(object);
                    self.write(".");
                    let member_str = self.resolve(member.node);
                    self.write(&member_str);
                }
            }
            ExpressionKind::Index(object, index) => {
                self.generate_expression(object);
                self.write("[");
                self.generate_expression(index);
                self.write("]");
            }
            ExpressionKind::Array(elements) => {
                // Check if there are any spreads
                let has_spread = elements
                    .iter()
                    .any(|elem| matches!(elem, ArrayElement::Spread(_)));

                if !has_spread {
                    // Simple case: no spreads
                    self.write("{");
                    for (i, elem) in elements.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        match elem {
                            ArrayElement::Expression(expr) => self.generate_expression(expr),
                            ArrayElement::Spread(_) => unreachable!(),
                        }
                    }
                    self.write("}");
                } else {
                    // Complex case: has spreads - use helper pattern
                    self.write("(function() local __arr = {} ");

                    for elem in elements {
                        match elem {
                            ArrayElement::Expression(expr) => {
                                self.write("table.insert(__arr, ");
                                self.generate_expression(expr);
                                self.write(") ");
                            }
                            ArrayElement::Spread(expr) => {
                                self.write("for _, __v in ipairs(");
                                self.generate_expression(expr);
                                self.write(") do table.insert(__arr, __v) end ");
                            }
                        }
                    }

                    self.write("return __arr end)()");
                }
            }
            ExpressionKind::Object(props) => {
                // Check if there are any spreads
                let has_spread = props
                    .iter()
                    .any(|prop| matches!(prop, ObjectProperty::Spread { .. }));

                if !has_spread {
                    // Simple case: no spreads
                    self.write("{");
                    for (i, prop) in props.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.generate_object_property(prop);
                    }
                    self.write("}");
                } else {
                    // Complex case: has spreads - use helper pattern
                    self.write("(function() local __obj = {} ");

                    for prop in props {
                        match prop {
                            ObjectProperty::Property { key, value, .. } => {
                                self.write("__obj.");
                                let key_str = self.resolve(key.node);
                                self.write(&key_str);
                                self.write(" = ");
                                self.generate_expression(value);
                                self.write(" ");
                            }
                            ObjectProperty::Computed { key, value, .. } => {
                                self.write("__obj[");
                                self.generate_expression(key);
                                self.write("] = ");
                                self.generate_expression(value);
                                self.write(" ");
                            }
                            ObjectProperty::Spread { value, .. } => {
                                self.write("for __k, __v in pairs(");
                                self.generate_expression(value);
                                self.write(") do __obj[__k] = __v end ");
                            }
                        }
                    }

                    self.write("return __obj end)()");
                }
            }
            ExpressionKind::Function(func_expr) => {
                self.write("function(");
                for (i, param) in func_expr.parameters.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_pattern(&param.pattern);
                }
                self.write(")\n");
                self.indent();
                self.generate_block(&func_expr.body);
                self.dedent();
                self.write_indent();
                self.write("end");
            }
            ExpressionKind::Arrow(arrow_expr) => {
                self.write("function(");
                for (i, param) in arrow_expr.parameters.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_pattern(&param.pattern);
                }
                self.write(")\n");
                self.indent();
                match &arrow_expr.body {
                    ArrowBody::Expression(expr) => {
                        self.write_indent();
                        self.write("return ");
                        self.generate_expression(expr);
                        self.writeln("");
                    }
                    ArrowBody::Block(block) => {
                        self.generate_block(block);
                    }
                }
                self.dedent();
                self.write_indent();
                self.write("end");
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.write("(");
                self.generate_expression(cond);
                self.write(" and ");
                self.generate_expression(then_expr);
                self.write(" or ");
                self.generate_expression(else_expr);
                self.write(")");
            }
            ExpressionKind::Match(match_expr) => {
                self.generate_match_expression(match_expr);
            }
            ExpressionKind::Pipe(left, right) => {
                // Pipe operator: left |> right
                // Generates: right(left)
                // For chained pipes or more complex right expressions,
                // we check if right is a call and prepend left to its arguments
                match &right.kind {
                    ExpressionKind::Call(callee, arguments, _) => {
                        // right is already a function call, prepend left as first argument
                        self.generate_expression(callee);
                        self.write("(");
                        self.generate_expression(left);
                        if !arguments.is_empty() {
                            self.write(", ");
                            for (i, arg) in arguments.iter().enumerate() {
                                if i > 0 {
                                    self.write(", ");
                                }
                                if arg.is_spread {
                                    self.write("table.unpack(");
                                    self.generate_expression(&arg.value);
                                    self.write(")");
                                } else {
                                    self.generate_expression(&arg.value);
                                }
                            }
                        }
                        self.write(")");
                    }
                    _ => {
                        // right is just a function reference, call it with left
                        self.generate_expression(right);
                        self.write("(");
                        self.generate_expression(left);
                        self.write(")");
                    }
                }
            }
            ExpressionKind::MethodCall(object, method, args, _) => {
                self.generate_expression(object);
                self.write(":");
                let method_str = self.resolve(method.node);
                self.write(&method_str);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_argument(arg);
                }
                self.write(")");
            }
            ExpressionKind::Parenthesized(expr) => {
                self.write("(");
                self.generate_expression(expr);
                self.write(")");
            }
            ExpressionKind::SelfKeyword => {
                self.write("self");
            }
            ExpressionKind::SuperKeyword => {
                // Super keyword on its own (not in member access or call)
                // This is likely an error, but generate something reasonable
                if let Some(parent) = self.current_class_parent {
                    let parent_str = self.resolve(parent);
                    self.write(&parent_str);
                } else {
                    self.write("nil --[[super without parent class]]");
                }
            }
            ExpressionKind::Template(template_lit) => {
                // Generate template literal as concatenation with dedenting
                self.write("(");
                let mut first = true;

                let mut string_parts: Vec<String> = Vec::new();
                let mut expression_parts: Vec<&Expression> = Vec::new();

                for part in &template_lit.parts {
                    match part {
                        crate::ast::expression::TemplatePart::String(s) => {
                            string_parts.push(s.clone());
                        }
                        crate::ast::expression::TemplatePart::Expression(expr) => {
                            expression_parts.push(expr);
                        }
                    }
                }

                let string_iter = string_parts.iter();
                let mut expression_iter = expression_parts.iter().peekable();

                for s in string_iter {
                    if !first {
                        self.write(" .. ");
                    }
                    first = false;

                    let dedented = dedent(s);
                    self.write("\"");
                    self.write(&dedented.replace('\\', "\\\\").replace('"', "\\\""));
                    self.write("\"");

                    if expression_iter.peek().is_some() {
                        self.write(" .. tostring(");
                        self.generate_expression(expression_iter.next().unwrap());
                        self.write(")");
                    }
                }

                if first {
                    self.write("\"\"");
                }
                self.write(")");
            }
            ExpressionKind::TypeAssertion(expr, _type_annotation) => {
                // Type assertions are compile-time only, just generate the expression
                self.generate_expression(expr);
            }
            ExpressionKind::OptionalMember(object, member) => {
                // Optional member access: obj?.member
                // O2 optimization: Skip nil check for guaranteed non-nil expressions
                // O1: Simple expressions use (obj and obj.member or nil)
                // O1: Complex expressions use IIFE to avoid double evaluation
                if self.optimization_level.effective() >= OptimizationLevel::O2
                    && self.is_guaranteed_non_nil(object)
                {
                    self.generate_expression(object);
                    self.write(".");
                    let member_str = self.resolve(member.node);
                    self.write(&member_str);
                } else if self.is_simple_expression(object) {
                    self.write("(");
                    self.generate_expression(object);
                    self.write(" and ");
                    self.generate_expression(object);
                    self.write(".");
                    let member_str = self.resolve(member.node);
                    self.write(&member_str);
                    self.write(" or nil)");
                } else {
                    // Use IIFE to evaluate object once
                    self.write("(function() local __t = ");
                    self.generate_expression(object);
                    self.writeln("; if __t then return __t.");
                    self.write_indent();
                    let member_str = self.resolve(member.node);
                    self.write(&member_str);
                    self.writeln(" else return nil end end)()");
                }
            }
            ExpressionKind::OptionalIndex(object, index) => {
                // Optional index access: obj?.[index]
                // O2 optimization: Skip nil check for guaranteed non-nil expressions
                if self.optimization_level.effective() >= OptimizationLevel::O2
                    && self.is_guaranteed_non_nil(object)
                {
                    self.generate_expression(object);
                    self.write("[");
                    self.generate_expression(index);
                    self.write("]");
                } else if self.is_simple_expression(object) {
                    self.write("(");
                    self.generate_expression(object);
                    self.write(" and ");
                    self.generate_expression(object);
                    self.write("[");
                    self.generate_expression(index);
                    self.write("] or nil)");
                } else {
                    self.write("(function() local __t = ");
                    self.generate_expression(object);
                    self.writeln("; if __t then return __t[");
                    self.write_indent();
                    self.generate_expression(index);
                    self.writeln("] else return nil end end)()");
                }
            }
            ExpressionKind::OptionalCall(callee, _args, _) => {
                // Optional call: func?.()
                // O2 optimization: Skip nil check for guaranteed non-nil expressions
                if self.optimization_level.effective() >= OptimizationLevel::O2
                    && self.is_guaranteed_non_nil(callee)
                {
                    self.generate_expression(callee);
                    self.write("()");
                } else if self.is_simple_expression(callee) {
                    self.write("(");
                    self.generate_expression(callee);
                    self.write(" and ");
                    self.generate_expression(callee);
                    self.write("() or nil)");
                } else {
                    self.write("(function() local __t = ");
                    self.generate_expression(callee);
                    self.writeln("; if __t then return __t() else return nil end end)()");
                }
            }
            ExpressionKind::OptionalMethodCall(object, method, args, _) => {
                // Optional method call: obj?.method()
                // O2 optimization: Skip nil check for guaranteed non-nil expressions
                if self.optimization_level.effective() >= OptimizationLevel::O2
                    && self.is_guaranteed_non_nil(object)
                {
                    self.generate_expression(object);
                    self.write(":");
                    let method_str = self.resolve(method.node);
                    self.write(&method_str);
                    self.write("(");
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.generate_argument(arg);
                    }
                    self.write(")");
                } else if self.is_simple_expression(object) {
                    self.write("(");
                    self.generate_expression(object);
                    self.write(" and ");
                    self.generate_expression(object);
                    self.write(":");
                    let method_str = self.resolve(method.node);
                    self.write(&method_str);
                    self.write("(");
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.generate_argument(arg);
                    }
                    self.write(") or nil)");
                } else {
                    self.write("(function() local __t = ");
                    self.generate_expression(object);
                    self.write("; if __t then return __t:");
                    let method_str = self.resolve(method.node);
                    self.write(&method_str);
                    self.write("(");
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.generate_argument(arg);
                    }
                    self.writeln(") else return nil end end)()");
                }
            }
            ExpressionKind::Try(try_expr) => {
                // try expr catch var fallback
                // Generates: (function() local __ok, __result = pcall(function() return expr end); if __ok then return __result else return fallback end)()
                self.write("(function() local __ok, __result = pcall(function() return ");
                self.generate_expression(&try_expr.expression);
                self.writeln(" end); ");
                self.write("if __ok then return __result else ");
                let var_name = self.resolve(try_expr.catch_variable.node);
                self.write("local ");
                self.write(&var_name);
                self.write(" = __result; return ");
                self.generate_expression(&try_expr.catch_expression);
                self.writeln(" end end)()");
            }
            ExpressionKind::ErrorChain(left, right) => {
                // left !! right - error chaining operator
                // Generates: (function() local __ok, __result = pcall(function() return left end); if __ok then return __result else return right end)()
                self.write("(function() local __ok, __result = pcall(function() return ");
                self.generate_expression(left);
                self.writeln(" end); ");
                self.write("if __ok then return __result else return ");
                self.generate_expression(right);
                self.writeln(" end end)()");
            }
        }
    }

    fn generate_literal(&mut self, lit: &Literal) {
        match lit {
            Literal::Nil => self.write("nil"),
            Literal::Boolean(b) => self.write(if *b { "true" } else { "false" }),
            Literal::Number(n) => self.write(&n.to_string()),
            Literal::Integer(i) => self.write(&i.to_string()),
            Literal::String(s) => {
                self.write("\"");
                self.write(&s.replace('\\', "\\\\").replace('"', "\\\""));
                self.write("\"");
            }
        }
    }

    fn generate_argument(&mut self, arg: &Argument) {
        self.generate_expression(&arg.value);
    }

    fn generate_object_property(&mut self, prop: &ObjectProperty) {
        match prop {
            ObjectProperty::Property { key, value, .. } => {
                let key_str = self.resolve(key.node);
                self.write(&key_str);
                self.write(" = ");
                self.generate_expression(value);
            }
            ObjectProperty::Computed { key, value, .. } => {
                self.write("[");
                self.generate_expression(key);
                self.write("] = ");
                self.generate_expression(value);
            }
            ObjectProperty::Spread { value, .. } => {
                // Spread not directly supported in Lua
                // Would need to be implemented with table.unpack or similar
                self.generate_expression(value);
            }
        }
    }

    fn generate_binary_expression(&mut self, op: BinaryOp, left: &Expression, right: &Expression) {
        match op {
            BinaryOp::NullCoalesce => {
                self.generate_null_coalesce(left, right);
            }

            // Standard operators that work everywhere
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Modulo
            | BinaryOp::Power
            | BinaryOp::Concatenate
            | BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::LessThan
            | BinaryOp::LessThanOrEqual
            | BinaryOp::GreaterThan
            | BinaryOp::GreaterThanOrEqual
            | BinaryOp::And
            | BinaryOp::Or => {
                self.write("(");
                self.generate_expression(left);
                self.write(" ");
                self.write(self.simple_binary_op_to_string(op));
                self.write(" ");
                self.generate_expression(right);
                self.write(")");
            }

            // instanceof - generate runtime type check
            BinaryOp::Instanceof => {
                self.write("(type(");
                self.generate_expression(left);
                self.write(") == \"table\" and getmetatable(");
                self.generate_expression(left);
                self.write(") == ");
                self.generate_expression(right);
                self.write(")");
            }

            // Bitwise operators - native in Lua 5.3+
            BinaryOp::BitwiseAnd
            | BinaryOp::BitwiseOr
            | BinaryOp::BitwiseXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight
                if self.target.supports_bitwise_ops() =>
            {
                self.write("(");
                self.generate_expression(left);
                self.write(" ");
                self.write(self.simple_binary_op_to_string(op));
                self.write(" ");
                self.generate_expression(right);
                self.write(")");
            }

            // Bitwise via library for Lua 5.2/LuaJIT
            BinaryOp::BitwiseAnd => self.generate_bitwise_library_call("band", left, right),
            BinaryOp::BitwiseOr => self.generate_bitwise_library_call("bor", left, right),
            BinaryOp::BitwiseXor => self.generate_bitwise_library_call("bxor", left, right),
            BinaryOp::ShiftLeft => self.generate_bitwise_library_call("lshift", left, right),
            BinaryOp::ShiftRight => self.generate_bitwise_library_call("rshift", left, right),

            // Integer division
            BinaryOp::IntegerDivide if self.target.supports_integer_divide() => {
                self.write("(");
                self.generate_expression(left);
                self.write(" // ");
                self.generate_expression(right);
                self.write(")");
            }
            BinaryOp::IntegerDivide => {
                // Fallback: math.floor(a / b)
                self.write("math.floor(");
                self.generate_expression(left);
                self.write(" / ");
                self.generate_expression(right);
                self.write(")");
            }
        }
    }

    /// Generate null coalescing operator (??)
    /// O2 optimization: Skip nil check for guaranteed non-nil expressions
    /// Simple form: (a ~= nil and a or b) for simple expressions
    /// IIFE form: (function() local __left = expr; return __left ~= nil and __left or b end)()
    /// for complex expressions that might have side effects or need to be evaluated once
    fn generate_null_coalesce(&mut self, left: &Expression, right: &Expression) {
        if self.optimization_level.effective() >= OptimizationLevel::O2
            && self.is_guaranteed_non_nil(left)
        {
            self.generate_expression(left);
            return;
        }

        if self.is_simple_expression(left) {
            self.write("(");
            self.generate_expression(left);
            self.write(" ~= nil and ");
            self.generate_expression(left);
            self.write(" or ");
            self.generate_expression(right);
            self.write(")");
        } else {
            self.write("(function() local __left = ");
            self.generate_expression(left);
            self.writeln(";");
            self.write_indent();
            self.write("return __left ~= nil and __left or ");
            self.generate_expression(right);
            self.writeln("");
            self.write_indent();
            self.write("end)()");
        }
    }

    /// Check if an expression is guaranteed to never be nil
    /// Used for O2 null coalescing optimization to skip unnecessary nil checks
    fn is_guaranteed_non_nil(&self, expr: &Expression) -> bool {
        match &expr.kind {
            ExpressionKind::Literal(Literal::Boolean(_)) => true,
            ExpressionKind::Literal(Literal::Number(_)) => true,
            ExpressionKind::Literal(Literal::Integer(_)) => true,
            ExpressionKind::Literal(Literal::String(_)) => true,
            ExpressionKind::Object(_) => true,
            ExpressionKind::Array(_) => true,
            ExpressionKind::New(_, _) => true,
            ExpressionKind::Function(_) => true,
            ExpressionKind::Parenthesized(inner) => self.is_guaranteed_non_nil(inner),
            _ => false,
        }
    }

    /// Check if an expression is "simple" and can be safely evaluated twice
    /// Simple expressions: identifiers, literals, and simple member/index access
    fn is_simple_expression(&self, expr: &Expression) -> bool {
        match &expr.kind {
            ExpressionKind::Identifier(_) => true,
            ExpressionKind::Literal(_) => true,
            ExpressionKind::Member(obj, _) => self.is_simple_expression(obj),
            ExpressionKind::Index(obj, index) => {
                self.is_simple_expression(obj) && self.is_simple_expression(index)
            }
            _ => false,
        }
    }

    fn simple_binary_op_to_string(&self, op: BinaryOp) -> &'static str {
        match op {
            BinaryOp::Add => "+",
            BinaryOp::Subtract => "-",
            BinaryOp::Multiply => "*",
            BinaryOp::Divide => "/",
            BinaryOp::Modulo => "%",
            BinaryOp::Power => "^",
            BinaryOp::Concatenate => "..",
            BinaryOp::Equal => "==",
            BinaryOp::NotEqual => "~=",
            BinaryOp::LessThan => "<",
            BinaryOp::LessThanOrEqual => "<=",
            BinaryOp::GreaterThan => ">",
            BinaryOp::GreaterThanOrEqual => ">=",
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
            BinaryOp::NullCoalesce => unreachable!("null coalescing is handled separately"),
            BinaryOp::BitwiseAnd => "&",
            BinaryOp::BitwiseOr => "|",
            BinaryOp::BitwiseXor => "~",
            BinaryOp::ShiftLeft => "<<",
            BinaryOp::ShiftRight => ">>",
            BinaryOp::IntegerDivide => "//",
            BinaryOp::Instanceof => unreachable!("instanceof is handled separately"),
        }
    }

    fn generate_bitwise_library_call(&mut self, func: &str, left: &Expression, right: &Expression) {
        if let Some(lib) = self.target.bit_library_name() {
            self.write(lib);
            self.write(".");
            self.write(func);
            self.write("(");
            self.generate_expression(left);
            self.write(", ");
            self.generate_expression(right);
            self.write(")");
        } else {
            // Lua 5.1 without library - would need manual implementation
            // Use placeholder helper function name
            self.write("_bit_");
            self.write(func);
            self.write("(");
            self.generate_expression(left);
            self.write(", ");
            self.generate_expression(right);
            self.write(")");
        }
    }

    fn unary_op_to_string(&self, op: UnaryOp) -> &'static str {
        match op {
            UnaryOp::Negate => "-",
            UnaryOp::Not => "not ",
            UnaryOp::Length => "#",
            UnaryOp::BitwiseNot => "~",
        }
    }

    fn generate_match_expression(&mut self, match_expr: &MatchExpression) {
        // Generate match as an immediately invoked function that returns a value
        // This allows us to use if-elseif chains and still use it as an expression
        self.write("(function()");
        self.writeln("");
        self.indent();

        // Generate a local variable for the match value
        self.write_indent();
        self.write("local __match_value = ");
        self.generate_expression(&match_expr.value);
        self.writeln("");

        // Generate if-elseif chain for each arm
        for (i, arm) in match_expr.arms.iter().enumerate() {
            self.write_indent();
            if i == 0 {
                self.write("if ");
            } else {
                self.write("elseif ");
            }

            // Generate pattern match condition
            self.generate_pattern_match(&arm.pattern, "__match_value");

            // Add guard condition if present
            if let Some(guard) = &arm.guard {
                self.write(" and (");
                self.generate_expression(guard);
                self.write(")");
            }

            self.write(" then");
            self.writeln("");
            self.indent();

            // Generate destructuring bindings from pattern
            self.generate_pattern_bindings(&arm.pattern, "__match_value");

            // Generate the arm body
            self.write_indent();
            match &arm.body {
                MatchArmBody::Expression(expr) => {
                    self.write("return ");
                    self.generate_expression(expr);
                    self.writeln("");
                }
                MatchArmBody::Block(block) => {
                    // For blocks, generate all statements
                    for stmt in &block.statements {
                        self.generate_statement(stmt);
                    }
                    // If no explicit return, return nil
                    self.write_indent();
                    self.writeln("return nil");
                }
            }

            self.dedent();
        }

        // Add else clause that returns nil (or could error for non-exhaustive matches)
        self.write_indent();
        self.writeln("else");
        self.indent();
        self.write_indent();
        self.writeln("error(\"Non-exhaustive match\")");
        self.dedent();
        self.write_indent();
        self.writeln("end");

        self.dedent();
        self.write_indent();
        self.write("end)()");
    }

    fn generate_pattern_match(&mut self, pattern: &Pattern, value_var: &str) {
        use typedlua_parser::ast::pattern::*;

        match pattern {
            Pattern::Wildcard(_) => {
                // Wildcard matches everything
                self.write("true");
            }
            Pattern::Identifier(_) => {
                // Identifier matches everything (it's a binding)
                self.write("true");
            }
            Pattern::Literal(lit, _) => {
                // Compare literal value
                self.write(value_var);
                self.write(" == ");
                self.generate_literal(lit);
            }
            Pattern::Array(array_pattern) => {
                // Check if it's a table and has the right number of elements
                self.write("type(");
                self.write(value_var);
                self.write(") == \"table\"");

                // Check each element
                for (i, elem) in array_pattern.elements.iter().enumerate() {
                    match elem {
                        typedlua_parser::ast::pattern::ArrayPatternElement::Pattern(pat) => {
                            self.write(" and ");
                            let index_expr = format!("{}[{}]", value_var, i + 1);
                            self.generate_pattern_match(pat, &index_expr);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Rest(_) => {
                            // Rest pattern doesn't add conditions
                        }
                        ArrayPatternElement::Hole => {
                            // Hole doesn't add conditions
                        }
                    }
                }
            }
            Pattern::Object(_) => {
                // For object patterns, just check if it's a table
                self.write("type(");
                self.write(value_var);
                self.write(") == \"table\"");
            }
        }
    }

    fn generate_pattern_bindings(&mut self, pattern: &Pattern, value_var: &str) {
        use typedlua_parser::ast::pattern::*;

        match pattern {
            Pattern::Identifier(ident) => {
                // Bind the identifier to the value
                self.write_indent();
                self.write("local ");
                let ident_str = self.resolve(ident.node);
                self.write(&ident_str);
                self.write(" = ");
                self.write(value_var);
                self.writeln("");
            }
            Pattern::Array(array_pattern) => {
                // Bind each element
                for (i, elem) in array_pattern.elements.iter().enumerate() {
                    match elem {
                        ArrayPatternElement::Pattern(pat) => {
                            let index_expr = format!("{}[{}]", value_var, i + 1);
                            self.generate_pattern_bindings(pat, &index_expr);
                        }
                        ArrayPatternElement::Rest(ident) => {
                            // Create a slice of the rest
                            self.write_indent();
                            self.write("local ");
                            let ident_str = self.resolve(ident.node);
                            self.write(&ident_str);
                            self.write(" = {table.unpack(");
                            self.write(value_var);
                            self.write(", ");
                            self.write(&(i + 1).to_string());
                            self.write(")}");
                            self.writeln("");
                        }
                        ArrayPatternElement::Hole => {
                            // Hole doesn't bind anything
                        }
                    }
                }
            }
            Pattern::Object(object_pattern) => {
                // Bind each property
                for prop in &object_pattern.properties {
                    if let Some(value_pattern) = &prop.value {
                        let key_str = self.resolve(prop.key.node);
                        let prop_expr = format!("{}.{}", value_var, key_str);
                        self.generate_pattern_bindings(value_pattern, &prop_expr);
                    } else {
                        // Shorthand: bind the key directly
                        self.write_indent();
                        self.write("local ");
                        let key_str = self.resolve(prop.key.node);
                        self.write(&key_str);
                        self.write(" = ");
                        self.write(value_var);
                        self.write(".");
                        self.write(&key_str);
                        self.writeln("");
                    }
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_, _) => {
                // These don't bind anything
            }
        }
    }

    // Module System Code Generation

    fn generate_import(&mut self, import: &ImportDeclaration) {
        // Determine the require function and module path based on mode
        let (require_fn, module_path) = match &self.mode {
            CodeGenMode::Bundle { .. } => {
                // In bundle mode, use __require with resolved module ID
                let resolved_id = self
                    .import_map
                    .get(&import.source)
                    .cloned()
                    .unwrap_or_else(|| import.source.clone());
                ("__require", resolved_id)
            }
            CodeGenMode::Require => {
                // In require mode, use standard require with original source
                ("require", import.source.clone())
            }
        };

        match &import.clause {
            ImportClause::TypeOnly(_) => {
                // Type-only imports are completely erased
            }
            ImportClause::Named(specs) => {
                // Generate: local _mod = require("source") or __require("module_id")
                // Then: local foo, bar = _mod.foo, _mod.bar
                self.write_indent();
                self.write("local _mod = ");
                self.write(require_fn);
                self.write("(\"");
                self.write(&module_path);
                self.writeln("\")");

                self.write_indent();
                self.write("local ");
                for (i, spec) in specs.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    let local_name = spec.local.as_ref().unwrap_or(&spec.imported);
                    let name_str = self.resolve(local_name.node);
                    self.write(&name_str);
                }
                self.write(" = ");
                for (i, spec) in specs.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write("_mod.");
                    let imported_str = self.resolve(spec.imported.node);
                    self.write(&imported_str);
                }
                self.writeln("");
            }
            ImportClause::Default(ident) => {
                // Generate: local foo = require("source") or __require("module_id")
                self.write_indent();
                self.write("local ");
                let ident_str = self.resolve(ident.node);
                self.write(&ident_str);
                self.write(" = ");
                self.write(require_fn);
                self.write("(\"");
                self.write(&module_path);
                self.writeln("\")");
            }
            ImportClause::Namespace(ident) => {
                // Generate: local foo = require("source") or __require("module_id")
                self.write_indent();
                self.write("local ");
                let ident_str = self.resolve(ident.node);
                self.write(&ident_str);
                self.write(" = ");
                self.write(require_fn);
                self.write("(\"");
                self.write(&module_path);
                self.writeln("\")");
            }
        }
    }

    fn generate_export(&mut self, export: &ExportDeclaration) {
        match &export.kind {
            ExportKind::Declaration(stmt) => {
                // Generate the declaration normally
                self.generate_statement(stmt);

                // Track the exported symbol name
                if let Some(name) = self.get_declaration_name(stmt) {
                    self.exports.push(self.resolve(name).to_string());
                }
            }
            ExportKind::Named { specifiers, source } => {
                if let Some(source_path) = source {
                    // Re-export: export { foo } from './bar'
                    // Generate: local _mod = require("bar")
                    //           local foo = _mod.foo
                    self.generate_re_export(specifiers, source_path);
                } else {
                    // Regular named export
                    // Track exported symbols
                    for spec in specifiers {
                        self.exports.push(self.resolve(spec.local.node).to_string());
                    }
                }
            }
            ExportKind::Default(expr) => {
                // Generate: local _default = expr
                self.write_indent();
                self.write("local _default = ");
                self.generate_expression(expr);
                self.writeln("");
                self.has_default_export = true;
            }
        }
    }

    fn generate_re_export(&mut self, specifiers: &[ExportSpecifier], source: &str) {
        // Determine the require function and module path based on mode
        let (require_fn, module_path) = match &self.mode {
            CodeGenMode::Bundle { .. } => {
                // In bundle mode, use __require with resolved module ID
                let resolved_id = self
                    .import_map
                    .get(source)
                    .cloned()
                    .unwrap_or_else(|| source.to_string());
                ("__require", resolved_id)
            }
            CodeGenMode::Require => {
                // In require mode, use standard require with original source
                ("require", source.to_string())
            }
        };

        // Generate: local _mod = require("source") or __require("module_id")
        self.write_indent();
        self.write("local _mod = ");
        self.write(require_fn);
        self.write("(\"");
        self.write(&module_path);
        self.writeln("\")");

        // Import each symbol and track for export
        for spec in specifiers {
            // Generate: local foo = _mod.foo
            self.write_indent();
            self.write("local ");
            let local_str = self.resolve(spec.local.node);
            self.write(&local_str);
            self.write(" = _mod.");
            self.write(&local_str);
            self.writeln("");

            // Track the symbol for export
            self.exports.push(local_str);
        }
    }

    fn finalize_module(&mut self) {
        if self.current_namespace.is_some() {
            return;
        }

        // If there are exports or default export, create module table
        if !self.exports.is_empty() || self.has_default_export {
            self.writeln("");
            self.writeln("local M = {}");

            // Add named exports (clone to avoid borrow checker issues)
            let exports = self.exports.clone();
            for name in &exports {
                self.write("M.");
                self.write(name);
                self.write(" = ");
                self.writeln(name);
            }

            // Add default export
            if self.has_default_export {
                self.writeln("M.default = _default");
            }

            self.writeln("return M");
        }
    }

    fn finalize_namespace(&mut self) {
        if let Some(ns_path) = &self.current_namespace {
            if !ns_path.is_empty() {
                let ns_root = ns_path[0].clone();
                self.writeln("");
                self.write("return ");
                self.writeln(&ns_root);
            }
        }
    }

    fn get_declaration_name(&self, stmt: &Statement) -> Option<crate::string_interner::StringId> {
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

    fn generate_enum_declaration(&mut self, enum_decl: &EnumDeclaration) {
        let enum_name = self.resolve(enum_decl.name.node).to_string();

        if enum_decl.fields.is_empty()
            && enum_decl.constructor.is_none()
            && enum_decl.methods.is_empty()
        {
            self.write_indent();
            self.write("local ");
            self.write(&enum_name);
            self.write(" = {");
            self.writeln("");
            self.indent();
            for (i, member) in enum_decl.members.iter().enumerate() {
                self.write_indent();
                let member_name = self.resolve(member.name.node);
                self.write(&member_name);
                self.write(" = ");
                if let Some(value) = &member.value {
                    match value {
                        EnumValue::Number(n) => self.write(&n.to_string()),
                        EnumValue::String(s) => {
                            self.write("\"");
                            self.write(s);
                            self.write("\"");
                        }
                    }
                } else {
                    self.write(&i.to_string());
                }
                if i < enum_decl.members.len() - 1 {
                    self.write(",");
                }
                self.writeln("");
            }
            self.dedent();
            self.write_indent();
            self.writeln("}");
        } else {
            self.generate_rich_enum_declaration(enum_decl, &enum_name);
        }
    }

    fn generate_rich_enum_declaration(&mut self, enum_decl: &EnumDeclaration, enum_name: &str) {
        let mt_name = format!("{}__mt", enum_name);

        self.writeln("");
        self.write_indent();
        self.writeln(&format!("local {} = {}", enum_name, "{}"));

        self.write_indent();
        self.writeln(&format!("{}.__index = {}", enum_name, enum_name));

        self.write_indent();
        self.write(&format!("local {} = {{}}", mt_name));
        self.writeln("");
        self.write_indent();
        self.writeln(&format!("setmetatable({}, {})", mt_name, enum_name));

        self.write_indent();
        self.writeln(&format!("setmetatable({}, {{", enum_name));
        self.indent();
        self.write_indent();
        self.writeln(&format!("__metatable = {}", mt_name));
        self.write_indent();
        self.writeln("__call = function()");
        self.indent();
        self.write_indent();
        self.writeln(&format!(
            "error(\"Cannot instantiate enum {} directly\")",
            enum_name
        ));
        self.dedent();
        self.write_indent();
        self.writeln("end");
        self.dedent();
        self.write_indent();
        self.writeln("})");

        self.write_indent();
        self.write("function ");
        self.write(&mt_name);
        self.writeln(".__index(table, key)");
        self.indent();
        self.write_indent();
        self.writeln("return nil");
        self.dedent();
        self.write_indent();
        self.writeln("end");

        self.writeln("");
        self.write_indent();
        self.write("local function ");
        self.write(enum_name);
        self.write("__new(name, ordinal");

        for field in &enum_decl.fields {
            self.write(", ");
            self.write(&self.resolve(field.name.node));
        }
        self.writeln(")");
        self.indent();
        self.write_indent();
        self.write("local self = setmetatable({}, ");
        self.write(enum_name);
        self.writeln(")");
        self.dedent();

        self.writeln("");
        self.write_indent();
        self.writeln(&format!("{}.__values = {{}}", enum_name));

        self.writeln("");
        self.write_indent();
        self.write(&format!("{}.__byName = {{", enum_name));
        for (i, member) in enum_decl.members.iter().enumerate() {
            let member_name = self.resolve(member.name.node);
            self.write(&member_name);
            self.write(" = ");
            self.write(enum_name);
            self.write(".");
            self.write(&member_name);
            if i < enum_decl.members.len() - 1 {
                self.write(", ");
            }
        }
        self.writeln("}");

        let is_o2_or_higher = self.optimization_level.effective() >= OptimizationLevel::O2;

        for (i, member) in enum_decl.members.iter().enumerate() {
            self.writeln("");
            self.write_indent();
            self.write(enum_name);
            self.write(".");
            let member_name = self.resolve(member.name.node);
            self.write(&member_name);

            if is_o2_or_higher {
                self.writeln(" = setmetatable({");
                self.indent();
                self.write_indent();
                self.writeln(&format!("__name = \"{}\",", member_name));
                self.write_indent();
                self.writeln(&format!("__ordinal = {},", i));

                for (j, field) in enum_decl.fields.iter().enumerate() {
                    let field_name = self.resolve(field.name.node);
                    if j < member.arguments.len() {
                        self.write_indent();
                        self.write(&format!("{} = ", field_name));
                        self.generate_expression(&member.arguments[j]);
                        self.writeln(",");
                    } else {
                        self.write_indent();
                        self.writeln(&format!("{} = nil,", field_name));
                    }
                }

                self.dedent();
                self.write_indent();
                self.write("}, ");
                self.write(enum_name);
                self.writeln(")");
            } else {
                self.write(" = ");
                self.write(enum_name);
                self.write("__new(\"");
                self.write(&member_name);
                self.write("\", ");
                self.write(&i.to_string());

                for arg in &member.arguments {
                    self.write(", ");
                    self.generate_expression(arg);
                }
                self.writeln(")");
            }

            self.write_indent();
            self.write("table.insert(");
            self.write(enum_name);
            self.write(".__values, ");
            self.write(enum_name);
            self.write(".");
            self.write(&member_name);
            self.writeln(")");
        }
        self.write_indent();
        self.writeln(&format!("setmetatable({}, {})", enum_name, mt_name));

        self.writeln("");
        self.writeln(&enum_rt::ENUM_NAME.replace("{}", enum_name));
        self.writeln("");
        self.writeln(&enum_rt::ENUM_ORDINAL.replace("{}", enum_name));
        self.writeln("");
        self.writeln(
            &enum_rt::ENUM_VALUES
                .replace("{}", &enum_name)
                .replace("{}", enum_name),
        );
        self.writeln("");
        self.writeln(
            &enum_rt::ENUM_VALUE_OF
                .replace("{}", &enum_name)
                .replace("{}", enum_name),
        );

        for method in &enum_decl.methods {
            self.writeln("");
            self.write_indent();
            self.write(&format!(
                "function {}:{}",
                enum_name,
                self.resolve(method.name.node)
            ));
            self.write("(");
            for (i, param) in method.parameters.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                match &param.pattern {
                    Pattern::Identifier(ident) => {
                        self.write(&self.resolve(ident.node));
                    }
                    _ => {
                        self.write("_");
                    }
                }
            }
            self.writeln(")");
            self.indent();
            self.generate_block(&method.body);
            self.dedent();
            self.write_indent();
            self.writeln("end");
        }
    }

    fn generate_throw_statement(&mut self, stmt: &ThrowStatement) {
        self.write_indent();
        self.write("error(");
        self.generate_expression(&stmt.expression);
        self.writeln(")");
    }

    fn generate_rethrow_statement(&mut self, _span: Span) {
        self.write_indent();
        self.writeln("error(__error)");
    }

    fn generate_namespace_declaration(&mut self, ns: &NamespaceDeclaration) {
        let path: Vec<String> = ns
            .path
            .iter()
            .map(|ident| self.resolve(ident.node))
            .collect();

        if path.is_empty() {
            return;
        }

        self.current_namespace = Some(path.clone());

        self.write_indent();
        self.writeln(&format!("-- Namespace: {}", path.join(".")));

        self.write_indent();
        self.writeln(&format!("local {} = {}", path[0], "{}"));

        for i in 1..path.len() {
            self.write_indent();
            self.writeln(&format!("{}.{} = {}", path[0], path[i], "{}"));
        }

        self.writeln("");
    }

    fn generate_try_statement(&mut self, stmt: &TryStatement) {
        self.write_indent();
        self.writeln("-- try block");

        let has_typed_catches = stmt.catch_clauses.iter().any(|clause| {
            matches!(
                clause.pattern,
                CatchPattern::Typed { .. } | CatchPattern::MultiTyped { .. }
            )
        });

        let has_finally = stmt.finally_block.is_some();
        let needs_xpcall = has_typed_catches || has_finally;
        let prefer_xpcall = matches!(
            self.optimization_level,
            OptimizationLevel::O2 | OptimizationLevel::O3 | OptimizationLevel::Auto
        );

        if needs_xpcall || prefer_xpcall {
            self.generate_try_xpcall(stmt);
        } else {
            self.generate_try_pcall(stmt);
        }
    }

    fn generate_try_pcall(&mut self, stmt: &TryStatement) {
        self.write_indent();
        self.writeln("local __ok, __result = pcall(function()");

        self.indent();
        self.generate_block(&stmt.try_block);
        self.dedent();

        self.write_indent();
        self.writeln("end)");

        self.write_indent();
        self.writeln("if not __ok then");

        self.indent();
        self.write_indent();
        self.write("local __error = __result");

        for (i, catch_clause) in stmt.catch_clauses.iter().enumerate() {
            self.generate_catch_clause_pcall(catch_clause, i == stmt.catch_clauses.len() - 1);
        }
        self.dedent();

        self.write_indent();
        self.writeln("end");

        if let Some(finally_block) = &stmt.finally_block {
            self.generate_finally_block(finally_block);
        }
    }

    fn generate_try_xpcall(&mut self, stmt: &TryStatement) {
        self.write_indent();
        self.writeln("local __error");
        self.write_indent();
        self.writeln("xpcall(function()");

        self.indent();
        self.generate_block(&stmt.try_block);
        self.dedent();

        self.write_indent();
        self.writeln("end, ");

        let has_typed_catches = stmt.catch_clauses.iter().any(|clause| {
            matches!(
                clause.pattern,
                CatchPattern::Typed { .. } | CatchPattern::MultiTyped { .. }
            )
        });

        let use_debug_traceback = matches!(
            self.optimization_level,
            OptimizationLevel::O2 | OptimizationLevel::O3 | OptimizationLevel::Auto
        ) && !has_typed_catches;

        if use_debug_traceback {
            self.writeln("debug.traceback)");
        } else {
            self.writeln("function(__err)");
            self.indent();
            self.write_indent();
            self.writeln("__error = __err");

            for catch_clause in &stmt.catch_clauses {
                self.generate_catch_clause_xpcall(catch_clause);
            }

            self.dedent();
            self.write_indent();
            self.writeln("end)");
        }

        if use_debug_traceback {
            self.write_indent();
            self.writeln("if __error == nil then return end");
            self.write_indent();
            self.writeln("local e = __error");
        }

        for catch_clause in &stmt.catch_clauses {
            self.generate_block(&catch_clause.body);
        }

        if let Some(finally_block) = &stmt.finally_block {
            self.generate_finally_block(finally_block);
        }
    }

    fn generate_catch_clause_pcall(&mut self, clause: &CatchClause, is_last: bool) {
        let var_name = match &clause.pattern {
            CatchPattern::Untyped { variable, .. }
            | CatchPattern::Typed { variable, .. }
            | CatchPattern::MultiTyped { variable, .. } => self.resolve(variable.node),
        };

        self.write_indent();
        if is_last {
            self.writeln("else");
        } else {
            self.writeln("elseif false then");
        }

        self.indent();
        self.write_indent();
        self.writeln(&format!("local {} = __error", var_name));
        self.generate_block(&clause.body);
        self.dedent();
    }

    fn generate_catch_clause_xpcall(&mut self, clause: &CatchClause) {
        let var_name = match &clause.pattern {
            CatchPattern::Untyped { variable, .. }
            | CatchPattern::Typed { variable, .. }
            | CatchPattern::MultiTyped { variable, .. } => self.resolve(variable.node),
        };

        self.write_indent();
        self.writeln(&format!("if {} == nil then", var_name));
        self.indent();
        self.write_indent();
        self.writeln("return false");
        self.dedent();
        self.write_indent();
        self.writeln("end");
    }

    fn generate_finally_block(&mut self, block: &Block) {
        self.write_indent();
        self.writeln("-- finally block");
        self.generate_block(block);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use typedlua_parser::lexer::Lexer;
    use typedlua_parser::parser::Parser;
    use typedlua_parser::string_interner::StringInterner;

    fn generate_code(source: &str) -> String {
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let interner = Arc::new(interner);
        let mut lexer = Lexer::new(source, handler.clone(), &interner);
        let tokens = lexer.tokenize().expect("Lexing failed");
        let mut parser = Parser::new(tokens, handler, &interner, &common);
        let mut program = parser.parse().expect("Parsing failed");

        let mut generator = CodeGenerator::new(interner.clone());
        generator.generate(&mut program)
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
        println!("Generated output:\n{}", output);
        assert!(output.contains("while (x < 10) do"));
        // Assignment is parsed and will be in the output
        assert!(output.trim().len() > 0);
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
        let mut lexer = Lexer::new(source, handler.clone(), &interner);
        let tokens = lexer.tokenize().expect("Lexing failed");
        let mut parser = Parser::new(tokens, handler, &interner, &common);
        let mut program = parser.parse().expect("Parsing failed");

        let mut generator = CodeGenerator::new(interner.clone()).with_target(target);
        generator.generate(&mut program)
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
        let (mut interner, common) = StringInterner::new_with_common_identifiers();
        let mut lexer = Lexer::new(&generated, handler.clone(), &mut interner);
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler, &mut interner, &common);
        let _program = parser.parse().expect("Roundtrip parsing failed");
    }

    fn parse_roundtrip(source: &str) {
        let generated = generate_code(source);

        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let (mut interner, common) = StringInterner::new_with_common_identifiers();
        let mut lexer = Lexer::new(&generated, handler.clone(), &mut interner);
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler, &mut interner, &common);
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
        let (mut interner, common) = StringInterner::new_with_common_identifiers();
        let mut lexer = Lexer::new(&output, handler.clone(), &mut interner);
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler, &mut interner, &common);
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
        println!("Generated:\n{}", output);

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
        println!("Generated:\n{}", output);

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
        println!("Generated:\n{}", output);

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
        println!("Generated:\n{}", output);

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
        println!("Generated:\n{}", output);

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
        println!("Generated:\n{}", output);

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
        println!("Generated:\n{}", output);

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
        println!("Generated:\n{}", output);

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
        println!("Generated:\n{}", output);

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
        println!("Generated:\n{}", output);

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
}
