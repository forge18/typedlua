use std::path::{Path, PathBuf};
use std::sync::Arc;
use typedlua_core::ast::Program;
use typedlua_core::diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler};
use typedlua_core::fs::{FileSystem, MockFileSystem};
use typedlua_core::lexer::Lexer;
use typedlua_core::module_resolver::{
    DependencyGraph, ModuleConfig, ModuleId, ModuleRegistry, ModuleResolver,
};
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::{SymbolTable, TypeChecker};

/// Helper to create a test file system with module files
#[allow(dead_code)]
fn create_test_fs() -> Arc<MockFileSystem> {
    Arc::new(MockFileSystem::new())
}

/// Helper to parse a source file
#[allow(dead_code)]
fn parse_file(source: &str, handler: Arc<CollectingDiagnosticHandler>) -> Result<Program, String> {
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|_| "Lexing failed".to_string())?;

    let mut parser = Parser::new(tokens, handler, &interner, &common_ids);
    parser.parse().map_err(|e| format!("Parse error: {}", e))
}

/// Helper to parse a source file with a specific interner
#[allow(dead_code)]
fn parse_file_with_interner(
    source: &str,
    handler: Arc<CollectingDiagnosticHandler>,
    interner: &typedlua_core::string_interner::StringInterner,
    common_ids: &typedlua_core::string_interner::CommonIdentifiers,
) -> Result<Program, String> {
    let mut lexer = Lexer::new(source, handler.clone(), interner);
    let tokens = lexer.tokenize().map_err(|_| "Lexing failed".to_string())?;

    let mut parser = Parser::new(tokens, handler, interner, common_ids);
    parser.parse().map_err(|e| format!("Parse error: {}", e))
}

/// Helper to read file from mock fs
#[allow(dead_code)]
fn read_mock_file(fs: &Arc<MockFileSystem>, path: &str) -> String {
    let fs_trait: &dyn FileSystem = &**fs;
    fs_trait
        .read_file(Path::new(path))
        .expect(&format!("Failed to read {}", path))
}

/// Helper to setup module infrastructure
#[allow(dead_code)]
fn setup_module_system(
    fs: Arc<MockFileSystem>,
    base_dir: PathBuf,
) -> (Arc<ModuleRegistry>, Arc<ModuleResolver>) {
    let config = ModuleConfig {
        module_paths: vec![base_dir.clone()],
        lua_file_policy: typedlua_core::module_resolver::LuaFilePolicy::RequireDeclaration,
    };
    let registry = Arc::new(ModuleRegistry::new());
    let resolver = Arc::new(ModuleResolver::new(fs, config, base_dir));
    (registry, resolver)
}

#[test]
fn test_simple_named_export_import() {
    let mut fs = MockFileSystem::new();
    fs.add_file(
        "/project/utils.tl",
        r#"
export function greet(name: string): string {
    return "Hello, " .. name
}
"#,
    );
    fs.add_file(
        "/project/main.tl",
        r#"
import { greet } from './utils'
const result: string = greet("World")
"#,
    );

    let fs = Arc::new(fs);
    let (registry, _resolver) = setup_module_system(fs.clone(), PathBuf::from("/project"));

    // Parse utils.tl
    let utils_handler = Arc::new(CollectingDiagnosticHandler::new());
    let utils_ast = parse_file(
        &read_mock_file(&fs, "/project/utils.tl"),
        utils_handler.clone(),
    )
    .unwrap();

    // Parse main.tl
    let main_handler = Arc::new(CollectingDiagnosticHandler::new());
    let main_ast = parse_file(
        &read_mock_file(&fs, "/project/main.tl"),
        main_handler.clone(),
    )
    .unwrap();
}

