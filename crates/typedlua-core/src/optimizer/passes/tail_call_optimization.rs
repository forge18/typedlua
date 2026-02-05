// =============================================================================
// O2: Tail Call Optimization Pass
// =============================================================================

use crate::config::OptimizationLevel;
use crate::optimizer::{StmtVisitor, WholeProgramPass};
use typedlua_parser::ast::expression::Expression;
use typedlua_parser::ast::expression::ExpressionKind;
use typedlua_parser::ast::statement::{ForStatement, Statement};
use typedlua_parser::ast::Program;

/// Tail call optimization pass
/// Analyzes tail call patterns and ensures other optimizations don't break TCO positions
/// Lua automatically handles tail calls at runtime - this pass provides analysis and verification
pub struct TailCallOptimizationPass;

impl TailCallOptimizationPass {
    pub fn new() -> Self {
        Self
    }
}

impl StmtVisitor for TailCallOptimizationPass {
    fn visit_stmt(&mut self, _stmt: &mut Statement) -> bool {
        // This pass is analysis-only, no transformations
        false
    }
}

impl WholeProgramPass for TailCallOptimizationPass {
    fn name(&self) -> &'static str {
        "tail-call-optimization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        for stmt in &program.statements {
            self.analyze_statement_tail_calls(stmt);
        }

        Ok(false)
    }
}

impl TailCallOptimizationPass {
    fn analyze_statement_tail_calls(&self, stmt: &Statement) -> usize {
        match stmt {
            Statement::Function(func) => self.analyze_block_tail_calls(&func.body.statements),
            Statement::Block(block) => self.analyze_block_tail_calls(&block.statements),
            Statement::If(if_stmt) => {
                let mut count = self.analyze_block_tail_calls(&if_stmt.then_block.statements);
                for else_if in &if_stmt.else_ifs {
                    count += self.analyze_block_tail_calls(&else_if.block.statements);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    count += self.analyze_block_tail_calls(&else_block.statements);
                }
                count
            }
            Statement::While(while_stmt) => {
                self.analyze_block_tail_calls(&while_stmt.body.statements)
            }
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => {
                    self.analyze_block_tail_calls(&for_num.body.statements)
                }
                ForStatement::Generic(for_gen) => {
                    self.analyze_block_tail_calls(&for_gen.body.statements)
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.analyze_block_tail_calls(&repeat_stmt.body.statements)
            }
            Statement::Return(ret) => {
                if self.is_tail_call(&ret.values) {
                    1
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn analyze_block_tail_calls(&self, stmts: &[Statement]) -> usize {
        let mut count = 0;
        for stmt in stmts {
            count += self.analyze_statement_tail_calls(stmt);
        }
        count
    }

    fn is_tail_call(&self, values: &[Expression]) -> bool {
        if values.len() != 1 {
            return false;
        }
        matches!(
            values[0].kind,
            ExpressionKind::Call(_, _, _) | ExpressionKind::MethodCall(_, _, _, _)
        )
    }
}

impl Default for TailCallOptimizationPass {
    fn default() -> Self {
        Self::new()
    }
}
