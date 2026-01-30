use clap::Parser;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

/// TypedLua - A TypeScript-inspired type system for Lua
#[derive(Parser, Debug, Clone)]
#[command(name = "typedlua")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Input files to compile
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,

    /// Path to tlconfig.yaml configuration file
    #[arg(short, long, value_name = "FILE")]
    project: Option<PathBuf>,

    /// Output directory for compiled Lua files
    #[arg(long, value_name = "DIR")]
    out_dir: Option<PathBuf>,

    /// Output file (concatenates all output into a single file)
    #[arg(long, value_name = "FILE")]
    out_file: Option<PathBuf>,

    /// Target Lua version (5.1, 5.2, 5.3, 5.4)
    #[arg(long, value_name = "VERSION", default_value = "5.4")]
    target: String,

    /// Generate source maps
    #[arg(long)]
    source_map: bool,

    /// Inline source map in output file
    #[arg(long)]
    inline_source_map: bool,

    /// Do not emit output files
    #[arg(long)]
    no_emit: bool,

    /// Watch input files for changes
    #[arg(short, long)]
    watch: bool,

    /// Initialize a new TypedLua project
    #[arg(long)]
    init: bool,

    /// Pretty print diagnostics
    #[arg(long, default_value_t = true)]
    pretty: bool,

    /// Show diagnostic codes
    #[arg(long)]
    diagnostics: bool,

    /// Disable incremental compilation cache
    #[arg(long)]
    no_cache: bool,

    /// Disable strict null checks
    #[arg(long)]
    no_strict_null_checks: bool,

    /// Strict naming convention enforcement (error, warning, off)
    #[arg(long, value_name = "LEVEL")]
    strict_naming: Option<String>,

    /// Disallow implicit unknown types
    #[arg(long)]
    no_implicit_unknown: bool,

    /// Enable decorator syntax (default: true)
    #[arg(long)]
    enable_decorators: bool,

    /// Module code generation mode (require, bundle)
    #[arg(long, value_name = "MODE")]
    module_mode: Option<String>,

    /// Module search paths (comma-separated)
    #[arg(long, value_name = "PATHS")]
    module_paths: Option<String>,

    /// Enforce namespace declarations match file paths
    #[arg(long)]
    enforce_namespace_path: bool,

    /// Copy plain .lua files to output directory
    #[arg(long)]
    copy_lua_to_output: bool,
}

fn main() -> anyhow::Result<()> {
    // Initialize tracing subscriber
    // Set RUST_LOG=debug for detailed logs, RUST_LOG=info for normal output
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let cli = Cli::parse();

    // Handle --init flag
    if cli.init {
        init_project()?;
        return Ok(());
    }

    // Load configuration
    let (config, files) = load_config_and_files(&cli)?;

    // Validate that we have input files
    if files.is_empty() {
        eprintln!("Error: No input files specified. Use --help for usage information.");
        std::process::exit(1);
    }

    // Parse target Lua version from config
    let target = match config.compiler_options.target {
        typedlua_core::config::LuaVersion::Lua51 => typedlua_core::codegen::LuaTarget::Lua51,
        typedlua_core::config::LuaVersion::Lua52 => typedlua_core::codegen::LuaTarget::Lua52,
        typedlua_core::config::LuaVersion::Lua53 => typedlua_core::codegen::LuaTarget::Lua53,
        typedlua_core::config::LuaVersion::Lua54 => typedlua_core::codegen::LuaTarget::Lua54,
    };

    info!(
        "TypedLua CLI - Compiling with target Lua {:?}",
        config.compiler_options.target
    );
    info!("Input files: {} file(s)", files.len());
    if let Some(ref out_dir) = config.compiler_options.out_dir {
        info!("Output directory: {}", out_dir);
    }
    debug!("Source maps: {}", config.compiler_options.source_map);
    debug!("Watch mode: {}", cli.watch);

    // Create a modified CLI with resolved files and config options
    let mut resolved_cli = cli.clone();
    resolved_cli.files = files;
    resolved_cli.out_dir = config.compiler_options.out_dir.as_ref().map(PathBuf::from);
    resolved_cli.out_file = config.compiler_options.out_file.as_ref().map(PathBuf::from);
    resolved_cli.source_map = config.compiler_options.source_map;
    resolved_cli.no_emit = config.compiler_options.no_emit;
    resolved_cli.pretty = config.compiler_options.pretty;
    resolved_cli.copy_lua_to_output = config.compiler_options.copy_lua_to_output;

    if cli.watch {
        watch_mode(resolved_cli)?;
    } else {
        compile(resolved_cli, target)?;
    }

    Ok(())
}

