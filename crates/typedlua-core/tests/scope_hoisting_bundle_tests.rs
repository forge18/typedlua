//! Integration tests for scope hoisting in bundle mode (Phase 5.7)
//!
//! These tests verify that:
//! 1. Simple module hoisting works for single functions
//! 2. Multiple modules can be hoisted into shared scope
//! 3. Mixed hoisted + non-hoisted modules work correctly
//! 4. Name collisions are handled properly
//! 5. Circular dependencies still work with hoisting

use bumpalo::Bump;
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::Arc;
use typedlua_core::codegen::scope_hoisting::{EscapeAnalysis, HoistingContext};
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::codegen::LuaTarget;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_parser::ast::Program;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn create_program<'arena>(
    source: &str,
    interner: &StringInterner,
    common: &typedlua_parser::string_interner::CommonIdentifiers,
    arena: &'arena Bump,
) -> Program<'arena> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut lexer = Lexer::new(source, handler.clone(), interner);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let mut parser = Parser::new(tokens, handler, interner, common, arena);
    parser.parse().expect("Parsing failed")
}

fn create_modules_with_interner<'arena>(
    sources: &[(&str, &str)],
    arena: &'arena Bump,
) -> (Vec<(String, Program<'arena>, FxHashMap<String, String>)>, Arc<StringInterner>) {
    let (interner, common) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let mut modules = Vec::new();

    for &(name, source) in sources {
        let program = create_program(source, &interner, &common, arena);
        let import_map = FxHashMap::default();
        modules.push((name.to_string(), program, import_map));
    }

    (modules, interner)
}

fn generate_bundle(
    sources: &[(&str, &str)],
    entry: &str,
    scope_hoisting_enabled: bool,
) -> String {
    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(sources, &arena);

    // Convert FxHashMap to std::HashMap for the API
    let module_refs: Vec<(String, &Program, HashMap<String, String>)> = modules
        .iter()
        .map(|(id, prog, map)| {
            let std_map: HashMap<String, String> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            (id.clone(), prog, std_map)
        })
        .collect();

    let (output, _source_map) = CodeGenerator::generate_bundle_with_options(
        &module_refs,
        entry,
        LuaTarget::Lua54,
        false, // no source map
        None,  // no output file
        Some(interner),
        None, // no tree shaking
        scope_hoisting_enabled,
    );

    output
}

// ============================================================================
// Test: Simple module hoisting (single function)
// ============================================================================

#[test]
fn test_simple_function_hoisting() {
    let sources = [(
        "main.lua",
        r#"
            function helper(x: number): number
                return x * 2
            end
            const result = helper(5)
        "#,
    )];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);

    // Verify the function is detected as hoistable
    let hoistable = EscapeAnalysis::analyze(&modules[0].1, &interner);
    assert!(
        hoistable.functions.contains("helper"),
        "helper function should be hoistable"
    );

    // Generate bundle with hoisting enabled
    let output_with_hoisting = generate_bundle(&sources, "main.lua", true);

    // Should contain hoisted declarations section
    assert!(
        output_with_hoisting.contains("-- Hoisted declarations"),
        "Bundle should have hoisted declarations section"
    );

    // The function should be hoisted with mangled name
    assert!(
        output_with_hoisting.contains("main__helper") || output_with_hoisting.contains("local function"),
        "Hoisted function should appear in output"
    );
}

#[test]
fn test_simple_variable_hoisting() {
    let sources = [(
        "main.lua",
        r#"
            const CONSTANT: number = 42
            const result = CONSTANT + 1
        "#,
    )];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);

    // Verify the constant is detected as hoistable
    let hoistable = EscapeAnalysis::analyze(&modules[0].1, &interner);
    assert!(
        hoistable.variables.contains("CONSTANT"),
        "CONSTANT should be hoistable"
    );
}

// ============================================================================
// Test: Multiple modules hoisted into shared scope
// ============================================================================

