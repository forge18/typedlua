use rustc_hash::FxHashMap as HashMap;
use std::path::Path;
use std::sync::Arc;
use typedlua_core::codegen::tree_shaking::{ReachabilityAnalysis, ReachableSet};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_parser::ast::Program;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn create_program(
    source: &str,
    interner: &StringInterner,
    common: &typedlua_parser::string_interner::CommonIdentifiers,
) -> Program {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new(source, handler.clone(), interner);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, handler, interner, common);
    parser.parse().expect("Parsing failed")
}

fn create_modules(sources: &[(&str, &str)]) -> (HashMap<String, Program>, StringInterner) {
    let (interner, common) = StringInterner::new_with_common_identifiers();
    let mut modules = HashMap::default();

    for &(name, source) in sources {
        let program = create_program(source, &interner, &common);
        modules.insert(name.to_string(), program);
    }

    (modules, interner)
}

fn analyze_reachability(
    entry: &str,
    modules: &HashMap<String, Program>,
    interner: &StringInterner,
) -> ReachableSet {
    let entry_path = Path::new(entry);
    ReachabilityAnalysis::analyze(entry_path, modules, interner)
}

#[test]
fn test_single_module_bundle_no_shaking() {
    let sources = [(
        "main.lua",
        r#"
            export function add(a: number, b: number): number
                return a + b
            end
            export function multiply(a: number, b: number): number
                return a * b
            end
        "#,
    )];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("main.lua"));
    assert!(reachable.is_export_reachable("main.lua", "add"));
    assert!(reachable.is_export_reachable("main.lua", "multiply"));
}

