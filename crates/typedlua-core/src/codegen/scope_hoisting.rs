use rustc_hash::FxHashSet as HashSet;
use typedlua_parser::ast::expression::*;
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{ExportKind, Statement, VariableDeclaration, VariableKind};
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::StringId;
use typedlua_parser::string_interner::StringInterner;

#[derive(Debug, Clone, Default)]
pub struct HoistableDeclarations {
    pub functions: HashSet<String>,
    pub variables: HashSet<String>,
    pub classes: HashSet<String>,
    pub enums: HashSet<String>,
}

impl HoistableDeclarations {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_hoistable(&self, name: &str, kind: &DeclarationKind) -> bool {
        match kind {
            DeclarationKind::Function => self.functions.contains(name),
            DeclarationKind::Variable => self.variables.contains(name),
            DeclarationKind::Class => self.classes.contains(name),
            DeclarationKind::Enum => self.enums.contains(name),
        }
    }

    pub fn all_names(&self) -> HashSet<String> {
        let mut names = HashSet::default();
        names.extend(self.functions.clone());
        names.extend(self.variables.clone());
        names.extend(self.classes.clone());
        names.extend(self.enums.clone());
        names
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationKind {
    Function,
    Variable,
    Class,
    Enum,
}

#[derive(Debug, Clone)]
pub struct EscapeAnalysis<'a> {
    interner: &'a StringInterner,
    exported_names: HashSet<String>,
    return_values: HashSet<String>,
    module_locals: HashSet<String>,
    variable_declarations: HashSet<String>,
}

impl<'a> EscapeAnalysis<'a> {
    pub fn new(interner: &'a StringInterner) -> Self {
        EscapeAnalysis {
            interner,
            exported_names: HashSet::default(),
            return_values: HashSet::default(),
            module_locals: HashSet::default(),
            variable_declarations: HashSet::default(),
        }
    }

    pub fn analyze(program: &Program, interner: &StringInterner) -> HoistableDeclarations {
        let mut analysis = EscapeAnalysis::new(interner);
        analysis.collect_module_locals(program);
        analysis.collect_exports(program);
        analysis.collect_return_values_from_private_functions(program);
        let result = analysis.find_hoistable_declarations(program);
        // Debug: log what was found
        if cfg!(test) && !analysis.module_locals.is_empty() {
            eprintln!("DEBUG module_locals: {:?}", analysis.module_locals);
            eprintln!("DEBUG exported_names: {:?}", analysis.exported_names);
            eprintln!("DEBUG return_values: {:?}", analysis.return_values);
            eprintln!("DEBUG hoistable result: funcs={:?}, vars={:?}, classes={:?}, enums={:?}",
                result.functions, result.variables, result.classes, result.enums);
        }
        result
    }

    fn collect_module_locals(&mut self, program: &Program) {
        for statement in program.statements.iter() {
            if let Some((_, name)) = self.get_declaration_name(statement) {
                self.module_locals.insert(name.clone());
                // Track which ones are variables
                let is_variable = match statement {
                    Statement::Variable(_) => true,
                    Statement::Export(e) => {
                        matches!(&e.kind, ExportKind::Declaration(inner) if matches!(inner, Statement::Variable(_)))
                    }
                    _ => false,
                };
                if is_variable {
                    self.variable_declarations.insert(name);
                }
            }
        }
    }

    fn collect_exports(&mut self, program: &Program) {
        for statement in program.statements.iter() {
            if let Statement::Export(decl) = statement {
                match &decl.kind {
                    ExportKind::Declaration(inner) => {
                        if let Some((_, name)) = self.get_declaration_name(inner) {
                            self.exported_names.insert(name);
                        }
                    }
                    ExportKind::Named { specifiers, .. } => {
                        for spec in specifiers.iter() {
                            let local_id = spec.local.node;
                            let local_name = self.resolve_string(local_id);
                            self.exported_names.insert(local_name);
                        }
                    }
                    ExportKind::Default(_) => {
                        self.exported_names.insert("default".to_string());
                    }
                }
            }
        }
    }

