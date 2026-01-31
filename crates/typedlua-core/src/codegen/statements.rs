use super::super::config::OptimizationLevel;
use super::CodeGenerator;
use typedlua_parser::ast::pattern::{ArrayPattern, ArrayPatternElement, ObjectPattern, Pattern};
use typedlua_parser::ast::statement::*;
use typedlua_parser::prelude::Block;

impl CodeGenerator {
    pub fn generate_statement(&mut self, stmt: &Statement) {
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
            Statement::TypeAlias(_) => {}
            Statement::Enum(decl) => self.generate_enum_declaration(decl),
            Statement::Class(class_decl) => self.generate_class_declaration(class_decl),
            Statement::Import(import) => self.generate_import(import),
            Statement::Export(export) => self.generate_export(export),
            Statement::DeclareFunction(_)
            | Statement::DeclareNamespace(_)
            | Statement::DeclareType(_)
            | Statement::DeclareInterface(_)
            | Statement::DeclareConst(_) => {}
            Statement::Throw(throw_stmt) => self.generate_throw_statement(throw_stmt),
            Statement::Try(try_stmt) => self.generate_try_statement(try_stmt),
            Statement::Rethrow(span) => self.generate_rethrow_statement(*span),
            Statement::Namespace(ns) => self.generate_namespace_declaration(ns),
        }
    }

    pub fn generate_variable_declaration(&mut self, decl: &VariableDeclaration) {
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
            Pattern::Or(_) => {
                // Or-patterns should not appear in variable declarations
                // They are only valid in match expressions
                // Treat as wildcard (defensive programming)
                self.write_indent();
                self.write("local _ = ");
                self.generate_expression(&decl.initializer);
                self.writeln("");
            }
        }
    }

    /// Generate array destructuring assignments
    pub fn generate_array_destructuring(&mut self, pattern: &ArrayPattern, source: &str) {
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
                        Pattern::Or(_) => {
                            // Or-patterns should not appear in destructuring
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
    pub fn generate_object_destructuring(&mut self, pattern: &ObjectPattern, source: &str) {
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
                    Pattern::Or(_) => {
                        // Or-patterns should not appear in destructuring
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

    pub fn generate_function_declaration(&mut self, decl: &FunctionDeclaration) {
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

    pub fn generate_if_statement(&mut self, if_stmt: &IfStatement) {
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

    pub fn generate_while_statement(&mut self, while_stmt: &WhileStatement) {
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

    pub fn generate_for_statement(&mut self, for_stmt: &ForStatement) {
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

    pub fn generate_repeat_statement(&mut self, repeat_stmt: &RepeatStatement) {
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

    pub fn generate_return_statement(&mut self, return_stmt: &ReturnStatement) {
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

    pub fn generate_block(&mut self, block: &Block) {
        for statement in &block.statements {
            self.generate_statement(statement);
        }
    }

    pub fn generate_throw_statement(
        &mut self,
        stmt: &typedlua_parser::ast::statement::ThrowStatement,
    ) {
        self.write_indent();
        self.write("error(");
        self.generate_expression(&stmt.expression);
        self.writeln(")");
    }

    pub fn generate_rethrow_statement(&mut self, _span: typedlua_parser::span::Span) {
        self.write_indent();
        self.writeln("error(__error)");
    }

    pub fn generate_try_statement(&mut self, stmt: &typedlua_parser::ast::statement::TryStatement) {
        self.write_indent();
        self.writeln("-- try block");

        let has_typed_catches = stmt.catch_clauses.iter().any(|clause| {
            matches!(
                clause.pattern,
                typedlua_parser::ast::statement::CatchPattern::Typed { .. }
                    | typedlua_parser::ast::statement::CatchPattern::MultiTyped { .. }
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

    pub fn generate_try_pcall(&mut self, stmt: &typedlua_parser::ast::statement::TryStatement) {
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

    pub fn generate_try_xpcall(&mut self, stmt: &typedlua_parser::ast::statement::TryStatement) {
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
                typedlua_parser::ast::statement::CatchPattern::Typed { .. }
                    | typedlua_parser::ast::statement::CatchPattern::MultiTyped { .. }
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

    pub fn generate_catch_clause_pcall(
        &mut self,
        clause: &typedlua_parser::ast::statement::CatchClause,
        is_last: bool,
    ) {
        let var_name = match &clause.pattern {
            typedlua_parser::ast::statement::CatchPattern::Untyped { variable, .. }
            | typedlua_parser::ast::statement::CatchPattern::Typed { variable, .. }
            | typedlua_parser::ast::statement::CatchPattern::MultiTyped { variable, .. } => {
                self.resolve(variable.node)
            }
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

    pub fn generate_catch_clause_xpcall(
        &mut self,
        clause: &typedlua_parser::ast::statement::CatchClause,
    ) {
        let var_name = match &clause.pattern {
            typedlua_parser::ast::statement::CatchPattern::Untyped { variable, .. }
            | typedlua_parser::ast::statement::CatchPattern::Typed { variable, .. }
            | typedlua_parser::ast::statement::CatchPattern::MultiTyped { variable, .. } => {
                self.resolve(variable.node)
            }
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

    pub fn generate_finally_block(&mut self, block: &Block) {
        self.write_indent();
        self.writeln("-- finally block");
        self.generate_block(block);
    }
}
