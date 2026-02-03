use super::super::CodeGenerator;
use typedlua_parser::ast::expression::{AssignmentOp, BinaryOp};

impl CodeGenerator {
    pub fn generate_binary_expression(
        &mut self,
        op: BinaryOp,
        left: &typedlua_parser::ast::expression::Expression,
        right: &typedlua_parser::ast::expression::Expression,
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

    pub fn generate_unary_expression(
        &mut self,
        op: typedlua_parser::ast::expression::UnaryOp,
        operand: &typedlua_parser::ast::expression::Expression,
    ) {
        if op == typedlua_parser::ast::expression::UnaryOp::BitwiseNot
            && !self.strategy.supports_native_bitwise()
        {
            let operand_str = self.expression_to_string(operand);
            let result = self.strategy.generate_unary_bitwise_not(&operand_str);
            self.write(&result);
        } else {
            let op_str = self.unary_op_to_string(op).to_string();
            self.write(&op_str);
            self.generate_expression(operand);
        }
    }

    pub fn generate_assignment_expression(
        &mut self,
        target: &typedlua_parser::ast::expression::Expression,
        op: AssignmentOp,
        value: &typedlua_parser::ast::expression::Expression,
    ) {
        match op {
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
        }
    }
}
