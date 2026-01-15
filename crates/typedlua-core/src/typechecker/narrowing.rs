use crate::ast::expression::{BinaryOp, Expression, ExpressionKind, Literal, UnaryOp};
use crate::ast::types::{PrimitiveType, Type, TypeKind};
use rustc_hash::FxHashMap;

/// Type narrowing context - tracks refined types for variables in the current scope
#[derive(Debug, Clone)]
pub struct NarrowingContext {
    /// Map from variable name to narrowed type
    narrowed_types: FxHashMap<String, Type>,
}

impl Default for NarrowingContext {
    fn default() -> Self {
        Self::new()
    }
}

impl NarrowingContext {
    pub fn new() -> Self {
        Self {
            narrowed_types: FxHashMap::default(),
        }
    }

    /// Get the narrowed type for a variable, if any
    pub fn get_narrowed_type(&self, name: &str) -> Option<&Type> {
        self.narrowed_types.get(name)
    }

    /// Set a narrowed type for a variable
    pub fn set_narrowed_type(&mut self, name: String, typ: Type) {
        self.narrowed_types.insert(name, typ);
    }

    /// Remove a narrowed type (when variable is reassigned)
    pub fn remove_narrowed_type(&mut self, name: &str) {
        self.narrowed_types.remove(name);
    }

    /// Merge two narrowing contexts (for branch join points)
    pub fn merge(then_ctx: &Self, else_ctx: &Self) -> Self {
        // For now, we only keep types that are the same in both branches
        // More sophisticated: create union types for divergent branches
        let mut merged = NarrowingContext::new();

        for (name, then_type) in &then_ctx.narrowed_types {
            if let Some(else_type) = else_ctx.narrowed_types.get(name) {
                if types_equal(then_type, else_type) {
                    merged
                        .narrowed_types
                        .insert(name.clone(), then_type.clone());
                }
            }
        }

        merged
    }

    /// Clone the context for a new branch
    pub fn clone_for_branch(&self) -> Self {
        self.clone()
    }
}