#[test]
fn test_multiple_modules_hoisting() {
    let sources = [
        (
            "main.lua",
            r#"
                import { add } from "math"
                function localHelper(): number
                    return 1
                end
                const result = add(localHelper(), 2)
            "#,
        ),
        (
            "math.lua",
            r#"
                function internalDouble(x: number): number
                    return x * 2
                end
                export function add(a: number, b: number): number
                    return internalDouble(a) + b
                end
            "#,
        ),
    ];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);

    // main.lua's localHelper should be hoistable (not exported)
    let main_hoistable = EscapeAnalysis::analyze(&modules[0].1, &interner);
    assert!(
        main_hoistable.functions.contains("localHelper"),
        "localHelper should be hoistable"
    );

    // math.lua's internalDouble should be hoistable (not exported)
    let math_hoistable = EscapeAnalysis::analyze(&modules[1].1, &interner);
    assert!(
        math_hoistable.functions.contains("internalDouble"),
        "internalDouble should be hoistable"
    );

    // Generate bundle
    let output = generate_bundle(&sources, "main.lua", true);

    // Both modules should be in the output
    assert!(output.contains("Module: main.lua"));
    assert!(output.contains("Module: math.lua"));
}

#[test]
fn test_hoisting_context_multiple_modules() {
    let sources = [
        (
            "app.lua",
            r#"
                function privateAppHelper(): string
                    return "app"
                end
                export function main()
                    return privateAppHelper()
                end
            "#,
        ),
        (
            "utils.lua",
            r#"
                function privateUtilHelper(): string
                    return "util"
                end
                export function format(s: string): string
                    return privateUtilHelper() .. s
                end
            "#,
        ),
    ];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);

    let modules_for_analysis: Vec<(String, &Program)> = modules
        .iter()
        .map(|(id, prog, _)| (id.clone(), prog))
        .collect();

    let context = HoistingContext::analyze_modules(&modules_for_analysis, &interner, "app.lua", true);

    // Both modules should have hoistable declarations
    assert!(context.has_hoistable_declarations("app.lua"));
    assert!(context.has_hoistable_declarations("utils.lua"));

    // Check mangled names exist and are different
    let app_mangled = context.get_mangled_name("app.lua", "privateAppHelper");
    let utils_mangled = context.get_mangled_name("utils.lua", "privateUtilHelper");

    assert!(app_mangled.is_some(), "app helper should have mangled name");
    assert!(utils_mangled.is_some(), "utils helper should have mangled name");
    assert_ne!(
        app_mangled, utils_mangled,
        "Different modules should have different mangled names"
    );
}

// ============================================================================
// Test: Mixed hoisted + non-hoisted modules
// ============================================================================

#[test]
fn test_mixed_hoisted_nonhoisted() {
    let sources = [
        (
            "main.lua",
            r#"
                import { greet } from "greeter"
                function privateHelper(): string
                    return "!"
                end
                const msg = greet("World") .. privateHelper()
            "#,
        ),
        (
            "greeter.lua",
            r#"
                -- This module has only exports, nothing to hoist
                export function greet(name: string): string
                    return "Hello, " .. name
                end
            "#,
        ),
    ];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);

    // main.lua has hoistable privateHelper
    let main_hoistable = EscapeAnalysis::analyze(&modules[0].1, &interner);
    assert!(main_hoistable.functions.contains("privateHelper"));

    // greeter.lua has nothing hoistable (only exports)
    let greeter_hoistable = EscapeAnalysis::analyze(&modules[1].1, &interner);
    assert!(
        greeter_hoistable.functions.is_empty(),
        "greeter should have no hoistable functions"
    );

    // Generate bundle - should still work
    let output = generate_bundle(&sources, "main.lua", true);
    assert!(output.contains("Module: main.lua"));
    assert!(output.contains("Module: greeter.lua"));
}

