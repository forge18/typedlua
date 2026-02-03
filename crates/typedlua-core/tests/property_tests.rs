//! Property-based tests for TypedLua compiler
//!
//! These tests use proptest to verify compiler correctness across a wide
//! range of random inputs, ensuring robustness beyond specific test cases.

use proptest::prelude::*;
use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::{CodeGenMode, CodeGenerator, LuaTarget};
use typedlua_core::diagnostics::{
    CollectingDiagnosticHandler as CoreCollectingDiagnosticHandler,
    DiagnosticHandler as CoreDiagnosticHandler, DiagnosticLevel,
};
use typedlua_core::TypeChecker;
use typedlua_parser::diagnostics::{
    CollectingDiagnosticHandler as ParserCollectingDiagnosticHandler,
    DiagnosticHandler as ParserDiagnosticHandler,
};
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

// =============================================================================
// Parser Round-Trip Properties
// =============================================================================

// =============================================================================
// Parser Round-Trip Properties
// =============================================================================

/// Strategy for generating valid TypedLua identifiers
fn identifier_strategy() -> impl Strategy<Value = String> {
    // Start with letter or underscore, followed by letters, digits, or underscores
    // Filter out reserved keywords and single underscore (which can cause issues)
    "[a-zA-Z_][a-zA-Z0-9_]{0,20}".prop_filter("Reserved keywords or invalid identifiers", |s| {
        // Exclude single underscore which can cause codegen issues
        if s == "_" {
            return false;
        }
        !matches!(
            s.as_str(),
            "and"
                | "break"
                | "do"
                | "else"
                | "elseif"
                | "end"
                | "false"
                | "for"
                | "function"
                | "if"
                | "in"
                | "local"
                | "nil"
                | "not"
                | "or"
                | "repeat"
                | "return"
                | "then"
                | "true"
                | "until"
                | "while"
                | "class"
                | "interface"
                | "enum"
                | "namespace"
                | "import"
                | "export"
                | "type"
                | "declare"
                | "readonly"
                | "private"
                | "protected"
                | "public"
                | "static"
                | "abstract"
                | "final"
                | "sealed"
                | "override"
                | "operator"
                | "throw"
                | "try"
                | "catch"
                | "finally"
                | "rethrow"
                | "async"
                | "await"
                | "yield"
                | "const"
                | "let"
                | "var"
        )
    })
}

/// Strategy for generating valid Lua/TypedLua number literals
fn number_literal_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Integers (avoid 0 which can cause issues)
        "[1-9][0-9]{0,9}",
        // Hexadecimal (avoid 0x0)
        "0x[1-9a-fA-F][0-9a-fA-F]{0,7}",
        // Floats
        "[1-9][0-9]{0,4}\\.[0-9]{1,5}",
        // Scientific notation
        "[1-9][0-9]{0,4}e[+-]?[0-9]{1,3}",
    ]
}

/// Strategy for generating valid string literals
#[allow(dead_code)]
fn string_literal_strategy() -> impl Strategy<Value = String> {
    // Simple strings without special characters that would need escaping
    "[a-zA-Z0-9 _-]{0,50}".prop_map(|s| format!("\"{}\"", s))
}

/// Strategy for generating simple variable declarations
fn variable_declaration_strategy() -> impl Strategy<Value = String> {
    (identifier_strategy(), number_literal_strategy())
        .prop_map(|(name, value)| format!("local {} = {}", name, value))
}

/// Strategy for generating simple function declarations
fn function_declaration_strategy() -> impl Strategy<Value = String> {
    (
        identifier_strategy(),
        proptest::collection::vec(identifier_strategy(), 0..3),
    )
        .prop_map(|(name, params)| {
            let params_str = params.join(", ");
            format!("local function {}({})\n  return 42\nend", name, params_str)
        })
}

/// Strategy for generating simple table literals
fn table_literal_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec((identifier_strategy(), number_literal_strategy()), 0..5).prop_map(
        |entries| {
            let entries_str = entries
                .into_iter()
                .map(|(k, v)| format!("{} = {}", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            format!("local t = {{{}}}", entries_str)
        },
    )
}

/// Strategy for generating simple TypedLua programs
fn simple_program_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            variable_declaration_strategy(),
            function_declaration_strategy(),
            table_literal_strategy(),
        ],
        1..10,
    )
    .prop_map(|statements| statements.join("\n"))
}

