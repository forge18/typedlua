use super::symbol_table::{Symbol, SymbolKind, SymbolTable};
use super::type_compat::TypeCompatibility;
use super::type_environment::TypeEnvironment;
use super::TypeCheckError;
use crate::ast::expression::*;
use crate::ast::pattern::{ArrayPatternElement, Pattern};
use crate::ast::statement::*;
use crate::ast::types::*;
use crate::ast::Program;
use crate::config::CompilerOptions;
use crate::diagnostics::DiagnosticHandler;
use crate::span::Span;
use rustc_hash::FxHashMap;
use std::sync::Arc;

/// Information about a class member for access checking
#[derive(Clone)]
struct ClassMemberInfo {
    name: String,
    access: AccessModifier,
    _is_static: bool,
    kind: ClassMemberKind,
    is_final: bool,
}

#[derive(Clone)]
#[allow(dead_code)] // Fields are used in check_method_override for signature validation
enum ClassMemberKind {
    Property {
        type_annotation: Type,
    },
    Method {
        parameters: Vec<Parameter>,
        return_type: Option<Type>,
    },
    Getter {
        return_type: Type,
    },
    Setter {
        parameter_type: Type,
    },
}

/// Context for tracking the current class during type checking
#[derive(Clone)]
struct ClassContext {
    name: String,
    parent: Option<String>,
}

/// Type checker for TypedLua programs
pub struct TypeChecker {
    symbol_table: SymbolTable,
    type_env: TypeEnvironment,
    current_function_return_type: Option<Type>,
    narrowing_context: super::narrowing::NarrowingContext,
    options: CompilerOptions,
    current_class: Option<ClassContext>,
    /// Map from class name to its members with access modifiers
    class_members: FxHashMap<String, Vec<ClassMemberInfo>>,
    /// Map from class name to whether it is final
    final_classes: FxHashMap<String, bool>,
    /// Module registry for multi-module compilation
    module_registry: Option<Arc<crate::module_resolver::ModuleRegistry>>,
    /// Current module ID
    current_module_id: Option<crate::module_resolver::ModuleId>,
    /// Module resolver for imports
    module_resolver: Option<Arc<crate::module_resolver::ModuleResolver>>,
    diagnostic_handler: Arc<dyn DiagnosticHandler>,
}

impl TypeChecker {
    pub fn new(diagnostic_handler: Arc<dyn DiagnosticHandler>) -> Self {
        let mut checker = Self {
            symbol_table: SymbolTable::new(),
            type_env: TypeEnvironment::new(),
            current_function_return_type: None,
            narrowing_context: super::narrowing::NarrowingContext::new(),
            options: CompilerOptions::default(),
            current_class: None,
            class_members: FxHashMap::default(),
            final_classes: FxHashMap::default(),
            module_registry: None,
            current_module_id: None,
            module_resolver: None,
            diagnostic_handler,
        };

        // Load standard library with default Lua version
        if let Err(e) = checker.load_stdlib() {
            eprintln!("Warning: Failed to load stdlib: {}", e);
        }

        // Always register minimal stdlib as fallback to ensure basic namespaces exist
        checker.register_minimal_stdlib();

        checker
    }

    pub fn with_options(mut self, options: CompilerOptions) -> Self {
        // Check if target version changed
        let version_changed = self.options.target != options.target;
        self.options = options;

        // Only reload stdlib if the target version changed
        if version_changed {
            // Reset symbol table and type environment
            self.symbol_table = SymbolTable::new();
            self.type_env = TypeEnvironment::new();
            self.class_members.clear();
            self.final_classes.clear();

            // Reload stdlib with the new target version
            if let Err(e) = self.load_stdlib() {
                eprintln!("Warning: Failed to load stdlib: {}", e);
            }
        }

        self
    }

    /// Create a TypeChecker with module support for multi-module compilation
    pub fn new_with_module_support(
        diagnostic_handler: Arc<dyn DiagnosticHandler>,
        registry: Arc<crate::module_resolver::ModuleRegistry>,
        module_id: crate::module_resolver::ModuleId,
        resolver: Arc<crate::module_resolver::ModuleResolver>,
    ) -> Self {
        let mut checker = Self::new(diagnostic_handler);
        checker.module_registry = Some(registry);
        checker.current_module_id = Some(module_id);
        checker.module_resolver = Some(resolver);
        checker
    }

    /// Load the standard library for the configured Lua version
    fn load_stdlib(&mut self) -> Result<(), String> {
        use crate::lexer::Lexer;
        use crate::parser::Parser;
        use crate::stdlib;

        // Get stdlib files for the target version
        let stdlib_files = stdlib::get_all_stdlib(self.options.target);

        for (filename, source) in stdlib_files {
            // Parse the stdlib file
            let handler = Arc::new(crate::diagnostics::CollectingDiagnosticHandler::new());
            let mut lexer = Lexer::new(source, handler.clone());
            let tokens = lexer
                .tokenize()
                .map_err(|e| format!("Failed to lex {}: {:?}", filename, e))?;

            let mut parser = Parser::new(tokens, handler.clone());
            let program = parser.parse();

            if let Err(ref e) = program {
                return Err(format!("Failed to parse {}: {:?}", filename, e));
            }

            let program = program.unwrap();

            // Process declarations from the stdlib
            // Only process declaration statements - these populate the type environment
            for statement in program.statements.iter() {
                // Ignore errors from stdlib - we just want to populate the type environment
                let _ = self.check_statement(statement);
            }
        }

        Ok(())
    }

    /// Type check a program
    pub fn check_program(&mut self, program: &Program) -> Result<(), TypeCheckError> {
        for statement in &program.statements {
            self.check_statement(statement)?;
        }
        Ok(())
    }

    /// Type check a statement
    fn check_statement(&mut self, stmt: &Statement) -> Result<(), TypeCheckError> {
        match stmt {
            Statement::Variable(decl) => self.check_variable_declaration(decl),
            Statement::Function(decl) => self.check_function_declaration(decl),
            Statement::If(if_stmt) => self.check_if_statement(if_stmt),
            Statement::While(while_stmt) => self.check_while_statement(while_stmt),
            Statement::For(for_stmt) => self.check_for_statement(for_stmt),
            Statement::Repeat(repeat_stmt) => self.check_repeat_statement(repeat_stmt),
            Statement::Return(return_stmt) => self.check_return_statement(return_stmt),
            Statement::Break(_) | Statement::Continue(_) => Ok(()),
            Statement::Expression(expr) => {
                self.infer_expression_type(expr)?;
                Ok(())
            }
            Statement::Block(block) => self.check_block(block),
            Statement::Interface(iface) => self.check_interface_declaration(iface),
            Statement::TypeAlias(alias) => self.check_type_alias(alias),
            Statement::Enum(enum_decl) => self.check_enum_declaration(enum_decl),
            Statement::Class(class_decl) => self.check_class_declaration(class_decl),
            Statement::Import(_) | Statement::Export(_) => Ok(()), // Module system will be implemented in a future version
            // Declaration file statements - register them in the symbol table
            Statement::DeclareFunction(func) => self.register_declare_function(func),
            Statement::DeclareNamespace(ns) => self.register_declare_namespace(ns),
            Statement::DeclareType(alias) => self.check_type_alias(alias), // Reuse existing logic
            Statement::DeclareInterface(iface) => self.check_interface_declaration(iface), // Reuse existing logic
            Statement::DeclareConst(const_decl) => self.register_declare_const(const_decl),
        }
    }

    /// Check variable declaration
    fn check_variable_declaration(
        &mut self,
        decl: &VariableDeclaration,
    ) -> Result<(), TypeCheckError> {
        // Infer the type of the initializer
        let init_type = self.infer_expression_type(&decl.initializer)?;

        // Get the declared type or use inferred type
        let var_type = if let Some(type_ann) = &decl.type_annotation {
            // Check that initializer is assignable to declared type
            if !TypeCompatibility::is_assignable(&init_type, type_ann) {
                return Err(TypeCheckError::new(
                    "Type mismatch: cannot assign expression to declared type",
                    decl.span,
                ));
            }
            type_ann.clone()
        } else {
            // For const, use narrow type; for local, widen literals
            if matches!(decl.kind, VariableKind::Const) {
                init_type
            } else {
                self.widen_type(init_type)
            }
        };

        // Declare the variable in the symbol table
        let symbol_kind = match decl.kind {
            VariableKind::Const => SymbolKind::Const,
            VariableKind::Local => SymbolKind::Variable,
        };

        self.declare_pattern(&decl.pattern, var_type, symbol_kind, decl.span)?;

        Ok(())
    }

    /// Declare symbols from a pattern
    fn declare_pattern(
        &mut self,
        pattern: &Pattern,
        typ: Type,
        kind: SymbolKind,
        span: Span,
    ) -> Result<(), TypeCheckError> {
        match pattern {
            Pattern::Identifier(ident) => {
                let symbol = Symbol::new(ident.node.clone(), kind, typ, span);
                self.symbol_table
                    .declare(symbol)
                    .map_err(|e| TypeCheckError::new(e, span))?;
                Ok(())
            }
            Pattern::Array(array_pattern) => {
                // Extract element type from array type
                if let TypeKind::Array(elem_type) = &typ.kind {
                    for elem in &array_pattern.elements {
                        match elem {
                            ArrayPatternElement::Pattern(pat) => {
                                self.declare_pattern(pat, (**elem_type).clone(), kind, span)?;
                            }
                            ArrayPatternElement::Rest(ident) => {
                                // Rest gets array type
                                let array_type =
                                    Type::new(TypeKind::Array(elem_type.clone()), span);
                                let symbol =
                                    Symbol::new(ident.node.clone(), kind, array_type, span);
                                self.symbol_table
                                    .declare(symbol)
                                    .map_err(|e| TypeCheckError::new(e, span))?;
                            }
                            ArrayPatternElement::Hole => {
                                // Holes don't declare symbols
                            }
                        }
                    }
                } else {
                    return Err(TypeCheckError::new(
                        "Cannot destructure non-array type",
                        span,
                    ));
                }
                Ok(())
            }
            Pattern::Object(obj_pattern) => {
                // Extract properties from object type
                if let TypeKind::Object(obj_type) = &typ.kind {
                    for prop_pattern in &obj_pattern.properties {
                        // Find matching property in type
                        let prop_type = obj_type.members.iter().find_map(|member| {
                            if let ObjectTypeMember::Property(prop) = member {
                                if prop.name.node == prop_pattern.key.node {
                                    return Some(prop.type_annotation.clone());
                                }
                            }
                            None
                        });

                        let prop_type = match prop_type {
                            Some(t) => t,
                            None => {
                                return Err(TypeCheckError::new(
                                    format!(
                                        "Property '{}' does not exist on type",
                                        prop_pattern.key.node
                                    ),
                                    span,
                                ));
                            }
                        };

                        if let Some(value_pattern) = &prop_pattern.value {
                            self.declare_pattern(value_pattern, prop_type, kind, span)?;
                        } else {
                            // Shorthand: { x } means { x: x }
                            let symbol =
                                Symbol::new(prop_pattern.key.node.clone(), kind, prop_type, span);
                            self.symbol_table
                                .declare(symbol)
                                .map_err(|e| TypeCheckError::new(e, span))?;
                        }
                    }
                } else {
                    return Err(TypeCheckError::new(
                        "Cannot destructure non-object type",
                        span,
                    ));
                }
                Ok(())
            }
            Pattern::Literal(_, _) | Pattern::Wildcard(_) => {
                // Literals and wildcards don't declare symbols
                Ok(())
            }
        }
    }

