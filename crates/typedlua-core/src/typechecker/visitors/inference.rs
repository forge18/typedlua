use super::super::generics::infer_type_arguments;
use super::super::symbol_table::{Symbol, SymbolKind, SymbolTable};
use super::super::type_compat::TypeCompatibility;
use super::super::type_environment::TypeEnvironment;
use super::super::visitors::{AccessControl, AccessControlVisitor, ClassMemberKind};
use super::super::TypeCheckError;
use super::TypeCheckVisitor;
use typedlua_parser::ast::expression::*;
use typedlua_parser::ast::pattern::{ArrayPatternElement, Pattern};
use typedlua_parser::ast::types::*;
use typedlua_parser::prelude::{
    Argument, MatchArm, MatchArmBody, MatchExpression, PropertySignature,
};
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::StringInterner;

/// Trait for type inference operations
pub trait TypeInferenceVisitor: TypeCheckVisitor {
    /// Infer the type of an expression
    fn infer_expression(&mut self, expr: &mut Expression) -> Result<Type, TypeCheckError>;

    /// Infer type of binary operation
    fn infer_binary_op(
        &self,
        op: BinaryOp,
        left: &Type,
        right: &Type,
        span: Span,
    ) -> Result<Type, TypeCheckError>;

    /// Infer type of unary operation
    fn infer_unary_op(
        &self,
        op: UnaryOp,
        operand: &Type,
        span: Span,
    ) -> Result<Type, TypeCheckError>;

    /// Infer type of function call
    fn infer_call(
        &self,
        callee_type: &Type,
        _args: &[Argument],
        span: Span,
    ) -> Result<Type, TypeCheckError>;

    /// Infer type of a method call on an object
    fn infer_method(
        &self,
        obj_type: &Type,
        method_name: &str,
        _args: &[Argument],
        span: Span,
    ) -> Result<Type, TypeCheckError>;

    /// Infer type of member access
    fn infer_member(
        &self,
        obj_type: &Type,
        member: &str,
        span: Span,
    ) -> Result<Type, TypeCheckError>;

    /// Infer type of index access
    fn infer_index(&self, obj_type: &Type, span: Span) -> Result<Type, TypeCheckError>;

    /// Make a type optional by adding nil to the union
    fn make_optional(&self, typ: Type, span: Span) -> Result<Type, TypeCheckError>;

    /// Remove nil from a type
    fn remove_nil(&self, typ: &Type, span: Span) -> Result<Type, TypeCheckError>;

    /// Check if a type is nil
    fn is_nil(&self, typ: &Type) -> bool;

    /// Infer type of null coalescing operation
    fn infer_null_coalesce(
        &self,
        left: &Type,
        right: &Type,
        span: Span,
    ) -> Result<Type, TypeCheckError>;

    /// Check match expression and return result type
    fn check_match(&mut self, match_expr: &mut MatchExpression) -> Result<Type, TypeCheckError>;

    /// Check a pattern and bind variables
    fn check_pattern(
        &mut self,
        pattern: &Pattern,
        expected_type: &Type,
    ) -> Result<(), TypeCheckError>;
}

/// Type inference implementation
pub struct TypeInferrer<'a> {
    symbol_table: &'a mut SymbolTable,
    type_env: &'a mut TypeEnvironment,
    narrowing_context: &'a mut super::NarrowingContext,
    access_control: &'a AccessControl,
    interner: &'a StringInterner,
}

impl<'a> TypeInferrer<'a> {
    pub fn new(
        symbol_table: &'a mut SymbolTable,
        type_env: &'a mut TypeEnvironment,
        narrowing_context: &'a mut super::NarrowingContext,
        access_control: &'a AccessControl,
        interner: &'a StringInterner,
    ) -> Self {
        Self {
            symbol_table,
            type_env,
            narrowing_context,
            access_control,
            interner,
        }
    }
}

impl TypeCheckVisitor for TypeInferrer<'_> {
    fn name(&self) -> &'static str {
        "TypeInferrer"
    }
}