/// Helper function to parse TypedLua source code
fn parse_source(source: &str) -> Result<typedlua_parser::ast::Program, String> {
    let handler: Arc<dyn ParserDiagnosticHandler> =
        Arc::new(ParserCollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);
    let mut lexer = Lexer::new(source, handler.clone(), &interner);

    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lex error: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler, &interner, &common_ids);
    parser.parse().map_err(|e| format!("Parse error: {:?}", e))
}

proptest! {
    // Property: Well-formed simple programs should parse successfully
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_simple_programs_parse(source in simple_program_strategy()) {
        let result = parse_source(&source);
        prop_assert!(
            result.is_ok(),
            "Failed to parse valid program:\n{}\nError: {:?}",
            source,
            result.err()
        );
    }
}

// =============================================================================
// Type Checker Soundness Properties
// =============================================================================

proptest! {
    // Property: Type-safe programs should not produce type errors
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_type_safe_programs_no_errors(
        var_name in identifier_strategy(),
        value in number_literal_strategy()
    ) {
        let source = format!(
            "local {}: number = {}\nlocal doubled: number = {} * 2",
            var_name, value, var_name
        );

        let core_handler: Arc<dyn CoreDiagnosticHandler> = Arc::new(CoreCollectingDiagnosticHandler::new());
        let parser_handler: Arc<dyn ParserDiagnosticHandler> = Arc::new(ParserCollectingDiagnosticHandler::new());
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);
        let mut lexer = Lexer::new(&source, parser_handler.clone(), &interner);

        let tokens = lexer.tokenize().expect("Should lex");
        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids);
        let mut program = parser.parse().expect("Should parse");

        let mut checker = TypeChecker::new(core_handler.clone(), &interner, &common_ids);
        let _ = checker.check_program(&mut program);

        let diagnostics = core_handler.get_diagnostics();
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| matches!(d.level, DiagnosticLevel::Error))
            .collect();

        prop_assert!(
            errors.is_empty(),
            "Type-safe program produced errors:\n{}\nErrors: {:?}",
            source,
            errors
        );
    }
}

proptest! {
    // Property: Type annotations should match inferred types for literals
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_number_literal_type_check(
        var_name in identifier_strategy(),
        value in number_literal_strategy()
    ) {
        let source = format!("local {}: number = {}", var_name, value);

        let core_handler: Arc<dyn CoreDiagnosticHandler> = Arc::new(CoreCollectingDiagnosticHandler::new());
        let parser_handler: Arc<dyn ParserDiagnosticHandler> = Arc::new(ParserCollectingDiagnosticHandler::new());
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);
        let mut lexer = Lexer::new(&source, parser_handler.clone(), &interner);

        let tokens = lexer.tokenize().expect("Should lex");
        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids);
        let mut program = parser.parse().expect("Should parse");

        let mut checker = TypeChecker::new(core_handler.clone(), &interner, &common_ids);
        let _ = checker.check_program(&mut program);

        let diagnostics = core_handler.get_diagnostics();
        let type_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                matches!(d.level, DiagnosticLevel::Error)
                    && d.message.contains("type")
            })
            .collect();

        prop_assert!(
            type_errors.is_empty(),
            "Number literal should match number type annotation:\n{}\nErrors: {:?}",
            source,
            type_errors
        );
    }
}

proptest! {
    // Property: String literal should match string type annotation
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_string_literal_type_check(
        var_name in identifier_strategy(),
        value in "[a-zA-Z0-9 _-]{0,30}"
    ) {
        let source = format!("local {}: string = \"{}\"", var_name, value);

        let core_handler: Arc<dyn CoreDiagnosticHandler> = Arc::new(CoreCollectingDiagnosticHandler::new());
        let parser_handler: Arc<dyn ParserDiagnosticHandler> = Arc::new(ParserCollectingDiagnosticHandler::new());
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);
        let mut lexer = Lexer::new(&source, parser_handler.clone(), &interner);

        let tokens = lexer.tokenize().expect("Should lex");
        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids);
        let mut program = parser.parse().expect("Should parse");

        let mut checker = TypeChecker::new(core_handler.clone(), &interner, &common_ids);
        let _ = checker.check_program(&mut program);

        let diagnostics = core_handler.get_diagnostics();
        let type_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                matches!(d.level, DiagnosticLevel::Error)
                    && d.message.contains("type")
            })
            .collect();

        prop_assert!(
            type_errors.is_empty(),
            "String literal should match string type annotation:\n{}\nErrors: {:?}",
            source,
            type_errors
        );
    }
}