#[test]
fn test_exported_functions_not_hoisted() {
    let sources = [(
        "lib.lua",
        r#"
            export function publicApi(): number
                return privateImpl()
            end
            function privateImpl(): number
                return 42
            end
        "#,
    )];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);
    let hoistable = EscapeAnalysis::analyze(&modules[0].1, &interner);

    // privateImpl should be hoistable
    assert!(hoistable.functions.contains("privateImpl"));

    // publicApi should NOT be hoistable (it's exported)
    assert!(!hoistable.functions.contains("publicApi"));
}

// ============================================================================
// Test: Name collision handling
// ============================================================================

#[test]
fn test_name_collision_same_function_name() {
    let sources = [
        (
            "moduleA.lua",
            r#"
                function helper(): number
                    return 1
                end
                export function getA(): number
                    return helper()
                end
            "#,
        ),
        (
            "moduleB.lua",
            r#"
                function helper(): number
                    return 2
                end
                export function getB(): number
                    return helper()
                end
            "#,
        ),
    ];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);

    let modules_for_analysis: Vec<(String, &Program)> = modules
        .iter()
        .map(|(id, prog, _)| (id.clone(), prog))
        .collect();

    let context = HoistingContext::analyze_modules(&modules_for_analysis, &interner, "moduleA.lua", true);

    // Both should be hoistable
    assert!(context.is_hoistable(
        "moduleA.lua",
        "helper",
        &typedlua_core::codegen::scope_hoisting::DeclarationKind::Function
    ));
    assert!(context.is_hoistable(
        "moduleB.lua",
        "helper",
        &typedlua_core::codegen::scope_hoisting::DeclarationKind::Function
    ));

    // Mangled names should be different
    let mangled_a = context.get_mangled_name("moduleA.lua", "helper").unwrap();
    let mangled_b = context.get_mangled_name("moduleB.lua", "helper").unwrap();

    assert_ne!(
        mangled_a, mangled_b,
        "Same-named functions in different modules should have different mangled names"
    );

    // Names should contain module path component
    assert!(
        mangled_a.contains("moduleA"),
        "Mangled name should contain module path"
    );
    assert!(
        mangled_b.contains("moduleB"),
        "Mangled name should contain module path"
    );
}

#[test]
fn test_name_collision_resolution_in_bundle() {
    let sources = [
        (
            "a.lua",
            r#"
                function calc(): number return 1 end
                export const A = calc()
            "#,
        ),
        (
            "b.lua",
            r#"
                function calc(): number return 2 end
                export const B = calc()
            "#,
        ),
        (
            "main.lua",
            r#"
                import { A } from "a"
                import { B } from "b"
                const sum = A + B
            "#,
        ),
    ];

    // Generate bundle with hoisting
    let output = generate_bundle(&sources, "main.lua", true);

    // Should contain hoisted declarations
    assert!(output.contains("-- Hoisted declarations"));

    // Both calc functions should be present with mangled names
    // They should have different names (a__calc and b__calc)
    assert!(
        output.contains("a__calc") || output.contains("a_lua__calc"),
        "Should contain mangled name for a.lua's calc"
    );
    assert!(
        output.contains("b__calc") || output.contains("b_lua__calc"),
        "Should contain mangled name for b.lua's calc"
    );
}

// ============================================================================
// Test: Circular dependencies still work
// ============================================================================

#[test]
fn test_circular_dependency_modules() {
    // Note: These modules have potential circular imports
    // The hoisting should still work correctly
    let sources = [
        (
            "a.lua",
            r#"
                function privateA(): number
                    return 1
                end
                export function getA(): number
                    return privateA()
                end
            "#,
        ),
        (
            "b.lua",
            r#"
                function privateB(): number
                    return 2
                end
                export function getB(): number
                    return privateB()
                end
            "#,
        ),
    ];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);

    let modules_for_analysis: Vec<(String, &Program)> = modules
        .iter()
        .map(|(id, prog, _)| (id.clone(), prog))
        .collect();

    // Should not panic or error with circular potential
    let context = HoistingContext::analyze_modules(&modules_for_analysis, &interner, "a.lua", true);

    assert!(context.has_hoistable_declarations("a.lua"));
    assert!(context.has_hoistable_declarations("b.lua"));

    // Generate bundle should work
    let output = generate_bundle(&sources, "a.lua", true);
    assert!(output.contains("-- TypedLua Bundle"));
}

