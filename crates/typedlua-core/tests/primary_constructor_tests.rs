use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::diagnostics::{CollectingDiagnosticHandler, DiagnosticHandler};
use typedlua_core::lexer::Lexer;
use typedlua_core::parser::Parser;
use typedlua_core::string_interner::StringInterner;
use typedlua_core::typechecker::TypeChecker;

fn parse(source: &str) -> Result<(), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let result = parser.parse();

    // Check both parser errors and diagnostic errors
    if let Err(e) = result {
        return Err(e.message);
    }

    if handler.error_count() > 0 {
        let diagnostics = handler.get_diagnostics();
        if let Some(diag) = diagnostics.first() {
            return Err(diag.message.clone());
        }
    }

    Ok(())
}

fn type_check(source: &str) -> Result<(), String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser.parse().map_err(|e| e.message)?;

    let mut checker = TypeChecker::new(handler.clone(), &interner, &common_ids);
    let result = checker.check_program(&mut program);

    // Check both type checker errors and diagnostic errors
    if let Err(e) = result {
        return Err(e.message);
    }

    if handler.error_count() > 0 {
        let diagnostics = handler.get_diagnostics();
        if let Some(diag) = diagnostics.first() {
            return Err(diag.message.clone());
        }
    }

    Ok(())
}

fn compile_to_lua(source: &str) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Arc::new(interner);
    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser.parse().map_err(|e| e.message)?;

    let mut checker = TypeChecker::new(handler.clone(), &interner, &common_ids);
    checker.check_program(&mut program).map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone());
    let lua_code = codegen.generate(&mut program);

    Ok(lua_code)
}

#[test]
fn test_basic_primary_constructor() {
    let source = r#"
        class Point(public x: number, public y: number) {
        }
    "#;

    assert!(
        parse(source).is_ok(),
        "Basic primary constructor should parse successfully"
    );
}

#[test]
fn test_primary_constructor_with_access_modifiers() {
    let source = r#"
        class Person(
            public name: string,
            private age: number,
            protected id: string
        ) {
        }
    "#;

    assert!(
        parse(source).is_ok(),
        "Primary constructor with different access modifiers should parse"
    );
}

#[test]
fn test_primary_constructor_with_readonly() {
    let source = r#"
        class Point(public readonly x: number, private readonly y: number) {
        }
    "#;

    assert!(
        parse(source).is_ok(),
        "Primary constructor with readonly modifiers should parse"
    );
}

#[test]
fn test_primary_constructor_with_default_values() {
    let source = r#"
        class Point(public x: number = 0, public y: number = 0) {
        }
    "#;

    assert!(
        parse(source).is_ok(),
        "Primary constructor with default values should parse"
    );
}

#[test]
fn test_primary_constructor_with_inheritance() {
    let source = r#"
        class Shape(public color: string) {
        }

        class Circle(public radius: number) extends Shape("red") {
        }
    "#;

    assert!(
        parse(source).is_ok(),
        "Primary constructor with inheritance and parent constructor args should parse"
    );
}

#[test]
fn test_primary_constructor_with_parent_args() {
    let source = r#"
        class Point3D(public x: number, public y: number, public z: number) extends Point(x, y) {
        }
    "#;

    assert!(
        parse(source).is_ok(),
        "Primary constructor with parent constructor forwarding should parse"
    );
}

#[test]
fn test_primary_constructor_with_additional_members() {
    let source = r#"
        class Point(public x: number, public y: number) {
            distance(): number {
                return Math.sqrt(this.x * this.x + this.y * this.y)
            }
        }
    "#;

    assert!(
        parse(source).is_ok(),
        "Primary constructor with additional methods should parse"
    );
}

#[test]
fn test_error_mixing_primary_and_parameterized_constructor() {
    let source = r#"
        class Point(public x: number, public y: number) {
            constructor(x: number, y: number) {
                this.x = x
                this.y = y
            }
        }
    "#;

    let result = parse(source);
    match result {
        Ok(_) => panic!("Should error when mixing primary and parameterized constructor"),
        Err(msg) => assert!(
            msg.contains("Cannot have both"),
            "Error message should mention the conflict, got: {}",
            msg
        ),
    }
}

#[test]
fn test_empty_primary_constructor() {
    let source = r#"
        class Empty() {
        }
    "#;

    assert!(
        parse(source).is_ok(),
        "Empty primary constructor should parse"
    );
}

#[test]
fn test_primary_constructor_mixed_modifiers() {
    let source = r#"
        class Complex(
            public readonly a: number,
            private b: string,
            protected readonly c: boolean = true
        ) {
        }
    "#;

    assert!(
        parse(source).is_ok(),
        "Primary constructor with mixed modifiers should parse"
    );
}

// Type Checking Tests

#[test]
fn test_typecheck_primary_constructor_creates_properties() {
    let source = r#"
        class Point(public x: number, public y: number) {
        }
    "#;

    match type_check(source) {
        Ok(_) => (),
        Err(e) => panic!(
            "Primary constructor should type check successfully. Error: {}",
            e
        ),
    }
}

