use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("Cache manifest not found")]
    ManifestNotFound,

    #[error("Cache version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: u32, found: u32 },

    #[error("Config hash mismatch (cache invalidated)")]
    ConfigMismatch,

    #[error("Corrupted cache file: {path}")]
    CorruptedFile { path: PathBuf },

    #[error("Module not found in cache: {path}")]
    ModuleNotFound { path: PathBuf },
}

pub type Result<T> = std::result::Result<T, CacheError>;
