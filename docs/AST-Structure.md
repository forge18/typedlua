# TypedLua AST Structure

**Document Version:** 0.1  
**Last Updated:** 2024-12-31

This document defines the Abstract Syntax Tree (AST) structure for TypedLua using Rust types.

---

## Design Principles

1. **Type Safety** - Use Rust's type system to prevent invalid AST construction
2. **Span Tracking** - Every node tracks its source location for error reporting
3. **Immutability** - AST nodes are immutable after construction
4. **Arena Allocation** - Large ASTs use arena allocators for performance
5. **Pattern Matching** - Enums enable exhaustive pattern matching

---

## Core Types

```rust
// ast/mod.rs

use std::sync::Arc;

/// Source code location for error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Span { start, end, line, column }
    }
    
    pub fn combine(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line.min(other.line),
            column: self.column,
        }
    }
}

/// Wrapper for AST nodes with span information
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Spanned { node, span }
    }
}

/// Identifier
pub type Ident = Spanned<String>;

/// Top-level program
#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
    pub span: Span,
}
```

---

## Statements

```rust
// ast/statement.rs

#[derive(Debug, Clone)]
pub enum Statement {
    Variable(VariableDeclaration),
    Function(FunctionDeclaration),
    Class(ClassDeclaration),
    Interface(InterfaceDeclaration),
    TypeAlias(TypeAliasDeclaration),
    Enum(EnumDeclaration),
    Import(ImportDeclaration),
    Export(ExportDeclaration),
    If(IfStatement),
    While(WhileStatement),
    For(ForStatement),
    Return(ReturnStatement),
    Break(Span),
    Continue(Span),
    Expression(Expression),
    Block(Block),
}

#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub kind: VariableKind,
    pub pattern: Pattern,
    pub type_annotation: Option<Type>,
    pub initializer: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableKind {
    Const,
    Local,
}

#[derive(Debug, Clone)]
pub struct FunctionDeclaration {
    pub name: Ident,
    pub type_parameters: Option<Vec<TypeParameter>>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ClassDeclaration {
    pub decorators: Vec<Decorator>,
    pub is_abstract: bool,
    pub name: Ident,
    pub type_parameters: Option<Vec<TypeParameter>>,
    pub extends: Option<Type>,
    pub implements: Vec<Type>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ClassMember {
    Property(PropertyDeclaration),
    Constructor(ConstructorDeclaration),
    Method(MethodDeclaration),
    Getter(GetterDeclaration),
    Setter(SetterDeclaration),
}

#[derive(Debug, Clone)]
pub struct PropertyDeclaration {
    pub decorators: Vec<Decorator>,
    pub access: Option<AccessModifier>,
    pub is_static: bool,
    pub is_readonly: bool,
    pub name: Ident,
    pub type_annotation: Type,
    pub initializer: Option<Expression>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ConstructorDeclaration {
    pub decorators: Vec<Decorator>,
    pub parameters: Vec<Parameter>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MethodDeclaration {
    pub decorators: Vec<Decorator>,
    pub access: Option<AccessModifier>,
    pub is_static: bool,
    pub is_abstract: bool,
    pub name: Ident,
    pub type_parameters: Option<Vec<TypeParameter>>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub body: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct GetterDeclaration {
    pub decorators: Vec<Decorator>,
    pub access: Option<AccessModifier>,
    pub is_static: bool,
    pub name: Ident,
    pub return_type: Type,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct SetterDeclaration {
    pub decorators: Vec<Decorator>,
    pub access: Option<AccessModifier>,
    pub is_static: bool,
    pub name: Ident,
    pub parameter: Parameter,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessModifier {
    Public,
    Private,
    Protected,
}

#[derive(Debug, Clone)]
pub struct InterfaceDeclaration {
    pub name: Ident,
    pub type_parameters: Option<Vec<TypeParameter>>,
    pub extends: Vec<Type>,
    pub members: Vec<InterfaceMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum InterfaceMember {
    Property(PropertySignature),
    Method(MethodSignature),
    Index(IndexSignature),
}

#[derive(Debug, Clone)]
pub struct PropertySignature {
    pub is_readonly: bool,
    pub name: Ident,
    pub is_optional: bool,
    pub type_annotation: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MethodSignature {
    pub name: Ident,
    pub type_parameters: Option<Vec<TypeParameter>>,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IndexSignature {
    pub key_name: Ident,
    pub key_type: IndexKeyType,
    pub value_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexKeyType {
    String,
    Number,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDeclaration {
    pub name: Ident,
    pub type_parameters: Option<Vec<TypeParameter>>,
    pub type_annotation: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDeclaration {
    pub name: Ident,
    pub members: Vec<EnumMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumMember {
    pub name: Ident,
    pub value: Option<EnumValue>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum EnumValue {
    Number(f64),
    String(String),
}

#[derive(Debug, Clone)]
pub struct ImportDeclaration {
    pub clause: ImportClause,
    pub source: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ImportClause {
    Default(Ident),
    Named(Vec<ImportSpecifier>),
    Namespace(Ident),
    TypeOnly(Vec<ImportSpecifier>),
}

#[derive(Debug, Clone)]
pub struct ImportSpecifier {
    pub imported: Ident,
    pub local: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExportDeclaration {
    pub kind: ExportKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExportKind {
    Declaration(Box<Statement>),
    Named(Vec<ExportSpecifier>),
    Default(Expression),
}

#[derive(Debug, Clone)]
pub struct ExportSpecifier {
    pub local: Ident,
    pub exported: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IfStatement {
    pub condition: Expression,
    pub then_block: Block,
    pub else_ifs: Vec<ElseIf>,
    pub else_block: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ElseIf {
    pub condition: Expression,
    pub block: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WhileStatement {
    pub condition: Expression,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ForStatement {
    Numeric(ForNumeric),
    Generic(ForGeneric),
}

#[derive(Debug, Clone)]
pub struct ForNumeric {
    pub variable: Ident,
    pub start: Expression,
    pub end: Expression,
    pub step: Option<Expression>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ForGeneric {
    pub variables: Vec<Ident>,
    pub iterators: Vec<Expression>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ReturnStatement {
    pub values: Vec<Expression>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}
```

