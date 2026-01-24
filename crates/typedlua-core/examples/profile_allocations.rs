/// Heap allocation profiler using dhat
///
/// This tool profiles memory allocations during compilation to identify hotspots
/// and measure the impact of optimizations like string interning and arena allocation.
///
/// Usage:
/// ```bash
/// cargo run --release --example profile_allocations
/// ```
///
/// Output: dhat-heap.json (open with https://nnethercote.github.io/dh_view/dh_view.html)
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn compile_source(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();

    // Lex
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    // Parse
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    // Type check
    let mut type_checker = TypeChecker::new(handler, &interner, common_ids);
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(&interner);
    let output = codegen.generate(&program);

    Ok(output)
}

fn main() {
    let _profiler = dhat::Profiler::new_heap();

    println!("Profiling TypedLua compiler allocations with string interning...");
    println!();

    // Test 1: Simple variable declarations
    let simple_code = r#"
        const x: number = 42
        const y: string = "hello"
        const z: boolean = true
        const w: number = x + 10
    "#;

    println!("1. Compiling simple code (4 statements)...");
    compile_source(simple_code).expect("Simple code should compile");

    // Test 2: Function definitions
    let function_code = r#"
        function add(a: number, b: number): number {
            return a + b
        }

        function multiply(x: number, y: number): number {
            return x * y
        }

        function divide(a: number, b: number): number {
            if b == 0 then
                return 0
            end
            return a / b
        }

        const result = add(multiply(2, 3), divide(10, 2))
    "#;

    println!("2. Compiling functions (4 functions + 1 call)...");
    compile_source(function_code).expect("Function code should compile");

    // Test 3: Class with methods
    let class_code = r#"
        class Point {
            x: number
            y: number

            constructor(x: number, y: number) {
                self.x = x
                self.y = y
            }

            distance(): number {
                return (self.x * self.x + self.y * self.y) ^ 0.5
            }
        }
    "#;

    println!("3. Compiling class (1 class with 2 methods)...");
    compile_source(class_code).expect("Class code should compile");

    // Test 4: Large file (stress test)
    let large_code = (0..100)
        .map(|i| {
            format!(
                "function func{}(x: number, y: number): number\n    const temp{}: number = x + y\n    const result{}: number = temp{} * 2\n    return result{}\nend\n",
                i, i, i, i, i
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    println!("4. Compiling large file (100 functions)...");
    compile_source(&large_code).expect("Large code should compile");

    println!();
    println!("Profiling complete!");
    println!("Results written to: dhat-heap.json");
    println!();
    println!("To view results:");
    println!("  1. Open https://nnethercote.github.io/dh_view/dh_view.html");
    println!("  2. Load dhat-heap.json");
    println!();
    println!("Key metrics to examine:");
    println!("  - Total bytes allocated");
    println!("  - Total allocations count");
    println!("  - Peak memory usage");
    println!("  - Allocation hotspots (top functions)");
}
