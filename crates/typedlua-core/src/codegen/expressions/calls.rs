use super::super::CodeGenerator;
use typedlua_parser::ast::expression::ExpressionKind;

impl CodeGenerator {
    pub fn generate_call_expression(
        &mut self,
        callee: &typedlua_parser::ast::expression::Expression,
        args: &[typedlua_parser::ast::expression::Argument],
    ) {
        if matches!(&callee.kind, ExpressionKind::SuperKeyword) {
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

        let is_super_method_call = matches!(
            &callee.kind,
            ExpressionKind::Member(obj, _) if matches!(obj.kind, ExpressionKind::SuperKeyword)
        );

        self.generate_expression(callee);
        self.write("(");

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

    pub fn generate_method_call_expression(
        &mut self,
        obj: &typedlua_parser::ast::expression::Expression,
        method: &typedlua_parser::ast::Spanned<typedlua_parser::string_interner::StringId>,
        args: &[typedlua_parser::ast::expression::Argument],
    ) {
        let method_str = self.resolve(method.node);
        self.generate_expression(obj);
        self.write(":");
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

    pub fn generate_new_expression(
        &mut self,
        callee: &typedlua_parser::ast::expression::Expression,
        args: &[typedlua_parser::ast::expression::Argument],
    ) {
        self.write("(");
        self.generate_expression(callee);
        self.write(".new(");
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.generate_argument(arg);
        }
        self.write("))");
    }
}
