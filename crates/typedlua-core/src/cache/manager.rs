use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::config::CompilerOptions;

use super::{
    hash_config, hash_file, CacheEntry, CacheError, CacheManifest, CachedModule,
    InvalidationEngine, Result, CACHE_DIR_NAME, MANIFEST_FILE_NAME, MODULES_DIR_NAME,
};

/// Main interface for cache operations
pub struct CacheManager {
    /// Base directory for the cache
    cache_dir: PathBuf,

    /// Modules subdirectory
    modules_dir: PathBuf,

    /// Path to manifest file
    manifest_path: PathBuf,

    /// Loaded cache manifest
    manifest: Option<CacheManifest>,

    /// Hash of current compiler configuration
    config_hash: String,
}

impl CacheManager {
    /// Create a new cache manager
    ///
    /// # Arguments
    /// * `base_dir` - Project root directory (cache will be at base_dir/.typed-lua-cache)
    /// * `config` - Compiler options (used to detect config changes)
    pub fn new(base_dir: &Path, config: &CompilerOptions) -> Result<Self> {
        let cache_dir = base_dir.join(CACHE_DIR_NAME);
        let modules_dir = cache_dir.join(MODULES_DIR_NAME);
        let manifest_path = cache_dir.join(MANIFEST_FILE_NAME);
        let config_hash = hash_config(config);

        Ok(Self {
            cache_dir,
            modules_dir,
            manifest_path,
            manifest: None,
            config_hash,
        })
    }