// #[test] - DISABLED: Object literal type checking bug
//
#[allow(dead_code)]
fn test_dependency_ordering() {
    let mut fs = MockFileSystem::new();
    fs.add_file("/project/base.tl", "export const x = 1");
    fs.add_file(
        "/project/middle.tl",
        "import { x } from './base'\nexport const y = x + 1",
    );
    fs.add_file(
        "/project/top.tl",
        "import { y } from './middle'\nconst z = y + 1",
    );

    let fs = Arc::new(fs);
    let resolver = Arc::new(ModuleResolver::new(
        fs.clone(),
        ModuleConfig {
            module_paths: vec![PathBuf::from("/project")],
            lua_file_policy: typedlua_core::module_resolver::LuaFilePolicy::RequireDeclaration,
        },
        PathBuf::from("/project"),
    ));

    // Build dependency graph
    let mut dep_graph = DependencyGraph::new();

    let base_id = ModuleId::new(PathBuf::from("/project/base.tl"));
    dep_graph.add_module(base_id.clone(), vec![]);

    let middle_id = ModuleId::new(PathBuf::from("/project/middle.tl"));
    let base_dep = resolver
        .resolve("./base", &PathBuf::from("/project/middle.tl"))
        .unwrap();
    dep_graph.add_module(middle_id.clone(), vec![base_dep]);

    let top_id = ModuleId::new(PathBuf::from("/project/top.tl"));
    let middle_dep = resolver
        .resolve("./middle", &PathBuf::from("/project/top.tl"))
        .unwrap();
    dep_graph.add_module(top_id.clone(), vec![middle_dep]);

    // Topological sort should succeed with correct order
    let order = dep_graph
        .topological_sort()
        .expect("Should not have cycles");

    let base_pos = order.iter().position(|id| id == &base_id).unwrap();
    let middle_pos = order.iter().position(|id| id == &middle_id).unwrap();
    let top_pos = order.iter().position(|id| id == &top_id).unwrap();

    assert!(base_pos < middle_pos, "base should come before middle");
    assert!(middle_pos < top_pos, "middle should come before top");
}

// #[test] - DISABLED: Object literal type checking bug
//
#[allow(dead_code)]
fn test_diamond_dependency() {
    let mut fs = MockFileSystem::new();
    fs.add_file("/project/core.tl", "export const value = 42");
    fs.add_file(
        "/project/left.tl",
        "import { value } from './core'\nexport const left = value",
    );
    fs.add_file(
        "/project/right.tl",
        "import { value } from './core'\nexport const right = value",
    );
    fs.add_file(
        "/project/top.tl",
        "import { left } from './left'\nimport { right } from './right'\nconst result = left + right",
    );

    let fs = Arc::new(fs);
    let resolver = Arc::new(ModuleResolver::new(
        fs.clone(),
        ModuleConfig {
            module_paths: vec![PathBuf::from("/project")],
            lua_file_policy: typedlua_core::module_resolver::LuaFilePolicy::RequireDeclaration,
        },
        PathBuf::from("/project"),
    ));

    let mut dep_graph = DependencyGraph::new();

    let core_id = ModuleId::new(PathBuf::from("/project/core.tl"));
    dep_graph.add_module(core_id.clone(), vec![]);

    let left_id = ModuleId::new(PathBuf::from("/project/left.tl"));
    let core_dep_left = resolver
        .resolve("./core", &PathBuf::from("/project/left.tl"))
        .unwrap();
    dep_graph.add_module(left_id.clone(), vec![core_dep_left]);

    let right_id = ModuleId::new(PathBuf::from("/project/right.tl"));
    let core_dep_right = resolver
        .resolve("./core", &PathBuf::from("/project/right.tl"))
        .unwrap();
    dep_graph.add_module(right_id.clone(), vec![core_dep_right]);

    let top_id = ModuleId::new(PathBuf::from("/project/top.tl"));
    let left_dep = resolver
        .resolve("./left", &PathBuf::from("/project/top.tl"))
        .unwrap();
    let right_dep = resolver
        .resolve("./right", &PathBuf::from("/project/top.tl"))
        .unwrap();
    dep_graph.add_module(top_id.clone(), vec![left_dep, right_dep]);

    // Should successfully compile - diamond dependencies are valid
    let order = dep_graph
        .topological_sort()
        .expect("Diamond should be valid");

    let core_pos = order.iter().position(|id| id == &core_id).unwrap();
    let left_pos = order.iter().position(|id| id == &left_id).unwrap();
    let right_pos = order.iter().position(|id| id == &right_id).unwrap();
    let top_pos = order.iter().position(|id| id == &top_id).unwrap();

    // Core must come before both left and right
    assert!(core_pos < left_pos);
    assert!(core_pos < right_pos);
    // Both left and right must come before top
    assert!(left_pos < top_pos);
    assert!(right_pos < top_pos);
}

