use crate::ast::expression::Literal;
use crate::ast::types::{FunctionType, ObjectType, ObjectTypeMember, PrimitiveType, Type, TypeKind};

/// Type compatibility checker
pub struct TypeCompatibility;

impl TypeCompatibility {
    /// Check if `source` is assignable to `target`
    pub fn is_assignable(source: &Type, target: &Type) -> bool {
        // Unknown is assignable to/from anything
        if matches!(source.kind, TypeKind::Primitive(PrimitiveType::Unknown))
            || matches!(target.kind, TypeKind::Primitive(PrimitiveType::Unknown))
        {
            return true;
        }

        // Never is assignable to anything
        if matches!(source.kind, TypeKind::Primitive(PrimitiveType::Never)) {
            return true;
        }

        // Nothing is assignable to Never
        if matches!(target.kind, TypeKind::Primitive(PrimitiveType::Never)) {
            return false;
        }

        match (&source.kind, &target.kind) {
            // Primitive types
            (TypeKind::Primitive(s), TypeKind::Primitive(t)) => {
                Self::is_primitive_assignable(*s, *t)
            }

            // Literal types
            (TypeKind::Literal(s_lit), TypeKind::Literal(t_lit)) => s_lit == t_lit,

            // Literal to primitive
            (TypeKind::Literal(lit), TypeKind::Primitive(prim)) => {
                Self::is_literal_assignable_to_primitive(lit, *prim)
            }

            // Union types
            (_, TypeKind::Union(targets)) => {
                // Source is assignable to union if assignable to any member
                targets.iter().any(|t| Self::is_assignable(source, t))
            }
            (TypeKind::Union(sources), _) => {
                // Union is assignable to target if all members are assignable
                sources.iter().all(|s| Self::is_assignable(s, target))
            }

            // Intersection types
            (TypeKind::Intersection(sources), _) => {
                // Intersection is assignable to target if any member is assignable
                sources.iter().any(|s| Self::is_assignable(s, target))
            }
            (_, TypeKind::Intersection(targets)) => {
                // Source is assignable to intersection if assignable to all members
                targets.iter().all(|t| Self::is_assignable(source, t))
            }

            // Array types
            (TypeKind::Array(s_elem), TypeKind::Array(t_elem)) => {
                Self::is_assignable(s_elem, t_elem)
            }

            // Tuple types
            (TypeKind::Tuple(s_elems), TypeKind::Tuple(t_elems)) => {
                if s_elems.len() != t_elems.len() {
                    return false;
                }
                s_elems
                    .iter()
                    .zip(t_elems.iter())
                    .all(|(s, t)| Self::is_assignable(s, t))
            }

            // Function types
            (TypeKind::Function(s_func), TypeKind::Function(t_func)) => {
                Self::is_function_assignable(s_func, t_func)
            }

            // Object types
            (TypeKind::Object(s_obj), TypeKind::Object(t_obj)) => {
                Self::is_object_assignable(s_obj, t_obj)
            }

            // Nullable types
            (TypeKind::Nullable(s_inner), TypeKind::Nullable(t_inner)) => {
                Self::is_assignable(s_inner, t_inner)
            }
            (TypeKind::Primitive(PrimitiveType::Nil), TypeKind::Nullable(_)) => true,
            (_, TypeKind::Nullable(t_inner)) => Self::is_assignable(source, t_inner),

            // Parenthesized types
            (TypeKind::Parenthesized(s_inner), _) => Self::is_assignable(s_inner, target),
            (_, TypeKind::Parenthesized(t_inner)) => Self::is_assignable(source, t_inner),

            // Type references - for now, names must match exactly
            
            (TypeKind::Reference(s_ref), TypeKind::Reference(t_ref)) => {
                s_ref.name.node == t_ref.name.node
            }

            _ => false,
        }
    }

    /// Check if primitive types are compatible
    fn is_primitive_assignable(source: PrimitiveType, target: PrimitiveType) -> bool {
        if source == target {
            return true;
        }

        match (source, target) {
            // Integer is assignable to number
            (PrimitiveType::Integer, PrimitiveType::Number) => true,
            _ => false,
        }
    }

    /// Check if a literal is assignable to a primitive type
    fn is_literal_assignable_to_primitive(lit: &Literal, prim: PrimitiveType) -> bool {
        matches!(
            (lit, prim),
            (Literal::Number(_), PrimitiveType::Number)
                | (Literal::String(_), PrimitiveType::String)
                | (Literal::Boolean(_), PrimitiveType::Boolean)
                | (Literal::Nil, PrimitiveType::Nil)
        )
    }

    /// Check function type compatibility (contravariant parameters, covariant return)
    fn is_function_assignable(source: &FunctionType, target: &FunctionType) -> bool {
        // Check parameter count
        if source.parameters.len() != target.parameters.len() {
            return false;
        }

        // Parameters are contravariant: target params must be assignable to source params
        for (s_param, t_param) in source.parameters.iter().zip(target.parameters.iter()) {
            if let (Some(s_type), Some(t_type)) = (&s_param.type_annotation, &t_param.type_annotation) {
                if !Self::is_assignable(t_type, s_type) {
                    return false;
                }
            }
        }

        // Return type is covariant: source return must be assignable to target return
        Self::is_assignable(&source.return_type, &target.return_type)
    }

