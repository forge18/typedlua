use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_and_check(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);

    // Lex
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    // Parse
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    // Type check
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ========================================
// Trivial Unreachability Tests
// ========================================

#[test]
fn test_unreachable_after_wildcard() {
    let source = r#"
        const x = 5
        const result = match x {
            _ => "any",
            1 => "one"
        }
    "#;

    let _result = compile_and_check(source);
    // Note: Currently testing infrastructure doesn't easily access warnings
    // This test documents the expected behavior
}

#[test]
fn test_unreachable_after_identifier() {
    let source = r#"
        const x = 5
        const result = match x {
            n => n + 1,
            5 => 10
        }
    "#;

    let _result = compile_and_check(source);
    // Should warn: 5 is unreachable after identifier
}

#[test]
fn test_reachable_after_guarded_wildcard() {
    let source = r#"
        const x = 5
        const result = match x {
            n when n > 10 => "big",
            _ => "small"
        }
    "#;

    let result = compile_and_check(source);
    // Should NOT warn: previous pattern has guard
    assert!(result.is_ok(), "Should compile without errors");
}

// ========================================
// Literal Subsumption Tests
// ========================================

#[test]
fn test_duplicate_literal() {
    let source = r#"
        const x: boolean = true
        const result = match x {
            true => "yes",
            true => "also yes"
        }
    "#;

    let _result = compile_and_check(source);
    // Should warn: duplicate pattern
}

#[test]
fn test_or_pattern_subsumes_literal() {
    let source = r#"
        const x = 2
        const result = match x {
            1 | 2 | 3 => "small",
            2 => "two"
        }
    "#;

    let _result = compile_and_check(source);
    // Should warn: 2 is already covered by or-pattern
}

#[test]
fn test_or_pattern_partial_overlap_no_warning() {
    let source = r#"
        const x = 3
        const result = match x {
            1 | 2 => "one or two",
            2 | 3 => "two or three"
        }
    "#;

    let result = compile_and_check(source);
    // Should NOT warn - only partial overlap (3 is new)
    assert!(result.is_ok(), "Should compile without errors");
}

// ========================================
// Or-Pattern Subsumption Tests
// ========================================

#[test]
fn test_or_pattern_fully_subsumed() {
    let source = r#"
        const x = 2
        const result = match x {
            1 | 2 | 3 | 4 => "small",
            2 | 3 => "middle"
        }
    "#;

    let _result = compile_and_check(source);
    // Should warn: {2, 3} is subset of {1, 2, 3, 4}
}

#[test]
fn test_multiple_literal_alternatives() {
    let source = r#"
        const x = 1
        const result = match x {
            1 => "one",
            1 => "duplicate"
        }
    "#;

    let _result = compile_and_check(source);
    // Should warn: exact duplicate
}

// ========================================
// Array Pattern Tests
// ========================================

#[test]
fn test_array_wildcard_subsumes_literal() {
    let source = r#"
        const x = {1, 2}
        const result = match x {
            {a, b} => "any pair",
            {1, 2} => "specific"
        }
    "#;

    let _result = compile_and_check(source);
    // Should warn: identifiers subsume literals
}

#[test]
fn test_array_no_warning_different_length() {
    let source = r#"
        const x: {number, number} | {number, number, number} = {1, 2}
        const result = match x {
            {a, b} => "two",
            {a, b, c} => "three"
        }
    "#;

    let result = compile_and_check(source);
    // Note: Different array lengths in patterns are handled by exhaustiveness checking
    // This test documents the expected behavior (both patterns needed for exhaustiveness)
    // No unreachable warning should be emitted even if compilation succeeds or fails
    let _ = result;
}

#[test]
fn test_array_rest_pattern_subsumption() {
    let source = r#"
        const x = {1, 2, 3}
        const result = match x {
            {1, ...rest} => "starts with 1",
            {1, 2, 3} => "exact"
        }
    "#;

    let _result = compile_and_check(source);
    // Should warn: rest pattern covers exact match
}

// ========================================
// Object Pattern Tests
// ========================================

#[test]
fn test_object_wildcard_subsumes_literal() {
    let source = r#"
        const x = {a: 1, b: 2}
        const result = match x {
            {a, b} => "any object",
            {a: 1, b: 2} => "specific"
        }
    "#;

    let _result = compile_and_check(source);
    // Should warn: identifiers subsume literals
}

#[test]
fn test_object_different_keys_no_warning() {
    let source = r#"
        const x = {a: 1, b: 2}
        const result = match x {
            {a: 1} => "has a=1",
            {b: 2} => "has b=2"
        }
    "#;

    let result = compile_and_check(source);
    // Should NOT warn - different constraints
    assert!(result.is_ok(), "Should compile without errors");
}

#[test]
fn test_object_missing_property_no_warning() {
    let source = r#"
        const x = {a: 1, b: 2}
        const result = match x {
            {a: 1, b: 2} => "both",
            {a: 1} => "only a"
        }
    "#;

    let result = compile_and_check(source);
    // Should NOT warn - later pattern has fewer constraints (open world assumption)
    assert!(result.is_ok(), "Should compile without errors");
}

// ========================================
// Guard Interaction Tests
// ========================================

#[test]
fn test_guarded_pattern_not_subsumer() {
    let source = r#"
        const x = 5
        const result = match x {
            5 when x > 10 => "big five",
            5 => "normal five"
        }
    "#;

    let result = compile_and_check(source);
    // Should NOT warn: earlier has guard
    assert!(result.is_ok(), "Should compile without errors");
}

#[test]
fn test_unreachable_despite_guard() {
    let source = r#"
        const x = 5
        const result = match x {
            _ => "any",
            5 when x > 0 => "positive five"
        }
    "#;

    let _result = compile_and_check(source);
    // Should warn: unreachable even with guard
}

// ========================================
// Edge Cases
// ========================================

#[test]
fn test_literal_boolean_true_vs_false() {
    let source = r#"
        const x: boolean = true
        const result = match x {
            true => "yes",
            false => "no",
            true => "duplicate"
        }
    "#;

    let _result = compile_and_check(source);
    // Should warn: duplicate true
}

#[test]
fn test_single_arm_no_warning() {
    let source = r#"
        const x = 5
        const result = match x {
            _ => "any"
        }
    "#;

    let result = compile_and_check(source);
    // Should NOT warn - only one arm
    assert!(result.is_ok(), "Should compile without errors");
}

#[test]
fn test_two_different_patterns_no_warning() {
    let source = r#"
        const x: 1 | 2 = 1
        const result = match x {
            1 => "one",
            2 => "two"
        }
    "#;

    let result = compile_and_check(source);
    // Should NOT warn - no subsumption between different literals
    assert!(result.is_ok(), "Should compile without errors");
}
