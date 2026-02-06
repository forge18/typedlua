use rustc_hash::FxHashSet as HashSet;
use typedlua_parser::ast::expression::*;
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{ExportKind, Statement, VariableDeclaration, VariableKind};
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::StringId;
use typedlua_parser::string_interner::StringInterner;

#[derive(Debug, Clone, Default)]
pub struct HoistableDeclarations {
    pub functions: HashSet<String>,
    pub variables: HashSet<String>,
    pub classes: HashSet<String>,
    pub enums: HashSet<String>,
}

impl HoistableDeclarations {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_hoistable(&self, name: &str, kind: &DeclarationKind) -> bool {
        match kind {
            DeclarationKind::Function => self.functions.contains(name),
            DeclarationKind::Variable => self.variables.contains(name),
            DeclarationKind::Class => self.classes.contains(name),
            DeclarationKind::Enum => self.enums.contains(name),
        }
    }

    pub fn all_names(&self) -> HashSet<String> {
        let mut names = HashSet::default();
        names.extend(self.functions.clone());
        names.extend(self.variables.clone());
        names.extend(self.classes.clone());
        names.extend(self.enums.clone());
        names
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationKind {
    Function,
    Variable,
    Class,
    Enum,
}

#[derive(Debug, Clone)]
pub struct EscapeAnalysis<'a> {
    interner: &'a StringInterner,
    exported_names: HashSet<String>,
    return_values: HashSet<String>,
    module_locals: HashSet<String>,
    variable_declarations: HashSet<String>,
}

impl<'a> EscapeAnalysis<'a> {
    pub fn new(interner: &'a StringInterner) -> Self {
        EscapeAnalysis {
            interner,
            exported_names: HashSet::default(),
            return_values: HashSet::default(),
            module_locals: HashSet::default(),
            variable_declarations: HashSet::default(),
        }
    }

    pub fn analyze(program: &Program, interner: &StringInterner) -> HoistableDeclarations {
        let mut analysis = EscapeAnalysis::new(interner);
        analysis.collect_module_locals(program);
        analysis.collect_exports(program);
        analysis.collect_return_values_from_private_functions(program);
        let result = analysis.find_hoistable_declarations(program);
        // Debug: log what was found
        if cfg!(test) {
            if !analysis.module_locals.is_empty() {
                eprintln!("DEBUG module_locals: {:?}", analysis.module_locals);
                eprintln!("DEBUG exported_names: {:?}", analysis.exported_names);
                eprintln!("DEBUG return_values: {:?}", analysis.return_values);
                eprintln!("DEBUG hoistable result: funcs={:?}, vars={:?}, classes={:?}, enums={:?}",
                    result.functions, result.variables, result.classes, result.enums);
            }
        }
        result
    }

    fn collect_module_locals(&mut self, program: &Program) {
        for statement in &program.statements {
            if let Some((_, name)) = self.get_declaration_name(statement) {
                self.module_locals.insert(name.clone());
                // Track which ones are variables
                let is_variable = match statement {
                    Statement::Variable(_) => true,
                    Statement::Export(e) => {
                        matches!(e.kind, ExportKind::Declaration(ref inner) if matches!(inner.as_ref(), Statement::Variable(_)))
                    }
                    _ => false,
                };
                if is_variable {
                    self.variable_declarations.insert(name);
                }
            }
        }
    }

    fn collect_exports(&mut self, program: &Program) {
        for statement in &program.statements {
            match statement {
                Statement::Export(decl) => match &decl.kind {
                    ExportKind::Declaration(inner) => {
                        if let Some((_, name)) = self.get_declaration_name(inner) {
                            self.exported_names.insert(name);
                        }
                    }
                    ExportKind::Named { specifiers, .. } => {
                        for spec in specifiers {
                            let local_id = spec.local.node;
                            let local_name = self.resolve_string(local_id);
                            self.exported_names.insert(local_name);
                        }
                    }
                    ExportKind::Default(_) => {
                        self.exported_names.insert("default".to_string());
                    }
                },
                _ => {}
            }
        }
    }

