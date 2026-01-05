use crate::ast::statement::TypeParameter;
use crate::ast::types::{PrimitiveType, Type, TypeKind};
use crate::span::Span;
use rustc_hash::FxHashMap;

/// A generic type alias with type parameters
#[derive(Debug, Clone)]
pub struct GenericTypeAlias {
    pub type_parameters: Vec<TypeParameter>,
    pub typ: Type,
}

/// Type environment managing type aliases and interfaces
#[derive(Debug)]
pub struct TypeEnvironment {
    /// Type aliases (type Foo = ...)
    type_aliases: FxHashMap<String, Type>,
    /// Generic type aliases (type Foo<T> = ...)
    generic_type_aliases: FxHashMap<String, GenericTypeAlias>,
    /// Interface types
    interfaces: FxHashMap<String, Type>,
    /// Built-in types
    builtins: FxHashMap<String, Type>,
    /// Currently resolving types (for cycle detection)
    resolving: std::cell::RefCell<std::collections::HashSet<String>>,
}

impl TypeEnvironment {
    pub fn new() -> Self {
        let mut env = Self {
            type_aliases: FxHashMap::default(),
            generic_type_aliases: FxHashMap::default(),
            interfaces: FxHashMap::default(),
            builtins: FxHashMap::default(),
            resolving: std::cell::RefCell::new(std::collections::HashSet::new()),
        };

        env.register_builtins();
        env
    }

    /// Register built-in types
    fn register_builtins(&mut self) {
        let span = Span::new(0, 0, 0, 0);

        // Primitive types
        self.builtins.insert(
            "nil".to_string(),
            Type::new(TypeKind::Primitive(PrimitiveType::Nil), span),
        );
        self.builtins.insert(
            "boolean".to_string(),
            Type::new(TypeKind::Primitive(PrimitiveType::Boolean), span),
        );
        self.builtins.insert(
            "number".to_string(),
            Type::new(TypeKind::Primitive(PrimitiveType::Number), span),
        );
        self.builtins.insert(
            "integer".to_string(),
            Type::new(TypeKind::Primitive(PrimitiveType::Integer), span),
        );
        self.builtins.insert(
            "string".to_string(),
            Type::new(TypeKind::Primitive(PrimitiveType::String), span),
        );
        self.builtins.insert(
            "unknown".to_string(),
            Type::new(TypeKind::Primitive(PrimitiveType::Unknown), span),
        );
        self.builtins.insert(
            "never".to_string(),
            Type::new(TypeKind::Primitive(PrimitiveType::Never), span),
        );
        self.builtins.insert(
            "void".to_string(),
            Type::new(TypeKind::Primitive(PrimitiveType::Void), span),
        );
        self.builtins.insert(
            "table".to_string(),
            Type::new(TypeKind::Primitive(PrimitiveType::Table), span),
        );
        self.builtins.insert(
            "coroutine".to_string(),
            Type::new(TypeKind::Primitive(PrimitiveType::Coroutine), span),
        );
    }

    /// Register a type alias
    pub fn register_type_alias(&mut self, name: String, typ: Type) -> Result<(), String> {
        if self.type_aliases.contains_key(&name) {
            return Err(format!("Type alias '{}' already defined", name));
        }
        self.type_aliases.insert(name, typ);
        Ok(())
    }

    /// Register a generic type alias
    pub fn register_generic_type_alias(
        &mut self,
        name: String,
        type_parameters: Vec<TypeParameter>,
        typ: Type,
    ) -> Result<(), String> {
        if self.generic_type_aliases.contains_key(&name) {
            return Err(format!("Generic type alias '{}' already defined", name));
        }
        self.generic_type_aliases.insert(name, GenericTypeAlias { type_parameters, typ });
        Ok(())
    }

    /// Register an interface
    pub fn register_interface(&mut self, name: String, typ: Type) -> Result<(), String> {
        if self.interfaces.contains_key(&name) {
            return Err(format!("Interface '{}' already defined", name));
        }
        self.interfaces.insert(name, typ);
        Ok(())
    }