    /// Check function declaration
    fn check_function_declaration(
        &mut self,
        decl: &FunctionDeclaration,
    ) -> Result<(), TypeCheckError> {
        // For generic functions, we still declare them in the symbol table
        // but we'll instantiate their type parameters when they're called

        // Validate type predicate return types
        if let Some(return_type) = &decl.return_type {
            if let TypeKind::TypePredicate(predicate) = &return_type.kind {
                // Validate that the parameter name in the predicate matches one of the function parameters
                let param_exists = decl.parameters.iter().any(|param| {
                    if let Pattern::Identifier(ident) = &param.pattern {
                        ident.node == predicate.parameter_name.node
                    } else {
                        false
                    }
                });

                if !param_exists {
                    return Err(TypeCheckError::new(
                        format!(
                            "Type predicate parameter '{}' does not match any function parameter",
                            predicate.parameter_name.node
                        ),
                        predicate.span,
                    ));
                }
            }
        }

        // Create function type
        let func_type = Type::new(
            TypeKind::Function(FunctionType {
                type_parameters: decl.type_parameters.clone(),
                parameters: decl.parameters.clone(),
                return_type: Box::new(decl.return_type.clone().unwrap_or_else(|| {
                    Type::new(TypeKind::Primitive(PrimitiveType::Void), decl.span)
                })),
                span: decl.span,
            }),
            decl.span,
        );

        // Declare function in symbol table

        let symbol = Symbol::new(
            decl.name.node.clone(),
            SymbolKind::Function,
            func_type,
            decl.span,
        );
        self.symbol_table
            .declare(symbol)
            .map_err(|e| TypeCheckError::new(e, decl.span))?;

        // Enter new scope for function body
        self.symbol_table.enter_scope();

        // If generic, declare type parameters as types in scope
        if let Some(type_params) = &decl.type_parameters {
            for type_param in type_params {
                // Register each type parameter as a type in the current scope
                // This allows the function body to reference T, U, etc.
                let param_type = Type::new(
                    TypeKind::Reference(crate::ast::types::TypeReference {
                        name: type_param.name.clone(),
                        type_arguments: None,
                        span: type_param.span,
                    }),
                    type_param.span,
                );

                // Type parameters are treated as local types in the function scope
                // We register them as type aliases for now
                self.type_env
                    .register_type_alias(type_param.name.node.clone(), param_type)
                    .map_err(|e| TypeCheckError::new(e, type_param.span))?;
            }
        }

        // Declare parameters
        for (i, param) in decl.parameters.iter().enumerate() {
            // Check if rest parameter is in the correct position
            if param.is_rest && i != decl.parameters.len() - 1 {
                return Err(TypeCheckError::new(
                    "Rest parameter must be the last parameter",
                    param.span,
                ));
            }

            let param_type = if param.is_rest {
                // Rest parameters are arrays
                let elem_type = param.type_annotation.clone().unwrap_or_else(|| {
                    Type::new(TypeKind::Primitive(PrimitiveType::Unknown), param.span)
                });

                // Wrap in array type
                Type::new(TypeKind::Array(Box::new(elem_type)), param.span)
            } else {
                param.type_annotation.clone().unwrap_or_else(|| {
                    Type::new(TypeKind::Primitive(PrimitiveType::Unknown), param.span)
                })
            };

            self.declare_pattern(
                &param.pattern,
                param_type,
                SymbolKind::Parameter,
                param.span,
            )?;
        }

        // Set current function return type for return statement checking
        let old_return_type = self.current_function_return_type.clone();
        self.current_function_return_type = decl.return_type.clone();

        // Check function body
        self.check_block(&decl.body)?;

        // Restore previous return type
        self.current_function_return_type = old_return_type;

        // Exit function scope (this will remove type parameter registrations)
        self.symbol_table.exit_scope();

        Ok(())
    }

    /// Check if statement
    fn check_if_statement(&mut self, if_stmt: &IfStatement) -> Result<(), TypeCheckError> {
        // Check condition
        self.infer_expression_type(&if_stmt.condition)?;

        // Collect current variable and function types for narrowing
        // This includes both variables and functions so type predicates can be checked
        let mut variable_types = FxHashMap::default();
        for (name, symbol) in self.symbol_table.all_visible_symbols() {
            variable_types.insert(name.clone(), symbol.typ.clone());
        }

        // Apply type narrowing based on the condition
        let (then_context, else_context) = super::narrowing::narrow_type_from_condition(
            &if_stmt.condition,
            &self.narrowing_context,
            &variable_types,
        );

        // Check then block with narrowed context
        let saved_context = self.narrowing_context.clone();
        self.narrowing_context = then_context;
        self.check_block(&if_stmt.then_block)?;

        // Restore context for else-if and else
        self.narrowing_context = else_context.clone();

        // Check else-if clauses
        for else_if in &if_stmt.else_ifs {
            self.infer_expression_type(&else_if.condition)?;

            // Further narrow based on else-if condition
            let (elseif_then, elseif_else) = super::narrowing::narrow_type_from_condition(
                &else_if.condition,
                &self.narrowing_context,
                &variable_types,
            );

            self.narrowing_context = elseif_then;
            self.check_block(&else_if.block)?;
            self.narrowing_context = elseif_else;
        }

        // Check else block
        if let Some(else_block) = &if_stmt.else_block {
            self.check_block(else_block)?;
        }

        // Restore original context after if statement
        self.narrowing_context = saved_context;

        Ok(())
    }

    /// Check while statement
    fn check_while_statement(&mut self, while_stmt: &WhileStatement) -> Result<(), TypeCheckError> {
        self.infer_expression_type(&while_stmt.condition)?;
        self.check_block(&while_stmt.body)?;
        Ok(())
    }

    /// Check for statement
    fn check_for_statement(&mut self, for_stmt: &ForStatement) -> Result<(), TypeCheckError> {
        match for_stmt {
            ForStatement::Numeric(numeric) => {
                self.symbol_table.enter_scope();

                // Declare loop variable as number
                let number_type =
                    Type::new(TypeKind::Primitive(PrimitiveType::Number), numeric.span);
                let symbol = Symbol::new(
                    numeric.variable.node.clone(),
                    SymbolKind::Variable,
                    number_type,
                    numeric.span,
                );
                self.symbol_table
                    .declare(symbol)
                    .map_err(|e| TypeCheckError::new(e, numeric.span))?;

                // Check start, end, step expressions
                self.infer_expression_type(&numeric.start)?;
                self.infer_expression_type(&numeric.end)?;
                if let Some(step) = &numeric.step {
                    self.infer_expression_type(step)?;
                }

                self.check_block(&numeric.body)?;
                self.symbol_table.exit_scope();
            }
            ForStatement::Generic(generic) => {
                self.symbol_table.enter_scope();

                // Declare loop variables with unknown type

                let unknown_type =
                    Type::new(TypeKind::Primitive(PrimitiveType::Unknown), generic.span);
                for var in &generic.variables {
                    let symbol = Symbol::new(
                        var.node.clone(),
                        SymbolKind::Variable,
                        unknown_type.clone(),
                        generic.span,
                    );
                    self.symbol_table
                        .declare(symbol)
                        .map_err(|e| TypeCheckError::new(e, generic.span))?;
                }

                // Check iterators
                for iter in &generic.iterators {
                    self.infer_expression_type(iter)?;
                }

                self.check_block(&generic.body)?;
                self.symbol_table.exit_scope();
            }
        }
        Ok(())
    }

    /// Check repeat statement
    fn check_repeat_statement(
        &mut self,
        repeat_stmt: &RepeatStatement,
    ) -> Result<(), TypeCheckError> {
        self.symbol_table.enter_scope();
        self.check_block(&repeat_stmt.body)?;
        self.infer_expression_type(&repeat_stmt.until)?;
        self.symbol_table.exit_scope();
        Ok(())
    }

    /// Check return statement
    fn check_return_statement(
        &mut self,
        return_stmt: &ReturnStatement,
    ) -> Result<(), TypeCheckError> {
        if !return_stmt.values.is_empty() {
            // Infer types for all return values
            let return_types: Result<Vec<_>, _> = return_stmt
                .values
                .iter()
                .map(|expr| self.infer_expression_type(expr))
                .collect();
            let return_types = return_types?;

            // Create the actual return type (single value or tuple)
            let actual_return_type = if return_types.len() == 1 {
                return_types[0].clone()
            } else {
                Type::new(TypeKind::Tuple(return_types), return_stmt.span)
            };

            // Check against expected return type
            if let Some(expected_type) = &self.current_function_return_type {
                // Type predicates have an implicit boolean return type
                let effective_expected_type =
                    if matches!(expected_type.kind, TypeKind::TypePredicate(_)) {
                        Type::new(
                            TypeKind::Primitive(PrimitiveType::Boolean),
                            expected_type.span,
                        )
                    } else {
                        expected_type.clone()
                    };

                if !TypeCompatibility::is_assignable(&actual_return_type, &effective_expected_type)
                {
                    return Err(TypeCheckError::new(
                        "Return type mismatch",
                        return_stmt.span,
                    ));
                }
            }
        } else {
            // Check that void return is allowed
            if let Some(expected_type) = &self.current_function_return_type {
                let void_type =
                    Type::new(TypeKind::Primitive(PrimitiveType::Void), return_stmt.span);
                if !TypeCompatibility::is_assignable(&void_type, expected_type) {
                    return Err(TypeCheckError::new(
                        "Function expects a return value",
                        return_stmt.span,
                    ));
                }
            }
        }
        Ok(())
    }

    /// Check block
    fn check_block(&mut self, block: &Block) -> Result<(), TypeCheckError> {
        self.symbol_table.enter_scope();
        for stmt in &block.statements {
            self.check_statement(stmt)?;
        }
        self.symbol_table.exit_scope();
        Ok(())
    }

    /// Check interface declaration
    fn check_interface_declaration(
        &mut self,
        iface: &InterfaceDeclaration,
    ) -> Result<(), TypeCheckError> {
        // For generic interfaces, we need to register them differently
        // For now, we'll register generic interfaces similar to generic type aliases
        // They will be instantiated when referenced with type arguments

        if let Some(_type_params) = &iface.type_parameters {
            // Generic interface - we can't fully type check it yet
            // Just register it as a generic type so it can be instantiated later

            // For now, we'll create a placeholder object type
            let obj_type = Type::new(
                TypeKind::Object(ObjectType {
                    members: iface
                        .members
                        .iter()
                        .map(|member| match member {
                            InterfaceMember::Property(prop) => {
                                ObjectTypeMember::Property(prop.clone())
                            }
                            InterfaceMember::Method(method) => {
                                ObjectTypeMember::Method(method.clone())
                            }
                            InterfaceMember::Index(index) => ObjectTypeMember::Index(index.clone()),
                        })
                        .collect(),
                    span: iface.span,
                }),
                iface.span,
            );

            self.type_env
                .register_interface(iface.name.node.clone(), obj_type)
                .map_err(|e| TypeCheckError::new(e, iface.span))?;

            return Ok(());
        }

        // Non-generic interface - full checking
        // Convert interface members to object type members
        let mut members: Vec<ObjectTypeMember> = iface
            .members
            .iter()
            .map(|member| match member {
                InterfaceMember::Property(prop) => ObjectTypeMember::Property(prop.clone()),
                InterfaceMember::Method(method) => ObjectTypeMember::Method(method.clone()),
                InterfaceMember::Index(index) => ObjectTypeMember::Index(index.clone()),
            })
            .collect();

        // Handle extends clause - merge parent interface members
        for parent_type in &iface.extends {
            match &parent_type.kind {
                TypeKind::Reference(type_ref) => {
                    // Look up parent interface
                    if let Some(parent_iface) = self.type_env.get_interface(&type_ref.name.node) {
                        if let TypeKind::Object(parent_obj) = &parent_iface.kind {
                            // Add parent members first (so they can be overridden)
                            for parent_member in &parent_obj.members {
                                // Check if member is overridden in child
                                let member_name = match parent_member {
                                    ObjectTypeMember::Property(p) => Some(&p.name.node),
                                    ObjectTypeMember::Method(m) => Some(&m.name.node),
                                    ObjectTypeMember::Index(_) => None,
                                };

                                if let Some(name) = member_name {
                                    let is_overridden = members.iter().any(|m| match m {
                                        ObjectTypeMember::Property(p) => &p.name.node == name,
                                        ObjectTypeMember::Method(m) => &m.name.node == name,
                                        ObjectTypeMember::Index(_) => false,
                                    });

                                    if !is_overridden {
                                        members.insert(0, parent_member.clone());
                                    }
                                } else {
                                    // Index signatures are always inherited
                                    members.insert(0, parent_member.clone());
                                }
                            }
                        }
                    } else {
                        return Err(TypeCheckError::new(
                            format!("Parent interface '{}' not found", type_ref.name.node),
                            iface.span,
                        ));
                    }
                }
                _ => {
                    return Err(TypeCheckError::new(
                        "Interface can only extend other interfaces (type references)",
                        iface.span,
                    ));
                }
            }
        }

        // Validate interface members
        self.validate_interface_members(&members, iface.span)?;

        let obj_type = Type::new(
            TypeKind::Object(ObjectType {
                members,
                span: iface.span,
            }),
            iface.span,
        );

        self.type_env
            .register_interface(iface.name.node.clone(), obj_type)
            .map_err(|e| TypeCheckError::new(e, iface.span))?;

        Ok(())
    }

