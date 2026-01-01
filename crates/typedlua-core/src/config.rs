use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LuaVersion {
    #[serde(rename = "5.1")]
    Lua51,
    #[serde(rename = "5.2")]
    Lua52,
    #[serde(rename = "5.3")]
    Lua53,
    #[serde(rename = "5.4")]
    Lua54,
}

impl Default for LuaVersion {
    fn default() -> Self {
        LuaVersion::Lua54
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrictLevel {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "error")]
    Error,
}

impl Default for StrictLevel {
    fn default() -> Self {
        StrictLevel::Error
    }
}

/// Compiler options that control type checking and code generation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerOptions {
    /// Enable strict null checking (default: true)
    #[serde(default = "default_true")]
    pub strict_null_checks: bool,

    /// Naming convention enforcement (default: error)
    #[serde(default)]
    pub strict_naming: StrictLevel,

    /// Disallow implicit unknown types (default: false)
    #[serde(default)]
    pub no_implicit_unknown: bool,

    /// Disallow explicit unknown types (default: false)
    #[serde(default)]
    pub no_explicit_unknown: bool,

    /// Target Lua version (default: 5.4)
    #[serde(default)]
    pub target: LuaVersion,

    /// Enable OOP features (default: true)
    #[serde(default = "default_true")]
    pub enable_oop: bool,

    /// Enable functional programming features (default: true)
    #[serde(default = "default_true")]
    pub enable_fp: bool,

    /// Enable decorator syntax (default: true)
    #[serde(default = "default_true")]
    pub enable_decorators: bool,

    /// Allow importing non-typed Lua files (default: true)
    #[serde(default = "default_true")]
    pub allow_non_typed_lua: bool,

    /// Output directory for compiled files
    #[serde(default)]
    pub out_dir: Option<String>,

    /// Output file (bundle all into one file)
    #[serde(default)]
    pub out_file: Option<String>,

    /// Generate source maps (default: false)
    #[serde(default)]
    pub source_map: bool,

    /// Don't emit output files (type check only, default: false)
    #[serde(default)]
    pub no_emit: bool,

    /// Pretty-print diagnostics (default: true)
    #[serde(default = "default_true")]
    pub pretty: bool,
}

fn default_true() -> bool {
    true
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            strict_null_checks: true,
            strict_naming: StrictLevel::Error,
            no_implicit_unknown: false,
            no_explicit_unknown: false,
            target: LuaVersion::Lua54,
            enable_oop: true,
            enable_fp: true,
            enable_decorators: true,
            allow_non_typed_lua: true,
            out_dir: None,
            out_file: None,
            source_map: false,
            no_emit: false,
            pretty: true,
        }
    }
}

/// Main compiler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerConfig {
    /// Compiler options
    #[serde(default)]
    pub compiler_options: CompilerOptions,

    /// Files to include (glob patterns)
    #[serde(default)]
    pub include: Vec<String>,

    /// Files to exclude (glob patterns)
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
}

fn default_exclude() -> Vec<String> {
    vec!["**/node_modules/**".to_string(), "**/dist/**".to_string()]
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            compiler_options: CompilerOptions::default(),
            include: vec!["**/*.tl".to_string()],
            exclude: default_exclude(),
        }
    }
}

impl CompilerConfig {
    /// Load configuration from a JSON file
    pub fn from_file(path: &Path) -> Result<Self, crate::errors::CompilationError> {
        let content = std::fs::read_to_string(path)?;
        let config: CompilerConfig = serde_json::from_str(&content)
            .map_err(|e| crate::errors::CompilationError::ConfigError(e.to_string()))?;
        Ok(config)
    }

    /// Create a default configuration and write it to a file
    pub fn init_file(path: &Path) -> Result<(), crate::errors::CompilationError> {
        let config = CompilerConfig::default();
        let json = serde_json::to_string_pretty(&config)
            .map_err(|e| crate::errors::CompilationError::ConfigError(e.to_string()))?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Merge this configuration with CLI overrides
    pub fn merge_with_cli(&mut self, cli_config: CompilerOptions) {
        // This would merge CLI flags into the file config
        // For now, we'll keep it simple
        self.compiler_options = cli_config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CompilerConfig::default();
        assert!(config.compiler_options.strict_null_checks);
        assert!(config.compiler_options.enable_oop);
        assert_eq!(config.compiler_options.target, LuaVersion::Lua54);
    }

    #[test]
    fn test_serialize_config() {
        let config = CompilerConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("compilerOptions"));
    }

    #[test]
    fn test_deserialize_config() {
        let json = r#"{
            "compilerOptions": {
                "target": "5.3",
                "enableOop": false
            }
        }"#;
        let config: CompilerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.compiler_options.target, LuaVersion::Lua53);
        assert!(!config.compiler_options.enable_oop);
    }
}
