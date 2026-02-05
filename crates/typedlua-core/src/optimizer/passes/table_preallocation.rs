use crate::config::OptimizationLevel;
use crate::optimizer::{ExprVisitor, WholeProgramPass};
use typedlua_parser::ast::expression::Expression;
use typedlua_parser::ast::statement::Statement;
use typedlua_parser::ast::Program;

pub struct TablePreallocationPass;

impl TablePreallocationPass {
    pub fn new() -> Self {
        Self
    }
}

impl ExprVisitor for TablePreallocationPass {
    fn visit_expr(&mut self, _expr: &mut Expression) -> bool {
        // This pass is currently analysis-only, no transformations
        // Future: Could add metadata to table expressions for codegen hints
        false
    }
}

impl WholeProgramPass for TablePreallocationPass {
    fn name(&self) -> &'static str {
        "table-preallocation"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        // Analyze table constructors and collect size information
        // This pass doesn't modify the AST directly, but could add metadata
        // for codegen to generate table.create() calls with size hints
        let mut _table_count = 0;

        for stmt in &program.statements {
            _table_count += self.count_tables_in_statement(stmt);
        }

        // Currently a no-op analysis pass - codegen handles preallocation
        Ok(false)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl TablePreallocationPass {
    fn count_tables_in_statement(&self, stmt: &Statement) -> usize {
        use typedlua_parser::ast::statement::Statement;

        match stmt {
            Statement::Variable(decl) => self.count_tables_in_expression(&decl.initializer),
            Statement::Expression(expr) => self.count_tables_in_expression(expr),
            Statement::If(if_stmt) => {
                let mut count = 0;
                for s in &if_stmt.then_block.statements {
                    count += self.count_tables_in_statement(s);
                }
                for else_if in &if_stmt.else_ifs {
                    for s in &else_if.block.statements {
                        count += self.count_tables_in_statement(s);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for s in &else_block.statements {
                        count += self.count_tables_in_statement(s);
                    }
                }
                count
            }
            Statement::Function(func) => {
                let mut count = 0;
                for s in &func.body.statements {
                    count += self.count_tables_in_statement(s);
                }
                count
            }
            _ => 0,
        }
    }

    fn count_tables_in_expression(&self, expr: &Expression) -> usize {
        use typedlua_parser::ast::expression::ExpressionKind;

        match &expr.kind {
            ExpressionKind::Object(fields) => {
                let mut count = 1; // Count this table
                for field in fields {
                    match field {
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            value,
                            ..
                        } => {
                            count += self.count_tables_in_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            value,
                            ..
                        } => {
                            count += self.count_tables_in_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value, ..
                        } => {
                            count += self.count_tables_in_expression(value);
                        }
                    }
                }
                count
            }
            ExpressionKind::Array(elements) => {
                let mut count = 1; // Count this array
                for elem in elements {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(expr) => {
                            count += self.count_tables_in_expression(expr);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(expr) => {
                            count += self.count_tables_in_expression(expr);
                        }
                    }
                }
                count
            }
            ExpressionKind::Binary(_, left, right) => {
                self.count_tables_in_expression(left) + self.count_tables_in_expression(right)
            }
            ExpressionKind::Unary(_, operand) => self.count_tables_in_expression(operand),
            ExpressionKind::Call(func, args, _) => {
                let mut count = self.count_tables_in_expression(func);
                for arg in args {
                    count += self.count_tables_in_expression(&arg.value);
                }
                count
            }
            _ => 0,
        }
    }
}

impl Default for TablePreallocationPass {
    fn default() -> Self {
        Self::new()
    }
}
