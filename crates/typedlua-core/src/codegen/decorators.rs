use super::CodeGenerator;
use typedlua_parser::ast::statement::*;

impl CodeGenerator {
    pub fn generate_decorator_call(&mut self, decorator: &Decorator, target: &str) {
        use typedlua_parser::ast::statement::DecoratorExpression;

        match &decorator.expression {
            DecoratorExpression::Identifier(name) => {
                let decorator_name = self.resolve(name.node);
                self.write(&decorator_name);
                self.write("(");
                self.write(target);
                self.write(")");
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
                self.write(")(");
                self.write(target);
                self.write(")");
            }
            DecoratorExpression::Member {
                object, property, ..
            } => {
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

    pub fn generate_decorator_expression(
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

    pub fn is_built_in_decorator(&self, name: &str) -> bool {
        matches!(name, "readonly" | "sealed" | "deprecated")
    }

    pub fn detect_decorators(&mut self, program: &typedlua_parser::ast::Program) {
        self.detect_decorators_from_statements(program.statements);
    }

    pub fn detect_decorators_from_statements(&mut self, statements: &[typedlua_parser::ast::statement::Statement]) {
        for statement in statements {
            if self.statement_uses_built_in_decorators(statement) {
                self.uses_built_in_decorators = true;
                return;
            }
        }
    }

    pub fn statement_uses_built_in_decorators(&self, stmt: &Statement) -> bool {
        match stmt {
            Statement::Class(class_decl) => {
                for decorator in class_decl.decorators.iter() {
                    if self.is_decorator_built_in(&decorator.expression) {
                        return true;
                    }
                }

                for member in class_decl.members.iter() {
                    let decorators: &[Decorator] = match member {
                        ClassMember::Method(method) => method.decorators,
                        ClassMember::Property(prop) => prop.decorators,
                        ClassMember::Getter(getter) => getter.decorators,
                        ClassMember::Setter(setter) => setter.decorators,
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

    pub fn is_decorator_built_in(
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
                if let DecoratorExpression::Identifier(name) = callee {
                    let name_str = self.resolve(name.node);
                    self.is_built_in_decorator(&name_str)
                } else {
                    false
                }
            }
            DecoratorExpression::Member {
                object, property, ..
            } => {
                if let DecoratorExpression::Identifier(obj_name) = object {
                    let obj_str = self.resolve(obj_name.node);
                    let prop_str = self.resolve(property.node);
                    obj_str == "TypedLua" && self.is_built_in_decorator(&prop_str)
                } else {
                    false
                }
            }
        }
    }

    pub fn embed_runtime_library(&mut self) {
        self.writeln(typedlua_runtime::decorator::DECORATOR_RUNTIME);
        self.writeln("");
    }
}
