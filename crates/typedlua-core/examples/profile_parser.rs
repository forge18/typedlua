// Profile parser allocation performance with arena allocator
use bumpalo::Bump;
use std::sync::Arc;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn main() {
    // Generate a medium-sized realistic program
    let source = generate_medium_program();

    // Run many iterations to get meaningful profiling data
    for _ in 0..1000 {
        parse_program(&source);
    }
}

fn parse_program(source: &str) {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let arena = Bump::new();

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    if let Ok(tokens) = lexer.tokenize() {
        let mut parser = Parser::new(tokens, handler, &interner, &common_ids, &arena);
        let _ = parser.parse();
    }
}

fn generate_medium_program() -> String {
    let mut program = String::new();

    // Generate variables
    for i in 0..50 {
        program.push_str(&format!("const var{}: number = {}\n", i, i));
    }

    // Generate functions
    for i in 0..10 {
        program.push_str(&format!(
            "function func{}(x: number): number {{\n  return x + {}\n}}\n",
            i, i
        ));
    }

    // Generate classes
    for i in 0..5 {
        program.push_str(&format!(
            "class Class{} {{\n  public prop: number\n  constructor(val: number) {{\n    self.prop = val\n  }}\n}}\n",
            i
        ));
    }

    // Generate interfaces
    for i in 0..5 {
        program.push_str(&format!(
            "interface Interface{} {{\n  method(): number\n}}\n",
            i
        ));
    }

    program
}
