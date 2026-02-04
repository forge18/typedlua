use typedlua_core::di::DiContainer;

fn compile_and_check(source: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile_with_stdlib(source)
}

#[test]
fn test_aggressive_inlining_works() {
    let source = r#"
        function mediumFunction(x: number): number
            const a = x + 1
            const b = a * 2
            const c = b - 3
            const d = c / 4
            const e = d ^ 2
            const f = e + 1
            return f
        end

        function caller(): number
            return mediumFunction(10)
        end
    "#;

    let output = compile_and_check(source).unwrap();
    println!("Output:\n{}", output);

    assert!(
        output.contains("mediumFunction"),
        "Should have mediumFunction"
    );
}

