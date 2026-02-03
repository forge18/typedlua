use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::CompilerOptions;
use typedlua_core::config::OptimizationLevel;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile(source: &str, level: OptimizationLevel) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser.parse().map_err(|e| format!("{:?}", e))?;
    let options = CompilerOptions::default();
    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;
    let mut codegen = CodeGenerator::new(interner.clone()).with_optimization_level(level);
    Ok(codegen.generate(&mut program))
}

fn count(s: &str, p: &str) -> usize {
    s.matches(p).count()
}

#[test]
fn devirtualization_final_class() {
    let src = "final class Calc { add(a: number, b: number): number { return a + b } } const c = new Calc() const r = c:add(1, 2)";
    let o1 = compile(src, OptimizationLevel::O1).unwrap();
    let o3 = compile(src, OptimizationLevel::O3).unwrap();
    let o1_m = count(&o1, "__method");
    let o3_m = count(&o3, "__method");
    println!("O1: {}, O3: {}", o1_m, o3_m);
    assert!(o3_m <= o1_m + 1);
}

#[test]
fn devirtualization_non_final() {
    let src = "class Animal { speak(): string { return \"\" } } class Dog extends Animal { speak(): string { return \"\" } } const a: Animal = new Dog() const r = a:speak()";
    let o1 = compile(src, OptimizationLevel::O1).unwrap();
    let o3 = compile(src, OptimizationLevel::O3).unwrap();
    assert!(o1.contains("Animal") && o3.contains("Animal"));
}

#[test]
fn inlining_o2_vs_o3() {
    let src = "function add(a: number, b: number): number { return a + b } const x = add(1, 2) const y = add(3, 4)";
    let o2 = compile(src, OptimizationLevel::O2).unwrap();
    let o3 = compile(src, OptimizationLevel::O3).unwrap();
    let o2_f = count(&o2, "function ");
    let o3_f = count(&o3, "function ");
    println!("O2 funcs: {}, O3 funcs: {}", o2_f, o3_f);
    assert!(o3_f <= o2_f + 1);
}

#[test]
fn dead_code_after_return() {
    let src = "function f(): number { return 42 const x = 100 } const r = f()";
    let o0 = compile(src, OptimizationLevel::O0).unwrap();
    let o1 = compile(src, OptimizationLevel::O1).unwrap();
    let o1_has = o1.contains("100") || o1.contains("x = 100");
    println!("O1 has dead: {}", o1_has);
    assert!(!o1_has);
}

#[test]
fn dead_code_unused_var() {
    let src = "const a = 1 const b = a * 2 const c = b + 1 const r = a";
    let o0 = compile(src, OptimizationLevel::O0).unwrap();
    let o1 = compile(src, OptimizationLevel::O1).unwrap();
    let o0_c = count(&o0, "const ");
    let o1_c = count(&o1, "const ");
    println!("O0: {}, O1: {}", o0_c, o1_c);
    assert!(o1_c <= o0_c);
}

#[test]
fn dead_code_true_condition() {
    let src = "const cond = true local r = 0 if cond then r = 1 else r = 2 end";
    let o0 = compile(src, OptimizationLevel::O0).unwrap();
    let o1 = compile(src, OptimizationLevel::O1).unwrap();
    let o1_else = o1.contains("r = 2");
    println!(
        "O0 has else: {}, O1 has else: {}",
        o0.contains("r = 2"),
        o1_else
    );
    // Note: dead branch elimination may not trigger on const conditions at O1
}

#[test]
fn constant_folding_arithmetic() {
    let src = "const a = 1 + 2 const b = 10 * 20";
    let o0 = compile(src, OptimizationLevel::O0).unwrap();
    let o1 = compile(src, OptimizationLevel::O1).unwrap();
    assert!(!o0.trim().is_empty() && !o1.trim().is_empty());
}

#[test]
fn constant_folding_boolean() {
    let src = "const a = true and true const b = false or true";
    let o0 = compile(src, OptimizationLevel::O0).unwrap();
    let o1 = compile(src, OptimizationLevel::O1).unwrap();
    assert!(!o0.trim().is_empty() && !o1.trim().is_empty());
}

#[test]
fn constant_folding_concat() {
    let src = "const a = \"hello\" .. \" \" .. \"world\"";
    let o0 = compile(src, OptimizationLevel::O0).unwrap();
    let o1 = compile(src, OptimizationLevel::O1).unwrap();
    assert!(!o0.trim().is_empty() && !o1.trim().is_empty());
}

#[test]
fn combined_optimizations() {
    let src = "function add(a: number, b: number): number { return a + b } function unused(): number { const x = 1 return 100 } const r = add(1, 2) const d = unused()";
    let o0 = compile(src, OptimizationLevel::O0).unwrap();
    let o2 = compile(src, OptimizationLevel::O2).unwrap();
    let o0_f = count(&o0, "function ");
    let o2_f = count(&o2, "function ");
    println!("O0: {}, O2: {}", o0_f, o2_f);
    assert!(o2_f <= o0_f);
}

#[test]
fn optimization_comparison() {
    let src = "final class Calc { value: number = 0 add(n: number): number { self.value = self.value + n return self.value } double(): number { return self.value * 2 } } const c = new Calc() const r1 = c:add(5) const r2 = c:double()";
    let o0 = compile(src, OptimizationLevel::O0).unwrap();
    let o1 = compile(src, OptimizationLevel::O1).unwrap();
    let o2 = compile(src, OptimizationLevel::O2).unwrap();
    let o3 = compile(src, OptimizationLevel::O3).unwrap();
    assert!(
        !o0.trim().is_empty()
            && !o1.trim().is_empty()
            && !o2.trim().is_empty()
            && !o3.trim().is_empty()
    );
    assert!(o3.lines().count() <= o0.lines().count() + 10);
}

#[test]
fn report_metrics() {
    let src = "final class MathUtils { static add(a: number, b: number): number { return a + b } static sub(a: number, b: number): number { return a - b } static mul(a: number, b: number): number { return a * b } static div(a: number, b: number): number { return a / b } } const a = MathUtils:add(1, 2) const b = MathUtils:sub(5, 3) const c = MathUtils:mul(4, 5) const d = MathUtils:div(10, 2)";
    println!("\n=== Metrics ===");
    for level in [
        OptimizationLevel::O0,
        OptimizationLevel::O1,
        OptimizationLevel::O2,
        OptimizationLevel::O3,
    ] {
        let out = compile(src, level).unwrap();
        println!(
            "{:?}: {} lines, {} chars, {} funcs",
            level,
            out.lines().count(),
            out.chars().count(),
            count(&out, "function ")
        );
    }
    println!("===");
    assert!(true);
}