/// Initialize a new TypedLua project with a configuration file
fn init_project() -> anyhow::Result<()> {
    println!("Initializing new TypedLua project...");

    let config = r#"# TypedLua Configuration File
# https://typedlua.dev/docs/configuration

compilerOptions:
  target: "5.4"          # Lua version: 5.1, 5.2, 5.3, 5.4
  outDir: "./dist"       # Output directory for compiled files
  sourceMap: true        # Generate source maps
  strict: true           # Enable strict type checking

include:
  - "src/**/*"           # Files to include in compilation

exclude:
  - "node_modules"       # Files to exclude from compilation
  - "dist"
"#;

    std::fs::write("tlconfig.yaml", config)?;
    println!("Created tlconfig.yaml");

    // Create src directory if it doesn't exist
    std::fs::create_dir_all("src")?;
    println!("Created src/ directory");

    // Create a sample file
    let sample = r#"-- Welcome to TypedLua!
-- This is a sample file to get you started.

type Person = {
    name: string,
    age: number,
}

function greet(person: Person): string
    return "Hello, " .. person.name .. "!"
end

const user: Person = {
    name = "World",
    age = 42,
}

print(greet(user))
"#;

    std::fs::write("src/main.tl", sample)?;
    println!("Created src/main.tl");

    println!("\nProject initialized successfully!");
    println!("Run 'typedlua src/main.tl' to compile your first file.");

    Ok(())
}

/// Parse the Lua target version string
fn parse_lua_target(target: &str) -> anyhow::Result<typedlua_core::codegen::LuaTarget> {
    use typedlua_core::codegen::LuaTarget;

    match target {
        "5.1" | "51" => Ok(LuaTarget::Lua51),
        "5.2" | "52" => Ok(LuaTarget::Lua52),
        "5.3" | "53" => Ok(LuaTarget::Lua53),
        "5.4" | "54" => Ok(LuaTarget::Lua54),
        _ => Err(anyhow::anyhow!(
            "Invalid Lua target '{}'. Supported targets: 5.1, 5.2, 5.3, 5.4",
            target
        )),
    }
}

