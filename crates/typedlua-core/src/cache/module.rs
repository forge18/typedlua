use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{CacheError, Result};

/// Cached module data
///
/// Note: During the arena allocation migration (Phase 4), the AST, exports,
/// and symbol table types lost their Deserialize derives because they contain
/// arena-allocated references. The cache module uses a simplified serializable
/// representation until a proper owned-type serialization strategy is implemented.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedModule {
    /// Module identifier (canonical path)
    pub path: PathBuf,

    /// Hash of the source file for cache invalidation
    pub source_hash: String,

    /// Interned string table — needed to reconstruct a StringInterner
    /// so that StringId values resolve correctly.
    pub interner_strings: Vec<String>,

    /// Serialized export names (simplified representation)
    pub export_names: Vec<String>,

    /// Whether a default export exists
    pub has_default_export: bool,
}

impl CachedModule {
    /// Create a new cached module
    pub fn new(
        path: PathBuf,
        source_hash: String,
        interner_strings: Vec<String>,
        export_names: Vec<String>,
        has_default_export: bool,
    ) -> Self {
        Self {
            path,
            source_hash,
            interner_strings,
            export_names,
            has_default_export,
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

    fn make_test_module() -> CachedModule {
        CachedModule::new(
            PathBuf::from("/test/module.tl"),
            "abc123".to_string(),
            vec![],
            vec![],
            false,
        )
    }

    #[test]
    fn test_cached_module_serialization() {
        let module = make_test_module();

        let bytes = module.to_bytes().unwrap();
        let deserialized = CachedModule::from_bytes(&bytes).unwrap();

        assert_eq!(module.path, deserialized.path);
        assert_eq!(module.source_hash, deserialized.source_hash);
    }

    #[test]
    fn test_compute_hash_consistency() {
        let module = make_test_module();

        let hash1 = module.compute_hash();
        let hash2 = module.compute_hash();

        assert_eq!(hash1, hash2);
    }
}