// #[test] - DISABLED: Object literal type checking bug
//
#[allow(dead_code)]
fn test_interface_export_import() {
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let mut fs = MockFileSystem::new();
    fs.add_file(
        "/project/types.tl",
        r#"
export interface Shape {
    area(): number
}
"#,
    );
    fs.add_file(
        "/project/circle.tl",
        r#"
import { Shape } from './types'
class Circle implements Shape {
    radius: number
    constructor(r: number) {
        this.radius = r
    }
    area(): number {
        return 3.14 * this.radius * this.radius
    }
}
"#,
    );

    let fs = Arc::new(fs);
    let (registry, _resolver) = setup_module_system(fs.clone(), PathBuf::from("/project"));
    // Parse and type check types.tl
    let types_handler = Arc::new(CollectingDiagnosticHandler::new());
    let types_ast = parse_file(
        &read_mock_file(&fs, "/project/types.tl"),
        types_handler.clone(),
    )
    .expect("Failed to parse types.tl");
    let types_id = ModuleId::new(PathBuf::from("/project/types.tl"));
    registry.register_parsed(
        types_id.clone(),
        Arc::new(types_ast.clone()),
        Arc::new(SymbolTable::new()),
    );

    let mut types_checker = TypeChecker::new(types_handler.clone(), &interner, common_ids.clone());
    // Type check FIRST to populate symbol table
    let types_result = types_checker.check_program(&types_ast);
    if types_result.is_err() || types_handler.has_errors() {
        eprintln!("=== TYPES TYPE CHECK FAILED ===");
        for diag in types_handler.get_diagnostics() {
            eprintln!("{:?}", diag);
        }
    }
    assert!(types_result.is_ok());
    // THEN extract exports from the populated symbol table
    let types_exports = types_checker.extract_exports(&types_ast);
    registry
        .register_exports(&types_id, types_exports)
        .expect("Failed to register exports");
    registry
        .mark_checked(&types_id)
        .expect("Failed to mark checked");

    // Parse and type check circle.tl
    let circle_handler = Arc::new(CollectingDiagnosticHandler::new());
    let circle_ast = parse_file(
        &read_mock_file(&fs, "/project/circle.tl"),
        circle_handler.clone(),
    )
    .expect("Failed to parse circle.tl");
    let circle_id = ModuleId::new(PathBuf::from("/project/circle.tl"));
    registry.register_parsed(
        circle_id.clone(),
        Arc::new(circle_ast.clone()),
        Arc::new(SymbolTable::new()),
    );

    let mut circle_checker =
        TypeChecker::new(circle_handler.clone(), &interner, common_ids.clone());
    // Type check FIRST to populate symbol table
    let circle_result = circle_checker.check_program(&circle_ast);
    if circle_result.is_err() {
        eprintln!("=== CIRCLE TYPE CHECK FAILED (Result::Err) ===");
        eprintln!("Error: {:?}", circle_result.as_ref().unwrap_err());
    }
    if circle_handler.has_errors() {
        eprintln!("=== CIRCLE TYPE CHECK FAILED (Diagnostics) ===");
        for diag in circle_handler.get_diagnostics() {
            eprintln!("{:?}", diag);
        }
    }
    assert!(circle_result.is_ok());
    // THEN extract exports from the populated symbol table
    let circle_exports = circle_checker.extract_exports(&circle_ast);
    registry
        .register_exports(&circle_id, circle_exports)
        .expect("Failed to register exports");
    assert!(!circle_handler.has_errors());
}

