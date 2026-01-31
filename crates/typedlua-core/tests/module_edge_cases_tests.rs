use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::{CompilerOptions, OptimizationLevel};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::fs::MockFileSystem;
use typedlua_core::module_resolver::{ModuleConfig, ModuleId, ModuleRegistry, ModuleResolver, LuaFilePolicy};
use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_and_check(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let options = CompilerOptions::default();

    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

/// Compile multiple modules with a shared module registry (for cross-module imports)
fn compile_modules_with_registry(modules: Vec<(&str, &str)>) -> Result<Vec<String>, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);
    let options = CompilerOptions::default();

    // Create mock filesystem with all modules
    let mut fs = MockFileSystem::new();
    for (path, _) in &modules {
        fs.add_file(path, "");
    }
    let fs = Arc::new(fs);

    // Create module resolver and registry
    let config = ModuleConfig {
        module_paths: vec![PathBuf::from("/")],
        lua_file_policy: LuaFilePolicy::RequireDeclaration,
    };
    let resolver = Arc::new(ModuleResolver::new(fs, config, PathBuf::from("/")));
    let registry = Arc::new(ModuleRegistry::new());

    // Store parsed programs
    let mut programs = Vec::new();

    // PASS 1: Parse all modules and register them
    for (path, source) in &modules {
        let module_id = ModuleId::new(PathBuf::from(path));

        // Lex and parse
        let mut lexer = Lexer::new(source, handler.clone(), &interner);
        let tokens = lexer
            .tokenize()
            .map_err(|e| format!("Lexing failed for {}: {:?}", path, e))?;

        let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
        let program = parser
            .parse()
            .map_err(|e| format!("Parsing failed for {}: {:?}", path, e))?;

        // Register the parsed module in the registry
        registry.register_parsed(
            module_id.clone(),
            Arc::new(program.clone()),
            Arc::new(Default::default()),
        );

        programs.push((path, module_id, program));
    }

    // PASS 2: Type check each module and immediately register exports
    // This ensures exports are available for subsequent modules
    let mut outputs = Vec::new();
    for (path, module_id, mut program) in programs {
        // Type check with module support
        let mut type_checker = TypeChecker::new_with_module_support(
            handler.clone(),
            &interner,
            &common_ids,
            registry.clone(),
            module_id.clone(),
            resolver.clone(),
        )
        .with_options(options.clone());

        type_checker
            .check_program(&mut program)
            .map_err(|e| format!("Type checking failed for {}: {}", path, e.message))?;

        // Extract and register exports IMMEDIATELY so next module can use them
        let exports = type_checker.extract_exports(&program);
        registry
            .register_exports(&module_id, exports)
            .map_err(|e| format!("Failed to register exports for {}: {:?}", path, e))?;

        // Generate code
        let mut codegen = CodeGenerator::new(interner.clone());
        outputs.push(codegen.generate(&mut program));
    }

    Ok(outputs)
}

// ============================================================================
// Circular Dependencies - Simple
// ============================================================================

#[test]
fn test_simple_circular_dependency() {
    let source_a = r#"
        namespace ModuleA;
        
        import { helperB } from "./moduleB"
        
        export function helperA(): string {
            return "A" .. helperB()
        }
        
        export const valueA = 1
    "#;

    let source_b = r#"
        namespace ModuleB;
        
        import { valueA } from "./moduleA"
        
        export function helperB(): string {
            return "B" .. tostring(valueA)
        }
        
        export const valueB = 2
    "#;

    let result_a = compile_and_check(source_a);
    let result_b = compile_and_check(source_b);

    assert!(
        result_a.is_ok(),
        "Module A in circular dependency should compile: {:?}",
        result_a.err()
    );
    assert!(
        result_b.is_ok(),
        "Module B in circular dependency should compile: {:?}",
        result_b.err()
    );
}

