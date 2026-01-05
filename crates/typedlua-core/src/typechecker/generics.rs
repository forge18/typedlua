use crate::ast::statement::TypeParameter;
use crate::ast::types::{Type, TypeKind, TypeReference};
use rustc_hash::FxHashMap;

#[cfg(test)]
use crate::span::Span;

/// Substitutes type parameters with concrete types in a type
pub fn instantiate_type(
    typ: &Type,
    type_params: &[TypeParameter],
    type_args: &[Type],
) -> Result<Type, String> {
    if type_params.len() != type_args.len() {
        return Err(format!(
            "Expected {} type arguments, but got {}",
            type_params.len(),
            type_args.len()
        ));
    }

    // Build substitution map
    let mut substitutions: FxHashMap<String, Type> = FxHashMap::default();
    for (param, arg) in type_params.iter().zip(type_args.iter()) {
        substitutions.insert(param.name.node.clone(), arg.clone());
    }

    substitute_type(typ, &substitutions)
}

/// Recursively substitute type parameters in a type
fn substitute_type(typ: &Type, substitutions: &FxHashMap<String, Type>) -> Result<Type, String> {
    match &typ.kind {
        // If this is a type reference that matches a type parameter, substitute it
        TypeKind::Reference(type_ref) => {
            let name = &type_ref.name.node;

            // Check if this is a type parameter
            if let Some(substituted) = substitutions.get(name) {
                // Apply type arguments if present (e.g., for higher-kinded types)
                if let Some(ref args) = type_ref.type_arguments {
                    // This would be a higher-kinded type - not common, but we should handle it
                    // For now, just return an error
                    if !args.is_empty() {
                        return Err(format!(
                            "Type parameter '{}' cannot have type arguments",
                            name
                        ));
                    }
                }
                Ok(substituted.clone())
            } else {
                // Not a type parameter - recursively substitute in type arguments
                if let Some(ref args) = type_ref.type_arguments {
                    let substituted_args: Result<Vec<_>, _> = args
                        .iter()
                        .map(|arg| substitute_type(arg, substitutions))
                        .collect();

                    Ok(Type::new(
                        TypeKind::Reference(TypeReference {
                            name: type_ref.name.clone(),
                            type_arguments: Some(substituted_args?),
                            span: type_ref.span,
                        }),
                        typ.span,
                    ))
                } else {
                    Ok(typ.clone())
                }
            }
        }

        // Array type: substitute element type
        TypeKind::Array(elem) => {
            let substituted_elem = substitute_type(elem, substitutions)?;
            Ok(Type::new(
                TypeKind::Array(Box::new(substituted_elem)),
                typ.span,
            ))
        }

        // Tuple type: substitute each element
        TypeKind::Tuple(elems) => {
            let substituted_elems: Result<Vec<_>, _> = elems
                .iter()
                .map(|elem| substitute_type(elem, substitutions))
                .collect();

            Ok(Type::new(TypeKind::Tuple(substituted_elems?), typ.span))
        }

        // Union type: substitute each member
        TypeKind::Union(members) => {
            let substituted_members: Result<Vec<_>, _> = members
                .iter()
                .map(|member| substitute_type(member, substitutions))
                .collect();

            Ok(Type::new(TypeKind::Union(substituted_members?), typ.span))
        }

        // Intersection type: substitute each member
        TypeKind::Intersection(members) => {
            let substituted_members: Result<Vec<_>, _> = members
                .iter()
                .map(|member| substitute_type(member, substitutions))
                .collect();

            Ok(Type::new(
                TypeKind::Intersection(substituted_members?),
                typ.span,
            ))
        }

        // Function type: substitute parameter and return types
        TypeKind::Function(func_type) => {
            use crate::ast::statement::Parameter;

            let substituted_params: Result<Vec<Parameter>, String> = func_type
                .parameters
                .iter()
                .map(|param| {
                    if let Some(ref type_ann) = param.type_annotation {
                        let substituted = substitute_type(type_ann, substitutions)?;
                        Ok(Parameter {
                            pattern: param.pattern.clone(),
                            type_annotation: Some(substituted),
                            default: param.default.clone(),
                            is_rest: param.is_rest,
                            is_optional: param.is_optional,
                            span: param.span,
                        })
                    } else {
                        Ok(param.clone())
                    }
                })
                .collect();

            let substituted_return = substitute_type(&func_type.return_type, substitutions)?;

            Ok(Type::new(
                TypeKind::Function(crate::ast::types::FunctionType {
                    parameters: substituted_params?,
                    return_type: Box::new(substituted_return),
                    span: func_type.span,
                }),
                typ.span,
            ))
        }

        // Nullable type: substitute inner type
        TypeKind::Nullable(inner) => {
            let substituted_inner = substitute_type(inner, substitutions)?;
            Ok(Type::new(
                TypeKind::Nullable(Box::new(substituted_inner)),
                typ.span,
            ))
        }

        // Parenthesized type: substitute inner type
        TypeKind::Parenthesized(inner) => {
            let substituted_inner = substitute_type(inner, substitutions)?;
            Ok(Type::new(
                TypeKind::Parenthesized(Box::new(substituted_inner)),
                typ.span,
            ))
        }

        // Object types, conditional types, mapped types, etc. would need similar handling
        // For now, just clone types that don't contain type parameters
        _ => Ok(typ.clone()),
    }
}