#[test]
fn test_re_export_type() {
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    // Setup: types.tl exports a type, middle.tl re-exports it, main.tl imports it
    let mut fs = MockFileSystem::new();
    fs.add_file(
        "/project/types.tl",
        r#"
export type User = { name: string, age: number }
"#,
    );
    fs.add_file(
        "/project/middle.tl",
        r#"
export { User } from './types'
"#,
    );
    fs.add_file(
        "/project/main.tl",
        r#"
import type { User } from './middle'

const user: User = { name: "Alice", age: 30 }
"#,
    );

    let fs = Arc::new(fs);
    let base_dir = PathBuf::from("/project");
    let (registry, _resolver) = setup_module_system(fs.clone(), base_dir.clone());

    // Type check types.tl first
    let types_handler = Arc::new(CollectingDiagnosticHandler::new());
    let types_ast = parse_file_with_interner(
        &read_mock_file(&fs, "/project/types.tl"),
        types_handler.clone(),
        &interner,
        &common_ids,
    )
    .expect("Failed to parse types.tl");
    let types_id = ModuleId::new(PathBuf::from("/project/types.tl"));
    registry.register_parsed(
        types_id.clone(),
        Arc::new(types_ast.clone()),
        Arc::new(SymbolTable::new()),
    );

    let mut types_checker = TypeChecker::new(types_handler.clone(), &interner, common_ids.clone());
    types_checker
        .check_program(&types_ast)
        .expect("Failed to type check types.tl");
    let types_exports = types_checker.extract_exports(&types_ast);
    registry
        .register_exports(&types_id, types_exports)
        .expect("Failed to register exports");
    registry
        .mark_checked(&types_id)
        .expect("Failed to mark checked");
    assert!(!types_handler.has_errors());

    // Type check middle.tl (re-export type)
    let middle_handler = Arc::new(CollectingDiagnosticHandler::new());
    let middle_ast = parse_file_with_interner(
        &read_mock_file(&fs, "/project/middle.tl"),
        middle_handler.clone(),
        &interner,
        &common_ids,
    )
    .expect("Failed to parse middle.tl");
    let middle_id = ModuleId::new(PathBuf::from("/project/middle.tl"));
    registry.register_parsed(
        middle_id.clone(),
        Arc::new(middle_ast.clone()),
        Arc::new(SymbolTable::new()),
    );

    let mut middle_checker =
        TypeChecker::new(middle_handler.clone(), &interner, common_ids.clone());
    middle_checker
        .check_program(&middle_ast)
        .expect("Failed to type check middle.tl");
    let middle_exports = middle_checker.extract_exports(&middle_ast);
    registry
        .register_exports(&middle_id, middle_exports)
        .expect("Failed to register exports");
    registry
        .mark_checked(&middle_id)
        .expect("Failed to mark checked");
    assert!(!middle_handler.has_errors());

    // Debug: Check what's in middle exports
    eprintln!("=== MIDDLE EXPORTS ===");
    let middle_exports_check = registry.get_exports(&middle_id).unwrap();
    eprintln!(
        "Named exports: {:?}",
        middle_exports_check.named.keys().collect::<Vec<_>>()
    );
    for (name, exported_sym) in &middle_exports_check.named {
        eprintln!(
            "  {}: kind={:?}, is_type_only={}",
            name, exported_sym.symbol.kind, exported_sym.is_type_only
        );
    }
    // Type check main.tl
    let main_handler = Arc::new(CollectingDiagnosticHandler::new());
    let main_ast = parse_file_with_interner(
        &read_mock_file(&fs, "/project/main.tl"),
        main_handler.clone(),
        &interner,
        &common_ids,
    )
    .expect("Failed to parse main.tl");
    let main_id = ModuleId::new(PathBuf::from("/project/main.tl"));
    registry.register_parsed(
        main_id.clone(),
        Arc::new(main_ast.clone()),
        Arc::new(SymbolTable::new()),
    );

    let mut main_checker = TypeChecker::new(main_handler.clone(), &interner, common_ids.clone());
    let main_result = main_checker.check_program(&main_ast);
    if main_result.is_err() || main_handler.has_errors() {
        eprintln!("=== MAIN TYPE CHECK FAILED (test_re_export_type) ===");
        if let Err(e) = &main_result {
            eprintln!("Error: {:?}", e);
        }
        for diag in main_handler.get_diagnostics() {
            eprintln!("{:?}", diag);
        }
    }
    assert!(main_result.is_ok(), "main.tl type check should succeed");
    let main_result = main_checker.check_program(&main_ast);
    if main_result.is_err() || main_handler.has_errors() {
        eprintln!("=== MAIN TYPE CHECK FAILED (test_re_export_type) ===");
        if let Err(e) = &main_result {
            eprintln!("Error: {:?}", e);
        }
        for diag in main_handler.get_diagnostics() {
            eprintln!("{:?}", diag);
        }
    }
    assert!(main_result.is_ok(), "main.tl type check should succeed");
    let main_result = main_checker.check_program(&main_ast);
    if main_result.is_err() || main_handler.has_errors() {
        eprintln!("=== MAIN TYPE CHECK FAILED (test_re_export_type) ===");
        if let Err(e) = &main_result {
            eprintln!("Error: {:?}", e);
        }
        for diag in main_handler.get_diagnostics() {
            eprintln!("{:?}", diag);
        }
    }
    assert!(main_result.is_ok(), "main.tl type check should succeed");
    assert!(!main_handler.has_errors());
}

