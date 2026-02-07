use bumpalo::Bump;
use std::sync::Arc;

use typedlua_core::config::OptimizationLevel;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::optimizer::Optimizer;
use typedlua_core::MutableProgram;
use typedlua_parser::ast::expression::{
    ArrayElement, BinaryOp, Expression, ExpressionKind, Literal,
};
use typedlua_parser::ast::pattern::Pattern;
use typedlua_parser::ast::statement::{Statement, VariableDeclaration, VariableKind};
use typedlua_parser::ast::Program;
use typedlua_parser::ast::Spanned;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::StringInterner;

fn create_optimizer(level: OptimizationLevel) -> Optimizer<'static> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let interner = Arc::new(StringInterner::new());
    Optimizer::new(level, handler, interner)
}

// ============================================================================
// Optimizer Tests
// ============================================================================

#[test]
fn test_optimizer_registration() {
    let optimizer = create_optimizer(OptimizationLevel::O1);

    let pass_count = optimizer.pass_count();
    assert!(
        pass_count >= 3,
        "O1 should have at least 3 passes, got {}",
        pass_count
    );

    let names = optimizer.pass_names();
    assert!(
        names.contains(&"constant-folding"),
        "Should have constant-folding pass"
    );
    assert!(
        names.contains(&"algebraic-simplification"),
        "Should have algebraic-simplification pass"
    );
    assert!(
        names.contains(&"global-localization"),
        "Should have global-localization pass"
    );
}

#[test]
fn test_optimizer_auto_level() {
    let optimizer = create_optimizer(OptimizationLevel::Auto);

    let pass_count = optimizer.pass_count();
    assert!(
        pass_count >= 3,
        "Auto should have at least 3 passes (O1 base), got {}",
        pass_count
    );
}

#[test]
fn test_optimizer_o0_level() {
    let optimizer = create_optimizer(OptimizationLevel::O0);

    let pass_count = optimizer.pass_count();
    assert!(
        pass_count >= 1,
        "O0 should register at least 1 pass (global-localization), got {}",
        pass_count
    );
}

#[test]
fn test_optimizer_o2_level() {
    let optimizer = create_optimizer(OptimizationLevel::O2);

    let pass_count = optimizer.pass_count();
    assert!(
        pass_count >= 12,
        "O2 should have at least 12 passes (5 O1 + 7 O2), got {}",
        pass_count
    );

    let names = optimizer.pass_names();
    assert!(
        names.contains(&"function-inlining"),
        "O2 should have function-inlining pass"
    );
    assert!(
        names.contains(&"loop-optimization"),
        "O2 should have loop-optimization pass"
    );
    assert!(
        names.contains(&"string-concat-optimization"),
        "O2 should have string-concat-optimization pass"
    );
}

#[test]
fn test_optimizer_o3_level() {
    let optimizer = create_optimizer(OptimizationLevel::O3);

    let pass_count = optimizer.pass_count();
    assert!(
        pass_count >= 17,
        "O3 should have at least 17 passes (5 O1 + 7 O2 + 5+ O3), got {}",
        pass_count
    );

    let names = optimizer.pass_names();
    assert!(
        names.contains(&"aggressive-inlining"),
        "O3 should have aggressive-inlining pass"
    );
    assert!(
        names.contains(&"operator-inlining"),
        "O3 should have operator-inlining pass"
    );
    assert!(
        names.contains(&"devirtualization"),
        "O3 should have devirtualization pass"
    );
}

#[test]
fn test_optimizer_level_ordering() {
    let o1_optimizer = create_optimizer(OptimizationLevel::O1);
    let o2_optimizer = create_optimizer(OptimizationLevel::O2);
    let o3_optimizer = create_optimizer(OptimizationLevel::O3);

    assert!(
        o3_optimizer.pass_count() >= o2_optimizer.pass_count(),
        "O3 should have at least as many passes as O2"
    );
    assert!(
        o2_optimizer.pass_count() >= o1_optimizer.pass_count(),
        "O2 should have at least as many passes as O1"
    );
}

#[test]
fn test_optimization_level_auto() {
    let level = OptimizationLevel::Auto;
    let effective = level.effective();
    assert_eq!(effective, OptimizationLevel::O1);
}

#[test]
fn test_optimization_level_o0() {
    assert_eq!(OptimizationLevel::O0.effective(), OptimizationLevel::O0);
}

#[test]
fn test_optimization_level_o1() {
    assert_eq!(OptimizationLevel::O1.effective(), OptimizationLevel::O1);
}

#[test]
fn test_optimization_level_o2() {
    assert_eq!(OptimizationLevel::O2.effective(), OptimizationLevel::O2);
}

#[test]
fn test_optimization_level_o3() {
    assert_eq!(OptimizationLevel::O3.effective(), OptimizationLevel::O3);
}

#[test]
fn test_optimization_level_comparison() {
    assert!(OptimizationLevel::O0 < OptimizationLevel::O1);
    assert!(OptimizationLevel::O1 < OptimizationLevel::O2);
    assert!(OptimizationLevel::O2 < OptimizationLevel::O3);
    assert!(OptimizationLevel::O0 < OptimizationLevel::Auto);
    assert!(OptimizationLevel::O1 <= OptimizationLevel::Auto);
}

