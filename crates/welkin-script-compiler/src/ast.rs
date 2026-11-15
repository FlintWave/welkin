// SPDX-License-Identifier: LGPL-3.0-or-later
//! LSL Abstract Syntax Tree types.

use crate::LslType;
use crate::lexer::Span;

/// A complete LSL script: global declarations, global functions, and states.
#[derive(Debug, Clone)]
pub struct Script {
    pub globals: Vec<GlobalVar>,
    pub functions: Vec<FuncDecl>,
    pub states: Vec<State>,
}

/// A global variable declaration.
#[derive(Debug, Clone)]
pub struct GlobalVar {
    pub ty: LslType,
    pub name: String,
    pub init: Option<Expr>,
    pub span: Span,
}

/// A function declaration (global or event handler).
#[derive(Debug, Clone)]
pub struct FuncDecl {
    pub return_type: LslType,
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// A function/event parameter.
#[derive(Debug, Clone)]
pub struct Param {
    pub ty: LslType,
    pub name: String,
    pub span: Span,
}

/// A state block: `default { ... }` or `state name { ... }`.
#[derive(Debug, Clone)]
pub struct State {
    pub name: String,
    pub events: Vec<EventHandler>,
    pub span: Span,
}

/// An event handler inside a state.
#[derive(Debug, Clone)]
pub struct EventHandler {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// LSL statements.
#[derive(Debug, Clone)]
pub enum Stmt {
    Block(Vec<Stmt>, Span),
    VarDecl {
        ty: LslType,
        name: String,
        init: Option<Expr>,
        span: Span,
    },
    If {
        cond: Expr,
        then_body: Box<Stmt>,
        else_body: Option<Box<Stmt>>,
        span: Span,
    },
    For {
        init: Vec<Expr>,
        cond: Option<Expr>,
        update: Vec<Expr>,
        body: Box<Stmt>,
        span: Span,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
        span: Span,
    },
    DoWhile {
        body: Box<Stmt>,
        cond: Expr,
        span: Span,
    },
    Jump {
        label: String,
        span: Span,
    },
    Label {
        name: String,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    StateChange {
        target: String,
        span: Span,
    },
    Expression(Expr, Span),
    Empty(Span),
}

/// LSL expressions.
#[derive(Debug, Clone)]
pub enum Expr {
    IntegerLit(i32, Span),
    FloatLit(f64, Span),
    StringLit(String, Span),
    Ident(String, Span),
    VectorLit {
        x: Box<Expr>,
        y: Box<Expr>,
        z: Box<Expr>,
        span: Span,
    },
    RotationLit {
        x: Box<Expr>,
        y: Box<Expr>,
        z: Box<Expr>,
        s: Box<Expr>,
        span: Span,
    },
    ListLit(Vec<Expr>, Span),
    BinaryOp {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    PostfixOp {
        op: PostfixOp,
        operand: Box<Expr>,
        span: Span,
    },
    Assignment {
        op: AssignOp,
        target: Box<Expr>,
        value: Box<Expr>,
        span: Span,
    },
    FuncCall {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },
    MemberAccess {
        object: Box<Expr>,
        member: String,
        span: Span,
    },
    TypeCast {
        target_type: LslType,
        expr: Box<Expr>,
        span: Span,
    },
    Paren(Box<Expr>, Span),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntegerLit(_, s)
            | Expr::FloatLit(_, s)
            | Expr::StringLit(_, s)
            | Expr::Ident(_, s)
            | Expr::VectorLit { span: s, .. }
            | Expr::RotationLit { span: s, .. }
            | Expr::ListLit(_, s)
            | Expr::BinaryOp { span: s, .. }
            | Expr::UnaryOp { span: s, .. }
            | Expr::PostfixOp { span: s, .. }
            | Expr::Assignment { span: s, .. }
            | Expr::FuncCall { span: s, .. }
            | Expr::MemberAccess { span: s, .. }
            | Expr::TypeCast { span: s, .. }
            | Expr::Paren(_, s) => *s,
        }
    }
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    LogicAnd,
    LogicOr,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// Unary (prefix) operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    PreInc,
    PreDec,
}

/// Postfix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostfixOp {
    PostInc,
    PostDec,
}

/// Assignment operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
}