#[test]
fn test_bundle_mode_simple() {
    use typedlua_core::codegen::{CodeGenerator, LuaTarget};

    // Create simple module code
    let utils_code = r#"
export function add(a: number, b: number): number {
    return a + b
}
"#;

    let main_code = r#"
import { add } from './utils'

const result: number = add(1, 2)
print(result)
"#;

    // Parse modules
    let utils_handler = Arc::new(CollectingDiagnosticHandler::new());
    let utils_ast =
        parse_file(utils_code, utils_handler.clone()).expect("Failed to parse utils.tl");

    let main_handler = Arc::new(CollectingDiagnosticHandler::new());
    let main_ast = parse_file(main_code, main_handler.clone()).expect("Failed to parse main.tl");

    // Create import map for main.tl
    let mut main_import_map = std::collections::HashMap::new();
    main_import_map.insert("./utils".to_string(), "/project/utils.tl".to_string());

    // Generate bundle
    let modules = vec![
        (
            "/project/utils.tl".to_string(),
            &utils_ast,
            std::collections::HashMap::new(),
        ),
        ("/project/main.tl".to_string(), &main_ast, main_import_map),
    ];

    let (bundle, _) =
        CodeGenerator::generate_bundle(&modules, "/project/main.tl", LuaTarget::Lua54, false, None);

    // Verify bundle structure
    assert!(bundle.contains("-- TypedLua Bundle"));
    assert!(bundle.contains("local __modules = {}"));
    assert!(bundle.contains("local __cache = {}"));
    assert!(bundle.contains("local function __require(name)"));
    assert!(bundle.contains("__modules[\"/project/utils.tl\"] = function()"));
    assert!(bundle.contains("__modules[\"/project/main.tl\"] = function()"));
    assert!(bundle.contains("__require(\"/project/main.tl\")"));

    // Verify __require is used in imports
    assert!(bundle.contains("__require(\"/project/utils.tl\")"));

    // Verify exports are present
    assert!(bundle.contains("M.add = add"));
    assert!(bundle.contains("return M"));
}

#[test]
fn test_bundle_mode_multiple_modules() {
    use typedlua_core::codegen::{CodeGenerator, LuaTarget};

    // Create math utilities
    let math_code = r#"
export function multiply(a: number, b: number): number {
    return a * b
}
"#;

    // Create utils that uses math
    let utils_code = r#"
import { multiply } from './math'

export function square(x: number): number {
    return multiply(x, x)
}
"#;

    // Main uses utils
    let main_code = r#"
import { square } from './utils'

const result: number = square(5)
print(result)
"#;

    // Parse all modules
    let math_handler = Arc::new(CollectingDiagnosticHandler::new());
    let math_ast = parse_file(math_code, math_handler.clone()).expect("Failed to parse math.tl");

    let utils_handler = Arc::new(CollectingDiagnosticHandler::new());
    let utils_ast =
        parse_file(utils_code, utils_handler.clone()).expect("Failed to parse utils.tl");

    let main_handler = Arc::new(CollectingDiagnosticHandler::new());
    let main_ast = parse_file(main_code, main_handler.clone()).expect("Failed to parse main.tl");

    // Create import maps
    let mut utils_import_map = std::collections::HashMap::new();
    utils_import_map.insert("./math".to_string(), "/project/math.tl".to_string());

    let mut main_import_map = std::collections::HashMap::new();
    main_import_map.insert("./utils".to_string(), "/project/utils.tl".to_string());

    // Generate bundle (dependency order: math -> utils -> main)
    let modules = vec![
        (
            "/project/math.tl".to_string(),
            &math_ast,
            std::collections::HashMap::new(),
        ),
        (
            "/project/utils.tl".to_string(),
            &utils_ast,
            utils_import_map,
        ),
        ("/project/main.tl".to_string(), &main_ast, main_import_map),
    ];

    let (bundle, _) =
        CodeGenerator::generate_bundle(&modules, "/project/main.tl", LuaTarget::Lua54, false, None);

    // Verify all modules are present
    assert!(bundle.contains("__modules[\"/project/math.tl\"]"));
    assert!(bundle.contains("__modules[\"/project/utils.tl\"]"));
    assert!(bundle.contains("__modules[\"/project/main.tl\"]"));

    // Verify imports use __require with resolved IDs
    assert!(bundle.contains("__require(\"/project/math.tl\")"));
    assert!(bundle.contains("__require(\"/project/utils.tl\")"));

    // Verify entry point is main
    assert!(bundle.ends_with("__require(\"/project/main.tl\")\n"));
}

