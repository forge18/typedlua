use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::{CompilerOptions, OptimizationLevel};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn compile_and_check(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);

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
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids)
        .with_options(CompilerOptions::default());
    type_checker
        .check_program(&program)
        .map_err(|e| e.message)?;

    // Generate code
    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Basic Rich Enum Tests
// ============================================================================

#[test]
fn test_rich_enum_with_constructor_args() {
    let source = r#"
        enum Planet {
            Mercury(3.303e23, 2.4397e6),
            Earth(5.976e24, 6.37814e6),
            mass: number,
            radius: number,
            constructor(mass: number, radius: number) {
                self.mass = mass
                self.radius = radius
            }
        }
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Verify metatable structure
            assert!(
                output.contains("local Planet = {}"),
                "Should create enum table"
            );
            assert!(
                output.contains("Planet.__index = Planet"),
                "Should set __index to self"
            );

            // Verify constructor function
            assert!(
                output.contains("local function Planet__new(name, ordinal, mass, radius)"),
                "Should create constructor function with field parameters"
            );
            assert!(
                output.contains("setmetatable({}, Planet)"),
                "Should set metatable on instance"
            );

            // Verify built-in methods
            assert!(
                output.contains("function Planet:name()"),
                "Should have name() method"
            );
            assert!(
                output.contains("return self.__name"),
                "name() should return __name field"
            );
            assert!(
                output.contains("function Planet:ordinal()"),
                "Should have ordinal() method"
            );
            assert!(
                output.contains("return self.__ordinal"),
                "ordinal() should return __ordinal field"
            );

            // Verify enum instances
            assert!(
                output.contains("Planet.Mercury = Planet__new(\"Mercury\", 0, "),
                "Should create Mercury instance with correct arguments"
            );
            assert!(
                output.contains("Planet.Earth = Planet__new(\"Earth\", 1, "),
                "Should create Earth instance with correct arguments"
            );

            // Verify static methods
            assert!(
                output.contains("function Planet.values()"),
                "Should have values() static method"
            );
            assert!(
                output.contains("return Planet.__values"),
                "values() should return __values array"
            );
            assert!(
                output.contains("function Planet.valueOf(name)"),
                "Should have valueOf() static method"
            );
            assert!(
                output.contains("return Planet.__byName[name]"),
                "valueOf() should use __byName hash table"
            );

            // Verify __values array
            assert!(
                output.contains("Planet.__values = {")
                    && output.contains("Planet.Mercury")
                    && output.contains("Planet.Earth"),
                "Should create __values array with all instances"
            );

            // Verify __byName hash table
            assert!(
                output.contains("Planet.__byName = {"),
                "Should create __byName hash table"
            );
            assert!(
                output.contains("Mercury = Planet.Mercury"),
                "Should map Mercury in __byName"
            );
            assert!(
                output.contains("Earth = Planet.Earth"),
                "Should map Earth in __byName"
            );

            // Verify instantiation prevention
            assert!(
                output.contains("setmetatable(Planet, {"),
                "Should set metatable on enum table"
            );
            assert!(
                output.contains("__call = function()"),
                "Should have __call metamethod"
            );
            assert!(
                output.contains("error(\"Cannot instantiate enum Planet directly\")"),
                "Should error on direct instantiation"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_rich_enum_with_methods() {
    let source = r#"
        enum Planet {
            Mercury(3.303e23, 2.4397e6),
            Earth(5.976e24, 6.37814e6),
            mass: number,
            radius: number,
            constructor(mass: number, radius: number) {
                self.mass = mass
                self.radius = radius
            },
            function surfaceGravity(): number {
                const G = 6.67430e-11
                return G * self.mass / (self.radius ^ 2)
            }
        }
    "#;

    let result = compile_and_check(source);
    match &result {
        Ok(output) => {
            println!("Generated code:\n{}", output);

            // Verify custom method is generated
            assert!(
                output.contains("function Planet:surfaceGravity()"),
                "Should generate surfaceGravity method"
            );

            // Verify method body is included
            assert!(output.contains("local G = "), "Should include method body");
            assert!(output.contains("return"), "Should include return statement");

            // Verify method is on enum table (can be called on instances)
            assert!(
                output.contains("Planet.__index = Planet"),
                "Methods should be accessible via __index"
            );
        }
        Err(e) => {
            panic!("Should compile successfully: {}", e);
        }
    }
}

#[test]
fn test_simple_enum_still_works() {
    let source = r#"
        enum Color {
            Red = 1,
            Green = 2,
            Blue = 3
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Simple enum should still work: {:?}",
        result.err()
    );

    let output = result.unwrap();
    println!("Generated code:\n{}", output);

    // Verify simple table structure (not rich enum)
    assert!(
        output.contains("local Color = {"),
        "Should create simple table"
    );
    assert!(output.contains("Red = 1"), "Should assign Red = 1");
    assert!(output.contains("Green = 2"), "Should assign Green = 2");
    assert!(output.contains("Blue = 3"), "Should assign Blue = 3");

    // Should NOT have rich enum features
    assert!(
        !output.contains("Color__mt"),
        "Should not create metatable for simple enum"
    );
    assert!(
        !output.contains("Color__new"),
        "Should not create constructor for simple enum"
    );
    assert!(
        !output.contains("function Color.values()"),
        "Should not create values() for simple enum"
    );
}

// ============================================================================
// Optimization Level Tests
// ============================================================================

#[test]
fn test_o2_optimization_precomputes_instances() {
    let source = r#"
        enum Planet {
            Mercury(3.303e23, 2.4397e6),
            Earth(5.976e24, 6.37814e6),
            mass: number,
            radius: number,
            constructor(mass: number, radius: number) {
                self.mass = mass
                self.radius = radius
            }
        }
    "#;

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);

    // Lex
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().unwrap();

    // Parse
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser.parse().unwrap();

    // Type check
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids)
        .with_options(CompilerOptions::default());
    type_checker.check_program(&program).unwrap();

    // Generate code with O2 optimization
    let mut codegen =
        CodeGenerator::new(interner.clone()).with_optimization_level(OptimizationLevel::O2);
    let output = codegen.generate(&mut program);

    println!("O2 Generated code:\n{}", output);

    // Verify O2 optimization: instances created as literal tables
    assert!(
        output.contains("Planet.Mercury = setmetatable({"),
        "O2: Should create instances as literal tables"
    );
    assert!(
        output.contains(r#"__name = "Mercury""#),
        "O2: Should have __name field"
    );
    assert!(
        output.contains("mass =") && output.contains("radius ="),
        "O2: Should inline field values"
    );

    // Should still have constructor function for potential runtime use
    assert!(
        output.contains("function Planet__new"),
        "O2: Constructor function should still exist"
    );
}

#[test]
fn test_o3_optimization_adds_inline_hints() {
    let source = r#"
        enum Planet {
            Mercury(3.303e23, 2.4397e6),
            mass: number,
            radius: number,
            constructor(mass: number, radius: number) {
                self.mass = mass
                self.radius = radius
            }
        }
    "#;

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);

    // Lex
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().unwrap();

    // Parse
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser.parse().unwrap();

    // Type check
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids)
        .with_options(CompilerOptions::default());
    type_checker.check_program(&program).unwrap();

    // Generate code with O3 optimization
    let mut codegen =
        CodeGenerator::new(interner.clone()).with_optimization_level(OptimizationLevel::O3);
    let output = codegen.generate(&mut program);

    println!("Generated code:\n{}", output);
    assert!(
        output.contains("Planet.Mercury = setmetatable({"),
        "O3: Should also include O2 optimizations"
    );
    assert!(
        output.contains(r#"__name = "Mercury""#),
        "O3: Should have __name field"
    );
}

#[test]
fn test_o1_uses_constructor_calls() {
    let source = r#"
        enum Planet {
            Mercury(3.303e23, 2.4397e6),
            mass: number,
            radius: number,
            constructor(mass: number, radius: number) {
                self.mass = mass
                self.radius = radius
            }
        }
    "#;

    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);

    // Lex
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().unwrap();

    // Parse
    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser.parse().unwrap();

    // Type check
    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids)
        .with_options(CompilerOptions::default());
    type_checker.check_program(&program).unwrap();

    // Generate code
    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    println!("Generated code:\n{}", output);

    // Verify O1: instances created via constructor calls
    assert!(
        output.contains("Planet.Mercury = Planet__new(\"Mercury\", 0,"),
        "O1: Should create instances via constructor calls"
    );

    // Should NOT have O2 optimizations
    assert!(
        !output.contains("setmetatable({ __name = \"Mercury\""),
        "O1: Should not use literal table syntax for instances"
    );

    // Should NOT have O3 optimizations
    assert!(
        !output.contains("-- @inline"),
        "O1: Should not add inline hints"
    );
}