/// Narrow a type based on a condition expression
/// Returns (then_context, else_context) with refined types for each branch
pub fn narrow_type_from_condition(
    condition: &Expression,
    base_ctx: &NarrowingContext,
    original_types: &FxHashMap<String, Type>,
) -> (NarrowingContext, NarrowingContext) {
    let mut then_ctx = base_ctx.clone_for_branch();
    let mut else_ctx = base_ctx.clone_for_branch();

    match &condition.kind {
        // typeof x == "string"
        ExpressionKind::Binary(BinaryOp::Equal, left, right) => {
            if let Some((var_name, type_name)) = extract_typeof_check(left, right) {
                if let Some(narrowed_type) = typeof_string_to_type(&type_name) {
                    then_ctx.set_narrowed_type(var_name.clone(), narrowed_type.clone());

                    // In else branch, exclude the checked type
                    if let Some(original) = original_types.get(&var_name) {
                        if let Some(else_type) = exclude_type(original, &narrowed_type) {
                            else_ctx.set_narrowed_type(var_name, else_type);
                        }
                    }
                }
            } else {
                // Check for x == nil equality narrowing
                if let Some((var_name, is_nil)) = extract_nil_check(left, right) {
                    if is_nil {
                        // then: x is nil
                        then_ctx.set_narrowed_type(
                            var_name.clone(),
                            Type::new(TypeKind::Primitive(PrimitiveType::Nil), condition.span),
                        );

                        // else: x is non-nil
                        if let Some(original) = original_types.get(&var_name) {
                            if let Some(non_nil) = remove_nil_from_type(original) {
                                else_ctx.set_narrowed_type(var_name, non_nil);
                            }
                        }
                    }
                }
            }
        }

        // typeof x != "string"
        ExpressionKind::Binary(BinaryOp::NotEqual, left, right) => {
            if let Some((var_name, type_name)) = extract_typeof_check(left, right) {
                if let Some(narrowed_type) = typeof_string_to_type(&type_name) {
                    // Flip the narrowing for != operator
                    else_ctx.set_narrowed_type(var_name.clone(), narrowed_type.clone());

                    if let Some(original) = original_types.get(&var_name) {
                        if let Some(then_type) = exclude_type(original, &narrowed_type) {
                            then_ctx.set_narrowed_type(var_name, then_type);
                        }
                    }
                }
            } else {
                // x != nil
                if let Some((var_name, is_nil)) = extract_nil_check(left, right) {
                    if is_nil {
                        // Flip for != operator
                        // then: x is non-nil
                        if let Some(original) = original_types.get(&var_name) {
                            if let Some(non_nil) = remove_nil_from_type(original) {
                                then_ctx.set_narrowed_type(var_name.clone(), non_nil);
                            }
                        }

                        // else: x is nil
                        else_ctx.set_narrowed_type(
                            var_name,
                            Type::new(TypeKind::Primitive(PrimitiveType::Nil), condition.span),
                        );
                    }
                }
            }
        }

        // not condition (flip the branches)
        ExpressionKind::Unary(UnaryOp::Not, operand) => {
            let (inner_then, inner_else) =
                narrow_type_from_condition(operand, base_ctx, original_types);
            return (inner_else, inner_then); // Flip!
        }

        // condition1 and condition2
        ExpressionKind::Binary(BinaryOp::And, left, right) => {
            // First narrow with left condition
            let (left_then, _left_else) =
                narrow_type_from_condition(left, base_ctx, original_types);

            // Then narrow the 'then' branch with right condition
            let (final_then, _final_else) =
                narrow_type_from_condition(right, &left_then, original_types);

            return (final_then, else_ctx);
        }

        // condition1 or condition2
        ExpressionKind::Binary(BinaryOp::Or, left, right) => {
            // For 'or', we narrow in the else branch with the right condition
            let (left_then, left_else) = narrow_type_from_condition(left, base_ctx, original_types);
            let (right_then, right_else) =
                narrow_type_from_condition(right, &left_else, original_types);

            // Then branch: either left or right was true
            let merged_then = NarrowingContext::merge(&left_then, &right_then);

            return (merged_then, right_else);
        }

        // Type guard function call: isString(x)
        ExpressionKind::Call(function, arguments) => {
            if let Some((var_name, narrowed_type)) =
                extract_type_guard_call(function, arguments, original_types)
            {
                // In then branch: narrow to the guarded type
                then_ctx.set_narrowed_type(var_name.clone(), narrowed_type.clone());

                // In else branch: exclude the guarded type
                if let Some(original) = original_types.get(&var_name) {
                    if let Some(else_type) = exclude_type(original, &narrowed_type) {
                        else_ctx.set_narrowed_type(var_name, else_type);
                    }
                }
            }
        }

        // instanceof check: x instanceof ClassName
        ExpressionKind::Binary(BinaryOp::Instanceof, left, right) => {
            if let ExpressionKind::Identifier(var_name) = &left.kind {
                if let ExpressionKind::Identifier(class_name) = &right.kind {
                    // In then branch: narrow to the class type
                    // For now, create a reference to the class type
                    let class_type = Type::new(
                        TypeKind::Reference(crate::ast::types::TypeReference {
                            name: crate::ast::Ident::new(class_name.clone(), condition.span),
                            type_arguments: None,
                            span: condition.span,
                        }),
                        condition.span,
                    );
                    then_ctx.set_narrowed_type(var_name.clone(), class_type.clone());

                    // In else branch: exclude the class type
                    if let Some(original) = original_types.get(var_name) {
                        if let Some(else_type) = exclude_type(original, &class_type) {
                            else_ctx.set_narrowed_type(var_name.clone(), else_type);
                        }
                    }
                }
            }
        }

        // Truthiness check: if x then ...
        ExpressionKind::Identifier(name) => {
            if let Some(original) = original_types.get(name) {
                // In then branch: x is truthy (non-nil, non-false)
                if let Some(truthy_type) = make_truthy_type(original) {
                    then_ctx.set_narrowed_type(name.clone(), truthy_type);
                }

                // In else branch: x is falsy (nil or false)
                if let Some(falsy_type) = make_falsy_type(original) {
                    else_ctx.set_narrowed_type(name.clone(), falsy_type);
                }
            }
        }

        _ => {
            // No narrowing for other expression types
        }
    }

    (then_ctx, else_ctx)
}

