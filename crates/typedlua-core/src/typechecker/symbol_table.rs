use crate::ast::types::Type;
use crate::span::Span;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// Kind of symbol (variable, function, class, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Variable,
    Const,
    Function,
    Class,
    Interface,
    TypeAlias,
    Enum,
    Parameter,
}

/// A symbol in the symbol table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub typ: Type,
    pub span: Span,
    pub is_exported: bool,
    pub references: Vec<Span>,
}

impl Symbol {
    pub fn new(name: String, kind: SymbolKind, typ: Type, span: Span) -> Self {
        Self {
            name,
            kind,
            typ,
            span,
            is_exported: false,
            references: Vec::new(),
        }
    }

    /// Add a reference to this symbol
    pub fn add_reference(&mut self, span: Span) {
        self.references.push(span);
    }
}

/// A scope containing symbols
#[derive(Debug, Clone)]
pub struct Scope {
    symbols: FxHashMap<String, Symbol>,
    parent: Option<Box<Scope>>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            symbols: FxHashMap::default(),
            parent: None,
        }
    }

    pub fn with_parent(parent: Scope) -> Self {
        Self {
            symbols: FxHashMap::default(),
            parent: Some(Box::new(parent)),
        }
    }

    /// Declare a symbol in this scope
    pub fn declare(&mut self, symbol: Symbol) -> Result<(), String> {
        if self.symbols.contains_key(&symbol.name) {
            return Err(format!(
                "Symbol '{}' already declared in this scope",
                symbol.name
            ));
        }
        self.symbols.insert(symbol.name.clone(), symbol);
        Ok(())
    }

    /// Look up a symbol in this scope or parent scopes
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        if let Some(symbol) = self.symbols.get(name) {
            return Some(symbol);
        }

        if let Some(parent) = &self.parent {
            return parent.lookup(name);
        }

        None
    }

    /// Look up a symbol only in this scope (not parent scopes)
    pub fn lookup_local(&self, name: &str) -> Option<&Symbol> {
        self.symbols.get(name)
    }

    /// Get all symbols in this scope
    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.values()
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

/// Symbol table managing scopes
#[derive(Debug)]
pub struct SymbolTable {
    current_scope: Scope,
    scope_stack: Vec<Scope>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            current_scope: Scope::new(),
            scope_stack: Vec::new(),
        }
    }

    /// Enter a new scope
    pub fn enter_scope(&mut self) {
        let parent = std::mem::take(&mut self.current_scope);
        self.scope_stack.push(parent);

        if let Some(parent) = self.scope_stack.last() {
            self.current_scope.parent = Some(Box::new(Scope {
                symbols: parent.symbols.clone(),
                parent: parent.parent.clone(),
            }));
        }
    }

    /// Exit current scope
    pub fn exit_scope(&mut self) {
        if let Some(parent) = self.scope_stack.pop() {
            self.current_scope = parent;
        }
    }

    /// Declare a symbol in the current scope
    pub fn declare(&mut self, symbol: Symbol) -> Result<(), String> {
        self.current_scope.declare(symbol)
    }

    /// Look up a symbol in current or parent scopes
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        self.current_scope.lookup(name)
    }

    /// Look up a symbol only in the current scope
    pub fn lookup_local(&self, name: &str) -> Option<&Symbol> {
        self.current_scope.lookup_local(name)
    }

    /// Add a reference to a symbol
    /// Returns true if the symbol was found and reference was added
    pub fn add_reference(&mut self, name: &str, span: Span) -> bool {
        // Try to find and update the symbol in current scope
        if self.current_scope.symbols.contains_key(name) {
            if let Some(symbol) = self.current_scope.symbols.get_mut(name) {
                symbol.add_reference(span);
                return true;
            }
        }

        // Try parent scopes
        let mut current_parent = &mut self.current_scope.parent;
        while let Some(ref mut parent_box) = current_parent {
            if parent_box.symbols.contains_key(name) {
                if let Some(symbol) = parent_box.symbols.get_mut(name) {
                    symbol.add_reference(span);
                    return true;
                }
            }
            current_parent = &mut parent_box.parent;
        }

        false
    }

    /// Get the current scope
    pub fn current_scope(&self) -> &Scope {
        &self.current_scope
    }

    /// Get all symbols visible from the current scope (current + all parents)
    pub fn all_visible_symbols(&self) -> FxHashMap<String, &Symbol> {
        let mut result = FxHashMap::default();
        self.collect_symbols_recursive(&self.current_scope, &mut result);
        result
    }

    fn collect_symbols_recursive<'a>(
        &'a self,
        scope: &'a Scope,
        result: &mut FxHashMap<String, &'a Symbol>,
    ) {
        // Add parent symbols first (so they can be shadowed)
        if let Some(parent) = &scope.parent {
            self.collect_symbols_recursive(parent, result);
        }

        // Add current scope symbols (shadowing parent symbols with same name)
        for (name, symbol) in &scope.symbols {
            result.insert(name.clone(), symbol);
        }
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializable representation of a symbol with scope depth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub typ: Type,
    pub span: Span,
    pub is_exported: bool,
    pub references: Vec<Span>,
    pub scope_depth: usize,
}

/// Serializable representation of SymbolTable (flattened scopes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableSymbolTable {
    pub symbols: Vec<SerializableSymbol>,
}

impl SymbolTable {
    /// Convert to serializable format by flattening scope hierarchy
    pub fn to_serializable(&self) -> SerializableSymbolTable {
        let mut symbols = Vec::new();
        self.flatten_scope(&self.current_scope, 0, &mut symbols);
        SerializableSymbolTable { symbols }
    }