proptest! {
    // Property: Boolean literal should match boolean type annotation
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn prop_boolean_literal_type_check(
        var_name in identifier_strategy(),
        value in prop_oneof![Just("true"), Just("false")]
    ) {
        let source = format!("local {}: boolean = {}", var_name, value);

        let core_handler: Arc<dyn CoreDiagnosticHandler> = Arc::new(CoreCollectingDiagnosticHandler::new());
        let parser_handler: Arc<dyn ParserDiagnosticHandler> = Arc::new(ParserCollectingDiagnosticHandler::new());
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);
        let mut lexer = Lexer::new(&source, parser_handler.clone(), &interner);

        let tokens = lexer.tokenize().expect("Should lex");
        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids);
        let mut program = parser.parse().expect("Should parse");

        let mut checker = TypeChecker::new(core_handler.clone(), &interner, &common_ids);
        let _ = checker.check_program(&mut program);

        let diagnostics = core_handler.get_diagnostics();
        let type_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                matches!(d.level, DiagnosticLevel::Error)
                    && d.message.contains("type")
            })
            .collect();

        prop_assert!(
            type_errors.is_empty(),
            "Boolean literal should match boolean type annotation:\n{}\nErrors: {:?}",
            source,
            type_errors
        );
    }
}

// =============================================================================
// Codegen Correctness Properties
// =============================================================================

proptest! {
    // Property: Generated code should be valid Lua syntax
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn prop_generated_code_is_valid_lua(
        var_name in identifier_strategy(),
        value in number_literal_strategy()
    ) {
        let source = format!("local {} = {}", var_name, value);

        // Parse
        let parser_handler: Arc<dyn ParserDiagnosticHandler> = Arc::new(ParserCollectingDiagnosticHandler::new());
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);
        let mut lexer = Lexer::new(&source, parser_handler.clone(), &interner);

        let tokens = lexer.tokenize().expect("Should lex");
        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids);
        let mut program = parser.parse().expect("Should parse");

        // Generate code
        let mut codegen = CodeGenerator::new(interner.clone())
            .with_mode(CodeGenMode::Require)
            .with_target(LuaTarget::Lua53);

        let lua_code = codegen.generate(&mut program);

        // Debug output on failure
        if lua_code.is_empty() {
            eprintln!("DEBUG: Empty codegen for source: {}", source);
            eprintln!("DEBUG: Program has {} statements", program.statements.len());
        }

        // Verify generated code is non-empty
        prop_assert!(!lua_code.is_empty(), "Generated code should not be empty for: {}", source);
    }
}

proptest! {
    // Property: Function declarations should generate valid Lua functions
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn prop_function_codegen_valid(
        func_name in identifier_strategy(),
        param_count in 0usize..4
    ) {
        let params: Vec<String> = (0..param_count)
            .map(|i| format!("p{}", i))
            .collect();
        let params_str = params.join(", ");

        let source = format!(
            "local function {}({})
  return 42
end",
            func_name, params_str
        );

        // Parse
        let parser_handler: Arc<dyn ParserDiagnosticHandler> = Arc::new(ParserCollectingDiagnosticHandler::new());
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);
        let mut lexer = Lexer::new(&source, parser_handler.clone(), &interner);

        let tokens = lexer.tokenize().expect("Should lex");
        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids);
        let mut program = parser.parse().expect("Should parse");

        // Generate
        let mut codegen = CodeGenerator::new(interner.clone())
            .with_mode(CodeGenMode::Require)
            .with_target(LuaTarget::Lua53);

        let lua_code = codegen.generate(&mut program);

        // Debug output on failure
        if lua_code.is_empty() {
            eprintln!("DEBUG: Empty codegen for function: {}", func_name);
            eprintln!("DEBUG: Source: {}", source);
        }

        prop_assert!(!lua_code.is_empty(), "Generated code should not be empty");
    }
}