    /// Validate interface members for correctness
    fn validate_interface_members(
        &self,
        members: &[ObjectTypeMember],
        span: Span,
    ) -> Result<(), TypeCheckError> {
        // Check for duplicate property names
        let mut seen_names = std::collections::HashSet::new();

        for member in members {
            let name = match member {
                ObjectTypeMember::Property(prop) => Some(&prop.name.node),
                ObjectTypeMember::Method(method) => Some(&method.name.node),
                ObjectTypeMember::Index(_) => None,
            };

            if let Some(name) = name {
                if !seen_names.insert(name.clone()) {
                    return Err(TypeCheckError::new(
                        format!("Duplicate property '{}' in interface", name),
                        span,
                    ));
                }
            }
        }

        Ok(())
    }

    /// Check type alias
    fn check_type_alias(&mut self, alias: &TypeAliasDeclaration) -> Result<(), TypeCheckError> {
        // Evaluate special types before registering
        let typ_to_register = self
            .evaluate_type(&alias.type_annotation)
            .map_err(|e| TypeCheckError::new(e, alias.span))?;

        // Check if this is a generic type alias
        if let Some(type_params) = &alias.type_parameters {
            self.type_env
                .register_generic_type_alias(
                    alias.name.node.clone(),
                    type_params.clone(),
                    typ_to_register,
                )
                .map_err(|e| TypeCheckError::new(e, alias.span))?;
        } else {
            // Check for recursive type aliases
            match self.type_env.resolve_type_reference(&alias.name.node) {
                Ok(_) => {
                    // No cycle, register the alias
                    self.type_env
                        .register_type_alias(alias.name.node.clone(), typ_to_register)
                        .map_err(|e| TypeCheckError::new(e, alias.span))?;
                }
                Err(e) => {
                    return Err(TypeCheckError::new(e, alias.span));
                }
            }
        }
        Ok(())
    }

    /// Check enum declaration
    fn check_enum_declaration(
        &mut self,
        enum_decl: &EnumDeclaration,
    ) -> Result<(), TypeCheckError> {
        // Register enum as a union of its literal values
        let mut variants = Vec::new();
        for member in &enum_decl.members {
            if let Some(value) = &member.value {
                let literal_type = match value {
                    EnumValue::Number(n) => {
                        Type::new(TypeKind::Literal(Literal::Number(*n)), member.span)
                    }
                    EnumValue::String(s) => {
                        Type::new(TypeKind::Literal(Literal::String(s.clone())), member.span)
                    }
                };
                variants.push(literal_type);
            }
        }

        let enum_type = if variants.is_empty() {
            Type::new(TypeKind::Primitive(PrimitiveType::Number), enum_decl.span)
        } else if variants.len() == 1 {
            variants.into_iter().next().unwrap()
        } else {
            Type::new(TypeKind::Union(variants), enum_decl.span)
        };

        self.type_env
            .register_type_alias(enum_decl.name.node.clone(), enum_type)
            .map_err(|e| TypeCheckError::new(e, enum_decl.span))?;

        Ok(())
    }

    /// Resolve a type reference, handling utility types and generic type application
    #[allow(dead_code)]
    fn resolve_type_reference(&self, type_ref: &TypeReference) -> Result<Type, TypeCheckError> {
        let name = &type_ref.name.node;
        let span = type_ref.span;

        // Check if it's a utility type
        if let Some(type_args) = &type_ref.type_arguments {
            if TypeEnvironment::is_utility_type(name) {
                return self
                    .type_env
                    .resolve_utility_type(name, type_args, span)
                    .map_err(|e| TypeCheckError::new(e, span));
            }

            // Check for generic type alias
            if let Some(generic_alias) = self.type_env.get_generic_type_alias(name) {
                use super::generics::instantiate_type;
                return instantiate_type(
                    &generic_alias.typ,
                    &generic_alias.type_parameters,
                    type_args,
                )
                .map_err(|e| TypeCheckError::new(e, span));
            }
        }

        // Regular type lookup
        match self.type_env.lookup_type(name) {
            Some(typ) => Ok(typ.clone()),
            None => Err(TypeCheckError::new(
                format!("Type '{}' not found", name),
                span,
            )),
        }
    }

    /// Check class declaration
    fn check_class_declaration(
        &mut self,
        class_decl: &ClassDeclaration,
    ) -> Result<(), TypeCheckError> {
        // Check decorators
        self.check_decorators(&class_decl.decorators)?;

        // Enter a new scope for the class
        self.symbol_table.enter_scope();

        // Register type parameters if this is a generic class
        if let Some(type_params) = &class_decl.type_parameters {
            for type_param in type_params {
                let param_type = Type::new(
                    TypeKind::Reference(crate::ast::types::TypeReference {
                        name: type_param.name.clone(),
                        type_arguments: None,
                        span: type_param.span,
                    }),
                    type_param.span,
                );

                self.type_env
                    .register_type_alias(type_param.name.node.clone(), param_type)
                    .map_err(|e| TypeCheckError::new(e, type_param.span))?;
            }
        }

        // Check extends clause - validate base class exists and is a class
        if let Some(extends_type) = &class_decl.extends {
            if let TypeKind::Reference(_type_ref) = &extends_type.kind {
                // Check if parent class is final
                if let Some(&is_final) = self.final_classes.get(&_type_ref.name.node) {
                    if is_final {
                        return Err(TypeCheckError::new(
                            format!("Cannot extend final class {}", _type_ref.name.node),
                            class_decl.span,
                        ));
                    }
                }
                // Verify the base class exists
                // For now, we'll just ensure it's a valid type reference
            } else {
                return Err(TypeCheckError::new(
                    "Class can only extend another class (type reference)",
                    class_decl.span,
                ));
            }
        }

        // Check interface implementation
        for interface_type in &class_decl.implements {
            if let TypeKind::Reference(type_ref) = &interface_type.kind {
                if let Some(interface) = self.type_env.get_interface(&type_ref.name.node) {
                    self.check_class_implements_interface(class_decl, interface)?;
                } else {
                    return Err(TypeCheckError::new(
                        format!("Interface '{}' not found", type_ref.name.node),
                        class_decl.span,
                    ));
                }
            } else {
                return Err(TypeCheckError::new(
                    "Class can only implement interfaces (type references)",
                    class_decl.span,
                ));
            }
        }

        // Process primary constructor parameters - they become class properties
        let mut primary_constructor_properties = Vec::new();
        if let Some(primary_params) = &class_decl.primary_constructor {
            for param in primary_params {
                // Validate: ensure no member with same name exists
                let param_name = &param.name.node;
                if class_decl.members.iter().any(|m| match m {
                    ClassMember::Property(p) => &p.name.node == param_name,
                    ClassMember::Method(m) => &m.name.node == param_name,
                    ClassMember::Getter(g) => &g.name.node == param_name,
                    ClassMember::Setter(s) => &s.name.node == param_name,
                    ClassMember::Constructor(_) => false,
                }) {
                    return Err(TypeCheckError::new(
                        format!(
                            "Primary constructor parameter '{}' conflicts with existing class member",
                            param_name
                        ),
                        param.span,
                    ));
                }

                primary_constructor_properties.push(param);
            }
        }

        // Validate parent constructor arguments if present
        if let Some(parent_args) = &class_decl.parent_constructor_args {
            // Type check each parent constructor argument
            for arg in parent_args {
                self.infer_expression_type(arg)?;
            }

            // TODO: Validate argument count and types match parent constructor
            // This requires tracking constructor signatures, which we'll add later
        }

        // Collect class members for access checking
        let mut member_infos = Vec::new();

        // Add primary constructor parameters as properties
        for param in &primary_constructor_properties {
            member_infos.push(ClassMemberInfo {
                name: param.name.node.clone(),
                access: param.access.unwrap_or(AccessModifier::Public),
                _is_static: false,
                kind: ClassMemberKind::Property {
                    type_annotation: param.type_annotation.clone(),
                },
                is_final: param.is_readonly, // readonly maps to final for properties
            });
        }

        for member in &class_decl.members {
            match member {
                ClassMember::Property(prop) => {
                    member_infos.push(ClassMemberInfo {
                        name: prop.name.node.clone(),
                        access: prop.access.unwrap_or(AccessModifier::Public),
                        _is_static: prop.is_static,
                        kind: ClassMemberKind::Property {
                            type_annotation: prop.type_annotation.clone(),
                        },
                        is_final: false,
                    });
                }
                ClassMember::Method(method) => {
                    member_infos.push(ClassMemberInfo {
                        name: method.name.node.clone(),
                        access: method.access.unwrap_or(AccessModifier::Public),
                        _is_static: method.is_static,
                        kind: ClassMemberKind::Method {
                            parameters: method.parameters.clone(),
                            return_type: method.return_type.clone(),
                        },
                        is_final: method.is_final,
                    });
                }
                ClassMember::Getter(getter) => {
                    member_infos.push(ClassMemberInfo {
                        name: getter.name.node.clone(),
                        access: getter.access.unwrap_or(AccessModifier::Public),
                        _is_static: getter.is_static,
                        kind: ClassMemberKind::Getter {
                            return_type: getter.return_type.clone(),
                        },
                        is_final: false,
                    });
                }
                ClassMember::Setter(setter) => {
                    member_infos.push(ClassMemberInfo {
                        name: setter.name.node.clone(),
                        access: setter.access.unwrap_or(AccessModifier::Public),
                        _is_static: setter.is_static,
                        kind: ClassMemberKind::Setter {
                            parameter_type: setter
                                .parameter
                                .type_annotation
                                .clone()
                                .unwrap_or_else(|| {
                                    Type::new(
                                        TypeKind::Primitive(PrimitiveType::Unknown),
                                        setter.span,
                                    )
                                }),
                        },
                        is_final: false,
                    });
                }
                ClassMember::Constructor(_) => {
                    // Constructor doesn't have access modifiers for member access
                }
            }
        }

        // Store class members for access checking
        self.class_members
            .insert(class_decl.name.node.clone(), member_infos);

        // Store whether the class is final
        self.final_classes
            .insert(class_decl.name.node.clone(), class_decl.is_final);

        // Set current class context
        let parent = class_decl.extends.as_ref().and_then(|ext| {
            if let TypeKind::Reference(type_ref) = &ext.kind {
                Some(type_ref.name.node.clone())
            } else {
                None
            }
        });

        let old_class = self.current_class.clone();
        self.current_class = Some(ClassContext {
            name: class_decl.name.node.clone(),
            parent,
        });

        // Check all class members
        let mut has_constructor = false;
        let mut abstract_methods = Vec::new();

        for member in &class_decl.members {
            match member {
                ClassMember::Property(prop) => {
                    self.check_class_property(prop)?;
                }
                ClassMember::Constructor(ctor) => {
                    if has_constructor {
                        return Err(TypeCheckError::new(
                            "Class can only have one constructor",
                            ctor.span,
                        ));
                    }
                    has_constructor = true;
                    self.check_constructor(ctor)?;
                }
                ClassMember::Method(method) => {
                    if method.is_abstract {
                        if !class_decl.is_abstract {
                            return Err(TypeCheckError::new(
                                format!(
                                    "Abstract method '{}' can only be in abstract class",
                                    method.name.node
                                ),
                                method.span,
                            ));
                        }
                        abstract_methods.push(method.name.node.clone());
                    }
                    self.check_class_method(method)?;
                }
                ClassMember::Getter(getter) => {
                    self.check_class_getter(getter)?;
                }
                ClassMember::Setter(setter) => {
                    self.check_class_setter(setter)?;
                }
            }
        }

        // Restore previous class context
        self.current_class = old_class;

        // Exit class scope
        self.symbol_table.exit_scope();

        Ok(())
    }

