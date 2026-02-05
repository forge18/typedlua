use crate::config::OptimizationLevel;
use crate::optimizer::{StmtVisitor, WholeProgramPass};
use typedlua_parser::ast::statement::{ForStatement, Statement};
use typedlua_parser::ast::Program;

pub struct DeadCodeEliminationPass;

impl DeadCodeEliminationPass {
    pub fn new() -> Self {
        Self
    }
}

impl StmtVisitor for DeadCodeEliminationPass {
    fn visit_stmt(&mut self, _stmt: &mut Statement) -> bool {
        // Dead code elimination happens at the block level, not individual statements
        // The actual elimination is done in run() via eliminate_dead_code()
        false
    }
}

impl WholeProgramPass for DeadCodeEliminationPass {
    fn name(&self) -> &'static str {
        "dead-code-elimination"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        let original_len = program.statements.len();
        self.eliminate_dead_code(&mut program.statements);
        Ok(program.statements.len() != original_len)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl DeadCodeEliminationPass {
    fn eliminate_dead_code(&mut self, stmts: &mut Vec<Statement>) -> bool {
        let mut changed = false;
        let mut i = 0;

        while i < stmts.len() {
            // Check if this is a return/break/continue statement
            let is_terminal = matches!(
                stmts[i],
                Statement::Return(_) | Statement::Break(_) | Statement::Continue(_)
            );

            if is_terminal {
                // Remove all statements after this one
                let new_len = i + 1;
                if stmts.len() > new_len {
                    stmts.truncate(new_len);
                    changed = true;
                }
                break;
            }

            // Recurse into blocks
            changed |= match &mut stmts[i] {
                Statement::If(if_stmt) => {
                    let mut local_changed =
                        self.eliminate_dead_code(&mut if_stmt.then_block.statements);
                    for else_if in &mut if_stmt.else_ifs {
                        local_changed |= self.eliminate_dead_code(&mut else_if.block.statements);
                    }
                    if let Some(else_block) = &mut if_stmt.else_block {
                        local_changed |= self.eliminate_dead_code(&mut else_block.statements);
                    }
                    local_changed
                }
                Statement::While(while_stmt) => {
                    self.eliminate_dead_code(&mut while_stmt.body.statements)
                }
                Statement::For(for_stmt) => match &mut **for_stmt {
                    ForStatement::Numeric(for_num) => {
                        self.eliminate_dead_code(&mut for_num.body.statements)
                    }
                    ForStatement::Generic(for_gen) => {
                        self.eliminate_dead_code(&mut for_gen.body.statements)
                    }
                },
                Statement::Function(func) => self.eliminate_dead_code(&mut func.body.statements),
                _ => false,
            };

            i += 1;
        }

        changed
    }
}

impl Default for DeadCodeEliminationPass {
    fn default() -> Self {
        Self::new()
    }
}