---

## Expressions

```rust
// ast/expression.rs

#[derive(Debug, Clone)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExpressionKind {
    Identifier(String),
    Literal(Literal),
    Binary(BinaryOp, Box<Expression>, Box<Expression>),
    Unary(UnaryOp, Box<Expression>),
    Assignment(Box<Expression>, AssignmentOp, Box<Expression>),
    Member(Box<Expression>, Ident),
    Index(Box<Expression>, Box<Expression>),
    Call(Box<Expression>, Vec<Argument>),
    MethodCall(Box<Expression>, Ident, Vec<Argument>),
    Array(Vec<ArrayElement>),
    Object(Vec<ObjectProperty>),
    Function(FunctionExpression),
    Arrow(ArrowFunction),
    Conditional(Box<Expression>, Box<Expression>, Box<Expression>),
    Pipe(Box<Expression>, Box<Expression>),
    Match(MatchExpression),
    Parenthesized(Box<Expression>),
    SelfKeyword,
    SuperKeyword,
    Template(TemplateLiteral),
    TypeAssertion(Box<Expression>, Type),
}

#[derive(Debug, Clone)]
pub enum Literal {
    Nil,
    Boolean(bool),
    Number(f64),
    Integer(i64),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add, Subtract, Multiply, Divide, Modulo, IntegerDivide, Power,
    Equal, NotEqual, LessThan, LessThanOrEqual, GreaterThan, GreaterThanOrEqual,
    And, Or,
    Concatenate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not, Negate, Length,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOp {
    Assign, AddAssign, SubtractAssign, MultiplyAssign, DivideAssign, ModuloAssign, ConcatenateAssign,
}

#[derive(Debug, Clone)]
pub struct Argument {
    pub value: Expression,
    pub is_spread: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ArrayElement {
    Expression(Expression),
    Spread(Expression),
}

#[derive(Debug, Clone)]
pub enum ObjectProperty {
    Property { key: Ident, value: Expression, span: Span },
    Computed { key: Expression, value: Expression, span: Span },
    Spread { value: Expression, span: Span },
}

#[derive(Debug, Clone)]
pub struct FunctionExpression {
    pub type_parameters: Option<Vec<TypeParameter>>,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArrowFunction {
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub body: ArrowBody,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ArrowBody {
    Expression(Box<Expression>),
    Block(Block),
}

#[derive(Debug, Clone)]
pub struct MatchExpression {
    pub value: Box<Expression>,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expression>,
    pub body: MatchArmBody,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum MatchArmBody {
    Expression(Expression),
    Block(Block),
}

#[derive(Debug, Clone)]
pub struct TemplateLiteral {
    pub parts: Vec<TemplatePart>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TemplatePart {
    String(String),
    Expression(Expression),
}
```