    /// Look up a type by name (checks type aliases, interfaces, and builtins)
    pub fn lookup_type(&self, name: &str) -> Option<&Type> {
        self.type_aliases
            .get(name)
            .or_else(|| self.interfaces.get(name))
            .or_else(|| self.builtins.get(name))
    }

    /// Look up a type alias
    pub fn lookup_type_alias(&self, name: &str) -> Option<&Type> {
        self.type_aliases.get(name)
    }

    /// Look up an interface
    pub fn lookup_interface(&self, name: &str) -> Option<&Type> {
        self.interfaces.get(name)
    }

    /// Get an interface (alias for lookup_interface)
    pub fn get_interface(&self, name: &str) -> Option<&Type> {
        self.lookup_interface(name)
    }

    /// Check if a type name is defined
    pub fn is_type_defined(&self, name: &str) -> bool {
        self.lookup_type(name).is_some()
    }

    /// Resolve a type reference, detecting cycles
    pub fn resolve_type_reference(&self, name: &str) -> Result<Option<Type>, String> {
        // Check if we're already resolving this type (cycle detection)
        if self.resolving.borrow().contains(name) {
            return Err(format!("Recursive type alias '{}' detected", name));
        }

        // Mark as resolving
        self.resolving.borrow_mut().insert(name.to_string());

        // Look up the type
        let result = self.lookup_type(name).cloned();

        // Unmark
        self.resolving.borrow_mut().remove(name);

        Ok(result)
    }

    /// Get a generic type alias
    pub fn get_generic_type_alias(&self, name: &str) -> Option<&GenericTypeAlias> {
        self.generic_type_aliases.get(name)
    }

    /// Check if a name is a built-in utility type
    pub fn is_utility_type(name: &str) -> bool {
        matches!(
            name,
            "Partial" | "Required" | "Readonly" | "Record" | "Pick" | "Omit"
            | "Exclude" | "Extract" | "NonNilable" | "Nilable" | "ReturnType" | "Parameters"
        )
    }

    /// Resolve a utility type with type arguments
    pub fn resolve_utility_type(
        &self,
        name: &str,
        type_args: &[Type],
        span: Span,
    ) -> Result<Type, String> {
        use super::utility_types::apply_utility_type;
        apply_utility_type(name, type_args, span)
    }
}

impl Default for TypeEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtins_registered() {
        let env = TypeEnvironment::new();

        assert!(env.lookup_type("number").is_some());
        assert!(env.lookup_type("string").is_some());
        assert!(env.lookup_type("boolean").is_some());
        assert!(env.lookup_type("nil").is_some());
        assert!(env.lookup_type("unknown").is_some());
        assert!(env.lookup_type("never").is_some());
        assert!(env.lookup_type("void").is_some());
    }

    #[test]
    fn test_register_type_alias() {
        let mut env = TypeEnvironment::new();

        let typ = Type::new(
            TypeKind::Primitive(PrimitiveType::Number),
            Span::new(0, 0, 0, 0),
        );

        env.register_type_alias("MyNumber".to_string(), typ).unwrap();

        assert!(env.lookup_type("MyNumber").is_some());
        assert!(env.lookup_type_alias("MyNumber").is_some());
    }

    #[test]
    fn test_register_interface() {
        let mut env = TypeEnvironment::new();

        let typ = Type::new(
            TypeKind::Primitive(PrimitiveType::Table),
            Span::new(0, 0, 0, 0),
        );

        env.register_interface("MyInterface".to_string(), typ).unwrap();

        assert!(env.lookup_type("MyInterface").is_some());
        assert!(env.lookup_interface("MyInterface").is_some());
    }

    #[test]
    fn test_duplicate_type_alias() {
        let mut env = TypeEnvironment::new();

        let typ = Type::new(
            TypeKind::Primitive(PrimitiveType::Number),
            Span::new(0, 0, 0, 0),
        );

        env.register_type_alias("Foo".to_string(), typ.clone()).unwrap();
        assert!(env.register_type_alias("Foo".to_string(), typ).is_err());
    }
}
