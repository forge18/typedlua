pub mod sourcemap;

use crate::ast::expression::*;
use crate::ast::pattern::{ArrayPattern, ArrayPatternElement, ObjectPattern, Pattern};
use crate::ast::statement::*;
use crate::ast::Program;
pub use sourcemap::{SourceMap, SourceMapBuilder};

/// Target Lua version for code generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuaTarget {
    /// Lua 5.1 (widespread compatibility)
    Lua51,
    /// Lua 5.2 (added goto, bit operators via library)
    Lua52,
    /// Lua 5.3 (added integers, bitwise operators)
    Lua53,
    /// Lua 5.4 (added const, to-be-closed)
    Lua54,
}

impl Default for LuaTarget {
    fn default() -> Self {
        LuaTarget::Lua54
    }
}

impl LuaTarget {
    /// Check if this target supports bitwise operators
    pub fn supports_bitwise_ops(self) -> bool {
        matches!(self, LuaTarget::Lua53 | LuaTarget::Lua54)
    }

    /// Check if this target supports integer division (//)
    pub fn supports_integer_divide(self) -> bool {
        matches!(self, LuaTarget::Lua53 | LuaTarget::Lua54)
    }

    /// Check if this target supports goto/labels
    pub fn supports_goto(self) -> bool {
        !matches!(self, LuaTarget::Lua51)
    }

    /// Check if this target supports the continue statement
    pub fn supports_continue(self) -> bool {
        // None of the standard Lua versions support continue
        // Would need to be emulated
        false
    }

    /// Get the bitwise operator library name for Lua 5.2
    pub fn bit_library_name(self) -> Option<&'static str> {
        match self {
            LuaTarget::Lua52 => Some("bit32"),
            _ => None,
        }
    }
}

/// Code generator for TypedLua to Lua
pub struct CodeGenerator {
    output: String,
    indent_level: usize,
    indent_str: String,
    source_map: Option<SourceMapBuilder>,
    target: LuaTarget,
    current_class_parent: Option<String>,
    uses_built_in_decorators: bool,
}