---

## Patterns

```rust
// ast/pattern.rs

#[derive(Debug, Clone)]
pub enum Pattern {
    Identifier(Ident),
    Literal(Literal, Span),
    Array(ArrayPattern),
    Object(ObjectPattern),
    Wildcard(Span),
}

#[derive(Debug, Clone)]
pub struct ArrayPattern {
    pub elements: Vec<ArrayPatternElement>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ArrayPatternElement {
    Pattern(Pattern),
    Rest(Ident),
    Hole,
}

#[derive(Debug, Clone)]
pub struct ObjectPattern {
    pub properties: Vec<ObjectPatternProperty>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ObjectPatternProperty {
    pub key: Ident,
    pub value: Option<Pattern>,
    pub default: Option<Expression>,
    pub span: Span,
}
```

---

## Types

```rust
// ast/types.rs

#[derive(Debug, Clone)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    Primitive(PrimitiveType),
    Reference(TypeReference),
    Union(Vec<Type>),
    Intersection(Vec<Type>),
    Object(ObjectType),
    Array(Box<Type>),
    Tuple(Vec<Type>),
    Function(FunctionType),
    Literal(Literal),
    TypeQuery(Box<Expression>),
    KeyOf(Box<Type>),
    IndexAccess(Box<Type>, Box<Type>),
    Conditional(ConditionalType),
    Mapped(MappedType),
    TemplateLiteral(TemplateLiteralType),
    Nullable(Box<Type>),
    Parenthesized(Box<Type>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    Nil, Boolean, Number, Integer, String, Unknown, Never, Void, Table, Coroutine,
}

#[derive(Debug, Clone)]
pub struct TypeReference {
    pub name: Ident,
    pub type_arguments: Option<Vec<Type>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ObjectType {
    pub members: Vec<ObjectTypeMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ObjectTypeMember {
    Property(PropertySignature),
    Method(MethodSignature),
    Index(IndexSignature),
}

#[derive(Debug, Clone)]
pub struct FunctionType {
    pub parameters: Vec<Parameter>,
    pub return_type: Box<Type>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ConditionalType {
    pub check_type: Box<Type>,
    pub extends_type: Box<Type>,
    pub true_type: Box<Type>,
    pub false_type: Box<Type>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MappedType {
    pub is_readonly: bool,
    pub type_parameter: TypeParameter,
    pub in_type: Box<Type>,
    pub is_optional: bool,
    pub value_type: Box<Type>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TemplateLiteralType {
    pub parts: Vec<TemplateLiteralTypePart>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TemplateLiteralTypePart {
    String(String),
    Type(Type),
}

#[derive(Debug, Clone)]
pub struct TypeParameter {
    pub name: Ident,
    pub constraint: Option<Type>,
    pub default: Option<Type>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub pattern: Pattern,
    pub type_annotation: Option<Type>,
    pub default: Option<Expression>,
    pub is_rest: bool,
    pub span: Span,
}
```

---

## Decorators

```rust
// ast/decorator.rs

#[derive(Debug, Clone)]
pub struct Decorator {
    pub expression: DecoratorExpression,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum DecoratorExpression {
    Identifier(Ident),
    Call { callee: Box<DecoratorExpression>, arguments: Vec<Expression>, span: Span },
    Member { object: Box<DecoratorExpression>, property: Ident, span: Span },
}
```

---

## Visitors