    /// Initialize cache directories
    fn ensure_cache_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.cache_dir)?;
        std::fs::create_dir_all(&self.modules_dir)?;
        Ok(())
    }

    /// Load manifest from disk
    ///
    /// Returns Ok(()) if manifest loaded successfully or created new
    /// Returns Err if manifest is corrupted (caller should handle by clearing cache)
    pub fn load_manifest(&mut self) -> Result<()> {
        self.ensure_cache_dirs()?;

        if !self.manifest_path.exists() {
            info!("No cache manifest found, creating new");
            self.manifest = Some(CacheManifest::new(self.config_hash.clone()));
            return Ok(());
        }

        match std::fs::read(&self.manifest_path) {
            Ok(bytes) => match CacheManifest::from_bytes(&bytes) {
                Ok(manifest) => {
                    if !manifest.is_version_compatible() {
                        warn!(
                            "Cache version mismatch: expected {}, found {}",
                            super::CACHE_VERSION,
                            manifest.version
                        );
                        return Err(CacheError::VersionMismatch {
                            expected: super::CACHE_VERSION,
                            found: manifest.version,
                        });
                    }

                    info!(
                        "Loaded cache manifest with {} modules",
                        manifest.modules.len()
                    );
                    self.manifest = Some(manifest);
                    Ok(())
                }
                Err(e) => {
                    warn!("Corrupted cache manifest: {:?}", e);
                    Err(e)
                }
            },
            Err(e) => {
                warn!("Failed to read cache manifest: {:?}", e);
                Err(CacheError::from(e))
            }
        }
    }

    /// Check if cache is valid (config hasn't changed)
    pub fn is_valid(&self) -> bool {
        self.manifest
            .as_ref()
            .map(|m| m.config_hash == self.config_hash)
            .unwrap_or(false)
    }

    /// Detect which files have changed
    ///
    /// Compares current file hashes with cached hashes
    pub fn detect_changes(&self, files: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let manifest = self.manifest.as_ref().ok_or(CacheError::ManifestNotFound)?;

        let mut changed = Vec::new();

        for file in files {
            let canonical = file.canonicalize().unwrap_or_else(|_| file.clone());

            // File is changed if:
            // 1. Not in cache, or
            // 2. Hash doesn't match cached hash
            let is_changed = match manifest.get_entry(&canonical) {
                Some(entry) => {
                    let current_hash = hash_file(&canonical)?;
                    current_hash != entry.source_hash
                }
                None => true, // Not in cache = changed
            };

            if is_changed {
                changed.push(canonical);
            }
        }

        Ok(changed)
    }

    /// Compute all modules that need recompilation (transitive invalidation)
    pub fn compute_stale_modules(&self, changed_files: &[PathBuf]) -> FxHashSet<PathBuf> {
        match &self.manifest {
            Some(manifest) => {
                let engine = InvalidationEngine::new(manifest);
                engine.compute_stale_modules(changed_files)
            }
            None => {
                // No manifest = all files are stale
                changed_files.iter().cloned().collect()
            }
        }
    }

    /// Get a cached module
    pub fn get_cached_module(&self, path: &Path) -> Result<Option<CachedModule>> {
        let manifest = self.manifest.as_ref().ok_or(CacheError::ManifestNotFound)?;

        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        let entry = match manifest.get_entry(&canonical) {
            Some(e) => e,
            None => return Ok(None),
        };

        // Compute module cache file path from cache hash
        let module_file = self.modules_dir.join(format!("{}.bin", entry.cache_hash));

        if !module_file.exists() {
            warn!("Cache file missing for {:?}", canonical);
            return Ok(None);
        }

        match std::fs::read(&module_file) {
            Ok(bytes) => match CachedModule::from_bytes(&bytes) {
                Ok(module) => Ok(Some(module)),
                Err(e) => {
                    warn!("Corrupted cache file for {:?}: {:?}", canonical, e);
                    Ok(None) // Treat as cache miss
                }
            },
            Err(e) => {
                warn!("Failed to read cache file for {:?}: {:?}", canonical, e);
                Ok(None) // Treat as cache miss
            }
        }
    }

    /// Save a module to cache
    pub fn save_module(
        &mut self,
        path: &Path,
        module: &CachedModule,
        dependencies: Vec<PathBuf>,
    ) -> Result<()> {
        self.ensure_cache_dirs()?;

        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let source_hash = hash_file(&canonical)?;
        let cache_hash = module.compute_hash();

        // Write module to disk
        let module_file = self.modules_dir.join(format!("{}.bin", cache_hash));
        let module_bytes = module.to_bytes()?;
        std::fs::write(&module_file, &module_bytes)?;

        // Update manifest
        let entry = CacheEntry::new(canonical.clone(), source_hash, cache_hash, dependencies);

        let manifest = self.manifest.as_mut().ok_or(CacheError::ManifestNotFound)?;

        manifest.insert_entry(canonical, entry);

        Ok(())
    }

    /// Save manifest to disk
    pub fn save_manifest(&self) -> Result<()> {
        let manifest = self.manifest.as_ref().ok_or(CacheError::ManifestNotFound)?;

        let bytes = manifest.to_bytes()?;
        std::fs::write(&self.manifest_path, &bytes)?;

        info!(
            "Saved cache manifest with {} modules",
            manifest.modules.len()
        );
        Ok(())
    }

    /// Clear the entire cache
    pub fn clear(&mut self) -> Result<()> {
        if self.cache_dir.exists() {
            std::fs::remove_dir_all(&self.cache_dir)?;
        }

        self.manifest = Some(CacheManifest::new(self.config_hash.clone()));
        self.ensure_cache_dirs()?;

        info!("Cache cleared");
        Ok(())
    }

    /// Clean up cache entries for files that no longer exist
    pub fn cleanup_stale_entries(&mut self, current_files: &[PathBuf]) -> Result<()> {
        if let Some(manifest) = &mut self.manifest {
            let canonical_files: Vec<_> = current_files
                .iter()
                .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
                .collect();

            manifest.cleanup_stale_entries(&canonical_files);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = CompilerOptions::default();

        let manager = CacheManager::new(temp_dir.path(), &config).unwrap();

        assert!(manager.cache_dir.ends_with(CACHE_DIR_NAME));
        assert!(manager.manifest.is_none());
    }

    #[test]
    fn test_load_manifest_creates_new() {
        let temp_dir = TempDir::new().unwrap();
        let config = CompilerOptions::default();

        let mut manager = CacheManager::new(temp_dir.path(), &config).unwrap();
        manager.load_manifest().unwrap();

        assert!(manager.manifest.is_some());
        assert!(manager.cache_dir.exists());
    }

    #[test]
    fn test_clear_cache() {
        let temp_dir = TempDir::new().unwrap();
        let config = CompilerOptions::default();

        let mut manager = CacheManager::new(temp_dir.path(), &config).unwrap();
        manager.load_manifest().unwrap();
        manager.clear().unwrap();

        assert!(manager.manifest.is_some());
        assert!(manager.cache_dir.exists());
    }
}