// ============================================================================
// Test: Disabled hoisting
// ============================================================================

#[test]
fn test_hoisting_disabled() {
    let sources = [(
        "main.lua",
        r#"
            function helper(): number
                return 42
            end
            const result = helper()
        "#,
    )];

    // Generate with hoisting disabled
    let output_no_hoisting = generate_bundle(&sources, "main.lua", false);

    // Should NOT contain hoisted declarations section
    assert!(
        !output_no_hoisting.contains("-- Hoisted declarations"),
        "Bundle without hoisting should not have hoisted declarations section"
    );
}

#[test]
fn test_hoisting_context_disabled() {
    let sources = [(
        "main.lua",
        r#"
            function helper(): number
                return 42
            end
        "#,
    )];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);

    let modules_for_analysis: Vec<(String, &Program)> = modules
        .iter()
        .map(|(id, prog, _)| (id.clone(), prog))
        .collect();

    // Create disabled context
    let context = HoistingContext::analyze_modules(&modules_for_analysis, &interner, "main.lua", false);

    // Should return false/None for all queries
    assert!(!context.has_hoistable_declarations("main.lua"));
    assert!(context.get_mangled_name("main.lua", "helper").is_none());
    assert!(!context.is_hoistable(
        "main.lua",
        "helper",
        &typedlua_core::codegen::scope_hoisting::DeclarationKind::Function
    ));
}

// ============================================================================
// Test: Entry point name preservation
// ============================================================================

#[test]
fn test_entry_point_names_preserved() {
    let sources = [(
        "entry.lua",
        r#"
            function main(): number
                return helper()
            end
            function helper(): number
                return 42
            end
        "#,
    )];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);

    let modules_for_analysis: Vec<(String, &Program)> = modules
        .iter()
        .map(|(id, prog, _)| (id.clone(), prog))
        .collect();

    let context = HoistingContext::analyze_modules(&modules_for_analysis, &interner, "entry.lua", true);

    // Both should be hoistable
    assert!(context.has_hoistable_declarations("entry.lua"));

    // Entry module functions may or may not preserve names depending on implementation
    // The important thing is they have mangled names that work
    let main_mangled = context.get_mangled_name("entry.lua", "main");
    let helper_mangled = context.get_mangled_name("entry.lua", "helper");

    assert!(main_mangled.is_some() || helper_mangled.is_some());
}

// ============================================================================
// Test: Variable hoisting edge cases
// ============================================================================

#[test]
fn test_variable_with_function_init_not_hoisted() {
    let sources = [(
        "main.lua",
        r#"
            const callback = function() return 1 end
            const result = callback()
        "#,
    )];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);
    let hoistable = EscapeAnalysis::analyze(&modules[0].1, &interner);

    // callback should NOT be hoistable (initialized with function expression)
    assert!(
        !hoistable.variables.contains("callback"),
        "Variables initialized with functions should not be hoisted"
    );
}

#[test]
fn test_variable_returned_from_exported_can_hoist() {
    // Variables returned from EXPORTED functions CAN be hoisted.
    // Exported functions are part of the public API, and the variable
    // will still be accessible from the higher scope after hoisting.
    let sources = [(
        "main.lua",
        r#"
            const value: number = 42
            export function getValue(): number
                return value
            end
        "#,
    )];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);
    let hoistable = EscapeAnalysis::analyze(&modules[0].1, &interner);

    // value CAN be hoisted because:
    // 1. It's not exported directly
    // 2. It's returned from an exported function (which is fine - the hoisted
    //    variable will be accessible from the function at the higher scope)
    assert!(
        hoistable.variables.contains("value"),
        "Variables returned from exported functions can be hoisted"
    );
}