#[test]
fn test_circular_dependency_with_types() {
    let source_a = r#"
        namespace ModuleA;
        
        import type { ConfigB } from "./moduleB"
        
        export interface ConfigA {
            name: string
            nested: ConfigB | nil
        }
        
        export function createConfig(name: string): ConfigA {
            return { name, nested: nil }
        }
    "#;

    let source_b = r#"
        namespace ModuleB;
        
        import type { ConfigA } from "./moduleA"
        
        export interface ConfigB {
            value: number
            parent: ConfigA | nil
        }
        
        export function createConfig(value: number): ConfigB {
            return { value, parent: nil }
        }
    "#;

    let result_a = compile_and_check(source_a);
    let result_b = compile_and_check(source_b);

    assert!(
        result_a.is_ok(),
        "Module A with type circular dependency should compile: {:?}",
        result_a.err()
    );
    assert!(
        result_b.is_ok(),
        "Module B with type circular dependency should compile: {:?}",
        result_b.err()
    );
}

// ============================================================================
// Circular Dependencies - Complex
// ============================================================================

#[test]
fn test_complex_circular_dependency_a_b_c() {
    let source_a = r#"
        namespace ModuleA;
        
        import { processC } from "./moduleC"
        
        export function processA(input: string): string {
            return "A:" .. processC(input)
        }
        
        export const shared = "shared"
    "#;

    let source_b = r#"
        namespace ModuleB;
        
        import { shared } from "./moduleA"
        
        export function processB(input: string): string {
            return "B:" .. shared .. ":" .. input
        }
    "#;

    let source_c = r#"
        namespace ModuleC;
        
        import { processB } from "./moduleB"
        
        export function processC(input: string): string {
            return "C:" .. processB(input)
        }
    "#;

    let result_a = compile_and_check(source_a);
    let result_b = compile_and_check(source_b);
    let result_c = compile_and_check(source_c);

    assert!(
        result_a.is_ok(),
        "Module A in complex circular dependency should compile: {:?}",
        result_a.err()
    );
    assert!(
        result_b.is_ok(),
        "Module B in complex circular dependency should compile: {:?}",
        result_b.err()
    );
    assert!(
        result_c.is_ok(),
        "Module C in complex circular dependency should compile: {:?}",
        result_c.err()
    );
}

#[test]
fn test_circular_with_type_only_imports() {
    let source_a = r#"
        namespace ModuleA;
        
        import type { TypeB } from "./moduleB"
        
        export type TypeA = {
            ref: TypeB
        }
        
        export function useTypeB(val: TypeB): string {
            return "using B"
        }
    "#;

    let source_b = r#"
        namespace ModuleB;
        
        import type { TypeA } from "./moduleA"
        
        export type TypeB = {
            ref: TypeA
        }
        
        export function useTypeA(val: TypeA): string {
            return "using A"
        }
    "#;

    let result_a = compile_and_check(source_a);
    let result_b = compile_and_check(source_b);

    assert!(
        result_a.is_ok(),
        "Circular with type-only imports should compile for A: {:?}",
        result_a.err()
    );
    assert!(
        result_b.is_ok(),
        "Circular with type-only imports should compile for B: {:?}",
        result_b.err()
    );
}

// ============================================================================
// Dynamic Imports
// ============================================================================