/// Load configuration from file (if specified) and resolve input files
fn load_config_and_files(
    cli: &Cli,
) -> anyhow::Result<(typedlua_core::config::CompilerConfig, Vec<PathBuf>)> {
    use typedlua_core::config::{CliOverrides, CompilerConfig, LuaVersion};

    // Start with default config
    let mut config = if let Some(ref project_path) = cli.project {
        // Load from file
        CompilerConfig::from_file(project_path)
            .map_err(|e| anyhow::anyhow!("Failed to load config file: {}", e))?
    } else {
        // Try to find tlconfig.yaml in current directory
        let default_path = PathBuf::from("tlconfig.yaml");
        if default_path.exists() {
            CompilerConfig::from_file(&default_path)
                .map_err(|e| anyhow::anyhow!("Failed to load tlconfig.yaml: {}", e))?
        } else {
            CompilerConfig::default()
        }
    };

    // Build CLI overrides
    let mut overrides = CliOverrides::default();

    // Override target if specified via CLI
    if cli.target != "5.4" {
        overrides.target = Some(match cli.target.as_str() {
            "5.1" | "51" => LuaVersion::Lua51,
            "5.2" | "52" => LuaVersion::Lua52,
            "5.3" | "53" => LuaVersion::Lua53,
            "5.4" | "54" => LuaVersion::Lua54,
            _ => LuaVersion::Lua54,
        });
    }

    // Override output options if specified
    if let Some(ref out_dir) = cli.out_dir {
        overrides.out_dir = Some(out_dir.to_string_lossy().to_string());
    }
    if let Some(ref out_file) = cli.out_file {
        overrides.out_file = Some(out_file.to_string_lossy().to_string());
    }
    if cli.source_map {
        overrides.source_map = Some(true);
    }
    if cli.no_emit {
        overrides.no_emit = Some(true);
    }

    // Override type checking options
    if cli.no_strict_null_checks {
        overrides.strict_null_checks = Some(false);
    }
    if let Some(ref naming) = cli.strict_naming {
        overrides.strict_naming = Some(match naming.as_str() {
            "error" => typedlua_core::config::StrictLevel::Error,
            "warning" => typedlua_core::config::StrictLevel::Warning,
            "off" => typedlua_core::config::StrictLevel::Off,
            _ => typedlua_core::config::StrictLevel::Error,
        });
    }
    if cli.no_implicit_unknown {
        overrides.no_implicit_unknown = Some(true);
    }

    // Override module options
    overrides.enable_decorators = Some(cli.enable_decorators);
    if let Some(ref mode) = cli.module_mode {
        overrides.module_mode = Some(match mode.as_str() {
            "bundle" => typedlua_core::config::ModuleMode::Bundle,
            _ => typedlua_core::config::ModuleMode::Require,
        });
    }
    if let Some(ref paths) = cli.module_paths {
        overrides.module_paths = Some(paths.split(',').map(|s| s.to_string()).collect());
    }
    overrides.enforce_namespace_path = Some(cli.enforce_namespace_path);
    overrides.copy_lua_to_output = Some(cli.copy_lua_to_output);

    // Merge CLI overrides into config
    config.merge(&overrides);

    // Determine input files
    let files = if !cli.files.is_empty() {
        // Use files from command line
        cli.files.clone()
    } else {
        // Use files from config (would need glob expansion here)
        // For now, just return empty - in a full implementation, we'd expand glob patterns
        Vec::new()
    };

    Ok((config, files))
}

/// Result of compiling a single file
struct CompilationResult {
    file_path: PathBuf,
    result: Result<CompilationOutput, CompilationError>,
}

struct CompilationOutput {
    lua_code: String,
    source_map: Option<typedlua_core::codegen::SourceMap>,
    output_path: PathBuf,
    /// Module to save to cache after compilation (stale files only)
    /// Tuple of (path, cached_module, dependencies)
    cache_entry: Option<(PathBuf, typedlua_core::cache::CachedModule, Vec<PathBuf>)>,
}

struct CompilationError {
    diagnostics: Vec<typedlua_core::diagnostics::Diagnostic>,
    source: String,
}

