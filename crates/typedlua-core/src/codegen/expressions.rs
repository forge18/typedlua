use super::dedent;
use super::CodeGenerator;
use crate::config::OptimizationLevel;
use typedlua_parser::ast::expression::*;
use typedlua_parser::ast::pattern::{ArrayPatternElement, Pattern};
use typedlua_parser::prelude::{MatchArmBody, MatchExpression};

/// Check if an expression is guaranteed to never be nil
/// Used for O2 null coalescing optimization to skip unnecessary nil checks
pub fn is_guaranteed_non_nil(expr: &Expression) -> bool {
    match &expr.kind {
        ExpressionKind::Literal(Literal::Boolean(_)) => true,
        ExpressionKind::Literal(Literal::Number(_)) => true,
        ExpressionKind::Literal(Literal::Integer(_)) => true,
        ExpressionKind::Literal(Literal::String(_)) => true,
        ExpressionKind::Object(_) => true,
        ExpressionKind::Array(_) => true,
        ExpressionKind::New(_, _) => true,
        ExpressionKind::Function(_) => true,
        ExpressionKind::Parenthesized(inner) => is_guaranteed_non_nil(inner),
        _ => false,
    }
}

/// Check if an expression is "simple" and can be safely evaluated twice
/// Simple expressions: identifiers, literals, and simple member/index access
pub fn is_simple_expression(expr: &Expression) -> bool {
    match &expr.kind {
        ExpressionKind::Identifier(_) => true,
        ExpressionKind::Literal(_) => true,
        ExpressionKind::Member(obj, _) => is_simple_expression(obj),
        ExpressionKind::Index(obj, index) => {
            is_simple_expression(obj) && is_simple_expression(index)
        }
        _ => false,
    }
}

/// Convert binary op to string
pub fn simple_binary_op_to_string(op: BinaryOp) -> &'static str {
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

/// Convert unary op to string
pub fn unary_op_to_string(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Negate => "-",
        UnaryOp::Not => "not ",
        UnaryOp::Length => "#",
        UnaryOp::BitwiseNot => "~",
    }
}

impl CodeGenerator {
    pub fn is_guaranteed_non_nil(&self, expr: &Expression) -> bool {
        is_guaranteed_non_nil(expr)
    }

    pub fn is_simple_expression(&self, expr: &Expression) -> bool {
        is_simple_expression(expr)
    }