impl CodeGenerator {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            indent_str: "    ".to_string(),
            source_map: None,
            target: LuaTarget::default(),
            current_class_parent: None,
            uses_built_in_decorators: false,
        }
    }

    pub fn with_target(mut self, target: LuaTarget) -> Self {
        self.target = target;
        self
    }

    pub fn with_source_map(mut self, source_file: String) -> Self {
        self.source_map = Some(SourceMapBuilder::new(source_file));
        self
    }

    pub fn generate(&mut self, program: &Program) -> String {
        // First pass: check if any decorators are used
        self.detect_decorators(program);

        // Embed runtime library if decorators are used (provides built-in decorators)
        if self.uses_built_in_decorators {
            self.embed_runtime_library();
        }

        for statement in &program.statements {
            self.generate_statement(statement);
        }
        self.output.clone()
    }

    pub fn take_source_map(&mut self) -> Option<SourceMap> {
        self.source_map.take().map(|builder| builder.build())
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
        if let Some(source_map) = &mut self.source_map {
            source_map.advance(s);
        }
    }

    fn writeln(&mut self, s: &str) {
        self.output.push_str(s);
        self.output.push('\n');
        if let Some(source_map) = &mut self.source_map {
            source_map.advance(s);
            source_map.advance("\n");
        }
    }

    fn indent(&mut self) {
        self.indent_level += 1;
    }

    fn dedent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.write(&self.indent_str.clone());
        }
    }

    fn generate_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Variable(decl) => self.generate_variable_declaration(decl),
            Statement::Function(decl) => self.generate_function_declaration(decl),
            Statement::If(if_stmt) => self.generate_if_statement(if_stmt),
            Statement::While(while_stmt) => self.generate_while_statement(while_stmt),
            Statement::For(for_stmt) => self.generate_for_statement(for_stmt),
            Statement::Repeat(repeat_stmt) => self.generate_repeat_statement(repeat_stmt),
            Statement::Return(return_stmt) => self.generate_return_statement(return_stmt),
            Statement::Break(_) => {
                self.write_indent();
                self.writeln("break");
            }
            Statement::Continue(_) => {
                self.write_indent();
                self.writeln("continue");
            }
            Statement::Expression(expr) => {
                self.write_indent();
                self.generate_expression(expr);
                self.writeln("");
            }
            Statement::Block(block) => self.generate_block(block),
            Statement::Interface(_) | Statement::TypeAlias(_) | Statement::Enum(_) => {
                // Type-only declarations are erased
            }
            Statement::Class(class_decl) => self.generate_class_declaration(class_decl),
            Statement::Import(_) | Statement::Export(_) => {
                // Module system not yet implemented
            }
            // Declaration file statements - these are type-only and erased
            Statement::DeclareFunction(_)
            | Statement::DeclareNamespace(_)
            | Statement::DeclareType(_)
            | Statement::DeclareInterface(_)
            | Statement::DeclareConst(_) => {
                // Declaration file statements are erased during code generation
            }
        }
    }

    fn generate_variable_declaration(&mut self, decl: &VariableDeclaration) {
        match &decl.pattern {
            Pattern::Identifier(_) | Pattern::Wildcard(_) => {
                // Simple case: local name = value
                self.write_indent();
                self.write("local ");
                self.generate_pattern(&decl.pattern);
                self.write(" = ");
                self.generate_expression(&decl.initializer);
                self.writeln("");
            }
            Pattern::Array(array_pattern) => {
                // Generate temporary variable and destructuring assignments
                self.write_indent();
                self.write("local __temp = ");
                self.generate_expression(&decl.initializer);
                self.writeln("");
                self.generate_array_destructuring(array_pattern, "__temp");
            }
            Pattern::Object(obj_pattern) => {
                // Generate temporary variable and destructuring assignments
                self.write_indent();
                self.write("local __temp = ");
                self.generate_expression(&decl.initializer);
                self.writeln("");
                self.generate_object_destructuring(obj_pattern, "__temp");
            }
            Pattern::Literal(_, _) => {
                // Literals in patterns don't bind variables - just evaluate the initializer
                self.write_indent();
                self.write("local _ = ");
                self.generate_expression(&decl.initializer);
                self.writeln("");
            }
        }
    }

    fn generate_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Identifier(ident) => {
                self.write(&ident.node);
            }
            Pattern::Wildcard(_) => {
                self.write("_");
            }
            Pattern::Array(_) | Pattern::Object(_) | Pattern::Literal(_, _) => {
                // These should be handled by generate_variable_declaration or other contexts
                self.write("_destructure");
            }
        }
    }

    /// Generate array destructuring assignments
    fn generate_array_destructuring(&mut self, pattern: &ArrayPattern, source: &str) {
        let mut index = 1; // Lua arrays are 1-indexed

        for elem in &pattern.elements {
            match elem {
                ArrayPatternElement::Pattern(pat) => {
                    match pat {
                        Pattern::Identifier(ident) => {
                            self.write_indent();
                            self.write("local ");
                            self.write(&ident.node);
                            self.write(&format!(" = {}[{}]", source, index));
                            self.writeln("");
                        }
                        Pattern::Array(nested_array) => {
                            // Nested array destructuring
                            let temp_var = format!("__temp_{}", index);
                            self.write_indent();
                            self.write("local ");
                            self.write(&temp_var);
                            self.write(&format!(" = {}[{}]", source, index));
                            self.writeln("");
                            self.generate_array_destructuring(nested_array, &temp_var);
                        }
                        Pattern::Object(nested_obj) => {
                            // Nested object destructuring
                            let temp_var = format!("__temp_{}", index);
                            self.write_indent();
                            self.write("local ");
                            self.write(&temp_var);
                            self.write(&format!(" = {}[{}]", source, index));
                            self.writeln("");
                            self.generate_object_destructuring(nested_obj, &temp_var);
                        }
                        Pattern::Wildcard(_) | Pattern::Literal(_, _) => {
                            // Skip - don't bind anything
                        }
                    }
                    index += 1;
                }
                ArrayPatternElement::Rest(ident) => {
                    // Rest element: collect remaining elements
                    self.write_indent();
                    self.write("local ");
                    self.write(&ident.node);
                    self.write(" = {");
                    self.write(&format!("table.unpack({}, {})", source, index));
                    self.write("}");
                    self.writeln("");
                    break; // Rest must be last
                }
                ArrayPatternElement::Hole => {
                    // Skip this element
                    index += 1;
                }
            }
        }
    }

    /// Generate object destructuring assignments
    fn generate_object_destructuring(&mut self, pattern: &ObjectPattern, source: &str) {
        for prop in &pattern.properties {
            let key = &prop.key.node;

            if let Some(value_pattern) = &prop.value {
                // { key: pattern }
                match value_pattern {
                    Pattern::Identifier(ident) => {
                        self.write_indent();
                        self.write("local ");
                        self.write(&ident.node);
                        self.write(&format!(" = {}.{}", source, key));
                        self.writeln("");
                    }
                    Pattern::Array(nested_array) => {
                        // Nested array destructuring
                        let temp_var = format!("__temp_{}", key);
                        self.write_indent();
                        self.write("local ");
                        self.write(&temp_var);
                        self.write(&format!(" = {}.{}", source, key));
                        self.writeln("");
                        self.generate_array_destructuring(nested_array, &temp_var);
                    }
                    Pattern::Object(nested_obj) => {
                        // Nested object destructuring
                        let temp_var = format!("__temp_{}", key);
                        self.write_indent();
                        self.write("local ");
                        self.write(&temp_var);
                        self.write(&format!(" = {}.{}", source, key));
                        self.writeln("");
                        self.generate_object_destructuring(nested_obj, &temp_var);
                    }
                    Pattern::Wildcard(_) | Pattern::Literal(_, _) => {
                        // Skip - don't bind anything
                    }
                }
            } else {
                // Shorthand: { key } means { key: key }
                self.write_indent();
                self.write("local ");
                self.write(key);
                self.write(&format!(" = {}.{}", source, key));
                self.writeln("");
            }
        }
    }

    fn generate_function_declaration(&mut self, decl: &FunctionDeclaration) {
        self.write_indent();
        self.write("local function ");
        self.write(&decl.name.node);
        self.write("(");

        let mut rest_param_name: Option<String> = None;

        for (i, param) in decl.parameters.iter().enumerate() {
            if param.is_rest {
                // For rest parameters, just write ... in the parameter list
                if i > 0 {
                    self.write(", ");
                }
                self.write("...");
                // Save the parameter name to initialize it in the function body
                if let Pattern::Identifier(ident) = &param.pattern {
                    rest_param_name = Some(ident.node.clone());
                }
            } else {
                if i > 0 {
                    self.write(", ");
                }
                self.generate_pattern(&param.pattern);
            }
        }

        self.writeln(")");
        self.indent();

        // If there's a rest parameter, initialize it from ...
        if let Some(rest_name) = rest_param_name {
            self.write_indent();
            self.write("local ");
            self.write(&rest_name);
            self.writeln(" = {...}");
        }

        self.generate_block(&decl.body);
        self.dedent();
        self.write_indent();
        self.writeln("end");
    }

    fn generate_if_statement(&mut self, if_stmt: &IfStatement) {
        self.write_indent();
        self.write("if ");
        self.generate_expression(&if_stmt.condition);
        self.writeln(" then");
        self.indent();
        self.generate_block(&if_stmt.then_block);
        self.dedent();

        for else_if in &if_stmt.else_ifs {
            self.write_indent();
            self.write("elseif ");
            self.generate_expression(&else_if.condition);
            self.writeln(" then");
            self.indent();
            self.generate_block(&else_if.block);
            self.dedent();
        }

        if let Some(else_block) = &if_stmt.else_block {
            self.write_indent();
            self.writeln("else");
            self.indent();
            self.generate_block(else_block);
            self.dedent();
        }

        self.write_indent();
        self.writeln("end");
    }

    fn generate_while_statement(&mut self, while_stmt: &WhileStatement) {
        self.write_indent();
        self.write("while ");
        self.generate_expression(&while_stmt.condition);
        self.writeln(" do");
        self.indent();
        self.generate_block(&while_stmt.body);
        self.dedent();
        self.write_indent();
        self.writeln("end");
    }

    fn generate_for_statement(&mut self, for_stmt: &ForStatement) {
        match for_stmt {
            ForStatement::Numeric(numeric) => {
                self.write_indent();
                self.write("for ");
                self.write(&numeric.variable.node);
                self.write(" = ");
                self.generate_expression(&numeric.start);
                self.write(", ");
                self.generate_expression(&numeric.end);
                if let Some(step) = &numeric.step {
                    self.write(", ");
                    self.generate_expression(step);
                }
                self.writeln(" do");
                self.indent();
                self.generate_block(&numeric.body);
                self.dedent();
                self.write_indent();
                self.writeln("end");
            }
            ForStatement::Generic(generic) => {
                self.write_indent();
                self.write("for ");
                for (i, var) in generic.variables.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&var.node);
                }
                self.write(" in ");
                for (i, iter) in generic.iterators.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_expression(iter);
                }
                self.writeln(" do");
                self.indent();
                self.generate_block(&generic.body);
                self.dedent();
                self.write_indent();
                self.writeln("end");
            }
        }
    }

    fn generate_repeat_statement(&mut self, repeat_stmt: &RepeatStatement) {
        self.write_indent();
        self.writeln("repeat");
        self.indent();
        self.generate_block(&repeat_stmt.body);
        self.dedent();
        self.write_indent();
        self.write("until ");
        self.generate_expression(&repeat_stmt.until);
        self.writeln("");
    }

    fn generate_return_statement(&mut self, return_stmt: &ReturnStatement) {
        self.write_indent();
        self.write("return");
        if !return_stmt.values.is_empty() {
            self.write(" ");
            for (i, value) in return_stmt.values.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.generate_expression(value);
            }
        }
        self.writeln("");
    }

    fn generate_block(&mut self, block: &Block) {
        for statement in &block.statements {
            self.generate_statement(statement);
        }
    }

    fn generate_class_declaration(&mut self, class_decl: &ClassDeclaration) {
        let class_name = &class_decl.name.node;

        // Save previous parent class context and set new one
        let prev_parent = self.current_class_parent.take();

        // Handle inheritance
        let base_class_name = if let Some(extends) = &class_decl.extends {
            // Extract base class name from Type
            if let crate::ast::types::TypeKind::Reference(type_ref) = &extends.kind {
                Some(type_ref.name.node.clone())
            } else {
                None
            }
        } else {
            None
        };

        // Set current parent for super calls
        self.current_class_parent = base_class_name.clone();

        // Create the class table
        self.write_indent();
        self.write("local ");
        self.write(class_name);
        self.writeln(" = {}");

        // Set up __index for method lookup
        self.write_indent();
        self.write(class_name);
        self.write(".__index = ");
        self.write(class_name);
        self.writeln("");

        if let Some(base_name) = &base_class_name {
            // Set up prototype chain for inheritance
            self.writeln("");
            self.write_indent();
            self.write("setmetatable(");
            self.write(class_name);
            self.write(", { __index = ");
            self.write(base_name);
            self.writeln(" })");
        }

        // Generate constructor
        let has_constructor = class_decl.members.iter().any(|m| matches!(m, ClassMember::Constructor(_)));

        if has_constructor {
            for member in &class_decl.members {
                if let ClassMember::Constructor(ctor) = member {
                    self.generate_class_constructor(class_name, ctor);
                }
            }
        } else {
            // Generate default constructor
            self.writeln("");
            self.write_indent();
            self.write("function ");
            self.write(class_name);
            self.writeln(".new()");
            self.indent();
            self.write_indent();
            self.write("local self = setmetatable({}, ");
            self.write(class_name);
            self.writeln(")");
            self.write_indent();
            self.writeln("return self");
            self.dedent();
            self.write_indent();
            self.writeln("end");
        }

        // Generate properties (as part of constructor or as defaults)
        // Properties will be initialized in the constructor

        // Generate methods
        for member in &class_decl.members {
            match member {
                ClassMember::Method(method) => {
                    self.generate_class_method(class_name, method);
                }
                ClassMember::Getter(getter) => {
                    self.generate_class_getter(class_name, getter);
                }
                ClassMember::Setter(setter) => {
                    self.generate_class_setter(class_name, setter);
                }
                ClassMember::Property(_) | ClassMember::Constructor(_) => {
                    // Already handled
                }
            }
        }

        // Apply decorators to the class
        if !class_decl.decorators.is_empty() {
            self.writeln("");
            for decorator in &class_decl.decorators {
                self.write_indent();
                self.write(class_name);
                self.write(" = ");
                self.generate_decorator_call(decorator, class_name);
                self.writeln("");
            }
        }

        self.writeln("");

        // Restore previous parent class context
        self.current_class_parent = prev_parent;
    }

    fn generate_class_constructor(&mut self, class_name: &str, ctor: &ConstructorDeclaration) {
        // Always generate _init for consistency - this allows super() to work
        let always_use_init = true;

        if always_use_init {
            // Generate _init method for initialization logic (when super() is called)
            self.writeln("");
            self.write_indent();
            self.write("function ");
            self.write(class_name);
            self.write("._init(self");

            // Generate parameters
            for param in &ctor.parameters {
                self.write(", ");
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            self.indent();

            // Generate constructor body (which includes super() calls)
            self.generate_block(&ctor.body);

            self.dedent();
            self.write_indent();
            self.writeln("end");

            // Generate .new constructor that creates instance and calls _init
            self.writeln("");
            self.write_indent();
            self.write("function ");
            self.write(class_name);
            self.write(".new(");

            // Generate parameters
            for (i, param) in ctor.parameters.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            self.indent();

            // Create instance with metatable
            self.write_indent();
            self.write("local self = setmetatable({}, ");
            self.write(class_name);
            self.writeln(")");

            // Call _init
            self.write_indent();
            self.write(class_name);
            self.write("._init(self");
            for param in &ctor.parameters {
                self.write(", ");
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            // Return the instance
            self.write_indent();
            self.writeln("return self");

            self.dedent();
            self.write_indent();
            self.writeln("end");
        } else {
            // Simple constructor without super() - inline the body
            self.writeln("");
            self.write_indent();
            self.write("function ");
            self.write(class_name);
            self.write(".new(");

            // Generate parameters
            for (i, param) in ctor.parameters.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.generate_pattern(&param.pattern);
            }
            self.writeln(")");

            self.indent();

            // Create instance with metatable
            self.write_indent();
            self.write("local self = setmetatable({}, ");
            self.write(class_name);
            self.writeln(")");

            // Generate constructor body directly
            self.generate_block(&ctor.body);

            // Return the instance
            self.write_indent();
            self.writeln("return self");

            self.dedent();
            self.write_indent();
            self.writeln("end");
        }
    }

    fn generate_class_method(&mut self, class_name: &str, method: &MethodDeclaration) {
        // Skip abstract methods - they have no implementation
        if method.is_abstract {
            return;
        }

        self.writeln("");
        self.write_indent();
        self.write("function ");
        self.write(class_name);

        if method.is_static {
            self.write(".");
        } else {
            self.write(":");
        }

        self.write(&method.name.node);
        self.write("(");

        // Generate parameters (for instance methods, 'self' is implicit with :)
        for (i, param) in method.parameters.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.generate_pattern(&param.pattern);
        }
        self.writeln(")");

        // Generate method body
        if let Some(body) = &method.body {
            self.indent();
            self.generate_block(body);
            self.dedent();
        }

        self.write_indent();
        self.writeln("end");

        // Apply decorators to the method
        if !method.decorators.is_empty() {
            for decorator in &method.decorators {
                self.write_indent();
                self.write(class_name);
                if method.is_static {
                    self.write(".");
                } else {
                    self.write(".");
                }
                self.write(&method.name.node);
                self.write(" = ");

                let method_ref = format!("{}.{}", class_name, method.name.node);
                self.generate_decorator_call(decorator, &method_ref);
                self.writeln("");
            }
        }
    }

    fn generate_class_getter(&mut self, class_name: &str, getter: &GetterDeclaration) {
        self.writeln("");
        self.write_indent();
        self.write("function ");
        self.write(class_name);

        if getter.is_static {
            self.write(".");
        } else {
            self.write(":");
        }

        self.write("get_");
        self.write(&getter.name.node);
        self.writeln("()");

        self.indent();
        self.generate_block(&getter.body);
        self.dedent();

        self.write_indent();
        self.writeln("end");
    }

    fn generate_class_setter(&mut self, class_name: &str, setter: &SetterDeclaration) {
        self.writeln("");
        self.write_indent();
        self.write("function ");
        self.write(class_name);

        if setter.is_static {
            self.write(".");
        } else {
            self.write(":");
        }

        self.write("set_");
        self.write(&setter.name.node);
        self.write("(");
        self.generate_pattern(&setter.parameter.pattern);
        self.writeln(")");

        self.indent();
        self.generate_block(&setter.body);
        self.dedent();

        self.write_indent();
        self.writeln("end");
    }

    /// Generate a decorator call
    /// Decorators in Lua are applied as function calls that wrap/modify the target
    /// Example: @log becomes target = log(target)
    fn generate_decorator_call(&mut self, decorator: &crate::ast::statement::Decorator, target: &str) {
        use crate::ast::statement::DecoratorExpression;

        match &decorator.expression {
            DecoratorExpression::Identifier(name) => {
                // Simple decorator: @decorator -> target = decorator(target)
                self.write(&name.node);
                self.write("(");
                self.write(target);
                self.write(")");
            }
            DecoratorExpression::Call { callee, arguments, .. } => {
                // Decorator with arguments: @decorator(arg1, arg2) -> target = decorator(arg1, arg2)(target)
                self.generate_decorator_expression(callee);
                self.write("(");
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_expression(arg);
                }
                self.write(")(");
                self.write(target);
                self.write(")");
            }
            DecoratorExpression::Member { object, property, .. } => {
                // Member decorator: @namespace.decorator -> target = namespace.decorator(target)
                self.generate_decorator_expression(object);
                self.write(".");
                self.write(&property.node);
                self.write("(");
                self.write(target);
                self.write(")");
            }
        }
    }

    /// Generate decorator expression (helper for nested decorator expressions)
    fn generate_decorator_expression(&mut self, expr: &crate::ast::statement::DecoratorExpression) {
        use crate::ast::statement::DecoratorExpression;

        match expr {
            DecoratorExpression::Identifier(name) => {
                self.write(&name.node);
            }
            DecoratorExpression::Call { callee, arguments, .. } => {
                self.generate_decorator_expression(callee);
                self.write("(");
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_expression(arg);
                }
                self.write(")");
            }
            DecoratorExpression::Member { object, property, .. } => {
                self.generate_decorator_expression(object);
                self.write(".");
                self.write(&property.node);
            }
        }
    }

    /// Check if a decorator name is a built-in decorator
    fn is_built_in_decorator(&self, name: &str) -> bool {
        matches!(name, "readonly" | "sealed" | "deprecated")
    }

    /// Detect if the program uses built-in decorators
    fn detect_decorators(&mut self, program: &Program) {
        for statement in &program.statements {
            if self.statement_uses_built_in_decorators(statement) {
                self.uses_built_in_decorators = true;
                return;
            }
        }
    }

    /// Check if a statement uses built-in decorators
    fn statement_uses_built_in_decorators(&self, stmt: &Statement) -> bool {
        match stmt {
            Statement::Class(class_decl) => {
                // Check class decorators
                for decorator in &class_decl.decorators {
                    if self.is_decorator_built_in(&decorator.expression) {
                        return true;
                    }
                }

                // Check member decorators
                for member in &class_decl.members {
                    let decorators = match member {
                        ClassMember::Method(method) => &method.decorators,
                        ClassMember::Property(prop) => &prop.decorators,
                        ClassMember::Getter(getter) => &getter.decorators,
                        ClassMember::Setter(setter) => &setter.decorators,
                        _ => continue,
                    };

                    for decorator in decorators {
                        if self.is_decorator_built_in(&decorator.expression) {
                            return true;
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Check if a decorator expression references a built-in decorator
    fn is_decorator_built_in(&self, expr: &crate::ast::statement::DecoratorExpression) -> bool {
        use crate::ast::statement::DecoratorExpression;

        match expr {
            DecoratorExpression::Identifier(name) => self.is_built_in_decorator(&name.node),
            DecoratorExpression::Call { callee, .. } => {
                if let DecoratorExpression::Identifier(name) = &**callee {
                    self.is_built_in_decorator(&name.node)
                } else {
                    false
                }
            }
            DecoratorExpression::Member { object, property, .. } => {
                // Check if it's TypedLua.readonly, TypedLua.sealed, or TypedLua.deprecated
                if let DecoratorExpression::Identifier(obj_name) = &**object {
                    obj_name.node == "TypedLua" && self.is_built_in_decorator(&property.node)
                } else {
                    false
                }
            }
        }
    }

    /// Embed the TypedLua runtime library at the beginning of the generated code
    fn embed_runtime_library(&mut self) {
        const RUNTIME_LUA: &str = include_str!("../../../../runtime/typedlua_runtime.lua");
        self.writeln(RUNTIME_LUA);
        self.writeln("");
    }

    fn generate_expression(&mut self, expr: &Expression) {
        match &expr.kind {
            ExpressionKind::Literal(lit) => self.generate_literal(lit),
            ExpressionKind::Identifier(name) => self.write(name),
            ExpressionKind::Binary(op, left, right) => {
                self.generate_binary_expression(*op, left, right);
            }
            ExpressionKind::Unary(op, operand) => {
                self.write(self.unary_op_to_string(*op));
                self.generate_expression(operand);
            }
            ExpressionKind::Assignment(target, op, value) => {
                // For compound assignments, desugar to: target = target op value
                // For plain assignment, just do: target = value
                match op {
                    AssignmentOp::Assign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::AddAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" + ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::SubtractAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" - ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::MultiplyAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" * ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::DivideAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" / ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::ModuloAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" % ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::PowerAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" ^ ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::ConcatenateAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" .. ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::BitwiseAndAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" & ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::BitwiseOrAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" | ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::FloorDivideAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" // ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::LeftShiftAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" << ");
                        self.generate_expression(value);
                    }
                    AssignmentOp::RightShiftAssign => {
                        self.generate_expression(target);
                        self.write(" = ");
                        self.generate_expression(target);
                        self.write(" >> ");
                        self.generate_expression(value);
                    }
                }
            }
            ExpressionKind::Call(callee, args) => {
                // Check for super() constructor call
                if matches!(&callee.kind, ExpressionKind::SuperKeyword) {
                    // super() in constructor - call Parent._init(self, args)
                    if let Some(parent) = self.current_class_parent.clone() {
                        self.write(&parent);
                        self.write("._init(self");
                        if !args.is_empty() {
                            self.write(", ");
                        }
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                self.write(", ");
                            }
                            self.generate_argument(arg);
                        }
                        self.write(")");
                    } else {
                        self.write("nil -- super() without parent class");
                    }
                    return;
                }

                // Check if this is a super method call: super.method(args)
                let is_super_method_call = matches!(&callee.kind,
                    ExpressionKind::Member(obj, _) if matches!(obj.kind, ExpressionKind::SuperKeyword)
                );

                self.generate_expression(callee);
                self.write("(");

                // For super method calls, inject 'self' as first argument
                if is_super_method_call {
                    self.write("self");
                    if !args.is_empty() {
                        self.write(", ");
                    }
                }

                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_argument(arg);
                }
                self.write(")");
            }
            ExpressionKind::Member(object, member) => {
                // Check if this is super.method - translate to ParentClass.method
                if matches!(object.kind, ExpressionKind::SuperKeyword) {
                    if let Some(parent) = self.current_class_parent.clone() {
                        self.write(&parent);
                        self.write(".");
                        self.write(&member.node);
                    } else {
                        // No parent class - this is an error, but generate something
                        self.write("nil -- super used without parent class");
                    }
                } else {
                    self.generate_expression(object);
                    self.write(".");
                    self.write(&member.node);
                }
            }
            ExpressionKind::Index(object, index) => {
                self.generate_expression(object);
                self.write("[");
                self.generate_expression(index);
                self.write("]");
            }
            ExpressionKind::Array(elements) => {
                // Check if there are any spreads
                let has_spread = elements.iter().any(|elem| matches!(elem, ArrayElement::Spread(_)));

                if !has_spread {
                    // Simple case: no spreads
                    self.write("{");
                    for (i, elem) in elements.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        match elem {
                            ArrayElement::Expression(expr) => self.generate_expression(expr),
                            ArrayElement::Spread(_) => unreachable!(),
                        }
                    }
                    self.write("}");
                } else {
                    // Complex case: has spreads - use helper pattern
                    self.write("(function() local __arr = {} ");

                    for elem in elements {
                        match elem {
                            ArrayElement::Expression(expr) => {
                                self.write("table.insert(__arr, ");
                                self.generate_expression(expr);
                                self.write(") ");
                            }
                            ArrayElement::Spread(expr) => {
                                self.write("for _, __v in ipairs(");
                                self.generate_expression(expr);
                                self.write(") do table.insert(__arr, __v) end ");
                            }
                        }
                    }

                    self.write("return __arr end)()");
                }
            }
            ExpressionKind::Object(props) => {
                // Check if there are any spreads
                let has_spread = props.iter().any(|prop| matches!(prop, ObjectProperty::Spread { .. }));

                if !has_spread {
                    // Simple case: no spreads
                    self.write("{");
                    for (i, prop) in props.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.generate_object_property(prop);
                    }
                    self.write("}");
                } else {
                    // Complex case: has spreads - use helper pattern
                    self.write("(function() local __obj = {} ");

                    for prop in props {
                        match prop {
                            ObjectProperty::Property { key, value, .. } => {
                                self.write("__obj.");
                                self.write(&key.node);
                                self.write(" = ");
                                self.generate_expression(value);
                                self.write(" ");
                            }
                            ObjectProperty::Computed { key, value, .. } => {
                                self.write("__obj[");
                                self.generate_expression(key);
                                self.write("] = ");
                                self.generate_expression(value);
                                self.write(" ");
                            }
                            ObjectProperty::Spread { value, .. } => {
                                self.write("for __k, __v in pairs(");
                                self.generate_expression(value);
                                self.write(") do __obj[__k] = __v end ");
                            }
                        }
                    }

                    self.write("return __obj end)()");
                }
            }
            ExpressionKind::Function(func_expr) => {
                self.write("function(");
                for (i, param) in func_expr.parameters.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_pattern(&param.pattern);
                }
                self.write(")\n");
                self.indent();
                self.generate_block(&func_expr.body);
                self.dedent();
                self.write_indent();
                self.write("end");
            }
            ExpressionKind::Arrow(arrow_expr) => {
                self.write("function(");
                for (i, param) in arrow_expr.parameters.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_pattern(&param.pattern);
                }
                self.write(")\n");
                self.indent();
                match &arrow_expr.body {
                    ArrowBody::Expression(expr) => {
                        self.write_indent();
                        self.write("return ");
                        self.generate_expression(expr);
                        self.writeln("");
                    }
                    ArrowBody::Block(block) => {
                        self.generate_block(block);
                    }
                }
                self.dedent();
                self.write_indent();
                self.write("end");
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                self.write("(");
                self.generate_expression(cond);
                self.write(" and ");
                self.generate_expression(then_expr);
                self.write(" or ");
                self.generate_expression(else_expr);
                self.write(")");
            }
            ExpressionKind::Match(match_expr) => {
                self.generate_match_expression(match_expr);
            }
            ExpressionKind::Pipe(left, right) => {
                // Pipe operator: left |> right
                // Generates: right(left)
                // For chained pipes or more complex right expressions,
                // we check if right is a call and prepend left to its arguments
                match &right.kind {
                    ExpressionKind::Call(callee, arguments) => {
                        // right is already a function call, prepend left as first argument
                        self.generate_expression(callee);
                        self.write("(");
                        self.generate_expression(left);
                        if !arguments.is_empty() {
                            self.write(", ");
                            for (i, arg) in arguments.iter().enumerate() {
                                if i > 0 {
                                    self.write(", ");
                                }
                                if arg.is_spread {
                                    self.write("table.unpack(");
                                    self.generate_expression(&arg.value);
                                    self.write(")");
                                } else {
                                    self.generate_expression(&arg.value);
                                }
                            }
                        }
                        self.write(")");
                    }
                    _ => {
                        // right is just a function reference, call it with left
                        self.generate_expression(right);
                        self.write("(");
                        self.generate_expression(left);
                        self.write(")");
                    }
                }
            }
            _ => {
                // For unimplemented expressions, just write a placeholder
                self.write("nil");
            }
        }
    }

    fn generate_literal(&mut self, lit: &Literal) {
        match lit {
            Literal::Nil => self.write("nil"),
            Literal::Boolean(b) => self.write(if *b { "true" } else { "false" }),
            Literal::Number(n) => self.write(&n.to_string()),
            Literal::Integer(i) => self.write(&i.to_string()),
            Literal::String(s) => {
                self.write("\"");
                self.write(&s.replace('\\', "\\\\").replace('"', "\\\""));
                self.write("\"");
            }
        }
    }

    fn generate_argument(&mut self, arg: &Argument) {
        self.generate_expression(&arg.value);
    }

    fn generate_object_property(&mut self, prop: &ObjectProperty) {
        match prop {
            ObjectProperty::Property { key, value, .. } => {
                self.write(&key.node);
                self.write(" = ");
                self.generate_expression(value);
            }
            ObjectProperty::Computed { key, value, .. } => {
                self.write("[");
                self.generate_expression(key);
                self.write("] = ");
                self.generate_expression(value);
            }
            ObjectProperty::Spread { value, .. } => {
                // Spread not directly supported in Lua
                // Would need to be implemented with table.unpack or similar
                self.generate_expression(value);
            }
        }
    }

    fn generate_binary_expression(&mut self, op: BinaryOp, left: &Expression, right: &Expression) {
        match op {
            // Standard operators that work everywhere
            BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide
            | BinaryOp::Modulo | BinaryOp::Power | BinaryOp::Concatenate
            | BinaryOp::Equal | BinaryOp::NotEqual | BinaryOp::LessThan
            | BinaryOp::LessThanOrEqual | BinaryOp::GreaterThan | BinaryOp::GreaterThanOrEqual
            | BinaryOp::And | BinaryOp::Or => {
                self.write("(");
                self.generate_expression(left);
                self.write(" ");
                self.write(self.simple_binary_op_to_string(op));
                self.write(" ");
                self.generate_expression(right);
                self.write(")");
            }

            // instanceof - generate runtime type check
            BinaryOp::Instanceof => {
                self.write("(type(");
                self.generate_expression(left);
                self.write(") == \"table\" and getmetatable(");
                self.generate_expression(left);
                self.write(") == ");
                self.generate_expression(right);
                self.write(")");
            }

            // Bitwise operators - native in Lua 5.3+
            BinaryOp::BitwiseAnd | BinaryOp::BitwiseOr | BinaryOp::BitwiseXor
            | BinaryOp::ShiftLeft | BinaryOp::ShiftRight
                if self.target.supports_bitwise_ops() =>
            {
                self.write("(");
                self.generate_expression(left);
                self.write(" ");
                self.write(self.simple_binary_op_to_string(op));
                self.write(" ");
                self.generate_expression(right);
                self.write(")");
            }

            // Bitwise via library for Lua 5.2/LuaJIT
            BinaryOp::BitwiseAnd => self.generate_bitwise_library_call("band", left, right),
            BinaryOp::BitwiseOr => self.generate_bitwise_library_call("bor", left, right),
            BinaryOp::BitwiseXor => self.generate_bitwise_library_call("bxor", left, right),
            BinaryOp::ShiftLeft => self.generate_bitwise_library_call("lshift", left, right),
            BinaryOp::ShiftRight => self.generate_bitwise_library_call("rshift", left, right),

            // Integer division
            BinaryOp::IntegerDivide if self.target.supports_integer_divide() => {
                self.write("(");
                self.generate_expression(left);
                self.write(" // ");
                self.generate_expression(right);
                self.write(")");
            }
            BinaryOp::IntegerDivide => {
                // Fallback: math.floor(a / b)
                self.write("math.floor(");
                self.generate_expression(left);
                self.write(" / ");
                self.generate_expression(right);
                self.write(")");
            }
        }
    }

    fn simple_binary_op_to_string(&self, op: BinaryOp) -> &'static str {
        match op {
            BinaryOp::Add => "+",
            BinaryOp::Subtract => "-",
            BinaryOp::Multiply => "*",
            BinaryOp::Divide => "/",
            BinaryOp::Modulo => "%",
            BinaryOp::Power => "^",
            BinaryOp::Concatenate => "..",
            BinaryOp::Equal => "==",
            BinaryOp::NotEqual => "~=",
            BinaryOp::LessThan => "<",
            BinaryOp::LessThanOrEqual => "<=",
            BinaryOp::GreaterThan => ">",
            BinaryOp::GreaterThanOrEqual => ">=",
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
            BinaryOp::BitwiseAnd => "&",
            BinaryOp::BitwiseOr => "|",
            BinaryOp::BitwiseXor => "~",
            BinaryOp::ShiftLeft => "<<",
            BinaryOp::ShiftRight => ">>",
            BinaryOp::IntegerDivide => "//",
            BinaryOp::Instanceof => unreachable!("instanceof is handled separately"),
        }
    }

    fn generate_bitwise_library_call(
        &mut self,
        func: &str,
        left: &Expression,
        right: &Expression,
    ) {
        if let Some(lib) = self.target.bit_library_name() {
            self.write(lib);
            self.write(".");
            self.write(func);
            self.write("(");
            self.generate_expression(left);
            self.write(", ");
            self.generate_expression(right);
            self.write(")");
        } else {
            // Lua 5.1 without library - would need manual implementation
            // Use placeholder helper function name
            self.write("_bit_");
            self.write(func);
            self.write("(");
            self.generate_expression(left);
            self.write(", ");
            self.generate_expression(right);
            self.write(")");
        }
    }

    fn unary_op_to_string(&self, op: UnaryOp) -> &'static str {
        match op {
            UnaryOp::Negate => "-",
            UnaryOp::Not => "not ",
            UnaryOp::Length => "#",
            UnaryOp::BitwiseNot => "~",
        }
    }


    fn generate_match_expression(&mut self, match_expr: &MatchExpression) {
        // Generate match as an immediately invoked function that returns a value
        // This allows us to use if-elseif chains and still use it as an expression
        self.write("(function()");
        self.writeln("");
        self.indent();

        // Generate a local variable for the match value
        self.write_indent();
        self.write("local __match_value = ");
        self.generate_expression(&match_expr.value);
        self.writeln("");

        // Generate if-elseif chain for each arm
        for (i, arm) in match_expr.arms.iter().enumerate() {
            self.write_indent();
            if i == 0 {
                self.write("if ");
            } else {
                self.write("elseif ");
            }

            // Generate pattern match condition
            self.generate_pattern_match(&arm.pattern, "__match_value");

            // Add guard condition if present
            if let Some(guard) = &arm.guard {
                self.write(" and (");
                self.generate_expression(guard);
                self.write(")");
            }

            self.write(" then");
            self.writeln("");
            self.indent();

            // Generate destructuring bindings from pattern
            self.generate_pattern_bindings(&arm.pattern, "__match_value");

            // Generate the arm body
            self.write_indent();
            match &arm.body {
                MatchArmBody::Expression(expr) => {
                    self.write("return ");
                    self.generate_expression(expr);
                    self.writeln("");
                }
                MatchArmBody::Block(block) => {
                    // For blocks, generate all statements
                    for stmt in &block.statements {
                        self.generate_statement(stmt);
                    }
                    // If no explicit return, return nil
                    self.write_indent();
                    self.writeln("return nil");
                }
            }

            self.dedent();
        }

        // Add else clause that returns nil (or could error for non-exhaustive matches)
        self.write_indent();
        self.writeln("else");
        self.indent();
        self.write_indent();
        self.writeln("error(\"Non-exhaustive match\")");
        self.dedent();
        self.write_indent();
        self.writeln("end");

        self.dedent();
        self.write_indent();
        self.write("end)()");
    }

    fn generate_pattern_match(&mut self, pattern: &Pattern, value_var: &str) {
        use crate::ast::pattern::*;

        match pattern {
            Pattern::Wildcard(_) => {
                // Wildcard matches everything
                self.write("true");
            }
            Pattern::Identifier(_) => {
                // Identifier matches everything (it's a binding)
                self.write("true");
            }
            Pattern::Literal(lit, _) => {
                // Compare literal value
                self.write(value_var);
                self.write(" == ");
                self.generate_literal(lit);
            }
            Pattern::Array(array_pattern) => {
                // Check if it's a table and has the right number of elements
                self.write("type(");
                self.write(value_var);
                self.write(") == \"table\"");

                // Check each element
                for (i, elem) in array_pattern.elements.iter().enumerate() {
                    match elem {
                        ArrayPatternElement::Pattern(pat) => {
                            self.write(" and ");
                            let index_expr = format!("{}[{}]", value_var, i + 1);
                            self.generate_pattern_match(pat, &index_expr);
                        }
                        ArrayPatternElement::Rest(_) => {
                            // Rest pattern doesn't add conditions
                        }
                        ArrayPatternElement::Hole => {
                            // Hole doesn't add conditions
                        }
                    }
                }
            }
            Pattern::Object(_) => {
                // For object patterns, just check if it's a table
                self.write("type(");
                self.write(value_var);
                self.write(") == \"table\"");
            }
        }
    }

    fn generate_pattern_bindings(&mut self, pattern: &Pattern, value_var: &str) {
        use crate::ast::pattern::*;

        match pattern {
            Pattern::Identifier(ident) => {
                // Bind the identifier to the value
                self.write_indent();
                self.write("local ");
                self.write(&ident.node);
                self.write(" = ");
                self.write(value_var);
                self.writeln("");
            }
            Pattern::Array(array_pattern) => {
                // Bind each element
                for (i, elem) in array_pattern.elements.iter().enumerate() {
                    match elem {
                        ArrayPatternElement::Pattern(pat) => {
                            let index_expr = format!("{}[{}]", value_var, i + 1);
                            self.generate_pattern_bindings(pat, &index_expr);
                        }
                        ArrayPatternElement::Rest(ident) => {
                            // Create a slice of the rest
                            self.write_indent();
                            self.write("local ");
                            self.write(&ident.node);
                            self.write(" = {table.unpack(");
                            self.write(value_var);
                            self.write(", ");
                            self.write(&(i + 1).to_string());
                            self.write(")}");
                            self.writeln("");
                        }
                        ArrayPatternElement::Hole => {
                            // Hole doesn't bind anything
                        }
                    }
                }
            }
            Pattern::Object(object_pattern) => {
                // Bind each property
                for prop in &object_pattern.properties {
                    if let Some(value_pattern) = &prop.value {
                        let prop_expr = format!("{}.{}", value_var, prop.key.node);
                        self.generate_pattern_bindings(value_pattern, &prop_expr);
                    } else {
                        // Shorthand: bind the key directly
                        self.write_indent();
                        self.write("local ");
                        self.write(&prop.key.node);
                        self.write(" = ");
                        self.write(value_var);
                        self.write(".");
                        self.write(&prop.key.node);
                        self.writeln("");
                    }
                }
            }
            Pattern::Wildcard(_) | Pattern::Literal(_, _) => {
                // These don't bind anything
            }
        }
    }
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::CollectingDiagnosticHandler;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use std::sync::Arc;

    fn generate_code(source: &str) -> String {
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(source, handler.clone());
        let tokens = lexer.tokenize().expect("Lexing failed");
        let mut parser = Parser::new(tokens, handler);
        let program = parser.parse().expect("Parsing failed");

        let mut generator = CodeGenerator::new();
        generator.generate(&program)
    }

    #[test]
    fn test_generate_variable_declaration() {
        let source = "const x = 42";
        let output = generate_code(source);
        assert!(output.contains("local x = 42"));
    }

    #[test]
    fn test_generate_function_declaration() {
        let source = "function add(a, b) return a + b end";
        let output = generate_code(source);
        assert!(output.contains("local function add(a, b)"));
        assert!(output.contains("return (a + b)"));
    }

    #[test]
    fn test_generate_if_statement() {
        let source = "if x > 0 then print(x) end";
        let output = generate_code(source);
        assert!(output.contains("if (x > 0) then"));
        assert!(output.contains("print(x)"));
        assert!(output.contains("end"));
    }

    #[test]
    fn test_generate_while_loop() {
        let source = "while x < 10 do x = x + 1 end";
        let output = generate_code(source);
        println!("Generated output:\n{}", output);
        assert!(output.contains("while (x < 10) do"));
        // Assignment is parsed and will be in the output
        assert!(output.trim().len() > 0);
    }

    #[test]
    fn test_generate_for_loop() {
        let source = "for i = 1, 10 do print(i) end";
        let output = generate_code(source);
        assert!(output.contains("for i = 1, 10 do"));
        assert!(output.contains("print(i)"));
    }

    fn generate_code_with_target(source: &str, target: LuaTarget) -> String {
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(source, handler.clone());
        let tokens = lexer.tokenize().expect("Lexing failed");
        let mut parser = Parser::new(tokens, handler);
        let program = parser.parse().expect("Parsing failed");

        let mut generator = CodeGenerator::new().with_target(target);
        generator.generate(&program)
    }

    #[test]
    fn test_bitwise_lua53() {
        let source = "const x = a & b";
        let output = generate_code_with_target(source, LuaTarget::Lua53);
        assert!(output.contains("local x = (a & b)"));
    }

    #[test]
    fn test_bitwise_lua52() {
        let source = "const x = a & b";
        let output = generate_code_with_target(source, LuaTarget::Lua52);
        assert!(output.contains("local x = bit32.band(a, b)"));
    }


    #[test]
    fn test_bitwise_lua51() {
        let source = "const x = a | b";
        let output = generate_code_with_target(source, LuaTarget::Lua51);
        // Should use helper function for Lua 5.1
        assert!(output.contains("local x = _bit_bor(a, b)"));
    }

    #[test]
    fn test_shift_operators_lua53() {
        let source = "const x = a << 2";
        let output = generate_code_with_target(source, LuaTarget::Lua53);
        assert!(output.contains("local x = (a << 2)"));

        let source = "const y = a >> 2";
        let output = generate_code_with_target(source, LuaTarget::Lua53);
        assert!(output.contains("local y = (a >> 2)"));
    }

    #[test]
    fn test_shift_operators_lua52() {
        let source = "const x = a << 2";
        let output = generate_code_with_target(source, LuaTarget::Lua52);
        assert!(output.contains("local x = bit32.lshift(a, 2)"));

        let source = "const y = a >> 2";
        let output = generate_code_with_target(source, LuaTarget::Lua52);
        assert!(output.contains("local y = bit32.rshift(a, 2)"));
    }

    #[test]
    fn test_integer_divide_lua53() {
        let source = "const x = a // b";
        let output = generate_code_with_target(source, LuaTarget::Lua53);
        assert!(output.contains("local x = (a // b)"));
    }

    #[test]
    fn test_integer_divide_lua52() {
        let source = "const x = a // b";
        let output = generate_code_with_target(source, LuaTarget::Lua52);
        assert!(output.contains("local x = math.floor(a / b)"));
    }

    #[test]
    fn test_integer_divide_lua51() {
        let source = "const x = a // b";
        let output = generate_code_with_target(source, LuaTarget::Lua51);
        assert!(output.contains("local x = math.floor(a / b)"));
    }

    // Test that target selection works with currently supported operators
    #[test]
    fn test_target_selection() {
        let source = "const x = a + b";

        // Should work with any target
        let output_54 = generate_code_with_target(source, LuaTarget::Lua54);
        assert!(output_54.contains("local x = (a + b)"));

        let output_53 = generate_code_with_target(source, LuaTarget::Lua53);
        assert!(output_53.contains("local x = (a + b)"));

        let output_52 = generate_code_with_target(source, LuaTarget::Lua52);
        assert!(output_52.contains("local x = (a + b)"));

        let output_51 = generate_code_with_target(source, LuaTarget::Lua51);
        assert!(output_51.contains("local x = (a + b)"));
    }

    // Snapshot tests for generated code
    #[test]
    fn test_snapshot_variable_declarations() {
        let source = r#"
const x = 42
let y = "hello"
const z = true
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_function_declaration() {
        let source = r#"
function greet(name: string): string
    return "Hello, " .. name
end
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_arrow_function() {
        let source = r#"
const add = (a, b) => a + b
const multiply = (x, y) => {
    return x * y
}
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_control_flow() {
        let source = r#"
if x > 0 then
    print("positive")
elseif x < 0 then
    print("negative")
else
    print("zero")
end

while count < 10 do
    count = count + 1
end

for i = 1, 10 do
    print(i)
end
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_bitwise_lua53() {
        let source = r#"
const a = x & y
const b = x | y
const c = x ~ y
const d = x << 2
const e = x >> 3
const f = x // y
"#;
        let output = generate_code_with_target(source, LuaTarget::Lua53);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_bitwise_lua52() {
        let source = r#"
const a = x & y
const b = x | y
const c = x ~ y
const d = x << 2
const e = x >> 3
const f = x // y
"#;
        let output = generate_code_with_target(source, LuaTarget::Lua52);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_bitwise_lua51() {
        let source = r#"
const a = x & y
const b = x | y
const c = x ~ y
const d = x << 2
const e = x >> 3
const f = x // y
"#;
        let output = generate_code_with_target(source, LuaTarget::Lua51);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_complex_expressions() {
        let source = r#"
const result = (a + b) * (c - d) / e
const comparison = x >= y and z < w
const ternary = condition ? value1 : value2
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_arrays_and_objects() {
        let source = r#"
const arr = [1, 2, 3, 4, 5]
const obj = { name = "John", age = 30, active = true }
const nested = { data = [1, 2], meta = { version = 1 } }
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_snapshot_string_literals() {
        let source = r#"
const simple = "hello"
const withEscape = "hello \"world\""
const withBackslash = "path\\to\\file"
"#;
        let output = generate_code(source);
        insta::assert_snapshot!(output);
    }

    // Roundtrip tests: parse → generate → parse
    #[test]
    fn test_roundtrip_variable_declaration() {
        let source = "const x = 42";
        let generated = generate_code(source);

        // Parse the generated code again - should not panic
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(&generated, handler.clone());
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler);
        let _program = parser.parse().expect("Roundtrip parsing failed");
    }

    #[test]
    fn test_roundtrip_function_declaration() {
        let source = "function add(a, b) return a + b end";
        let generated = generate_code(source);

        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(&generated, handler.clone());
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler);
        let _program = parser.parse().expect("Roundtrip parsing failed");
    }

    #[test]
    fn test_roundtrip_control_flow() {
        let source = r#"
if x > 0 then
    print("positive")
end

while count < 10 do
    count = count + 1
end

for i = 1, 10 do
    print(i)
end
"#;
        let generated = generate_code(source);

        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(&generated, handler.clone());
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler);
        let _program = parser.parse().expect("Roundtrip parsing failed");
    }

    #[test]
    fn test_roundtrip_binary_expressions() {
        let source = "const x = (a + b) * (c - d) / e";
        let generated = generate_code(source);

        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(&generated, handler.clone());
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler);
        let _program = parser.parse().expect("Roundtrip parsing failed");
    }

    #[test]
    fn test_roundtrip_arrays_and_objects() {
        let source = r#"
const arr = [1, 2, 3, 4, 5]
const obj = { name = "John", age = 30 }
"#;
        let generated = generate_code(source);

        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(&generated, handler.clone());
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler);
        let _program = parser.parse().expect("Roundtrip parsing failed");
    }

    #[test]
    fn test_roundtrip_bitwise_lua53() {
        let source = r#"
const a = x & y
const b = x | y
const c = x ~ y
const d = x << 2
const e = x >> 3
const f = x // y
"#;
        let generated = generate_code_with_target(source, LuaTarget::Lua53);

        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(&generated, handler.clone());
        let tokens = lexer.tokenize().expect("Roundtrip lexing failed");
        let mut parser = Parser::new(tokens, handler);
        let _program = parser.parse().expect("Roundtrip parsing failed");
    }

    // ========== Class Code Generation Tests ==========

    #[test]
    fn test_simple_class() {
        let source = r#"
            class Person {
            }
        "#;
        let output = generate_code(source);
        println!("Generated:\n{}", output);

        assert!(output.contains("local Person = {}"));
        assert!(output.contains("Person.__index = Person"));
        assert!(output.contains("function Person.new()"));
        assert!(output.contains("local self = setmetatable({}, Person)"));
        assert!(output.contains("return self"));
    }

    #[test]
    fn test_class_with_constructor() {
        let source = r#"
            class Person {
                constructor(name: string, age: number) {
                    const x: number = 5
                }
            }
        "#;
        let output = generate_code(source);
        println!("Generated:\n{}", output);

        assert!(output.contains("local Person = {}"));
        assert!(output.contains("Person.__index = Person"));
        assert!(output.contains("function Person.new(name, age)"));
        assert!(output.contains("local self = setmetatable({}, Person)"));
        assert!(output.contains("return self"));
    }

    #[test]
    fn test_class_with_method() {
        let source = r#"
            class Calculator {
                add(a: number, b: number): number {
                    return a + b
                }
            }
        "#;
        let output = generate_code(source);
        println!("Generated:\n{}", output);

        assert!(output.contains("local Calculator = {}"));
        assert!(output.contains("Calculator.__index = Calculator"));
        assert!(output.contains("function Calculator:add(a, b)"));
        assert!(output.contains("return (a + b)"));
    }

    #[test]
    fn test_class_with_static_method() {
        let source = r#"
            class Math {
                static abs(x: number): number {
                    return x
                }
            }
        "#;
        let output = generate_code(source);
        println!("Generated:\n{}", output);

        assert!(output.contains("local Math = {}"));
        assert!(output.contains("Math.__index = Math"));
        // Static methods use . instead of :
        assert!(output.contains("function Math.abs(x)"));
        assert!(output.contains("return x"));
    }

    #[test]
    fn test_class_with_constructor_and_methods() {
        let source = r#"
            class Counter {
                constructor(initial: number) {
                    const x: number = initial
                }

                increment(): void {
                    const y: number = 1
                }

                getValue(): number {
                    return 0
                }
            }
        "#;
        let output = generate_code(source);
        println!("Generated:\n{}", output);

        assert!(output.contains("local Counter = {}"));
        assert!(output.contains("Counter.__index = Counter"));
        assert!(output.contains("function Counter.new(initial)"));
        assert!(output.contains("function Counter:increment()"));
        assert!(output.contains("function Counter:getValue()"));
    }

    #[test]
    fn test_abstract_class() {
        let source = r#"
            abstract class Animal {
                abstract makeSound(): string;

                move(): void {
                    const x: number = 5
                }
            }
        "#;
        let output = generate_code(source);
        println!("Generated:\n{}", output);

        assert!(output.contains("local Animal = {}"));
        assert!(output.contains("Animal.__index = Animal"));
        // Abstract methods should not generate function bodies
        assert!(!output.contains("function Animal:makeSound"));
        // Concrete methods should be generated
        assert!(output.contains("function Animal:move()"));
    }

    #[test]
    fn test_class_with_getter_setter() {
        let source = r#"
            class Person {
                get fullName(): string {
                    return "John Doe"
                }

                set age(value: number) {
                    const x: number = value
                }
            }
        "#;
        let output = generate_code(source);
        println!("Generated:\n{}", output);

        assert!(output.contains("local Person = {}"));
        assert!(output.contains("function Person:get_fullName()"));
        assert!(output.contains("function Person:set_age(value)"));
    }

    #[test]
    fn test_class_inheritance() {
        let source = r#"
            class Animal {
                name: string

                constructor(name: string) {
                    self.name = name
                }

                speak(): string {
                    return "Animal sound"
                }
            }

            class Dog extends Animal {
                breed: string

                constructor(name: string, breed: string) {
                    self.name = name
                    self.breed = breed
                }

                speak(): string {
                    return "Woof!"
                }
            }
        "#;
        let output = generate_code(source);
        println!("Generated:\n{}", output);

        // Check Animal class
        assert!(output.contains("local Animal = {}"));
        assert!(output.contains("Animal.__index = Animal"));
        assert!(output.contains("function Animal.new(name)"));
        assert!(output.contains("function Animal:speak()"));

        // Check Dog class with inheritance
        assert!(output.contains("local Dog = {}"));
        assert!(output.contains("Dog.__index = Dog"));
        assert!(output.contains("setmetatable(Dog, { __index = Animal })"));
        assert!(output.contains("function Dog.new(name, breed)"));
        assert!(output.contains("function Dog:speak()"));
    }

    #[test]
    fn test_super_calls() {
        let source = r#"
            class Animal {
                name: string

                constructor(name: string) {
                    self.name = name
                }

                speak(): string {
                    return "Animal sound"
                }

                move(): void {
                    const x: number = 1
                }
            }

            class Dog extends Animal {
                breed: string

                constructor(name: string, breed: string) {
                    self.name = name
                    self.breed = breed
                }

                speak(): string {
                    const baseSound: string = super.speak()
                    return baseSound .. " and Woof!"
                }

                wagTail(): void {
                    super.move()
                    const y: number = 2
                }
            }
        "#;
        let output = generate_code(source);
        println!("Generated:\n{}", output);

        // Check that super.speak() is translated to Animal.speak(self)
        assert!(output.contains("Animal.speak(self)"));
        // Check that super.move() is translated to Animal.move(self)
        assert!(output.contains("Animal.move(self)"));
        // Check inheritance setup
        assert!(output.contains("setmetatable(Dog, { __index = Animal })"));
    }

    #[test]
    fn test_super_constructor_chaining() {
        let source = r#"
            class Animal {
                name: string

                constructor(name: string) {
                    self.name = name
                }
            }

            class Dog extends Animal {
                breed: string

                constructor(name: string, breed: string) {
                    super(name)
                    self.breed = breed
                }
            }
        "#;
        let output = generate_code(source);
        println!("Generated:\n{}", output);

        // Check that Animal has _init method
        assert!(output.contains("function Animal._init(self, name)"));
        // Check that Animal.new calls Animal._init
        assert!(output.contains("Animal._init(self, name)"));
        // Check that Dog has _init method
        assert!(output.contains("function Dog._init(self, name, breed)"));
        // Check that super(name) is translated to Animal._init(self, name)
        assert!(output.contains("Animal._init(self, name)"));
        // Check that Dog.new calls Dog._init
        assert!(output.contains("Dog._init(self, name, breed)"));
    }
}