/// Check if type arguments satisfy type parameter constraints
pub fn check_type_constraints(
    type_params: &[TypeParameter],
    type_args: &[Type],
) -> Result<(), String> {
    if type_params.len() != type_args.len() {
        return Err(format!(
            "Expected {} type arguments, but got {}",
            type_params.len(),
            type_args.len()
        ));
    }

    for (param, arg) in type_params.iter().zip(type_args.iter()) {
        if let Some(ref constraint) = param.constraint {
            // Check if arg is assignable to constraint
            // This is a simplified check - a real implementation would use TypeCompatibility
            // For now, we'll just do a basic check
            if !is_type_compatible(arg, constraint) {
                return Err(format!(
                    "Type argument does not satisfy constraint for parameter '{}'",
                    param.name.node
                ));
            }
        }
    }

    Ok(())
}

/// Check if a type is compatible with a constraint
/// Uses the TypeCompatibility module for proper checking
fn is_type_compatible(arg: &Type, constraint: &Type) -> bool {
    use super::type_compat::TypeCompatibility;
    TypeCompatibility::is_assignable(arg, constraint)
}

/// Infer type arguments for a generic function from argument types
/// Returns a map from type parameter name to inferred type
pub fn infer_type_arguments(
    type_params: &[TypeParameter],
    function_params: &[crate::ast::statement::Parameter],
    arg_types: &[Type],
) -> Result<Vec<Type>, String> {
    if function_params.len() != arg_types.len() {
        return Err(format!(
            "Expected {} arguments, got {}",
            function_params.len(),
            arg_types.len()
        ));
    }

    let mut inferred: FxHashMap<String, Type> = FxHashMap::default();

    // For each parameter-argument pair, try to infer type arguments
    for (param, arg_type) in function_params.iter().zip(arg_types.iter()) {
        if let Some(param_type) = &param.type_annotation {
            infer_from_types(param_type, arg_type, &mut inferred)?;
        }
    }

    // Build result vector in the same order as type parameters
    let mut result = Vec::new();
    for type_param in type_params {
        if let Some(inferred_type) = inferred.get(&type_param.name.node) {
            result.push(inferred_type.clone());
        } else if let Some(default) = &type_param.default {
            // Use default type if no inference
            result.push((**default).clone());
        } else {
            return Err(format!(
                "Could not infer type argument for parameter '{}'",
                type_param.name.node
            ));
        }
    }

    Ok(result)
}

