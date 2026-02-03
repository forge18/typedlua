use super::super::CodeGenerator;
use typedlua_parser::ast::expression::{ArrayElement, Literal, ObjectProperty};

impl CodeGenerator {
    pub fn generate_literal(&mut self, lit: &Literal) {
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

    pub fn generate_argument(&mut self, arg: &typedlua_parser::ast::expression::Argument) {
        self.generate_expression(&arg.value);
    }

    pub fn generate_object_property(
        &mut self,
        prop: &typedlua_parser::ast::expression::ObjectProperty,
    ) {
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
                self.generate_expression(value);
            }
        }
    }

    pub fn generate_identifier(&mut self, name: typedlua_parser::string_interner::StringId) {
        let name_str = self.resolve(name);
        self.write(&name_str);
    }

    pub fn generate_array_element(&mut self, elem: &ArrayElement) {
        match elem {
            ArrayElement::Expression(expr) => self.generate_expression(expr),
            ArrayElement::Spread(expr) => self.generate_expression(expr),
        }
    }
}