#[test]
fn test_require_with_computed_path() {
    let source = r#"
        function loadModule(moduleName: string): unknown {
            return require("./modules/" .. moduleName)
        }
        
        const http = loadModule("http")
        const fs = loadModule("fs")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "require() with computed path should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_require_conditional() {
    let source = r#"
        namespace TestModule;

        function getPlatformModule(isServer: boolean): unknown {
            return require("./server")
        }

        const module = getPlatformModule(true)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Conditional require() should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_require_in_try_catch() {
    let source = r#"
        function safeRequire(modulePath: string): unknown | nil {
            try {
                return require(modulePath)
            } catch (e) {
                print(`Failed to load module: ${modulePath}`)
                return nil
            }
        }
        
        const optional = safeRequire("./optional")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "require() in try-catch should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Type-Only Imports
// ============================================================================

#[test]
fn test_type_only_import_basic() {
    let source_module = r#"
        namespace DataModule;

        export interface User {
            id: number
            name: string
        }

        export interface Config {
            host: string
            port: number
        }

        export function createUser(name: string): User {
            return { id: 1, name }
        }
    "#;

    let source_consumer = r#"
        namespace ConsumerModule;

        import type { User, Config } from "./dataModule"

        function processUser(user: User): string {
            return user.name
        }

        function setupConfig(config: Config): void {
            print(`${config.host}:${config.port}`)
        }
    "#;

    let modules = vec![
        ("/dataModule.tl", source_module),
        ("/consumerModule.tl", source_consumer),
    ];

    let result = compile_modules_with_registry(modules);
    assert!(
        result.is_ok(),
        "Type-only import should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_type_only_import_no_runtime_code() {
    let source = r#"
        namespace TestModule;
        
        import type { SomeType } from "./other"
        
        function useType(val: SomeType): void {
            print(val)
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Type-only import should not generate runtime code: {:?}",
        result.err()
    );

    let output = result.unwrap();
    assert!(
        !output.contains("require"),
        "Type-only import should not generate require call"
    );
}

// ============================================================================
// Default Exports
// ============================================================================

#[test]
fn test_default_export_class() {
    let source = r#"
        namespace MyModule;
        
        export default class Calculator {
            add(a: number, b: number): number {
                return a + b
            }
            
            subtract(a: number, b: number): number {
                return a - b
            }
        }
        
        export function helper(): void {
            print("helper")
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Default export class should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_default_export_function() {
    let source = r#"
        namespace UtilsModule;
        
        export default function greet(name: string): string {
            return `Hello, ${name}!`
        }
        
        export function farewell(name: string): string {
            return `Goodbye, ${name}!`
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Default export function should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_default_export_variable() {
    let source = r#"
        namespace ConfigModule;
        
        export default const config = {
            host: "localhost",
            port: 8080,
            debug: true
        }
        
        export function validateConfig(c: typeof config): boolean {
            return c.port > 0 && c.port < 65536
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Default export variable should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_mixed_imports() {
    let source_module = r#"
        namespace MixedModule;
        
        export default class MainClass {
            value: number
            constructor(v: number) {
                this.value = v
            }
        }
        
        export function helper1(): void {
            print("helper1")
        }
        
        export function helper2(): void {
            print("helper2")
        }
        
        export const constant = 42
    "#;

    let source_consumer = r#"
        namespace ConsumerModule;
        
        import MainClass, { helper1, helper2, constant } from "./mixedModule"
        
        const instance = new MainClass(10)
        helper1()
        helper2()
        const val = constant
    "#;

    let result_module = compile_and_check(source_module);
    let result_consumer = compile_and_check(source_consumer);

    assert!(
        result_module.is_ok(),
        "Module with mixed exports should compile: {:?}",
        result_module.err()
    );
    assert!(
        result_consumer.is_ok(),
        "Mixed import should compile: {:?}",
        result_consumer.err()
    );
}

#[test]
fn test_default_import_alias() {
    let source = r#"
        namespace ImportModule;
        
        import MyCalculator as Calc, { helper } from "./calculator"
        
        const calc = new Calc()
        helper()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Default import with alias should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Namespace Enforcement
// ============================================================================

#[test]
fn test_namespace_declaration() {
    let source = r#"
        namespace MyApp.Utils;
        
        export function formatDate(date: string): string {
            return date
        }
        
        export const VERSION = "1.0.0"
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Namespace declaration should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_namespace_nested() {
    let source = r#"
        namespace Company.Product.Module;
        
        export class Service {
            run(): void {
                print("running")
            }
        }
        
        export interface Config {
            enabled: boolean
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Nested namespace should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_namespace_import_resolution() {
    let source_a = r#"
        namespace App.Models;
        
        export class User {
            name: string
            constructor(name: string) {
                this.name = name
            }
        }
    "#;

    let source_b = r#"
        namespace App.Controllers;
        
        import { User } from "../models"
        
        export class UserController {
            getUser(id: number): User {
                return new User("Anonymous")
            }
        }
    "#;

    let result_a = compile_and_check(source_a);
    let result_b = compile_and_check(source_b);

    assert!(
        result_a.is_ok(),
        "Namespaced module A should compile: {:?}",
        result_a.err()
    );
    assert!(
        result_b.is_ok(),
        "Namespaced module B with import should compile: {:?}",
        result_b.err()
    );
}

// ============================================================================
// Re-exports
// ============================================================================

#[test]
fn test_re_export_all() {
    let source = r#"
        namespace ReExportModule;
        
        export * from "./utils"
        export * from "./helpers"
        
        export function localFunction(): void {
            print("local")
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Re-export all should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_re_export_named() {
    let source = r#"
        namespace ReExportModule;
        
        export { helper1, helper2 } from "./utils"
        export { config as default } from "./config"
        
        export function localFunction(): void {
            print("local")
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Re-export named should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Module Path Resolution
// ============================================================================

#[test]
fn test_relative_import_parent() {
    let source = r#"
        namespace Deep.Module;
        
        import { shared } from "../../shared"
        
        export function useShared(): string {
            return shared
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Relative import with parent path should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_relative_import_current() {
    let source = r#"
        namespace Current.Module;
        
        import { local } from "./local"
        import { sibling } from "../sibling"
        
        export function combine(): string {
            return local .. sibling
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Relative import with current/parent path should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Module-Level Statements
// ============================================================================

#[test]
fn test_module_level_code() {
    let source = r#"
        namespace InitModule;
        
        const initialized = initialize()
        
        function initialize(): boolean {
            print("Initializing module")
            return true
        }
        
        export function isInitialized(): boolean {
            return initialized
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Module-level code should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_module_level_side_effects() {
    let source = r#"
        namespace SideEffectModule;
        
        print("Module loading...")

        const config = loadConfig()

        function loadConfig(): unknown {
            return { debug: true }
        }

        export function getConfig(): unknown {
            return config
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Module-level side effects should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Import/Export Edge Cases
// ============================================================================

#[test]
fn test_export_type_alias() {
    let source = r#"
        namespace TypesModule;
        
        export type StringOrNumber = string | number
        export type Callback = (x: number) => void
        export type Optional<T> = T | nil

        export function useTypes(): void {
            const val: StringOrNumber = 42
            const cb: Callback = (x) => print(x)
            const n: Optional<string> = nil
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Export type alias should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_export_interface() {
    let source = r#"
        namespace InterfaceModule;
        
        export interface Drawable {
            draw(): void
            getBounds(): { x: number, y: number, w: number, h: number }
        }
        
        export interface Movable {
            move(dx: number, dy: number): void
        }
        
        export class Sprite implements Drawable, Movable {
            draw(): void {
                print("drawing")
            }
            
            getBounds(): { x: number, y: number, w: number, h: number } {
                return { x: 0, y: 0, w: 10, h: 10 }
            }
            
            move(dx: number, dy: number): void {
                print(`moving by ${dx}, ${dy}`)
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Export interface should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_export_enum() {
    let source = r#"
        namespace EnumModule;
        
        export enum Status {
            Pending,
            Active,
            Completed
        }
        
        export function getStatusName(s: Status): string {
            return s.name()
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Export enum should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_export_const_enum() {
    let source = r#"
        namespace ConstEnumModule;
        
        export const enum HttpStatus {
            OK = 200,
            NotFound = 404,
            Error = 500
        }
        
        export function checkStatus(code: HttpStatus): boolean {
            return code == HttpStatus.OK
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Export const enum should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Module Bundle Generation
// ============================================================================

#[test]
fn test_bundle_generation() {
    let source = r#"
        namespace BundleModule;
        
        import { helper } from "./helper"
        
        export function main(): void {
            helper()
            print("main executed")
        }
        
        if (require.main == module) {
            main()
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Bundle generation should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Ambient Module Declarations
// ============================================================================

#[test]
fn test_declare_module() {
    let source = r#"
        declare module "external-lib" {
            export function doSomething(): void
            export const version: string
        }
        
        import { doSomething, version } from "external-lib"
        
        namespace App;
        
        export function init(): void {
            print(version)
            doSomething()
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Declare module should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_declare_namespace() {
    let source = r#"
        declare namespace GlobalAPI {
            function fetchData(url: string): unknown
            function postData(url: string, data: unknown): unknown
        }

        namespace App;

        export function loadData(): void {
            const data = GlobalAPI.fetchData("/api/data")
            print(data)
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Declare namespace should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Global Augmentation
// ============================================================================

#[test]
fn test_global_augmentation() {
    let source = r#"
        declare global {
            interface String {
                reverse(): string
            }
        }
        
        namespace App;
        
        export function reverseString(s: string): string {
            return s.reverse()
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Global augmentation should compile: {:?}",
        result.err()
    );
}
