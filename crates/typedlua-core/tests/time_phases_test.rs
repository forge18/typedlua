#[test]
fn time_100k_phases() {
    use bumpalo::Bump;
    use std::sync::Arc;
    use std::time::Instant;
    use typedlua_core::diagnostics::CollectingDiagnosticHandler;
    use typedlua_core::TypeChecker;
    use typedlua_parser::lexer::Lexer;
    use typedlua_parser::parser::Parser;
    use typedlua_parser::string_interner::StringInterner;

    fn generate_test_code(target_lines: usize) -> String {
        let mut code = String::new();
        let mut line_count = 0;
        code.push_str("// Test\n\n");
        line_count += 2;

        let interface_count = target_lines / 100;
        for i in 0..interface_count {
            code.push_str(&format!(
                "interface I{} {{\n    value: number\n    name: string\n}}\n\n",
                i
            ));
            line_count += 4;
        }

        let class_count = target_lines / 150;
        for i in 0..class_count {
            code.push_str(&format!("class Class{} {{\n", i));
            code.push_str("    private _value: number\n");
            code.push_str("    public name: string\n\n");
            code.push_str("    constructor(value: number, name: string) {\n");
            code.push_str("        self._value = value\n");
            code.push_str("        self.name = name\n");
            code.push_str("    }\n\n");
            code.push_str("    getValue(): number {\n");
            code.push_str("        return self._value\n");
            code.push_str("    }\n\n");
            code.push_str("    setValue(v: number): void {\n");
            code.push_str("        self._value = v\n");
            code.push_str("    }\n");
            code.push_str("}\n\n");
            line_count += 15;
        }

        while line_count < target_lines {
            let func_index = line_count / 5;
            code.push_str(&format!(
                "function compute{}(a: number, b: number): number {{\n",
                func_index
            ));
            code.push_str("    const x = a + b\n");
            code.push_str("    const y = a * b\n");
            code.push_str("    const z = x - y\n");
            code.push_str("    return z\n");
            code.push_str("}\n\n");
            line_count += 6;
        }
        code
    }

    let source = generate_test_code(100000);
    println!("Code: {} lines", source.lines().count());

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let arena = Bump::new();

    let start = Instant::now();
    let mut lexer = Lexer::new(&source, handler.clone(), &interner);
    let tokens = lexer.tokenize().expect("Lexing failed");
    let lex_time = start.elapsed();
    println!("Lexing: {:?} ({} tokens)", lex_time, tokens.len());

    let start = Instant::now();
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids, &arena);
    let program = parser.parse().expect("Parsing failed");
    let parse_time = start.elapsed();
    println!(
        "Parsing: {:?} ({} statements)",
        parse_time,
        program.statements.len()
    );

    let start = Instant::now();
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids, &arena);
    type_checker
        .check_program(&program)
        .expect("Type checking failed");
    let typecheck_time = start.elapsed();
    println!("Type checking: {:?}", typecheck_time);
}
