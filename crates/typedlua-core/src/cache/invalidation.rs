use rustc_hash::FxHashSet;
use std::path::PathBuf;

use super::CacheManifest;

/// Engine for computing which modules need to be recompiled
pub struct InvalidationEngine<'a> {
    manifest: &'a CacheManifest,
}

impl<'a> InvalidationEngine<'a> {
    /// Create a new invalidation engine
    pub fn new(manifest: &'a CacheManifest) -> Self {
        Self { manifest }
    }

    /// Compute all modules that are stale (need recompilation)
    ///
    /// This includes:
    /// 1. Directly changed files
    /// 2. All modules that transitively depend on changed files
    ///
    /// Algorithm: BFS/DFS from changed files through reverse dependency graph
    pub fn compute_stale_modules(&self, changed_files: &[PathBuf]) -> FxHashSet<PathBuf> {
        let mut stale = FxHashSet::default();

        // Add directly changed files
        for file in changed_files {
            stale.insert(file.clone());
        }

        // Build reverse dependency map: dependency -> list of modules that depend on it
        let mut reverse_deps: rustc_hash::FxHashMap<PathBuf, Vec<PathBuf>> =
            rustc_hash::FxHashMap::default();

        for (module_path, deps) in &self.manifest.dependencies {
            for dep in deps {
                reverse_deps
                    .entry(dep.clone())
                    .or_default()
                    .push(module_path.clone());
            }
        }

        // Transitively invalidate dependents
        let mut to_process: Vec<_> = changed_files.to_vec();

        while let Some(changed) = to_process.pop() {
            // Find all modules that depend on this changed module
            if let Some(dependents) = reverse_deps.get(&changed) {
                for dependent in dependents {
                    if !stale.contains(dependent) {
                        stale.insert(dependent.clone());
                        to_process.push(dependent.clone());
                    }
                }
            }
        }

        stale
    }

    /// Check if a specific module is affected by changes
    pub fn is_module_stale(&self, module_path: &PathBuf, changed_files: &[PathBuf]) -> bool {
        let stale = self.compute_stale_modules(changed_files);
        stale.contains(module_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{CacheEntry, CacheManifest};

    #[test]
    fn test_simple_invalidation() {
        let mut manifest = CacheManifest::new("test".to_string());

        // Create A.tl (no dependencies)
        let entry_a = CacheEntry::new(
            PathBuf::from("/test/A.tl"),
            "hash_a".to_string(),
            "cache_a".to_string(),
            vec![],
        );
        manifest.insert_entry(PathBuf::from("/test/A.tl"), entry_a);

        let engine = InvalidationEngine::new(&manifest);
        let changed = vec![PathBuf::from("/test/A.tl")];
        let stale = engine.compute_stale_modules(&changed);

        assert_eq!(stale.len(), 1);
        assert!(stale.contains(&PathBuf::from("/test/A.tl")));
    }

    #[test]
    fn test_transitive_invalidation() {
        let mut manifest = CacheManifest::new("test".to_string());

        // Create C.tl (no dependencies)
        let entry_c = CacheEntry::new(
            PathBuf::from("/test/C.tl"),
            "hash_c".to_string(),
            "cache_c".to_string(),
            vec![],
        );
        manifest.insert_entry(PathBuf::from("/test/C.tl"), entry_c);

        // Create B.tl (depends on C)
        let entry_b = CacheEntry::new(
            PathBuf::from("/test/B.tl"),
            "hash_b".to_string(),
            "cache_b".to_string(),
            vec![PathBuf::from("/test/C.tl")],
        );
        manifest.insert_entry(PathBuf::from("/test/B.tl"), entry_b);

        // Create A.tl (depends on B)
        let entry_a = CacheEntry::new(
            PathBuf::from("/test/A.tl"),
            "hash_a".to_string(),
            "cache_a".to_string(),
            vec![PathBuf::from("/test/B.tl")],
        );
        manifest.insert_entry(PathBuf::from("/test/A.tl"), entry_a);

        let engine = InvalidationEngine::new(&manifest);

        // Change C.tl -> should invalidate B and A as well
        let changed = vec![PathBuf::from("/test/C.tl")];
        let stale = engine.compute_stale_modules(&changed);

        assert_eq!(stale.len(), 3, "All three modules should be stale");
        assert!(stale.contains(&PathBuf::from("/test/C.tl")));
        assert!(stale.contains(&PathBuf::from("/test/B.tl")));
        assert!(stale.contains(&PathBuf::from("/test/A.tl")));
    }

    #[test]
    fn test_partial_invalidation() {
        let mut manifest = CacheManifest::new("test".to_string());

        // Create independent modules
        let entry_a = CacheEntry::new(
            PathBuf::from("/test/A.tl"),
            "hash_a".to_string(),
            "cache_a".to_string(),
            vec![],
        );
        manifest.insert_entry(PathBuf::from("/test/A.tl"), entry_a);

        let entry_b = CacheEntry::new(
            PathBuf::from("/test/B.tl"),
            "hash_b".to_string(),
            "cache_b".to_string(),
            vec![],
        );
        manifest.insert_entry(PathBuf::from("/test/B.tl"), entry_b);

        let engine = InvalidationEngine::new(&manifest);

        // Change only A -> B should remain valid
        let changed = vec![PathBuf::from("/test/A.tl")];
        let stale = engine.compute_stale_modules(&changed);

        assert_eq!(stale.len(), 1);
        assert!(stale.contains(&PathBuf::from("/test/A.tl")));
        assert!(!stale.contains(&PathBuf::from("/test/B.tl")));
    }
}
