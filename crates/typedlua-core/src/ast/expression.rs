use super::{pattern::Pattern, statement::TypeParameter, types::Type, Ident};
use crate::span::Span;

use super::statement::{Block, Parameter};

#[derive(Debug, Clone)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub span: Span,
}

impl Expression {
    pub fn new(kind: ExpressionKind, span: Span) -> Self {
        Expression { kind, span }
    }
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

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Nil,
    Boolean(bool),
    Number(f64),
    Integer(i64),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    IntegerDivide,
    Power,
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    And,
    Or,
    Concatenate,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    ShiftLeft,
    ShiftRight,
    Instanceof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Negate,
    Length,
    BitwiseNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOp {
    Assign,              // =
    AddAssign,           // +=
    SubtractAssign,      // -=
    MultiplyAssign,      // *=
    DivideAssign,        // /=
    ModuloAssign,        // %=
    PowerAssign,         // ^=
    ConcatenateAssign,   // ..=
    BitwiseAndAssign,    // &=
    BitwiseOrAssign,     // |=
    FloorDivideAssign,   // //=
    LeftShiftAssign,     // <<=
    RightShiftAssign,    // >>=
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
    Property {
        key: Ident,
        value: Expression,
        span: Span,
    },
    Computed {
        key: Expression,
        value: Expression,
        span: Span,
    },
    Spread {
        value: Expression,
        span: Span,
    },
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