impl TypeInferenceVisitor for TypeInferrer<'_> {
    fn infer_expression(&mut self, expr: &mut Expression) -> Result<Type, TypeCheckError> {
        let span = expr.span;

        match &mut expr.kind {
            ExpressionKind::Literal(lit) => Ok(Type::new(TypeKind::Literal(lit.clone()), span)),

            ExpressionKind::Identifier(name) => {
                let name_str = self.interner.resolve(*name);
                // Check for narrowed type first
                if let Some(narrowed_type) = self.narrowing_context.get_narrowed_type(*name) {
                    return Ok(narrowed_type.clone());
                }

                // Fall back to symbol table
                if let Some(symbol) = self.symbol_table.lookup(&name_str) {
                    Ok(symbol.typ.clone())
                } else {
                    Err(TypeCheckError::new(
                        format!("Undefined variable '{}'", name_str),
                        span,
                    ))
                }
            }

            ExpressionKind::Binary(op, left, right) => {
                let left_type = self.infer_expression(left)?;
                let right_type = self.infer_expression(right)?;
                self.infer_binary_op(*op, &left_type, &right_type, span)
            }

            ExpressionKind::Unary(op, operand) => {
                let operand_type = self.infer_expression(operand)?;
                self.infer_unary_op(*op, &operand_type, span)
            }

            ExpressionKind::Call(callee, args, ref mut stored_type_args) => {
                let callee_type = self.infer_expression(callee)?;

                // If callee is a generic function, infer and store type arguments
                if let TypeKind::Function(func_type) = &callee_type.kind {
                    if let Some(type_params) = &func_type.type_parameters {
                        // Infer argument types
                        let mut arg_types = Vec::with_capacity(args.len());
                        for arg in args.iter_mut() {
                            let arg_type =
                                self.infer_expression(&mut arg.value).unwrap_or_else(|_| {
                                    Type::new(
                                        TypeKind::Primitive(PrimitiveType::Unknown),
                                        arg.value.span,
                                    )
                                });
                            arg_types.push(arg_type);
                        }

                        // Infer type arguments from function signature and argument types
                        if let Ok(inferred_types) =
                            infer_type_arguments(type_params, &func_type.parameters, &arg_types)
                        {
                            *stored_type_args = Some(inferred_types);
                        }
                    }
                }

                self.infer_call(&callee_type, args, span)
            }

            ExpressionKind::MethodCall(object, method, args, _) => {
                let obj_type = self.infer_expression(object)?;
                let method_name = self.interner.resolve(method.node);
                let method_type = self.infer_method(&obj_type, &method_name, args, span)?;

                // Set receiver_class based on inferred type (not variable name)
                // This enables method-to-function conversion optimization
                if let TypeKind::Reference(type_ref) = &obj_type.kind {
                    let type_name = self.interner.resolve(type_ref.name.node);
                    // Only set for classes (not interfaces) - check class_members
                    if self.access_control.get_class_members(&type_name).is_some() {
                        expr.receiver_class = Some(ReceiverClassInfo {
                            class_name: type_ref.name.node,
                            is_static: false,
                        });
                    }
                }

                expr.annotated_type = Some(method_type.clone());
                Ok(method_type)
            }

            ExpressionKind::Member(object, member) => {
                let obj_type = self.infer_expression(object)?;
                let member_name = self.interner.resolve(member.node);
                self.infer_member(&obj_type, &member_name, span)
            }

            ExpressionKind::Index(object, index) => {
                let obj_type = self.infer_expression(object)?;
                let _index_type = self.infer_expression(index)?;
                self.infer_index(&obj_type, span)
            }

            ExpressionKind::OptionalMember(object, member) => {
                let obj_type = self.infer_expression(object)?;
                let member_name = self.interner.resolve(member.node);
                let member_type = self.infer_member(&obj_type, &member_name, span)?;
                self.make_optional(member_type, span)
            }

            ExpressionKind::OptionalIndex(object, index) => {
                let obj_type = self.infer_expression(object)?;
                let _index_type = self.infer_expression(index)?;
                let indexed_type = self.infer_index(&obj_type, span)?;
                self.make_optional(indexed_type, span)
            }

            ExpressionKind::OptionalCall(callee, args, ref mut stored_type_args) => {
                let callee_type = self.infer_expression(callee)?;

                // If callee is a generic function, infer and store type arguments
                if let TypeKind::Function(func_type) = &callee_type.kind {
                    if let Some(type_params) = &func_type.type_parameters {
                        let mut arg_types = Vec::with_capacity(args.len());
                        for arg in args.iter_mut() {
                            let arg_type =
                                self.infer_expression(&mut arg.value).unwrap_or_else(|_| {
                                    Type::new(
                                        TypeKind::Primitive(PrimitiveType::Unknown),
                                        arg.value.span,
                                    )
                                });
                            arg_types.push(arg_type);
                        }

                        if let Ok(inferred_types) =
                            infer_type_arguments(type_params, &func_type.parameters, &arg_types)
                        {
                            *stored_type_args = Some(inferred_types);
                        }
                    }
                }

                let return_type = self.infer_call(&callee_type, args, span)?;
                self.make_optional(return_type, span)
            }

            ExpressionKind::OptionalMethodCall(object, method, args, _) => {
                let obj_type = self.infer_expression(object)?;
                let method_name = self.interner.resolve(method.node);
                let method_type = self.infer_method(&obj_type, &method_name, args, span)?;
                self.make_optional(method_type, span)
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
                            let elem_type = self.infer_expression(expr)?;
                            element_types.push(elem_type);
                        }
                        ArrayElement::Spread(expr) => {
                            // Spread expression should be an array
                            let spread_type = self.infer_expression(expr)?;
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
                            let value_type = self.infer_expression(value)?;

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
                            let key_type = self.infer_expression(key)?;
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
                            self.infer_expression(value)?;

                            // Note: We can't add computed properties to the static object type
                            // since we don't know the key at compile time, but we still validate them
                        }
                        ObjectProperty::Spread {
                            value,
                            span: spread_span,
                        } => {
                            // Spread object properties
                            let spread_type = self.infer_expression(value)?;
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
                let _cond_type = self.infer_expression(cond)?;
                let then_type = self.infer_expression(then_expr)?;
                let else_type = self.infer_expression(else_expr)?;

                // Return union of both branches
                if TypeCompatibility::is_assignable(&then_type, &else_type) {
                    Ok(else_type)
                } else if TypeCompatibility::is_assignable(&else_type, &then_type) {
                    Ok(then_type)
                } else {
                    Ok(Type::new(TypeKind::Union(vec![then_type, else_type]), span))
                }
            }

            ExpressionKind::Match(match_expr) => self.check_match(match_expr),

            ExpressionKind::Pipe(left_expr, right_expr) => {
                // Pipe operator: left |> right
                // The right side should be a function, and we apply left as the first argument
                let _left_type = self.infer_expression(left_expr)?;

                // For now, we'll infer the result type by checking the right expression
                // In a full implementation, we'd check if right is a function and apply left to it
                // For simplicity, we'll type check right and return its type
                // (This handles cases like: value |> func where func returns something)
                self.infer_expression(right_expr)
            }

            ExpressionKind::Try(try_expr) => {
                let expr_type = self.infer_expression(&mut try_expr.expression)?;
                let catch_type = self.infer_expression(&mut try_expr.catch_expression)?;

                if TypeCompatibility::is_assignable(&expr_type, &catch_type) {
                    Ok(catch_type)
                } else if TypeCompatibility::is_assignable(&catch_type, &expr_type) {
                    Ok(expr_type)
                } else {
                    Ok(Type::new(
                        TypeKind::Union(vec![expr_type, catch_type]),
                        span,
                    ))
                }
            }

            ExpressionKind::ErrorChain(left_expr, right_expr) => {
                let _left_type = self.infer_expression(left_expr)?;
                self.infer_expression(right_expr)
            }

            ExpressionKind::New(callee, _args) => {
                // Infer the class type from the callee expression
                // For `new ClassName(args)`, callee is Identifier("ClassName")
                match &callee.kind {
                    ExpressionKind::Identifier(name) => {
                        // Return a Reference type to the class
                        Ok(Type::new(
                            TypeKind::Reference(TypeReference {
                                name: typedlua_parser::ast::Spanned::new(*name, span),
                                type_arguments: None,
                                span,
                            }),
                            span,
                        ))
                    }
                    _ => {
                        // For complex callee expressions, infer the callee type
                        // and use it as the result type
                        Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
                    }
                }
            }

            _ => {
                // For unimplemented expression types, return unknown
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }
        }
    }

    fn infer_binary_op(
        &self,
        op: BinaryOp,
        left: &Type,
        right: &Type,
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
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Number), span))
            }
            BinaryOp::Concatenate => {
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::String), span))
            }
            BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::LessThan
            | BinaryOp::LessThanOrEqual
            | BinaryOp::GreaterThan
            | BinaryOp::GreaterThanOrEqual
            | BinaryOp::Instanceof => {
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Boolean), span))
            }
            BinaryOp::And | BinaryOp::Or => {
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }
            BinaryOp::NullCoalesce => self.infer_null_coalesce(left, right, span),
            BinaryOp::BitwiseAnd
            | BinaryOp::BitwiseOr
            | BinaryOp::BitwiseXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight => {
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Number), span))
            }
        }
    }

    fn infer_unary_op(
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

    fn infer_call(
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

    fn infer_method(
        &self,
        obj_type: &Type,
        method_name: &str,
        _args: &[Argument],
        span: Span,
    ) -> Result<Type, TypeCheckError> {
        // Look up the method in the object type
        match &obj_type.kind {
            TypeKind::Object(obj) => {
                for member in &obj.members {
                    if let ObjectTypeMember::Method(method) = member {
                        if self.interner.resolve(method.name.node) == method_name {
                            // Return the return type of the method
                            return Ok(method.return_type.clone());
                        }
                    }
                }
                // Method not found - return unknown
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }
            TypeKind::Reference(type_ref) => {
                let type_name = self.interner.resolve(type_ref.name.node);
                if let Some(class_members) = self.access_control.get_class_members(&type_name) {
                    for member in class_members {
                        if member.name == method_name {
                            if let ClassMemberKind::Method { return_type, .. } = &member.kind {
                                return Ok(return_type.clone().unwrap_or_else(|| {
                                    Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span)
                                }));
                            }
                        }
                    }
                }
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }
            _ => Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span)),
        }
    }

    fn infer_member(
        &self,
        obj_type: &Type,
        member: &str,
        span: Span,
    ) -> Result<Type, TypeCheckError> {
        match &obj_type.kind {
            TypeKind::Reference(type_ref) => {
                let type_name = self.interner.resolve(type_ref.name.node);

                // Check if this is a generic type alias with type arguments
                if let Some(type_args) = &type_ref.type_arguments {
                    if let Some(generic_alias) = self.type_env.get_generic_type_alias(&type_name) {
                        // Instantiate the generic type alias with the provided type arguments
                        use super::super::generics::instantiate_type;
                        let instantiated = instantiate_type(
                            &generic_alias.typ,
                            &generic_alias.type_parameters,
                            type_args,
                        )
                        .map_err(|e| TypeCheckError::new(e, span))?;
                        return self.infer_member(&instantiated, member, span);
                    }
                }

                // Check access modifiers for class members (only for actual classes)
                self.check_member_access(&type_name, member, span)?;

                // Try to resolve the type reference to get the actual type
                // Use lookup_type to check both type aliases and interfaces
                if let Some(resolved) = self.type_env.lookup_type(&type_name) {
                    // Check for infinite recursion - if resolved type is the same as input, avoid loop
                    if matches!(resolved.kind, TypeKind::Reference(_)) {
                        // If resolved is still a reference, check if it's the same reference
                        if let TypeKind::Reference(resolved_ref) = &resolved.kind {
                            if resolved_ref.name.node == type_ref.name.node {
                                // Same type reference - check if it's a field of the enum
                                // For enums, we need to check fields defined in the enum declaration
                                // For now, return unknown to avoid infinite loop
                                // The field will be looked up from the symbol table instead
                                return Ok(Type::new(
                                    TypeKind::Primitive(PrimitiveType::Unknown),
                                    span,
                                ));
                            }
                        }
                    }
                    return self.infer_member(resolved, member, span);
                }

                // If not resolvable, return unknown
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }
            TypeKind::Object(obj) => {
                // Find member in object type
                let member_id = self.interner.intern(member);
                for obj_member in &obj.members {
                    match obj_member {
                        ObjectTypeMember::Property(prop) => {
                            if prop.name.node == member_id {
                                return Ok(prop.type_annotation.clone());
                            }
                        }
                        ObjectTypeMember::Method(method) => {
                            if method.name.node == member_id {
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
            TypeKind::Union(types) => {
                // For union types, try to find the member in each non-nil variant
                let non_nil_types: Vec<&Type> = types.iter().filter(|t| !self.is_nil(t)).collect();

                if non_nil_types.is_empty() {
                    // All types are nil - member access on nil returns nil
                    Ok(Type::new(TypeKind::Primitive(PrimitiveType::Nil), span))
                } else if non_nil_types.len() == 1 {
                    // Single non-nil type - look up member on that
                    self.infer_member(non_nil_types[0], member, span)
                } else {
                    // Multiple non-nil types - try to look up member on first valid one
                    // For simplicity, we try each type and return the first successful lookup
                    for typ in non_nil_types {
                        if let Ok(member_type) = self.infer_member(typ, member, span) {
                            return Ok(member_type);
                        }
                    }
                    // If none succeeded, return unknown
                    Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
                }
            }
            TypeKind::Nullable(inner) => {
                // For nullable types, look up member on the inner type
                self.infer_member(inner, member, span)
            }
            _ => {
                // Non-object member access - return unknown
                Ok(Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span))
            }
        }
    }

    fn infer_index(&self, obj_type: &Type, span: Span) -> Result<Type, TypeCheckError> {
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

    fn make_optional(&self, typ: Type, span: Span) -> Result<Type, TypeCheckError> {
        let nil_type = Type::new(TypeKind::Primitive(PrimitiveType::Nil), span);
        Ok(Type::new(TypeKind::Union(vec![typ, nil_type]), span))
    }

    fn remove_nil(&self, typ: &Type, span: Span) -> Result<Type, TypeCheckError> {
        match &typ.kind {
            TypeKind::Union(types) => {
                let non_nil_types: Vec<Type> =
                    types.iter().filter(|t| !self.is_nil(t)).cloned().collect();
                if non_nil_types.is_empty() {
                    Ok(Type::new(TypeKind::Primitive(PrimitiveType::Never), span))
                } else if non_nil_types.len() == 1 {
                    Ok(non_nil_types[0].clone())
                } else {
                    Ok(Type::new(TypeKind::Union(non_nil_types), span))
                }
            }
            _ => {
                if self.is_nil(typ) {
                    Ok(Type::new(TypeKind::Primitive(PrimitiveType::Never), span))
                } else {
                    Ok(typ.clone())
                }
            }
        }
    }

    fn is_nil(&self, typ: &Type) -> bool {
        match &typ.kind {
            TypeKind::Primitive(PrimitiveType::Nil) => true,
            TypeKind::Literal(Literal::Nil) => true,
            TypeKind::Nullable(inner) => self.is_nil(inner),
            _ => false,
        }
    }

    fn infer_null_coalesce(
        &self,
        left: &Type,
        right: &Type,
        span: Span,
    ) -> Result<Type, TypeCheckError> {
        // If left is T | nil, the result is T (left without nil)
        // If left is just nil, the result is the type of right
        // Otherwise, the result is the type of left
        let left_without_nil = self.remove_nil(left, span)?;

        // If left was just nil, return right's type
        // Otherwise return left's type without nil
        let result = if matches!(
            left_without_nil.kind,
            TypeKind::Primitive(PrimitiveType::Never)
        ) {
            right.clone()
        } else {
            left_without_nil
        };

        Ok(result)
    }

    fn check_match(&mut self, match_expr: &mut MatchExpression) -> Result<Type, TypeCheckError> {
        // Type check the value being matched
        let value_type = self.infer_expression(&mut match_expr.value)?;
        eprintln!(
            "DEBUG check_match: value_type = {:?}, arms = {}",
            value_type.kind,
            match_expr.arms.len()
        );

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

        for arm in match_expr.arms.iter_mut() {
            // Enter a new scope for this arm
            self.symbol_table.enter_scope();

            // Narrow the type based on the pattern
            let narrowed_type = self.narrow_type_by_pattern(&arm.pattern, &value_type)?;

            // Check the pattern and bind variables with the narrowed type
            self.check_pattern(&arm.pattern, &narrowed_type)?;

            // Check the guard if present
            if let Some(guard) = &mut arm.guard {
                let guard_type = self.infer_expression(guard)?;
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
            let arm_type = match &mut arm.body {
                MatchArmBody::Expression(expr) => self.infer_expression(expr)?,
                MatchArmBody::Block(block) => {
                    // Type check the block
                    for _stmt in &mut block.statements {
                        // For now, we can't easily check statements here
                        // This would require access to the full type checker
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

    fn check_pattern(
        &mut self,
        pattern: &Pattern,
        expected_type: &Type,
    ) -> Result<(), TypeCheckError> {
        match pattern {
            Pattern::Identifier(ident) => {
                // Bind the identifier to the expected type
                let symbol = Symbol::new(
                    self.interner.resolve(ident.node).to_string(),
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
                                        self.interner.resolve(ident.node).to_string(),
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
                                    self.interner.resolve(prop.key.node).to_string(),
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
                                    self.interner.resolve(prop.key.node).to_string(),
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
}

impl TypeInferrer<'_> {
    /// Check member access permissions
    fn check_member_access(
        &self,
        class_name: &str,
        member_name: &str,
        span: Span,
    ) -> Result<(), TypeCheckError> {
        self.access_control.check_member_access(
            self.access_control.get_current_class(),
            class_name,
            member_name,
            span,
        )
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
            let is_wildcard = matches!(arm.pattern, Pattern::Wildcard(_) | Pattern::Identifier(_))
                && arm.guard.is_none();
            eprintln!(
                "DEBUG check_exhaustiveness: arm pattern = {:?}, is_wildcard = {}",
                arm.pattern, is_wildcard
            );
            is_wildcard
        });
        eprintln!(
            "DEBUG check_exhaustiveness: has_wildcard = {}",
            has_wildcard
        );

        if has_wildcard {
            return Ok(());
        }

        // Check exhaustiveness based on type
        eprintln!(
            "DEBUG check_exhaustiveness: value_type.kind = {:?}",
            value_type.kind
        );
        match &value_type.kind {
            TypeKind::Primitive(PrimitiveType::Boolean) => {
                // Boolean must match both true and false
                let mut has_true = false;
                let mut has_false = false;

                eprintln!(
                    "DEBUG exhaustiveness: checking {} arms for boolean",
                    arms.len()
                );
                for arm in arms {
                    if let Pattern::Literal(Literal::Boolean(b), _) = &arm.pattern {
                        if *b {
                            has_true = true;
                        } else {
                            has_false = true;
                        }
                    }
                }
                eprintln!(
                    "DEBUG exhaustiveness: has_true={}, has_false={}",
                    has_true, has_false
                );

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
                        let matching_types: Vec<_> = types
                            .iter()
                            .filter(|t| {
                                if let TypeKind::Object(obj_type) = &t.kind {
                                    // Check if all pattern properties exist in this object type
                                    obj_pattern.properties.iter().all(|prop| {
                                        obj_type.members.iter().any(|member| {
                                            if let ObjectTypeMember::Property(prop_sig) = member {
                                                prop_sig.name.node == prop.key.node
                                            } else {
                                                false
                                            }
                                        })
                                    })
                                } else {
                                    false
                                }
                            })
                            .cloned()
                            .collect();

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
}

#[cfg(test)]
mod inference_tests;