/// Helper to infer type arguments by matching param_type pattern against arg_type
fn infer_from_types(
    param_type: &Type,
    arg_type: &Type,
    inferred: &mut FxHashMap<String, Type>,
) -> Result<(), String> {
    match &param_type.kind {
        // If parameter is a type reference (e.g., T), and it's a type parameter
        TypeKind::Reference(type_ref) if type_ref.type_arguments.is_none() => {
            // This might be a type parameter - record the inference
            let param_name = type_ref.name.node.clone();

            // Check if we already inferred this type parameter
            if let Some(existing) = inferred.get(&param_name) {
                // Verify they match (simplified - should use proper type equality)
                if !types_equal(existing, arg_type) {
                    return Err(format!(
                        "Conflicting type inference for parameter '{}'",
                        param_name
                    ));
                }
            } else {
                inferred.insert(param_name, arg_type.clone());
            }
            Ok(())
        }

        // If parameter is Array<T>, and argument is Array<U>, infer T = U
        TypeKind::Array(elem_param) => {
            if let TypeKind::Array(elem_arg) = &arg_type.kind {
                infer_from_types(elem_param, elem_arg, inferred)
            } else {
                Ok(()) // Type mismatch, but don't error during inference
            }
        }

        // If parameter is a generic type application like Map<K, V>
        TypeKind::Reference(type_ref) if type_ref.type_arguments.is_some() => {
            if let TypeKind::Reference(arg_ref) = &arg_type.kind {
                // Names should match
                if type_ref.name.node == arg_ref.name.node {
                    if let (Some(param_args), Some(arg_args)) =
                        (&type_ref.type_arguments, &arg_ref.type_arguments) {
                        // Infer from each type argument pair
                        for (p, a) in param_args.iter().zip(arg_args.iter()) {
                            infer_from_types(p, a, inferred)?;
                        }
                    }
                }
            }
            Ok(())
        }

        // For other types, no inference needed
        _ => Ok(()),
    }
}

