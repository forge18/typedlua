use crate::config::CompilerOptions;
use std::path::Path;

/// Compute Blake3 hash of file content
/// Blake3 is faster than SHA-256 while maintaining cryptographic security
pub fn hash_file(path: &Path) -> std::io::Result<String> {
    let content = std::fs::read(path)?;
    let hash = blake3::hash(&content);
    Ok(hash.to_hex().to_string())
}

/// Hash compiler configuration to detect config changes
/// Any change in compiler options should invalidate the cache
pub fn hash_config(config: &CompilerOptions) -> String {
    // Serialize config to JSON for stable hashing
    let json = serde_json::to_string(config).expect("Failed to serialize config");
    let hash = blake3::hash(json.as_bytes());
    hash.to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_hash_file_consistency() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"test content").unwrap();
        file.flush().unwrap();

        let hash1 = hash_file(file.path()).unwrap();
        let hash2 = hash_file(file.path()).unwrap();

        assert_eq!(hash1, hash2, "Hash should be consistent");
    }

    #[test]
    fn test_hash_file_different_content() {
        let mut file1 = NamedTempFile::new().unwrap();
        file1.write_all(b"content A").unwrap();
        file1.flush().unwrap();

        let mut file2 = NamedTempFile::new().unwrap();
        file2.write_all(b"content B").unwrap();
        file2.flush().unwrap();

        let hash1 = hash_file(file1.path()).unwrap();
        let hash2 = hash_file(file2.path()).unwrap();

        assert_ne!(
            hash1, hash2,
            "Different content should produce different hashes"
        );
    }

    #[test]
    fn test_hash_config_consistency() {
        let config = CompilerOptions::default();

        let hash1 = hash_config(&config);
        let hash2 = hash_config(&config);

        assert_eq!(hash1, hash2, "Config hash should be consistent");
    }
}