/// Compile the input files
fn compile(cli: Cli, target: typedlua_core::codegen::LuaTarget) -> anyhow::Result<()> {
    use rayon::prelude::*;
    use rustc_hash::FxHashSet;
    use std::collections::HashMap;
    use std::sync::Arc;
    use typedlua_core::cache::{CacheManager, CachedModule};
    use typedlua_core::codegen::CodeGeneratorBuilder;
    use typedlua_core::config::{CompilerConfig, CompilerOptions};
    use typedlua_core::diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler};

    use typedlua_core::module_resolver::{ModuleConfig, ModuleId, ModuleRegistry, ModuleResolver};
    use typedlua_parser::lexer::Lexer;
    use typedlua_parser::parser::Parser;
    use typedlua_parser::string_interner::StringInterner;

    info!("Compiling {} file(s)...", cli.files.len());

    // --- DI Container setup ---
    let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let compiler_config = CompilerConfig::default();
    let container = typedlua_core::di::Container::new(compiler_config);
    let use_cache = !cli.no_cache;

    // Determine which files need recompilation
    let stale_files: FxHashSet<PathBuf>;
    let cached_modules: HashMap<PathBuf, CachedModule>;

    if use_cache {
        let config = CompilerOptions::default();
        let mut cache_manager = CacheManager::new(&project_root, &config)
            .unwrap_or_else(|_| CacheManager::new(Path::new("."), &config).unwrap());

        if cache_manager.load_manifest().is_err() {
            let _ = cache_manager.clear();
            let _ = cache_manager.load_manifest();
        }

        // Detect changes and compute stale set
        let changed = cache_manager.detect_changes(&cli.files).unwrap_or_default();
        stale_files = cache_manager.compute_stale_modules(&changed);

        // Pre-load cached modules for non-stale files
        let mut loaded = HashMap::new();
        for file_path in &cli.files {
            let canonical = file_path
                .canonicalize()
                .unwrap_or_else(|_| file_path.clone());
            if !stale_files.contains(&canonical) {
                if let Ok(Some(cached)) = cache_manager.get_cached_module(&canonical) {
                    loaded.insert(canonical, cached);
                }
            }
        }
        cached_modules = loaded;

        let cache_hits = cached_modules.len();
        let total = cli.files.len();
        if cache_hits > 0 {
            info!(
                "Cache: {} of {} files unchanged, {} need recompilation",
                cache_hits,
                total,
                total - cache_hits
            );
        }
    } else {
        // No cache: everything is stale
        stale_files = cli
            .files
            .iter()
            .map(|f| f.canonicalize().unwrap_or_else(|_| f.clone()))
            .collect();
        cached_modules = HashMap::new();
    }

    // --- Module registry and resolver for cross-file type resolution ---
    let registry = Arc::new(ModuleRegistry::new());

    // Create module resolver using FileSystem from DI Container
    let module_config =
        ModuleConfig::from_compiler_options(&CompilerOptions::default(), &project_root);
    let resolver = Arc::new(ModuleResolver::new(
        container.file_system().clone(),
        module_config,
        project_root.clone(),
    ));

    // Pre-populate registry with cached exports for non-stale files
    for (canonical, cached) in &cached_modules {
        let module_id = ModuleId::new(canonical.clone());
        let symbol_table = Arc::new(typedlua_core::typechecker::SymbolTable::from_serializable(
            cached.symbol_table.clone(),
        ));
        registry.register_from_cache(module_id, cached.exports.clone(), symbol_table);
    }

    // --- Compile files ---
    let results: Vec<CompilationResult> = cli
        .files
        .par_iter()
        .map(|file_path| {
            let canonical = file_path
                .canonicalize()
                .unwrap_or_else(|_| file_path.clone());
            let is_stale = stale_files.contains(&canonical);

            // --- Cache hit: use cached AST, skip parse + type check ---
            if !is_stale {
                if let Some(cached) = cached_modules.get(&canonical) {
                    debug!("Cache hit: {:?}", file_path);

                    if cli.no_emit {
                        return CompilationResult {
                            file_path: file_path.clone(),
                            result: Ok(CompilationOutput {
                                lua_code: String::new(),
                                source_map: None,
                                output_path: PathBuf::new(),
                                cache_entry: None,
                            }),
                        };
                    }

                    // Reconstruct interner from cached strings
                    let interner = std::rc::Rc::new(StringInterner::from_strings(
                        cached.interner_strings.clone(),
                    ));
                    let mut program = cached.ast.clone();

                    // Use CodeGeneratorBuilder for fluent configuration
                    let mut builder = CodeGeneratorBuilder::new(interner).target(target);
                    if cli.source_map || cli.inline_source_map {
                        builder = builder.source_map(file_path.to_string_lossy().to_string());
                    }
                    let mut generator = builder.build();

                    let lua_code = generator.generate(&mut program);

                    let output_path = determine_output_path(file_path, &cli);
                    let source_map = generator.take_source_map();

                    return CompilationResult {
                        file_path: file_path.clone(),
                        result: Ok(CompilationOutput {
                            lua_code,
                            source_map,
                            output_path,
                            cache_entry: None,
                        }),
                    };
                }
            }

            // --- Cache miss: full compilation ---
            debug!("Compiling {:?}...", file_path);

            // Read input file using FileSystem abstraction from DI Container
            let source = match container.file_system().read_file(file_path) {
                Ok(s) => s,
                Err(e) => {
                    return CompilationResult {
                        file_path: file_path.clone(),
                        result: Err(CompilationError {
                            diagnostics: vec![],
                            source: format!("Failed to read file: {}", e),
                        }),
                    };
                }
            };

            // Create diagnostic handler
            let handler = Arc::new(CollectingDiagnosticHandler::new());

            // Create string interner with common identifiers
            let (interner, common_ids) = StringInterner::new_with_common_identifiers();
            let interner = std::rc::Rc::new(interner);

            // Lex the source
            let mut lexer = Lexer::new(&source, handler.clone(), &interner);
            let tokens = match lexer.tokenize() {
                Ok(tokens) => tokens,
                Err(_) => {
                    return CompilationResult {
                        file_path: file_path.clone(),
                        result: Err(CompilationError {
                            diagnostics: handler.get_diagnostics(),
                            source,
                        }),
                    };
                }
            };

            // Parse the tokens
            let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
            let mut program = match parser.parse() {
                Ok(program) => program,
                Err(_) => {
                    return CompilationResult {
                        file_path: file_path.clone(),
                        result: Err(CompilationError {
                            diagnostics: handler.get_diagnostics(),
                            source,
                        }),
                    };
                }
            };

            // Check for any diagnostics after parsing
            if handler.has_errors() {
                return CompilationResult {
                    file_path: file_path.clone(),
                    result: Err(CompilationError {
                        diagnostics: handler.get_diagnostics(),
                        source,
                    }),
                };
            }

            // Type check the program (with module support for import resolution)
            use typedlua_core::typechecker::TypeChecker;

            let module_id = ModuleId::new(canonical.clone());
            let mut type_checker = TypeChecker::new_with_module_support(
                handler.clone(),
                &interner,
                &common_ids,
                registry.clone(),
                module_id.clone(),
                resolver.clone(),
            );

            if type_checker.check_program(&mut program).is_err() {
                return CompilationResult {
                    file_path: file_path.clone(),
                    result: Err(CompilationError {
                        diagnostics: handler.get_diagnostics(),
                        source,
                    }),
                };
            }

            // Check for type errors
            if handler.has_errors() {
                return CompilationResult {
                    file_path: file_path.clone(),
                    result: Err(CompilationError {
                        diagnostics: handler.get_diagnostics(),
                        source,
                    }),
                };
            }

            // Register exports in shared registry for other files
            let exports = type_checker.extract_exports(&program);
            let serializable_st = type_checker.symbol_table().to_serializable();
            let symbol_table_arc = Arc::new(typedlua_core::SymbolTable::from_serializable(
                serializable_st.clone(),
            ));
            registry.register_from_cache(module_id.clone(), exports.clone(), symbol_table_arc);

            // Build cache entry to save after parallel section
            let cache_entry = if use_cache {
                // Get dependencies for cache invalidation
                let dependencies: Vec<PathBuf> = type_checker.get_module_dependencies().to_vec();

                Some((
                    canonical,
                    CachedModule::new(
                        file_path
                            .canonicalize()
                            .unwrap_or_else(|_| file_path.clone()),
                        program.clone(),
                        exports,
                        type_checker.symbol_table().to_serializable(),
                        interner.to_strings(),
                    ),
                    dependencies,
                ))
            } else {
                None
            };

            // Generate Lua code
            if cli.no_emit {
                return CompilationResult {
                    file_path: file_path.clone(),
                    result: Ok(CompilationOutput {
                        lua_code: String::new(),
                        source_map: None,
                        output_path: PathBuf::new(),
                        cache_entry,
                    }),
                };
            }

            // Use CodeGeneratorBuilder for fluent configuration
            let mut builder = CodeGeneratorBuilder::new(interner.clone()).target(target);

            if cli.source_map || cli.inline_source_map {
                builder = builder.source_map(file_path.to_string_lossy().to_string());
            }

            let mut generator = builder.build();
            let lua_code = generator.generate(&mut program);

            let output_path = determine_output_path(file_path, &cli);
            let source_map = generator.take_source_map();

            CompilationResult {
                file_path: file_path.clone(),
                result: Ok(CompilationOutput {
                    lua_code,
                    source_map,
                    output_path,
                    cache_entry,
                }),
            }
        })
        .collect();

    // --- Save cache entries (sequential — CacheManager needs &mut self) ---
    if use_cache {
        let config = CompilerOptions::default();
        if let Ok(mut cache_manager) = CacheManager::new(&project_root, &config) {
            if cache_manager.load_manifest().is_err() {
                let _ = cache_manager.clear();
                let _ = cache_manager.load_manifest();
            }

            for result in &results {
                if let Ok(output) = &result.result {
                    if let Some((ref path, ref cached_module, ref dependencies)) =
                        output.cache_entry
                    {
                        let _ =
                            cache_manager.save_module(path, cached_module, dependencies.clone());
                    }
                }
            }

            let _ = cache_manager.save_manifest();
        }
    }

    // Process results sequentially (for deterministic output and error reporting)
    let mut had_errors = false;

    for result in results {
        match result.result {
            Ok(output) => {
                if !cli.no_emit {
                    // Write output file
                    if let Some(parent) = output.output_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&output.output_path, &output.lua_code)?;
                    info!("Generated: {:?}", output.output_path);

                    // Write source map if requested
                    if cli.source_map && !cli.inline_source_map {
                        if let Some(source_map) = output.source_map {
                            let map_path = output.output_path.with_extension("lua.map");
                            let map_json = source_map.to_json()?;
                            std::fs::write(&map_path, map_json)?;
                            info!("Generated source map: {:?}", map_path);
                        }
                    }
                }
            }
            Err(error) => {
                had_errors = true;
                if error.diagnostics.is_empty() {
                    // File read error or similar
                    eprintln!("Error compiling {:?}: {}", result.file_path, error.source);
                } else {
                    // Print diagnostics
                    print_diagnostics_from_vec(
                        &error.diagnostics,
                        &error.source,
                        &result.file_path,
                        cli.pretty,
                        cli.diagnostics,
                    );
                }
            }
        }
    }

    if had_errors {
        std::process::exit(1);
    }

    // Copy plain .lua files to output directory if requested
    if cli.copy_lua_to_output && !cli.no_emit {
        copy_lua_files_to_output(&cli)?;
    }

    info!("Compilation completed successfully!");

    Ok(())
}