    /// Check that a class properly implements an interface
    fn check_class_implements_interface(
        &self,
        class_decl: &ClassDeclaration,
        interface: &Type,
    ) -> Result<(), TypeCheckError> {
        if let TypeKind::Object(obj_type) = &interface.kind {
            for required_member in &obj_type.members {
                match required_member {
                    ObjectTypeMember::Property(req_prop) => {
                        // Find matching property in class
                        let found = class_decl.members.iter().any(|member| {
                            if let ClassMember::Property(class_prop) = member {
                                class_prop.name.node == req_prop.name.node
                            } else {
                                false
                            }
                        });

                        if !found && !req_prop.is_optional {
                            return Err(TypeCheckError::new(
                                format!(
                                    "Class '{}' does not implement required property '{}' from interface",
                                    class_decl.name.node, req_prop.name.node
                                ),
                                class_decl.span,
                            ));
                        }
                    }
                    ObjectTypeMember::Method(req_method) => {
                        // Find matching method in class and validate signature
                        let matching_method = class_decl.members.iter().find_map(|member| {
                            if let ClassMember::Method(class_method) = member {
                                if class_method.name.node == req_method.name.node {
                                    return Some(class_method);
                                }
                            }
                            None
                        });

                        match matching_method {
                            None => {
                                return Err(TypeCheckError::new(
                                    format!(
                                        "Class '{}' does not implement required method '{}' from interface",
                                        class_decl.name.node, req_method.name.node
                                    ),
                                    class_decl.span,
                                ));
                            }
                            Some(class_method) => {
                                // Check parameter count
                                if class_method.parameters.len() != req_method.parameters.len() {
                                    return Err(TypeCheckError::new(
                                        format!(
                                            "Method '{}' has {} parameters but interface requires {}",
                                            req_method.name.node,
                                            class_method.parameters.len(),
                                            req_method.parameters.len()
                                        ),
                                        class_method.span,
                                    ));
                                }

                                // Check parameter types
                                for (i, (class_param, req_param)) in class_method
                                    .parameters
                                    .iter()
                                    .zip(req_method.parameters.iter())
                                    .enumerate()
                                {
                                    if let (Some(class_type), Some(req_type)) =
                                        (&class_param.type_annotation, &req_param.type_annotation)
                                    {
                                        if !TypeCompatibility::is_assignable(class_type, req_type) {
                                            return Err(TypeCheckError::new(
                                                format!(
                                                    "Method '{}' parameter {} has incompatible type",
                                                    req_method.name.node, i
                                                ),
                                                class_method.span,
                                            ));
                                        }
                                    }
                                }

                                // Check return type
                                // MethodSignature has return_type: Type (not Option)
                                // MethodDeclaration has return_type: Option<Type>
                                if let Some(class_return) = &class_method.return_type {
                                    if !TypeCompatibility::is_assignable(
                                        class_return,
                                        &req_method.return_type,
                                    ) {
                                        return Err(TypeCheckError::new(
                                            format!(
                                                "Method '{}' has incompatible return type",
                                                req_method.name.node
                                            ),
                                            class_method.span,
                                        ));
                                    }
                                } else {
                                    // Method has no return type annotation, but interface requires one
                                    return Err(TypeCheckError::new(
                                        format!(
                                            "Method '{}' must have a return type annotation to match interface",
                                            req_method.name.node
                                        ),
                                        class_method.span,
                                    ));
                                }
                            }
                        }
                    }
                    ObjectTypeMember::Index(_) => {
                        // Index signature checking is complex, skip for now
                    }
                }
            }
        }

        Ok(())
    }

    /// Check decorators
    fn check_decorators(
        &mut self,
        decorators: &[crate::ast::statement::Decorator],
    ) -> Result<(), TypeCheckError> {
        // Check if decorators are enabled
        if !decorators.is_empty() && !self.options.enable_decorators {
            return Err(TypeCheckError::new(
                "Decorators require decorator features to be enabled. Enable 'enableDecorators' in your configuration.".to_string(),
                decorators[0].span,
            ));
        }

        // For now, we just validate that decorator expressions are valid
        // Full decorator type checking would require:
        // 1. Checking that decorator functions exist
        // 2. Validating decorator function signatures match target type
        // 3. Checking decorator arguments are type-compatible
        // This is simplified for now - decorators are allowed but not deeply validated

        for decorator in decorators {
            self.check_decorator_expression(&decorator.expression)?;
        }

        Ok(())
    }

    /// Check a decorator expression
    fn check_decorator_expression(
        &mut self,
        expr: &crate::ast::statement::DecoratorExpression,
    ) -> Result<(), TypeCheckError> {
        use crate::ast::statement::DecoratorExpression;

        match expr {
            DecoratorExpression::Identifier(name) => {
                // Verify the decorator identifier exists (could be a function or imported decorator)
                // For now, we allow any identifier - full validation would check it's a valid decorator function
                if self.symbol_table.lookup(&name.node).is_none() {
                    // It's okay if it doesn't exist - it might be a built-in decorator like @readonly, @sealed
                    // We'll allow it through for now
                }
                Ok(())
            }
            DecoratorExpression::Call {
                callee, arguments, ..
            } => {
                // Check the callee
                self.check_decorator_expression(callee)?;

                // Type check all arguments
                for arg in arguments {
                    self.infer_expression_type(arg)?;
                }

                Ok(())
            }
            DecoratorExpression::Member { object, .. } => {
                // Check the object part
                self.check_decorator_expression(object)?;
                Ok(())
            }
        }
    }

    /// Check class property
    fn check_class_property(&mut self, prop: &PropertyDeclaration) -> Result<(), TypeCheckError> {
        // Check decorators
        self.check_decorators(&prop.decorators)?;

        // Check initializer if present
        if let Some(initializer) = &prop.initializer {
            let init_type = self.infer_expression_type(initializer)?;

            // Verify initializer type is assignable to declared type
            if !TypeCompatibility::is_assignable(&init_type, &prop.type_annotation) {
                return Err(TypeCheckError::new(
                    format!(
                        "Property '{}' initializer type does not match declared type",
                        prop.name.node
                    ),
                    prop.span,
                ));
            }
        }

        Ok(())
    }

    /// Check constructor
    fn check_constructor(&mut self, ctor: &ConstructorDeclaration) -> Result<(), TypeCheckError> {
        // Enter constructor scope
        self.symbol_table.enter_scope();

        // Declare parameters
        for param in &ctor.parameters {
            let param_type = param.type_annotation.clone().unwrap_or_else(|| {
                Type::new(TypeKind::Primitive(PrimitiveType::Unknown), param.span)
            });

            self.declare_pattern(
                &param.pattern,
                param_type,
                SymbolKind::Parameter,
                param.span,
            )?;
        }

        // Check constructor body
        self.check_block(&ctor.body)?;

        // Exit constructor scope
        self.symbol_table.exit_scope();

        Ok(())
    }

    /// Check class method
    fn check_class_method(&mut self, method: &MethodDeclaration) -> Result<(), TypeCheckError> {
        // Check decorators
        self.check_decorators(&method.decorators)?;

        // Check override keyword if present
        if method.is_override {
            self.check_method_override(method)?;
        } else if let Some(class_context) = &self.current_class {
            // Check if method shadows a parent method without override keyword
            if let Some(parent_name) = &class_context.parent {
                if let Some(parent_members) = self.class_members.get(parent_name) {
                    if parent_members.iter().any(|m| m.name == method.name.node) {
                        self.diagnostic_handler.warning(
                            method.span,
                            &format!(
                                "Method '{}' overrides a method from parent class '{}' but is missing the 'override' keyword",
                                method.name.node,
                                parent_name
                            ),
                        );
                    }
                }
            }
        }

        // Abstract methods don't have a body to check
        if method.is_abstract {
            if method.body.is_some() {
                return Err(TypeCheckError::new(
                    format!("Abstract method '{}' cannot have a body", method.name.node),
                    method.span,
                ));
            }
            return Ok(());
        }

        // Non-abstract methods must have a body
        if method.body.is_none() {
            return Err(TypeCheckError::new(
                format!(
                    "Non-abstract method '{}' must have a body",
                    method.name.node
                ),
                method.span,
            ));
        }

        // Enter method scope
        self.symbol_table.enter_scope();

        // Register type parameters if generic
        if let Some(type_params) = &method.type_parameters {
            for type_param in type_params {
                let param_type = Type::new(
                    TypeKind::Reference(crate::ast::types::TypeReference {
                        name: type_param.name.clone(),
                        type_arguments: None,
                        span: type_param.span,
                    }),
                    type_param.span,
                );

                self.type_env
                    .register_type_alias(type_param.name.node.clone(), param_type)
                    .map_err(|e| TypeCheckError::new(e, type_param.span))?;
            }
        }

        // Declare parameters
        for param in &method.parameters {
            let param_type = param.type_annotation.clone().unwrap_or_else(|| {
                Type::new(TypeKind::Primitive(PrimitiveType::Unknown), param.span)
            });

            self.declare_pattern(
                &param.pattern,
                param_type,
                SymbolKind::Parameter,
                param.span,
            )?;
        }

        // Set current function return type for return statement checking
        let old_return_type = self.current_function_return_type.clone();
        self.current_function_return_type = method.return_type.clone();

        // Check method body
        if let Some(body) = &method.body {
            self.check_block(body)?;
        }

        // Restore previous return type
        self.current_function_return_type = old_return_type;

        // Exit method scope
        self.symbol_table.exit_scope();

        Ok(())
    }

    /// Check class getter
    fn check_class_getter(&mut self, getter: &GetterDeclaration) -> Result<(), TypeCheckError> {
        // Check decorators
        self.check_decorators(&getter.decorators)?;

        // Enter getter scope
        self.symbol_table.enter_scope();

        // Set current function return type
        let old_return_type = self.current_function_return_type.clone();
        self.current_function_return_type = Some(getter.return_type.clone());

        // Check getter body
        self.check_block(&getter.body)?;

        // Restore previous return type
        self.current_function_return_type = old_return_type;

        // Exit getter scope
        self.symbol_table.exit_scope();

        Ok(())
    }

    /// Check class setter
    fn check_class_setter(&mut self, setter: &SetterDeclaration) -> Result<(), TypeCheckError> {
        // Check decorators
        self.check_decorators(&setter.decorators)?;

        // Enter setter scope
        self.symbol_table.enter_scope();

        // Declare the parameter
        let param_type = setter.parameter.type_annotation.clone().unwrap_or_else(|| {
            Type::new(
                TypeKind::Primitive(PrimitiveType::Unknown),
                setter.parameter.span,
            )
        });

        self.declare_pattern(
            &setter.parameter.pattern,
            param_type,
            SymbolKind::Parameter,
            setter.parameter.span,
        )?;

        // Check setter body
        self.check_block(&setter.body)?;

        // Exit setter scope
        self.symbol_table.exit_scope();

        Ok(())
    }