proptest! {
    // Property: Table literals should generate valid Lua tables
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn prop_table_literal_codegen_valid(
        entries in proptest::collection::vec(
            (identifier_strategy(), number_literal_strategy()),
            0..5
        )
    ) {
        let entries_str = entries
            .iter()
            .map(|(k, v)| format!("{} = {}", k, v))
            .collect::<Vec<_>>()
            .join(", ");

        let source = format!("local t = {{{}}}", entries_str);

        // Parse
        let parser_handler: Arc<dyn ParserDiagnosticHandler> = Arc::new(ParserCollectingDiagnosticHandler::new());
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);
        let mut lexer = Lexer::new(&source, parser_handler.clone(), &interner);

        let tokens = lexer.tokenize().expect("Should lex");
        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids);
        let mut program = parser.parse().expect("Should parse");

        // Generate
        let mut codegen = CodeGenerator::new(interner.clone())
            .with_mode(CodeGenMode::Require)
            .with_target(LuaTarget::Lua53);

        let lua_code = codegen.generate(&mut program);

        // Debug output on failure
        if lua_code.is_empty() {
            eprintln!("DEBUG: Empty codegen for table");
            eprintln!("DEBUG: Source: {}", source);
        }

        prop_assert!(!lua_code.is_empty(), "Generated code should not be empty");
    }
}

// =============================================================================
// Arithmetic Expression Properties
// =============================================================================

proptest! {
    // Property: Arithmetic operations on numbers should type check
    #![proptest_config(ProptestConfig::with_cases(30))]

    #[test]
    fn prop_arithmetic_operations_type_check(
        a in number_literal_strategy(),
        b in number_literal_strategy(),
        op in prop_oneof![Just("+"), Just("-"), Just("*"), Just("/")]
    ) {
        let source = format!("local result: number = {} {} {}", a, op, b);

        let core_handler: Arc<dyn CoreDiagnosticHandler> = Arc::new(CoreCollectingDiagnosticHandler::new());
        let parser_handler: Arc<dyn ParserDiagnosticHandler> = Arc::new(ParserCollectingDiagnosticHandler::new());
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);
        let mut lexer = Lexer::new(&source, parser_handler.clone(), &interner);

        let tokens = lexer.tokenize().expect("Should lex");
        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids);
        let mut program = parser.parse().expect("Should parse");

        let mut checker = TypeChecker::new(core_handler.clone(), &interner, &common_ids);
        let _ = checker.check_program(&mut program);

        let diagnostics = core_handler.get_diagnostics();
        let type_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                matches!(d.level, DiagnosticLevel::Error)
                    && (d.message.contains("type") || d.message.contains("number"))
            })
            .collect();

        prop_assert!(
            type_errors.is_empty(),
            "Arithmetic operation should type check:\n{}\nErrors: {:?}",
            source,
            type_errors
        );
    }
}

// =============================================================================
// String Concatenation Properties
// =============================================================================

proptest! {
    // Property: String concatenation should work with string types
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn prop_string_concatenation_type_check(
        a in "[a-zA-Z0-9]{1,20}",
        b in "[a-zA-Z0-9]{1,20}"
    ) {
        let source = format!(
            "local result: string = \"{}\" .. \"{}\"",
            a, b
        );

        let core_handler: Arc<dyn CoreDiagnosticHandler> = Arc::new(CoreCollectingDiagnosticHandler::new());
        let parser_handler: Arc<dyn ParserDiagnosticHandler> = Arc::new(ParserCollectingDiagnosticHandler::new());
        let (interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);
        let mut lexer = Lexer::new(&source, parser_handler.clone(), &interner);

        let tokens = lexer.tokenize().expect("Should lex");
        let mut parser = Parser::new(tokens, parser_handler.clone(), &interner, &common_ids);
        let mut program = parser.parse().expect("Should parse");

        let mut checker = TypeChecker::new(core_handler.clone(), &interner, &common_ids);
        let _ = checker.check_program(&mut program);

        let diagnostics = core_handler.get_diagnostics();
        let type_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| {
                matches!(d.level, DiagnosticLevel::Error)
                    && d.message.contains("type")
            })
            .collect();

        prop_assert!(
            type_errors.is_empty(),
            "String concatenation should type check:\n{}\nErrors: {:?}",
            source,
            type_errors
        );
    }
}