/// Copy plain .lua files to the output directory
fn copy_lua_files_to_output(cli: &Cli) -> anyhow::Result<()> {
    use std::fs;
    use walkdir::WalkDir;

    let out_dir = cli.out_dir.clone().unwrap_or_else(|| PathBuf::from("."));

    info!("Copying .lua files to output directory: {:?}", out_dir);

    // Walk the current directory looking for .lua files
    for entry in WalkDir::new(".")
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip if not a .lua file
        if !path.extension().map(|e| e == "lua").unwrap_or(false) {
            continue;
        }

        // Skip files in node_modules, .git, etc.
        let path_str = path.to_string_lossy();
        if path_str.contains("node_modules")
            || path_str.contains(".git")
            || path_str.contains(".typed-lua-cache")
            || path_str.contains("target")
        {
            continue;
        }

        // Determine output path
        let file_name = path.file_name().unwrap_or_default();
        let output_path = out_dir.join(file_name);

        // Copy the file
        match fs::copy(path, &output_path) {
            Ok(_) => {
                info!("Copied: {:?} -> {:?}", path, output_path);
            }
            Err(e) => {
                warn!("Failed to copy {:?}: {}", path, e);
            }
        }
    }

    Ok(())
}

/// Determine the output file path for a given input file
fn determine_output_path(file_path: &Path, cli: &Cli) -> PathBuf {
    if let Some(out_file) = &cli.out_file {
        out_file.clone()
    } else if let Some(out_dir) = &cli.out_dir {
        let file_name = file_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        out_dir.join(format!("{}.lua", file_name))
    } else {
        file_path.with_extension("lua")
    }
}

