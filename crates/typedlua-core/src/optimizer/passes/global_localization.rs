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

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
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
            ExpressionKind::New(callee, args, _) => {
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
            ExpressionKind::New(callee, args, _) => {
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