/// Extract typeof check: typeof x == "string" -> Some((x, "string"))
fn extract_typeof_check(left: &Expression, right: &Expression) -> Option<(String, String)> {
    // Check: typeof x == "string"
    if let ExpressionKind::Call(function, arguments) = &left.kind {
        if let ExpressionKind::Identifier(func_name) = &function.kind {
            if func_name == "typeof" && arguments.len() == 1 {
                if let ExpressionKind::Identifier(var_name) = &arguments[0].value.kind {
                    if let ExpressionKind::Literal(Literal::String(type_name)) = &right.kind {
                        return Some((var_name.clone(), type_name.clone()));
                    }
                }
            }
        }
    }

    // Check: "string" == typeof x (reversed)
    if let ExpressionKind::Literal(Literal::String(type_name)) = &left.kind {
        if let ExpressionKind::Call(function, arguments) = &right.kind {
            if let ExpressionKind::Identifier(func_name) = &function.kind {
                if func_name == "typeof" && arguments.len() == 1 {
                    if let ExpressionKind::Identifier(var_name) = &arguments[0].value.kind {
                        return Some((var_name.clone(), type_name.clone()));
                    }
                }
            }
        }
    }

    None
}

/// Extract type guard function call: isString(x) -> Some((x, string))
/// Type guards are functions with return type `param is Type`
fn extract_type_guard_call(
    function: &Expression,
    arguments: &[crate::ast::expression::Argument],
    original_types: &FxHashMap<String, Type>,
) -> Option<(String, Type)> {
    // Check if this is a function call with one argument
    if arguments.len() != 1 {
        return None;
    }

    // Get the variable being checked
    let var_name = match &arguments[0].value.kind {
        ExpressionKind::Identifier(name) => name.clone(),
        _ => return None,
    };

    // Try to get the function type from the passed context
    // This allows checking actual type signatures when available
    if let ExpressionKind::Identifier(func_name) = &function.kind {
        // Check if we have type information for this function
        if let Some(func_type) = original_types.get(func_name) {
            // Check if it's a function with a TypePredicate return type
            if let TypeKind::Function(func_sig) = &func_type.kind {
                if let TypeKind::TypePredicate(predicate) = &func_sig.return_type.kind {
                    // Verify the parameter name matches the argument
                    if predicate.parameter_name.node == var_name {
                        return Some((var_name, (*predicate.type_annotation).clone()));
                    }
                }
            }
        }

        // Fallback to heuristic for backwards compatibility:
        // Functions named "is*" are assumed to be type guards
        if let Some(stripped) = func_name.strip_prefix("is") {
            // Extract the type name from the function name (e.g., "isString" -> "string")
            let type_name = stripped.to_lowercase();
            if let Some(narrowed_type) = typeof_string_to_type(&type_name) {
                return Some((var_name, narrowed_type));
            }
        }
    }

    None
}

/// Extract nil check: x == nil -> Some((x, true))
fn extract_nil_check(left: &Expression, right: &Expression) -> Option<(String, bool)> {
    // Check: x == nil
    if let ExpressionKind::Identifier(var_name) = &left.kind {
        if let ExpressionKind::Literal(Literal::Nil) = &right.kind {
            return Some((var_name.clone(), true));
        }
    }

    // Check: nil == x (reversed)
    if let ExpressionKind::Literal(Literal::Nil) = &left.kind {
        if let ExpressionKind::Identifier(var_name) = &right.kind {
            return Some((var_name.clone(), true));
        }
    }

    None
}