/// Print diagnostics from a vec (used by parallel compilation)
fn print_diagnostics_from_vec(
    diagnostics: &[typedlua_core::diagnostics::Diagnostic],
    source: &str,
    file_path: &Path,
    pretty: bool,
    show_codes: bool,
) {
    use typedlua_core::diagnostics::DiagnosticLevel;

    if diagnostics.is_empty() {
        return;
    }

    let file_name = file_path.to_string_lossy();

    for diagnostic in diagnostics {
        // Format diagnostic code if present and requested
        let code_str = if show_codes {
            diagnostic
                .code
                .as_ref()
                .map(|c| format!(" [{}]", c.as_str()))
                .unwrap_or_default()
        } else {
            String::new()
        };

        if pretty {
            // Pretty format with colors and context
            let severity_str = match diagnostic.level {
                DiagnosticLevel::Error => "\x1b[31merror\x1b[0m",
                DiagnosticLevel::Warning => "\x1b[33mwarning\x1b[0m",
                DiagnosticLevel::Info => "\x1b[34minfo\x1b[0m",
            };

            eprintln!(
                "\n{} [{}:{}:{}]: {}{}",
                severity_str,
                file_name,
                diagnostic.span.line,
                diagnostic.span.column,
                diagnostic.message,
                code_str
            );

            // Show the source line with a caret pointing to the error
            let lines: Vec<&str> = source.lines().collect();
            if diagnostic.span.line > 0 && (diagnostic.span.line as usize) <= lines.len() {
                let line = lines[diagnostic.span.line as usize - 1];
                eprintln!("    {}", line);
                eprintln!(
                    "    {}\x1b[31m^\x1b[0m",
                    " ".repeat(diagnostic.span.column.saturating_sub(1) as usize)
                );
            }
        } else {
            // Simple format (no colors)
            let severity_str = match diagnostic.level {
                DiagnosticLevel::Error => "error",
                DiagnosticLevel::Warning => "warning",
                DiagnosticLevel::Info => "info",
            };

            eprintln!(
                "{}:{}:{}: {}: {}{}",
                file_name,
                diagnostic.span.line,
                diagnostic.span.column,
                severity_str,
                diagnostic.message,
                code_str
            );
        }
    }

    eprintln!();
}

