// Keep core-specific modules
pub mod cache;
pub mod codegen;
pub mod di;
pub mod optimizer;
pub mod type_checker;

// Re-export everything from external typechecker
pub use typedlua_typechecker::{
    // Utility types
    apply_utility_type,
    // Generics
    build_substitutions,
    check_type_constraints,
    // Modules (full re-export)
    config,
    diagnostics,
    errors,
    evaluate_conditional_type,
    evaluate_keyof,
    evaluate_mapped_type,
    evaluate_template_literal_type,

    fs,
    infer_type_arguments,
    instantiate_function_declaration,
    instantiate_type,

    module_resolver,
    narrow_type_from_condition,

    // Narrowing
    NarrowingContext,
    Scope,
    SerializableSymbol,
    SerializableSymbolTable,

    // Symbol table
    Symbol,
    SymbolKind,
    SymbolTable,
    TypeCheckError,
    // Main types
    TypeChecker,
    TypeCheckerState,

    TypeCompatibility,

    // Type system
    TypeEnvironment,
};

// Re-export common diagnostics for backward compatibility
pub use typedlua_typechecker::diagnostics::{
    CollectingDiagnosticHandler, Diagnostic, DiagnosticHandler, DiagnosticLevel,
};