/// Convert typeof string to a type
fn typeof_string_to_type(type_name: &str) -> Option<Type> {
    let span = crate::span::Span::new(0, 0, 0, 0);
    match type_name {
        "nil" => Some(Type::new(TypeKind::Primitive(PrimitiveType::Nil), span)),
        "boolean" => Some(Type::new(TypeKind::Primitive(PrimitiveType::Boolean), span)),
        "number" => Some(Type::new(TypeKind::Primitive(PrimitiveType::Number), span)),
        "string" => Some(Type::new(TypeKind::Primitive(PrimitiveType::String), span)),
        "table" => Some(Type::new(TypeKind::Primitive(PrimitiveType::Table), span)),
        _ => None,
    }
}

/// Exclude a type from a union
fn exclude_type(typ: &Type, to_exclude: &Type) -> Option<Type> {
    match &typ.kind {
        TypeKind::Union(types) => {
            let remaining: Vec<Type> = types
                .iter()
                .filter(|t| !types_equal(t, to_exclude))
                .cloned()
                .collect();

            if remaining.is_empty() {
                Some(Type::new(
                    TypeKind::Primitive(PrimitiveType::Never),
                    typ.span,
                ))
            } else if remaining.len() == 1 {
                Some(remaining.into_iter().next().unwrap())
            } else {
                Some(Type::new(TypeKind::Union(remaining), typ.span))
            }
        }
        _ if types_equal(typ, to_exclude) => Some(Type::new(
            TypeKind::Primitive(PrimitiveType::Never),
            typ.span,
        )),
        _ => Some(typ.clone()),
    }
}

/// Remove nil from a type (for non-nil narrowing)
fn remove_nil_from_type(typ: &Type) -> Option<Type> {
    match &typ.kind {
        TypeKind::Union(types) => {
            let remaining: Vec<Type> = types.iter().filter(|t| !is_nil_type(t)).cloned().collect();

            if remaining.is_empty() {
                Some(Type::new(
                    TypeKind::Primitive(PrimitiveType::Never),
                    typ.span,
                ))
            } else if remaining.len() == 1 {
                Some(remaining.into_iter().next().unwrap())
            } else {
                Some(Type::new(TypeKind::Union(remaining), typ.span))
            }
        }
        _ if is_nil_type(typ) => Some(Type::new(
            TypeKind::Primitive(PrimitiveType::Never),
            typ.span,
        )),
        _ => Some(typ.clone()),
    }
}

/// Check if a type is nil (handles both Literal(Nil) and Primitive(Nil))
fn is_nil_type(typ: &Type) -> bool {
    matches!(
        typ.kind,
        TypeKind::Primitive(PrimitiveType::Nil) | TypeKind::Literal(Literal::Nil)
    )
}

/// Make a type truthy (remove nil and false)
fn make_truthy_type(typ: &Type) -> Option<Type> {
    match &typ.kind {
        TypeKind::Union(types) => {
            let truthy: Vec<Type> = types
                .iter()
                .filter(|t| !is_falsy_type(t))
                .cloned()
                .collect();

            if truthy.is_empty() {
                Some(Type::new(
                    TypeKind::Primitive(PrimitiveType::Never),
                    typ.span,
                ))
            } else if truthy.len() == 1 {
                Some(truthy.into_iter().next().unwrap())
            } else {
                Some(Type::new(TypeKind::Union(truthy), typ.span))
            }
        }
        _ if is_falsy_type(typ) => Some(Type::new(
            TypeKind::Primitive(PrimitiveType::Never),
            typ.span,
        )),
        _ => Some(typ.clone()),
    }
}