/// Watch mode - recompile on file changes
fn watch_mode(cli: Cli) -> anyhow::Result<()> {
    use notify::{
        event::{EventKind, ModifyKind},
        Event, RecursiveMode, Watcher,
    };
    use std::sync::mpsc::channel;
    use std::time::Duration;

    let target = parse_lua_target(&cli.target)?;

    println!("Watching for changes... (Press Ctrl+C to stop)");

    // Initial compilation
    println!("\nInitial compilation:");
    let _ = compile(cli.clone(), target);

    // Create a channel to receive file system events
    let (tx, rx) = channel();

    // Create a watcher
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;

    // Watch all input files and their parent directories
    for file_path in &cli.files {
        if let Some(parent) = file_path.parent() {
            watcher.watch(parent, RecursiveMode::NonRecursive)?;
        } else {
            watcher.watch(file_path, RecursiveMode::NonRecursive)?;
        }
    }

    // Handle file system events
    let mut last_compile = std::time::Instant::now();
    let debounce_duration = Duration::from_millis(100);

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                // Check if this is a file modification event
                let should_recompile = matches!(
                    event.kind,
                    EventKind::Modify(ModifyKind::Data(_)) | EventKind::Create(_)
                );

                if should_recompile {
                    // Check if any of the changed paths match our input files
                    let changed_our_files = event.paths.iter().any(|path| {
                        cli.files
                            .iter()
                            .any(|file| path.file_name() == file.file_name())
                    });

                    if changed_our_files {
                        // Debounce: only recompile if enough time has passed
                        let now = std::time::Instant::now();
                        if now.duration_since(last_compile) >= debounce_duration {
                            println!("\n\nFile changed, recompiling...");
                            let _ = compile(cli.clone(), target);
                            last_compile = now;
                        }
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // No events, continue watching
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(anyhow::anyhow!("File watcher disconnected"));
            }
        }
    }
}