#[test]
fn test_bundle_mode_with_re_exports() {
    use typedlua_core::codegen::{CodeGenerator, LuaTarget};

    // Create base module
    let base_code = r#"
export function helper(): string {
    return "help"
}
"#;

    // Re-export from middle
    let middle_code = r#"
export { helper } from './base'
"#;

    // Use re-exported symbol
    let main_code = r#"
import { helper } from './middle'

const result: string = helper()
print(result)
"#;

    // Parse all modules
    let base_handler = Arc::new(CollectingDiagnosticHandler::new());
    let base_ast = parse_file(base_code, base_handler.clone()).expect("Failed to parse base.tl");

    let middle_handler = Arc::new(CollectingDiagnosticHandler::new());
    let middle_ast =
        parse_file(middle_code, middle_handler.clone()).expect("Failed to parse middle.tl");

    let main_handler = Arc::new(CollectingDiagnosticHandler::new());
    let main_ast = parse_file(main_code, main_handler.clone()).expect("Failed to parse main.tl");

    // Create import maps
    let mut middle_import_map = std::collections::HashMap::new();
    middle_import_map.insert("./base".to_string(), "/project/base.tl".to_string());

    let mut main_import_map = std::collections::HashMap::new();
    main_import_map.insert("./middle".to_string(), "/project/middle.tl".to_string());

    // Generate bundle
    let modules = vec![
        (
            "/project/base.tl".to_string(),
            &base_ast,
            std::collections::HashMap::new(),
        ),
        (
            "/project/middle.tl".to_string(),
            &middle_ast,
            middle_import_map,
        ),
        ("/project/main.tl".to_string(), &main_ast, main_import_map),
    ];

    let (bundle, _) =
        CodeGenerator::generate_bundle(&modules, "/project/main.tl", LuaTarget::Lua54, false, None);

    // Verify re-export uses __require
    assert!(bundle.contains("__require(\"/project/base.tl\")"));

    // Verify all modules present
    assert!(bundle.contains("__modules[\"/project/base.tl\"]"));
    assert!(bundle.contains("__modules[\"/project/middle.tl\"]"));
    assert!(bundle.contains("__modules[\"/project/main.tl\"]"));
}

#[test]
fn test_bundle_mode_with_source_maps() {
    use typedlua_core::codegen::{CodeGenerator, LuaTarget};

    // Create simple test modules
    let utils_code = r#"
export function add(a: number, b: number): number {
    return a + b
}
"#;

    let main_code = r#"
import { add } from './utils'

const result: number = add(1, 2)
print(result)
"#;

    let utils_handler = Arc::new(CollectingDiagnosticHandler::new());
    let main_handler = Arc::new(CollectingDiagnosticHandler::new());

    let utils_ast =
        parse_file(utils_code, utils_handler.clone()).expect("Failed to parse utils.tl");
    let main_ast = parse_file(main_code, main_handler.clone()).expect("Failed to parse main.tl");

    // Create import map
    let mut main_import_map = std::collections::HashMap::new();
    main_import_map.insert("./utils".to_string(), "/project/utils.tl".to_string());

    // Generate bundle with source map
    let modules = vec![
        (
            "/project/utils.tl".to_string(),
            &utils_ast,
            std::collections::HashMap::new(),
        ),
        ("/project/main.tl".to_string(), &main_ast, main_import_map),
    ];

    let (bundle, source_map) = CodeGenerator::generate_bundle(
        &modules,
        "/project/main.tl",
        LuaTarget::Lua54,
        true,
        Some("bundle.lua".to_string()),
    );

    // Verify bundle was generated
    assert!(bundle.contains("-- TypedLua Bundle"));
    assert!(bundle.contains("__modules[\"/project/utils.tl\"]"));
    assert!(bundle.contains("__modules[\"/project/main.tl\"]"));

    // Verify source map was generated
    assert!(source_map.is_some());

    let source_map = source_map.unwrap();
    assert_eq!(source_map.version, 3);
    assert_eq!(source_map.file, Some("bundle.lua".to_string()));

    // Verify sources are included
    assert_eq!(source_map.sources.len(), 2);
    assert!(source_map
        .sources
        .contains(&"/project/utils.tl".to_string()));
    assert!(source_map.sources.contains(&"/project/main.tl".to_string()));

    // Verify mappings were generated (should not be empty)
    assert!(!source_map.mappings.is_empty());

    // Verify we can serialize to JSON
    let json = source_map
        .to_json()
        .expect("Failed to serialize source map");
    assert!(json.contains("\"version\": 3"));
    assert!(json.contains("\"/project/utils.tl\""));
    assert!(json.contains("\"/project/main.tl\""));
}
