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
    /// Collect all declared local names in a block (variables and functions)
    fn collect_declared_locals(
        &self,
        block: &Block,
        locals: &mut HashSet<crate::string_interner::StringId>,
    ) {
        for stmt in &block.statements {
            match stmt {
                Statement::Variable(decl) => {
                    // Extract name(s) from pattern
                    self.collect_pattern_names(&decl.pattern, locals);
                }
                Statement::Function(func) => {
                    // Function name is a local declaration
                    locals.insert(func.name.node);
                }
                _ => {}
            }
        }
    }

    /// Extract all identifier names from a pattern
    fn collect_pattern_names(
        &self,
        pattern: &Pattern,
        locals: &mut HashSet<crate::string_interner::StringId>,
    ) {
        match pattern {
            Pattern::Identifier(ident) => {
                locals.insert(ident.node);
            }
            Pattern::Array(arr) => {
                for elem in &arr.elements {
                    match elem {
                        crate::ast::pattern::ArrayPatternElement::Pattern(p) => {
                            self.collect_pattern_names(p, locals);
                        }
                        crate::ast::pattern::ArrayPatternElement::Rest(ident) => {
                            locals.insert(ident.node);
                        }
                        crate::ast::pattern::ArrayPatternElement::Hole => {}
                    }
                }
            }
            Pattern::Object(obj) => {
                for prop in &obj.properties {
                    if let Some(value_pattern) = &prop.value {
                        self.collect_pattern_names(value_pattern, locals);
                    } else {
                        // If no value pattern, the key itself is the binding
                        locals.insert(prop.key.node);
                    }
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_, _) => {}
        }
    }

    /// Check if a name looks like it was created by this pass (starts with underscore)
    fn is_localized_name(&self, name: crate::string_interner::StringId) -> bool {
        let resolved = self.interner.resolve(name);
        resolved.starts_with('_')
    }

    fn localize_in_block(
        &self,
        block: &mut Block,
        mut declared_locals: HashSet<crate::string_interner::StringId>,
    ) -> bool {
        let mut changed = false;

        // First, collect all locally declared names (variables and functions)
        self.collect_declared_locals(block, &mut declared_locals);

        let mut global_usage: HashMap<crate::string_interner::StringId, usize> = HashMap::new();

        for stmt in &block.statements {
            self.collect_global_usage_optimized(stmt, &mut global_usage, &declared_locals);
        }

        let frequently_used: Vec<_> = global_usage
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .filter(|(name, _)| !declared_locals.contains(name))
            // Skip names that already start with underscore to prevent cascading localization
            .filter(|(name, _)| !self.is_localized_name(*name))
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
                // Function parameters are locals within the function body
                let mut func_locals = declared_locals.clone();
                for param in &func.parameters {
                    self.collect_pattern_names(&param.pattern, &mut func_locals);
                }
                for s in &func.body.statements {
                    self.collect_global_usage_optimized(s, usage, &func_locals);
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

use crate::ast::expression::ArrowBody;
use crate::ast::statement::{FunctionDeclaration, Parameter, ReturnStatement};
use crate::string_interner::StringId;

enum InlineResult {
    /// Direct expression substitution - for simple single-return functions
    /// The expression can be directly substituted for the call
    Direct(Expression),
    /// Complex inlining - contains statements to insert and the result variable
    Replaced {
        stmts: Vec<Statement>,
        result_var: StringId,
    },
}

/// Function inlining optimization pass (threshold: 5 statements)
/// Inlines small functions at call sites
pub struct FunctionInliningPass {
    threshold: usize,
    next_temp_id: usize,
    functions: HashMap<StringId, FunctionDeclaration>,
    interner: Option<Arc<StringInterner>>,
}

impl Default for FunctionInliningPass {
    fn default() -> Self {
        Self {
            threshold: 5,
            next_temp_id: 0,
            functions: HashMap::new(),
            interner: None,
        }
    }
}

impl FunctionInliningPass {
    pub fn set_interner(&mut self, interner: Arc<StringInterner>) {
        self.interner = Some(interner);
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
        self.next_temp_id = 0;
        self.functions.clear();

        self.collect_functions(program);

        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.inline_in_statement(stmt);
        }

        Ok(changed)
    }
}

impl FunctionInliningPass {
    fn collect_functions(&mut self, program: &Program) {
        for stmt in &program.statements {
            self.collect_functions_in_stmt(stmt);
        }
    }

    fn collect_functions_in_stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Function(func) => {
                self.functions.insert(func.name.node, func.clone());
                for s in &func.body.statements {
                    self.collect_functions_in_stmt(s);
                }
            }
            Statement::If(if_stmt) => {
                for s in &if_stmt.then_block.statements {
                    self.collect_functions_in_stmt(s);
                }
                for else_if in &if_stmt.else_ifs {
                    for s in &else_if.block.statements {
                        self.collect_functions_in_stmt(s);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for s in &else_block.statements {
                        self.collect_functions_in_stmt(s);
                    }
                }
            }
            Statement::While(while_stmt) => {
                for s in &while_stmt.body.statements {
                    self.collect_functions_in_stmt(s);
                }
            }
            Statement::For(for_stmt) => {
                let body = match &**for_stmt {
                    ForStatement::Numeric(for_num) => &for_num.body,
                    ForStatement::Generic(for_gen) => &for_gen.body,
                };
                for s in &body.statements {
                    self.collect_functions_in_stmt(s);
                }
            }
            _ => {}
        }
    }
}

impl FunctionInliningPass {
    fn inline_in_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::Function(func) => {
                let mut changed = false;
                for s in &mut func.body.statements {
                    changed |= self.inline_in_statement(s);
                }
                changed
            }
            Statement::If(if_stmt) => {
                let mut changed = self.inline_in_expression(&mut if_stmt.condition);
                changed |= self.inline_in_block(&mut if_stmt.then_block);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.inline_in_expression(&mut else_if.condition);
                    changed |= self.inline_in_block(&mut else_if.block);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.inline_in_block(else_block);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.inline_in_expression(&mut while_stmt.condition);
                changed |= self.inline_in_block(&mut while_stmt.body);
                changed
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Numeric(for_num) => {
                    let mut changed = self.inline_in_expression(&mut for_num.start);
                    changed |= self.inline_in_expression(&mut for_num.end);
                    if let Some(step) = &mut for_num.step {
                        changed |= self.inline_in_expression(step);
                    }
                    changed |= self.inline_in_block(&mut for_num.body);
                    changed
                }
                ForStatement::Generic(for_gen) => {
                    let mut changed = false;
                    for expr in &mut for_gen.iterators {
                        changed |= self.inline_in_expression(expr);
                    }
                    changed |= self.inline_in_block(&mut for_gen.body);
                    changed
                }
            },
            Statement::Variable(decl) => {
                if let Some(result) = self.try_inline_call(&mut decl.initializer) {
                    match result {
                        InlineResult::Direct(_) => {
                            // Expression was directly substituted - no extra statements needed
                            true
                        }
                        InlineResult::Replaced { stmts, .. } => {
                            // decl.initializer has been modified to reference the result variable
                            let span = decl.span;
                            let var_stmt = Statement::Variable(decl.clone());
                            *stmt = Statement::Block(Block {
                                statements: {
                                    let mut new_stmts = stmts;
                                    new_stmts.push(var_stmt);
                                    new_stmts
                                },
                                span,
                            });
                            true
                        }
                    }
                } else {
                    false
                }
            }
            Statement::Expression(expr) => {
                if let Some(result) = self.try_inline_call(expr) {
                    match result {
                        InlineResult::Direct(_) => {
                            // Expression was directly substituted - no extra statements needed
                            true
                        }
                        InlineResult::Replaced { stmts, .. } => {
                            let span = expr.span;
                            *stmt = Statement::Block(Block {
                                statements: stmts,
                                span,
                            });
                            true
                        }
                    }
                } else {
                    false
                }
            }
            Statement::Return(ret) => {
                let mut changed = false;
                let ret_span = ret.span;

                for expr in ret.values.iter_mut() {
                    if let Some(result) = self.try_inline_call(expr) {
                        match result {
                            InlineResult::Direct(_) => {
                                // Expression was directly substituted - no extra statements needed
                                changed = true;
                            }
                            InlineResult::Replaced { stmts, .. } => {
                                // expr has been modified to reference the result variable
                                let span = ret_span;
                                let new_ret = ReturnStatement {
                                    values: ret.values.clone(),
                                    span,
                                };
                                *stmt = Statement::Block(Block {
                                    statements: {
                                        let mut new_stmts = stmts;
                                        new_stmts.push(Statement::Return(new_ret));
                                        new_stmts
                                    },
                                    span,
                                });
                                changed = true;
                                break;
                            }
                        }
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn inline_in_block(&mut self, block: &mut Block) -> bool {
        let mut changed = false;
        let mut i = 0;
        while i < block.statements.len() {
            changed |= self.inline_in_statement(&mut block.statements[i]);
            i += 1;
        }
        changed
    }

    fn inline_in_expression(&mut self, expr: &mut Expression) -> bool {
        match &mut expr.kind {
            ExpressionKind::Call(func, args) => {
                let mut changed = self.inline_in_expression(func);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                // Also try to inline this call itself (for nested inlining)
                if let Some(result) = self.try_inline_call(expr) {
                    if let InlineResult::Direct(_) = result {
                        changed = true;
                    }
                    // Note: For Replaced, we can't easily insert statements here,
                    // so we rely on the fixed-point iteration to catch it in the next pass
                }
                changed
            }
            ExpressionKind::MethodCall(obj, _, args) => {
                let mut changed = self.inline_in_expression(obj);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Binary(_op, left, right) => {
                let mut changed = self.inline_in_expression(left);
                changed |= self.inline_in_expression(right);
                changed
            }
            ExpressionKind::Unary(_op, operand) => self.inline_in_expression(operand),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut changed = self.inline_in_expression(cond);
                changed |= self.inline_in_expression(then_expr);
                changed |= self.inline_in_expression(else_expr);
                changed
            }
            ExpressionKind::Pipe(left, right) => {
                let mut changed = self.inline_in_expression(left);
                changed |= self.inline_in_expression(right);
                changed
            }
            ExpressionKind::Match(match_expr) => {
                let mut changed = self.inline_in_expression(&mut match_expr.value);
                for arm in &mut match_expr.arms {
                    match &mut arm.body {
                        MatchArmBody::Expression(expr) => {
                            changed |= self.inline_in_expression(expr);
                        }
                        MatchArmBody::Block(block) => {
                            changed |= self.inline_in_block(block);
                        }
                    }
                }
                changed
            }
            ExpressionKind::Arrow(arrow) => {
                let mut changed = false;
                for param in &mut arrow.parameters {
                    if let Some(default) = &mut param.default {
                        changed |= self.inline_in_expression(default);
                    }
                }
                match &mut arrow.body {
                    ArrowBody::Expression(expr) => {
                        changed |= self.inline_in_expression(expr);
                    }
                    ArrowBody::Block(block) => {
                        changed |= self.inline_in_block(block);
                    }
                }
                changed
            }
            ExpressionKind::New(callee, args) => {
                let mut changed = self.inline_in_expression(callee);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Try(try_expr) => {
                let mut changed = self.inline_in_expression(&mut try_expr.expression);
                changed |= self.inline_in_expression(&mut try_expr.catch_expression);
                changed
            }
            ExpressionKind::ErrorChain(left, right) => {
                let mut changed = self.inline_in_expression(left);
                changed |= self.inline_in_expression(right);
                changed
            }
            ExpressionKind::OptionalMember(obj, _) => self.inline_in_expression(obj),
            ExpressionKind::OptionalIndex(obj, index) => {
                let mut changed = self.inline_in_expression(obj);
                changed |= self.inline_in_expression(index);
                changed
            }
            ExpressionKind::OptionalCall(obj, args) => {
                let mut changed = self.inline_in_expression(obj);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::OptionalMethodCall(obj, _, args) => {
                let mut changed = self.inline_in_expression(obj);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::TypeAssertion(expr, _) => self.inline_in_expression(expr),
            ExpressionKind::Member(obj, _) => self.inline_in_expression(obj),
            ExpressionKind::Index(obj, index) => {
                let mut changed = self.inline_in_expression(obj);
                changed |= self.inline_in_expression(index);
                changed
            }
            _ => false,
        }
    }

    fn try_inline_call(&mut self, expr: &mut Expression) -> Option<InlineResult> {
        if let ExpressionKind::Call(func, args) = &expr.kind.clone() {
            if let ExpressionKind::Identifier(func_name) = &func.kind {
                if let Some(func_decl) = self.find_function_definition(expr, *func_name) {
                    if self.is_inlinable(&func_decl) {
                        let result = self.inline_call(func_decl.clone(), args);
                        // Replace the call expression based on the inline result
                        match &result {
                            InlineResult::Direct(inlined_expr) => {
                                // Direct substitution - replace call with the inlined expression
                                *expr = inlined_expr.clone();
                            }
                            InlineResult::Replaced { result_var, .. } => {
                                // Reference the result variable
                                expr.kind = ExpressionKind::Identifier(*result_var);
                            }
                        }
                        return Some(result);
                    }
                }
            }
        }
        None
    }

    fn find_function_definition(
        &self,
        _expr: &Expression,
        name: StringId,
    ) -> Option<&FunctionDeclaration> {
        self.functions.get(&name)
    }

    fn is_inlinable(&self, func: &FunctionDeclaration) -> bool {
        if func.body.statements.len() > self.threshold {
            return false;
        }
        if self.is_recursive(func) {
            return false;
        }
        if self.has_complex_control_flow(&func.body) {
            return false;
        }
        if self.has_closures(&func.body) {
            return false;
        }
        true
    }

    fn is_recursive(&self, func: &FunctionDeclaration) -> bool {
        let name = func.name.node;
        for stmt in &func.body.statements {
            if self.contains_call_to(stmt, name) {
                return true;
            }
        }
        false
    }

    fn contains_call_to(&self, stmt: &Statement, name: StringId) -> bool {
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
            Statement::While(while_stmt) => {
                self.expr_contains_call_to(&while_stmt.condition, name)
                    || while_stmt
                        .body
                        .statements
                        .iter()
                        .any(|s| self.contains_call_to(s, name))
            }
            Statement::For(for_stmt) => {
                let stmts = match &**for_stmt {
                    ForStatement::Numeric(for_num) => &for_num.body.statements,
                    ForStatement::Generic(for_gen) => &for_gen.body.statements,
                };
                stmts.iter().any(|s| self.contains_call_to(s, name))
            }
            _ => false,
        }
    }

    fn expr_contains_call_to(&self, expr: &Expression, name: StringId) -> bool {
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
            ExpressionKind::MethodCall(obj, method_name, args) => {
                if method_name.node == name {
                    return true;
                }
                self.expr_contains_call_to(obj, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::Binary(_, left, right) => {
                self.expr_contains_call_to(left, name) || self.expr_contains_call_to(right, name)
            }
            ExpressionKind::Unary(_, operand) => self.expr_contains_call_to(operand, name),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.expr_contains_call_to(cond, name)
                    || self.expr_contains_call_to(then_expr, name)
                    || self.expr_contains_call_to(else_expr, name)
            }
            ExpressionKind::Arrow(arrow) => {
                for param in &arrow.parameters {
                    if let Some(default) = &param.default {
                        if self.expr_contains_call_to(default, name) {
                            return true;
                        }
                    }
                }
                match &arrow.body {
                    ArrowBody::Expression(expr) => self.expr_contains_call_to(expr, name),
                    ArrowBody::Block(block) => block
                        .statements
                        .iter()
                        .any(|s| self.contains_call_to(s, name)),
                }
            }
            ExpressionKind::Match(match_expr) => {
                self.expr_contains_call_to(&match_expr.value, name)
                    || match_expr.arms.iter().any(|arm| match &arm.body {
                        MatchArmBody::Expression(expr) => self.expr_contains_call_to(expr, name),
                        MatchArmBody::Block(block) => block
                            .statements
                            .iter()
                            .any(|s| self.contains_call_to(s, name)),
                    })
            }
            ExpressionKind::New(callee, args) => {
                self.expr_contains_call_to(callee, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::Try(try_expr) => {
                self.expr_contains_call_to(&try_expr.expression, name)
                    || self.expr_contains_call_to(&try_expr.catch_expression, name)
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.expr_contains_call_to(left, name) || self.expr_contains_call_to(right, name)
            }
            ExpressionKind::OptionalMember(obj, _) => self.expr_contains_call_to(obj, name),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.expr_contains_call_to(obj, name) || self.expr_contains_call_to(index, name)
            }
            ExpressionKind::OptionalCall(obj, args) => {
                self.expr_contains_call_to(obj, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::OptionalMethodCall(obj, _, args) => {
                self.expr_contains_call_to(obj, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::TypeAssertion(expr, _) => self.expr_contains_call_to(expr, name),
            ExpressionKind::Member(obj, _) => self.expr_contains_call_to(obj, name),
            ExpressionKind::Index(obj, index) => {
                self.expr_contains_call_to(obj, name) || self.expr_contains_call_to(index, name)
            }
            _ => false,
        }
    }

    fn has_complex_control_flow(&self, body: &Block) -> bool {
        for stmt in &body.statements {
            if self.stmt_has_complex_flow(stmt) {
                return true;
            }
        }
        false
    }

    fn stmt_has_complex_flow(&self, stmt: &Statement) -> bool {
        match stmt {
            Statement::If(if_stmt) => {
                if self.block_has_multiple_returns(&if_stmt.then_block) {
                    return true;
                }
                for else_if in &if_stmt.else_ifs {
                    if self.block_has_multiple_returns(&else_if.block) {
                        return true;
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    if self.block_has_multiple_returns(else_block) {
                        return true;
                    }
                }
                false
            }
            Statement::While(_) | Statement::For(_) | Statement::Repeat(_) => true,
            _ => false,
        }
    }

    fn block_has_multiple_returns(&self, block: &Block) -> bool {
        let mut return_count = 0;
        for stmt in &block.statements {
            if matches!(stmt, Statement::Return(_)) {
                return_count += 1;
                if return_count > 1 {
                    return true;
                }
            }
        }
        false
    }

    fn has_closures(&self, body: &Block) -> bool {
        self.block_has_closures(body)
    }

    fn block_has_closures(&self, block: &Block) -> bool {
        for stmt in &block.statements {
            if self.stmt_has_closures(stmt) {
                return true;
            }
        }
        false
    }

    fn stmt_has_closures(&self, stmt: &Statement) -> bool {
        match stmt {
            Statement::Function(func) => self.block_has_closures(&func.body),
            Statement::Variable(decl) => self.expr_has_closures(&decl.initializer),
            Statement::Expression(expr) => self.expr_has_closures(expr),
            Statement::If(if_stmt) => {
                self.expr_has_closures(&if_stmt.condition)
                    || self.block_has_closures(&if_stmt.then_block)
                    || if_stmt.else_ifs.iter().any(|ei| {
                        self.expr_has_closures(&ei.condition) || self.block_has_closures(&ei.block)
                    })
                    || if_stmt
                        .else_block
                        .as_ref()
                        .map_or(false, |eb| self.block_has_closures(eb))
            }
            Statement::While(while_stmt) => {
                self.expr_has_closures(&while_stmt.condition)
                    || self.block_has_closures(&while_stmt.body)
            }
            Statement::For(for_stmt) => {
                let body = match &**for_stmt {
                    ForStatement::Numeric(for_num) => &for_num.body,
                    ForStatement::Generic(for_gen) => &for_gen.body,
                };
                self.block_has_closures(body)
            }
            Statement::Return(ret) => ret.values.iter().any(|e| self.expr_has_closures(e)),
            _ => false,
        }
    }

    fn expr_has_closures(&self, expr: &Expression) -> bool {
        match &expr.kind {
            ExpressionKind::Function(func) => self.block_has_closures(&func.body),
            ExpressionKind::Arrow(arrow) => match &arrow.body {
                ArrowBody::Expression(expr) => self.expr_has_closures(expr),
                ArrowBody::Block(block) => self.block_has_closures(block),
            },
            ExpressionKind::Call(func, args) => {
                self.expr_has_closures(func)
                    || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::MethodCall(obj, _, args) => {
                self.expr_has_closures(obj) || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::Binary(_, left, right) => {
                self.expr_has_closures(left) || self.expr_has_closures(right)
            }
            ExpressionKind::Unary(_, operand) => self.expr_has_closures(operand),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.expr_has_closures(cond)
                    || self.expr_has_closures(then_expr)
                    || self.expr_has_closures(else_expr)
            }
            ExpressionKind::Pipe(left, right) => {
                self.expr_has_closures(left) || self.expr_has_closures(right)
            }
            ExpressionKind::Match(match_expr) => {
                self.expr_has_closures(&match_expr.value)
                    || match_expr.arms.iter().any(|arm| match &arm.body {
                        MatchArmBody::Expression(expr) => self.expr_has_closures(expr),
                        MatchArmBody::Block(block) => self.block_has_closures(block),
                    })
            }
            ExpressionKind::New(callee, args) => {
                self.expr_has_closures(callee)
                    || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::Try(try_expr) => {
                self.expr_has_closures(&try_expr.expression)
                    || self.expr_has_closures(&try_expr.catch_expression)
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.expr_has_closures(left) || self.expr_has_closures(right)
            }
            ExpressionKind::OptionalMember(obj, _) => self.expr_has_closures(obj),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.expr_has_closures(obj) || self.expr_has_closures(index)
            }
            ExpressionKind::OptionalCall(obj, args) => {
                self.expr_has_closures(obj) || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::OptionalMethodCall(obj, _, args) => {
                self.expr_has_closures(obj) || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::TypeAssertion(expr, _) => self.expr_has_closures(expr),
            ExpressionKind::Member(obj, _) => self.expr_has_closures(obj),
            ExpressionKind::Index(obj, index) => {
                self.expr_has_closures(obj) || self.expr_has_closures(index)
            }
            _ => false,
        }
    }

    fn inline_call(&mut self, func: FunctionDeclaration, args: &[Argument]) -> InlineResult {
        let param_subst = self.create_parameter_substitution(&func.parameters, args);

        // Check for simple single-return function: just `return expr`
        if func.body.statements.len() == 1 {
            if let Statement::Return(ret) = &func.body.statements[0] {
                if ret.values.len() == 1 {
                    // Simple case: directly substitute the return expression
                    let mut inlined_expr = ret.values[0].clone();
                    self.inline_expression(&mut inlined_expr, &param_subst);
                    return InlineResult::Direct(inlined_expr);
                }
            }
        }

        // Complex case: create intermediate variable
        let mut inlined_body = Vec::new();
        let return_var = self.create_temp_variable();
        let has_return = self.has_return_value(&func.body);

        for stmt in &func.body.statements.clone() {
            let inlined_stmt = self.inline_statement(stmt, &param_subst, &return_var, has_return);
            inlined_body.push(inlined_stmt);
        }

        InlineResult::Replaced {
            stmts: inlined_body,
            result_var: return_var,
        }
    }

    fn create_parameter_substitution(
        &self,
        parameters: &[Parameter],
        args: &[Argument],
    ) -> HashMap<StringId, Expression> {
        let mut subst = HashMap::new();
        for (param, arg) in parameters.iter().zip(args.iter()) {
            if let Pattern::Identifier(ident) = &param.pattern {
                subst.insert(ident.node, arg.value.clone());
            }
        }
        subst
    }

    fn create_temp_variable(&mut self) -> StringId {
        let name = format!("_inline_result_{}", self.next_temp_id);
        self.next_temp_id += 1;
        debug_assert!(
            self.interner.is_some(),
            "String interner not set for FunctionInliningPass"
        );
        match &self.interner {
            Some(interner) => interner.get_or_intern(&name),
            None => unsafe { std::hint::unreachable_unchecked() },
        }
    }

    fn has_return_value(&self, body: &Block) -> bool {
        for stmt in &body.statements {
            if let Statement::Return(ret) = stmt {
                if !ret.values.is_empty() {
                    return true;
                }
            }
        }
        false
    }

    fn inline_statement(
        &self,
        stmt: &Statement,
        param_subst: &HashMap<StringId, Expression>,
        return_var: &StringId,
        has_return: bool,
    ) -> Statement {
        match stmt {
            Statement::Variable(decl) => {
                let mut new_decl = decl.clone();
                self.inline_expression(&mut new_decl.initializer, param_subst);
                Statement::Variable(new_decl)
            }
            Statement::Expression(expr) => {
                let mut new_expr = expr.clone();
                self.inline_expression(&mut new_expr, param_subst);
                Statement::Expression(new_expr)
            }
            Statement::Return(ret) => {
                if !ret.values.is_empty() && has_return {
                    let values: Vec<Expression> = ret
                        .values
                        .iter()
                        .map(|v| {
                            let val = v.clone();
                            let mut substituted = val.clone();
                            self.inline_expression(&mut substituted, param_subst);
                            substituted
                        })
                        .collect();
                    Statement::Variable(VariableDeclaration {
                        kind: VariableKind::Local,
                        pattern: Pattern::Identifier(crate::ast::Spanned::new(
                            *return_var,
                            Span::dummy(),
                        )),
                        type_annotation: None,
                        initializer: Expression::new(
                            if values.len() == 1 {
                                values[0].kind.clone()
                            } else {
                                ExpressionKind::Array(
                                    values
                                        .iter()
                                        .map(|e| ArrayElement::Expression(e.clone()))
                                        .collect(),
                                )
                            },
                            Span::dummy(),
                        ),
                        span: Span::dummy(),
                    })
                } else {
                    Statement::Return(ret.clone())
                }
            }
            _ => stmt.clone(),
        }
    }

    fn inline_expression(
        &self,
        expr: &mut Expression,
        param_subst: &HashMap<StringId, Expression>,
    ) {
        match &mut expr.kind {
            ExpressionKind::Identifier(id) => {
                if let Some(substituted) = param_subst.get(id) {
                    expr.kind = substituted.kind.clone();
                }
            }
            ExpressionKind::Call(func, args) => {
                self.inline_expression(func, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::MethodCall(obj, _, args) => {
                self.inline_expression(obj, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::Binary(_op, left, right) => {
                self.inline_expression(left, param_subst);
                self.inline_expression(right, param_subst);
            }
            ExpressionKind::Unary(_op, operand) => {
                self.inline_expression(operand, param_subst);
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.inline_expression(cond, param_subst);
                self.inline_expression(then_expr, param_subst);
                self.inline_expression(else_expr, param_subst);
            }
            ExpressionKind::Arrow(arrow) => {
                for param in &mut arrow.parameters {
                    if let Some(default) = &mut param.default {
                        self.inline_expression(default, param_subst);
                    }
                }
                match &mut arrow.body {
                    ArrowBody::Expression(expr) => self.inline_expression(expr, param_subst),
                    ArrowBody::Block(_) => {}
                }
            }
            ExpressionKind::Match(match_expr) => {
                self.inline_expression(&mut match_expr.value, param_subst);
                for arm in &mut match_expr.arms {
                    match &mut arm.body {
                        MatchArmBody::Expression(expr) => self.inline_expression(expr, param_subst),
                        MatchArmBody::Block(_) => {}
                    }
                }
            }
            ExpressionKind::New(callee, args) => {
                self.inline_expression(callee, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.inline_expression(&mut try_expr.expression, param_subst);
                self.inline_expression(&mut try_expr.catch_expression, param_subst);
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.inline_expression(left, param_subst);
                self.inline_expression(right, param_subst);
            }
            ExpressionKind::OptionalMember(obj, _) => self.inline_expression(obj, param_subst),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.inline_expression(obj, param_subst);
                self.inline_expression(index, param_subst);
            }
            ExpressionKind::OptionalCall(obj, args) => {
                self.inline_expression(obj, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args) => {
                self.inline_expression(obj, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::TypeAssertion(expr, _) => self.inline_expression(expr, param_subst),
            ExpressionKind::Member(obj, _) => self.inline_expression(obj, param_subst),
            ExpressionKind::Index(obj, index) => {
                self.inline_expression(obj, param_subst);
                self.inline_expression(index, param_subst);
            }
            _ => {}
        }
    }
}

use crate::ast::expression::Argument;
use crate::ast::expression::ArrayElement;
use crate::ast::expression::MatchArmBody;

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
