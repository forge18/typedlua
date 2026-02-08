// =============================================================================
// O2: Dead Store Elimination Pass
// =============================================================================

use bumpalo::Bump;
use crate::optimizer::BlockVisitor;
use std::collections::HashSet;
use typedlua_parser::ast::expression::{BinaryOp, Expression, ExpressionKind};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{Block, ForStatement, Statement};
use typedlua_parser::string_interner::StringId;

/// Dead store elimination pass
/// Removes assignments to variables that are never read
pub struct DeadStoreEliminationPass;

impl DeadStoreEliminationPass {
    pub fn new() -> Self {
        Self
    }
}

impl<'arena> BlockVisitor<'arena> for DeadStoreEliminationPass {
    fn visit_block_stmts(&mut self, stmts: &mut Vec<Statement<'arena>>, arena: &'arena Bump) -> bool {
        self.eliminate_dead_stores_in_vec(stmts, arena)
    }
}

impl DeadStoreEliminationPass {
    fn eliminate_dead_stores_in_statement<'arena>(
        &mut self,
        stmt: &mut Statement<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        match stmt {
            Statement::Function(func) => self.eliminate_dead_stores_in_block(&mut func.body, arena),
            Statement::Block(block) => self.eliminate_dead_stores_in_block(block, arena),
            Statement::Variable(decl) => {
                self.eliminate_dead_stores_in_expression(&mut decl.initializer, arena)
            }
            Statement::Expression(expr) => {
                self.eliminate_dead_stores_in_expression(expr, arena)
            }
            Statement::If(if_stmt) => {
                let mut changed =
                    self.eliminate_dead_stores_in_block(&mut if_stmt.then_block, arena);
                let mut new_else_ifs: Vec<_> = if_stmt.else_ifs.to_vec();
                let mut eic = false;
                for else_if in &mut new_else_ifs {
                    eic |= self.eliminate_dead_stores_in_block(&mut else_if.block, arena);
                }
                if eic {
                    if_stmt.else_ifs = arena.alloc_slice_clone(&new_else_ifs);
                    changed = true;
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.eliminate_dead_stores_in_block(else_block, arena);
                }
                changed
            }
            Statement::While(while_stmt) => {
                self.eliminate_dead_stores_in_block(&mut while_stmt.body, arena)
            }
            Statement::For(for_stmt) => {
                match &**for_stmt {
                    ForStatement::Numeric(for_num_ref) => {
                        let mut new_num = (**for_num_ref).clone();
                        let changed =
                            self.eliminate_dead_stores_in_block(&mut new_num.body, arena);
                        if changed {
                            *stmt = Statement::For(
                                arena.alloc(ForStatement::Numeric(arena.alloc(new_num))),
                            );
                        }
                        changed
                    }
                    ForStatement::Generic(for_gen_ref) => {
                        let mut new_gen = for_gen_ref.clone();
                        let changed =
                            self.eliminate_dead_stores_in_block(&mut new_gen.body, arena);
                        if changed {
                            *stmt = Statement::For(
                                arena.alloc(ForStatement::Generic(new_gen)),
                            );
                        }
                        changed
                    }
                }
            }
            Statement::Repeat(repeat_stmt) => {
                self.eliminate_dead_stores_in_block(&mut repeat_stmt.body, arena)
            }
            Statement::Return(ret) => {
                let mut vals: Vec<_> = ret.values.to_vec();
                let mut changed = false;
                for expr in &mut vals {
                    changed |= self.eliminate_dead_stores_in_expression(expr, arena);
                }
                if changed {
                    ret.values = arena.alloc_slice_clone(&vals);
                }
                changed
            }
            _ => false,
        }
    }

    fn eliminate_dead_stores_in_expression<'arena>(
        &mut self,
        expr: &mut Expression<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        match &expr.kind {
            ExpressionKind::Function(func) => {
                let mut new_func = func.clone();
                let changed = self.eliminate_dead_stores_in_block(&mut new_func.body, arena);
                if changed {
                    expr.kind = ExpressionKind::Function(new_func);
                }
                changed
            }
            ExpressionKind::Arrow(arrow) => match &arrow.body {
                typedlua_parser::ast::expression::ArrowBody::Block(_) => {
                    let mut new_arrow = arrow.clone();
                    if let typedlua_parser::ast::expression::ArrowBody::Block(ref mut block) =
                        new_arrow.body
                    {
                        let changed = self.eliminate_dead_stores_in_block(block, arena);
                        if changed {
                            expr.kind = ExpressionKind::Arrow(new_arrow);
                        }
                        changed
                    } else {
                        unreachable!()
                    }
                }
                typedlua_parser::ast::expression::ArrowBody::Expression(inner) => {
                    let mut new_inner = (**inner).clone();
                    let changed = self.eliminate_dead_stores_in_expression(&mut new_inner, arena);
                    if changed {
                        let mut new_arrow = arrow.clone();
                        new_arrow.body = typedlua_parser::ast::expression::ArrowBody::Expression(
                            arena.alloc(new_inner),
                        );
                        expr.kind = ExpressionKind::Arrow(new_arrow);
                    }
                    changed
                }
            },
            ExpressionKind::Call(func_expr, args, type_args) => {
                let mut new_func = (**func_expr).clone();
                let mut func_changed = self.eliminate_dead_stores_in_expression(&mut new_func, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut args_changed = false;
                for arg in &mut new_args {
                    args_changed |= self.eliminate_dead_stores_in_expression(&mut arg.value, arena);
                }
                let type_args = *type_args;
                if func_changed || args_changed {
                    expr.kind = ExpressionKind::Call(
                        arena.alloc(new_func),
                        arena.alloc_slice_clone(&new_args),
                        type_args,
                    );
                    func_changed = true;
                }
                func_changed
            }
            ExpressionKind::MethodCall(obj, method, args, type_args) => {
                let method = method.clone();
                let mut new_obj = (**obj).clone();
                let mut obj_changed = self.eliminate_dead_stores_in_expression(&mut new_obj, arena);
                let mut new_args: Vec<_> = args.to_vec();
                let mut args_changed = false;
                for arg in &mut new_args {
                    args_changed |= self.eliminate_dead_stores_in_expression(&mut arg.value, arena);
                }
                let type_args = *type_args;
                if obj_changed || args_changed {
                    expr.kind = ExpressionKind::MethodCall(
                        arena.alloc(new_obj),
                        method,
                        arena.alloc_slice_clone(&new_args),
                        type_args,
                    );
                    obj_changed = true;
                }
                obj_changed
            }
            ExpressionKind::Binary(op, left, right) => {
                let op = *op;
                let mut new_left = (**left).clone();
                let mut new_right = (**right).clone();
                let left_changed = self.eliminate_dead_stores_in_expression(&mut new_left, arena);
                let right_changed = self.eliminate_dead_stores_in_expression(&mut new_right, arena);
                if left_changed || right_changed {
                    expr.kind = ExpressionKind::Binary(
                        op,
                        arena.alloc(new_left),
                        arena.alloc(new_right),
                    );
                }
                left_changed || right_changed
            }
            ExpressionKind::Unary(op, operand) => {
                let op = *op;
                let mut new_operand = (**operand).clone();
                let changed = self.eliminate_dead_stores_in_expression(&mut new_operand, arena);
                if changed {
                    expr.kind = ExpressionKind::Unary(op, arena.alloc(new_operand));
                }
                changed
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut new_cond = (**cond).clone();
                let mut new_then = (**then_expr).clone();
                let mut new_else = (**else_expr).clone();
                let c1 = self.eliminate_dead_stores_in_expression(&mut new_cond, arena);
                let c2 = self.eliminate_dead_stores_in_expression(&mut new_then, arena);
                let c3 = self.eliminate_dead_stores_in_expression(&mut new_else, arena);
                if c1 || c2 || c3 {
                    expr.kind = ExpressionKind::Conditional(
                        arena.alloc(new_cond),
                        arena.alloc(new_then),
                        arena.alloc(new_else),
                    );
                }
                c1 || c2 || c3
            }
            ExpressionKind::Array(elements) => {
                let mut new_elements: Vec<_> = elements.to_vec();
                let mut changed = false;
                for elem in &mut new_elements {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(e) => {
                            changed |= self.eliminate_dead_stores_in_expression(e, arena);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(e) => {
                            changed |= self.eliminate_dead_stores_in_expression(e, arena);
                        }
                    }
                }
                if changed {
                    expr.kind =
                        ExpressionKind::Array(arena.alloc_slice_clone(&new_elements));
                }
                changed
            }
            ExpressionKind::Object(properties) => {
                use typedlua_parser::ast::expression::ObjectProperty;
                let mut new_props: Vec<_> = properties.to_vec();
                let mut changed = false;
                for prop in &mut new_props {
                    match prop {
                        ObjectProperty::Property { key, value, span } => {
                            let mut new_val = (**value).clone();
                            if self.eliminate_dead_stores_in_expression(&mut new_val, arena) {
                                *prop = ObjectProperty::Property {
                                    key: key.clone(),
                                    value: arena.alloc(new_val),
                                    span: *span,
                                };
                                changed = true;
                            }
                        }
                        ObjectProperty::Computed { key, value, span } => {
                            let mut new_key = (**key).clone();
                            let mut new_val = (**value).clone();
                            let kc = self.eliminate_dead_stores_in_expression(&mut new_key, arena);
                            let vc = self.eliminate_dead_stores_in_expression(&mut new_val, arena);
                            if kc || vc {
                                *prop = ObjectProperty::Computed {
                                    key: arena.alloc(new_key),
                                    value: arena.alloc(new_val),
                                    span: *span,
                                };
                                changed = true;
                            }
                        }
                        ObjectProperty::Spread { value, span } => {
                            let mut new_val = (**value).clone();
                            if self.eliminate_dead_stores_in_expression(&mut new_val, arena) {
                                *prop = ObjectProperty::Spread {
                                    value: arena.alloc(new_val),
                                    span: *span,
                                };
                                changed = true;
                            }
                        }
                    }
                }
                if changed {
                    expr.kind =
                        ExpressionKind::Object(arena.alloc_slice_clone(&new_props));
                }
                changed
            }
            _ => false,
        }
    }

    fn eliminate_dead_stores_in_block<'arena>(
        &mut self,
        block: &mut Block<'arena>,
        arena: &'arena Bump,
    ) -> bool {
        let mut stmts: Vec<_> = block.statements.to_vec();
        let changed = self.eliminate_dead_stores_in_vec(&mut stmts, arena);
        if changed {
            block.statements = arena.alloc_slice_clone(&stmts);
        }
        changed
    }

    fn eliminate_dead_stores_in_vec<'arena>(
        &mut self,
        stmts: &mut Vec<Statement<'arena>>,
        arena: &'arena Bump,
    ) -> bool {
        if stmts.is_empty() {
            return false;
        }

        let captured = self.collect_captured_variables_from_slice(stmts);

        let mut new_statements: Vec<Statement<'arena>> = Vec::new();
        let mut changed = false;
        let mut current_live_vars: HashSet<StringId> = HashSet::new();

        for stmt in stmts.iter().rev() {
            let names = self.names_from_pattern(stmt);
            let has_side_effects = self.statement_has_side_effects(stmt);
            let stmt_reads = self.collect_statement_reads(stmt);

            // A statement is dead if it declares variables that are never read
            // It's NOT dead if:
            // - It has no variable declarations (not a dead store candidate)
            // - Any declared variable is captured by a closure
            // - The statement has side effects
            // - Any declared variable is live (used later)
            let is_dead = if names.is_empty() {
                // Not a variable declaration, can't be a dead store
                false
            } else if has_side_effects {
                // Side effects must be preserved
                false
            } else {
                // Check if ALL declared names are dead (not captured and not live)
                names
                    .iter()
                    .all(|name| !captured.contains(name) && !current_live_vars.contains(name))
            };

            if !is_dead {
                let mut stmt_clone = stmt.clone();
                changed |= self.eliminate_dead_stores_in_statement(&mut stmt_clone, arena);
                new_statements.push(stmt_clone);
                // Add variables read by this statement to the live set
                for id in stmt_reads {
                    current_live_vars.insert(id);
                }
            } else {
                changed = true;
            }
        }

        if changed {
            new_statements.reverse();
            *stmts = new_statements;
        }

        changed
    }

    fn statement_has_side_effects<'arena>(&self, stmt: &Statement<'arena>) -> bool {
        match stmt {
            Statement::Variable(decl) => self.expression_has_side_effects(&decl.initializer),
            Statement::Expression(expr) => {
                if let ExpressionKind::Assignment(_, _, _) = &expr.kind {
                    true
                } else {
                    self.expression_has_side_effects(expr)
                }
            }
            Statement::If(if_stmt) => {
                self.expression_has_side_effects(&if_stmt.condition)
                    || self.block_has_side_effects(&if_stmt.then_block)
                    || if_stmt
                        .else_ifs
                        .iter()
                        .any(|ei| self.block_has_side_effects(&ei.block))
                    || if_stmt
                        .else_block
                        .as_ref()
                        .is_some_and(|eb| self.block_has_side_effects(eb))
            }
            Statement::While(while_stmt) => {
                self.expression_has_side_effects(&while_stmt.condition)
                    || self.block_has_side_effects(&while_stmt.body)
            }
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => {
                    self.expression_has_side_effects(&for_num.start)
                        || self.expression_has_side_effects(&for_num.end)
                        || for_num
                            .step
                            .as_ref()
                            .is_some_and(|s| self.expression_has_side_effects(s))
                        || self.block_has_side_effects(&for_num.body)
                }
                ForStatement::Generic(for_gen) => {
                    for_expr_has_side_effects(for_gen.iterators)
                        || self.block_has_side_effects(&for_gen.body)
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.expression_has_side_effects(&repeat_stmt.until)
                    || self.block_has_side_effects(&repeat_stmt.body)
            }
            Statement::Return(ret) => ret
                .values
                .iter()
                .any(|e| self.expression_has_side_effects(e)),
            Statement::Try(try_stmt) => {
                self.block_has_side_effects(&try_stmt.try_block)
                    || try_stmt
                        .catch_clauses
                        .iter()
                        .any(|c| self.block_has_side_effects(&c.body))
                    || try_stmt
                        .finally_block
                        .as_ref()
                        .is_some_and(|fb| self.block_has_side_effects(fb))
            }
            Statement::Function(func) => self.block_has_side_effects(&func.body),
            Statement::Block(block) => self.block_has_side_effects(block),
            _ => false,
        }
    }

    fn block_has_side_effects<'arena>(&self, block: &Block<'arena>) -> bool {
        block
            .statements
            .iter()
            .any(|s| self.statement_has_side_effects(s))
    }

    fn names_from_pattern<'arena>(&self, stmt: &Statement<'arena>) -> Vec<StringId> {
        let mut names = Vec::new();
        if let Statement::Variable(decl) = stmt {
            self.collect_names_from_pattern(&decl.pattern, &mut names);
        }
        names
    }

    fn collect_names_from_pattern<'arena>(
        &self,
        pattern: &Pattern<'arena>,
        names: &mut Vec<StringId>,
    ) {
        match pattern {
            Pattern::Identifier(ident) => {
                names.push(ident.node);
            }
            Pattern::Array(arr) => {
                for elem in arr.elements {
                    match elem {
                        typedlua_parser::ast::pattern::ArrayPatternElement::Pattern(typedlua_parser::ast::pattern::PatternWithDefault { pattern: p, .. }) => {
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
                for prop in obj.properties {
                    if let Some(value_pattern) = &prop.value {
                        self.collect_names_from_pattern(value_pattern, names);
                    } else {
                        names.push(prop.key.node);
                    }
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_, _) => {}
            Pattern::Or(or_pattern) => {
                if let Some(first) = or_pattern.alternatives.first() {
                    self.collect_names_from_pattern(first, names);
                }
            }
        }
    }

    fn expression_has_side_effects<'arena>(&self, expr: &Expression<'arena>) -> bool {
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

    fn collect_statement_reads<'arena>(&self, stmt: &Statement<'arena>) -> HashSet<StringId> {
        let mut reads = HashSet::new();
        self.collect_statement_reads_into(stmt, &mut reads);
        reads
    }

    fn collect_statement_reads_into<'arena>(
        &self,
        stmt: &Statement<'arena>,
        reads: &mut HashSet<StringId>,
    ) {
        match stmt {
            Statement::Variable(decl) => {
                self.collect_expression_reads_into(&decl.initializer, reads);
            }
            Statement::Expression(expr) => {
                self.collect_expression_reads_into(expr, reads);
            }
            Statement::Return(ret) => {
                for expr in ret.values {
                    self.collect_expression_reads_into(expr, reads);
                }
            }
            Statement::If(if_stmt) => {
                self.collect_expression_reads_into(&if_stmt.condition, reads);
                self.collect_block_reads_into(&if_stmt.then_block, reads);
                for else_if in if_stmt.else_ifs {
                    self.collect_expression_reads_into(&else_if.condition, reads);
                    self.collect_block_reads_into(&else_if.block, reads);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.collect_block_reads_into(else_block, reads);
                }
            }
            Statement::While(while_stmt) => {
                self.collect_expression_reads_into(&while_stmt.condition, reads);
                self.collect_block_reads_into(&while_stmt.body, reads);
            }
            Statement::For(for_stmt) => match &**for_stmt {
                ForStatement::Numeric(for_num) => {
                    self.collect_expression_reads_into(&for_num.start, reads);
                    self.collect_expression_reads_into(&for_num.end, reads);
                    if let Some(step) = &for_num.step {
                        self.collect_expression_reads_into(step, reads);
                    }
                    self.collect_block_reads_into(&for_num.body, reads);
                }
                ForStatement::Generic(for_gen) => {
                    for iter_expr in for_gen.iterators {
                        self.collect_expression_reads_into(iter_expr, reads);
                    }
                    self.collect_block_reads_into(&for_gen.body, reads);
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.collect_block_reads_into(&repeat_stmt.body, reads);
                self.collect_expression_reads_into(&repeat_stmt.until, reads);
            }
            Statement::Function(func) => {
                self.collect_block_reads_into(&func.body, reads);
            }
            Statement::Block(block) => {
                self.collect_block_reads_into(block, reads);
            }
            Statement::Try(try_stmt) => {
                self.collect_block_reads_into(&try_stmt.try_block, reads);
                for catch in try_stmt.catch_clauses {
                    self.collect_block_reads_into(&catch.body, reads);
                }
                if let Some(finally_block) = &try_stmt.finally_block {
                    self.collect_block_reads_into(finally_block, reads);
                }
            }
            _ => {}
        }
    }

    fn collect_block_reads_into<'arena>(
        &self,
        block: &Block<'arena>,
        reads: &mut HashSet<StringId>,
    ) {
        for stmt in block.statements {
            self.collect_statement_reads_into(stmt, reads);
        }
    }

    fn collect_expression_reads_into<'arena>(
        &self,
        expr: &Expression<'arena>,
        reads: &mut HashSet<StringId>,
    ) {
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
                for arg in *args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::MethodCall(obj, _, args, _) => {
                self.collect_expression_reads_into(obj, reads);
                for arg in *args {
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
                for elem in *elements {
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
                for prop in *properties {
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
                self.collect_block_reads_into(&func.body, reads);
            }
            ExpressionKind::Arrow(arrow) => match &arrow.body {
                typedlua_parser::ast::expression::ArrowBody::Expression(expr) => {
                    self.collect_expression_reads_into(expr, reads);
                }
                typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                    self.collect_block_reads_into(block, reads);
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
                self.collect_expression_reads_into(match_expr.value, reads);
                for arm in match_expr.arms {
                    match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                            self.collect_expression_reads_into(expr, reads);
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            self.collect_block_reads_into(block, reads);
                        }
                    }
                }
            }
            ExpressionKind::Template(template) => {
                for part in template.parts {
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
            ExpressionKind::New(expr, args, _) => {
                self.collect_expression_reads_into(expr, reads);
                for arg in *args {
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
                for arg in *args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::OptionalMethodCall(obj, _, args, _) => {
                self.collect_expression_reads_into(obj, reads);
                for arg in *args {
                    self.collect_expression_reads_into(&arg.value, reads);
                }
            }
            ExpressionKind::Try(try_expr) => {
                self.collect_expression_reads_into(try_expr.expression, reads);
                self.collect_expression_reads_into(try_expr.catch_expression, reads);
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

    fn collect_captured_variables_from_slice<'arena>(
        &self,
        stmts: &[Statement<'arena>],
    ) -> HashSet<StringId> {
        let mut captured = HashSet::new();
        for stmt in stmts {
            self.collect_captures_in_statement(stmt, &mut captured);
        }
        captured
    }

    fn collect_captured_variables<'arena>(&self, block: &Block<'arena>) -> HashSet<StringId> {
        let mut captured = HashSet::new();
        self.collect_captures_in_block(block, &mut captured);
        captured
    }

    fn collect_captures_in_block<'arena>(
        &self,
        block: &Block<'arena>,
        captured: &mut HashSet<StringId>,
    ) {
        for stmt in block.statements {
            self.collect_captures_in_statement(stmt, captured);
        }
    }

    fn collect_captures_in_statement<'arena>(
        &self,
        stmt: &Statement<'arena>,
        captured: &mut HashSet<StringId>,
    ) {
        match stmt {
            Statement::Variable(decl) => {
                if self.expression_captures_variables(&decl.initializer) {
                    let mut names = Vec::new();
                    self.collect_names_from_pattern(&decl.pattern, &mut names);
                    for name in names {
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
                for else_if in if_stmt.else_ifs {
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
                for catch in try_stmt.catch_clauses {
                    self.collect_captures_in_block(&catch.body, captured);
                }
                if let Some(finally_block) = &try_stmt.finally_block {
                    self.collect_captures_in_block(finally_block, captured);
                }
            }
            _ => {}
        }
    }

    fn expression_captures_variables<'arena>(&self, expr: &Expression<'arena>) -> bool {
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
                self.expression_captures_variables(match_expr.value)
                    || match_expr.arms.iter().any(|arm| match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(expr) => {
                            self.expression_captures_variables(expr)
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            self.block_captures_variables(block)
                        }
                    })
            }
            ExpressionKind::New(expr, args, _) => {
                self.expression_captures_variables(expr)
                    || args
                        .iter()
                        .any(|arg| self.expression_captures_variables(&arg.value))
            }
            ExpressionKind::Try(try_expr) => {
                self.expression_captures_variables(try_expr.expression)
                    || self.expression_captures_variables(try_expr.catch_expression)
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

    fn block_captures_variables<'arena>(&self, block: &Block<'arena>) -> bool {
        for stmt in block.statements {
            if self.statement_captures_variables(stmt) {
                return true;
            }
        }
        false
    }

    fn statement_captures_variables<'arena>(&self, stmt: &Statement<'arena>) -> bool {
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

impl Default for DeadStoreEliminationPass {
    fn default() -> Self {
        Self::new()
    }
}

fn for_expr_has_side_effects<'arena>(
    exprs: &[typedlua_parser::ast::expression::Expression<'arena>],
) -> bool {
    exprs.iter().any(|e| {
        matches!(
            &e.kind,
            ExpressionKind::Call(_, _, _)
                | ExpressionKind::MethodCall(_, _, _, _)
                | ExpressionKind::Assignment(_, _, _)
        )
    })
}
