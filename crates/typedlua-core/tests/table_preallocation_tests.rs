use std::sync::Arc;
use typedlua_core::ast::expression::{Expression, ExpressionKind};
use typedlua_core::ast::pattern::Pattern;
use typedlua_core::ast::statement::{Statement, VariableDeclaration, VariableKind};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::optimizer::Optimizer;
use typedlua_core::span::Span;
use typedlua_core::string_interner::StringInterner;

// ============================================================================
// Optimizer Tests
// ============================================================================

#[test]
fn test_optimizer_registration() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let interner = Arc::new(StringInterner::new());
    let optimizer = Optimizer::new(
        typedlua_core::config::OptimizationLevel::O1,
        handler,
        interner,
    );

    assert_eq!(optimizer.pass_count(), 15);
    let names = optimizer.pass_names();
    assert!(names.contains(&"constant-folding"));
    assert!(names.contains(&"dead-code-elimination"));
    assert!(names.contains(&"algebraic-simplification"));
    assert!(names.contains(&"table-preallocation"));
    assert!(names.contains(&"global-localization"));
}

#[test]
fn test_optimizer_auto_level() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let interner = Arc::new(StringInterner::new());
    // Auto should default to O1 in debug mode
    let optimizer = Optimizer::new(
        typedlua_core::config::OptimizationLevel::Auto,
        handler,
        interner,
    );

    assert_eq!(optimizer.pass_count(), 15);
}

#[test]
fn test_optimizer_o0_level() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let interner = Arc::new(StringInterner::new());
    let optimizer = Optimizer::new(
        typedlua_core::config::OptimizationLevel::O0,
        handler,
        interner,
    );

    // O0 should still register all passes (but will not run them)
    assert_eq!(optimizer.pass_count(), 15);
}

#[test]
fn test_optimizer_o2_level() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let interner = Arc::new(StringInterner::new());
    let optimizer = Optimizer::new(
        typedlua_core::config::OptimizationLevel::O2,
        handler,
        interner,
    );

    // O2 should include O1 and O2 passes
    assert_eq!(optimizer.pass_count(), 15);
}

#[test]
fn test_optimizer_o3_level() {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let interner = Arc::new(StringInterner::new());
    let optimizer = Optimizer::new(
        typedlua_core::config::OptimizationLevel::O3,
        handler,
        interner,
    );

    // O3 should include all passes
    assert_eq!(optimizer.pass_count(), 15);
}

#[test]
fn test_optimization_level_auto() {
    // Test that Auto resolves to O1 in debug mode
    let level = typedlua_core::config::OptimizationLevel::Auto;
    let effective = level.effective();
    assert_eq!(effective, typedlua_core::config::OptimizationLevel::O1);
}

#[test]
fn test_optimization_level_o0() {
    assert_eq!(
        typedlua_core::config::OptimizationLevel::O0.effective(),
        typedlua_core::config::OptimizationLevel::O0
    );
}

#[test]
fn test_optimization_level_o1() {
    assert_eq!(
        typedlua_core::config::OptimizationLevel::O1.effective(),
        typedlua_core::config::OptimizationLevel::O1
    );
}

#[test]
fn test_optimization_level_o2() {
    assert_eq!(
        typedlua_core::config::OptimizationLevel::O2.effective(),
        typedlua_core::config::OptimizationLevel::O2
    );
}

#[test]
fn test_optimization_level_o3() {
    assert_eq!(
        typedlua_core::config::OptimizationLevel::O3.effective(),
        typedlua_core::config::OptimizationLevel::O3
    );
}

#[test]
fn test_optimization_level_comparison() {
    use typedlua_core::config::OptimizationLevel::*;

    assert!(O0 < O1);
    assert!(O1 < O2);
    assert!(O2 < O3);
    assert!(O0 < Auto);
    assert!(O1 <= Auto);
}

#[test]
fn test_global_localization_creates_local_references() {
    use typedlua_core::ast::{Program, Spanned};
    use typedlua_core::optimizer::OptimizationPass;

    let interner = Arc::new(StringInterner::new());
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let mut optimizer = Optimizer::new(
        typedlua_core::config::OptimizationLevel::O1,
        handler,
        interner.clone(),
    );

    let math_id = interner.get_or_intern("math");
    let sin_id = interner.get_or_intern("sin");
    let cos_id = interner.get_or_intern("cos");
    let _math_id = interner.get_or_intern("_math");

    let span = Span::dummy();

    let math_ident = Spanned::new(math_id, span);
    let sin_ident = Spanned::new(sin_id, span);
    let cos_ident = Spanned::new(cos_id, span);

    let math_ref = Expression::new(
        typedlua_core::ast::expression::ExpressionKind::Identifier(math_id),
        span,
    );
    let sin_call = Expression::new(
        typedlua_core::ast::expression::ExpressionKind::Call(
            Box::new(Expression::new(
                typedlua_core::ast::expression::ExpressionKind::Member(
                    Box::new(math_ref.clone()),
                    sin_ident.clone(),
                ),
                span,
            )),
            vec![],
        ),
        span,
    );
    let cos_call = Expression::new(
        typedlua_core::ast::expression::ExpressionKind::Call(
            Box::new(Expression::new(
                typedlua_core::ast::expression::ExpressionKind::Member(
                    Box::new(math_ref.clone()),
                    cos_ident.clone(),
                ),
                span,
            )),
            vec![],
        ),
        span,
    );

    let initializer = Expression::new(
        typedlua_core::ast::expression::ExpressionKind::Binary(
            typedlua_core::ast::expression::BinaryOp::Add,
            Box::new(sin_call),
            Box::new(cos_call),
        ),
        span,
    );

    let pattern = Pattern::Identifier(Spanned::new(math_id, span));
    let var_decl = VariableDeclaration {
        kind: VariableKind::Local,
        pattern,
        type_annotation: None,
        initializer,
        span,
    };

    let mut program = Program::new(vec![Statement::Variable(var_decl)], span);

    let result = optimizer.optimize(&mut program);
    assert!(result.is_ok(), "Optimization should succeed");

    println!("Optimized program statements:");
    for stmt in &program.statements {
        println!("  {:?}", stmt);
    }

    let has_local_math = program.statements.iter().any(|s| {
        if let Statement::Variable(decl) = s {
            if let Pattern::Identifier(ident) = &decl.pattern {
                return interner.resolve(ident.node) == "_math";
            }
        }
        false
    });
    assert!(
        has_local_math,
        "Should create local reference '_math' for frequently used global"
    );
}
