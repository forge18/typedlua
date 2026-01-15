use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{CacheError, Result};
use crate::ast::Program;
use crate::module_resolver::ModuleExports;
use crate::typechecker::SerializableSymbolTable;

/// Cached module data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedModule {
    /// Module identifier (canonical path)
    pub path: PathBuf,

    /// Serialized AST
    pub ast: Program,

    /// Module exports
    pub exports: ModuleExports,

    /// Symbol table (flattened for serialization)
    pub symbol_table: SerializableSymbolTable,
}

impl CachedModule {
    /// Create a new cached module
    pub fn new(
        path: PathBuf,
        ast: Program,
        exports: ModuleExports,
        symbol_table: SerializableSymbolTable,
    ) -> Self {
        Self {
            path,
            ast,
            exports,
            symbol_table,
        }
    }

    /// Serialize to binary format
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(CacheError::from)
    }

    /// Deserialize from binary format
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(CacheError::from)
    }

    /// Compute hash of cached module data (for integrity checking)
    pub fn compute_hash(&self) -> String {
        let bytes = self.to_bytes().unwrap_or_default();
        let hash = blake3::hash(&bytes);
        hash.to_hex().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;
    use crate::typechecker::SerializableSymbolTable;

    fn make_test_program() -> Program {
        Program::new(vec![], Span::new(0, 0, 0, 0))
    }

    #[test]
    fn test_cached_module_serialization() {
        let module = CachedModule::new(
            PathBuf::from("/test/module.tl"),
            make_test_program(),
            ModuleExports::new(),
            SerializableSymbolTable { symbols: vec![] },
        );

        let bytes = module.to_bytes().unwrap();
        let deserialized = CachedModule::from_bytes(&bytes).unwrap();

        assert_eq!(module.path, deserialized.path);
    }

    #[test]
    fn test_compute_hash_consistency() {
        let module = CachedModule::new(
            PathBuf::from("/test/module.tl"),
            make_test_program(),
            ModuleExports::new(),
            SerializableSymbolTable { symbols: vec![] },
        );

        let hash1 = module.compute_hash();
        let hash2 = module.compute_hash();

        assert_eq!(hash1, hash2);
    }
}