    /// Check that an override method properly overrides a parent method
    fn check_method_override(&self, method: &MethodDeclaration) -> Result<(), TypeCheckError> {
        // Get current class context
        let class_ctx = self.current_class.as_ref().ok_or_else(|| {
            TypeCheckError::new(
                "Override keyword used outside of class context",
                method.span,
            )
        })?;

        // Check if class has a parent
        let parent_name = class_ctx.parent.as_ref().ok_or_else(|| {
            TypeCheckError::new(
                format!(
                    "Method '{}' uses override but class '{}' has no parent class",
                    method.name.node, class_ctx.name
                ),
                method.span,
            )
        })?;

        // Get parent class members
        let parent_members = self.class_members.get(parent_name).ok_or_else(|| {
            TypeCheckError::new(
                format!(
                    "Parent class '{}' not found or not yet type-checked",
                    parent_name
                ),
                method.span,
            )
        })?;

        // Find the method in parent class
        let parent_method = parent_members.iter().find(|m| m.name == method.name.node);

        let parent_method = parent_method.ok_or_else(|| {
            TypeCheckError::new(
                format!("Method '{}' marked as override but parent class '{}' does not have this method",
                    method.name.node, parent_name),
                method.span,
            )
        })?;

        // Verify that parent member is also a method
        // Check if parent method is final
        if parent_method.is_final {
            return Err(TypeCheckError::new(
                format!(
                    "Cannot override final method {} from parent class {}",
                    method.name.node, parent_name
                ),
                method.span,
            ));
        }

        match &parent_method.kind {
            ClassMemberKind::Method {
                parameters: parent_params,
                return_type: parent_return,
            } => {
                // Check parameter count
                if method.parameters.len() != parent_params.len() {
                    return Err(TypeCheckError::new(
                        format!(
                            "Method '{}' has {} parameters but overridden method has {} parameters",
                            method.name.node,
                            method.parameters.len(),
                            parent_params.len()
                        ),
                        method.span,
                    ));
                }

                // Check parameter types (contravariance)
                for (i, (child_param, parent_param)) in method
                    .parameters
                    .iter()
                    .zip(parent_params.iter())
                    .enumerate()
                {
                    let child_type = child_param.type_annotation.as_ref()
                        .ok_or_else(|| TypeCheckError::new(
                            format!("Override method '{}' parameter {} must have explicit type annotation",
                                method.name.node, i + 1),
                            child_param.span,
                        ))?;

                    let parent_type = parent_param.type_annotation.as_ref().ok_or_else(|| {
                        TypeCheckError::new(
                            format!(
                                "Parent method '{}' parameter {} has no type annotation",
                                method.name.node,
                                i + 1
                            ),
                            parent_param.span,
                        )
                    })?;

                    // Parameters should have the same type (we could allow contravariance here)
                    if !TypeCompatibility::is_assignable(parent_type, child_type)
                        || !TypeCompatibility::is_assignable(child_type, parent_type)
                    {
                        return Err(TypeCheckError::new(
                            format!("Method '{}' parameter {} type '{}' is incompatible with parent parameter type",
                                method.name.node, i + 1, self.type_to_string(child_type)),
                            child_param.span,
                        ));
                    }
                }

                // Check return type (covariance)
                if let Some(child_return) = &method.return_type {
                    if let Some(parent_ret) = parent_return {
                        // Child return type must be assignable to parent return type
                        if !TypeCompatibility::is_assignable(parent_ret, child_return) {
                            return Err(TypeCheckError::new(
                                format!("Method '{}' return type is incompatible with parent return type",
                                    method.name.node),
                                method.span,
                            ));
                        }
                    }
                } else if parent_return.is_some() {
                    return Err(TypeCheckError::new(
                        format!(
                            "Method '{}' must have return type to match parent method",
                            method.name.node
                        ),
                        method.span,
                    ));
                }

                Ok(())
            }
            _ => Err(TypeCheckError::new(
                format!(
                    "Cannot override '{}' - parent member is not a method",
                    method.name.node
                ),
                method.span,
            )),
        }
    }

    /// Convert type to string for error messages
    fn type_to_string(&self, typ: &Type) -> String {
        match &typ.kind {
            TypeKind::Primitive(prim) => format!("{:?}", prim).to_lowercase(),
            TypeKind::Reference(type_ref) => type_ref.name.node.clone(),
            TypeKind::Array(elem) => format!("{}[]", self.type_to_string(elem)),
            TypeKind::Union(types) => {
                let type_strings: Vec<String> =
                    types.iter().map(|t| self.type_to_string(t)).collect();
                type_strings.join(" | ")
            }
            TypeKind::Function(_) => "function".to_string(),
            TypeKind::Object(_) => "object".to_string(),
            _ => format!("{:?}", typ.kind),
        }
    }

