use super::CodeGenerator;
use typedlua_parser as parser;
use typedlua_parser::ast::statement::*;

impl CodeGenerator {
    pub fn generate_class_declaration(&mut self, class_decl: &ClassDeclaration) {
        let class_name = self.resolve(class_decl.name.node).to_string();

        let prev_parent = self.current_class_parent.take();

        let base_class_name = if let Some(extends) = &class_decl.extends {
            if let typedlua_parser::ast::types::TypeKind::Reference(type_ref) = &extends.kind {
                Some(type_ref.name.node)
            } else {
                None
            }
        } else {
            None
        };

        self.current_class_parent = base_class_name;

        self.write_indent();
        self.write("local ");
        self.write(&class_name);
        self.writeln(" = {}");

        self.write_indent();
        self.write(&class_name);
        self.write(".__index = ");
        self.write(&class_name);
        self.writeln("");

        if let Some(base_name) = &base_class_name {
            self.writeln("");
            self.write_indent();
            self.write("setmetatable(");
            self.write(&class_name);
            self.write(", { __index = ");
            let base_name_str = self.resolve(*base_name);
            self.write(&base_name_str);
            self.writeln(" })");
        }

        let has_constructor = class_decl
            .members
            .iter()
            .any(|m| matches!(m, ClassMember::Constructor(_)));

        let has_primary_constructor = class_decl.primary_constructor.is_some();

        if has_primary_constructor {
            self.generate_primary_constructor(class_decl, &class_name, base_class_name);
        } else if has_constructor {
            for member in class_decl.members.iter() {
                if let ClassMember::Constructor(ctor) = member {
                    self.generate_class_constructor(&class_name, ctor, class_decl.is_abstract);
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

            // Check for abstract class instantiation
            if class_decl.is_abstract {
                self.write_indent();
                self.write("if self == nil or self.__typeName == \"");
                self.write(&class_name);
                self.writeln("\" then");
                self.indent();
                self.write_indent();
                self.write("error(\"Cannot instantiate abstract class '");
                self.write(&class_name);
                self.writeln("'\")");
                self.dedent();
                self.write_indent();
                self.writeln("end");
            }

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

        for member in class_decl.members.iter() {
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
                ClassMember::Property(_) | ClassMember::Constructor(_) => {}
            }
        }

        let mut has_operators = false;

        for member in class_decl.members.iter() {
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
            for member in class_decl.members.iter() {
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

        if !class_decl.decorators.is_empty() {
            self.writeln("");
            for decorator in class_decl.decorators.iter() {
                self.write_indent();
                self.write(&class_name);
                self.write(" = ");
                self.generate_decorator_call(decorator, &class_name);
                self.writeln("");
            }
        }

        self.writeln("");
        for impl_type in class_decl.implements.iter() {
            if let typedlua_parser::ast::types::TypeKind::Reference(type_ref) = &impl_type.kind {
                let interface_name = self.resolve(type_ref.name.node).to_string();

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

        // -- Class infrastructure (always emitted) --
        let type_id = self.next_type_id;
        self.next_type_id += 1;

        self.write_indent();
        self.write(&class_name);
        self.write(".__typeName = \"");
        self.write(&class_name);
        self.writeln("\"");

        self.write_indent();
        self.write(&class_name);
        self.writeln(&format!(".__typeId = {}", type_id));

        // Mark class as final if needed
        if class_decl.is_final {
            self.write_indent();
            self.write(&class_name);
            self.writeln(".__final = true");
        }

        // Track final methods
        let final_methods: Vec<String> = class_decl
            .members
            .iter()
            .filter_map(|member| {
                if let ClassMember::Method(method) = member {
                    if method.is_final {
                        Some(self.resolve(method.name.node).to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        if !final_methods.is_empty() {
            self.write_indent();
            self.write(&class_name);
            self.writeln(".__finalMethods = {");
            self.indent();
            for method_name in &final_methods {
                self.write_indent();
                self.writeln(&format!("\"{}\",", method_name));
            }
            self.dedent();
            self.write_indent();
            self.writeln("}");
        }

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

        if let Some(base_name) = &base_class_name {
            let base_name_str = self.resolve(*base_name);
            self.write_indent();
            self.write(&class_name);
            self.writeln(&format!(".__parent = {}", base_name_str));

            // Check if parent class is final
            self.writeln("");
            self.write_indent();
            self.writeln(&format!("if {}.__final then", base_name_str));
            self.indent();
            self.write_indent();
            self.writeln(&format!(
                "error(\"Cannot extend final class '{}'\")",
                base_name_str
            ));
            self.dedent();
            self.write_indent();
            self.writeln("end");

            // Check for final method overrides
            self.writeln("");
            self.write_indent();
            self.writeln(&format!("if {}.__finalMethods then", base_name_str));
            self.indent();
            self.write_indent();
            self.writeln(&format!(
                "for _, methodName in ipairs({}.__finalMethods) do",
                base_name_str
            ));
            self.indent();
            self.write_indent();
            self.writeln(&format!(
                "if {}[methodName] and {}[methodName] ~= {}[methodName] then",
                class_name, class_name, base_name_str
            ));
            self.indent();
            self.write_indent();
            self.writeln("error(\"Cannot override final method '\" .. methodName .. \"'\")");
            self.dedent();
            self.write_indent();
            self.writeln("end");
            self.dedent();
            self.write_indent();
            self.writeln("end");
            self.dedent();
            self.write_indent();
            self.writeln("end");
        }

        // -- Reflection metadata (gated on reflection mode) --
        if self.should_emit_reflection() {
            self.registered_types.insert(class_name.clone(), type_id);

            // __ownFields with v2 bit flags
            self.write_indent();
            self.write(&class_name);
            self.writeln(".__ownFields = {");
            self.indent();
            for member in class_decl.members.iter() {
                if let ClassMember::Property(prop) = member {
                    let prop_name = self.resolve(prop.name.node).to_string();
                    let flags = Self::encode_field_flags(prop);
                    let type_code = self.encode_type_code(&prop.type_annotation);
                    self.write_indent();
                    self.writeln(&format!(
                        "{{ name = \"{}\", type = \"{}\", _flags = {} }},",
                        prop_name, type_code, flags
                    ));
                }
            }
            self.dedent();
            self.write_indent();
            self.writeln("}");

            // __ownMethods with compact signatures
            self.write_indent();
            self.write(&class_name);
            self.writeln(".__ownMethods = {");
            self.indent();
            for member in class_decl.members.iter() {
                if let ClassMember::Method(method) = member {
                    let method_name = self.resolve(method.name.node).to_string();
                    let params: String = method
                        .parameters
                        .iter()
                        .map(|p| {
                            p.type_annotation
                                .as_ref()
                                .map(|ty| self.encode_type_code(ty))
                                .unwrap_or_else(|| "o".to_string())
                        })
                        .collect();
                    let ret = method
                        .return_type
                        .as_ref()
                        .map(|ty| self.encode_type_code(ty))
                        .unwrap_or_else(|| "v".to_string());
                    self.write_indent();
                    self.writeln(&format!(
                        "{{ name = \"{}\", params = \"{}\", ret = \"{}\" }},",
                        method_name, params, ret
                    ));
                }
            }
            self.dedent();
            self.write_indent();
            self.writeln("}");

            self.writeln("");
            self.writeln(
                &typedlua_runtime::class::BUILD_ALL_FIELDS
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
                &typedlua_runtime::class::BUILD_ALL_METHODS
                    .replace("{}", &class_name)
                    .replace("{}", &class_name)
                    .replace("{}", &class_name)
                    .replace("{}", &class_name)
                    .replace("{}", &class_name)
                    .replace("{}", &class_name)
                    .replace("{}", &class_name),
            );
        }

        self.writeln("");

        self.current_class_parent = prev_parent;
    }

    /// Encode field access modifiers as bit flags per v2 reflection spec.
    /// Bit 0=Public(1), Bit 1=Private(2), Bit 2=Protected(4), Bit 3=Readonly(8), Bit 4=Static(16)
    fn encode_field_flags(prop: &PropertyDeclaration) -> u8 {
        let mut flags: u8 = match prop.access {
            Some(AccessModifier::Public) | None => 1,
            Some(AccessModifier::Private) => 2,
            Some(AccessModifier::Protected) => 4,
        };
        if prop.is_readonly {
            flags |= 8;
        }
        if prop.is_static {
            flags |= 16;
        }
        flags
    }

    /// Encode a type annotation as a compact type code string per v2 reflection spec.
    /// n=number, s=string, b=boolean, t=table, f=function, v=void, o=any/unknown
    fn encode_type_code(&self, ty: &parser::ast::types::Type) -> String {
        use parser::ast::types::{PrimitiveType, TypeKind};
        match &ty.kind {
            TypeKind::Primitive(p) => match p {
                PrimitiveType::Number | PrimitiveType::Integer => "n".to_string(),
                PrimitiveType::String => "s".to_string(),
                PrimitiveType::Boolean => "b".to_string(),
                PrimitiveType::Table => "t".to_string(),
                PrimitiveType::Void | PrimitiveType::Nil => "v".to_string(),
                _ => "o".to_string(),
            },
            TypeKind::Function(_) => "f".to_string(),
            TypeKind::Array(inner) => format!("[{}]", self.encode_type_code(inner)),
            TypeKind::Nullable(inner) => format!("?{}", self.encode_type_code(inner)),
            TypeKind::Union(types) => types
                .iter()
                .map(|t| self.encode_type_code(t))
                .collect::<Vec<_>>()
                .join("|"),
            TypeKind::Reference(type_ref) => self.resolve(type_ref.name.node),
            _ => "o".to_string(),
        }
    }

    pub fn generate_interface_declaration(&mut self, iface_decl: &InterfaceDeclaration) {
        let interface_name = self.resolve(iface_decl.name.node).to_string();

        for member in iface_decl.members.iter() {
            if let InterfaceMember::Method(method) = member {
                if let Some(body) = &method.body {
                    let method_name = self.resolve(method.name.node).to_string();
                    let default_fn_name = format!("{}__{}", interface_name, method_name);

                    self.writeln("");
                    self.write_indent();
                    self.write("function ");
                    self.write(&default_fn_name);
                    self.write("(self");

                    for param in method.parameters.iter() {
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

    pub fn generate_class_constructor(
        &mut self,
        class_name: &str,
        ctor: &ConstructorDeclaration,
        is_abstract: bool,
    ) {
        let always_use_init = true;

        if always_use_init {
            self.writeln("");
            self.write_indent();
            self.write("function ");
            self.write(class_name);
            self.write("._init(self");

            for param in ctor.parameters.iter() {
                self.write(", ");
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            self.indent();

            self.generate_block(&ctor.body);

            self.dedent();
            self.write_indent();
            self.writeln("end");

            self.writeln("");
            self.write_indent();
            self.write("function ");
            self.write(class_name);
            self.write(".new(");

            for (i, param) in ctor.parameters.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            self.indent();

            self.write_indent();
            self.write("local self = setmetatable({}, ");
            self.write(class_name);
            self.writeln(")");

            // Check for abstract class instantiation
            if is_abstract {
                self.write_indent();
                self.write("if self.__typeName == \"");
                self.write(class_name);
                self.writeln("\" then");
                self.indent();
                self.write_indent();
                self.write("error(\"Cannot instantiate abstract class '");
                self.write(class_name);
                self.writeln("'\")");
                self.dedent();
                self.write_indent();
                self.writeln("end");
            }

            self.write_indent();
            self.write(class_name);
            self.write("._init(self");
            for param in ctor.parameters.iter() {
                self.write(", ");
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            self.write_indent();
            self.writeln("return self");

            self.dedent();
            self.write_indent();
            self.writeln("end");
        } else {
            self.writeln("");
            self.write_indent();
            self.write("function ");
            self.write(class_name);
            self.write(".new(");

            for (i, param) in ctor.parameters.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            self.indent();

            self.write_indent();
            self.write("local self = setmetatable({}, ");
            self.write(class_name);
            self.writeln(")");

            // Check for abstract class instantiation
            if is_abstract {
                self.write_indent();
                self.write("if self.__typeName == \"");
                self.write(class_name);
                self.writeln("\" then");
                self.indent();
                self.write_indent();
                self.write("error(\"Cannot instantiate abstract class '");
                self.write(class_name);
                self.writeln("'\")");
                self.dedent();
                self.write_indent();
                self.writeln("end");
            }

            self.generate_block(&ctor.body);

            self.write_indent();
            self.writeln("return self");

            self.dedent();
            self.write_indent();
            self.writeln("end");
        }
    }

    pub fn generate_primary_constructor(
        &mut self,
        class_decl: &ClassDeclaration,
        class_name: &str,
        base_class_name: Option<parser::string_interner::StringId>,
    ) {
        let primary_params = class_decl.primary_constructor.unwrap();

        self.writeln("");
        self.write_indent();
        self.write("function ");
        self.write(class_name);
        self.write("._init(self");

        for param in primary_params {
            self.write(", ");
            let param_name = self.resolve(param.name.node);
            self.write(&param_name);
        }
        self.writeln(")");

        self.indent();

        if let Some(parent_args) = class_decl.parent_constructor_args {
            if let Some(parent_name) = base_class_name {
                self.write_indent();
                let parent_name_str = self.resolve(parent_name);
                self.write(&parent_name_str);
                self.write("._init(self");
                for arg in parent_args.iter() {
                    self.write(", ");
                    self.generate_expression(arg);
                }
                self.writeln(")");
            }
        }

        for param in primary_params {
            self.write_indent();

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

        self.writeln("");
        self.write_indent();
        self.write("function ");
        self.write(class_name);
        self.write(".new(");

        for (i, param) in primary_params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            let param_name = self.resolve(param.name.node);
            self.write(&param_name);
        }
        self.writeln(")");

        self.indent();

        self.write_indent();
        self.write("local self = setmetatable({}, ");
        self.write(class_name);
        self.writeln(")");

        // Check for abstract class instantiation
        if class_decl.is_abstract {
            self.write_indent();
            self.write("if self.__typeName == \"");
            self.write(class_name);
            self.writeln("\" then");
            self.indent();
            self.write_indent();
            self.write("error(\"Cannot instantiate abstract class '");
            self.write(class_name);
            self.writeln("'\")");
            self.dedent();
            self.write_indent();
            self.writeln("end");
        }

        self.write_indent();
        self.write(class_name);
        self.write("._init(self");
        for param in primary_params {
            self.write(", ");
            let param_name = self.resolve(param.name.node);
            self.write(&param_name);
        }
        self.writeln(")");

        self.write_indent();
        self.writeln("return self");

        self.dedent();
        self.write_indent();
        self.writeln("end");
    }

    pub fn generate_class_method(&mut self, class_name: &str, method: &MethodDeclaration) {
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

        for (i, param) in method.parameters.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.generate_pattern(&param.pattern);
        }
        self.writeln(")");

        if let Some(body) = &method.body {
            self.indent();
            self.generate_block(body);
            self.dedent();
        }

        self.write_indent();
        self.writeln("end");

        if !method.decorators.is_empty() {
            for decorator in method.decorators.iter() {
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

    pub fn generate_class_getter(&mut self, class_name: &str, getter: &GetterDeclaration) {
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

    pub fn generate_class_setter(&mut self, class_name: &str, setter: &SetterDeclaration) {
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

    pub fn generate_operator_declaration(&mut self, class_name: &str, op: &OperatorDeclaration) {
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

        for decorator in op.decorators.iter() {
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

    pub fn operator_kind_name(&self, op: &typedlua_parser::ast::statement::OperatorKind) -> String {
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
}
