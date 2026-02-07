use super::super::config::OptimizationLevel;
use super::CodeGenerator;
use typedlua_runtime::enum_rt;

impl CodeGenerator {
    pub fn generate_enum_declaration(
        &mut self,
        enum_decl: &typedlua_parser::ast::statement::EnumDeclaration,
    ) {
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
                        typedlua_parser::ast::statement::EnumValue::Number(n) => {
                            self.write(&n.to_string())
                        }
                        typedlua_parser::ast::statement::EnumValue::String(s) => {
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

    fn generate_rich_enum_declaration(
        &mut self,
        enum_decl: &typedlua_parser::ast::statement::EnumDeclaration,
        enum_name: &str,
    ) {
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

        for field in enum_decl.fields.iter() {
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

                for arg in member.arguments.iter() {
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

        let is_o3 = self.optimization_level.effective() >= OptimizationLevel::O3;

        // O3: Add inline hints for built-in methods
        self.writeln("");
        if is_o3 {
            self.write_indent();
            self.writeln("-- @inline");
        }
        self.writeln(&enum_rt::ENUM_ORDINAL.replace("{}", enum_name));
        self.writeln("");
        if is_o3 {
            self.write_indent();
            self.writeln("-- @inline");
        }
        self.writeln(&enum_rt::ENUM_NAME.replace("{}", enum_name));
        self.writeln("");
        if is_o3 {
            self.write_indent();
            self.writeln("-- @inline");
        }
        self.writeln(
            &enum_rt::ENUM_VALUES
                .replace("{}", enum_name)
                .replace("{}", enum_name),
        );
        self.writeln("");
        if is_o3 {
            self.write_indent();
            self.writeln("-- @inline");
        }
        self.writeln(
            &enum_rt::ENUM_VALUE_OF
                .replace("{}", enum_name)
                .replace("{}", enum_name),
        );

        for method in enum_decl.methods.iter() {
            self.writeln("");

            // O3: Add inline hints for simple methods
            if is_o3 && Self::is_simple_method_for_inline(method) {
                self.write_indent();
                self.writeln("-- @inline");
            }

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
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");
            self.indent();
            self.generate_block(&method.body);
            self.dedent();
            self.write_indent();
            self.writeln("end");
        }
    }

    /// Check if a method is simple enough to be a candidate for inlining at O3
    /// Simple methods: no parameters, single return statement with simple expression
    fn is_simple_method_for_inline(method: &typedlua_parser::ast::statement::EnumMethod) -> bool {
        use typedlua_parser::ast::statement::{Block, Statement};

        // Must have no parameters
        if !method.parameters.is_empty() {
            return false;
        }

        // Check if body is a simple block with just a return statement
        let Block { statements, .. } = &method.body;

        if statements.len() != 1 {
            return false;
        }

        matches!(&statements[0], Statement::Return(_))
    }
}
