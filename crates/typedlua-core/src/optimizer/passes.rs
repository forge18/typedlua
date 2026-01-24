use crate::ast::expression::{BinaryOp, Expression, ExpressionKind, Literal, UnaryOp};
use crate::ast::pattern::Pattern;
use crate::ast::statement::{Block, ForStatement, Statement, VariableDeclaration, VariableKind};
use crate::ast::Program;
use crate::ast::Spanned;
use crate::config::OptimizationLevel;
use crate::errors::CompilationError;
use crate::optimizer::OptimizationPass;
use crate::span::Span;
use crate::string_interner::StringInterner;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Constant folding optimization pass
/// Evaluates constant expressions at compile time
pub struct ConstantFoldingPass;

impl OptimizationPass for ConstantFoldingPass {
    fn name(&self) -> &'static str {
        "constant-folding"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        let mut changed = false;

        for stmt in &mut program.statements {
            changed |= self.fold_statement(stmt);
        }

        Ok(changed)
    }
}

impl ConstantFoldingPass {
    fn fold_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::Variable(decl) => self.fold_expression(&mut decl.initializer),
            Statement::Expression(expr) => self.fold_expression(expr),
            Statement::If(if_stmt) => {
                let mut changed = self.fold_expression(&mut if_stmt.condition);
                changed |= self.fold_block_statements(&mut if_stmt.then_block.statements);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.fold_expression(&mut else_if.condition);
                    changed |= self.fold_block_statements(&mut else_if.block.statements);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.fold_block_statements(&mut else_block.statements);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.fold_expression(&mut while_stmt.condition);
                changed |= self.fold_block_statements(&mut while_stmt.body.statements);
                changed
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Numeric(for_num) => {
                    let mut changed = self.fold_expression(&mut for_num.start);
                    changed |= self.fold_expression(&mut for_num.end);
                    if let Some(step) = &mut for_num.step {
                        changed |= self.fold_expression(step);
                    }
                    changed |= self.fold_block_statements(&mut for_num.body.statements);
                    changed
                }
                ForStatement::Generic(for_gen) => {
                    let mut changed = false;
                    for expr in &mut for_gen.iterators {
                        changed |= self.fold_expression(expr);
                    }
                    changed |= self.fold_block_statements(&mut for_gen.body.statements);
                    changed
                }
            },
            Statement::Return(ret_stmt) => {
                let mut changed = false;
                for expr in &mut ret_stmt.values {
                    changed |= self.fold_expression(expr);
                }
                changed
            }
            Statement::Function(func) => self.fold_block_statements(&mut func.body.statements),
            Statement::Class(_) => false, // Skip for now
            _ => false,
        }
    }

    fn fold_block_statements(&mut self, stmts: &mut [Statement]) -> bool {
        let mut changed = false;
        for stmt in stmts {
            changed |= self.fold_statement(stmt);
        }
        changed
    }

    fn fold_expression(&mut self, expr: &mut Expression) -> bool {
        match &mut expr.kind {
            ExpressionKind::Binary(op, left, right) => {
                let left_changed = self.fold_expression(left);
                let right_changed = self.fold_expression(right);

                // Try to fold if both operands are literals
                if let (
                    ExpressionKind::Literal(Literal::Number(l)),
                    ExpressionKind::Literal(Literal::Number(r)),
                ) = (&left.kind, &right.kind)
                {
                    if let Some(result) = self.fold_numeric_binary_op(*op, *l, *r) {
                        expr.kind = ExpressionKind::Literal(Literal::Number(result));
                        return true;
                    }
                }

                // Try to fold boolean operations
                if let (
                    ExpressionKind::Literal(Literal::Boolean(l)),
                    ExpressionKind::Literal(Literal::Boolean(r)),
                ) = (&left.kind, &right.kind)
                {
                    if let Some(result) = self.fold_boolean_binary_op(*op, *l, *r) {
                        expr.kind = ExpressionKind::Literal(Literal::Boolean(result));
                        return true;
                    }
                }

                left_changed || right_changed
            }
            ExpressionKind::Unary(op, operand) => {
                let changed = self.fold_expression(operand);

                // Try to fold unary operations
                match (&operand.kind, op) {
                    (ExpressionKind::Literal(Literal::Number(n)), UnaryOp::Negate) => {
                        expr.kind = ExpressionKind::Literal(Literal::Number(-n));
                        return true;
                    }
                    (ExpressionKind::Literal(Literal::Boolean(b)), UnaryOp::Not) => {
                        expr.kind = ExpressionKind::Literal(Literal::Boolean(!b));
                        return true;
                    }
                    _ => {}
                }

                changed
            }
            ExpressionKind::Call(func, args) => {
                let mut changed = self.fold_expression(func);
                for arg in args {
                    changed |= self.fold_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Index(obj, index) => {
                let obj_changed = self.fold_expression(obj);
                let index_changed = self.fold_expression(index);
                obj_changed || index_changed
            }
            ExpressionKind::Member(obj, _) => self.fold_expression(obj),
            ExpressionKind::Object(fields) => {
                let mut changed = false;
                for field in fields {
                    match field {
                        crate::ast::expression::ObjectProperty::Property { value, .. } => {
                            changed |= self.fold_expression(value);
                        }
                        crate::ast::expression::ObjectProperty::Computed { key, value, .. } => {
                            changed |= self.fold_expression(key);
                            changed |= self.fold_expression(value);
                        }
                        crate::ast::expression::ObjectProperty::Spread { value, .. } => {
                            changed |= self.fold_expression(value);
                        }
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn fold_numeric_binary_op(&self, op: BinaryOp, left: f64, right: f64) -> Option<f64> {
        let l = left;
        let r = right;

        match op {
            BinaryOp::Add => Some(l + r),
            BinaryOp::Subtract => Some(l - r),
            BinaryOp::Multiply => Some(l * r),
            BinaryOp::Divide => {
                if r != 0.0 {
                    Some(l / r)
                } else {
                    None // Don't fold division by zero
                }
            }
            BinaryOp::Modulo => {
                if r != 0.0 {
                    Some(l % r)
                } else {
                    None
                }
            }
            BinaryOp::Power => Some(l.powf(r)),
            _ => None,
        }
    }

    fn fold_boolean_binary_op(&self, op: BinaryOp, left: bool, right: bool) -> Option<bool> {
        match op {
            BinaryOp::And => Some(left && right),
            BinaryOp::Or => Some(left || right),
            BinaryOp::Equal => Some(left == right),
            BinaryOp::NotEqual => Some(left != right),
            _ => None,
        }
    }
}

/// Dead code elimination pass
/// Removes unreachable code after return statements
pub struct DeadCodeEliminationPass;

impl OptimizationPass for DeadCodeEliminationPass {
    fn name(&self) -> &'static str {
        "dead-code-elimination"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        let original_len = program.statements.len();
        self.eliminate_dead_code(&mut program.statements);
        Ok(program.statements.len() != original_len)
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

/// Algebraic simplification pass
/// Simplifies expressions using algebraic identities
pub struct AlgebraicSimplificationPass;

impl OptimizationPass for AlgebraicSimplificationPass {
    fn name(&self) -> &'static str {
        "algebraic-simplification"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        let mut changed = false;

        for stmt in &mut program.statements {
            changed |= self.simplify_statement(stmt);
        }

        Ok(changed)
    }
}

impl AlgebraicSimplificationPass {
    fn simplify_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::Variable(decl) => self.simplify_expression(&mut decl.initializer),
            Statement::Expression(expr) => self.simplify_expression(expr),
            Statement::If(if_stmt) => {
                let mut changed = self.simplify_expression(&mut if_stmt.condition);
                changed |= self.simplify_block_statements(&mut if_stmt.then_block.statements);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.simplify_expression(&mut else_if.condition);
                    changed |= self.simplify_block_statements(&mut else_if.block.statements);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.simplify_block_statements(&mut else_block.statements);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.simplify_expression(&mut while_stmt.condition);
                changed |= self.simplify_block_statements(&mut while_stmt.body.statements);
                changed
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Numeric(for_num) => {
                    let mut changed = self.simplify_expression(&mut for_num.start);
                    changed |= self.simplify_expression(&mut for_num.end);
                    if let Some(step) = &mut for_num.step {
                        changed |= self.simplify_expression(step);
                    }
                    changed |= self.simplify_block_statements(&mut for_num.body.statements);
                    changed
                }
                ForStatement::Generic(for_gen) => {
                    let mut changed = false;
                    for expr in &mut for_gen.iterators {
                        changed |= self.simplify_expression(expr);
                    }
                    changed |= self.simplify_block_statements(&mut for_gen.body.statements);
                    changed
                }
            },
            Statement::Return(ret_stmt) => {
                let mut changed = false;
                for expr in &mut ret_stmt.values {
                    changed |= self.simplify_expression(expr);
                }
                changed
            }
            _ => false,
        }
    }

    fn simplify_block_statements(&mut self, stmts: &mut [Statement]) -> bool {
        let mut changed = false;
        for stmt in stmts {
            changed |= self.simplify_statement(stmt);
        }
        changed
    }

    fn simplify_expression(&mut self, expr: &mut Expression) -> bool {
        match &mut expr.kind {
            ExpressionKind::Binary(op, left, right) => {
                let mut changed = self.simplify_expression(left);
                changed |= self.simplify_expression(right);

                // Algebraic simplifications
                match op {
                    // x + 0 = x or 0 + x = x
                    BinaryOp::Add => {
                        if is_zero(&right.kind) {
                            *expr = (**left).clone();
                            return true;
                        }
                        if is_zero(&left.kind) {
                            *expr = (**right).clone();
                            return true;
                        }
                    }
                    // x - 0 = x
                    BinaryOp::Subtract => {
                        if is_zero(&right.kind) {
                            *expr = (**left).clone();
                            return true;
                        }
                    }
                    // x * 0 = 0 or 0 * x = 0
                    BinaryOp::Multiply => {
                        if is_zero(&right.kind) || is_zero(&left.kind) {
                            expr.kind = ExpressionKind::Literal(Literal::Number(0.0));
                            return true;
                        }
                        // x * 1 = x or 1 * x = x
                        if is_one(&right.kind) {
                            *expr = (**left).clone();
                            return true;
                        }
                        if is_one(&left.kind) {
                            *expr = (**right).clone();
                            return true;
                        }
                    }
                    // x / 1 = x
                    BinaryOp::Divide => {
                        if is_one(&right.kind) {
                            *expr = (**left).clone();
                            return true;
                        }
                    }
                    // true && x = x, false && x = false
                    BinaryOp::And => {
                        if let ExpressionKind::Literal(Literal::Boolean(b)) = &left.kind {
                            if *b {
                                *expr = (**right).clone();
                            } else {
                                expr.kind = ExpressionKind::Literal(Literal::Boolean(false));
                            }
                            return true;
                        }
                        if let ExpressionKind::Literal(Literal::Boolean(b)) = &right.kind {
                            if *b {
                                *expr = (**left).clone();
                            } else {
                                expr.kind = ExpressionKind::Literal(Literal::Boolean(false));
                            }
                            return true;
                        }
                    }
                    // true || x = true, false || x = x
                    BinaryOp::Or => {
                        if let ExpressionKind::Literal(Literal::Boolean(b)) = &left.kind {
                            if *b {
                                expr.kind = ExpressionKind::Literal(Literal::Boolean(true));
                            } else {
                                *expr = (**right).clone();
                            }
                            return true;
                        }
                        if let ExpressionKind::Literal(Literal::Boolean(b)) = &right.kind {
                            if *b {
                                expr.kind = ExpressionKind::Literal(Literal::Boolean(true));
                            } else {
                                *expr = (**left).clone();
                            }
                            return true;
                        }
                    }
                    _ => {}
                }

                changed
            }
            ExpressionKind::Unary(op, operand) => {
                let changed = self.simplify_expression(operand);

                // !!x = x (double negation)
                if let UnaryOp::Not = op {
                    if let ExpressionKind::Unary(UnaryOp::Not, inner) = &operand.kind {
                        *expr = (**inner).clone();
                        return true;
                    }
                }

                changed
            }
            ExpressionKind::Call(func, args) => {
                let mut changed = self.simplify_expression(func);
                for arg in args {
                    changed |= self.simplify_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Member(obj, _) => self.simplify_expression(obj),
            _ => false,
        }
    }
}

// Helper functions
fn is_zero(expr: &ExpressionKind) -> bool {
    matches!(
        expr,
        ExpressionKind::Literal(Literal::Number(n)) if *n == 0.0
    )
}

fn is_one(expr: &ExpressionKind) -> bool {
    matches!(
        expr,
        ExpressionKind::Literal(Literal::Number(n)) if *n == 1.0
    )
}

// =============================================================================
// O1: Table Preallocation Pass
// =============================================================================

/// Table preallocation optimization pass
/// Analyzes table constructors and adds size hints for Lua
/// Note: This is a placeholder - actual hints would be used by codegen
pub struct TablePreallocationPass;

impl OptimizationPass for TablePreallocationPass {
    fn name(&self) -> &'static str {
        "table-preallocation"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
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
}

impl TablePreallocationPass {
    fn count_tables_in_statement(&self, stmt: &Statement) -> usize {
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
        match &expr.kind {
            ExpressionKind::Object(fields) => {
                let mut count = 1; // Count this table
                for field in fields {
                    match field {
                        crate::ast::expression::ObjectProperty::Property { value, .. } => {
                            count += self.count_tables_in_expression(value);
                        }
                        crate::ast::expression::ObjectProperty::Computed { value, .. } => {
                            count += self.count_tables_in_expression(value);
                        }
                        crate::ast::expression::ObjectProperty::Spread { value, .. } => {
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
                        crate::ast::expression::ArrayElement::Expression(expr) => {
                            count += self.count_tables_in_expression(expr);
                        }
                        crate::ast::expression::ArrayElement::Spread(expr) => {
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
            ExpressionKind::Call(func, args) => {
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

// =============================================================================
// O1: Global Localization Pass
// =============================================================================

/// Global localization optimization pass
/// Identifies frequently used globals and creates local references to reduce
/// repeated table lookups in Lua (e.g., `local _math = math` then use `_math.sin`)
pub struct GlobalLocalizationPass {
    interner: Arc<StringInterner>,
}

impl GlobalLocalizationPass {
    /// Create a new pass with the given string interner
    pub fn new(interner: Arc<StringInterner>) -> Self {
        GlobalLocalizationPass { interner }
    }
}

impl OptimizationPass for GlobalLocalizationPass {
    fn name(&self) -> &'static str {
        "global-localization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O1
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        // Apply global localization to the top-level program statements as a block.
        // This ensures that frequently used globals are hoisted to a local variable at the program scope.
        let mut block = Block {
            statements: std::mem::take(&mut program.statements),
            span: program.span,
        };
        let changed = self.localize_in_block(&mut block, HashSet::new());
        program.statements = block.statements;
        program.span = block.span; // preserve span (unchanged)
        Ok(changed)
    }
}

impl GlobalLocalizationPass {
    #[allow(dead_code)]

    fn localize_in_block(
        &self,
        block: &mut Block,
        mut declared_locals: HashSet<crate::string_interner::StringId>,
    ) -> bool {
        let mut changed = false;

        let mut global_usage: HashMap<crate::string_interner::StringId, usize> = HashMap::new();

        for stmt in &block.statements {
            self.collect_global_usage_optimized(stmt, &mut global_usage, &declared_locals);
        }

        let frequently_used: Vec<_> = global_usage
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .filter(|(name, _)| !declared_locals.contains(name))
            .collect();

        let mut new_statements = Vec::new();

        for (name, count) in &frequently_used {
            if *count > 1 {
                let local_name_id = self.create_local_name(*name);
                let span = Span::dummy();
                let var_decl = self.create_local_declaration(*name, local_name_id, span);

                new_statements.push(Statement::Variable(var_decl));
                declared_locals.insert(local_name_id);
                changed = true;
            }
        }

        for stmt in &mut block.statements {
            self.replace_global_usages(stmt, &frequently_used, &declared_locals);
        }

        new_statements.extend(block.statements.clone());
        block.statements = new_statements;

        changed
    }

    fn create_local_declaration(
        &self,
        global_name: crate::string_interner::StringId,
        local_name: crate::string_interner::StringId,
        span: Span,
    ) -> VariableDeclaration {
        let local_ident = Spanned::new(local_name, span);
        let initializer = Expression::new(ExpressionKind::Identifier(global_name), span);

        VariableDeclaration {
            kind: VariableKind::Local,
            pattern: Pattern::Identifier(local_ident),
            type_annotation: None,
            initializer,
            span,
        }
    }

    fn create_local_name(
        &self,
        original: crate::string_interner::StringId,
    ) -> crate::string_interner::StringId {
        let name = self.interner.resolve(original);
        let local_name = format!("_{}", name);
        self.interner.get_or_intern(&local_name)
    }

    fn collect_global_usage_optimized(
        &self,
        stmt: &Statement,
        usage: &mut HashMap<crate::string_interner::StringId, usize>,
        declared_locals: &HashSet<crate::string_interner::StringId>,
    ) {
        match stmt {
            Statement::Variable(decl) => {
                self.collect_from_expression_optimized(&decl.initializer, usage, declared_locals);
            }
            Statement::Expression(expr) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            Statement::If(if_stmt) => {
                self.collect_from_expression_optimized(&if_stmt.condition, usage, declared_locals);
                for s in &if_stmt.then_block.statements {
                    self.collect_global_usage_optimized(s, usage, declared_locals);
                }
                for else_if in &if_stmt.else_ifs {
                    self.collect_from_expression_optimized(
                        &else_if.condition,
                        usage,
                        declared_locals,
                    );
                    for s in &else_if.block.statements {
                        self.collect_global_usage_optimized(s, usage, declared_locals);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for s in &else_block.statements {
                        self.collect_global_usage_optimized(s, usage, declared_locals);
                    }
                }
            }
            Statement::Function(func) => {
                for s in &func.body.statements {
                    self.collect_global_usage_optimized(s, usage, declared_locals);
                }
            }
            Statement::Return(ret) => {
                for expr in &ret.values {
                    self.collect_from_expression_optimized(expr, usage, declared_locals);
                }
            }
            Statement::For(for_stmt) => {
                let mut locals = declared_locals.clone();
                match &**for_stmt {
                    ForStatement::Numeric(for_num) => {
                        locals.insert(for_num.variable.node);
                    }
                    ForStatement::Generic(for_gen) => {
                        for var in &for_gen.variables {
                            locals.insert(var.node);
                        }
                    }
                }
                match &**for_stmt {
                    ForStatement::Numeric(for_num) => {
                        for s in &for_num.body.statements {
                            self.collect_global_usage_optimized(s, usage, &locals);
                        }
                    }
                    ForStatement::Generic(for_gen) => {
                        for s in &for_gen.body.statements {
                            self.collect_global_usage_optimized(s, usage, &locals);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_from_expression_optimized(
        &self,
        expr: &Expression,
        usage: &mut HashMap<crate::string_interner::StringId, usize>,
        declared_locals: &HashSet<crate::string_interner::StringId>,
    ) {
        match &expr.kind {
            ExpressionKind::Identifier(name) => {
                if !declared_locals.contains(name) {
                    *usage.entry(*name).or_insert(0) += 1;
                }
            }
            ExpressionKind::Binary(_, left, right) => {
                self.collect_from_expression_optimized(left, usage, declared_locals);
                self.collect_from_expression_optimized(right, usage, declared_locals);
            }
            ExpressionKind::Unary(_, operand) => {
                self.collect_from_expression_optimized(operand, usage, declared_locals);
            }
            ExpressionKind::Call(func, args) => {
                self.collect_from_expression_optimized(func, usage, declared_locals);
                for arg in args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::Member(obj, _) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
            }
            ExpressionKind::MethodCall(obj, _, args) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
                for arg in args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::Index(obj, index) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
                self.collect_from_expression_optimized(index, usage, declared_locals);
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.collect_from_expression_optimized(cond, usage, declared_locals);
                self.collect_from_expression_optimized(then_expr, usage, declared_locals);
                self.collect_from_expression_optimized(else_expr, usage, declared_locals);
            }
            ExpressionKind::Pipe(left, right) => {
                self.collect_from_expression_optimized(left, usage, declared_locals);
                self.collect_from_expression_optimized(right, usage, declared_locals);
            }
            ExpressionKind::Arrow(arrow) => {
                for param in &arrow.parameters {
                    if let Some(default) = &param.default {
                        self.collect_from_expression_optimized(default, usage, declared_locals);
                    }
                }
                self.collect_from_arrow_body(&arrow.body, usage, declared_locals);
            }
            ExpressionKind::Match(match_expr) => {
                self.collect_from_expression_optimized(&match_expr.value, usage, declared_locals);
                for arm in &match_expr.arms {
                    self.collect_from_match_arm_body(&arm.body, usage, declared_locals);
                }
            }
            ExpressionKind::New(callee, args) => {
                self.collect_from_expression_optimized(callee, usage, declared_locals);
                for arg in args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.collect_from_expression_optimized(
                    &try_expr.expression,
                    usage,
                    declared_locals,
                );
                self.collect_from_expression_optimized(
                    &try_expr.catch_expression,
                    usage,
                    declared_locals,
                );
            }
            ExpressionKind::ErrorChain(_, expr) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            ExpressionKind::OptionalMember(obj, _) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
                self.collect_from_expression_optimized(index, usage, declared_locals);
            }
            ExpressionKind::OptionalCall(obj, args) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
                for arg in args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
                for arg in args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::TypeAssertion(expr, _) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            _ => {}
        }
    }

    fn collect_from_arrow_body(
        &self,
        body: &crate::ast::expression::ArrowBody,
        usage: &mut HashMap<crate::string_interner::StringId, usize>,
        declared_locals: &HashSet<crate::string_interner::StringId>,
    ) {
        match body {
            crate::ast::expression::ArrowBody::Expression(expr) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            crate::ast::expression::ArrowBody::Block(block) => {
                for stmt in &block.statements {
                    self.collect_global_usage_optimized(stmt, usage, declared_locals);
                }
            }
        }
    }

    fn collect_from_match_arm_body(
        &self,
        body: &crate::ast::expression::MatchArmBody,
        usage: &mut HashMap<crate::string_interner::StringId, usize>,
        declared_locals: &HashSet<crate::string_interner::StringId>,
    ) {
        match body {
            crate::ast::expression::MatchArmBody::Expression(expr) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            crate::ast::expression::MatchArmBody::Block(block) => {
                for stmt in &block.statements {
                    self.collect_global_usage_optimized(stmt, usage, declared_locals);
                }
            }
        }
    }

    fn replace_global_usages(
        &self,
        stmt: &mut Statement,
        frequently_used: &[(crate::string_interner::StringId, usize)],
        declared_locals: &HashSet<crate::string_interner::StringId>,
    ) {
        match stmt {
            Statement::Variable(decl) => {
                self.replace_in_expression(&mut decl.initializer, frequently_used, declared_locals);
            }
            Statement::Expression(expr) => {
                self.replace_in_expression(expr, frequently_used, declared_locals);
            }
            Statement::If(if_stmt) => {
                self.replace_in_expression(
                    &mut if_stmt.condition,
                    frequently_used,
                    declared_locals,
                );
                for s in &mut if_stmt.then_block.statements {
                    self.replace_global_usages(s, frequently_used, declared_locals);
                }
                for else_if in &mut if_stmt.else_ifs {
                    self.replace_in_expression(
                        &mut else_if.condition,
                        frequently_used,
                        declared_locals,
                    );
                    for s in &mut else_if.block.statements {
                        self.replace_global_usages(s, frequently_used, declared_locals);
                    }
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    for s in &mut else_block.statements {
                        self.replace_global_usages(s, frequently_used, declared_locals);
                    }
                }
            }
            Statement::While(while_stmt) => {
                self.replace_in_expression(
                    &mut while_stmt.condition,
                    frequently_used,
                    declared_locals,
                );
                for s in &mut while_stmt.body.statements {
                    self.replace_global_usages(s, frequently_used, declared_locals);
                }
            }
            Statement::For(for_stmt) => {
                let mut new_locals = declared_locals.clone();
                match &mut **for_stmt {
                    ForStatement::Numeric(ref mut for_num) => {
                        new_locals.insert(for_num.variable.node);
                    }
                    ForStatement::Generic(ref mut for_gen) => {
                        for var in &for_gen.variables {
                            new_locals.insert(var.node);
                        }
                    }
                }
                match &mut **for_stmt {
                    ForStatement::Numeric(ref mut for_num) => {
                        for s in &mut for_num.body.statements {
                            self.replace_global_usages(s, frequently_used, &new_locals);
                        }
                    }
                    ForStatement::Generic(ref mut for_gen) => {
                        for s in &mut for_gen.body.statements {
                            self.replace_global_usages(s, frequently_used, &new_locals);
                        }
                    }
                }
            }
            Statement::Return(ret) => {
                for expr in &mut ret.values {
                    self.replace_in_expression(expr, frequently_used, declared_locals);
                }
            }
            Statement::Function(func) => {
                let new_locals = declared_locals.clone();
                for s in &mut func.body.statements {
                    self.replace_global_usages(s, frequently_used, &new_locals);
                }
            }
            _ => {}
        }
    }

    fn replace_in_expression(
        &self,
        expr: &mut Expression,
        frequently_used: &[(crate::string_interner::StringId, usize)],
        declared_locals: &HashSet<crate::string_interner::StringId>,
    ) {
        match &mut expr.kind {
            ExpressionKind::Identifier(name) => {
                if !declared_locals.contains(name) {
                    for (global_name, count) in frequently_used {
                        if *name == *global_name && *count > 1 {
                            let local_name_id = self.create_local_name(*name);
                            *name = local_name_id;
                            break;
                        }
                    }
                }
            }
            ExpressionKind::Binary(_op, left, right) => {
                self.replace_in_expression(left, frequently_used, declared_locals);
                self.replace_in_expression(right, frequently_used, declared_locals);
            }
            ExpressionKind::Unary(_op, operand) => {
                self.replace_in_expression(operand, frequently_used, declared_locals);
            }
            ExpressionKind::Call(func, args) => {
                self.replace_in_expression(func, frequently_used, declared_locals);
                for arg in args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals);
                }
            }
            ExpressionKind::Member(obj, _) => {
                self.replace_in_expression(obj, frequently_used, declared_locals);
            }
            ExpressionKind::MethodCall(obj, _, args) => {
                self.replace_in_expression(obj, frequently_used, declared_locals);
                for arg in args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals);
                }
            }
            ExpressionKind::Index(obj, index) => {
                self.replace_in_expression(obj, frequently_used, declared_locals);
                self.replace_in_expression(index, frequently_used, declared_locals);
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.replace_in_expression(cond, frequently_used, declared_locals);
                self.replace_in_expression(then_expr, frequently_used, declared_locals);
                self.replace_in_expression(else_expr, frequently_used, declared_locals);
            }
            ExpressionKind::Pipe(left, right) => {
                self.replace_in_expression(left, frequently_used, declared_locals);
                self.replace_in_expression(right, frequently_used, declared_locals);
            }
            ExpressionKind::Match(match_expr) => {
                self.replace_in_expression(&mut match_expr.value, frequently_used, declared_locals);
                for arm in &mut match_expr.arms {
                    self.replace_in_match_arm_body(&mut arm.body, frequently_used, declared_locals);
                }
            }
            ExpressionKind::Arrow(arrow) => {
                for param in &mut arrow.parameters {
                    if let Some(default) = &mut param.default {
                        self.replace_in_expression(default, frequently_used, declared_locals);
                    }
                }
                self.replace_in_arrow_body(&mut arrow.body, frequently_used, declared_locals);
            }
            ExpressionKind::New(callee, args) => {
                self.replace_in_expression(callee, frequently_used, declared_locals);
                for arg in args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.replace_in_expression(
                    &mut try_expr.expression,
                    frequently_used,
                    declared_locals,
                );
                self.replace_in_expression(
                    &mut try_expr.catch_expression,
                    frequently_used,
                    declared_locals,
                );
            }
            ExpressionKind::ErrorChain(_, expr) => {
                self.replace_in_expression(expr, frequently_used, declared_locals);
            }
            ExpressionKind::OptionalMember(obj, _) => {
                self.replace_in_expression(obj, frequently_used, declared_locals);
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                self.replace_in_expression(obj, frequently_used, declared_locals);
                self.replace_in_expression(index, frequently_used, declared_locals);
            }
            ExpressionKind::OptionalCall(obj, args) => {
                self.replace_in_expression(obj, frequently_used, declared_locals);
                for arg in args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args) => {
                self.replace_in_expression(obj, frequently_used, declared_locals);
                for arg in args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals);
                }
            }
            ExpressionKind::TypeAssertion(expr, _) => {
                self.replace_in_expression(expr, frequently_used, declared_locals);
            }
            _ => {}
        }
    }

    fn replace_in_arrow_body(
        &self,
        body: &mut crate::ast::expression::ArrowBody,
        frequently_used: &[(crate::string_interner::StringId, usize)],
        declared_locals: &HashSet<crate::string_interner::StringId>,
    ) {
        match body {
            crate::ast::expression::ArrowBody::Expression(expr) => {
                self.replace_in_expression(expr, frequently_used, declared_locals);
            }
            crate::ast::expression::ArrowBody::Block(block) => {
                for stmt in &mut block.statements {
                    self.replace_global_usages(stmt, frequently_used, declared_locals);
                }
            }
        }
    }

    fn replace_in_match_arm_body(
        &self,
        body: &mut crate::ast::expression::MatchArmBody,
        frequently_used: &[(crate::string_interner::StringId, usize)],
        declared_locals: &HashSet<crate::string_interner::StringId>,
    ) {
        match body {
            crate::ast::expression::MatchArmBody::Expression(expr) => {
                self.replace_in_expression(expr, frequently_used, declared_locals);
            }
            crate::ast::expression::MatchArmBody::Block(block) => {
                for stmt in &mut block.statements {
                    self.replace_global_usages(stmt, frequently_used, declared_locals);
                }
            }
        }
    }
}

// =============================================================================
// O2: Function Inlining Pass
// =============================================================================

/// Function inlining optimization pass (threshold: 5 statements)
/// Inlines small functions at call sites
pub struct FunctionInliningPass {
    threshold: usize,
}

impl Default for FunctionInliningPass {
    fn default() -> Self {
        Self { threshold: 5 }
    }
}

impl OptimizationPass for FunctionInliningPass {
    fn name(&self) -> &'static str {
        "function-inlining"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        // Collect inlinable functions (small, non-recursive, single return)
        let mut inlinable: std::collections::HashMap<
            crate::string_interner::StringId,
            &crate::ast::statement::FunctionDeclaration,
        > = std::collections::HashMap::new();

        for stmt in &program.statements {
            if let Statement::Function(func) = stmt {
                if func.body.statements.len() <= self.threshold && !self.is_recursive(func) {
                    inlinable.insert(func.name.node, func);
                }
            }
        }

        // Currently analysis-only - actual inlining is complex and requires
        // careful handling of scoping, parameter substitution, and return values
        let _ = inlinable;

        Ok(false)
    }
}

impl FunctionInliningPass {
    fn is_recursive(&self, func: &crate::ast::statement::FunctionDeclaration) -> bool {
        // Check if function calls itself
        let name = func.name.node;
        for stmt in &func.body.statements {
            if self.contains_call_to(stmt, name) {
                return true;
            }
        }
        false
    }

    fn contains_call_to(&self, stmt: &Statement, name: crate::string_interner::StringId) -> bool {
        match stmt {
            Statement::Expression(expr) => self.expr_contains_call_to(expr, name),
            Statement::Variable(decl) => self.expr_contains_call_to(&decl.initializer, name),
            Statement::Return(ret) => ret
                .values
                .iter()
                .any(|e| self.expr_contains_call_to(e, name)),
            Statement::If(if_stmt) => {
                self.expr_contains_call_to(&if_stmt.condition, name)
                    || if_stmt
                        .then_block
                        .statements
                        .iter()
                        .any(|s| self.contains_call_to(s, name))
                    || if_stmt.else_ifs.iter().any(|ei| {
                        self.expr_contains_call_to(&ei.condition, name)
                            || ei
                                .block
                                .statements
                                .iter()
                                .any(|s| self.contains_call_to(s, name))
                    })
                    || if_stmt.else_block.as_ref().map_or(false, |eb| {
                        eb.statements.iter().any(|s| self.contains_call_to(s, name))
                    })
            }
            _ => false,
        }
    }

    fn expr_contains_call_to(
        &self,
        expr: &Expression,
        name: crate::string_interner::StringId,
    ) -> bool {
        match &expr.kind {
            ExpressionKind::Call(func, args) => {
                if let ExpressionKind::Identifier(id) = &func.kind {
                    if *id == name {
                        return true;
                    }
                }
                self.expr_contains_call_to(func, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::Binary(_, left, right) => {
                self.expr_contains_call_to(left, name) || self.expr_contains_call_to(right, name)
            }
            ExpressionKind::Unary(_, operand) => self.expr_contains_call_to(operand, name),
            _ => false,
        }
    }
}

// =============================================================================
// O2: Loop Optimization Pass
// =============================================================================

/// Loop optimization pass
/// Converts ipairs to numeric for when possible, hoists invariants
pub struct LoopOptimizationPass;

impl OptimizationPass for LoopOptimizationPass {
    fn name(&self) -> &'static str {
        "loop-optimization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        let mut changed = false;

        for stmt in &mut program.statements {
            changed |= self.optimize_loops_in_statement(stmt);
        }

        Ok(changed)
    }
}

impl LoopOptimizationPass {
    fn optimize_loops_in_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::For(for_stmt) => {
                match &mut **for_stmt {
                    ForStatement::Generic(for_gen) => {
                        // Check if this is ipairs(array) - could be converted to numeric for
                        // Analysis only for now - would need interner to check name
                        if for_gen.iterators.len() == 1 {
                            if let ExpressionKind::Call(_func, _args) = &for_gen.iterators[0].kind {
                                // Could analyze call target - analysis only
                            }
                        }
                    }
                    ForStatement::Numeric(for_num) => {
                        // Optimize loop body
                        let mut changed = false;
                        for s in &mut for_num.body.statements {
                            changed |= self.optimize_loops_in_statement(s);
                        }
                        return changed;
                    }
                }
            }
            Statement::While(while_stmt) => {
                let mut changed = false;
                for s in &mut while_stmt.body.statements {
                    changed |= self.optimize_loops_in_statement(s);
                }
                return changed;
            }
            Statement::Function(func) => {
                let mut changed = false;
                for s in &mut func.body.statements {
                    changed |= self.optimize_loops_in_statement(s);
                }
                return changed;
            }
            _ => {}
        }
        false
    }
}

// =============================================================================
// O2: String Concatenation Optimization Pass
// =============================================================================

/// String concatenation optimization pass
/// Converts multiple concatenations to table.concat for 3+ parts
pub struct StringConcatOptimizationPass;

impl OptimizationPass for StringConcatOptimizationPass {
    fn name(&self) -> &'static str {
        "string-concat-optimization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        // Analyze string concatenation patterns
        // When 3+ parts are concatenated, using table.concat is more efficient
        // This is analysis-only - codegen handles the transformation

        for stmt in &program.statements {
            self.analyze_concat_in_statement(stmt);
        }

        Ok(false)
    }
}

impl StringConcatOptimizationPass {
    fn analyze_concat_in_statement(&self, stmt: &Statement) {
        match stmt {
            Statement::Variable(decl) => {
                self.count_concat_parts(&decl.initializer);
            }
            Statement::Expression(expr) => {
                self.count_concat_parts(expr);
            }
            Statement::Function(func) => {
                for s in &func.body.statements {
                    self.analyze_concat_in_statement(s);
                }
            }
            Statement::Return(ret) => {
                for expr in &ret.values {
                    self.count_concat_parts(expr);
                }
            }
            _ => {}
        }
    }

    fn count_concat_parts(&self, expr: &Expression) -> usize {
        if let ExpressionKind::Binary(BinaryOp::Concatenate, left, right) = &expr.kind {
            self.count_concat_parts(left) + self.count_concat_parts(right)
        } else {
            1
        }
    }
}

// =============================================================================
// O2: Dead Store Elimination Pass
// =============================================================================

/// Dead store elimination pass
/// Removes assignments to variables that are never read
pub struct DeadStoreEliminationPass;

impl OptimizationPass for DeadStoreEliminationPass {
    fn name(&self) -> &'static str {
        "dead-store-elimination"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        // Analyze variable usage to find dead stores
        // A store is dead if the variable is reassigned before being read
        let mut _variable_reads: std::collections::HashSet<crate::string_interner::StringId> =
            std::collections::HashSet::new();
        let mut _variable_writes: Vec<(crate::string_interner::StringId, usize)> = Vec::new();

        for (idx, stmt) in program.statements.iter().enumerate() {
            self.collect_variable_usage(stmt, idx, &mut _variable_reads, &mut _variable_writes);
        }

        // Currently analysis-only - actual elimination requires careful ordering
        Ok(false)
    }
}

impl DeadStoreEliminationPass {
    fn collect_variable_usage(
        &self,
        stmt: &Statement,
        _idx: usize,
        reads: &mut std::collections::HashSet<crate::string_interner::StringId>,
        _writes: &mut Vec<(crate::string_interner::StringId, usize)>,
    ) {
        match stmt {
            Statement::Variable(decl) => {
                self.collect_reads_from_expr(&decl.initializer, reads);
            }
            Statement::Expression(expr) => {
                self.collect_reads_from_expr(expr, reads);
            }
            Statement::Return(ret) => {
                for expr in &ret.values {
                    self.collect_reads_from_expr(expr, reads);
                }
            }
            _ => {}
        }
    }

    fn collect_reads_from_expr(
        &self,
        expr: &Expression,
        reads: &mut std::collections::HashSet<crate::string_interner::StringId>,
    ) {
        match &expr.kind {
            ExpressionKind::Identifier(name) => {
                reads.insert(*name);
            }
            ExpressionKind::Binary(_, left, right) => {
                self.collect_reads_from_expr(left, reads);
                self.collect_reads_from_expr(right, reads);
            }
            ExpressionKind::Unary(_, operand) => {
                self.collect_reads_from_expr(operand, reads);
            }
            ExpressionKind::Call(func, args) => {
                self.collect_reads_from_expr(func, reads);
                for arg in args {
                    self.collect_reads_from_expr(&arg.value, reads);
                }
            }
            _ => {}
        }
    }
}

// =============================================================================
// O2: Tail Call Optimization Pass
// =============================================================================

/// Tail call optimization pass
/// Identifies and marks tail calls for Lua's TCO
pub struct TailCallOptimizationPass;

impl OptimizationPass for TailCallOptimizationPass {
    fn name(&self) -> &'static str {
        "tail-call-optimization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        // Analyze functions for tail call patterns
        // A tail call is: return func(args) where func result is returned directly

        for stmt in &program.statements {
            if let Statement::Function(func) = stmt {
                self.analyze_tail_calls(&func.body.statements);
            }
        }

        // Lua automatically handles tail calls - this is analysis only
        Ok(false)
    }
}

impl TailCallOptimizationPass {
    fn analyze_tail_calls(&self, stmts: &[Statement]) {
        if let Some(Statement::Return(ret)) = stmts.last() {
            // Check if return value is a single function call
            if ret.values.len() == 1 {
                if let ExpressionKind::Call(_, _) = &ret.values[0].kind {
                    // This is a tail call position - Lua handles it automatically
                }
            }
        }
    }
}

// =============================================================================
// O3: Aggressive Inlining Pass
// =============================================================================

/// Aggressive function inlining pass (threshold: 15 statements)
/// More aggressive inlining for O3 optimization
pub struct AggressiveInliningPass {
    threshold: usize,
}

impl Default for AggressiveInliningPass {
    fn default() -> Self {
        Self { threshold: 15 }
    }
}

impl OptimizationPass for AggressiveInliningPass {
    fn name(&self) -> &'static str {
        "aggressive-inlining"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        // Similar to FunctionInliningPass but with higher threshold
        let mut inlinable_count = 0;

        for stmt in &program.statements {
            if let Statement::Function(func) = stmt {
                if func.body.statements.len() <= self.threshold {
                    inlinable_count += 1;
                }
            }
        }

        let _ = inlinable_count;
        Ok(false)
    }
}

// =============================================================================
// O3: Operator Inlining Pass
// =============================================================================

/// Operator inlining pass
/// Inlines operator overload implementations when types are known
pub struct OperatorInliningPass;

impl OptimizationPass for OperatorInliningPass {
    fn name(&self) -> &'static str {
        "operator-inlining"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        // Collect classes that might have operator overloads
        // Note: Operator overloads are implemented as methods in TypedLua
        let mut _class_count = 0;

        for stmt in &program.statements {
            if let Statement::Class(class) = stmt {
                // Count classes for potential operator analysis
                _class_count += 1;
                // Methods could potentially be operators (based on naming convention)
                let _method_count = class
                    .members
                    .iter()
                    .filter(|m| matches!(m, crate::ast::statement::ClassMember::Method(_)))
                    .count();
            }
        }

        // Analysis only - actual inlining requires type information
        Ok(false)
    }
}

// =============================================================================
// O3: Interface Method Inlining Pass
// =============================================================================

/// Interface method inlining pass
/// Inlines interface method implementations when type is known
pub struct InterfaceMethodInliningPass;

impl OptimizationPass for InterfaceMethodInliningPass {
    fn name(&self) -> &'static str {
        "interface-method-inlining"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        // Collect interface methods
        let mut _interface_method_count = 0;

        for stmt in &program.statements {
            if let Statement::Interface(iface) = stmt {
                // Count methods in interfaces for potential inlining analysis
                _interface_method_count += iface
                    .members
                    .iter()
                    .filter(|m| matches!(m, crate::ast::statement::InterfaceMember::Method(_)))
                    .count();
            }
        }

        // Analysis only
        Ok(false)
    }
}

// =============================================================================
// O3: Devirtualization Pass
// =============================================================================

/// Devirtualization pass
/// Converts virtual method calls to direct calls when type is known
pub struct DevirtualizationPass;

impl OptimizationPass for DevirtualizationPass {
    fn name(&self) -> &'static str {
        "devirtualization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        // Collect class methods for potential devirtualization
        let mut _class_methods: std::collections::HashMap<
            crate::string_interner::StringId,
            Vec<crate::string_interner::StringId>,
        > = std::collections::HashMap::new();

        for stmt in &program.statements {
            if let Statement::Class(class) = stmt {
                let class_name = class.name.node;
                let mut methods = Vec::new();

                for member in &class.members {
                    if let crate::ast::statement::ClassMember::Method(method) = member {
                        methods.push(method.name.node);
                    }
                }

                _class_methods.insert(class_name, methods);
            }
        }

        // Analysis only - requires type flow information
        Ok(false)
    }
}

// =============================================================================
// O3: Generic Specialization Pass
// =============================================================================

/// Generic specialization pass
/// Creates specialized versions of generic functions for known types
pub struct GenericSpecializationPass;

impl OptimizationPass for GenericSpecializationPass {
    fn name(&self) -> &'static str {
        "generic-specialization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        // Collect generic functions and their instantiation sites
        let mut _generic_functions: Vec<crate::string_interner::StringId> = Vec::new();

        for stmt in &program.statements {
            if let Statement::Function(func) = stmt {
                if func.type_parameters.is_some() {
                    _generic_functions.push(func.name.node);
                }
            }
        }

        // Analysis only - actual specialization is complex
        Ok(false)
    }
}