#[test]
fn test_typecheck_error_duplicate_property_name() {
    let source = r#"
        class Point(public x: number) {
            x: string
        }
    "#;

    let result = type_check(source);
    assert!(
        result.is_err(),
        "Should error when primary constructor parameter conflicts with property"
    );
    assert!(
        result
            .unwrap_err()
            .contains("conflicts with existing class member"),
        "Error should mention the conflict"
    );
}

#[test]
fn test_typecheck_parent_constructor_args() {
    let source = r#"
        class Shape(public color: string) {
        }

        class Circle(public radius: number) extends Shape("red") {
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Parent constructor arguments should type check"
    );
}

#[test]
fn test_typecheck_primary_constructor_with_methods() {
    let source = r#"
        class Rectangle(public width: number, public height: number) {
            area(): number {
                return 42
            }

            perimeter(): number {
                return 100
            }
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Primary constructor with methods should type check"
    );
}

#[test]
fn test_typecheck_access_modifiers() {
    let source = r#"
        class Person(
            public name: string,
            private age: number,
            protected id: string
        ) {
        }
    "#;

    assert!(
        type_check(source).is_ok(),
        "Different access modifiers should type check correctly"
    );
}

// Code Generation Tests

#[test]
fn test_codegen_basic_primary_constructor() {
    let source = r#"
        class Point(public x: number, public y: number) {
        }
    "#;

    let lua_code = compile_to_lua(source).expect("Should compile successfully");

    // Should generate both _init and new methods
    assert!(
        lua_code.contains("function Point._init(self, x, y)"),
        "Should generate _init method"
    );
    assert!(
        lua_code.contains("function Point.new(x, y)"),
        "Should generate new method"
    );
    assert!(
        lua_code.contains("self.x = x"),
        "Should initialize x property"
    );
    assert!(
        lua_code.contains("self.y = y"),
        "Should initialize y property"
    );
    assert!(
        lua_code.contains("Point._init(self, x, y)"),
        "new method should call _init"
    );
}

#[test]
fn test_codegen_private_access_modifier() {
    let source = r#"
        class Person(public name: string, private age: number, protected id: string) {
        }
    "#;

    let lua_code = compile_to_lua(source).expect("Should compile successfully");

    // Private properties should be prefixed with _
    assert!(
        lua_code.contains("self.name = name"),
        "Public property should not have prefix"
    );
    assert!(
        lua_code.contains("self._age = age"),
        "Private property should have _ prefix"
    );
    assert!(
        lua_code.contains("self.id = id"),
        "Protected property should not have prefix"
    );
}

#[test]
fn test_codegen_parent_constructor_forwarding() {
    let source = r#"
        class Shape(public color: string) {
        }

        class Circle(public radius: number) extends Shape("red") {
        }
    "#;

    let lua_code = compile_to_lua(source).expect("Should compile successfully");

    // Circle should call Shape._init with parent args
    assert!(
        lua_code.contains("Shape._init(self, \"red\")"),
        "Should forward to parent constructor"
    );
    assert!(
        lua_code.contains("self.radius = radius"),
        "Should initialize own property after parent call"
    );
}

#[test]
fn test_codegen_inheritance_with_parameter_forwarding() {
    let source = r#"
        class Point(public x: number, public y: number) {
        }

        class Point3D(public z: number) extends Point(0, 0) {
        }
    "#;

    let lua_code = compile_to_lua(source).expect("Should compile successfully");

    // Point3D should forward literal args to Point
    assert!(
        lua_code.contains("Point._init(self, 0, 0)"),
        "Should forward arguments to parent"
    );
    assert!(
        lua_code.contains("self.z = z"),
        "Should initialize own property"
    );
}

#[test]
fn test_codegen_primary_constructor_with_methods() {
    let source = r#"
        class Rectangle(public width: number, public height: number) {
            area(): number {
                return 42
            }
        }
    "#;

    let lua_code = compile_to_lua(source).expect("Should compile successfully");

    // Should generate constructor and method
    assert!(
        lua_code.contains("function Rectangle._init(self, width, height)"),
        "Should generate constructor"
    );
    assert!(
        lua_code.contains("function Rectangle:area()")
            || lua_code.contains("function Rectangle.area(self)"),
        "Should generate method"
    );
    assert!(
        lua_code.contains("self.width = width"),
        "Should initialize width"
    );
    assert!(
        lua_code.contains("self.height = height"),
        "Should initialize height"
    );
}

#[test]
fn test_codegen_empty_primary_constructor() {
    let source = r#"
        class Empty() {
        }
    "#;

    let lua_code = compile_to_lua(source).expect("Should compile successfully");

    // Should still generate constructor methods even with no parameters
    assert!(
        lua_code.contains("function Empty._init(self)"),
        "Should generate _init with just self"
    );
    assert!(
        lua_code.contains("function Empty.new()"),
        "Should generate new with no parameters"
    );
}

#[test]
fn test_codegen_metatable_setup() {
    let source = r#"
        class Point(public x: number, public y: number) {
        }
    "#;

    let lua_code = compile_to_lua(source).expect("Should compile successfully");

    // Verify proper metatable setup in new method
    assert!(
        lua_code.contains("local self = setmetatable({}, Point)"),
        "Should create instance with metatable"
    );
    assert!(lua_code.contains("return self"), "Should return instance");
}