    fn collect_return_values_from_private_functions(&mut self, program: &Program) {
        // Only track returns from PRIVATE functions
        // Returns from exported functions don't prevent hoisting (they're part of the public API)
        for statement in &program.statements {
            match statement {
                Statement::Function(func) => {
                    // Track returns from private functions
                    self.walk_statements_for_returns_skip_functions(&func.body.statements);
                }
                Statement::Return(ret) => {
                    // Module-level return
                    for value in &ret.values {
                        if let ExpressionKind::Identifier(ident) = &value.kind {
                            let name = self.resolve_string(*ident);
                            // Track ALL returns (variables, classes, enums, functions)
                            self.return_values.insert(name);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn walk_statements_for_returns_skip_functions(&mut self, statements: &[Statement]) {
        for statement in statements {
            match statement {
                Statement::Return(ret) => {
                    for value in &ret.values {
                        if let ExpressionKind::Identifier(ident) = &value.kind {
                            let name = self.resolve_string(*ident);
                            // Track ALL returns (variables, classes, enums, functions)
                            self.return_values.insert(name);
                        }
                    }
                }
                Statement::If(if_stmt) => {
                    self.walk_statements_for_returns_skip_functions(&if_stmt.then_block.statements);
                    for elseif in &if_stmt.else_ifs {
                        self.walk_statements_for_returns_skip_functions(&elseif.block.statements);
                    }
                    if let Some(else_block) = &if_stmt.else_block {
                        self.walk_statements_for_returns_skip_functions(&else_block.statements);
                    }
                }
                Statement::While(while_stmt) => {
                    self.walk_statements_for_returns_skip_functions(&while_stmt.body.statements);
                }
                Statement::For(for_stmt) => match **for_stmt {
                    typedlua_parser::ast::statement::ForStatement::Numeric(ref num) => {
                        self.walk_statements_for_returns_skip_functions(&num.body.statements);
                    }
                    typedlua_parser::ast::statement::ForStatement::Generic(ref generic) => {
                        self.walk_statements_for_returns_skip_functions(&generic.body.statements);
                    }
                },
                Statement::Repeat(repeat_stmt) => {
                    self.walk_statements_for_returns_skip_functions(&repeat_stmt.body.statements);
                }
                Statement::Block(block) => {
                    self.walk_statements_for_returns_skip_functions(&block.statements);
                }
                _ => {}
            }
        }
    }

    fn find_hoistable_declarations(&self, program: &Program) -> HoistableDeclarations {
        let mut hoistable = HoistableDeclarations::new();

        for statement in &program.statements {
            self.check_declaration_hoistability(statement, &mut hoistable);
        }

        hoistable
    }

    fn check_declaration_hoistability(
        &self,
        statement: &Statement,
        hoistable: &mut HoistableDeclarations,
    ) {
        match statement {
            Statement::Function(decl) => {
                let name = self.resolve_string(decl.name.node);
                if !self.is_exported_by_name(&name) && self.can_hoist_function(decl, &name) {
                    hoistable.functions.insert(name);
                }
            }
            Statement::Variable(decl) => {
                if let Pattern::Identifier(ident) = &decl.pattern {
                    let name = self.resolve_string(ident.node);
                    if !self.is_exported_by_name(&name) && self.can_hoist_variable(decl, &name) {
                        hoistable.variables.insert(name);
                    }
                }
            }
            Statement::Class(decl) => {
                let name = self.resolve_string(decl.name.node);
                if !self.is_exported_by_name(&name) && self.can_hoist_class(decl, &name) {
                    hoistable.classes.insert(name);
                }
            }
            Statement::Enum(decl) => {
                let name = self.resolve_string(decl.name.node);
                if !self.is_exported_by_name(&name) && self.can_hoist_enum(decl, &name) {
                    hoistable.enums.insert(name);
                }
            }
            Statement::Export(export_decl) => {
                // Check the inner declaration (though it will be marked as exported)
                if let ExportKind::Declaration(inner) = &export_decl.kind {
                    self.check_declaration_hoistability(inner, hoistable);
                }
            }
            _ => {}
        }
    }

    fn is_exported_by_name(&self, name: &str) -> bool {
        self.exported_names.contains(name)
    }

    fn can_hoist_function(
        &self,
        decl: &typedlua_parser::ast::statement::FunctionDeclaration,
        name: &str,
    ) -> bool {
        // Only concern: can the function be relocated to a higher scope?
        // Can't hoist if the function itself returns other module-locals
        // (losing access to them at the higher scope would break the function)
        !self.function_returns_any_local(&decl.body.statements)
    }

    fn function_returns_any_local(&self, statements: &[Statement]) -> bool {
        for statement in statements {
            match statement {
                Statement::Return(ret) => {
                    for value in &ret.values {
                        if let ExpressionKind::Identifier(ident) = &value.kind {
                            let name = self.resolve_string(*ident);
                            if self.module_locals.contains(&name) {
                                return true;
                            }
                        }
                    }
                }
                Statement::If(if_stmt) => {
                    if self.function_returns_any_local(&if_stmt.then_block.statements) {
                        return true;
                    }
                    for elseif in &if_stmt.else_ifs {
                        if self.function_returns_any_local(&elseif.block.statements) {
                            return true;
                        }
                    }
                    if let Some(else_block) = &if_stmt.else_block {
                        if self.function_returns_any_local(&else_block.statements) {
                            return true;
                        }
                    }
                }
                Statement::While(while_stmt) => {
                    if self.function_returns_any_local(&while_stmt.body.statements) {
                        return true;
                    }
                }
                Statement::For(for_stmt) => {
                    let body = match **for_stmt {
                        typedlua_parser::ast::statement::ForStatement::Numeric(ref num) => {
                            &num.body.statements
                        }
                        typedlua_parser::ast::statement::ForStatement::Generic(ref generic) => {
                            &generic.body.statements
                        }
                    };
                    if self.function_returns_any_local(body) {
                        return true;
                    }
                }
                Statement::Repeat(repeat_stmt) => {
                    if self.function_returns_any_local(&repeat_stmt.body.statements) {
                        return true;
                    }
                }
                Statement::Block(block) => {
                    if self.function_returns_any_local(&block.statements) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn can_hoist_variable(&self, decl: &VariableDeclaration, name: &str) -> bool {
        match decl.kind {
            VariableKind::Const => {
                // Rule 1: Can't hoist if initializer references other module-local declarations
                if self.initializer_references_any_local(&decl.initializer, name) {
                    return false;
                }

                // Rule 2: Can't hoist if returned from any function
                if self.returns_local_by_name(name) {
                    return false;
                }

                // Rule 3: Can't hoist if initializer is a function expression
                if matches!(decl.initializer.kind, ExpressionKind::Function(_)) {
                    return false;
                }

                // Rule 4: Can't hoist if initializer is a complex type (object or array)
                // These are typically returned or have external dependencies
                if matches!(decl.initializer.kind, ExpressionKind::Object(_) | ExpressionKind::Array(_)) {
                    return false;
                }

                true
            }
            VariableKind::Local => false,
        }
    }

    fn returns_local_by_name(&self, name: &str) -> bool {
        self.return_values.contains(name)
    }

    fn initializer_references_any_local(&self, expr: &Expression, self_name: &str) -> bool {
        self.walk_expression_checking_any_local(expr, self_name)
    }

    fn walk_expression_checking_any_local(&self, expr: &Expression, self_name: &str) -> bool {
        match &expr.kind {
            ExpressionKind::Identifier(ident) => {
                let name = self.resolve_string(*ident);
                // Check if this identifier refers to a module-local (other than self)
                name != self_name && self.module_locals.contains(&name)
            }
            ExpressionKind::Object(props) => {
                for prop in props {
                    match prop {
                        ObjectProperty::Property { value, .. } => {
                            if self.walk_expression_checking_any_local(value, self_name) {
                                return true;
                            }
                        }
                        ObjectProperty::Computed { key, value, .. } => {
                            if self.walk_expression_checking_any_local(key, self_name)
                                || self.walk_expression_checking_any_local(value, self_name)
                            {
                                return true;
                            }
                        }
                        ObjectProperty::Spread { value, .. } => {
                            if self.walk_expression_checking_any_local(value, self_name) {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            ExpressionKind::Array(elements) => {
                for elem in elements {
                    match elem {
                        ArrayElement::Expression(expr) => {
                            if self.walk_expression_checking_any_local(expr, self_name) {
                                return true;
                            }
                        }
                        ArrayElement::Spread(expr) => {
                            if self.walk_expression_checking_any_local(expr, self_name) {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            ExpressionKind::Function(func) => {
                self.walk_statements_checking_any_local(&func.body.statements, self_name)
            }
            ExpressionKind::MethodCall(_, _, args, _) => {
                for arg in args {
                    if self.walk_expression_checking_any_local(&arg.value, self_name) {
                        return true;
                    }
                }
                false
            }
            ExpressionKind::Call(_, args, _) => {
                for arg in args {
                    if self.walk_expression_checking_any_local(&arg.value, self_name) {
                        return true;
                    }
                }
                false
            }
            ExpressionKind::Binary(_, left, right) => {
                self.walk_expression_checking_any_local(left, self_name)
                    || self.walk_expression_checking_any_local(right, self_name)
            }
            ExpressionKind::Unary(_, operand) => {
                self.walk_expression_checking_any_local(operand, self_name)
            }
            ExpressionKind::Parenthesized(inner) => {
                self.walk_expression_checking_any_local(inner, self_name)
            }
            ExpressionKind::TypeAssertion(inner, _) => {
                self.walk_expression_checking_any_local(inner, self_name)
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.walk_expression_checking_any_local(cond, self_name)
                    || self.walk_expression_checking_any_local(then_expr, self_name)
                    || self.walk_expression_checking_any_local(else_expr, self_name)
            }
            ExpressionKind::New(new_expr, args, _) => {
                if self.walk_expression_checking_any_local(new_expr, self_name) {
                    return true;
                }
                for arg in args {
                    if self.walk_expression_checking_any_local(&arg.value, self_name) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn walk_statements_checking_any_local(&self, statements: &[Statement], self_name: &str) -> bool {
        for statement in statements {
            match statement {
                Statement::Return(ret) => {
                    for value in &ret.values {
                        if self.walk_expression_checking_any_local(value, self_name) {
                            return true;
                        }
                    }
                }
                Statement::If(if_stmt) => {
                    if self.walk_statements_checking_any_local(&if_stmt.then_block.statements, self_name)
                    {
                        return true;
                    }
                    for elseif in &if_stmt.else_ifs {
                        if self.walk_statements_checking_any_local(&elseif.block.statements, self_name) {
                            return true;
                        }
                    }
                    if let Some(else_block) = &if_stmt.else_block {
                        if self.walk_statements_checking_any_local(&else_block.statements, self_name) {
                            return true;
                        }
                    }
                }
                Statement::While(while_stmt) => {
                    if self.walk_statements_checking_any_local(&while_stmt.body.statements, self_name) {
                        return true;
                    }
                }
                Statement::Block(block) => {
                    if self.walk_statements_checking_any_local(&block.statements, self_name) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn can_hoist_class(
        &self,
        _decl: &typedlua_parser::ast::statement::ClassDeclaration,
        name: &str,
    ) -> bool {
        // Can't hoist if the class is returned directly from a private function
        !self.returns_local_by_name(name)
    }

    fn can_hoist_enum(
        &self,
        _decl: &typedlua_parser::ast::statement::EnumDeclaration,
        name: &str,
    ) -> bool {
        // Can't hoist if the enum is returned directly from a private function
        !self.returns_local_by_name(name)
    }


    fn get_declaration_name(&self, stmt: &Statement) -> Option<(StringId, String)> {
        match stmt {
            Statement::Function(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Class(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Enum(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Interface(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::TypeAlias(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Variable(decl) => {
                if let Pattern::Identifier(ident) = &decl.pattern {
                    let id = ident.node;
                    let name = self.resolve_string(id);
                    Some((id, name))
                } else {
                    None
                }
            }
            Statement::Export(export_decl) => {
                // Unwrap export declarations to get the inner declaration
                if let ExportKind::Declaration(inner) = &export_decl.kind {
                    self.get_declaration_name(inner)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn resolve_string(&self, id: StringId) -> String {
        self.interner.resolve(id).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::CollectingDiagnosticHandler;
    use std::sync::Arc;
    use typedlua_parser::lexer::Lexer;
    use typedlua_parser::parser::Parser;

    fn create_program(
        source: &str,
        interner: &StringInterner,
        common: &typedlua_parser::string_interner::CommonIdentifiers,
    ) -> Program {
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(source, handler.clone(), interner);
        let tokens = lexer.tokenize().expect("Lexing failed");
        let mut parser = Parser::new(tokens, handler, interner, common);
        parser.parse().expect("Parsing failed")
    }

    #[test]
    fn test_private_function_can_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            function helper(a, b)
                return a + b
            end

            export function main()
                return helper(1, 2)
            end
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(hoistable.functions.contains("helper"));
        assert!(!hoistable.functions.contains("main"));
    }

    #[test]
    fn test_exported_function_cannot_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            export function add(a, b)
                return a + b
            end
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.functions.contains("add"));
    }

    #[test]
    fn test_function_returning_local_cannot_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            function getHelper()
                return helper
            end

            function helper()
                return 42
            end
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.functions.contains("getHelper"));
        assert!(hoistable.functions.contains("helper"));
    }

    #[test]
    fn test_private_variable_can_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const CONSTANT = 42

            export function getValue()
                return CONSTANT
            end
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(hoistable.variables.contains("CONSTANT"));
    }

    #[test]
    fn test_exported_variable_cannot_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            export const PI = 3.14
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("PI"));
    }

    #[test]
    fn test_variable_returned_cannot_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            function getValue()
                return VALUE
            end

            const VALUE = 42
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("VALUE"));
    }

    #[test]
    fn test_private_class_can_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            class HelperClass {
                value: number
            }

            export function create()
                return HelperClass.new(42)
            end
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(hoistable.classes.contains("HelperClass"));
    }

    #[test]
    fn test_exported_class_cannot_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            export class MyClass {}
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.classes.contains("MyClass"));
    }

    #[test]
    fn test_class_returned_cannot_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            function getClass()
                return MyClass
            end

            class MyClass {}
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.classes.contains("MyClass"));
    }

    #[test]
    fn test_private_enum_can_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            enum Status {
                Active = 1,
                Inactive = 2
            }

            export function getStatus(): number
                return Status.Active
            end
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(hoistable.enums.contains("Status"));
    }

    #[test]
    fn test_exported_enum_cannot_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            export enum Color {
                Red = 1,
                Green = 2,
                Blue = 3
            }
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.enums.contains("Color"));
    }

    #[test]
    fn test_enum_returned_cannot_hoist() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            function getEnum()
                return MyEnum
            end

            enum MyEnum {
                A = 1,
                B = 2
            }
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.enums.contains("MyEnum"));
    }

    #[test]
    fn test_mixed_declarations() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const internal = 42
            function internalFunc() return 1 end
            class InternalClass {}
            enum InternalEnum { A = 1 }

            export const exposed = 1
            export function exposedFunc() return 2 end
            export class ExposedClass {}
            export enum ExposedEnum { B = 1 }
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(hoistable.variables.contains("internal"));
        assert!(hoistable.functions.contains("internalFunc"));
        assert!(hoistable.classes.contains("InternalClass"));
        assert!(hoistable.enums.contains("InternalEnum"));

        assert!(!hoistable.variables.contains("exposed"));
        assert!(!hoistable.functions.contains("exposedFunc"));
        assert!(!hoistable.classes.contains("ExposedClass"));
        assert!(!hoistable.enums.contains("ExposedEnum"));
    }

    #[test]
    fn test_named_export() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const value = 42
            const other = 100
            export { value }
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("value"));
        assert!(hoistable.variables.contains("other"));
    }

    #[test]
    fn test_variable_init_with_local_ref() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const helper = 42
            const obj = { value = helper }
            export function getObj() return obj end
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("obj"));
        assert!(hoistable.variables.contains("helper"));
    }

    #[test]
    fn test_variable_init_with_table() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const obj = { value = 42, nested = { inner = 100 } }
            export function getObj() return obj end
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("obj"));
    }

    #[test]
    fn test_variable_init_with_function() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            const callback = function() return 1 end
            export function run() return callback() end
        "#;
        let program = create_program(source, &interner, &common);
        let hoistable = EscapeAnalysis::analyze(&program, &interner);

        assert!(!hoistable.variables.contains("callback"));
    }
}