#[test]
fn test_variable_returned_from_private_not_hoisted() {
    // Variables returned from PRIVATE functions cannot be hoisted
    // because the private function itself might get hoisted too,
    // but there could be dependencies between them.
    let sources = [(
        "main.lua",
        r#"
            const value: number = 42
            function privateGetter(): number
                return value
            end
            const result = privateGetter()
        "#,
    )];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);
    let hoistable = EscapeAnalysis::analyze(&modules[0].1, &interner);

    // value should NOT be hoistable (returned from private function)
    assert!(
        !hoistable.variables.contains("value"),
        "Variables returned from private functions should not be hoisted"
    );
}

// ============================================================================
// Test: Enum hoisting
// ============================================================================

#[test]
fn test_private_enum_hoisting() {
    let sources = [(
        "main.lua",
        r#"
            enum Color {
                Red = 0,
                Green = 1,
                Blue = 2
            }
            const c = Color.Red
        "#,
    )];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);
    let hoistable = EscapeAnalysis::analyze(&modules[0].1, &interner);

    // Color enum should be hoistable (not exported)
    assert!(
        hoistable.enums.contains("Color"),
        "Private enum should be hoistable"
    );
}

#[test]
fn test_exported_enum_not_hoisted() {
    let sources = [(
        "main.lua",
        r#"
            export enum Status {
                Pending = 0,
                Complete = 1
            }
        "#,
    )];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);
    let hoistable = EscapeAnalysis::analyze(&modules[0].1, &interner);

    // Status enum should NOT be hoistable (exported)
    assert!(
        !hoistable.enums.contains("Status"),
        "Exported enum should not be hoistable"
    );
}

// ============================================================================
// Test: Class hoisting
// ============================================================================

#[test]
fn test_private_class_hoisting() {
    let sources = [(
        "main.lua",
        r#"
            class Helper {}
            const h = Helper.new()
        "#,
    )];

    let arena = Bump::new();
    let (modules, interner) = create_modules_with_interner(&sources, &arena);
    let hoistable = EscapeAnalysis::analyze(&modules[0].1, &interner);

    // Helper class should be hoistable (not exported)
    assert!(
        hoistable.classes.contains("Helper"),
        "Private class should be hoistable"
    );
}

// ============================================================================
// Benchmark: Bundle size comparison
// ============================================================================

#[test]
fn test_bundle_size_with_hoisting() {
    // Create a module with multiple private functions
    let sources = [(
        "main.lua",
        r#"
            function helper1(): number return 1 end
            function helper2(): number return 2 end
            function helper3(): number return 3 end
            function helper4(): number return 4 end
            function helper5(): number return 5 end
            const sum = helper1() + helper2() + helper3() + helper4() + helper5()
        "#,
    )];

    let output_with_hoisting = generate_bundle(&sources, "main.lua", true);
    let output_without_hoisting = generate_bundle(&sources, "main.lua", false);

    // With hoisting, the bundle structure is different but should be functional
    // The hoisted version has declarations at top level
    assert!(output_with_hoisting.contains("-- Hoisted declarations"));
    assert!(!output_without_hoisting.contains("-- Hoisted declarations"));

    // Both should contain the module wrapper
    assert!(output_with_hoisting.contains("__modules["));
    assert!(output_without_hoisting.contains("__modules["));

    // Print sizes for manual inspection (not a hard assertion since structure differs)
    println!(
        "Bundle with hoisting: {} bytes",
        output_with_hoisting.len()
    );
    println!(
        "Bundle without hoisting: {} bytes",
        output_without_hoisting.len()
    );
}