#[test]
fn test_unused_function_removed() {
    let sources = [
        (
            "main.lua",
            r#"
            import { add } from "math"
            const result = add(1, 2)
        "#,
        ),
        (
            "math.lua",
            r#"
            export function add(a: number, b: number): number
                return a + b
            end
            export function subtract(a: number, b: number): number
                return a - b
            end
            export function multiply(a: number, b: number): number
                return a * b
            end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("main.lua"));
    assert!(reachable.is_module_reachable("math.lua"));
    assert!(reachable.is_export_reachable("math.lua", "add"));
    assert!(!reachable.is_export_reachable("math.lua", "subtract"));
    assert!(!reachable.is_export_reachable("math.lua", "multiply"));
}

#[test]
fn test_unused_entire_module_removed() {
    let sources = [
        (
            "main.lua",
            r#"
            import { greet } from "utils"
            const message = greet("World")
        "#,
        ),
        (
            "utils.lua",
            r#"
            export function greet(name: string): string
                return "Hello, " .. name
            end
            export function goodbye(name: string): string
                return "Goodbye, " .. name
            end
        "#,
        ),
        (
            "unused.lua",
            r#"
            export function useless(): number
                return 42
            end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("main.lua"));
    assert!(reachable.is_module_reachable("utils.lua"));
    assert!(!reachable.is_module_reachable("unused.lua"));
    assert!(reachable.is_export_reachable("utils.lua", "greet"));
    assert!(!reachable.is_export_reachable("utils.lua", "goodbye"));
}

#[test]
fn test_transitive_dependencies_preserved() {
    let sources = [
        (
            "main.lua",
            r#"
            import { compute } from "processor"
            const result = compute(5)
        "#,
        ),
        (
            "processor.lua",
            r#"
            import { calculate } from "calculator"
            export function compute(x: number): number
                return calculate(x)
            end
        "#,
        ),
        (
            "calculator.lua",
            r#"
            export function calculate(x: number): number
                return x * 2 + 1
            end
            export function unused(x: number): number
                return x * 3
            end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("main.lua"));
    assert!(reachable.is_module_reachable("processor.lua"));
    assert!(reachable.is_module_reachable("calculator.lua"));
    assert!(reachable.is_export_reachable("processor.lua", "compute"));
    assert!(reachable.is_export_reachable("calculator.lua", "calculate"));
    assert!(!reachable.is_export_reachable("calculator.lua", "unused"));
}

#[test]
fn test_entry_point_always_included() {
    let sources = [
        (
            "deeply/nested/main.lua",
            r#"
            export function run(): string
                return "Running..."
            end
        "#,
        ),
        (
            "helper.lua",
            r#"
            export function help(): number return 1 end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("deeply/nested/main.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("deeply/nested/main.lua"));
    assert!(!reachable.is_module_reachable("helper.lua"));
}

#[test]
fn test_multiple_unused_exports_filtered() {
    let sources = [
        (
            "main.lua",
            r#"
            import { useOne } from "lib"
            const x = useOne()
        "#,
        ),
        (
            "lib.lua",
            r#"
            export function useOne(): number return 1 end
            export function useTwo(): number return 2 end
            export function useThree(): number return 3 end
            export function useFour(): number return 4 end
            export function useFive(): number return 5 end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    let lib_exports = reachable.get_reachable_exports("lib.lua").unwrap();
    assert!(lib_exports.contains("useOne"));
    assert_eq!(lib_exports.len(), 1);
}

#[test]
fn test_namespace_import_all_used() {
    let sources = [
        (
            "main.lua",
            r#"
            import * as math from "math"
            const a = math.add(1, 2)
            const b = math.subtract(5, 3)
        "#,
        ),
        (
            "math.lua",
            r#"
            export function add(a: number, b: number): number return a + b end
            export function subtract(a: number, b: number): number return a - b end
            export function multiply(a: number, b: number): number return a * b end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("main.lua"));
    assert!(reachable.is_module_reachable("math.lua"));
}

#[test]
fn test_partial_namespace_import() {
    let sources = [
        (
            "main.lua",
            r#"
            import * as math from "math"
            const a = math.add(1, 2)
        "#,
        ),
        (
            "math.lua",
            r#"
            export function add(a: number, b: number): number return a + b end
            export function subtract(a: number, b: number): number return a - b end
            export function multiply(a: number, b: number): number return a * b end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("main.lua"));
    assert!(reachable.is_module_reachable("math.lua"));
}

#[test]
fn test_default_export_included() {
    let sources = [
        (
            "main.lua",
            r#"
            import default from "module"
            const x = default()
        "#,
        ),
        (
            "module.lua",
            r#"
            export default function(): number return 42 end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("main.lua"));
    assert!(reachable.is_module_reachable("module.lua"));
}

#[test]
fn test_complex_dependency_chain() {
    let sources = [
        (
            "app.lua",
            r#"
            import { start } from "bootstrap"
            start()
        "#,
        ),
        (
            "bootstrap.lua",
            r#"
            import { createApp } from "core"
            import { loadConfig } from "config"
            export function start()
                local app = createApp()
                loadConfig(app)
            end
        "#,
        ),
        (
            "core.lua",
            r#"
            export function createApp()
                return {}
            end
            export function destroyApp(app) end
        "#,
        ),
        (
            "config.lua",
            r#"
            export function loadConfig(app)
                app.settings = {}
            end
            export function saveConfig(app) end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("app.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("app.lua"));
    assert!(reachable.is_module_reachable("bootstrap.lua"));
    assert!(reachable.is_module_reachable("core.lua"));
    assert!(reachable.is_module_reachable("config.lua"));

    let core_exports = reachable.get_reachable_exports("core.lua").unwrap();
    assert!(core_exports.contains("createApp"));
    assert!(!core_exports.contains("destroyApp"));

    let config_exports = reachable.get_reachable_exports("config.lua").unwrap();
    assert!(config_exports.contains("loadConfig"));
    assert!(!config_exports.contains("saveConfig"));
}

#[test]
fn test_re_exports_tracking() {
    let sources = [
        (
            "main.lua",
            r#"
            import { util } from "reporter"
            util.log("test")
        "#,
        ),
        (
            "reporter.lua",
            r#"
            import { log } from "utils"
            import { format } from "formatter"
            export const util = { log = log }
            export { format as pretty }
        "#,
        ),
        (
            "utils.lua",
            r#"
            export function log(msg: string) end
        "#,
        ),
        (
            "formatter.lua",
            r#"
            export function format(x: any): string return tostring(x) end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("main.lua"));
    assert!(reachable.is_module_reachable("reporter.lua"));
    assert!(reachable.is_module_reachable("utils.lua"));
    assert!(reachable.is_module_reachable("formatter.lua"));
}

#[test]
fn test_all_exports_used() {
    let sources = [
        (
            "main.lua",
            r#"
            import { a, b, c } from "lib"
            const x = a()
            const y = b()
            const z = c()
        "#,
        ),
        (
            "lib.lua",
            r#"
            export function a(): number return 1 end
            export function b(): number return 2 end
            export function c(): number return 3 end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    let lib_exports = reachable.get_reachable_exports("lib.lua").unwrap();
    assert!(lib_exports.contains("a"));
    assert!(lib_exports.contains("b"));
    assert!(lib_exports.contains("c"));
    assert_eq!(lib_exports.len(), 3);
}

#[test]
fn test_mixed_import_types() {
    let sources = [
        (
            "main.lua",
            r#"
            import default from "default_mod"
            import { used } from "named_mod"
            import * as ns from "namespace_mod"
            const x = used()
            const y = ns.ns_func()
        "#,
        ),
        (
            "default_mod.lua",
            r#"
            export default function(): number return 1 end
        "#,
        ),
        (
            "named_mod.lua",
            r#"
            export function used(): number return 2 end
            export function unused(): number return 3 end
        "#,
        ),
        (
            "namespace_mod.lua",
            r#"
            export function ns_func(): number return 4 end
            export function other_func(): number return 5 end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("main.lua"));
    assert!(reachable.is_module_reachable("default_mod.lua"));
    assert!(reachable.is_module_reachable("named_mod.lua"));
    assert!(reachable.is_module_reachable("namespace_mod.lua"));

    let named_exports = reachable.get_reachable_exports("named_mod.lua").unwrap();
    assert!(named_exports.contains("used"));
    assert!(!named_exports.contains("unused"));

    assert!(reachable.is_module_reachable("namespace_mod.lua"));
}

#[test]
fn test_self_referential_module() {
    let sources = [
        (
            "main.lua",
            r#"
            import { Helper } from "helper"
            const h = Helper.new()
        "#,
        ),
        (
            "helper.lua",
            r#"
            export class Helper
                public function new(): Helper
                    return self
                end
            end
        "#,
        ),
    ];

    let (modules, interner) = create_modules(&sources);
    let reachable = analyze_reachability("main.lua", &modules, &interner);

    assert!(reachable.is_module_reachable("main.lua"));
    assert!(reachable.is_module_reachable("helper.lua"));
}