/// Simple type equality check (simplified)
fn types_equal(t1: &Type, t2: &Type) -> bool {
    // Simplified - just check if both are the same primitive
    match (&t1.kind, &t2.kind) {
        (TypeKind::Primitive(p1), TypeKind::Primitive(p2)) => p1 == p2,
        (TypeKind::Reference(r1), TypeKind::Reference(r2)) => r1.name.node == r2.name.node,
        _ => false, // For now, consider other types as not equal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::types::{PrimitiveType, TypeKind};
    use crate::ast::Spanned;

    #[test]
    fn test_instantiate_simple_type() {
        let span = Span::new(0, 0, 0, 0);

        // Type parameter T
        let type_param = TypeParameter {
            name: Spanned::new("T".to_string(), span),
            constraint: None,
            default: None,
            span,
        };

        // Type reference T
        let type_ref_t = Type::new(
            TypeKind::Reference(TypeReference {
                name: Spanned::new("T".to_string(), span),
                type_arguments: None,
                span,
            }),
            span,
        );

        // Type argument: number
        let number_type = Type::new(
            TypeKind::Primitive(PrimitiveType::Number),
            span,
        );

        // Instantiate T with number
        let result = instantiate_type(&type_ref_t, &[type_param], &[number_type.clone()]).unwrap();

        // Result should be number
        assert!(matches!(result.kind, TypeKind::Primitive(PrimitiveType::Number)));
    }

    #[test]
    fn test_instantiate_array_type() {
        let span = Span::new(0, 0, 0, 0);

        // Type parameter T
        let type_param = TypeParameter {
            name: Spanned::new("T".to_string(), span),
            constraint: None,
            default: None,
            span,
        };

        // Array<T>
        let array_t = Type::new(
            TypeKind::Array(Box::new(Type::new(
                TypeKind::Reference(TypeReference {
                    name: Spanned::new("T".to_string(), span),
                    type_arguments: None,
                    span,
                }),
                span,
            ))),
            span,
        );

        // Type argument: string
        let string_type = Type::new(
            TypeKind::Primitive(PrimitiveType::String),
            span,
        );

        // Instantiate Array<T> with string
        let result = instantiate_type(&array_t, &[type_param], &[string_type]).unwrap();

        // Result should be Array<string>
        match &result.kind {
            TypeKind::Array(elem) => {
                assert!(matches!(elem.kind, TypeKind::Primitive(PrimitiveType::String)));
            }
            _ => panic!("Expected array type"),
        }
    }

    #[test]
    fn test_wrong_number_of_type_args() {
        let span = Span::new(0, 0, 0, 0);

        // Type parameter T
        let type_param = TypeParameter {
            name: Spanned::new("T".to_string(), span),
            constraint: None,
            default: None,
            span,
        };

        let type_ref_t = Type::new(
            TypeKind::Reference(TypeReference {
                name: Spanned::new("T".to_string(), span),
                type_arguments: None,
                span,
            }),
            span,
        );

        let number_type = Type::new(
            TypeKind::Primitive(PrimitiveType::Number),
            span,
        );

        let string_type = Type::new(
            TypeKind::Primitive(PrimitiveType::String),
            span,
        );

        // Try to instantiate with wrong number of type arguments
        let result = instantiate_type(
            &type_ref_t,
            &[type_param],
            &[number_type, string_type],
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_infer_type_arguments_simple() {
        use crate::ast::pattern::Pattern;
        use crate::ast::statement::Parameter;

        let span = Span::new(0, 0, 0, 0);

        // Type parameter T
        let type_param = TypeParameter {
            name: Spanned::new("T".to_string(), span),
            constraint: None,
            default: None,
            span,
        };

        // Function parameter: (value: T)
        let func_param = Parameter {
            pattern: Pattern::Identifier(Spanned::new("value".to_string(), span)),
            type_annotation: Some(Type::new(
                TypeKind::Reference(TypeReference {
                    name: Spanned::new("T".to_string(), span),
                    type_arguments: None,
                    span,
                }),
                span,
            )),
            default: None,
            is_rest: false,
            is_optional: false,
            span,
        };

        // Argument type: number
        let arg_type = Type::new(TypeKind::Primitive(PrimitiveType::Number), span);

        // Infer type arguments
        let result = infer_type_arguments(&[type_param], &[func_param], &[arg_type]);

        assert!(result.is_ok());
        let inferred = result.unwrap();
        assert_eq!(inferred.len(), 1);
        assert!(matches!(inferred[0].kind, TypeKind::Primitive(PrimitiveType::Number)));
    }

    #[test]
    fn test_infer_type_arguments_array() {
        use crate::ast::pattern::Pattern;
        use crate::ast::statement::Parameter;

        let span = Span::new(0, 0, 0, 0);

        // Type parameter T
        let type_param = TypeParameter {
            name: Spanned::new("T".to_string(), span),
            constraint: None,
            default: None,
            span,
        };

        // Function parameter: (values: Array<T>)
        let func_param = Parameter {
            pattern: Pattern::Identifier(Spanned::new("values".to_string(), span)),
            type_annotation: Some(Type::new(
                TypeKind::Array(Box::new(Type::new(
                    TypeKind::Reference(TypeReference {
                        name: Spanned::new("T".to_string(), span),
                        type_arguments: None,
                        span,
                    }),
                    span,
                ))),
                span,
            )),
            default: None,
            is_rest: false,
            is_optional: false,
            span,
        };

        // Argument type: Array<string>
        let arg_type = Type::new(
            TypeKind::Array(Box::new(Type::new(
                TypeKind::Primitive(PrimitiveType::String),
                span,
            ))),
            span,
        );

        // Infer type arguments
        let result = infer_type_arguments(&[type_param], &[func_param], &[arg_type]);

        assert!(result.is_ok());
        let inferred = result.unwrap();
        assert_eq!(inferred.len(), 1);
        assert!(matches!(inferred[0].kind, TypeKind::Primitive(PrimitiveType::String)));
    }

    #[test]
    fn test_check_constraints_pass() {
        let span = Span::new(0, 0, 0, 0);

        // Type parameter T extends number
        let type_param = TypeParameter {
            name: Spanned::new("T".to_string(), span),
            constraint: Some(Box::new(Type::new(
                TypeKind::Primitive(PrimitiveType::Number),
                span,
            ))),
            default: None,
            span,
        };

        // Type argument: number (should satisfy constraint)
        let number_type = Type::new(TypeKind::Primitive(PrimitiveType::Number), span);

        let result = check_type_constraints(&[type_param], &[number_type]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_constraints_fail() {
        let span = Span::new(0, 0, 0, 0);

        // Type parameter T extends number
        let type_param = TypeParameter {
            name: Spanned::new("T".to_string(), span),
            constraint: Some(Box::new(Type::new(
                TypeKind::Primitive(PrimitiveType::Number),
                span,
            ))),
            default: None,
            span,
        };

        // Type argument: string (should NOT satisfy constraint)
        let string_type = Type::new(TypeKind::Primitive(PrimitiveType::String), span);

        let result = check_type_constraints(&[type_param], &[string_type]);
        assert!(result.is_err());
    }
}
