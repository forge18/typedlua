use super::{expression::Expression, pattern::Pattern, types::Type, Ident};
use crate::span::Span;

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
    Repeat(RepeatStatement),
    Return(ReturnStatement),
    Break(Span),
    Continue(Span),
    Expression(Expression),
    Block(Block),
    // Declaration file statements
    DeclareFunction(DeclareFunctionStatement),
    DeclareNamespace(DeclareNamespaceStatement),
    DeclareType(TypeAliasDeclaration),  // Same as TypeAlias
    DeclareInterface(InterfaceDeclaration),  // Same as Interface
    DeclareConst(DeclareConstStatement),
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
pub struct RepeatStatement {
    pub body: Block,
    pub until: Expression,
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

#[derive(Debug, Clone)]
pub struct TypeParameter {
    pub name: Ident,
    pub constraint: Option<Box<Type>>,
    pub default: Option<Box<Type>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub pattern: Pattern,
    pub type_annotation: Option<Type>,
    pub default: Option<Expression>,
    pub is_rest: bool,
    pub is_optional: bool,  // For optional parameters (parameter?: Type)
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Decorator {
    pub expression: DecoratorExpression,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum DecoratorExpression {
    Identifier(Ident),
    Call {
        callee: Box<DecoratorExpression>,
        arguments: Vec<Expression>,
        span: Span,
    },
    Member {
        object: Box<DecoratorExpression>,
        property: Ident,
        span: Span,
    },
}

// Declaration file-specific statements

#[derive(Debug, Clone)]
pub struct DeclareFunctionStatement {
    pub name: Ident,
    pub type_parameters: Option<Vec<TypeParameter>>,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub is_export: bool,  // For `export function` inside namespaces
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DeclareNamespaceStatement {
    pub name: Ident,
    pub members: Vec<Statement>,  // Can contain export function, export const, etc.
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DeclareConstStatement {
    pub name: Ident,
    pub type_annotation: Type,
    pub is_export: bool,  // For `export const` inside namespaces
    pub span: Span,
}