/// Make a type falsy (only nil or false)
fn make_falsy_type(typ: &Type) -> Option<Type> {
    match &typ.kind {
        TypeKind::Union(types) => {
            let falsy: Vec<Type> = types.iter().filter(|t| is_falsy_type(t)).cloned().collect();

            if falsy.is_empty() {
                Some(Type::new(
                    TypeKind::Primitive(PrimitiveType::Never),
                    typ.span,
                ))
            } else if falsy.len() == 1 {
                Some(falsy.into_iter().next().unwrap())
            } else {
                Some(Type::new(TypeKind::Union(falsy), typ.span))
            }
        }
        _ if is_falsy_type(typ) => Some(typ.clone()),
        _ => Some(Type::new(
            TypeKind::Primitive(PrimitiveType::Never),
            typ.span,
        )),
    }
}

/// Check if a type is falsy (nil or false)
fn is_falsy_type(typ: &Type) -> bool {
    matches!(
        typ.kind,
        TypeKind::Primitive(PrimitiveType::Nil)
            | TypeKind::Literal(Literal::Nil)
            | TypeKind::Literal(Literal::Boolean(false))
    )
}

/// Simple type equality check
fn types_equal(t1: &Type, t2: &Type) -> bool {
    match (&t1.kind, &t2.kind) {
        (TypeKind::Primitive(p1), TypeKind::Primitive(p2)) => p1 == p2,
        (TypeKind::Literal(l1), TypeKind::Literal(l2)) => l1 == l2,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;

    fn make_span() -> Span {
        Span::new(0, 0, 0, 0)
    }

    #[test]
    fn test_narrowing_context_basic() {
        let mut ctx = NarrowingContext::new();

        let string_type = Type::new(TypeKind::Primitive(PrimitiveType::String), make_span());
        ctx.set_narrowed_type("x".to_string(), string_type.clone());

        assert!(ctx.get_narrowed_type("x").is_some());
        assert!(ctx.get_narrowed_type("y").is_none());

        ctx.remove_narrowed_type("x");
        assert!(ctx.get_narrowed_type("x").is_none());
    }

    #[test]
    fn test_narrowing_context_merge() {
        let mut then_ctx = NarrowingContext::new();
        let mut else_ctx = NarrowingContext::new();

        let string_type = Type::new(TypeKind::Primitive(PrimitiveType::String), make_span());
        let number_type = Type::new(TypeKind::Primitive(PrimitiveType::Number), make_span());

        // Both have 'x' as string - should be kept
        then_ctx.set_narrowed_type("x".to_string(), string_type.clone());
        else_ctx.set_narrowed_type("x".to_string(), string_type.clone());

        // Only then has 'y' - should not be kept
        then_ctx.set_narrowed_type("y".to_string(), number_type.clone());

        let merged = NarrowingContext::merge(&then_ctx, &else_ctx);

        assert!(merged.get_narrowed_type("x").is_some());
        assert!(merged.get_narrowed_type("y").is_none());
    }

    #[test]
    fn test_remove_nil_from_union() {
        let union_type = Type::new(
            TypeKind::Union(vec![
                Type::new(TypeKind::Primitive(PrimitiveType::String), make_span()),
                Type::new(TypeKind::Primitive(PrimitiveType::Nil), make_span()),
            ]),
            make_span(),
        );

        let non_nil = remove_nil_from_type(&union_type).unwrap();

        assert!(matches!(
            non_nil.kind,
            TypeKind::Primitive(PrimitiveType::String)
        ));
    }

    #[test]
    fn test_make_truthy_type() {
        let union_type = Type::new(
            TypeKind::Union(vec![
                Type::new(TypeKind::Primitive(PrimitiveType::String), make_span()),
                Type::new(TypeKind::Primitive(PrimitiveType::Nil), make_span()),
                Type::new(TypeKind::Literal(Literal::Boolean(false)), make_span()),
                Type::new(TypeKind::Primitive(PrimitiveType::Number), make_span()),
            ]),
            make_span(),
        );

        let truthy = make_truthy_type(&union_type).unwrap();

        if let TypeKind::Union(types) = &truthy.kind {
            assert_eq!(types.len(), 2); // string and number
        } else {
            panic!("Expected union type");
        }
    }
}