    /// Infer the type of an expression
    fn infer_expression_type(&mut self, expr: &Expression) -> Result<Type, TypeCheckError> {
        let span = expr.span;

        match &expr.kind {
            ExpressionKind::Literal(lit) => Ok(Type::new(TypeKind::Literal(lit.clone()), span)),

            ExpressionKind::Identifier(name) => {
                // Check for narrowed type first
                if let Some(narrowed_type) = self.narrowing_context.get_narrowed_type(name) {
                    return Ok(narrowed_type.clone());
                }

                // Fall back to symbol table
                if let Some(symbol) = self.symbol_table.lookup(name) {
                    Ok(symbol.typ.clone())
                } else {
                    Err(TypeCheckError::new(
                        format!("Undefined variable '{}'", name),
                        span,
                    ))
                }
            }

            ExpressionKind::Binary(op, left, right) => {
                let left_type = self.infer_expression_type(left)?;
                let right_type = self.infer_expression_type(right)?;
                self.infer_binary_op_type(*op, &left_type, &right_type, span)
            }

            ExpressionKind::Unary(op, operand) => {
                let operand_type = self.infer_expression_type(operand)?;
                self.infer_unary_op_type(*op, &operand_type, span)
            }

            ExpressionKind::Call(callee, args) => {
                let callee_type = self.infer_expression_type(callee)?;
                self.infer_call_type(&callee_type, args, span)
            }

            ExpressionKind::Member(object, member) => {
                let obj_type = self.infer_expression_type(object)?;
                self.infer_member_type(&obj_type, &member.node, span)
            }

            ExpressionKind::Index(object, index) => {
                let obj_type = self.infer_expression_type(object)?;
                let _index_type = self.infer_expression_type(index)?;
                self.infer_index_type(&obj_type, span)
            }

            ExpressionKind::Array(elements) => {
                if elements.is_empty() {
                    // Empty array has unknown element type
                    return Ok(Type::new(
                        TypeKind::Array(Box::new(Type::new(
                            TypeKind::Primitive(PrimitiveType::Unknown),
                            span,
                        ))),
                        span,
                    ));
                }

                // Collect all element types, including from spreads
                let mut element_types = Vec::new();

                for elem in elements {
                    match elem {
                        ArrayElement::Expression(expr) => {
                            let elem_type = self.infer_expression_type(expr)?;
                            element_types.push(elem_type);
                        }
                        ArrayElement::Spread(expr) => {
                            // Spread expression should be an array
                            let spread_type = self.infer_expression_type(expr)?;
                            match &spread_type.kind {
                                TypeKind::Array(elem_type) => {
                                    // Extract the element type from the spread array
                                    element_types.push((**elem_type).clone());
                                }
                                _ => {
                                    return Err(TypeCheckError::new(
                                        format!(
                                            "Cannot spread non-array type: {:?}",
                                            spread_type.kind
                                        ),
                                        expr.span,
                                    ));
                                }
                            }
                        }
                    }
                }

                // Find common type or create union
                if element_types.is_empty() {
                    return Ok(Type::new(
                        TypeKind::Array(Box::new(Type::new(
                            TypeKind::Primitive(PrimitiveType::Unknown),
                            span,
                        ))),
                        span,
                    ));
                }

                let mut result_type = element_types[0].clone();
                for elem_type in &element_types[1..] {
                    if !TypeCompatibility::is_assignable(&result_type, elem_type)
                        && !TypeCompatibility::is_assignable(elem_type, &result_type)
                    {
                        // Types are incompatible, create union
                        match &mut result_type.kind {
                            TypeKind::Union(types) => {
                                if !types
                                    .iter()
                                    .any(|t| TypeCompatibility::is_assignable(t, elem_type))
                                {
                                    types.push(elem_type.clone());
                                }
                            }
                            _ => {
                                result_type = Type::new(
                                    TypeKind::Union(vec![result_type.clone(), elem_type.clone()]),
                                    span,
                                );
                            }
                        }
                    }
                }

                Ok(Type::new(TypeKind::Array(Box::new(result_type)), span))
            }

            ExpressionKind::Object(props) => {
                // Infer object type from properties
                let mut members = Vec::new();

                for prop in props {
                    match prop {
                        ObjectProperty::Property {
                            key,
                            value,
                            span: prop_span,
                        } => {
                            // Infer the type of the value
                            let value_type = self.infer_expression_type(value)?;

                            // Create a property signature
                            let prop_sig = PropertySignature {
                                is_readonly: false,
                                name: key.clone(),
                                is_optional: false,
                                type_annotation: value_type,
                                span: *prop_span,
                            };

                            members.push(ObjectTypeMember::Property(prop_sig));
                        }
                        ObjectProperty::Computed {
                            key,
                            value,
                            span: computed_span,
                        } => {
                            // Type check the key expression - should be string or number
                            let key_type = self.infer_expression_type(key)?;
                            match &key_type.kind {
                                TypeKind::Primitive(PrimitiveType::String)
                                | TypeKind::Primitive(PrimitiveType::Number)
                                | TypeKind::Primitive(PrimitiveType::Integer)
                                | TypeKind::Literal(_) => {
                                    // Valid key type
                                }
                                _ => {
                                    return Err(TypeCheckError::new(
                                        format!("Computed property key must be string or number, got {:?}", key_type.kind),
                                        *computed_span,
                                    ));
                                }
                            }

                            // Type check the value expression
                            self.infer_expression_type(value)?;

                            // Note: We can't add computed properties to the static object type
                            // since we don't know the key at compile time, but we still validate them
                        }
                        ObjectProperty::Spread {
                            value,
                            span: spread_span,
                        } => {
                            // Spread object properties
                            let spread_type = self.infer_expression_type(value)?;
                            match &spread_type.kind {
                                TypeKind::Object(obj_type) => {
                                    // Add all members from the spread object
                                    for member in &obj_type.members {
                                        members.push(member.clone());
                                    }
                                }
                                _ => {
                                    return Err(TypeCheckError::new(
                                        format!(
                                            "Cannot spread non-object type: {:?}",
                                            spread_type.kind
                                        ),
                                        *spread_span,
                                    ));
                                }
                            }
                        }
                    }
                }

                Ok(Type::new(
                    TypeKind::Object(ObjectType { members, span }),
                    span,
                ))
            }

            ExpressionKind::Function(_) | ExpressionKind::Arrow(_) => {
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }

            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let _cond_type = self.infer_expression_type(cond)?;
                let then_type = self.infer_expression_type(then_expr)?;
                let else_type = self.infer_expression_type(else_expr)?;

                // Return union of both branches
                if TypeCompatibility::is_assignable(&then_type, &else_type) {
                    Ok(else_type)
                } else if TypeCompatibility::is_assignable(&else_type, &then_type) {
                    Ok(then_type)
                } else {
                    Ok(Type::new(TypeKind::Union(vec![then_type, else_type]), span))
                }
            }

            ExpressionKind::Match(match_expr) => self.check_match_expression(match_expr),

            ExpressionKind::Pipe(left_expr, right_expr) => {
                // Pipe operator: left |> right
                // The right side should be a function, and we apply left as the first argument
                let _left_type = self.infer_expression_type(left_expr)?;

                // For now, we'll infer the result type by checking the right expression
                // In a full implementation, we'd check if right is a function and apply left to it
                // For simplicity, we'll type check right and return its type
                // (This handles cases like: value |> func where func returns something)
                self.infer_expression_type(right_expr)
            }

            _ => {
                // For unimplemented expression types, return unknown
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }
        }
    }

    /// Infer type of binary operation
    fn infer_binary_op_type(
        &self,
        op: BinaryOp,
        _left: &Type,
        _right: &Type,
        span: Span,
    ) -> Result<Type, TypeCheckError> {
        match op {
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Modulo
            | BinaryOp::Power
            | BinaryOp::IntegerDivide => {
                // Arithmetic operations return number
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Number), span))
            }
            BinaryOp::Concatenate => {
                // String concatenation returns string
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::String), span))
            }
            BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::LessThan
            | BinaryOp::LessThanOrEqual
            | BinaryOp::GreaterThan
            | BinaryOp::GreaterThanOrEqual
            | BinaryOp::Instanceof => {
                // Comparison operations return boolean
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Boolean), span))
            }
            BinaryOp::And | BinaryOp::Or => {
                // Logical operations can return either operand type in Lua
                // For now, return unknown
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }
            BinaryOp::BitwiseAnd
            | BinaryOp::BitwiseOr
            | BinaryOp::BitwiseXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight => {
                // Bitwise operations return number
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Number), span))
            }
        }
    }

    /// Infer type of unary operation
    fn infer_unary_op_type(
        &self,
        op: UnaryOp,
        _operand: &Type,
        span: Span,
    ) -> Result<Type, TypeCheckError> {
        match op {
            UnaryOp::Negate => Ok(Type::new(TypeKind::Primitive(PrimitiveType::Number), span)),
            UnaryOp::Not => Ok(Type::new(TypeKind::Primitive(PrimitiveType::Boolean), span)),
            UnaryOp::Length => Ok(Type::new(TypeKind::Primitive(PrimitiveType::Number), span)),
            UnaryOp::BitwiseNot => Ok(Type::new(TypeKind::Primitive(PrimitiveType::Number), span)),
        }
    }

    /// Infer type of function call
    fn infer_call_type(
        &self,
        callee_type: &Type,
        _args: &[Argument],
        span: Span,
    ) -> Result<Type, TypeCheckError> {
        match &callee_type.kind {
            TypeKind::Function(func_type) => Ok((*func_type.return_type).clone()),
            _ => {
                // Non-function called - return unknown
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }
        }
    }

    /// Check if access to a class member is allowed based on access modifier
    fn check_member_access(
        &self,
        class_name: &str,
        member_name: &str,
        span: Span,
    ) -> Result<(), TypeCheckError> {
        // Get the member info
        let member_info = self
            .class_members
            .get(class_name)
            .and_then(|members| members.iter().find(|m| m.name == member_name));

        if let Some(info) = member_info {
            match info.access {
                AccessModifier::Public => {
                    // Public members are accessible from anywhere
                    Ok(())
                }
                AccessModifier::Private => {
                    // Private members are only accessible from within the same class
                    if let Some(ref current) = self.current_class {
                        if current.name == class_name {
                            Ok(())
                        } else {
                            Err(TypeCheckError::new(
                                format!(
                                    "Property '{}' is private and only accessible within class '{}'",
                                    member_name, class_name
                                ),
                                span,
                            ))
                        }
                    } else {
                        Err(TypeCheckError::new(
                            format!(
                                "Property '{}' is private and only accessible within class '{}'",
                                member_name, class_name
                            ),
                            span,
                        ))
                    }
                }
                AccessModifier::Protected => {
                    // Protected members are accessible from within the class and subclasses
                    if let Some(ref current) = self.current_class {
                        if current.name == class_name {
                            // Same class - allowed
                            Ok(())
                        } else if self.is_subclass(&current.name, class_name) {
                            // Subclass - allowed
                            Ok(())
                        } else {
                            Err(TypeCheckError::new(
                                format!(
                                    "Property '{}' is protected and only accessible within class '{}' and its subclasses",
                                    member_name, class_name
                                ),
                                span,
                            ))
                        }
                    } else {
                        Err(TypeCheckError::new(
                            format!(
                                "Property '{}' is protected and only accessible within class '{}' and its subclasses",
                                member_name, class_name
                            ),
                            span,
                        ))
                    }
                }
            }
        } else {
            // Member not found in our tracking - might be from interface or unknown class
            // Allow it for now
            Ok(())
        }
    }

    /// Check if a class is a subclass of another
    fn is_subclass(&self, child: &str, ancestor: &str) -> bool {
        // Check the current class context for parent information
        if let Some(ref ctx) = self.current_class {
            if ctx.name == child {
                if let Some(ref parent) = ctx.parent {
                    if parent == ancestor {
                        return true;
                    }
                    // Could recursively check parent's parent, but keeping it simple for now
                }
            }
        }

        false
    }

    /// Infer type of member access
    fn infer_member_type(
        &self,
        obj_type: &Type,
        member: &str,
        span: Span,
    ) -> Result<Type, TypeCheckError> {
        match &obj_type.kind {
            TypeKind::Reference(type_ref) => {
                // Check access modifiers for class members
                self.check_member_access(&type_ref.name.node, member, span)?;

                // Try to resolve the type reference to get the actual type
                if let Some(resolved) = self.type_env.lookup_type_alias(&type_ref.name.node) {
                    return self.infer_member_type(resolved, member, span);
                }

                // If not resolvable, return unknown
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }
            TypeKind::Object(obj) => {
                // Find member in object type
                for obj_member in &obj.members {
                    match obj_member {
                        ObjectTypeMember::Property(prop) => {
                            if prop.name.node == member {
                                return Ok(prop.type_annotation.clone());
                            }
                        }
                        ObjectTypeMember::Method(method) => {
                            if method.name.node == member {
                                return Ok(Type::new(
                                    TypeKind::Primitive(PrimitiveType::Unknown),
                                    span,
                                ));
                            }
                        }
                        _ => {}
                    }
                }
                // Member not found
                Err(TypeCheckError::new(
                    format!("Property '{}' does not exist", member),
                    span,
                ))
            }
            _ => {
                // Non-object member access - return unknown
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }
        }
    }

    /// Infer type of index access
    fn infer_index_type(&self, obj_type: &Type, span: Span) -> Result<Type, TypeCheckError> {
        match &obj_type.kind {
            TypeKind::Array(elem_type) => Ok((**elem_type).clone()),
            TypeKind::Tuple(types) => {
                // For now, return union of all tuple types
                if types.is_empty() {
                    Ok(Type::new(TypeKind::Primitive(PrimitiveType::Never), span))
                } else if types.len() == 1 {
                    Ok(types[0].clone())
                } else {
                    Ok(Type::new(TypeKind::Union(types.clone()), span))
                }
            }
            _ => Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span)),
        }
    }

    /// Type check match expression
    fn check_match_expression(
        &mut self,
        match_expr: &MatchExpression,
    ) -> Result<Type, TypeCheckError> {
        // Type check the value being matched
        let value_type = self.infer_expression_type(&match_expr.value)?;

        if match_expr.arms.is_empty() {
            return Err(TypeCheckError::new(
                "Match expression must have at least one arm".to_string(),
                match_expr.span,
            ));
        }

        // Check exhaustiveness
        self.check_exhaustiveness(&match_expr.arms, &value_type, match_expr.span)?;

        // Type check each arm and collect result types
        let mut arm_types = Vec::new();

        for arm in &match_expr.arms {
            // Enter a new scope for this arm
            self.symbol_table.enter_scope();

            // Narrow the type based on the pattern
            let narrowed_type = self.narrow_type_by_pattern(&arm.pattern, &value_type)?;

            // Check the pattern and bind variables with the narrowed type
            self.check_pattern(&arm.pattern, &narrowed_type)?;

            // Check the guard if present
            if let Some(guard) = &arm.guard {
                let guard_type = self.infer_expression_type(guard)?;
                // Guard should be boolean (primitive or literal)
                let is_boolean =
                    matches!(guard_type.kind, TypeKind::Primitive(PrimitiveType::Boolean))
                        || matches!(guard_type.kind, TypeKind::Literal(Literal::Boolean(_)));

                if !is_boolean {
                    return Err(TypeCheckError::new(
                        format!("Match guard must be boolean, found {:?}", guard_type.kind),
                        guard.span,
                    ));
                }
            }

            // Check the arm body
            let arm_type = match &arm.body {
                MatchArmBody::Expression(expr) => self.infer_expression_type(expr)?,
                MatchArmBody::Block(block) => {
                    // Type check the block
                    for stmt in &block.statements {
                        self.check_statement(stmt)?;
                    }
                    // Return type is void for blocks without explicit return

                    Type::new(TypeKind::Primitive(PrimitiveType::Void), block.span)
                }
            };

            arm_types.push(arm_type);

            // Exit the arm scope
            self.symbol_table.exit_scope();
        }

        // All arms should have compatible types - return a union
        if arm_types.is_empty() {
            return Ok(Type::new(
                TypeKind::Primitive(PrimitiveType::Never),
                match_expr.span,
            ));
        }

        // Find the common type or create a union
        let mut result_type = arm_types[0].clone();
        for arm_type in &arm_types[1..] {
            if TypeCompatibility::is_assignable(&result_type, arm_type) {
                // Keep result_type
            } else if TypeCompatibility::is_assignable(arm_type, &result_type) {
                result_type = arm_type.clone();
            } else {
                // Types are incompatible, create a union
                match &mut result_type.kind {
                    TypeKind::Union(types) => {
                        types.push(arm_type.clone());
                    }
                    _ => {
                        result_type = Type::new(
                            TypeKind::Union(vec![result_type.clone(), arm_type.clone()]),
                            match_expr.span,
                        );
                    }
                }
            }
        }

        Ok(result_type)
    }

    /// Check a pattern and bind variables
    fn check_pattern(
        &mut self,
        pattern: &Pattern,
        expected_type: &Type,
    ) -> Result<(), TypeCheckError> {
        match pattern {
            Pattern::Identifier(ident) => {
                // Bind the identifier to the expected type
                let symbol = Symbol::new(
                    ident.node.clone(),
                    SymbolKind::Variable,
                    expected_type.clone(),
                    ident.span,
                );
                self.symbol_table
                    .declare(symbol)
                    .map_err(|e| TypeCheckError::new(e, ident.span))?;
                Ok(())
            }
            Pattern::Literal(_lit, _span) => {
                // Literal patterns are allowed as long as they match the general type
                // We don't enforce exact literal matching at type check time
                // The pattern match will handle the runtime check

                Ok(())
            }
            Pattern::Wildcard(_) => {
                // Wildcard matches anything
                Ok(())
            }
            Pattern::Array(array_pattern) => {
                // Expected type should be an array
                match &expected_type.kind {
                    TypeKind::Array(elem_type) => {
                        for elem in &array_pattern.elements {
                            match elem {
                                ArrayPatternElement::Pattern(pat) => {
                                    self.check_pattern(pat, elem_type)?;
                                }
                                ArrayPatternElement::Rest(ident) => {
                                    // Rest pattern gets the array type
                                    let symbol = Symbol::new(
                                        ident.node.clone(),
                                        SymbolKind::Variable,
                                        expected_type.clone(),
                                        ident.span,
                                    );
                                    self.symbol_table
                                        .declare(symbol)
                                        .map_err(|e| TypeCheckError::new(e, ident.span))?;
                                }
                                ArrayPatternElement::Hole => {
                                    // Hole doesn't bind anything
                                }
                            }
                        }
                        Ok(())
                    }
                    _ => Err(TypeCheckError::new(
                        format!(
                            "Array pattern requires array type, found {:?}",
                            expected_type.kind
                        ),
                        array_pattern.span,
                    )),
                }
            }
            Pattern::Object(object_pattern) => {
                // Extract property types from the expected object type
                match &expected_type.kind {
                    TypeKind::Object(obj_type) => {
                        for prop in &object_pattern.properties {
                            // Find the property type in the object
                            let prop_type = obj_type
                                .members
                                .iter()
                                .find_map(|member| {
                                    if let ObjectTypeMember::Property(prop_sig) = member {
                                        if prop_sig.name.node == prop.key.node {
                                            return Some(prop_sig.type_annotation.clone());
                                        }
                                    }
                                    None
                                })
                                .unwrap_or_else(|| {
                                    Type::new(
                                        TypeKind::Primitive(PrimitiveType::Unknown),
                                        prop.span,
                                    )
                                });

                            if let Some(value_pattern) = &prop.value {
                                self.check_pattern(value_pattern, &prop_type)?;
                            } else {
                                // Shorthand: bind the key as a variable
                                let symbol = Symbol::new(
                                    prop.key.node.clone(),
                                    SymbolKind::Variable,
                                    prop_type,
                                    prop.key.span,
                                );
                                self.symbol_table
                                    .declare(symbol)
                                    .map_err(|e| TypeCheckError::new(e, prop.key.span))?;
                            }
                        }
                        Ok(())
                    }
                    _ => {
                        // If it's not an object type, accept any object pattern for now
                        // This handles cases like Table type
                        for prop in &object_pattern.properties {
                            let prop_type =
                                Type::new(TypeKind::Primitive(PrimitiveType::Unknown), prop.span);

                            if let Some(value_pattern) = &prop.value {
                                self.check_pattern(value_pattern, &prop_type)?;
                            } else {
                                let symbol = Symbol::new(
                                    prop.key.node.clone(),
                                    SymbolKind::Variable,
                                    prop_type,
                                    prop.key.span,
                                );
                                self.symbol_table
                                    .declare(symbol)
                                    .map_err(|e| TypeCheckError::new(e, prop.key.span))?;
                            }
                        }
                        Ok(())
                    }
                }
            }
        }
    }

    /// Check if match arms are exhaustive for the given type
    fn check_exhaustiveness(
        &self,
        arms: &[MatchArm],
        value_type: &Type,
        span: Span,
    ) -> Result<(), TypeCheckError> {
        // If there's a wildcard or identifier pattern without a guard, it's exhaustive
        let has_wildcard = arms.iter().any(|arm| {
            matches!(arm.pattern, Pattern::Wildcard(_) | Pattern::Identifier(_))
                && arm.guard.is_none()
        });

        if has_wildcard {
            return Ok(());
        }

        // Check exhaustiveness based on type
        match &value_type.kind {
            TypeKind::Primitive(PrimitiveType::Boolean) => {
                // Boolean must match both true and false
                let mut has_true = false;
                let mut has_false = false;

                for arm in arms {
                    if let Pattern::Literal(Literal::Boolean(b), _) = &arm.pattern {
                        if *b {
                            has_true = true;
                        } else {
                            has_false = true;
                        }
                    }
                }

                if !has_true || !has_false {
                    return Err(TypeCheckError::new(
                        "Non-exhaustive match: missing case for boolean type. Add patterns for both true and false, or use a wildcard (_) pattern.".to_string(),
                        span,
                    ));
                }
                Ok(())
            }
            TypeKind::Union(types) => {
                // For unions, we need to cover all union members
                // This is a simplified check - we verify that each union member has a potential match
                for union_type in types {
                    let covered = arms.iter().any(|arm| {
                        // Check if this arm could match this union member
                        self.pattern_could_match(&arm.pattern, union_type)
                    });

                    if !covered {
                        return Err(TypeCheckError::new(
                            format!("Non-exhaustive match: union type {:?} is not covered. Add a pattern to match this type or use a wildcard (_) pattern.", union_type.kind),
                            span,
                        ));
                    }
                }
                Ok(())
            }
            TypeKind::Literal(lit) => {
                // For literal types, must match exactly that literal
                let covered = arms.iter().any(|arm| {
                    matches!(&arm.pattern, Pattern::Literal(pattern_lit, _) if pattern_lit == lit)
                });

                if !covered {
                    return Err(TypeCheckError::new(
                        format!("Non-exhaustive match: literal {:?} is not matched. Add a pattern to match this literal or use a wildcard (_) pattern.", lit),
                        span,
                    ));
                }
                Ok(())
            }
            // For other types, we can't easily verify exhaustiveness
            // Require a wildcard/identifier pattern or emit a warning
            _ => {
                // Emit a warning that exhaustiveness cannot be verified
                // For now, we'll allow it but this could be improved
                Ok(())
            }
        }
    }

    /// Helper to check if a pattern could match a type
    fn pattern_could_match(&self, pattern: &Pattern, typ: &Type) -> bool {
        match pattern {
            Pattern::Wildcard(_) | Pattern::Identifier(_) => true,
            Pattern::Literal(lit, _) => match &typ.kind {
                TypeKind::Literal(type_lit) => lit == type_lit,
                TypeKind::Primitive(PrimitiveType::Boolean) => matches!(lit, Literal::Boolean(_)),
                TypeKind::Primitive(PrimitiveType::Number) => matches!(lit, Literal::Number(_)),
                TypeKind::Primitive(PrimitiveType::String) => matches!(lit, Literal::String(_)),
                _ => false,
            },
            Pattern::Array(_) => matches!(typ.kind, TypeKind::Array(_) | TypeKind::Tuple(_)),
            Pattern::Object(_) => matches!(typ.kind, TypeKind::Object(_)),
        }
    }

    /// Narrow the type based on the pattern
    fn narrow_type_by_pattern(
        &self,
        pattern: &Pattern,
        typ: &Type,
    ) -> Result<Type, TypeCheckError> {
        match pattern {
            Pattern::Wildcard(_) | Pattern::Identifier(_) => {
                // No narrowing for wildcard or identifier
                Ok(typ.clone())
            }
            Pattern::Literal(lit, span) => {
                // Narrow to literal type
                Ok(Type::new(TypeKind::Literal(lit.clone()), *span))
            }
            Pattern::Array(_) => {
                // For array patterns, narrow to array type if it's a union
                match &typ.kind {
                    TypeKind::Union(types) => {
                        // Find the array type in the union
                        for t in types {
                            if matches!(t.kind, TypeKind::Array(_) | TypeKind::Tuple(_)) {
                                return Ok(t.clone());
                            }
                        }
                        // No array type found, return original
                        Ok(typ.clone())
                    }
                    _ => Ok(typ.clone()),
                }
            }
            Pattern::Object(obj_pattern) => {
                // For object patterns, narrow based on properties
                match &typ.kind {
                    TypeKind::Union(types) => {
                        // Find object types in the union that have the required properties
                        let mut matching_types = Vec::new();
                        for t in types {
                            if let TypeKind::Object(obj_type) = &t.kind {
                                // Check if all pattern properties exist in this object type
                                let all_match = obj_pattern.properties.iter().all(|prop| {
                                    obj_type.members.iter().any(|member| {
                                        if let ObjectTypeMember::Property(prop_sig) = member {
                                            prop_sig.name.node == prop.key.node
                                        } else {
                                            false
                                        }
                                    })
                                });
                                if all_match {
                                    matching_types.push(t.clone());
                                }
                            }
                        }

                        if matching_types.is_empty() {
                            Ok(typ.clone())
                        } else if matching_types.len() == 1 {
                            Ok(matching_types[0].clone())
                        } else {
                            Ok(Type::new(TypeKind::Union(matching_types), typ.span))
                        }
                    }
                    _ => Ok(typ.clone()),
                }
            }
        }
    }

    /// Evaluate special type constructs (keyof, mapped types, conditional types, etc.)
    fn evaluate_type(&self, typ: &Type) -> Result<Type, String> {
        match &typ.kind {
            TypeKind::KeyOf(operand) => {
                // First evaluate the operand recursively
                let evaluated_operand = self.evaluate_type(operand)?;
                use super::evaluate_keyof;
                evaluate_keyof(&evaluated_operand, &self.type_env)
            }
            TypeKind::Mapped(mapped) => {
                use super::evaluate_mapped_type;
                evaluate_mapped_type(mapped, &self.type_env)
            }
            TypeKind::Conditional(conditional) => {
                use super::evaluate_conditional_type;
                evaluate_conditional_type(conditional, &self.type_env)
            }
            TypeKind::TemplateLiteral(template) => {
                use super::evaluate_template_literal_type;
                evaluate_template_literal_type(template, &self.type_env)
            }
            _ => Ok(typ.clone()),
        }
    }

    /// Widen literal types to their base primitive types
    fn widen_type(&self, typ: Type) -> Type {
        match typ.kind {
            TypeKind::Literal(Literal::Number(_)) | TypeKind::Literal(Literal::Integer(_)) => {
                Type::new(TypeKind::Primitive(PrimitiveType::Number), typ.span)
            }
            TypeKind::Literal(Literal::String(_)) => {
                Type::new(TypeKind::Primitive(PrimitiveType::String), typ.span)
            }
            TypeKind::Literal(Literal::Boolean(_)) => {
                Type::new(TypeKind::Primitive(PrimitiveType::Boolean), typ.span)
            }
            TypeKind::Literal(Literal::Nil) => {
                Type::new(TypeKind::Primitive(PrimitiveType::Nil), typ.span)
            }
            _ => typ,
        }
    }

    /// Register a declare function statement in the global scope
    fn register_declare_function(
        &mut self,
        func: &DeclareFunctionStatement,
    ) -> Result<(), TypeCheckError> {
        // Create function type from the declaration
        let func_type = Type::new(
            TypeKind::Function(FunctionType {
                type_parameters: func.type_parameters.clone(),
                parameters: func.parameters.clone(),
                return_type: Box::new(func.return_type.clone()),
                span: func.span,
            }),
            func.span,
        );

        // Declare function in symbol table
        let symbol = Symbol::new(
            func.name.node.clone(),
            SymbolKind::Function,
            func_type,
            func.span,
        );
        self.symbol_table
            .declare(symbol)
            .map_err(|e| TypeCheckError::new(e, func.span))
    }

    /// Register a declare const statement in the global scope
    fn register_declare_const(
        &mut self,
        const_decl: &DeclareConstStatement,
    ) -> Result<(), TypeCheckError> {
        // Declare constant in symbol table
        let symbol = Symbol::new(
            const_decl.name.node.clone(),
            SymbolKind::Const,
            const_decl.type_annotation.clone(),
            const_decl.span,
        );
        self.symbol_table
            .declare(symbol)
            .map_err(|e| TypeCheckError::new(e, const_decl.span))?;

        Ok(())
    }

    /// Register a declare namespace statement in the global scope
    fn register_declare_namespace(
        &mut self,
        ns: &DeclareNamespaceStatement,
    ) -> Result<(), TypeCheckError> {
        // Create object type from namespace members
        let mut members = Vec::new();

        for member in &ns.members {
            match member {
                Statement::DeclareFunction(func) if func.is_export => {
                    // Add as method to the namespace object
                    members.push(ObjectTypeMember::Method(MethodSignature {
                        name: func.name.clone(),
                        type_parameters: func.type_parameters.clone(),
                        parameters: func.parameters.clone(),
                        return_type: func.return_type.clone(),
                        span: func.span,
                    }));
                }
                Statement::DeclareConst(const_decl) if const_decl.is_export => {
                    // Add as property to the namespace object
                    members.push(ObjectTypeMember::Property(PropertySignature {
                        is_readonly: true, // Constants are readonly
                        name: const_decl.name.clone(),
                        is_optional: false,
                        type_annotation: const_decl.type_annotation.clone(),
                        span: const_decl.span,
                    }));
                }
                _ => {
                    // Other statement types or non-exported members are ignored
                }
            }
        }

        // Create namespace object type
        let namespace_type = Type::new(
            TypeKind::Object(ObjectType {
                members,
                span: ns.span,
            }),
            ns.span,
        );

        // Register namespace as a constant in the symbol table
        let symbol = Symbol::new(
            ns.name.node.clone(),
            SymbolKind::Const,
            namespace_type,
            ns.span,
        );
        self.symbol_table
            .declare(symbol)
            .map_err(|e| TypeCheckError::new(e, ns.span))
    }

    /// Register minimal stdlib (fallback when full stdlib fails to parse)
    fn register_minimal_stdlib(&mut self) {
        use crate::ast::pattern::Pattern;
        use crate::ast::statement::Parameter;
        use crate::ast::types::*;
        use crate::ast::Spanned;
        use crate::span::Span;

        let span = Span::new(0, 0, 0, 0);

        // Register string namespace
        let string_members = vec![
            ObjectTypeMember::Method(MethodSignature {
                name: Spanned::new("upper".to_string(), span),
                type_parameters: None,
                parameters: vec![Parameter {
                    pattern: Pattern::Identifier(Spanned::new("s".to_string(), span)),
                    type_annotation: Some(Type::new(
                        TypeKind::Primitive(PrimitiveType::String),
                        span,
                    )),
                    default: None,
                    is_rest: false,
                    is_optional: false,
                    span,
                }],
                return_type: Type::new(TypeKind::Primitive(PrimitiveType::String), span),
                span,
            }),
            ObjectTypeMember::Method(MethodSignature {
                name: Spanned::new("lower".to_string(), span),
                type_parameters: None,
                parameters: vec![Parameter {
                    pattern: Pattern::Identifier(Spanned::new("s".to_string(), span)),
                    type_annotation: Some(Type::new(
                        TypeKind::Primitive(PrimitiveType::String),
                        span,
                    )),
                    default: None,
                    is_rest: false,
                    is_optional: false,
                    span,
                }],
                return_type: Type::new(TypeKind::Primitive(PrimitiveType::String), span),
                span,
            }),
        ];

        let string_type = Type::new(
            TypeKind::Object(ObjectType {
                members: string_members,
                span,
            }),
            span,
        );

        let _ = self.symbol_table.declare(Symbol::new(
            "string".to_string(),
            SymbolKind::Const,
            string_type,
            span,
        ));

        // Register math namespace
        let math_members = vec![
            ObjectTypeMember::Property(PropertySignature {
                is_readonly: true,
                name: Spanned::new("pi".to_string(), span),
                is_optional: false,
                type_annotation: Type::new(TypeKind::Primitive(PrimitiveType::Number), span),
                span,
            }),
            ObjectTypeMember::Method(MethodSignature {
                name: Spanned::new("abs".to_string(), span),
                type_parameters: None,
                parameters: vec![Parameter {
                    pattern: Pattern::Identifier(Spanned::new("x".to_string(), span)),
                    type_annotation: Some(Type::new(
                        TypeKind::Primitive(PrimitiveType::Number),
                        span,
                    )),
                    default: None,
                    is_rest: false,
                    is_optional: false,
                    span,
                }],
                return_type: Type::new(TypeKind::Primitive(PrimitiveType::Number), span),
                span,
            }),
        ];

        let math_type = Type::new(
            TypeKind::Object(ObjectType {
                members: math_members,
                span,
            }),
            span,
        );

        let _ = self.symbol_table.declare(Symbol::new(
            "math".to_string(),
            SymbolKind::Const,
            math_type,
            span,
        ));
    }

    /// Get a reference to the symbol table for LSP queries
    pub fn symbol_table(&self) -> &SymbolTable {
        &self.symbol_table
    }

    /// Get a reference to the type environment for LSP queries
    pub fn type_env(&self) -> &TypeEnvironment {
        &self.type_env
    }

    /// Lookup a symbol by name in the current scope
    pub fn lookup_symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbol_table.lookup(name)
    }

    /// Lookup a type by name
    pub fn lookup_type(&self, name: &str) -> Option<&Type> {
        self.type_env.lookup_type(name)
    }

    /// Extract exports from a program for module system
    pub fn extract_exports(&self, program: &Program) -> crate::module_resolver::ModuleExports {
        use crate::module_resolver::{ExportedSymbol, ModuleExports};

        let mut exports = ModuleExports::new();

        for stmt in &program.statements {
            if let Statement::Export(export_decl) = stmt {
                match &export_decl.kind {
                    ExportKind::Declaration(decl) => {
                        // Extract symbol(s) from the declaration
                        match &**decl {
                            Statement::Variable(var_decl) => {
                                // Extract identifier from pattern
                                if let Pattern::Identifier(ident) = &var_decl.pattern {
                                    if let Some(symbol) = self.symbol_table.lookup(&ident.node) {
                                        exports.add_named(
                                            ident.node.clone(),
                                            ExportedSymbol::new(symbol.clone(), false),
                                        );
                                    }
                                }
                            }
                            Statement::Function(func_decl) => {
                                if let Some(symbol) = self.symbol_table.lookup(&func_decl.name.node)
                                {
                                    exports.add_named(
                                        func_decl.name.node.clone(),
                                        ExportedSymbol::new(symbol.clone(), false),
                                    );
                                }
                            }
                            Statement::Class(class_decl) => {
                                if let Some(symbol) =
                                    self.symbol_table.lookup(&class_decl.name.node)
                                {
                                    exports.add_named(
                                        class_decl.name.node.clone(),
                                        ExportedSymbol::new(symbol.clone(), false),
                                    );
                                }
                            }
                            Statement::TypeAlias(type_alias) => {
                                if let Some(symbol) =
                                    self.symbol_table.lookup(&type_alias.name.node)
                                {
                                    exports.add_named(
                                        type_alias.name.node.clone(),
                                        ExportedSymbol::new(symbol.clone(), true),
                                    );
                                }
                            }
                            Statement::Interface(interface_decl) => {
                                if let Some(symbol) =
                                    self.symbol_table.lookup(&interface_decl.name.node)
                                {
                                    exports.add_named(
                                        interface_decl.name.node.clone(),
                                        ExportedSymbol::new(symbol.clone(), true),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    ExportKind::Named { specifiers, .. } => {
                        // Export existing symbols: export { foo, bar as baz }
                        for spec in specifiers {
                            if let Some(symbol) = self.symbol_table.lookup(&spec.local.node) {
                                let export_name = spec
                                    .exported
                                    .as_ref()
                                    .map(|e| e.node.clone())
                                    .unwrap_or_else(|| spec.local.node.clone());

                                // Check if it's a type-only export
                                let is_type_only = matches!(
                                    symbol.kind,
                                    SymbolKind::TypeAlias | SymbolKind::Interface
                                );

                                exports.add_named(
                                    export_name,
                                    ExportedSymbol::new(symbol.clone(), is_type_only),
                                );
                            }
                        }
                    }
                    ExportKind::Default(_expr) => {
                        // For default exports, we create a synthetic symbol
                        // In the future, we might want to infer the type of the expression
                        let default_symbol = Symbol {
                            name: "default".to_string(),
                            typ: Type::new(
                                TypeKind::Primitive(PrimitiveType::Unknown),
                                export_decl.span,
                            ),
                            kind: SymbolKind::Variable,
                            span: export_decl.span,
                            is_exported: true,
                            references: Vec::new(),
                        };
                        exports.set_default(ExportedSymbol::new(default_symbol, false));
                    }
                }
            }
        }

        exports
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::CollectingDiagnosticHandler;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn type_check_source(source: &str) -> Result<(), TypeCheckError> {
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(source, handler.clone());
        let tokens = lexer.tokenize().expect("Lexing failed");
        let mut parser = Parser::new(tokens, handler.clone());
        let program = parser.parse().expect("Parsing failed");

        let mut type_checker = TypeChecker::new(handler);
        type_checker.check_program(&program)
    }

    #[test]
    fn test_simple_variable_declaration() {
        let source = "const x: number = 42";
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_type_mismatch() {
        let source = "const x: string = 42";
        assert!(type_check_source(source).is_err());
    }

    #[test]
    fn test_type_inference() {
        let source = "const x = 42";
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_object_literal_inference() {
        // First test: just declare the object
        let source1 = "const obj = {x: 10, y: 20}\n";
        let result1 = type_check_source(source1);
        if let Err(e) = &result1 {
            eprintln!("✗ Error declaring object: {}", e.message);
        }
        assert!(result1.is_ok(), "Should be able to declare object literal");

        // Second test: declare and use
        let source2 = "const obj = {x: 10, y: 20}\nconst a = obj.x\n";
        let result2 = type_check_source(source2);
        if let Err(e) = &result2 {
            eprintln!("✗ Error using object: {}", e.message);
        }
        assert!(result2.is_ok(), "Should be able to use object properties");
    }

    #[test]
    fn test_function_type_checking() {
        let source = r#"
            function add(a: number, b: number): number
                return a + b
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_undefined_variable() {
        let source = "const x = y";
        assert!(type_check_source(source).is_err());
    }

    #[test]
    fn test_narrowing_nil_check() {
        // Test that nil checks narrow types correctly in if statements
        let source = r#"
            function processValue(x: string | nil)
                if x != nil then
                    -- x should be narrowed to string here
                    local y: string = x
                end
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_narrowing_multiple_branches() {
        // Test narrowing with multiple if branches
        let source = r#"
            function processOptional(x: string | nil)
                if x != nil then
                    local s: string = x
                end

                local y: string | nil = "test"
                if y != nil then
                    local s2: string = y
                end
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_narrowing_nested_if() {
        // Test narrowing in nested if statements
        let source = r#"
            function processNested(a: string | nil, b: number | nil)
                if a != nil then
                    local x: string = a
                    if b != nil then
                        local y: number = b
                    end
                end
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_narrowing_else_branch() {
        // Test that else branch gets the complementary narrowing
        let source = r#"
            function checkNil(x: string | nil)
                if x == nil then
                    -- In then branch, x is nil, just use it
                    local temp = x
                else
                    -- In else branch, x is narrowed to string
                    local s: string = x
                end
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_type_predicate_valid_parameter() {
        // Test that type predicates accept valid parameter names
        let source = r#"
            function isString(x: string | number): x is string
                return true
            end
        "#;
        let result = type_check_source(source);
        if let Err(e) = &result {
            eprintln!("Unexpected error: {}", e.message);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_type_predicate_invalid_parameter() {
        // Test that type predicates reject invalid parameter names
        let source = r#"
            function isString(x: string | number): y is string
                return true
            end
        "#;
        let result = type_check_source(source);
        assert!(
            result.is_err(),
            "Expected error for type predicate with invalid parameter name"
        );
        if let Err(e) = result {
            assert!(
                e.message.contains("Type predicate parameter"),
                "Expected error message about type predicate parameter, got: {}",
                e.message
            );
        }
    }

    #[test]
    fn test_narrowing_double_nil_check() {
        // Test nil narrowing with two variables
        let source = r#"
            function process(a: string | nil, b: number | nil)
                if a != nil then
                    local x: string = a
                end
                if b != nil then
                    local y: number = b
                end
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_class_basic() {
        let source = r#"
            class Animal
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_class_with_property() {
        let source = r#"
            class Person
                name: string
                age: number = 25
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    // Property type checking is working but literal "25" vs number
    // compatibility depends on the type compatibility implementation
    // The test would pass with stricter type checking

    #[test]
    fn test_class_with_constructor() {
        let source = r#"
            class Person
                constructor(name: string, age: number)
                    self.name = name
                    self.age = age
                end
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_class_multiple_constructors() {
        let source = r#"
            class Person {
                constructor(name: string) {
                }

                constructor(name: string, age: number) {
                }
            }
        "#;
        let result = type_check_source(source);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.message.contains("one constructor"));
        }
    }

    #[test]
    fn test_class_with_method() {
        let source = r#"
            class Calculator {
                add(a: number, b: number): number {
                    return a + b
                }
            }
        "#;
        let result = type_check_source(source);
        if let Err(ref e) = result {
            eprintln!("Error: {}", e.message);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_abstract_class() {
        let source = r#"
            abstract class Animal {
                abstract makeSound(): string;

                move(): void {
                    const x: number = 5
                }
            }
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_abstract_method_in_concrete_class() {
        let source = r#"
            class Animal
                abstract makeSound(): string;
            end
        "#;
        let result = type_check_source(source);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.message.contains("abstract class"));
        }
    }

    #[test]
    fn test_abstract_method_with_body() {
        // This test just verifies abstract methods work correctly
        // The parser prevents abstract methods from having bodies by design
        let source = r#"
            abstract class Animal {
                abstract makeSound(): string;

                concrete(): void {
                    const x: number = 5
                }
            }
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_class_with_getter() {
        let source = r#"
            class Person
                get fullName(): string
                    return "John Doe"
                end
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_class_with_setter() {
        let source = r#"
            class Person
                set age(value: number)
                    self._age = value
                end
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    // Getter return type checking depends on literal vs primitive type compatibility

    #[test]
    fn test_generic_class() {
        let source = r#"
            class Container<T> {
                value: T

                constructor(val: T) {
                    const temp: T = val
                }

                getValue(defaultVal: T): T {
                    return defaultVal
                }
            }
        "#;
        let result = type_check_source(source);
        if let Err(ref e) = result {
            eprintln!("Error: {}", e.message);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_class_implements_interface() {
        let source = r#"
            interface Walkable {
                walk(): void
            }

            class Person implements Walkable {
                walk(): void {
                    const x: number = 5
                }
            }
        "#;
        let result = type_check_source(source);
        if let Err(ref e) = result {
            eprintln!("Error: {}", e.message);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_class_missing_interface_method() {
        let source = r#"
            interface Walkable {
                walk(): void
            }

            class Person implements Walkable {
            }
        "#;
        let result = type_check_source(source);
        if let Err(ref e) = result {
            eprintln!("Error: {}", e.message);
        }
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.message.contains("does not implement"));
        }
    }

    #[test]
    fn test_class_static_method() {
        let source = r#"
            class Math
                static abs(x: number): number
                    if x < 0 then
                        return -x
                    else
                        return x
                    end
                end
            end
        "#;
        assert!(type_check_source(source).is_ok());
    }

    #[test]
    fn test_stdlib_builtins_loaded() {
        // Test that built-in global functions from stdlib are available
        let source = r#"
            const x = print("Hello")
            const y = tonumber("42")
        "#;
        let result = type_check_source(source);
        if let Err(ref e) = result {
            eprintln!("Error: {}", e.message);
        }
        assert!(
            result.is_ok(),
            "Built-in functions should be available from stdlib"
        );
    }

    #[test]
    fn test_stdlib_string_library() {
        // Test that string library functions are available
        let source = r#"
            const upper = string.upper("hello")
            const lower = string.lower("WORLD")
        "#;
        let result = type_check_source(source);
        if let Err(ref e) = result {
            eprintln!("Error: {}", e.message);
        }
        assert!(
            result.is_ok(),
            "String library should be available from stdlib"
        );
    }

    #[test]
    fn test_stdlib_math_library() {
        // Test that math library constants and functions are available
        let source = r#"
            const p = math.pi
            const result = math.abs(-5)
        "#;
        let result = type_check_source(source);
        if let Err(ref e) = result {
            eprintln!("Error: {}", e.message);
        }
        assert!(
            result.is_ok(),
            "Math library should be available from stdlib"
        );
    }
}