    fn flatten_scope(&self, scope: &Scope, depth: usize, out: &mut Vec<SerializableSymbol>) {
        // Add symbols from parent scope first (if exists)
        if let Some(parent) = &scope.parent {
            self.flatten_scope(parent, depth + 1, out);
        }

        // Add symbols from current scope
        for symbol in scope.symbols.values() {
            out.push(SerializableSymbol {
                name: symbol.name.clone(),
                kind: symbol.kind,
                typ: symbol.typ.clone(),
                span: symbol.span,
                is_exported: symbol.is_exported,
                references: symbol.references.clone(),
                scope_depth: depth,
            });
        }
    }

    /// Reconstruct from serializable format
    pub fn from_serializable(data: SerializableSymbolTable) -> Self {
        // Group symbols by scope depth
        let mut symbols_by_depth: FxHashMap<usize, Vec<SerializableSymbol>> = FxHashMap::default();
        for symbol in data.symbols {
            symbols_by_depth
                .entry(symbol.scope_depth)
                .or_default()
                .push(symbol);
        }

        // Find max depth to reconstruct from deepest to shallowest
        let max_depth = symbols_by_depth.keys().max().copied().unwrap_or(0);

        // Start with deepest scope and work up to root
        let mut scope_map: FxHashMap<usize, Scope> = FxHashMap::default();

        for depth in (0..=max_depth).rev() {
            let mut scope = Scope::new();

            // Set parent if this is not the deepest scope
            if depth < max_depth {
                if let Some(child_scope) = scope_map.get(&(depth + 1)) {
                    scope.parent = Some(Box::new(child_scope.clone()));
                }
            }

            // Add symbols at this depth
            if let Some(symbols) = symbols_by_depth.get(&depth) {
                for serializable in symbols {
                    let symbol = Symbol {
                        name: serializable.name.clone(),
                        kind: serializable.kind,
                        typ: serializable.typ.clone(),
                        span: serializable.span,
                        is_exported: serializable.is_exported,
                        references: serializable.references.clone(),
                    };
                    scope.symbols.insert(symbol.name.clone(), symbol);
                }
            }

            scope_map.insert(depth, scope);
        }

        // The root scope is at depth 0
        let current_scope = scope_map.remove(&0).unwrap_or_default();

        SymbolTable {
            current_scope,
            scope_stack: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::types::{PrimitiveType, TypeKind};

    fn make_test_type() -> Type {
        Type::new(
            TypeKind::Primitive(PrimitiveType::Number),
            Span::new(0, 0, 0, 0),
        )
    }

    #[test]
    fn test_scope_declare_and_lookup() {
        let mut scope = Scope::new();
        let symbol = Symbol::new(
            "x".to_string(),
            SymbolKind::Variable,
            make_test_type(),
            Span::new(0, 0, 0, 0),
        );

        scope.declare(symbol).unwrap();
        assert!(scope.lookup("x").is_some());
        assert!(scope.lookup("y").is_none());
    }

    #[test]
    fn test_scope_duplicate_declaration() {
        let mut scope = Scope::new();
        let symbol1 = Symbol::new(
            "x".to_string(),
            SymbolKind::Variable,
            make_test_type(),
            Span::new(0, 0, 0, 0),
        );
        let symbol2 = Symbol::new(
            "x".to_string(),
            SymbolKind::Variable,
            make_test_type(),
            Span::new(0, 0, 0, 0),
        );

        scope.declare(symbol1).unwrap();
        assert!(scope.declare(symbol2).is_err());
    }

    #[test]
    fn test_symbol_table_scopes() {
        let mut table = SymbolTable::new();

        // Declare in global scope
        let symbol1 = Symbol::new(
            "x".to_string(),
            SymbolKind::Variable,
            make_test_type(),
            Span::new(0, 0, 0, 0),
        );
        table.declare(symbol1).unwrap();

        // Enter new scope
        table.enter_scope();

        // Should still see x from parent
        assert!(table.lookup("x").is_some());

        // Declare y in inner scope
        let symbol2 = Symbol::new(
            "y".to_string(),
            SymbolKind::Variable,
            make_test_type(),
            Span::new(0, 0, 0, 0),
        );
        table.declare(symbol2).unwrap();

        assert!(table.lookup("y").is_some());

        // Exit scope
        table.exit_scope();

        // y should no longer be visible
        assert!(table.lookup("y").is_none());
        // x should still be visible
        assert!(table.lookup("x").is_some());
    }

    #[test]
    fn test_symbol_table_shadowing() {
        let mut table = SymbolTable::new();

        // Declare x in global scope
        let symbol1 = Symbol::new(
            "x".to_string(),
            SymbolKind::Variable,
            make_test_type(),
            Span::new(0, 0, 0, 0),
        );
        table.declare(symbol1).unwrap();

        // Enter new scope and shadow x
        table.enter_scope();
        let symbol2 = Symbol::new(
            "x".to_string(),
            SymbolKind::Const,
            make_test_type(),
            Span::new(1, 1, 1, 1),
        );
        table.declare(symbol2).unwrap();

        // Should see the inner x
        let x = table.lookup("x").unwrap();
        assert_eq!(x.kind, SymbolKind::Const);

        // Exit scope
        table.exit_scope();

        // Should see the outer x again
        let x = table.lookup("x").unwrap();
        assert_eq!(x.kind, SymbolKind::Variable);
    }
}