```rust
// ast/visitor.rs

pub trait Visitor: Sized {
    fn visit_program(&mut self, program: &Program) { walk_program(self, program); }
    fn visit_statement(&mut self, statement: &Statement) { walk_statement(self, statement); }
    fn visit_expression(&mut self, expression: &Expression) { walk_expression(self, expression); }
    fn visit_type(&mut self, ty: &Type) { walk_type(self, ty); }
}

pub trait VisitorMut: Sized {
    fn visit_program(&mut self, program: &mut Program) { walk_program_mut(self, program); }
    fn visit_statement(&mut self, statement: &mut Statement) { walk_statement_mut(self, statement); }
    fn visit_expression(&mut self, expression: &mut Expression) { walk_expression_mut(self, expression); }
    fn visit_type(&mut self, ty: &mut Type) { walk_type_mut(self, ty); }
}

pub fn walk_program<V: Visitor>(visitor: &mut V, program: &Program) {
    for statement in &program.statements {
        visitor.visit_statement(statement);
    }
}

pub fn walk_statement<V: Visitor>(visitor: &mut V, statement: &Statement) {
    match statement {
        Statement::Variable(decl) => {
            if let Some(ty) = &decl.type_annotation {
                visitor.visit_type(ty);
            }
            visitor.visit_expression(&decl.initializer);
        }
        Statement::Function(decl) => {
            for param in &decl.parameters {
                if let Some(ty) = &param.type_annotation {
                    visitor.visit_type(ty);
                }
            }
            if let Some(ty) = &decl.return_type {
                visitor.visit_type(ty);
            }
            for stmt in &decl.body.statements {
                visitor.visit_statement(stmt);
            }
        }
        _ => {}
    }
}
```

---

## Pretty Printing

```rust
// ast/display.rs

use std::fmt;

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for statement in &self.statements {
            writeln!(f, "{}", statement)?;
        }
        Ok(())
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Statement::Variable(decl) => {
                write!(f, "{} {} = {}", 
                    if decl.kind == VariableKind::Const { "const" } else { "local" },
                    decl.pattern, decl.initializer)
            }
            Statement::Function(decl) => {
                write!(f, "function {}(", decl.name.node)?;
                for (i, param) in decl.parameters.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", param)?;
                }
                write!(f, ")")?;
                if let Some(ret) = &decl.return_type {
                    write!(f, ": {}", ret)?;
                }
                write!(f, " {{ ... }}")
            }
            _ => write!(f, "<statement>")
        }
    }
}
```

---

## Usage Example

```rust
use typedlua_ast::*;

fn example() {
    let var_decl = Statement::Variable(VariableDeclaration {
        kind: VariableKind::Const,
        pattern: Pattern::Identifier(Ident::new("x".to_string(), Span::new(6, 7, 1, 7))),
        type_annotation: Some(Type {
            kind: TypeKind::Primitive(PrimitiveType::Number),
            span: Span::new(9, 15, 1, 10),
        }),
        initializer: Expression {
            kind: ExpressionKind::Literal(Literal::Number(42.0)),
            span: Span::new(18, 20, 1, 19),
        },
        span: Span::new(0, 20, 1, 1),
    });
    
    let program = Program {
        statements: vec![var_decl],
        span: Span::new(0, 20, 1, 1),
    };
    
    println!("{:?}", program);
}
```

---

## Memory Management

```rust
use bumpalo::Bump;

pub struct Arena {
    bump: Bump,
}

impl Arena {
    pub fn new() -> Self {
        Arena { bump: Bump::new() }
    }
    
    pub fn alloc<T>(&self, value: T) -> &T {
        self.bump.alloc(value)
    }
}
```

---

## Testing Helpers

```rust
#[cfg(test)]
pub mod testing {
    use super::*;
    
    pub fn dummy_span() -> Span {
        Span::new(0, 0, 1, 1)
    }
    
    pub fn test_ident(name: &str) -> Ident {
        Ident::new(name.to_string(), dummy_span())
    }
    
    pub fn num_expr(value: f64) -> Expression {
        Expression {
            kind: ExpressionKind::Literal(Literal::Number(value)),
            span: dummy_span(),
        }
    }
}
```

---

**Document Version:** 0.1  
**Last Updated:** 2024-12-31