    /// Check object type structural compatibility
    fn is_object_assignable(source: &ObjectType, target: &ObjectType) -> bool {
        // For each property in target, source must have a compatible property
        for t_member in &target.members {
            match t_member {
                ObjectTypeMember::Property(t_prop) => {
                    // Find corresponding property in source
                    let found = source.members.iter().any(|s_member| {
                        if let ObjectTypeMember::Property(s_prop) = s_member {
                            s_prop.name.node == t_prop.name.node
                                && Self::is_assignable(&s_prop.type_annotation, &t_prop.type_annotation)
                        } else {
                            false
                        }
                    });

                    if !found && !t_prop.is_optional {
                        return false;
                    }
                }
                ObjectTypeMember::Method(t_method) => {
                    // Find corresponding method in source
                    let found = source.members.iter().any(|s_member| {
                        if let ObjectTypeMember::Method(s_method) = s_member {
                            s_method.name.node == t_method.name.node
                        
                        } else {
                            false
                        }
                    });

                    if !found {
                        return false;
                    }
                }
                ObjectTypeMember::Index(_) => {
                    
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;

    fn make_type(kind: TypeKind) -> Type {
        Type::new(kind, Span::new(0, 0, 0, 0))
    }

    #[test]
    fn test_primitive_assignability() {
        let number = make_type(TypeKind::Primitive(PrimitiveType::Number));
        let string = make_type(TypeKind::Primitive(PrimitiveType::String));
        let integer = make_type(TypeKind::Primitive(PrimitiveType::Integer));

        assert!(TypeCompatibility::is_assignable(&number, &number));
        assert!(!TypeCompatibility::is_assignable(&number, &string));
        assert!(TypeCompatibility::is_assignable(&integer, &number));
        assert!(!TypeCompatibility::is_assignable(&number, &integer));
    }

    #[test]
    fn test_literal_assignability() {
        let num_lit = make_type(TypeKind::Literal(Literal::Number(42.0)));
        let str_lit = make_type(TypeKind::Literal(Literal::String("hello".to_string())));
        let number = make_type(TypeKind::Primitive(PrimitiveType::Number));
        let string = make_type(TypeKind::Primitive(PrimitiveType::String));

        assert!(TypeCompatibility::is_assignable(&num_lit, &number));
        assert!(!TypeCompatibility::is_assignable(&num_lit, &string));
        assert!(TypeCompatibility::is_assignable(&str_lit, &string));
        assert!(!TypeCompatibility::is_assignable(&str_lit, &number));
    }

    #[test]
    fn test_union_assignability() {
        let number = make_type(TypeKind::Primitive(PrimitiveType::Number));
        let string = make_type(TypeKind::Primitive(PrimitiveType::String));
        let number_or_string = make_type(TypeKind::Union(vec![number.clone(), string.clone()]));

        // number is assignable to number | string
        assert!(TypeCompatibility::is_assignable(&number, &number_or_string));
        // string is assignable to number | string
        assert!(TypeCompatibility::is_assignable(&string, &number_or_string));
    }

    #[test]
    fn test_array_assignability() {
        let number = make_type(TypeKind::Primitive(PrimitiveType::Number));
        let string = make_type(TypeKind::Primitive(PrimitiveType::String));
        let number_array = make_type(TypeKind::Array(Box::new(number.clone())));
        let string_array = make_type(TypeKind::Array(Box::new(string.clone())));

        assert!(TypeCompatibility::is_assignable(&number_array, &number_array));
        assert!(!TypeCompatibility::is_assignable(&number_array, &string_array));
    }

    #[test]
    fn test_nullable_assignability() {
        let number = make_type(TypeKind::Primitive(PrimitiveType::Number));
        let nullable_number = make_type(TypeKind::Nullable(Box::new(number.clone())));
        let nil = make_type(TypeKind::Primitive(PrimitiveType::Nil));

        // nil is assignable to number?
        assert!(TypeCompatibility::is_assignable(&nil, &nullable_number));
        // number is assignable to number?
        assert!(TypeCompatibility::is_assignable(&number, &nullable_number));
    }

    #[test]
    fn test_unknown_assignability() {
        let unknown = make_type(TypeKind::Primitive(PrimitiveType::Unknown));
        let number = make_type(TypeKind::Primitive(PrimitiveType::Number));

        // unknown is assignable to/from anything
        assert!(TypeCompatibility::is_assignable(&unknown, &number));
        assert!(TypeCompatibility::is_assignable(&number, &unknown));
    }

    #[test]
    fn test_never_assignability() {
        let never = make_type(TypeKind::Primitive(PrimitiveType::Never));
        let number = make_type(TypeKind::Primitive(PrimitiveType::Number));

        // never is assignable to anything
        assert!(TypeCompatibility::is_assignable(&never, &number));
        // nothing (except never) is assignable to never
        assert!(!TypeCompatibility::is_assignable(&number, &never));
        assert!(TypeCompatibility::is_assignable(&never, &never));
    }
}
