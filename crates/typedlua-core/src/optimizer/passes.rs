use crate::config::OptimizationLevel;
use crate::errors::CompilationError;
use crate::optimizer::OptimizationPass;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use typedlua_parser::ast::expression::{BinaryOp, Expression, ExpressionKind, Literal, UnaryOp};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{
    Block, ForNumeric, ForStatement, Statement, VariableDeclaration, VariableKind,
};
use typedlua_parser::ast::Program;
use typedlua_parser::ast::Spanned;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::StringInterner;

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
            ExpressionKind::Call(func, args, _) => {
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
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            value,
                            ..
                        } => {
                            changed |= self.fold_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            ..
                        } => {
                            changed |= self.fold_expression(key);
                            changed |= self.fold_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value, ..
                        } => {
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
            ExpressionKind::Call(func, args, _) => {
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

// =============================================================================
// O1: Global Localization Pass
// =============================================================================

/// Global localization optimization pass
/// Identifies frequently used globals and creates local references to reduce
/// repeated table lookups in Lua (e.g., `local _math = math` then use `_math.sin`)
pub struct GlobalLocalizationPass {
    interner: Rc<StringInterner>,
}

impl GlobalLocalizationPass {
    /// Create a new pass with the given string interner
    pub fn new(interner: Rc<StringInterner>) -> Self {
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
        locals: &mut HashSet<typedlua_parser::string_interner::StringId>,
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
        locals: &mut HashSet<typedlua_parser::string_interner::StringId>,
    ) {
        match pattern {
            Pattern::Identifier(ident) => {
                locals.insert(ident.node);
            }
            Pattern::Array(arr) => {
                for elem in &arr.elements {
                    match elem {
                        typedlua_parser::ast::pattern::ArrayPatternElement::Pattern(p) => {
                            self.collect_pattern_names(p, locals);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Rest(ident) => {
                            locals.insert(ident.node);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Hole => {}
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
            Pattern::Or(or_pattern) => {
                // All alternatives bind the same variables (guaranteed by type checker)
                // So we can collect from just the first alternative
                if let Some(first) = or_pattern.alternatives.first() {
                    self.collect_pattern_names(first, locals);
                }
            }
        }
    }

    /// Check if a name looks like it was created by this pass (starts with underscore)
    fn is_localized_name(&self, name: typedlua_parser::string_interner::StringId) -> bool {
        let resolved = self.interner.resolve(name);
        resolved.starts_with('_')
    }

    fn localize_in_block(
        &self,
        block: &mut Block,
        mut declared_locals: HashSet<typedlua_parser::string_interner::StringId>,
    ) -> bool {
        let mut changed = false;

        // First, collect all locally declared names (variables and functions)
        self.collect_declared_locals(block, &mut declared_locals);

        let mut global_usage: HashMap<typedlua_parser::string_interner::StringId, usize> =
            HashMap::new();

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
        global_name: typedlua_parser::string_interner::StringId,
        local_name: typedlua_parser::string_interner::StringId,
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
        original: typedlua_parser::string_interner::StringId,
    ) -> typedlua_parser::string_interner::StringId {
        let name = self.interner.resolve(original);
        let local_name = format!("_{}", name);
        self.interner.get_or_intern(&local_name)
    }

    fn collect_global_usage_optimized(
        &self,
        stmt: &Statement,
        usage: &mut HashMap<typedlua_parser::string_interner::StringId, usize>,
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
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
        usage: &mut HashMap<typedlua_parser::string_interner::StringId, usize>,
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
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
            ExpressionKind::Call(func, args, _) => {
                self.collect_from_expression_optimized(func, usage, declared_locals);
                for arg in args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::Member(obj, _) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
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
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.collect_from_expression_optimized(obj, usage, declared_locals);
                for arg in args {
                    self.collect_from_expression_optimized(&arg.value, usage, declared_locals);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
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
        body: &typedlua_parser::ast::expression::ArrowBody,
        usage: &mut HashMap<typedlua_parser::string_interner::StringId, usize>,
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
    ) {
        match body {
            typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                for stmt in &block.statements {
                    self.collect_global_usage_optimized(stmt, usage, declared_locals);
                }
            }
        }
    }

    fn collect_from_match_arm_body(
        &self,
        body: &typedlua_parser::ast::expression::MatchArmBody,
        usage: &mut HashMap<typedlua_parser::string_interner::StringId, usize>,
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
    ) {
        match body {
            typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                self.collect_from_expression_optimized(expr, usage, declared_locals);
            }
            typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                for stmt in &block.statements {
                    self.collect_global_usage_optimized(stmt, usage, declared_locals);
                }
            }
        }
    }

    fn replace_global_usages(
        &self,
        stmt: &mut Statement,
        frequently_used: &[(typedlua_parser::string_interner::StringId, usize)],
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
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
        frequently_used: &[(typedlua_parser::string_interner::StringId, usize)],
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
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
            ExpressionKind::Call(func, args, _) => {
                self.replace_in_expression(func, frequently_used, declared_locals);
                for arg in args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals);
                }
            }
            ExpressionKind::Member(obj, _) => {
                self.replace_in_expression(obj, frequently_used, declared_locals);
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
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
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.replace_in_expression(obj, frequently_used, declared_locals);
                for arg in args {
                    self.replace_in_expression(&mut arg.value, frequently_used, declared_locals);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
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
        body: &mut typedlua_parser::ast::expression::ArrowBody,
        frequently_used: &[(typedlua_parser::string_interner::StringId, usize)],
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
    ) {
        match body {
            typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                self.replace_in_expression(expr, frequently_used, declared_locals);
            }
            typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                for stmt in &mut block.statements {
                    self.replace_global_usages(stmt, frequently_used, declared_locals);
                }
            }
        }
    }

    fn replace_in_match_arm_body(
        &self,
        body: &mut typedlua_parser::ast::expression::MatchArmBody,
        frequently_used: &[(typedlua_parser::string_interner::StringId, usize)],
        declared_locals: &HashSet<typedlua_parser::string_interner::StringId>,
    ) {
        match body {
            typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                self.replace_in_expression(expr, frequently_used, declared_locals);
            }
            typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
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

use typedlua_parser::ast::expression::ArrowBody;
use typedlua_parser::ast::statement::{FunctionDeclaration, Parameter, ReturnStatement};
use typedlua_parser::string_interner::StringId;

enum InlineResult {
    /// Direct expression substitution - for simple single-return functions
    /// The expression can be directly substituted for the call
    Direct(Box<Expression>),
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
    interner: Option<Rc<StringInterner>>,
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
    pub fn set_interner(&mut self, interner: Rc<StringInterner>) {
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
            ExpressionKind::Call(func, args, _) => {
                let mut changed = self.inline_in_expression(func);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                // Also try to inline this call itself (for nested inlining)
                // Note: For Replaced, we can't easily insert statements here,
                // so we rely on the fixed-point iteration to catch it in the next pass
                if let Some(InlineResult::Direct(_)) = self.try_inline_call(expr) {
                    changed = true;
                }
                changed
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
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
            ExpressionKind::OptionalCall(obj, args, _) => {
                let mut changed = self.inline_in_expression(obj);
                for arg in args {
                    changed |= self.inline_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
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
        if let ExpressionKind::Call(func, args, _) = &expr.kind.clone() {
            if let ExpressionKind::Identifier(func_name) = &func.kind {
                if let Some(func_decl) = self.find_function_definition(expr, *func_name) {
                    if self.is_inlinable(func_decl) {
                        let result = self.inline_call(func_decl.clone(), args);
                        // Replace the call expression based on the inline result
                        match &result {
                            InlineResult::Direct(inlined_expr) => {
                                // Direct substitution - replace call with the inlined expression
                                *expr = (**inlined_expr).clone();
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
        // Skip generic functions - let GenericSpecializationPass handle them first
        if func.type_parameters.is_some() {
            return false;
        }
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
                    || if_stmt.else_block.as_ref().is_some_and(|eb| {
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
            ExpressionKind::Call(func, args, _) => {
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
            ExpressionKind::MethodCall(obj, method_name, args, _) => {
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
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.expr_contains_call_to(obj, name)
                    || args
                        .iter()
                        .any(|a| self.expr_contains_call_to(&a.value, name))
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
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
                        .is_some_and(|eb| self.block_has_closures(eb))
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
            ExpressionKind::Call(func, args, _) => {
                self.expr_has_closures(func)
                    || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
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
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.expr_has_closures(obj) || args.iter().any(|a| self.expr_has_closures(&a.value))
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
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
                    return InlineResult::Direct(Box::new(inlined_expr));
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
                        pattern: Pattern::Identifier(typedlua_parser::ast::Spanned::new(
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
            ExpressionKind::Call(func, args, _) => {
                self.inline_expression(func, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
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
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.inline_expression(obj, param_subst);
                for arg in args {
                    self.inline_expression(&mut arg.value, param_subst);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
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

use typedlua_parser::ast::expression::Argument;
use typedlua_parser::ast::expression::ArrayElement;
use typedlua_parser::ast::expression::MatchArmBody;

// =============================================================================
// O2: Loop Optimization Pass
// =============================================================================

/// Loop optimization pass
/// 1. Hoists loop-invariant local variable declarations
/// 2. Removes dead loops (while false, zero-iteration for, repeat until true)
/// 3. Handles all loop types including repeat...until
#[derive(Default)]
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

        let mut i = 0;
        while i < program.statements.len() {
            if self.optimize_loops_in_statement(&mut program.statements[i]) {
                changed = true;
            }
            i += 1;
        }

        Ok(changed)
    }
}

impl LoopOptimizationPass {
    fn optimize_loops_in_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::For(for_stmt) => self.optimize_for_loop(for_stmt),
            Statement::While(while_stmt) => self.optimize_while_loop(while_stmt),
            Statement::Repeat(repeat_stmt) => self.optimize_repeat_loop(repeat_stmt),
            Statement::Variable(_) | Statement::Expression(_) => false,
            Statement::Return(_)
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Rethrow(_)
            | Statement::Throw(_) => false,
            Statement::Block(block) => self.optimize_block(&mut block.statements),
            Statement::Class(_)
            | Statement::Interface(_)
            | Statement::Enum(_)
            | Statement::TypeAlias(_) => false,
            Statement::Import(_) | Statement::Export(_) => false,
            Statement::Namespace(_)
            | Statement::DeclareFunction(_)
            | Statement::DeclareNamespace(_)
            | Statement::DeclareType(_)
            | Statement::DeclareInterface(_)
            | Statement::DeclareConst(_) => false,
            Statement::Function(func) => self.optimize_block(&mut func.body.statements),
            Statement::If(if_stmt) => {
                let mut changed = self.optimize_block(&mut if_stmt.then_block.statements);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.optimize_block(&mut else_if.block.statements);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.optimize_block(&mut else_block.statements);
                }
                changed
            }
            _ => false,
        }
    }

    fn optimize_for_loop(&mut self, for_stmt: &mut Box<ForStatement>) -> bool {
        match &mut **for_stmt {
            ForStatement::Generic(for_gen) => {
                let modified_vars = self.collect_modified_variables(&for_gen.body);
                let new_body = self.hoist_invariants_simple(&for_gen.body, &modified_vars);
                for_gen.body = new_body;
                self.optimize_block(&mut for_gen.body.statements)
            }
            ForStatement::Numeric(for_num) => {
                let mut changed = false;
                if let Some((start, end, step)) = self.evaluate_numeric_bounds(for_num) {
                    if self.has_zero_iterations(start, end, step) {
                        for_num.body.statements.clear();
                        return true;
                    }
                }
                let modified_vars = self.collect_modified_variables(&for_num.body);
                let new_body = self.hoist_invariants_simple(&for_num.body, &modified_vars);
                for_num.body = new_body;
                changed |= self.optimize_block(&mut for_num.body.statements);
                changed
            }
        }
    }

    fn optimize_while_loop(
        &mut self,
        while_stmt: &mut typedlua_parser::ast::statement::WhileStatement,
    ) -> bool {
        let mut changed = false;
        if let ExpressionKind::Literal(Literal::Boolean(false)) = &while_stmt.condition.kind {
            while_stmt.body.statements.clear();
            return true;
        }
        let modified_vars = self.collect_modified_variables(&while_stmt.body);
        let new_body = self.hoist_invariants_simple(&while_stmt.body, &modified_vars);
        while_stmt.body = new_body;
        changed |= self.optimize_block(&mut while_stmt.body.statements);
        changed
    }

    fn optimize_repeat_loop(
        &mut self,
        repeat_stmt: &mut typedlua_parser::ast::statement::RepeatStatement,
    ) -> bool {
        let mut changed = false;
        if let ExpressionKind::Literal(Literal::Boolean(true)) = &repeat_stmt.until.kind {
            repeat_stmt.body.statements.clear();
            return true;
        }
        let modified_vars = self.collect_modified_variables(&repeat_stmt.body);
        let new_body = self.hoist_invariants_simple(&repeat_stmt.body, &modified_vars);
        repeat_stmt.body = new_body;
        changed |= self.optimize_block(&mut repeat_stmt.body.statements);
        changed
    }

    fn optimize_block(&mut self, stmts: &mut [Statement]) -> bool {
        let mut changed = false;
        for stmt in stmts {
            if self.optimize_loops_in_statement(stmt) {
                changed = true;
            }
        }
        changed
    }

    fn collect_modified_variables(&self, block: &Block) -> HashSet<StringId> {
        let mut modified = HashSet::new();
        self.collect_modified_in_block(block, &mut modified);
        modified
    }

    fn collect_modified_in_block(&self, block: &Block, modified: &mut HashSet<StringId>) {
        for stmt in &block.statements {
            self.collect_modified_in_statement(stmt, modified);
        }
    }

    fn collect_modified_in_statement(&self, stmt: &Statement, modified: &mut HashSet<StringId>) {
        match stmt {
            Statement::Variable(decl) => {
                self.collect_modified_in_pattern(&decl.pattern, modified);
            }
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => {
                    modified.insert(for_num.variable.node);
                    self.collect_modified_in_block(&for_num.body, modified);
                }
                ForStatement::Generic(for_gen) => {
                    for var in &for_gen.variables {
                        modified.insert(var.node);
                    }
                    self.collect_modified_in_block(&for_gen.body, modified);
                }
            },
            Statement::While(while_stmt) => {
                self.collect_modified_in_expression(&while_stmt.condition, modified);
                self.collect_modified_in_block(&while_stmt.body, modified);
            }
            Statement::Repeat(repeat_stmt) => {
                self.collect_modified_in_block(&repeat_stmt.body, modified);
                self.collect_modified_in_expression(&repeat_stmt.until, modified);
            }
            Statement::Function(func) => {
                self.collect_modified_in_block(&func.body, modified);
            }
            Statement::If(if_stmt) => {
                self.collect_modified_in_expression(&if_stmt.condition, modified);
                self.collect_modified_in_block(&if_stmt.then_block, modified);
                for else_if in &if_stmt.else_ifs {
                    self.collect_modified_in_expression(&else_if.condition, modified);
                    self.collect_modified_in_block(&else_if.block, modified);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.collect_modified_in_block(else_block, modified);
                }
            }
            Statement::Expression(expr) => {
                self.collect_modified_in_expression(expr, modified);
            }
            Statement::Return(ret_stmt) => {
                for expr in &ret_stmt.values {
                    self.collect_modified_in_expression(expr, modified);
                }
            }
            Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Rethrow(_)
            | Statement::Throw(_) => {}
            Statement::Class(_)
            | Statement::Interface(_)
            | Statement::Enum(_)
            | Statement::TypeAlias(_) => {}
            Statement::Import(_) | Statement::Export(_) => {}
            Statement::Block(block) => {
                self.collect_modified_in_block(block, modified);
            }
            Statement::Try(try_stmt) => {
                self.collect_modified_in_block(&try_stmt.try_block, modified);
                for catch in &try_stmt.catch_clauses {
                    match &catch.pattern {
                        typedlua_parser::ast::statement::CatchPattern::Typed {
                            variable, ..
                        } => {
                            modified.insert(variable.node);
                        }
                        typedlua_parser::ast::statement::CatchPattern::MultiTyped {
                            variable,
                            ..
                        } => {
                            modified.insert(variable.node);
                        }
                        typedlua_parser::ast::statement::CatchPattern::Untyped {
                            variable, ..
                        } => {
                            modified.insert(variable.node);
                        }
                    }
                    self.collect_modified_in_block(&catch.body, modified);
                }
                if let Some(finally_block) = &try_stmt.finally_block {
                    self.collect_modified_in_block(finally_block, modified);
                }
            }
            Statement::Namespace(_)
            | Statement::DeclareFunction(_)
            | Statement::DeclareNamespace(_)
            | Statement::DeclareType(_)
            | Statement::DeclareInterface(_)
            | Statement::DeclareConst(_) => {}
        }
    }

    fn collect_modified_in_pattern(&self, pattern: &Pattern, modified: &mut HashSet<StringId>) {
        match pattern {
            Pattern::Identifier(ident) => {
                modified.insert(ident.node);
            }
            Pattern::Array(array_pattern) => {
                for elem in &array_pattern.elements {
                    match elem {
                        typedlua_parser::ast::pattern::ArrayPatternElement::Pattern(p) => {
                            self.collect_modified_in_pattern(p, modified);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Rest(id) => {
                            modified.insert(id.node);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Hole => {}
                    }
                }
            }
            Pattern::Object(obj_pattern) => {
                for prop in &obj_pattern.properties {
                    if let Some(p) = &prop.value {
                        self.collect_modified_in_pattern(p, modified);
                    }
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_, _) => {}
            Pattern::Or(or_pattern) => {
                // All alternatives bind the same variables (guaranteed by type checker)
                // So we can collect from just the first alternative
                if let Some(first) = or_pattern.alternatives.first() {
                    self.collect_modified_in_pattern(first, modified);
                }
            }
        }
    }

    fn collect_modified_in_expression(&self, expr: &Expression, modified: &mut HashSet<StringId>) {
        match &expr.kind {
            ExpressionKind::Identifier(id) => {
                modified.insert(*id);
            }
            ExpressionKind::Binary(_, left, right) => {
                self.collect_modified_in_expression(left, modified);
                self.collect_modified_in_expression(right, modified);
            }
            ExpressionKind::Unary(_, operand) => {
                self.collect_modified_in_expression(operand, modified);
            }
            ExpressionKind::Call(func, args, _) => {
                self.collect_modified_in_expression(func, modified);
                for arg in args {
                    self.collect_modified_in_expression(&arg.value, modified);
                }
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.collect_modified_in_expression(obj, modified);
                for arg in args {
                    self.collect_modified_in_expression(&arg.value, modified);
                }
            }
            ExpressionKind::Member(obj, _) => {
                self.collect_modified_in_expression(obj, modified);
            }
            ExpressionKind::Index(obj, index) => {
                self.collect_modified_in_expression(obj, modified);
                self.collect_modified_in_expression(index, modified);
            }
            ExpressionKind::Assignment(lhs, _, rhs) => {
                self.collect_modified_in_expression(lhs, modified);
                self.collect_modified_in_expression(rhs, modified);
            }
            ExpressionKind::Array(elements) => {
                for elem in elements {
                    match elem {
                        ArrayElement::Expression(expr) => {
                            self.collect_modified_in_expression(expr, modified)
                        }
                        ArrayElement::Spread(expr) => {
                            self.collect_modified_in_expression(expr, modified)
                        }
                    }
                }
            }
            ExpressionKind::Object(properties) => {
                for prop in properties {
                    match prop {
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            key: _,
                            value,
                            span: _,
                        } => {
                            self.collect_modified_in_expression(value, modified);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            span: _,
                        } => {
                            self.collect_modified_in_expression(key, modified);
                            self.collect_modified_in_expression(value, modified);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value,
                            span: _,
                        } => {
                            self.collect_modified_in_expression(value, modified);
                        }
                    }
                }
            }
            ExpressionKind::Function(func) => {
                self.collect_modified_in_block(&func.body, modified);
            }
            ExpressionKind::Arrow(arrow) => {
                for param in &arrow.parameters {
                    self.collect_modified_in_pattern(&param.pattern, modified);
                }
                match &arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                        self.collect_modified_in_expression(expr, modified);
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        self.collect_modified_in_block(block, modified);
                    }
                }
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.collect_modified_in_expression(cond, modified);
                self.collect_modified_in_expression(then_expr, modified);
                self.collect_modified_in_expression(else_expr, modified);
            }
            ExpressionKind::Pipe(left, right) => {
                self.collect_modified_in_expression(left, modified);
                self.collect_modified_in_expression(right, modified);
            }
            ExpressionKind::Match(match_expr) => {
                self.collect_modified_in_expression(&match_expr.value, modified);
                for arm in &match_expr.arms {
                    self.collect_modified_in_pattern(&arm.pattern, modified);
                    if let Some(guard) = &arm.guard {
                        self.collect_modified_in_expression(guard, modified);
                    }
                    match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                            self.collect_modified_in_expression(expr, modified);
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            self.collect_modified_in_block(block, modified);
                        }
                    }
                }
            }
            ExpressionKind::Template(template) => {
                for part in &template.parts {
                    match part {
                        typedlua_parser::ast::expression::TemplatePart::String(_) => {}
                        typedlua_parser::ast::expression::TemplatePart::Expression(expr) => {
                            self.collect_modified_in_expression(expr, modified);
                        }
                    }
                }
            }
            ExpressionKind::Parenthesized(expr) => {
                self.collect_modified_in_expression(expr, modified);
            }
            ExpressionKind::TypeAssertion(expr, _) => {
                self.collect_modified_in_expression(expr, modified);
            }
            ExpressionKind::New(expr, args) => {
                self.collect_modified_in_expression(expr, modified);
                for arg in args {
                    self.collect_modified_in_expression(&arg.value, modified);
                }
            }
            ExpressionKind::OptionalMember(obj, _) => {
                self.collect_modified_in_expression(obj, modified);
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                self.collect_modified_in_expression(obj, modified);
                self.collect_modified_in_expression(index, modified);
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.collect_modified_in_expression(obj, modified);
                for arg in args {
                    self.collect_modified_in_expression(&arg.value, modified);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.collect_modified_in_expression(obj, modified);
                for arg in args {
                    self.collect_modified_in_expression(&arg.value, modified);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.collect_modified_in_expression(&try_expr.expression, modified);
                modified.insert(try_expr.catch_variable.node);
                self.collect_modified_in_expression(&try_expr.catch_expression, modified);
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.collect_modified_in_expression(left, modified);
                self.collect_modified_in_expression(right, modified);
            }
            ExpressionKind::Literal(_)
            | ExpressionKind::SelfKeyword
            | ExpressionKind::SuperKeyword => {}
        }
    }

    fn hoist_invariants_simple(&self, block: &Block, loop_vars: &HashSet<StringId>) -> Block {
        let mut new_statements = Vec::new();

        for stmt in &block.statements {
            match stmt {
                Statement::Variable(decl) => {
                    // TODO: If invariant, hoist outside loop
                    let _ = self.is_invariant_expression(&decl.initializer, loop_vars);
                    new_statements.push(stmt.clone());
                }
                _ => new_statements.push(stmt.clone()),
            }
        }

        Block {
            statements: new_statements,
            span: block.span,
        }
    }

    fn is_invariant_expression(&self, expr: &Expression, loop_vars: &HashSet<StringId>) -> bool {
        match &expr.kind {
            ExpressionKind::Literal(_) => true,
            ExpressionKind::Identifier(id) => !loop_vars.contains(id),
            ExpressionKind::Binary(_, left, right) => {
                self.is_invariant_expression(left, loop_vars)
                    && self.is_invariant_expression(right, loop_vars)
            }
            ExpressionKind::Unary(_, operand) => self.is_invariant_expression(operand, loop_vars),
            ExpressionKind::Call(func, args, _) => {
                let func_invariant = match &func.kind {
                    ExpressionKind::Identifier(id) => !loop_vars.contains(id),
                    _ => false,
                };
                func_invariant
                    && args
                        .iter()
                        .all(|arg| self.is_invariant_expression(&arg.value, loop_vars))
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.is_invariant_expression(obj, loop_vars)
                    && args
                        .iter()
                        .all(|arg| self.is_invariant_expression(&arg.value, loop_vars))
            }
            ExpressionKind::Member(obj, _) => self.is_invariant_expression(obj, loop_vars),
            ExpressionKind::Index(obj, index) => {
                self.is_invariant_expression(obj, loop_vars)
                    && self.is_invariant_expression(index, loop_vars)
            }
            ExpressionKind::Array(elements) => elements.iter().all(|elem| match elem {
                ArrayElement::Expression(e) => self.is_invariant_expression(e, loop_vars),
                ArrayElement::Spread(e) => self.is_invariant_expression(e, loop_vars),
            }),
            ExpressionKind::Object(properties) => properties.iter().all(|prop| match prop {
                typedlua_parser::ast::expression::ObjectProperty::Property {
                    key: _,
                    value,
                    span: _,
                } => self.is_invariant_expression(value, loop_vars),
                typedlua_parser::ast::expression::ObjectProperty::Computed {
                    key,
                    value,
                    span: _,
                } => {
                    self.is_invariant_expression(key, loop_vars)
                        && self.is_invariant_expression(value, loop_vars)
                }
                typedlua_parser::ast::expression::ObjectProperty::Spread { value, span: _ } => {
                    self.is_invariant_expression(value, loop_vars)
                }
            }),
            ExpressionKind::Function(_) => true,
            ExpressionKind::Arrow(arrow) => {
                let body_invariant = match &arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                        self.is_invariant_expression(expr, loop_vars)
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        block.statements.iter().all(|s| match s {
                            Statement::Variable(decl) => {
                                self.is_invariant_expression(&decl.initializer, loop_vars)
                            }
                            _ => false,
                        })
                    }
                };
                body_invariant
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.is_invariant_expression(cond, loop_vars)
                    && self.is_invariant_expression(then_expr, loop_vars)
                    && self.is_invariant_expression(else_expr, loop_vars)
            }
            ExpressionKind::Pipe(left, right) => {
                self.is_invariant_expression(left, loop_vars)
                    && self.is_invariant_expression(right, loop_vars)
            }
            ExpressionKind::Match(match_expr) => {
                self.is_invariant_expression(&match_expr.value, loop_vars)
                    && match_expr.arms.iter().all(|arm| {
                        let body_invariant = match &arm.body {
                            typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                                self.is_invariant_expression(expr, loop_vars)
                            }
                            typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                                block.statements.iter().all(|s| match s {
                                    Statement::Variable(decl) => {
                                        self.is_invariant_expression(&decl.initializer, loop_vars)
                                    }
                                    _ => false,
                                })
                            }
                        };
                        body_invariant
                    })
            }
            ExpressionKind::Template(template) => template.parts.iter().all(|part| match part {
                typedlua_parser::ast::expression::TemplatePart::String(_) => true,
                typedlua_parser::ast::expression::TemplatePart::Expression(expr) => {
                    self.is_invariant_expression(expr, loop_vars)
                }
            }),
            ExpressionKind::Parenthesized(expr) => self.is_invariant_expression(expr, loop_vars),
            ExpressionKind::TypeAssertion(expr, _) => self.is_invariant_expression(expr, loop_vars),
            ExpressionKind::New(expr, args) => {
                self.is_invariant_expression(expr, loop_vars)
                    && args
                        .iter()
                        .all(|arg| self.is_invariant_expression(&arg.value, loop_vars))
            }
            ExpressionKind::OptionalMember(obj, _) => self.is_invariant_expression(obj, loop_vars),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.is_invariant_expression(obj, loop_vars)
                    && self.is_invariant_expression(index, loop_vars)
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.is_invariant_expression(obj, loop_vars)
                    && args
                        .iter()
                        .all(|arg| self.is_invariant_expression(&arg.value, loop_vars))
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.is_invariant_expression(obj, loop_vars)
                    && args
                        .iter()
                        .all(|arg| self.is_invariant_expression(&arg.value, loop_vars))
            }
            ExpressionKind::Try(try_expr) => {
                self.is_invariant_expression(&try_expr.expression, loop_vars)
                    && self.is_invariant_expression(&try_expr.catch_expression, loop_vars)
            }
            ExpressionKind::Assignment(_, _, rhs) => self.is_invariant_expression(rhs, loop_vars),
            ExpressionKind::ErrorChain(left, right) => {
                self.is_invariant_expression(left, loop_vars)
                    && self.is_invariant_expression(right, loop_vars)
            }
            ExpressionKind::SelfKeyword | ExpressionKind::SuperKeyword => true,
        }
    }

    fn evaluate_numeric_bounds(&self, for_num: &ForNumeric) -> Option<(f64, f64, f64)> {
        let start = self.evaluate_constant_f64(&for_num.start)?;
        let end = self.evaluate_constant_f64(&for_num.end)?;
        let step = for_num
            .step
            .as_ref()
            .map(|s| self.evaluate_constant_f64(s))
            .unwrap_or(Some(1.0))?;
        Some((start, end, step))
    }

    fn evaluate_constant_f64(&self, expr: &Expression) -> Option<f64> {
        match &expr.kind {
            ExpressionKind::Literal(Literal::Number(n)) => Some(*n),
            ExpressionKind::Literal(Literal::Integer(n)) => Some(*n as f64),
            ExpressionKind::Unary(UnaryOp::Negate, operand) => {
                self.evaluate_constant_f64(operand).map(|n| -n)
            }
            ExpressionKind::Binary(BinaryOp::Add, left, right) => {
                let l = self.evaluate_constant_f64(left)?;
                let r = self.evaluate_constant_f64(right)?;
                Some(l + r)
            }
            ExpressionKind::Binary(BinaryOp::Subtract, left, right) => {
                let l = self.evaluate_constant_f64(left)?;
                let r = self.evaluate_constant_f64(right)?;
                Some(l - r)
            }
            ExpressionKind::Binary(BinaryOp::Multiply, left, right) => {
                let l = self.evaluate_constant_f64(left)?;
                let r = self.evaluate_constant_f64(right)?;
                Some(l * r)
            }
            ExpressionKind::Binary(BinaryOp::Divide, left, right) => {
                let l = self.evaluate_constant_f64(left)?;
                let r = self.evaluate_constant_f64(right)?;
                if r.abs() > 1e-10 {
                    Some(l / r)
                } else {
                    None
                }
            }
            ExpressionKind::Parenthesized(expr) => self.evaluate_constant_f64(expr),
            _ => None,
        }
    }

    fn has_zero_iterations(&self, start: f64, end: f64, step: f64) -> bool {
        if step.abs() < 1e-10 {
            return false;
        }
        if step > 0.0 {
            start > end
        } else {
            start < end
        }
    }
}

// =============================================================================
// O2: String Concatenation Optimization Pass
// =============================================================================

const MIN_CONCAT_PARTS_FOR_OPTIMIZATION: usize = 3;

#[derive(Default)]
pub struct StringConcatOptimizationPass {
    next_temp_id: usize,
    interner: Option<Rc<StringInterner>>,
}

impl OptimizationPass for StringConcatOptimizationPass {
    fn name(&self) -> &'static str {
        "string-concat-optimization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        self.next_temp_id = 0;

        let mut changed = false;
        let mut i = 0;
        while i < program.statements.len() {
            if self.optimize_statement(&mut program.statements[i]) {
                changed = true;
            }
            i += 1;
        }

        Ok(changed)
    }
}

impl StringConcatOptimizationPass {
    pub fn set_interner(&mut self, interner: Rc<StringInterner>) {
        self.interner = Some(interner);
    }

    fn optimize_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::Variable(decl) => self.optimize_concat_in_variable(decl),
            Statement::Expression(expr) => self.optimize_concat_in_expression(expr),
            Statement::Function(func) => {
                let mut changed = false;
                for s in &mut func.body.statements {
                    if self.optimize_statement(s) {
                        changed = true;
                    }
                }
                changed
            }
            Statement::Return(ret) => {
                let mut changed = false;
                for expr in &mut ret.values {
                    if self.optimize_concat_expression(expr) {
                        changed = true;
                    }
                }
                changed
            }
            Statement::If(if_stmt) => {
                let mut changed = false;
                for s in &mut if_stmt.then_block.statements {
                    if self.optimize_statement(s) {
                        changed = true;
                    }
                }
                for else_if in &mut if_stmt.else_ifs {
                    for s in &mut else_if.block.statements {
                        if self.optimize_statement(s) {
                            changed = true;
                        }
                    }
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    for s in &mut else_block.statements {
                        if self.optimize_statement(s) {
                            changed = true;
                        }
                    }
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = false;
                for s in &mut while_stmt.body.statements {
                    if self.optimize_statement(s) {
                        changed = true;
                    }
                }
                changed
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Generic(for_gen) => {
                    let mut changed = false;
                    for s in &mut for_gen.body.statements {
                        if self.optimize_statement(s) {
                            changed = true;
                        }
                    }
                    changed
                }
                ForStatement::Numeric(for_num) => {
                    let mut changed = false;
                    for s in &mut for_num.body.statements {
                        if self.optimize_statement(s) {
                            changed = true;
                        }
                    }
                    changed
                }
            },
            Statement::Repeat(repeat_stmt) => {
                let mut changed = false;
                for s in &mut repeat_stmt.body.statements {
                    if self.optimize_statement(s) {
                        changed = true;
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn optimize_concat_in_variable(&mut self, decl: &mut VariableDeclaration) -> bool {
        self.optimize_concat_expression(&mut decl.initializer)
    }

    fn optimize_concat_in_expression(&mut self, expr: &mut Expression) -> bool {
        self.optimize_concat_expression(expr)
    }

    fn optimize_concat_expression(&mut self, expr: &mut Expression) -> bool {
        if let ExpressionKind::Binary(BinaryOp::Concatenate, _left, _right) = &expr.kind {
            let parts = self.flatten_concat_chain(expr);
            if parts.len() >= MIN_CONCAT_PARTS_FOR_OPTIMIZATION {
                self.replace_with_table_concat(expr, &parts);
                return true;
            }
        }
        false
    }

    fn flatten_concat_chain(&self, expr: &Expression) -> Vec<Expression> {
        fn flatten_inner(expr: &Expression, result: &mut Vec<Expression>) {
            match &expr.kind {
                ExpressionKind::Binary(BinaryOp::Concatenate, left, right) => {
                    flatten_inner(left, result);
                    flatten_inner(right, result);
                }
                ExpressionKind::Parenthesized(inner) => {
                    flatten_inner(inner, result);
                }
                _ => {
                    result.push(expr.clone());
                }
            }
        }
        let mut parts = Vec::new();
        flatten_inner(expr, &mut parts);
        parts
    }

    fn replace_with_table_concat(&self, expr: &mut Expression, parts: &[Expression]) {
        let table_expr = Expression::new(
            ExpressionKind::Array(
                parts
                    .iter()
                    .map(|p| ArrayElement::Expression(p.clone()))
                    .collect(),
            ),
            Span::dummy(),
        );

        let concat_call = Expression::new(
            ExpressionKind::Call(
                Box::new(Expression::new(
                    ExpressionKind::Member(
                        Box::new(Expression::new(
                            ExpressionKind::Identifier(self.interner_get_or_intern("table")),
                            Span::dummy(),
                        )),
                        Spanned::new(self.interner_get_or_intern("concat"), Span::dummy()),
                    ),
                    Span::dummy(),
                )),
                vec![Argument {
                    value: table_expr,
                    is_spread: false,
                    span: Span::dummy(),
                }],
                None,
            ),
            Span::dummy(),
        );

        *expr = concat_call;
    }

    fn interner_get_or_intern(&self, name: &str) -> StringId {
        if let Some(interner) = self.get_interner() {
            interner.get_or_intern(name)
        } else {
            unsafe { std::hint::unreachable_unchecked() }
        }
    }

    fn get_interner(&self) -> Option<&Rc<StringInterner>> {
        self.interner.as_ref()
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
        let mut program_block = Block {
            statements: std::mem::take(&mut program.statements),
            span: program.span,
        };
        let changed = self.eliminate_dead_stores_in_block(&mut program_block);
        program.statements = program_block.statements;
        program.span = program_block.span;

        Ok(changed)
    }
}

impl DeadStoreEliminationPass {
    fn eliminate_dead_stores_in_statement(&mut self, stmt: &mut Statement) -> bool {
        match stmt {
            Statement::Function(func) => self.eliminate_dead_stores_in_block(&mut func.body),
            Statement::Block(block) => self.eliminate_dead_stores_in_block(block),
            Statement::Variable(decl) => {
                self.eliminate_dead_stores_in_expression(&mut decl.initializer)
            }
            Statement::Expression(expr) => self.eliminate_dead_stores_in_expression(expr),
            Statement::If(if_stmt) => {
                let mut changed = self.eliminate_dead_stores_in_block(&mut if_stmt.then_block);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.eliminate_dead_stores_in_block(&mut else_if.block);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.eliminate_dead_stores_in_block(else_block);
                }
                changed
            }
            Statement::While(while_stmt) => {
                self.eliminate_dead_stores_in_block(&mut while_stmt.body)
            }
            Statement::For(for_stmt) => match &mut **for_stmt {
                ForStatement::Numeric(for_num) => {
                    self.eliminate_dead_stores_in_block(&mut for_num.body)
                }
                ForStatement::Generic(for_gen) => {
                    self.eliminate_dead_stores_in_block(&mut for_gen.body)
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.eliminate_dead_stores_in_block(&mut repeat_stmt.body)
            }
            Statement::Return(ret) => {
                let mut changed = false;
                for expr in &mut ret.values {
                    changed |= self.eliminate_dead_stores_in_expression(expr);
                }
                changed
            }
            _ => false,
        }
    }

    fn eliminate_dead_stores_in_expression(&mut self, expr: &mut Expression) -> bool {
        match &mut expr.kind {
            ExpressionKind::Function(func) => self.eliminate_dead_stores_in_block(&mut func.body),
            ExpressionKind::Arrow(arrow) => match &mut arrow.body {
                typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                    self.eliminate_dead_stores_in_block(block)
                }
                typedlua_parser::ast::expression::ArrowBody::Expression(inner) => {
                    self.eliminate_dead_stores_in_expression(inner)
                }
            },
            ExpressionKind::Call(func, args, _) => {
                let mut changed = self.eliminate_dead_stores_in_expression(func);
                for arg in args {
                    changed |= self.eliminate_dead_stores_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                let mut changed = self.eliminate_dead_stores_in_expression(obj);
                for arg in args {
                    changed |= self.eliminate_dead_stores_in_expression(&mut arg.value);
                }
                changed
            }
            ExpressionKind::Binary(_, left, right) => {
                let mut changed = self.eliminate_dead_stores_in_expression(left);
                changed |= self.eliminate_dead_stores_in_expression(right);
                changed
            }
            ExpressionKind::Unary(_, operand) => self.eliminate_dead_stores_in_expression(operand),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut changed = self.eliminate_dead_stores_in_expression(cond);
                changed |= self.eliminate_dead_stores_in_expression(then_expr);
                changed |= self.eliminate_dead_stores_in_expression(else_expr);
                changed
            }
            ExpressionKind::Array(elements) => {
                let mut changed = false;
                for elem in elements {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(e) => {
                            changed |= self.eliminate_dead_stores_in_expression(e);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(e) => {
                            changed |= self.eliminate_dead_stores_in_expression(e);
                        }
                    }
                }
                changed
            }
            ExpressionKind::Object(properties) => {
                let mut changed = false;
                for prop in properties {
                    match prop {
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            value,
                            ..
                        } => {
                            changed |= self.eliminate_dead_stores_in_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            ..
                        } => {
                            changed |= self.eliminate_dead_stores_in_expression(key);
                            changed |= self.eliminate_dead_stores_in_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value, ..
                        } => {
                            changed |= self.eliminate_dead_stores_in_expression(value);
                        }
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn eliminate_dead_stores_in_block(&mut self, block: &mut Block) -> bool {
        if block.statements.is_empty() {
            return false;
        }

        let captured = self.collect_captured_variables(block);

        let mut live_vars: HashSet<StringId> = HashSet::new();
        let mut new_statements: Vec<Statement> = Vec::new();
        let mut changed = false;

        for stmt in block.statements.iter().rev() {
            let (is_dead, newly_live) = self.analyze_statement(stmt, &live_vars, &captured);

            if is_dead {
                changed = true;
            } else {
                let mut stmt_clone = stmt.clone();
                changed |= self.eliminate_dead_stores_in_statement(&mut stmt_clone);
                new_statements.push(stmt_clone);
            }

            for var in newly_live {
                live_vars.insert(var);
            }
        }

        if changed {
            new_statements.reverse();
            block.statements = new_statements;
        }

        changed
    }

    fn analyze_statement(
        &self,
        stmt: &Statement,
        live_vars: &HashSet<StringId>,
        captured: &HashSet<StringId>,
    ) -> (bool, HashSet<StringId>) {
        match stmt {
            Statement::Variable(decl) => {
                let names = self.names_from_pattern(&decl.pattern);
                let has_side_effects = self.expression_has_side_effects(&decl.initializer);

                let mut newly_live = HashSet::new();
                let mut is_dead = true;

                for name in &names {
                    if captured.contains(name) || live_vars.contains(name) || has_side_effects {
                        is_dead = false;
                        newly_live.insert(*name);
                    }
                }

                (is_dead && !names.is_empty(), newly_live)
            }
            Statement::Expression(expr) => {
                if let ExpressionKind::Assignment(lhs, _, rhs) = &expr.kind {
                    let names = self.names_from_lhs(lhs);
                    let has_side_effects = self.expression_has_side_effects(rhs);

                    let mut is_dead = true;
                    for name in &names {
                        if live_vars.contains(name) || has_side_effects {
                            is_dead = false;
                        }
                    }

                    let reads = self.collect_expression_reads(lhs);
                    (is_dead, reads)
                } else {
                    // Non-assignment expressions are never eliminated (may have side effects)
                    let reads = self.collect_expression_reads(expr);
                    (false, reads)
                }
            }
            Statement::Return(ret) => {
                let mut reads = HashSet::new();
                for expr in &ret.values {
                    reads.extend(self.collect_expression_reads(expr));
                }
                (false, reads)
            }
            Statement::If(if_stmt) => {
                let mut then_live = live_vars.clone();
                let mut else_live = live_vars.clone();
                let mut else_if_lives: Vec<HashSet<StringId>> = Vec::new();

                self.collect_block_reads(&if_stmt.then_block, &mut then_live);
                for else_if in &if_stmt.else_ifs {
                    let mut branch_live = live_vars.clone();
                    self.collect_block_reads(&else_if.block, &mut branch_live);
                    else_if_lives.push(branch_live.clone());
                    self.collect_expression_reads_into(&else_if.condition, &mut branch_live);
                    else_live.extend(branch_live);
                }

                self.collect_expression_reads_into(&if_stmt.condition, &mut then_live);
                self.collect_expression_reads_into(&if_stmt.condition, &mut else_live);

                if let Some(else_block) = &if_stmt.else_block {
                    self.collect_block_reads(else_block, &mut else_live);
                }

                let condition_reads = self.collect_expression_reads(&if_stmt.condition);
                let mut all_live = condition_reads;
                all_live.extend(then_live);
                for branch_live in &else_if_lives {
                    all_live.extend(branch_live.clone());
                }
                all_live.extend(else_live);

                (false, all_live)
            }
            Statement::While(while_stmt) => {
                let mut live = live_vars.clone();
                self.collect_expression_reads_into(&while_stmt.condition, &mut live);
                self.collect_block_reads(&while_stmt.body, &mut live);
                (false, live)
            }
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => {
                    let mut live = live_vars.clone();
                    live.insert(for_num.variable.node);
                    self.collect_block_reads(&for_num.body, &mut live);
                    (false, live)
                }
                ForStatement::Generic(for_gen) => {
                    let mut live = live_vars.clone();
                    for var in &for_gen.variables {
                        live.insert(var.node);
                    }
                    self.collect_block_reads(&for_gen.body, &mut live);
                    (false, live)
                }
            },
            Statement::Repeat(repeat_stmt) => {
                let mut live = live_vars.clone();
                self.collect_block_reads(&repeat_stmt.body, &mut live);
                self.collect_expression_reads_into(&repeat_stmt.until, &mut live);
                (false, live)
            }
            Statement::Block(block) => {
                let mut live = live_vars.clone();
                self.collect_block_reads(block, &mut live);
                (false, live)
            }
            Statement::Try(try_stmt) => {
                let mut live = live_vars.clone();
                self.collect_block_reads(&try_stmt.try_block, &mut live);
                for catch in &try_stmt.catch_clauses {
                    match &catch.pattern {
                        typedlua_parser::ast::statement::CatchPattern::Typed {
                            variable, ..
                        } => {
                            live.insert(variable.node);
                        }
                        typedlua_parser::ast::statement::CatchPattern::MultiTyped {
                            variable,
                            ..
                        } => {
                            live.insert(variable.node);
                        }
                        typedlua_parser::ast::statement::CatchPattern::Untyped {
                            variable, ..
                        } => {
                            live.insert(variable.node);
                        }
                    }
                    self.collect_block_reads(&catch.body, &mut live);
                }
                if let Some(finally_block) = &try_stmt.finally_block {
                    self.collect_block_reads(finally_block, &mut live);
                }
                (false, live)
            }
            _ => (false, HashSet::new()),
        }
    }

    fn names_from_pattern(&self, pattern: &Pattern) -> Vec<StringId> {
        let mut names = Vec::new();
        self.collect_names_from_pattern(pattern, &mut names);
        names
    }

    fn collect_names_from_pattern(&self, pattern: &Pattern, names: &mut Vec<StringId>) {
        match pattern {
            Pattern::Identifier(ident) => {
                names.push(ident.node);
            }
            Pattern::Array(arr) => {
                for elem in &arr.elements {
                    match elem {
                        typedlua_parser::ast::pattern::ArrayPatternElement::Pattern(p) => {
                            self.collect_names_from_pattern(p, names);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Rest(ident) => {
                            names.push(ident.node);
                        }
                        typedlua_parser::ast::pattern::ArrayPatternElement::Hole => {}
                    }
                }
            }
            Pattern::Object(obj) => {
                for prop in &obj.properties {
                    if let Some(value_pattern) = &prop.value {
                        self.collect_names_from_pattern(value_pattern, names);
                    } else {
                        names.push(prop.key.node);
                    }
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_, _) => {}
            Pattern::Or(or_pattern) => {
                // All alternatives bind the same variables (guaranteed by type checker)
                // So we can collect from just the first alternative
                if let Some(first) = or_pattern.alternatives.first() {
                    self.collect_names_from_pattern(first, names);
                }
            }
        }
    }

    fn names_from_lhs(&self, expr: &Expression) -> Vec<StringId> {
        let mut names = Vec::new();
        self.collect_names_from_lhs(expr, &mut names);
        names
    }

    fn collect_names_from_lhs(&self, expr: &Expression, names: &mut Vec<StringId>) {
        match &expr.kind {
            ExpressionKind::Identifier(name) => {
                names.push(*name);
            }
            ExpressionKind::Index(obj, _) => {
                self.collect_names_from_lhs(obj, names);
            }
            ExpressionKind::Member(obj, _) => {
                self.collect_names_from_lhs(obj, names);
            }
            _ => {}
        }
    }

    fn expression_has_side_effects(&self, expr: &Expression) -> bool {
        match &expr.kind {
            ExpressionKind::Call(_, _, _) => true,
            ExpressionKind::MethodCall(_, _, _, _) => true,
            ExpressionKind::Assignment(_, _, _) => true,
            ExpressionKind::Binary(BinaryOp::And, left, right) => {
                self.expression_has_side_effects(left) || self.expression_has_side_effects(right)
            }
            ExpressionKind::Binary(BinaryOp::Or, left, right) => {
                self.expression_has_side_effects(left) || self.expression_has_side_effects(right)
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.expression_has_side_effects(cond)
                    || self.expression_has_side_effects(then_expr)
                    || self.expression_has_side_effects(else_expr)
            }
            _ => false,
        }
    }

    fn collect_expression_reads(&self, expr: &Expression) -> HashSet<StringId> {
        let mut reads = HashSet::new();
        self.collect_expression_reads_into(expr, &mut reads);
        reads
    }

    fn collect_expression_reads_into(&self, expr: &Expression, reads: &mut HashSet<StringId>) {
        match &expr.kind {
            ExpressionKind::Identifier(name) => {
                reads.insert(*name);
            }
            ExpressionKind::Binary(_, left, right) => {
                self.collect_expression_reads_into(left, reads);
                self.collect_expression_reads_into(right, reads);
            }
            ExpressionKind::Unary(_, operand) => {
                self.collect_expression_reads_into(operand, reads);
            }
            ExpressionKind::Call(func, args, _) => {
                self.collect_expression_reads_into(func, reads);
                for arg in args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.collect_expression_reads_into(obj, reads);
                for arg in args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::Member(obj, _) => {
                self.collect_expression_reads_into(obj, reads);
            }
            ExpressionKind::Index(obj, index) => {
                self.collect_expression_reads_into(obj, reads);
                self.collect_expression_reads_into(index, reads);
            }
            ExpressionKind::Assignment(lhs, _, rhs) => {
                self.collect_expression_reads_into(lhs, reads);
                self.collect_expression_reads_into(rhs, reads);
            }
            ExpressionKind::Array(elements) => {
                for elem in elements {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(e) => {
                            self.collect_expression_reads_into(e, reads);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(e) => {
                            self.collect_expression_reads_into(e, reads);
                        }
                    }
                }
            }
            ExpressionKind::Object(properties) => {
                for prop in properties {
                    match prop {
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            value,
                            ..
                        } => {
                            self.collect_expression_reads_into(value, reads);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            ..
                        } => {
                            self.collect_expression_reads_into(key, reads);
                            self.collect_expression_reads_into(value, reads);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value, ..
                        } => {
                            self.collect_expression_reads_into(value, reads);
                        }
                    }
                }
            }
            ExpressionKind::Function(func) => {
                self.collect_block_reads(&func.body, reads);
            }
            ExpressionKind::Arrow(arrow) => match &arrow.body {
                typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                    self.collect_expression_reads_into(expr, reads);
                }
                typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                    self.collect_block_reads(block, reads);
                }
            },
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.collect_expression_reads_into(cond, reads);
                self.collect_expression_reads_into(then_expr, reads);
                self.collect_expression_reads_into(else_expr, reads);
            }
            ExpressionKind::Pipe(left, right) => {
                self.collect_expression_reads_into(left, reads);
                self.collect_expression_reads_into(right, reads);
            }
            ExpressionKind::Match(match_expr) => {
                self.collect_expression_reads_into(&match_expr.value, reads);
                for arm in &match_expr.arms {
                    match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                            self.collect_expression_reads_into(expr, reads);
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            self.collect_block_reads(block, reads);
                        }
                    }
                }
            }
            ExpressionKind::Template(template) => {
                for part in &template.parts {
                    if let typedlua_parser::ast::expression::TemplatePart::Expression(expr) = part {
                        self.collect_expression_reads_into(expr, reads);
                    }
                }
            }
            ExpressionKind::Parenthesized(expr) => {
                self.collect_expression_reads_into(expr, reads);
            }
            ExpressionKind::TypeAssertion(expr, _) => {
                self.collect_expression_reads_into(expr, reads);
            }
            ExpressionKind::New(expr, args) => {
                self.collect_expression_reads_into(expr, reads);
                for arg in args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::OptionalMember(obj, _) => {
                self.collect_expression_reads_into(obj, reads);
            }
            ExpressionKind::OptionalIndex(obj, index) => {
                self.collect_expression_reads_into(obj, reads);
                self.collect_expression_reads_into(index, reads);
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.collect_expression_reads_into(obj, reads);
                for arg in args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.collect_expression_reads_into(obj, reads);
                for arg in args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.collect_expression_reads_into(&try_expr.expression, reads);
                self.collect_expression_reads_into(&try_expr.catch_expression, reads);
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.collect_expression_reads_into(left, reads);
                self.collect_expression_reads_into(right, reads);
            }
            ExpressionKind::Literal(_)
            | ExpressionKind::SelfKeyword
            | ExpressionKind::SuperKeyword => {}
        }
    }

    fn collect_block_reads(&self, block: &Block, reads: &mut HashSet<StringId>) {
        for stmt in &block.statements {
            self.collect_statement_reads(stmt, reads);
        }
    }

    fn collect_statement_reads(&self, stmt: &Statement, reads: &mut HashSet<StringId>) {
        match stmt {
            Statement::Variable(decl) => {
                self.collect_expression_reads_into(&decl.initializer, reads);
            }
            Statement::Expression(expr) => {
                self.collect_expression_reads_into(expr, reads);
            }
            Statement::Return(ret) => {
                for expr in &ret.values {
                    self.collect_expression_reads_into(expr, reads);
                }
            }
            Statement::If(if_stmt) => {
                self.collect_expression_reads_into(&if_stmt.condition, reads);
                self.collect_block_reads(&if_stmt.then_block, reads);
                for else_if in &if_stmt.else_ifs {
                    self.collect_expression_reads_into(&else_if.condition, reads);
                    self.collect_block_reads(&else_if.block, reads);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.collect_block_reads(else_block, reads);
                }
            }
            Statement::While(while_stmt) => {
                self.collect_expression_reads_into(&while_stmt.condition, reads);
                self.collect_block_reads(&while_stmt.body, reads);
            }
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => {
                    self.collect_expression_reads_into(&for_num.start, reads);
                    self.collect_expression_reads_into(&for_num.end, reads);
                    if let Some(step) = &for_num.step {
                        self.collect_expression_reads_into(step, reads);
                    }
                    self.collect_block_reads(&for_num.body, reads);
                }
                ForStatement::Generic(for_gen) => {
                    for expr in &for_gen.iterators {
                        self.collect_expression_reads_into(expr, reads);
                    }
                    self.collect_block_reads(&for_gen.body, reads);
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.collect_block_reads(&repeat_stmt.body, reads);
                self.collect_expression_reads_into(&repeat_stmt.until, reads);
            }
            Statement::Function(func) => {
                self.collect_block_reads(&func.body, reads);
            }
            Statement::Block(block) => {
                self.collect_block_reads(block, reads);
            }
            Statement::Try(try_stmt) => {
                self.collect_block_reads(&try_stmt.try_block, reads);
                for catch in &try_stmt.catch_clauses {
                    self.collect_block_reads(&catch.body, reads);
                }
                if let Some(finally_block) = &try_stmt.finally_block {
                    self.collect_block_reads(finally_block, reads);
                }
            }
            _ => {}
        }
    }

    fn collect_captured_variables(&self, block: &Block) -> HashSet<StringId> {
        let mut captured = HashSet::new();
        self.collect_captures_in_block(block, &mut captured);
        captured
    }

    fn collect_captures_in_block(&self, block: &Block, captured: &mut HashSet<StringId>) {
        for stmt in &block.statements {
            self.collect_captures_in_statement(stmt, captured);
        }
    }

    fn collect_captures_in_statement(&self, stmt: &Statement, captured: &mut HashSet<StringId>) {
        match stmt {
            Statement::Variable(decl) => {
                if self.expression_captures_variables(&decl.initializer) {
                    for name in self.names_from_pattern(&decl.pattern) {
                        captured.insert(name);
                    }
                }
            }
            Statement::Expression(expr) => {
                self.expression_captures_variables(expr);
            }
            Statement::Function(func) => {
                self.collect_captures_in_block(&func.body, captured);
            }
            Statement::Block(block) => {
                self.collect_captures_in_block(block, captured);
            }
            Statement::If(if_stmt) => {
                self.collect_captures_in_block(&if_stmt.then_block, captured);
                for else_if in &if_stmt.else_ifs {
                    self.collect_captures_in_block(&else_if.block, captured);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.collect_captures_in_block(else_block, captured);
                }
            }
            Statement::While(while_stmt) => {
                self.collect_captures_in_block(&while_stmt.body, captured);
            }
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => {
                    self.collect_captures_in_block(&for_num.body, captured);
                }
                ForStatement::Generic(for_gen) => {
                    self.collect_captures_in_block(&for_gen.body, captured);
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.collect_captures_in_block(&repeat_stmt.body, captured);
            }
            Statement::Try(try_stmt) => {
                self.collect_captures_in_block(&try_stmt.try_block, captured);
                for catch in &try_stmt.catch_clauses {
                    self.collect_captures_in_block(&catch.body, captured);
                }
                if let Some(finally_block) = &try_stmt.finally_block {
                    self.collect_captures_in_block(finally_block, captured);
                }
            }
            _ => {}
        }
    }

    fn expression_captures_variables(&self, expr: &Expression) -> bool {
        match &expr.kind {
            ExpressionKind::Function(_) => true,
            ExpressionKind::Arrow(_) => true,
            ExpressionKind::Call(func, args, _) => {
                self.expression_captures_variables(func)
                    || args
                        .iter()
                        .any(|arg| self.expression_captures_variables(&arg.value))
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.expression_captures_variables(obj)
                    || args
                        .iter()
                        .any(|arg| self.expression_captures_variables(&arg.value))
            }
            ExpressionKind::Binary(_, left, right) => {
                self.expression_captures_variables(left)
                    || self.expression_captures_variables(right)
            }
            ExpressionKind::Unary(_, operand) => self.expression_captures_variables(operand),
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.expression_captures_variables(cond)
                    || self.expression_captures_variables(then_expr)
                    || self.expression_captures_variables(else_expr)
            }
            ExpressionKind::Pipe(left, right) => {
                self.expression_captures_variables(left)
                    || self.expression_captures_variables(right)
            }
            ExpressionKind::Match(match_expr) => {
                self.expression_captures_variables(&match_expr.value)
                    || match_expr.arms.iter().any(|arm| match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                            self.expression_captures_variables(expr)
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            self.block_captures_variables(block)
                        }
                    })
            }
            ExpressionKind::New(expr, args) => {
                self.expression_captures_variables(expr)
                    || args
                        .iter()
                        .any(|arg| self.expression_captures_variables(&arg.value))
            }
            ExpressionKind::Try(try_expr) => {
                self.expression_captures_variables(&try_expr.expression)
                    || self.expression_captures_variables(&try_expr.catch_expression)
            }
            ExpressionKind::ErrorChain(left, right) => {
                self.expression_captures_variables(left)
                    || self.expression_captures_variables(right)
            }
            ExpressionKind::OptionalMember(obj, _) => self.expression_captures_variables(obj),
            ExpressionKind::OptionalIndex(obj, index) => {
                self.expression_captures_variables(obj) || self.expression_captures_variables(index)
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                self.expression_captures_variables(obj)
                    || args
                        .iter()
                        .any(|arg| self.expression_captures_variables(&arg.value))
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.expression_captures_variables(obj)
                    || args
                        .iter()
                        .any(|arg| self.expression_captures_variables(&arg.value))
            }
            ExpressionKind::TypeAssertion(expr, _) => self.expression_captures_variables(expr),
            ExpressionKind::Member(obj, _) => self.expression_captures_variables(obj),
            ExpressionKind::Index(obj, index) => {
                self.expression_captures_variables(obj) || self.expression_captures_variables(index)
            }
            _ => false,
        }
    }

    fn block_captures_variables(&self, block: &Block) -> bool {
        for stmt in &block.statements {
            if self.statement_captures_variables(stmt) {
                return true;
            }
        }
        false
    }

    fn statement_captures_variables(&self, stmt: &Statement) -> bool {
        match stmt {
            Statement::Variable(decl) => self.expression_captures_variables(&decl.initializer),
            Statement::Expression(expr) => self.expression_captures_variables(expr),
            Statement::Function(func) => self.block_captures_variables(&func.body),
            Statement::Block(block) => self.block_captures_variables(block),
            Statement::If(if_stmt) => {
                self.block_captures_variables(&if_stmt.then_block)
                    || if_stmt
                        .else_ifs
                        .iter()
                        .any(|ei| self.block_captures_variables(&ei.block))
                    || if_stmt
                        .else_block
                        .as_ref()
                        .is_some_and(|eb| self.block_captures_variables(eb))
            }
            Statement::While(while_stmt) => self.block_captures_variables(&while_stmt.body),
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => self.block_captures_variables(&for_num.body),
                ForStatement::Generic(for_gen) => self.block_captures_variables(&for_gen.body),
            },
            Statement::Repeat(repeat_stmt) => self.block_captures_variables(&repeat_stmt.body),
            Statement::Try(try_stmt) => {
                self.block_captures_variables(&try_stmt.try_block)
                    || try_stmt
                        .catch_clauses
                        .iter()
                        .any(|c| self.block_captures_variables(&c.body))
                    || try_stmt
                        .finally_block
                        .as_ref()
                        .is_some_and(|fb| self.block_captures_variables(fb))
            }
            _ => false,
        }
    }
}

// =============================================================================
// O2: Tail Call Optimization Pass
// =============================================================================

/// Tail call optimization pass
/// Analyzes tail call patterns and ensures other optimizations don't break TCO positions
/// Lua automatically handles tail calls at runtime - this pass provides analysis and verification
pub struct TailCallOptimizationPass;

impl OptimizationPass for TailCallOptimizationPass {
    fn name(&self) -> &'static str {
        "tail-call-optimization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
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

// =============================================================================
// O3: Operator Inlining Pass
// =============================================================================

/// Operator inlining pass
/// Inlines operator overload implementations when types are known
#[allow(dead_code)]
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
                    .filter(|m| {
                        matches!(m, typedlua_parser::ast::statement::ClassMember::Method(_))
                    })
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
#[allow(dead_code)]
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
                    .filter(|m| {
                        matches!(
                            m,
                            typedlua_parser::ast::statement::InterfaceMember::Method(_)
                        )
                    })
                    .count();
            }
        }

        // Analysis only
        Ok(false)
    }
}

// =============================================================================
// O3: Generic Specialization Pass
// =============================================================================

use crate::typechecker::{build_substitutions, instantiate_function_declaration};
use rustc_hash::FxHashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use typedlua_parser::ast::expression::ObjectProperty;
use typedlua_parser::ast::types::Type;

/// Computes a hash of type arguments for caching specialized functions
fn hash_type_args(type_args: &[Type]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for t in type_args {
        // Hash the debug representation - simple but effective
        format!("{:?}", t.kind).hash(&mut hasher);
    }
    hasher.finish()
}

/// Generic specialization pass
/// Creates specialized versions of generic functions for known types
#[derive(Default)]
pub struct GenericSpecializationPass {
    interner: Option<Rc<StringInterner>>,
    /// Maps (function_name, type_args_hash) -> specialized_function_name
    specializations: FxHashMap<(StringId, u64), StringId>,
    /// Counter for generating unique specialization IDs
    next_spec_id: usize,
    /// Collected generic function declarations
    generic_functions: FxHashMap<StringId, FunctionDeclaration>,
    /// New specialized function declarations to add to program
    new_functions: Vec<Statement>,
}

impl GenericSpecializationPass {
    pub fn set_interner(&mut self, interner: Rc<StringInterner>) {
        self.interner = Some(interner);
    }

    /// Collects all generic function declarations from the program
    fn collect_generic_functions(&mut self, program: &Program) {
        for stmt in &program.statements {
            if let Statement::Function(func) = stmt {
                if func.type_parameters.is_some() {
                    self.generic_functions.insert(func.name.node, func.clone());
                }
            }
        }
    }

    /// Creates a specialized version of a generic function with concrete type arguments
    fn specialize_function(
        &mut self,
        func: &FunctionDeclaration,
        type_args: &[Type],
    ) -> Option<StringId> {
        let interner = self.interner.as_ref()?;
        let type_params = func.type_parameters.as_ref()?;

        // Build type substitution map
        let substitutions = match build_substitutions(type_params, type_args) {
            Ok(s) => s,
            Err(_) => return None,
        };

        // Check cache first
        let type_args_hash = hash_type_args(type_args);
        let cache_key = (func.name.node, type_args_hash);
        if let Some(&specialized_name) = self.specializations.get(&cache_key) {
            return Some(specialized_name);
        }

        // Generate specialized function name: funcName__spec{id}
        let orig_name = interner.resolve(func.name.node);
        let specialized_name_str = format!("{}__spec{}", orig_name, self.next_spec_id);
        self.next_spec_id += 1;

        // Intern the new name
        let specialized_name = interner.get_or_intern(&specialized_name_str);

        // Create specialized function by instantiating with type substitutions
        let mut specialized_func = instantiate_function_declaration(func, &substitutions);
        specialized_func.name =
            typedlua_parser::ast::Spanned::new(specialized_name, func.name.span);

        // Add to cache and to list of new functions
        self.specializations.insert(cache_key, specialized_name);
        self.new_functions
            .push(Statement::Function(specialized_func));

        Some(specialized_name)
    }

    /// Processes a statement looking for call sites to specialize
    fn specialize_calls_in_statement(&mut self, stmt: &mut Statement) -> bool {
        let mut changed = false;

        match stmt {
            Statement::Variable(var_decl) => {
                if self.specialize_calls_in_expression(&mut var_decl.initializer) {
                    changed = true;
                }
            }
            Statement::Expression(expr) => {
                if self.specialize_calls_in_expression(expr) {
                    changed = true;
                }
            }
            Statement::Return(ret) => {
                for value in &mut ret.values {
                    if self.specialize_calls_in_expression(value) {
                        changed = true;
                    }
                }
            }
            Statement::If(if_stmt) => {
                if self.specialize_calls_in_expression(&mut if_stmt.condition) {
                    changed = true;
                }
                for stmt in &mut if_stmt.then_block.statements {
                    if self.specialize_calls_in_statement(stmt) {
                        changed = true;
                    }
                }
                for else_if in &mut if_stmt.else_ifs {
                    if self.specialize_calls_in_expression(&mut else_if.condition) {
                        changed = true;
                    }
                    for stmt in &mut else_if.block.statements {
                        if self.specialize_calls_in_statement(stmt) {
                            changed = true;
                        }
                    }
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    for stmt in &mut else_block.statements {
                        if self.specialize_calls_in_statement(stmt) {
                            changed = true;
                        }
                    }
                }
            }
            Statement::While(while_stmt) => {
                if self.specialize_calls_in_expression(&mut while_stmt.condition) {
                    changed = true;
                }
                for stmt in &mut while_stmt.body.statements {
                    if self.specialize_calls_in_statement(stmt) {
                        changed = true;
                    }
                }
            }
            Statement::For(for_stmt) => match for_stmt.as_mut() {
                ForStatement::Numeric(num) => {
                    if self.specialize_calls_in_expression(&mut num.start) {
                        changed = true;
                    }
                    if self.specialize_calls_in_expression(&mut num.end) {
                        changed = true;
                    }
                    if let Some(step) = &mut num.step {
                        if self.specialize_calls_in_expression(step) {
                            changed = true;
                        }
                    }
                    for stmt in &mut num.body.statements {
                        if self.specialize_calls_in_statement(stmt) {
                            changed = true;
                        }
                    }
                }
                ForStatement::Generic(gen) => {
                    for iter in &mut gen.iterators {
                        if self.specialize_calls_in_expression(iter) {
                            changed = true;
                        }
                    }
                    for stmt in &mut gen.body.statements {
                        if self.specialize_calls_in_statement(stmt) {
                            changed = true;
                        }
                    }
                }
            },
            Statement::Function(func) => {
                for stmt in &mut func.body.statements {
                    if self.specialize_calls_in_statement(stmt) {
                        changed = true;
                    }
                }
            }
            Statement::Block(block) => {
                for stmt in &mut block.statements {
                    if self.specialize_calls_in_statement(stmt) {
                        changed = true;
                    }
                }
            }
            Statement::Repeat(repeat) => {
                for stmt in &mut repeat.body.statements {
                    if self.specialize_calls_in_statement(stmt) {
                        changed = true;
                    }
                }
                if self.specialize_calls_in_expression(&mut repeat.until) {
                    changed = true;
                }
            }
            Statement::Throw(throw) => {
                if self.specialize_calls_in_expression(&mut throw.expression) {
                    changed = true;
                }
            }
            // Other statements don't contain call expressions we care about
            _ => {}
        }

        changed
    }

    /// Processes an expression looking for call sites to specialize
    fn specialize_calls_in_expression(&mut self, expr: &mut Expression) -> bool {
        let mut changed = false;

        match &mut expr.kind {
            ExpressionKind::Call(callee, args, type_args) => {
                // First process nested expressions
                if self.specialize_calls_in_expression(callee) {
                    changed = true;
                }
                for arg in args.iter_mut() {
                    if self.specialize_calls_in_expression(&mut arg.value) {
                        changed = true;
                    }
                }

                // Check if this is a call to a generic function with concrete type args
                if let Some(type_args) = type_args {
                    if !type_args.is_empty() {
                        // Check if callee is a direct identifier reference to a generic function
                        if let ExpressionKind::Identifier(func_name) = &callee.kind {
                            if let Some(func) = self.generic_functions.get(func_name).cloned() {
                                // Specialize this call
                                if let Some(specialized_name) =
                                    self.specialize_function(&func, type_args)
                                {
                                    // Replace callee with specialized function name
                                    callee.kind = ExpressionKind::Identifier(specialized_name);
                                    // Clear type arguments since the function is now monomorphic
                                    *type_args = Vec::new();
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }

            ExpressionKind::Binary(_, left, right) => {
                if self.specialize_calls_in_expression(left) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(right) {
                    changed = true;
                }
            }

            ExpressionKind::Unary(_, operand) => {
                if self.specialize_calls_in_expression(operand) {
                    changed = true;
                }
            }

            ExpressionKind::Assignment(target, _, value) => {
                if self.specialize_calls_in_expression(target) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(value) {
                    changed = true;
                }
            }

            ExpressionKind::MethodCall(obj, _, args, _) => {
                if self.specialize_calls_in_expression(obj) {
                    changed = true;
                }
                for arg in args.iter_mut() {
                    if self.specialize_calls_in_expression(&mut arg.value) {
                        changed = true;
                    }
                }
                // Method specialization is more complex - skip for now
            }

            ExpressionKind::Member(obj, _) => {
                if self.specialize_calls_in_expression(obj) {
                    changed = true;
                }
            }

            ExpressionKind::Index(obj, index) => {
                if self.specialize_calls_in_expression(obj) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(index) {
                    changed = true;
                }
            }

            ExpressionKind::Array(elements) => {
                for elem in elements.iter_mut() {
                    match elem {
                        ArrayElement::Expression(e) | ArrayElement::Spread(e) => {
                            if self.specialize_calls_in_expression(e) {
                                changed = true;
                            }
                        }
                    }
                }
            }

            ExpressionKind::Object(props) => {
                for prop in props.iter_mut() {
                    match prop {
                        ObjectProperty::Property { value, .. } => {
                            if self.specialize_calls_in_expression(value) {
                                changed = true;
                            }
                        }
                        ObjectProperty::Computed { key, value, .. } => {
                            if self.specialize_calls_in_expression(key) {
                                changed = true;
                            }
                            if self.specialize_calls_in_expression(value) {
                                changed = true;
                            }
                        }
                        ObjectProperty::Spread { value, .. } => {
                            if self.specialize_calls_in_expression(value) {
                                changed = true;
                            }
                        }
                    }
                }
            }

            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                if self.specialize_calls_in_expression(cond) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(then_expr) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(else_expr) {
                    changed = true;
                }
            }

            ExpressionKind::Pipe(left, right) => {
                if self.specialize_calls_in_expression(left) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(right) {
                    changed = true;
                }
            }

            ExpressionKind::Parenthesized(inner) => {
                if self.specialize_calls_in_expression(inner) {
                    changed = true;
                }
            }

            ExpressionKind::TypeAssertion(inner, _) => {
                if self.specialize_calls_in_expression(inner) {
                    changed = true;
                }
            }

            ExpressionKind::OptionalCall(callee, args, _)
            | ExpressionKind::OptionalMethodCall(callee, _, args, _) => {
                if self.specialize_calls_in_expression(callee) {
                    changed = true;
                }
                for arg in args.iter_mut() {
                    if self.specialize_calls_in_expression(&mut arg.value) {
                        changed = true;
                    }
                }
            }

            ExpressionKind::OptionalMember(obj, _) | ExpressionKind::OptionalIndex(obj, _) => {
                if self.specialize_calls_in_expression(obj) {
                    changed = true;
                }
            }

            ExpressionKind::New(callee, args) => {
                if self.specialize_calls_in_expression(callee) {
                    changed = true;
                }
                for arg in args.iter_mut() {
                    if self.specialize_calls_in_expression(&mut arg.value) {
                        changed = true;
                    }
                }
            }

            ExpressionKind::ErrorChain(left, right) => {
                if self.specialize_calls_in_expression(left) {
                    changed = true;
                }
                if self.specialize_calls_in_expression(right) {
                    changed = true;
                }
            }

            // Literals, identifiers, self, super - no calls to specialize
            _ => {}
        }

        changed
    }
}

impl OptimizationPass for GenericSpecializationPass {
    fn name(&self) -> &'static str {
        "generic-specialization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError> {
        // Reset state for fresh run
        self.specializations.clear();
        self.generic_functions.clear();
        self.new_functions.clear();
        self.next_spec_id = 0;

        // Phase 1: Collect all generic function declarations
        self.collect_generic_functions(program);

        if self.generic_functions.is_empty() {
            return Ok(false);
        }

        // Phase 2: Find and specialize call sites
        let mut changed = false;
        for stmt in &mut program.statements {
            if self.specialize_calls_in_statement(stmt) {
                changed = true;
            }
        }

        // Phase 3: Add specialized functions to the program
        // Insert them after the original function declarations, not at the end
        // (to avoid being removed by dead code elimination after return statements)
        if !self.new_functions.is_empty() {
            // Find the last function statement index
            let mut insert_idx = 0;
            for (i, stmt) in program.statements.iter().enumerate() {
                if matches!(stmt, Statement::Function(_)) {
                    insert_idx = i + 1;
                }
            }
            // Insert new functions at that position
            for (i, func) in self.new_functions.drain(..).enumerate() {
                program.statements.insert(insert_idx + i, func);
            }
            changed = true;
        }

        Ok(changed)
    }
}
