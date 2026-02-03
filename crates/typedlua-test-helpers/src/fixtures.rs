//! Test fixtures - source code snippets for testing

/// Simple Lua programs for testing
pub fn simple_program() -> &'static str {
    "local x = 10"
}

pub fn function_program() -> &'static str {
    r#"function add(a: number, b: number): number
    return a + b
end"#
}

pub fn class_program() -> &'static str {
    r#"class Point
    x: number
    y: number
    
    new(x: number, y: number)
        self.x = x
        self.y = y
    end
    
    fn distance(): number
        return math.sqrt(self.x * self.x + self.y * self.y)
    end
end"#
}

pub fn interface_program() -> &'static str {
    r#"interface Drawable
    draw(): void
end"#
}

pub fn type_alias_program() -> &'static str {
    "type Point = { x: number, y: number }"
}

pub fn enum_program() -> &'static str {
    r#"enum Color
    Red
    Green
    Blue
end"#
}

/// Programs with type errors for testing diagnostics
pub fn type_error_assignment() -> &'static str {
    "local x: string = 42"
}

pub fn type_error_call() -> &'static str {
    r#"function greet(name: string): string
    return "Hello, " .. name
end
greet(123)"#
}

/// Programs with syntax errors
pub fn syntax_error_missing_end() -> &'static str {
    "function foo()\n    return 42"
}