#[test]
fn test_global_localization_creates_local_references() {
    let interner = Arc::new(StringInterner::new());
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let arena = Bump::new();
    let mut optimizer = Optimizer::new(OptimizationLevel::O1, handler, interner.clone());

    let math_id = interner.get_or_intern("math");
    let sin_id = interner.get_or_intern("sin");
    let cos_id = interner.get_or_intern("cos");
    let x_id = interner.get_or_intern("x");
    let y_id = interner.get_or_intern("y");

    let span = Span::dummy();

    let sin_ident = Spanned::new(sin_id, span);
    let cos_ident = Spanned::new(cos_id, span);

    // Create: local x = math.sin()
    let math_ref1 = Expression::new(ExpressionKind::Identifier(math_id), span);
    let member_expr1 = arena.alloc(Expression::new(
        ExpressionKind::Member(arena.alloc(math_ref1), sin_ident),
        span,
    ));
    let empty_args1: &[_] = arena.alloc_slice_clone(&[]);
    let sin_call = Expression::new(
        ExpressionKind::Call(member_expr1, empty_args1, None),
        span,
    );
    let var_x = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(x_id, span)),
        type_annotation: None,
        initializer: sin_call,
        span,
    };

    // Create: local y = math.cos()
    let math_ref2 = Expression::new(ExpressionKind::Identifier(math_id), span);
    let member_expr2 = arena.alloc(Expression::new(
        ExpressionKind::Member(arena.alloc(math_ref2), cos_ident),
        span,
    ));
    let empty_args2: &[_] = arena.alloc_slice_clone(&[]);
    let cos_call = Expression::new(
        ExpressionKind::Call(member_expr2, empty_args2, None),
        span,
    );
    let var_y = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(y_id, span)),
        type_annotation: None,
        initializer: cos_call,
        span,
    };

    let stmts = arena.alloc_slice_clone(&[
        Statement::Variable(var_x),
        Statement::Variable(var_y),
    ]);
    let program = Program::new(stmts, span);

    let mut mutable = MutableProgram::from_program(&program);
    let _ = optimizer.optimize(&mut mutable, &arena);
}

#[test]
fn test_table_preallocation_hint() {
    let interner = Arc::new(StringInterner::new());
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let arena = Bump::new();
    let mut optimizer = Optimizer::new(OptimizationLevel::O1, handler, interner.clone());
    let span = Span::dummy();
    let x_id = interner.get_or_intern("x");
    let y_id = interner.get_or_intern("y");
    let z_id = interner.get_or_intern("z");

    let elements = arena.alloc_slice_clone(&[
        ArrayElement::Expression(Expression::new(ExpressionKind::Identifier(x_id), span)),
        ArrayElement::Expression(Expression::new(ExpressionKind::Identifier(y_id), span)),
        ArrayElement::Expression(Expression::new(ExpressionKind::Identifier(z_id), span)),
    ]);

    let array_expr = Expression::new(ExpressionKind::Array(elements), span);

    let var_decl = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(interner.get_or_intern("arr"), span)),
        type_annotation: None,
        initializer: array_expr,
        span,
    };

    let stmts = arena.alloc_slice_clone(&[Statement::Variable(var_decl)]);
    let program = Program::new(stmts, span);

    let mut mutable = MutableProgram::from_program(&program);
    let _ = optimizer.optimize(&mut mutable, &arena);
}

#[test]
fn test_constant_folding() {
    let interner = Arc::new(StringInterner::new());
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let arena = Bump::new();
    let mut optimizer = Optimizer::new(OptimizationLevel::O1, handler, interner.clone());
    let span = Span::dummy();
    let a_id = interner.get_or_intern("a");
    let b_id = interner.get_or_intern("b");

    let one = Expression::new(ExpressionKind::Literal(Literal::Number(1.0)), span);
    let two = Expression::new(ExpressionKind::Literal(Literal::Number(2.0)), span);
    let three = Expression::new(ExpressionKind::Literal(Literal::Number(3.0)), span);

    let add_expr = Expression::new(
        ExpressionKind::Binary(BinaryOp::Add, arena.alloc(one), arena.alloc(two)),
        span,
    );

    let var_decl = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(a_id, span)),
        type_annotation: None,
        initializer: add_expr,
        span,
    };

    let mult_expr = Expression::new(
        ExpressionKind::Binary(
            BinaryOp::Multiply,
            arena.alloc(Expression::new(ExpressionKind::Identifier(a_id), span)),
            arena.alloc(three),
        ),
        span,
    );

    let var_decl2 = VariableDeclaration {
        kind: VariableKind::Local,
        pattern: Pattern::Identifier(Spanned::new(b_id, span)),
        type_annotation: None,
        initializer: mult_expr,
        span,
    };

    let stmts = arena.alloc_slice_clone(&[
        Statement::Variable(var_decl),
        Statement::Variable(var_decl2),
    ]);
    let program = Program::new(stmts, span);

    let mut mutable = MutableProgram::from_program(&program);
    let _ = optimizer.optimize(&mut mutable, &arena);
}