    fn collect_return_values_from_private_functions(&mut self, program: &Program) {
        // Only track returns from PRIVATE functions
        // Returns from exported functions don't prevent hoisting (they're part of the public API)
        for statement in program.statements.iter() {
            match statement {
                Statement::Function(func) => {
                    // Track returns from private functions
                    self.walk_statements_for_returns_skip_functions(func.body.statements);
                }
                Statement::Return(ret) => {
                    // Module-level return
                    for value in ret.values.iter() {
                        if let ExpressionKind::Identifier(ident) = &value.kind {
                            let name = self.resolve_string(*ident);
                            // Track ALL returns (variables, classes, enums, functions)
                            self.return_values.insert(name);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn walk_statements_for_returns_skip_functions(&mut self, statements: &[Statement]) {
        for statement in statements {
            match statement {
                Statement::Return(ret) => {
                    for value in ret.values.iter() {
                        if let ExpressionKind::Identifier(ident) = &value.kind {
                            let name = self.resolve_string(*ident);
                            // Track ALL returns (variables, classes, enums, functions)
                            self.return_values.insert(name);
                        }
                    }
                }
                Statement::If(if_stmt) => {
                    self.walk_statements_for_returns_skip_functions(if_stmt.then_block.statements);
                    for elseif in if_stmt.else_ifs.iter() {
                        self.walk_statements_for_returns_skip_functions(elseif.block.statements);
                    }
                    if let Some(else_block) = &if_stmt.else_block {
                        self.walk_statements_for_returns_skip_functions(else_block.statements);
                    }
                }
                Statement::While(while_stmt) => {
                    self.walk_statements_for_returns_skip_functions(while_stmt.body.statements);
                }
                Statement::For(for_stmt) => match for_stmt {
                    typedlua_parser::ast::statement::ForStatement::Numeric(num) => {
                        self.walk_statements_for_returns_skip_functions(num.body.statements);
                    }
                    typedlua_parser::ast::statement::ForStatement::Generic(generic) => {
                        self.walk_statements_for_returns_skip_functions(generic.body.statements);
                    }
                },
                Statement::Repeat(repeat_stmt) => {
                    self.walk_statements_for_returns_skip_functions(repeat_stmt.body.statements);
                }
                Statement::Block(block) => {
                    self.walk_statements_for_returns_skip_functions(block.statements);
                }
                _ => {}
            }
        }
    }

    fn find_hoistable_declarations(&self, program: &Program) -> HoistableDeclarations {
        let mut hoistable = HoistableDeclarations::new();

        for statement in program.statements.iter() {
            self.check_declaration_hoistability(statement, &mut hoistable);
        }

        hoistable
    }

    fn check_declaration_hoistability(
        &self,
        statement: &Statement,
        hoistable: &mut HoistableDeclarations,
    ) {
        match statement {
            Statement::Function(decl) => {
                let name = self.resolve_string(decl.name.node);
                if !self.is_exported_by_name(&name) && self.can_hoist_function(decl, &name) {
                    hoistable.functions.insert(name);
                }
            }
            Statement::Variable(decl) => {
                if let Pattern::Identifier(ident) = &decl.pattern {
                    let name = self.resolve_string(ident.node);
                    if !self.is_exported_by_name(&name) && self.can_hoist_variable(decl, &name) {
                        hoistable.variables.insert(name);
                    }
                }
            }
            Statement::Class(decl) => {
                let name = self.resolve_string(decl.name.node);
                if !self.is_exported_by_name(&name) && self.can_hoist_class(decl, &name) {
                    hoistable.classes.insert(name);
                }
            }
            Statement::Enum(decl) => {
                let name = self.resolve_string(decl.name.node);
                if !self.is_exported_by_name(&name) && self.can_hoist_enum(decl, &name) {
                    hoistable.enums.insert(name);
                }
            }
            Statement::Export(export_decl) => {
                // Check the inner declaration (though it will be marked as exported)
                if let ExportKind::Declaration(inner) = &export_decl.kind {
                    self.check_declaration_hoistability(inner, hoistable);
                }
            }
            _ => {}
        }
    }

    fn is_exported_by_name(&self, name: &str) -> bool {
        self.exported_names.contains(name)
    }

    fn can_hoist_function(
        &self,
        decl: &typedlua_parser::ast::statement::FunctionDeclaration,
        _name: &str,
    ) -> bool {
        // Only concern: can the function be relocated to a higher scope?
        // Can't hoist if the function itself returns other module-locals
        // (losing access to them at the higher scope would break the function)
        !self.function_returns_any_local(decl.body.statements)
    }

    fn function_returns_any_local(&self, statements: &[Statement]) -> bool {
        for statement in statements {
            match statement {
                Statement::Return(ret) => {
                    for value in ret.values.iter() {
                        if let ExpressionKind::Identifier(ident) = &value.kind {
                            let name = self.resolve_string(*ident);
                            if self.module_locals.contains(&name) {
                                return true;
                            }
                        }
                    }
                }
                Statement::If(if_stmt) => {
                    if self.function_returns_any_local(if_stmt.then_block.statements) {
                        return true;
                    }
                    for elseif in if_stmt.else_ifs.iter() {
                        if self.function_returns_any_local(elseif.block.statements) {
                            return true;
                        }
                    }
                    if let Some(else_block) = &if_stmt.else_block {
                        if self.function_returns_any_local(else_block.statements) {
                            return true;
                        }
                    }
                }
                Statement::While(while_stmt) => {
                    if self.function_returns_any_local(while_stmt.body.statements) {
                        return true;
                    }
                }
                Statement::For(for_stmt) => {
                    let body = match for_stmt {
                        typedlua_parser::ast::statement::ForStatement::Numeric(num) => {
                            num.body.statements
                        }
                        typedlua_parser::ast::statement::ForStatement::Generic(generic) => {
                            generic.body.statements
                        }
                    };
                    if self.function_returns_any_local(body) {
                        return true;
                    }
                }
                Statement::Repeat(repeat_stmt) => {
                    if self.function_returns_any_local(repeat_stmt.body.statements) {
                        return true;
                    }
                }
                Statement::Block(block) => {
                    if self.function_returns_any_local(block.statements) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn can_hoist_variable(&self, decl: &VariableDeclaration, name: &str) -> bool {
        match decl.kind {
            VariableKind::Const => {
                // Rule 1: Can't hoist if initializer references other module-local declarations
                if self.initializer_references_any_local(&decl.initializer, name) {
                    return false;
                }

                // Rule 2: Can't hoist if returned from any function
                if self.returns_local_by_name(name) {
                    return false;
                }

                // Rule 3: Can't hoist if initializer is a function expression
                if matches!(decl.initializer.kind, ExpressionKind::Function(_)) {
                    return false;
                }

                // Rule 4: Can't hoist if initializer is a complex type (object or array)
                // These are typically returned or have external dependencies
                if matches!(decl.initializer.kind, ExpressionKind::Object(_) | ExpressionKind::Array(_)) {
                    return false;
                }

                true
            }
            VariableKind::Local => false,
        }
    }

    fn returns_local_by_name(&self, name: &str) -> bool {
        self.return_values.contains(name)
    }

    fn initializer_references_any_local(&self, expr: &Expression, self_name: &str) -> bool {
        self.walk_expression_checking_any_local(expr, self_name)
    }

    fn walk_expression_checking_any_local(&self, expr: &Expression, self_name: &str) -> bool {
        match &expr.kind {
            ExpressionKind::Identifier(ident) => {
                let name = self.resolve_string(*ident);
                // Check if this identifier refers to a module-local (other than self)
                name != self_name && self.module_locals.contains(&name)
            }
            ExpressionKind::Object(props) => {
                for prop in props.iter() {
                    match prop {
                        ObjectProperty::Property { value, .. } => {
                            if self.walk_expression_checking_any_local(value, self_name) {
                                return true;
                            }
                        }
                        ObjectProperty::Computed { key, value, .. } => {
                            if self.walk_expression_checking_any_local(key, self_name)
                                || self.walk_expression_checking_any_local(value, self_name)
                            {
                                return true;
                            }
                        }
                        ObjectProperty::Spread { value, .. } => {
                            if self.walk_expression_checking_any_local(value, self_name) {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            ExpressionKind::Array(elements) => {
                for elem in elements.iter() {
                    match elem {
                        ArrayElement::Expression(expr) => {
                            if self.walk_expression_checking_any_local(expr, self_name) {
                                return true;
                            }
                        }
                        ArrayElement::Spread(expr) => {
                            if self.walk_expression_checking_any_local(expr, self_name) {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            ExpressionKind::Function(func) => {
                self.walk_statements_checking_any_local(func.body.statements, self_name)
            }
            ExpressionKind::MethodCall(_, _, args, _) => {
                for arg in args.iter() {
                    if self.walk_expression_checking_any_local(&arg.value, self_name) {
                        return true;
                    }
                }
                false
            }
            ExpressionKind::Call(_, args, _) => {
                for arg in args.iter() {
                    if self.walk_expression_checking_any_local(&arg.value, self_name) {
                        return true;
                    }
                }
                false
            }
            ExpressionKind::Binary(_, left, right) => {
                self.walk_expression_checking_any_local(left, self_name)
                    || self.walk_expression_checking_any_local(right, self_name)
            }
            ExpressionKind::Unary(_, operand) => {
                self.walk_expression_checking_any_local(operand, self_name)
            }
            ExpressionKind::Parenthesized(inner) => {
                self.walk_expression_checking_any_local(inner, self_name)
            }
            ExpressionKind::TypeAssertion(inner, _) => {
                self.walk_expression_checking_any_local(inner, self_name)
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.walk_expression_checking_any_local(cond, self_name)
                    || self.walk_expression_checking_any_local(then_expr, self_name)
                    || self.walk_expression_checking_any_local(else_expr, self_name)
            }
            ExpressionKind::New(new_expr, args, _) => {
                if self.walk_expression_checking_any_local(new_expr, self_name) {
                    return true;
                }
                for arg in args.iter() {
                    if self.walk_expression_checking_any_local(&arg.value, self_name) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn walk_statements_checking_any_local(&self, statements: &[Statement], self_name: &str) -> bool {
        for statement in statements {
            match statement {
                Statement::Return(ret) => {
                    for value in ret.values.iter() {
                        if self.walk_expression_checking_any_local(value, self_name) {
                            return true;
                        }
                    }
                }
                Statement::If(if_stmt) => {
                    if self.walk_statements_checking_any_local(if_stmt.then_block.statements, self_name)
                    {
                        return true;
                    }
                    for elseif in if_stmt.else_ifs.iter() {
                        if self.walk_statements_checking_any_local(elseif.block.statements, self_name) {
                            return true;
                        }
                    }
                    if let Some(else_block) = &if_stmt.else_block {
                        if self.walk_statements_checking_any_local(else_block.statements, self_name) {
                            return true;
                        }
                    }
                }
                Statement::While(while_stmt) => {
                    if self.walk_statements_checking_any_local(while_stmt.body.statements, self_name) {
                        return true;
                    }
                }
                Statement::Block(block) => {
                    if self.walk_statements_checking_any_local(block.statements, self_name) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn can_hoist_class(
        &self,
        _decl: &typedlua_parser::ast::statement::ClassDeclaration,
        name: &str,
    ) -> bool {
        // Can't hoist if the class is returned directly from a private function
        !self.returns_local_by_name(name)
    }

    fn can_hoist_enum(
        &self,
        _decl: &typedlua_parser::ast::statement::EnumDeclaration,
        name: &str,
    ) -> bool {
        // Can't hoist if the enum is returned directly from a private function
        !self.returns_local_by_name(name)
    }


    fn get_declaration_name(&self, stmt: &Statement) -> Option<(StringId, String)> {
        match stmt {
            Statement::Function(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Class(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Enum(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Interface(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::TypeAlias(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Variable(decl) => {
                if let Pattern::Identifier(ident) = &decl.pattern {
                    let id = ident.node;
                    let name = self.resolve_string(id);
                    Some((id, name))
                } else {
                    None
                }
            }
            Statement::Export(export_decl) => {
                // Unwrap export declarations to get the inner declaration
                if let ExportKind::Declaration(inner) = &export_decl.kind {
                    self.get_declaration_name(inner)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn resolve_string(&self, id: StringId) -> String {
        self.interner.resolve(id).to_string()
    }
}

// ============================================================
// Name Mangling for Scope Hoisting
// ============================================================

/// Name mangler for scope hoisting.
///
/// When declarations are hoisted from module scope to top-level bundle scope,
/// their names must be mangled to avoid collisions with declarations from other modules.
///
/// The mangling strategy converts `module/path/to/file.helper` → `module_path_to_file__helper`
#[derive(Debug, Clone)]
pub struct NameMangler {
    /// Maps (module_id, original_name) -> mangled_name
    mangled_names: std::collections::HashMap<(String, String), String>,
    /// Tracks all mangled names to detect collisions
    used_names: HashSet<String>,
    /// Entry module ID (names from entry point are preserved)
    entry_module_id: Option<String>,
    /// Reserved names that cannot be used (Lua keywords, runtime names, etc.)
    reserved_names: HashSet<String>,
}

impl Default for NameMangler {
    fn default() -> Self {
        Self::new()
    }
}

impl NameMangler {
    /// Create a new name mangler
    pub fn new() -> Self {
        let mut reserved = HashSet::default();
        // Lua keywords
        for keyword in &[
            "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto",
            "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until",
            "while",
        ] {
            reserved.insert((*keyword).to_string());
        }
        // TypedLua runtime names
        for name in &[
            "__modules",
            "__loaded",
            "__require",
            "__TypeRegistry",
            "__TypeIdToClass",
            "Reflect",
            "self",
            "_G",
            "_VERSION",
        ] {
            reserved.insert((*name).to_string());
        }

        Self {
            mangled_names: Default::default(),
            used_names: HashSet::default(),
            entry_module_id: None,
            reserved_names: reserved,
        }
    }

    /// Set the entry module ID. Names exported from this module are preserved.
    pub fn with_entry_module(mut self, entry_module_id: &str) -> Self {
        self.entry_module_id = Some(entry_module_id.to_string());
        self
    }

    /// Check if a name is reserved and cannot be used
    fn is_reserved(&self, name: &str) -> bool {
        self.reserved_names.contains(name)
    }

    /// Mangle a module path into a valid Lua identifier prefix.
    ///
    /// Converts path separators and dots to underscores, removes file extensions.
    ///
    /// # Examples
    /// - `"module/path/to/file"` → `"module_path_to_file"`
    /// - `"./src/utils.lua"` → `"src_utils"`
    /// - `"@mylib/helpers"` → `"mylib_helpers"`
    fn mangle_module_path(module_path: &str) -> String {
        let mut result = String::new();

        // Remove leading ./ or /
        let path = module_path
            .trim_start_matches("./")
            .trim_start_matches('/');

        // Remove file extension
        let path = path
            .trim_end_matches(".lua")
            .trim_end_matches(".tl")
            .trim_end_matches("/index");

        for ch in path.chars() {
            match ch {
                // Replace path separators and dots with underscore
                '/' | '.' | '-' => {
                    // Avoid consecutive underscores
                    if !result.ends_with('_') && !result.is_empty() {
                        result.push('_');
                    }
                }
                // Remove @ prefix (for scoped packages like @mylib/utils)
                '@' => {}
                // Keep alphanumeric and underscore
                c if c.is_ascii_alphanumeric() || c == '_' => {
                    result.push(c);
                }
                // Replace other characters with underscore
                _ => {
                    if !result.ends_with('_') && !result.is_empty() {
                        result.push('_');
                    }
                }
            }
        }

        // Remove trailing underscore
        while result.ends_with('_') {
            result.pop();
        }

        // Ensure it starts with a letter or underscore (valid Lua identifier)
        if result.is_empty() || result.chars().next().unwrap().is_ascii_digit() {
            result = format!("_{}", result);
        }

        result
    }

    /// Generate a mangled name for a declaration.
    ///
    /// # Arguments
    /// * `module_path` - The module path (e.g., "src/utils/helpers")
    /// * `name` - The original declaration name
    ///
    /// # Returns
    /// A unique mangled name like `src_utils_helpers__foo`
    pub fn mangle_name(&mut self, module_path: &str, name: &str) -> String {
        // Check if already mangled
        let key = (module_path.to_string(), name.to_string());
        if let Some(mangled) = self.mangled_names.get(&key) {
            return mangled.clone();
        }

        // Create the base mangled name
        let module_prefix = Self::mangle_module_path(module_path);
        let base_mangled = format!("{}__{}", module_prefix, name);

        // Handle collisions by appending a numeric suffix
        let mut mangled = base_mangled.clone();
        let mut suffix = 0;

        while self.used_names.contains(&mangled) || self.is_reserved(&mangled) {
            suffix += 1;
            mangled = format!("{}_{}", base_mangled, suffix);
        }

        // Register the mangled name
        self.used_names.insert(mangled.clone());
        self.mangled_names.insert(key, mangled.clone());

        mangled
    }

    /// Get the mangled name for a declaration if it exists.
    pub fn get_mangled_name(&self, module_path: &str, name: &str) -> Option<&String> {
        let key = (module_path.to_string(), name.to_string());
        self.mangled_names.get(&key)
    }

    /// Check if a declaration should preserve its original name.
    ///
    /// Entry point exports keep their original names since they're user-facing API.
    pub fn should_preserve_name(&self, module_path: &str, is_exported: bool) -> bool {
        if let Some(ref entry) = self.entry_module_id {
            // Preserve exported names from entry module
            module_path == entry && is_exported
        } else {
            false
        }
    }

    /// Mangle all hoistable declarations from multiple modules.
    ///
    /// # Arguments
    /// * `modules` - Iterator of (module_path, hoistable_declarations) pairs
    ///
    /// # Returns
    /// A mapping of (module_path, original_name) -> mangled_name
    pub fn mangle_all<'a>(
        &mut self,
        modules: impl Iterator<Item = (&'a str, &'a HoistableDeclarations)>,
    ) -> &std::collections::HashMap<(String, String), String> {
        for (module_path, hoistable) in modules {
            // Mangle all hoistable names from this module
            for name in hoistable.all_names() {
                self.mangle_name(module_path, &name);
            }
        }
        &self.mangled_names
    }

    /// Create a reverse mapping from mangled names back to (module, original_name).
    ///
    /// Useful for debugging and source maps.
    pub fn create_reverse_map(&self) -> std::collections::HashMap<String, (String, String)> {
        self.mangled_names
            .iter()
            .map(|((module, name), mangled)| (mangled.clone(), (module.clone(), name.clone())))
            .collect()
    }
}

/// Reference rewriter for updating identifier references after name mangling.
///
/// Used during the hoisting transform (Phase 5.6) to update all references
/// to hoisted declarations to use their mangled names.
#[derive(Debug)]
pub struct ReferenceRewriter<'a> {
    mangler: &'a NameMangler,
    current_module: String,
}

// ============================================================
// Hoisting Context for Bundle Generation (Phase 5.6)
// ============================================================

/// Context for scope hoisting during bundle generation.
///
/// Tracks which declarations can be hoisted from each module and
/// provides the mangled names for hoisted declarations.
#[derive(Debug, Clone, Default)]
pub struct HoistingContext {
    /// Hoistable declarations per module
    pub hoistable_by_module: std::collections::HashMap<String, HoistableDeclarations>,
    /// Name mangler for collision-free naming
    pub mangler: NameMangler,
    /// Whether scope hoisting is enabled
    pub enabled: bool,
}

impl HoistingContext {
    /// Create a new hoisting context
    pub fn new() -> Self {
        Self {
            hoistable_by_module: Default::default(),
            mangler: NameMangler::new(),
            enabled: true,
        }
    }

    /// Create a disabled hoisting context (no-op)
    pub fn disabled() -> Self {
        Self {
            hoistable_by_module: Default::default(),
            mangler: NameMangler::new(),
            enabled: false,
        }
    }

    /// Set the entry module for name preservation
    pub fn with_entry_module(mut self, entry_module_id: &str) -> Self {
        self.mangler = self.mangler.with_entry_module(entry_module_id);
        self
    }

    /// Analyze all modules and build the hoisting context
    pub fn analyze_modules(
        modules: &[(String, &typedlua_parser::ast::Program)],
        interner: &StringInterner,
        entry_module_id: &str,
        enabled: bool,
    ) -> Self {
        if !enabled {
            return Self::disabled();
        }

        let mut context = Self::new().with_entry_module(entry_module_id);

        // Analyze each module for hoistable declarations
        for (module_id, program) in modules {
            let hoistable = EscapeAnalysis::analyze(program, interner);
            if !hoistable.all_names().is_empty() {
                context.hoistable_by_module.insert(module_id.clone(), hoistable);
            }
        }

        // Mangle all hoistable names
        let modules_iter: Vec<(&str, &HoistableDeclarations)> = context
            .hoistable_by_module
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect();
        context.mangler.mangle_all(modules_iter.into_iter());

        context
    }

    /// Check if a declaration is hoistable in a given module
    pub fn is_hoistable(&self, module_id: &str, name: &str, kind: &DeclarationKind) -> bool {
        if !self.enabled {
            return false;
        }
        self.hoistable_by_module
            .get(module_id)
            .map(|h| h.is_hoistable(name, kind))
            .unwrap_or(false)
    }

    /// Get the mangled name for a hoisted declaration
    pub fn get_mangled_name(&self, module_id: &str, name: &str) -> Option<&String> {
        if !self.enabled {
            return None;
        }
        self.mangler.get_mangled_name(module_id, name)
    }

    /// Check if a module has any hoistable declarations
    pub fn has_hoistable_declarations(&self, module_id: &str) -> bool {
        if !self.enabled {
            return false;
        }
        self.hoistable_by_module
            .get(module_id)
            .map(|h| !h.all_names().is_empty())
            .unwrap_or(false)
    }

    /// Check if a module is fully hoistable (all declarations can be hoisted)
    ///
    /// A module is fully hoistable if:
    /// 1. It has no exports (or all exports are hoisted)
    /// 2. All top-level declarations are hoistable
    ///
    /// For fully hoistable modules, we can skip the module wrapper entirely.
    pub fn is_module_fully_hoistable(&self, _module_id: &str) -> bool {
        // For now, we don't support fully hoisting modules
        // This would require checking that all declarations are hoistable
        // AND that there are no side effects at the module level
        false
    }

    /// Get all hoistable declarations for a module
    pub fn get_hoistable_declarations(&self, module_id: &str) -> Option<&HoistableDeclarations> {
        if !self.enabled {
            return None;
        }
        self.hoistable_by_module.get(module_id)
    }

    /// Create a reference rewriter for a specific module
    pub fn create_rewriter(&self, module_id: &str) -> ReferenceRewriter<'_> {
        ReferenceRewriter::new(&self.mangler, module_id)
    }
}

impl<'a> ReferenceRewriter<'a> {
    /// Create a new reference rewriter for a specific module.
    pub fn new(mangler: &'a NameMangler, current_module: &str) -> Self {
        Self {
            mangler,
            current_module: current_module.to_string(),
        }
    }

    /// Get the rewritten name for an identifier reference.
    ///
    /// If the identifier was mangled, returns the mangled name.
    /// Otherwise, returns the original name.
    pub fn rewrite(&self, name: &str) -> String {
        self.mangler
            .get_mangled_name(&self.current_module, name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    /// Check if a name was mangled for the current module.
    pub fn is_mangled(&self, name: &str) -> bool {
        self.mangler
            .get_mangled_name(&self.current_module, name)
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;
    use crate::diagnostics::CollectingDiagnosticHandler;
    use std::sync::Arc;
    use typedlua_parser::lexer::Lexer;
    use typedlua_parser::parser::Parser;

    fn create_program<'arena>(
        source: &str,
        interner: &StringInterner,
        common: &typedlua_parser::string_interner::CommonIdentifiers,
        arena: &'arena Bump,
    ) -> Program<'arena> {
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(source, handler.clone(), interner);
        let tokens = lexer.tokenize().expect("Lexing failed");
        let mut parser = Parser::new(tokens, handler, interner, common, arena);
        parser.parse().expect("Parsing failed")
    }

    #[test]
    fn test_private_function_can_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            function helper(a, b)
                return a + b
            end

            export function main()
                return helper(1, 2)
            end
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(hoistable.functions.contains("helper"));
        assert!(!hoistable.functions.contains("main"));
    }

    #[test]
    fn test_exported_function_cannot_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            export function add(a, b)
                return a + b
            end
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.functions.contains("add"));
    }

    #[test]
    fn test_function_returning_local_cannot_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            function getHelper()
                return helper
            end

            function helper()
                return 42
            end
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.functions.contains("getHelper"));
        assert!(hoistable.functions.contains("helper"));
    }

    #[test]
    fn test_private_variable_can_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const CONSTANT = 42

            export function getValue()
                return CONSTANT
            end
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(hoistable.variables.contains("CONSTANT"));
    }

    #[test]
    fn test_exported_variable_cannot_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            export const PI = 3.14
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("PI"));
    }

    #[test]
    fn test_variable_returned_cannot_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            function getValue()
                return VALUE
            end

            const VALUE = 42
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("VALUE"));
    }

    #[test]
    fn test_private_class_can_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            class HelperClass {
                value: number
            }

            export function create()
                return HelperClass.new(42)
            end
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(hoistable.classes.contains("HelperClass"));
    }

    #[test]
    fn test_exported_class_cannot_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            export class MyClass {}
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.classes.contains("MyClass"));
    }

    #[test]
    fn test_class_returned_cannot_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            function getClass()
                return MyClass
            end

            class MyClass {}
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.classes.contains("MyClass"));
    }

    #[test]
    fn test_private_enum_can_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            enum Status {
                Active = 1,
                Inactive = 2
            }

            export function getStatus(): number
                return Status.Active
            end
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(hoistable.enums.contains("Status"));
    }

    #[test]
    fn test_exported_enum_cannot_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            export enum Color {
                Red = 1,
                Green = 2,
                Blue = 3
            }
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.enums.contains("Color"));
    }

    #[test]
    fn test_enum_returned_cannot_hoist() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            function getEnum()
                return MyEnum
            end

            enum MyEnum {
                A = 1,
                B = 2
            }
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.enums.contains("MyEnum"));
    }

    #[test]
    fn test_mixed_declarations() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const internal = 42
            function internalFunc() return 1 end
            class InternalClass {}
            enum InternalEnum { A = 1 }

            export const exposed = 1
            export function exposedFunc() return 2 end
            export class ExposedClass {}
            export enum ExposedEnum { B = 1 }
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(hoistable.variables.contains("internal"));
        assert!(hoistable.functions.contains("internalFunc"));
        assert!(hoistable.classes.contains("InternalClass"));
        assert!(hoistable.enums.contains("InternalEnum"));

        assert!(!hoistable.variables.contains("exposed"));
        assert!(!hoistable.functions.contains("exposedFunc"));
        assert!(!hoistable.classes.contains("ExposedClass"));
        assert!(!hoistable.enums.contains("ExposedEnum"));
    }

    #[test]
    fn test_named_export() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const value = 42
            const other = 100
            export { value }
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("value"));
        assert!(hoistable.variables.contains("other"));
    }

    #[test]
    fn test_variable_init_with_local_ref() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const helper = 42
            const obj = { value = helper }
            export function getObj() return obj end
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("obj"));
        assert!(hoistable.variables.contains("helper"));
    }

    #[test]
    fn test_variable_init_with_table() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const obj = { value = 42, nested = { inner = 100 } }
            export function getObj() return obj end
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("obj"));
    }

    #[test]
    fn test_variable_init_with_function() {
        let arena = Bump::new();
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const callback = function() return 1 end
            export function run() return callback() end
        "#;
        let program = create_program(source, &interner, &common, &arena);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("callback"));
    }

    // ============================================================
    // Name Mangling Tests
    // ============================================================

    #[test]
    fn test_mangle_simple_path() {
        let mut mangler = NameMangler::new();
        let mangled = mangler.mangle_name("src/utils", "helper");
        assert_eq!(mangled, "src_utils__helper");
    }

    #[test]
    fn test_mangle_nested_path() {
        let mut mangler = NameMangler::new();
        let mangled = mangler.mangle_name("src/lib/utils/helpers", "format");
        assert_eq!(mangled, "src_lib_utils_helpers__format");
    }

    #[test]
    fn test_mangle_removes_extension() {
        let mut mangler = NameMangler::new();
        let mangled = mangler.mangle_name("src/utils.lua", "helper");
        assert_eq!(mangled, "src_utils__helper");

        let mut mangler2 = NameMangler::new();
        let mangled2 = mangler2.mangle_name("src/utils.tl", "helper");
        assert_eq!(mangled2, "src_utils__helper");
    }

    #[test]
    fn test_mangle_removes_index() {
        let mut mangler = NameMangler::new();
        let mangled = mangler.mangle_name("src/utils/index", "helper");
        assert_eq!(mangled, "src_utils__helper");
    }

    #[test]
    fn test_mangle_relative_path() {
        let mut mangler = NameMangler::new();
        let mangled = mangler.mangle_name("./src/utils", "helper");
        assert_eq!(mangled, "src_utils__helper");
    }

    #[test]
    fn test_mangle_absolute_path() {
        let mut mangler = NameMangler::new();
        let mangled = mangler.mangle_name("/src/utils", "helper");
        assert_eq!(mangled, "src_utils__helper");
    }

    #[test]
    fn test_mangle_scoped_package() {
        let mut mangler = NameMangler::new();
        let mangled = mangler.mangle_name("@mylib/utils", "helper");
        assert_eq!(mangled, "mylib_utils__helper");
    }

    #[test]
    fn test_mangle_handles_dashes() {
        let mut mangler = NameMangler::new();
        let mangled = mangler.mangle_name("my-lib/my-utils", "helper");
        assert_eq!(mangled, "my_lib_my_utils__helper");
    }

    #[test]
    fn test_mangle_collision_resolution() {
        let mut mangler = NameMangler::new();

        // First use creates the base name
        let mangled1 = mangler.mangle_name("module_a", "foo");
        assert_eq!(mangled1, "module_a__foo");

        // Same module + name returns cached value
        let mangled1_again = mangler.mangle_name("module_a", "foo");
        assert_eq!(mangled1_again, "module_a__foo");

        // Different module but same resulting mangled base triggers collision
        let mangled2 = mangler.mangle_name("module/a", "foo");
        assert_eq!(mangled2, "module_a__foo_1");

        // Another collision
        let mut mangler2 = NameMangler::new();
        mangler2.mangle_name("a_b", "test");
        let collision = mangler2.mangle_name("a/b", "test");
        assert_eq!(collision, "a_b__test_1");
    }

    #[test]
    fn test_mangle_reserved_names() {
        let mut mangler = NameMangler::new();

        // If a mangled name would be reserved, append suffix
        // We need to construct a case where mangling produces a reserved word
        // Since reserved words are like "and", "function", etc., and our format
        // is always "prefix__name", it's unlikely to conflict, but let's test the logic

        // Add a custom reserved name for testing
        mangler.reserved_names.insert("test_mod__foo".to_string());

        let mangled = mangler.mangle_name("test_mod", "foo");
        assert_eq!(mangled, "test_mod__foo_1");
    }

    #[test]
    fn test_mangle_numeric_start_path() {
        let mut mangler = NameMangler::new();
        let mangled = mangler.mangle_name("123module", "helper");
        // Should prefix with underscore since Lua identifiers can't start with digit
        assert_eq!(mangled, "_123module__helper");
    }

    #[test]
    fn test_mangle_empty_path() {
        let mut mangler = NameMangler::new();
        let mangled = mangler.mangle_name("", "helper");
        assert_eq!(mangled, "___helper");
    }

    #[test]
    fn test_preserve_entry_point_names() {
        let mangler = NameMangler::new().with_entry_module("src/main");

        // Entry module exports should be preserved
        assert!(mangler.should_preserve_name("src/main", true));

        // Entry module non-exports should not be preserved
        assert!(!mangler.should_preserve_name("src/main", false));

        // Other module exports should not be preserved
        assert!(!mangler.should_preserve_name("src/utils", true));
    }

    #[test]
    fn test_get_mangled_name() {
        let mut mangler = NameMangler::new();

        // Before mangling, returns None
        assert!(mangler.get_mangled_name("src/utils", "helper").is_none());

        // After mangling, returns the mangled name
        mangler.mangle_name("src/utils", "helper");
        assert_eq!(
            mangler.get_mangled_name("src/utils", "helper"),
            Some(&"src_utils__helper".to_string())
        );
    }

    #[test]
    fn test_mangle_all_hoistable() {
        let mut mangler = NameMangler::new();

        let mut hoistable1 = HoistableDeclarations::new();
        hoistable1.functions.insert("foo".to_string());
        hoistable1.variables.insert("BAR".to_string());

        let mut hoistable2 = HoistableDeclarations::new();
        hoistable2.functions.insert("baz".to_string());
        hoistable2.classes.insert("MyClass".to_string());

        let modules = vec![
            ("src/module_a", &hoistable1),
            ("src/module_b", &hoistable2),
        ];

        mangler.mangle_all(modules.into_iter());

        // All names should be mangled
        assert!(mangler.get_mangled_name("src/module_a", "foo").is_some());
        assert!(mangler.get_mangled_name("src/module_a", "BAR").is_some());
        assert!(mangler.get_mangled_name("src/module_b", "baz").is_some());
        assert!(mangler
            .get_mangled_name("src/module_b", "MyClass")
            .is_some());

        // Check specific mangled names
        assert_eq!(
            mangler.get_mangled_name("src/module_a", "foo"),
            Some(&"src_module_a__foo".to_string())
        );
        assert_eq!(
            mangler.get_mangled_name("src/module_b", "MyClass"),
            Some(&"src_module_b__MyClass".to_string())
        );
    }

    #[test]
    fn test_create_reverse_map() {
        let mut mangler = NameMangler::new();

        mangler.mangle_name("src/utils", "helper");
        mangler.mangle_name("src/lib", "process");

        let reverse = mangler.create_reverse_map();

        assert_eq!(
            reverse.get("src_utils__helper"),
            Some(&("src/utils".to_string(), "helper".to_string()))
        );
        assert_eq!(
            reverse.get("src_lib__process"),
            Some(&("src/lib".to_string(), "process".to_string()))
        );
    }

    #[test]
    fn test_reference_rewriter() {
        let mut mangler = NameMangler::new();
        mangler.mangle_name("src/utils", "helper");
        mangler.mangle_name("src/utils", "CONSTANT");

        let rewriter = ReferenceRewriter::new(&mangler, "src/utils");

        // Mangled names should be rewritten
        assert_eq!(rewriter.rewrite("helper"), "src_utils__helper");
        assert_eq!(rewriter.rewrite("CONSTANT"), "src_utils__CONSTANT");

        // Non-mangled names should be preserved
        assert_eq!(rewriter.rewrite("unknown"), "unknown");

        // Check is_mangled
        assert!(rewriter.is_mangled("helper"));
        assert!(!rewriter.is_mangled("unknown"));
    }

    #[test]
    fn test_reference_rewriter_different_module() {
        let mut mangler = NameMangler::new();
        mangler.mangle_name("src/utils", "helper");
        mangler.mangle_name("src/lib", "helper");

        // Rewriter for src/utils
        let rewriter_utils = ReferenceRewriter::new(&mangler, "src/utils");
        assert_eq!(rewriter_utils.rewrite("helper"), "src_utils__helper");

        // Rewriter for src/lib
        let rewriter_lib = ReferenceRewriter::new(&mangler, "src/lib");
        assert_eq!(rewriter_lib.rewrite("helper"), "src_lib__helper");
    }

    #[test]
    fn test_module_path_special_characters() {
        let mut mangler = NameMangler::new();

        // Test various path formats
        assert_eq!(
            mangler.mangle_name("a.b.c", "foo"),
            "a_b_c__foo"
        );

        let mut m2 = NameMangler::new();
        assert_eq!(
            m2.mangle_name("a/b/c", "foo"),
            "a_b_c__foo"
        );

        // Consecutive separators should not create multiple underscores
        let mut m3 = NameMangler::new();
        assert_eq!(
            m3.mangle_name("a//b", "foo"),
            "a_b__foo"
        );
    }
}
