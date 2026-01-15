use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{CacheError, Result, CACHE_VERSION};

/// Cache manifest containing metadata and dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheManifest {
    /// Schema version for cache format
    pub version: u32,

    /// Hash of compiler configuration (invalidate on config change)
    pub config_hash: String,

    /// Cached modules: canonical path -> cache entry
    pub modules: FxHashMap<PathBuf, CacheEntry>,

    /// Dependency graph: module path -> list of dependency paths
    pub dependencies: FxHashMap<PathBuf, Vec<PathBuf>>,
}

/// Entry for a single cached module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Canonical path to source file
    pub source_path: PathBuf,

    /// Blake3 hash of source content
    pub source_hash: String,

    /// Hash of the cached binary file (for integrity)
    pub cache_hash: String,

    /// Timestamp when cached (for diagnostics)
    pub cached_at: u64,

    /// List of direct dependencies (for invalidation)
    pub dependencies: Vec<PathBuf>,
}

impl CacheManifest {
    /// Create a new empty manifest with the given config hash
    pub fn new(config_hash: String) -> Self {
        Self {
            version: CACHE_VERSION,
            config_hash,
            modules: FxHashMap::default(),
            dependencies: FxHashMap::default(),
        }
    }

    /// Check if manifest version matches current cache version
    pub fn is_version_compatible(&self) -> bool {
        self.version == CACHE_VERSION
    }

    /// Serialize manifest to binary format
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(CacheError::from)
    }

    /// Deserialize manifest from binary format
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(CacheError::from)
    }

    /// Add or update a cache entry
    pub fn insert_entry(&mut self, path: PathBuf, entry: CacheEntry) {
        // Update dependencies graph
        self.dependencies
            .insert(path.clone(), entry.dependencies.clone());

        // Insert the cache entry
        self.modules.insert(path, entry);
    }

    /// Remove a cache entry and its dependency information
    pub fn remove_entry(&mut self, path: &PathBuf) {
        self.modules.remove(path);
        self.dependencies.remove(path);
    }

    /// Get a cache entry for a module
    pub fn get_entry(&self, path: &PathBuf) -> Option<&CacheEntry> {
        self.modules.get(path)
    }

    /// Clean up entries for files that no longer exist
    pub fn cleanup_stale_entries(&mut self, current_files: &[PathBuf]) {
        let current_set: std::collections::HashSet<_> = current_files.iter().collect();

        self.modules.retain(|path, _| current_set.contains(path));
        self.dependencies
            .retain(|path, _| current_set.contains(path));
    }
}

impl CacheEntry {
    /// Create a new cache entry
    pub fn new(
        source_path: PathBuf,
        source_hash: String,
        cache_hash: String,
        dependencies: Vec<PathBuf>,
    ) -> Self {
        Self {
            source_path,
            source_hash,
            cache_hash,
            cached_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            dependencies,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_serialization_roundtrip() {
        let mut manifest = CacheManifest::new("test_hash".to_string());

        let entry = CacheEntry::new(
            PathBuf::from("/test/file.tl"),
            "source_hash".to_string(),
            "cache_hash".to_string(),
            vec![PathBuf::from("/test/dep.tl")],
        );

        manifest.insert_entry(PathBuf::from("/test/file.tl"), entry);

        let bytes = manifest.to_bytes().unwrap();
        let deserialized = CacheManifest::from_bytes(&bytes).unwrap();

        assert_eq!(manifest.version, deserialized.version);
        assert_eq!(manifest.config_hash, deserialized.config_hash);
        assert_eq!(manifest.modules.len(), deserialized.modules.len());
    }

    #[test]
    fn test_manifest_version_compatibility() {
        let manifest = CacheManifest::new("test".to_string());
        assert!(manifest.is_version_compatible());
    }

    #[test]
    fn test_cleanup_stale_entries() {
        let mut manifest = CacheManifest::new("test".to_string());

        let entry1 = CacheEntry::new(
            PathBuf::from("/test/file1.tl"),
            "hash1".to_string(),
            "cache1".to_string(),
            vec![],
        );

        let entry2 = CacheEntry::new(
            PathBuf::from("/test/file2.tl"),
            "hash2".to_string(),
            "cache2".to_string(),
            vec![],
        );

        manifest.insert_entry(PathBuf::from("/test/file1.tl"), entry1);
        manifest.insert_entry(PathBuf::from("/test/file2.tl"), entry2);

        // Only keep file1
        manifest.cleanup_stale_entries(&[PathBuf::from("/test/file1.tl")]);

        assert_eq!(manifest.modules.len(), 1);
        assert!(manifest
            .modules
            .contains_key(&PathBuf::from("/test/file1.tl")));
        assert!(!manifest
            .modules
            .contains_key(&PathBuf::from("/test/file2.tl")));
    }
}
