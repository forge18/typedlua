use super::{
    expression::Expression, expression::Literal,
    statement::{IndexSignature, MethodSignature, Parameter, PropertySignature, TypeParameter},
    Ident,
};
use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

impl Type {
    pub fn new(kind: TypeKind, span: Span) -> Self {
        Type { kind, span }
    }
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
    Nil,
    Boolean,
    Number,
    Integer,
    String,
    Unknown,
    Never,
    Void,
    Table,
    Coroutine,
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
    pub type_parameter: Box<TypeParameter>,
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