    pub fn simple_binary_op_to_string(&self, op: BinaryOp) -> &'static str {
        simple_binary_op_to_string(op)
    }

    pub fn unary_op_to_string(&self, op: UnaryOp) -> &'static str {
        unary_op_to_string(op)
    }

    /// Generate expression to Lua code (main dispatcher)
    pub fn generate_expression(&mut self, expr: &Expression) {
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
                if *op == UnaryOp::BitwiseNot && !self.strategy.supports_native_bitwise() {
                    let operand_str = self.expression_to_string(operand);
                    let result = self.strategy.generate_unary_bitwise_not(&operand_str);
                    self.write(&result);
                } else {
                    let op_str = unary_op_to_string(*op).to_string();
                    self.write(&op_str);
                    self.generate_expression(operand);
                }
            }
            ExpressionKind::Assignment(target, op, value) => match op {
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
            },
            ExpressionKind::Call(callee, args, _) => {
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

                let is_super_method_call = matches!(&callee.kind,
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
            ExpressionKind::New(constructor, args) => {
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
                if matches!(object.kind, ExpressionKind::SuperKeyword) {
                    if let Some(parent) = self.current_class_parent {
                        let parent_str = self.resolve(parent);
                        self.write(&parent_str);
                        self.write(".");
                        let member_str = self.resolve(member.node);
                        self.write(&member_str);
                    } else {
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
                let has_spread = elements
                    .iter()
                    .any(|elem| matches!(elem, ArrayElement::Spread(_)));

                if !has_spread {
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
                let has_spread = props
                    .iter()
                    .any(|prop| matches!(prop, ObjectProperty::Spread { .. }));

                if !has_spread {
                    self.write("{");
                    for (i, prop) in props.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.generate_object_property(prop);
                    }
                    self.write("}");
                } else {
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
            ExpressionKind::Pipe(left, right) => match &right.kind {
                ExpressionKind::Call(callee, arguments, _) => {
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
                    self.generate_expression(right);
                    self.write("(");
                    self.generate_expression(left);
                    self.write(")");
                }
            },
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
                if let Some(parent) = self.current_class_parent {
                    let parent_str = self.resolve(parent);
                    self.write(&parent_str);
                } else {
                    self.write("nil --[[super without parent class]]");
                }
            }
            ExpressionKind::Template(template_lit) => {
                self.write("(");
                let mut first = true;

                let mut string_parts: Vec<String> = Vec::new();
                let mut expression_parts: Vec<&Expression> = Vec::new();

                for part in &template_lit.parts {
                    match part {
                        typedlua_parser::ast::expression::TemplatePart::String(s) => {
                            string_parts.push(s.clone());
                        }
                        typedlua_parser::ast::expression::TemplatePart::Expression(expr) => {
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
                self.generate_expression(expr);
            }
            ExpressionKind::OptionalMember(object, member) => {
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
                self.write("(function() local __ok, __result = pcall(function() return ");
                self.generate_expression(left);
                self.writeln(" end); ");
                self.write("if __ok then return __result else return ");
                self.generate_expression(right);
                self.writeln(" end end)()");
            }
        }
    }

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

    pub fn generate_argument(&mut self, arg: &Argument) {
        self.generate_expression(&arg.value);
    }

    pub fn generate_object_property(&mut self, prop: &ObjectProperty) {
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

    pub fn generate_binary_expression(
        &mut self,
        op: BinaryOp,
        left: &Expression,
        right: &Expression,
    ) {
        match op {
            BinaryOp::NullCoalesce => {
                self.generate_null_coalesce(left, right);
            }

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

            BinaryOp::Instanceof => {
                self.write("(type(");
                self.generate_expression(left);
                self.write(") == \"table\" and getmetatable(");
                self.generate_expression(left);
                self.write(") == ");
                self.generate_expression(right);
                self.write(")");
            }

            BinaryOp::BitwiseAnd
            | BinaryOp::BitwiseOr
            | BinaryOp::BitwiseXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight => {
                let left_str = self.expression_to_string(left);
                let right_str = self.expression_to_string(right);
                let result = self.strategy.generate_bitwise_op(op, &left_str, &right_str);
                self.write(&result);
            }

            BinaryOp::IntegerDivide => {
                let left_str = self.expression_to_string(left);
                let right_str = self.expression_to_string(right);
                let result = self.strategy.generate_integer_divide(&left_str, &right_str);
                self.write(&result);
            }
        }
    }

    pub fn expression_to_string(&mut self, expr: &Expression) -> String {
        let original_output = std::mem::take(&mut self.output);
        self.generate_expression(expr);
        std::mem::replace(&mut self.output, original_output)
    }

    pub fn generate_null_coalesce(&mut self, left: &Expression, right: &Expression) {
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

    pub fn generate_match_expression(&mut self, match_expr: &MatchExpression) {
        self.write("(function()");
        self.writeln("");
        self.indent();

        self.write_indent();
        self.write("local __match_value = ");
        self.generate_expression(&match_expr.value);
        self.writeln("");

        for (i, arm) in match_expr.arms.iter().enumerate() {
            self.write_indent();
            if i == 0 {
                self.write("if ");
            } else {
                self.write("elseif ");
            }

            self.generate_pattern_match(&arm.pattern, "__match_value");

            if let Some(guard) = &arm.guard {
                self.write(" and (");
                self.generate_expression(guard);
                self.write(")");
            }

            self.write(" then");
            self.writeln("");
            self.indent();

            self.generate_pattern_bindings(&arm.pattern, "__match_value");

            self.write_indent();
            match &arm.body {
                MatchArmBody::Expression(expr) => {
                    self.write("return ");
                    self.generate_expression(expr);
                    self.writeln("");
                }
                MatchArmBody::Block(block) => {
                    for stmt in &block.statements {
                        self.generate_statement(stmt);
                    }
                    self.write_indent();
                    self.writeln("return nil");
                }
            }

            self.dedent();
        }

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

    pub fn generate_pattern_match(&mut self, pattern: &Pattern, value_var: &str) {
        match pattern {
            Pattern::Wildcard(_) => {
                self.write("true");
            }
            Pattern::Identifier(_) => {
                self.write("true");
            }
            Pattern::Literal(lit, _) => {
                self.write(value_var);
                self.write(" == ");
                self.generate_literal(lit);
            }
            Pattern::Array(array_pattern) => {
                self.write("type(");
                self.write(value_var);
                self.write(") == \"table\"");

                for (i, elem) in array_pattern.elements.iter().enumerate() {
                    match elem {
                        typedlua_parser::ast::pattern::ArrayPatternElement::Pattern(pat) => {
                            self.write(" and ");
                            let index_expr = format!("{}[{}]", value_var, i + 1);
                            self.generate_pattern_match(pat, &index_expr);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Rest(_) => {}
                        ArrayPatternElement::Hole => {}
                    }
                }
            }
            Pattern::Object(_) => {
                self.write("type(");
                self.write(value_var);
                self.write(") == \"table\"");
            }
            Pattern::Or(or_pattern) => {
                // Generate: (cond1 or cond2 or cond3 ...)
                self.write("(");
                for (i, alt) in or_pattern.alternatives.iter().enumerate() {
                    if i > 0 {
                        self.write(" or ");
                    }
                    self.generate_pattern_match(alt, value_var);
                }
                self.write(")");
            }
        }
    }

    pub fn generate_pattern_bindings(&mut self, pattern: &Pattern, value_var: &str) {
        match pattern {
            Pattern::Identifier(ident) => {
                self.write_indent();
                self.write("local ");
                let ident_str = self.resolve(ident.node);
                self.write(&ident_str);
                self.write(" = ");
                self.write(value_var);
                self.writeln("");
            }
            Pattern::Array(array_pattern) => {
                for (i, elem) in array_pattern.elements.iter().enumerate() {
                    match elem {
                        ArrayPatternElement::Pattern(pat) => {
                            let index_expr = format!("{}[{}]", value_var, i + 1);
                            self.generate_pattern_bindings(pat, &index_expr);
                        }
                        ArrayPatternElement::Rest(ident) => {
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
                        ArrayPatternElement::Hole => {}
                    }
                }
            }
            Pattern::Object(object_pattern) => {
                for prop in &object_pattern.properties {
                    if let Some(value_pattern) = &prop.value {
                        let key_str = self.resolve(prop.key.node);
                        let prop_expr = format!("{}.{}", value_var, key_str);
                        self.generate_pattern_bindings(value_pattern, &prop_expr);
                    } else {
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
            Pattern::Wildcard(_) | Pattern::Literal(_, _) => {}
            Pattern::Or(or_pattern) => {
                // All alternatives bind same variables (verified by type checker during checking)
                // Therefore we can safely use the first alternative for binding generation
                if let Some(first) = or_pattern.alternatives.first() {
                    self.generate_pattern_bindings(first, value_var);
                }
            }
        }
    }
}
