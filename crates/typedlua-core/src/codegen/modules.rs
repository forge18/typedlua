use super::{CodeGenMode, CodeGenerator};

impl CodeGenerator {
    pub fn generate_import(&mut self, import: &typedlua_parser::ast::statement::ImportDeclaration) {
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
        }
    }

    pub fn generate_export(&mut self, export: &typedlua_parser::ast::statement::ExportDeclaration) {
        match &export.kind {
            typedlua_parser::ast::statement::ExportKind::Declaration(stmt) => {
                self.generate_statement(stmt);

                if let Some(name) = self.get_declaration_name(stmt) {
                    self.exports.push(self.resolve(name).to_string());
                }
            }
            typedlua_parser::ast::statement::ExportKind::Named { specifiers, source } => {
                if let Some(source_path) = source {
                    self.generate_re_export(specifiers, source_path);
                } else {
                    for spec in specifiers {
                        self.exports.push(self.resolve(spec.local.node).to_string());
                    }
                }
            }
            typedlua_parser::ast::statement::ExportKind::Default(expr) => {
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
}
