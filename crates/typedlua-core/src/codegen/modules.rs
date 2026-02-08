use super::{CodeGenMode, CodeGenerator};

impl CodeGenerator {
    pub fn generate_import(&mut self, import: &typedlua_parser::ast::statement::ImportDeclaration) {
        // Detect @std/reflection import - set flag and skip code generation
        if import.source == "@std/reflection" {
            self.has_reflection_import = true;
            return;
        }

        let (require_fn, module_path) = match &self.mode {
            CodeGenMode::Bundle { .. } => {
                let resolved_id = self
                    .import_map
                    .get(&import.source)
                    .cloned()
                    .unwrap_or_else(|| import.source.clone());
                ("__require", resolved_id)
            }
            CodeGenMode::Require => ("require", import.source.clone()),
        };

        match &import.clause {
            typedlua_parser::ast::statement::ImportClause::TypeOnly(_) => {}
            typedlua_parser::ast::statement::ImportClause::Named(specs) => {
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
            typedlua_parser::ast::statement::ImportClause::Default(ident) => {
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
            typedlua_parser::ast::statement::ImportClause::Namespace(ident) => {
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
            typedlua_parser::ast::statement::ImportClause::Mixed { default, named } => {
                // Load module once
                self.write_indent();
                self.write("local _mod = ");
                self.write(require_fn);
                self.write("(\"");
                self.write(&module_path);
                self.writeln("\")");

                // Assign default export to the default identifier
                self.write_indent();
                self.write("local ");
                let default_str = self.resolve(default.node);
                self.write(&default_str);
                self.writeln(" = _mod");

                // Extract named imports
                if !named.is_empty() {
                    self.write_indent();
                    self.write("local ");
                    for (i, spec) in named.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        let local_name = spec.local.as_ref().unwrap_or(&spec.imported);
                        let name_str = self.resolve(local_name.node);
                        self.write(&name_str);
                    }
                    self.write(" = ");
                    for (i, spec) in named.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.write("_mod.");
                        let imported_str = self.resolve(spec.imported.node);
                        self.write(&imported_str);
                    }
                    self.writeln("");
                }
            }
        }
    }

    pub fn generate_export(&mut self, export: &typedlua_parser::ast::statement::ExportDeclaration) {
        match &export.kind {
            typedlua_parser::ast::statement::ExportKind::Declaration(stmt) => {
                if let Some(name) = self.get_declaration_name(stmt) {
                    let export_name = self.resolve(name).to_string();

                    // Tree shaking: skip unreachable exports
                    if self.tree_shaking_enabled && !self.is_export_reachable(&export_name) {
                        return;
                    }

                    self.generate_statement(stmt);
                    self.exports.push(export_name);
                } else {
                    self.generate_statement(stmt);
                }
            }
            typedlua_parser::ast::statement::ExportKind::Named { specifiers, source } => {
                if let Some(source_path) = source {
                    self.generate_re_export(specifiers, source_path);
                } else {
                    for spec in specifiers.iter() {
                        let export_name = self.resolve(spec.local.node).to_string();

                        // Tree shaking: skip unreachable exports
                        if self.tree_shaking_enabled && !self.is_export_reachable(&export_name) {
                            continue;
                        }

                        self.exports.push(export_name);
                    }
                }
            }
            typedlua_parser::ast::statement::ExportKind::Default(expr) => {
                // Tree shaking: in bundle mode, skip default export if tree shaking is enabled
                // and the module has no other reachable exports
                if self.tree_shaking_enabled {
                    let has_other_exports = !self.exports.is_empty();
                    if !has_other_exports && !matches!(self.mode, CodeGenMode::Bundle { .. }) {
                        return;
                    }
                }

                self.write_indent();
                self.write("local _default = ");
                self.generate_expression(expr);
                self.writeln("");
                self.has_default_export = true;
            }
        }
    }

    pub fn generate_re_export(
        &mut self,
        specifiers: &[typedlua_parser::ast::statement::ExportSpecifier],
        source: &str,
    ) {
        let (require_fn, module_path) = match &self.mode {
            CodeGenMode::Bundle { .. } => {
                let resolved_id = self
                    .import_map
                    .get(source)
                    .cloned()
                    .unwrap_or_else(|| source.to_string());
                ("__require", resolved_id)
            }
            CodeGenMode::Require => ("require", source.to_string()),
        };

        self.write_indent();
        self.write("local _mod = ");
        self.write(require_fn);
        self.write("(\"");
        self.write(&module_path);
        self.writeln("\")");

        self.write_indent();
        self.write("local ");
        for (i, spec) in specifiers.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write(&self.resolve(spec.local.node));
        }
        self.write(" = ");
        for (i, spec) in specifiers.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write("_mod.");
            // For re-exports, exported is None and local is the name to use
            let export_name = spec
                .exported
                .clone()
                .map(|e| e.node)
                .unwrap_or(spec.local.node);
            self.write(&self.resolve(export_name));
        }
        self.writeln("");
    }

    pub fn generate_namespace_declaration(
        &mut self,
        ns: &typedlua_parser::ast::statement::NamespaceDeclaration,
    ) {
        let path: Vec<String> = ns
            .path
            .iter()
            .map(|ident| self.resolve(ident.node))
            .collect();

        if path.is_empty() {
            return;
        }

        self.current_namespace = Some(path.clone());

        // Generate namespace table hierarchy
        self.writeln("");
        self.write_indent();
        self.writeln(&format!("-- Namespace: {}", path.join(".")));

        // Create root namespace table (or use existing)
        self.write_indent();
        self.writeln(&format!("local {} = {} or {{}}", path[0], path[0]));

        // Create nested namespace tables
        for i in 1..path.len() {
            let current_path = path[..=i].join(".");
            self.write_indent();
            self.writeln(&format!("{} = {} or {{}}", current_path, current_path));
        }

        self.writeln("");
    }
}
