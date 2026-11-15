// SPDX-License-Identifier: GPL-3.0-or-later
//! Recursive-descent parser for LSL.
//!
//! Consumes a token stream from [`crate::lexer`] and produces an
//! [`ast::Script`].  Uses precedence climbing (Pratt parsing) for
//! expressions.

use crate::LslType;
use crate::ast::*;
use crate::lexer::{Span, SpannedToken, Token};

/// A parse error with source location.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.span, self.message)
    }
}

/// LSL recursive-descent parser.
pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
    errors: Vec<ParseError>,
    /// When > 0, `>` and `>=` are NOT consumed as comparison operators
    /// (we are inside a `<...>` vector/rotation literal).
    no_angle_depth: usize,
}

const EVENT_NAMES: &[&str] = &[
    "state_entry",
    "state_exit",
    "touch_start",
    "touch",
    "touch_end",
    "collision_start",
    "collision",
    "collision_end",
    "land_collision_start",
    "land_collision",
    "land_collision_end",
    "timer",
    "listen",
    "sensor",
    "no_sensor",
    "control",
    "at_target",
    "not_at_target",
    "at_rot_target",
    "not_at_rot_target",
    "money",
    "email",
    "run_time_permissions",
    "changed",
    "attach",
    "dataserver",
    "moving_start",
    "moving_end",
    "on_rez",
    "object_rez",
    "remote_data",
    "http_response",
    "http_request",
    "link_message",
    "transaction_result",
    "path_update",
    "region_cross",
    "linkset_data",
    "experience_permissions",
    "experience_permissions_denied",
];

fn is_event_name(name: &str) -> bool {
    EVENT_NAMES.contains(&name)
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
            no_angle_depth: 0,
        }
    }

    /// Parse the full token stream into a Script.
    pub fn parse(mut self) -> Result<Script, Vec<ParseError>> {
        let mut globals = Vec::new();
        let mut functions = Vec::new();
        let mut states = Vec::new();

        while !self.at_end() {
            if self.errors.len() > 1000 {
                break;
            }
            if self.check(&Token::KwDefault) {
                match self.parse_state() {
                    Ok(s) => states.push(s),
                    Err(()) => self.synchronize_top(),
                }
            } else if self.check(&Token::KwState) {
                match self.parse_state() {
                    Ok(s) => states.push(s),
                    Err(()) => self.synchronize_top(),
                }
            } else if self.peek().is_type_keyword() {
                let ty_span = self.peek_span();
                let ty = self.parse_type().unwrap();
                if let Token::Ident(name) = self.peek().clone() {
                    self.advance();
                    if self.check(&Token::LParen) {
                        // function declaration
                        match self.parse_func_decl_rest(ty, name, ty_span) {
                            Ok(f) => functions.push(f),
                            Err(()) => self.synchronize_top(),
                        }
                    } else {
                        // global variable
                        match self.parse_global_var_rest(ty, name, ty_span) {
                            Ok(g) => globals.push(g),
                            Err(()) => self.synchronize_top(),
                        }
                    }
                } else {
                    self.error("expected identifier after type");
                    self.synchronize_top();
                }
            } else if let Token::Ident(ref name) = self.peek().clone() {
                if *self.peek_at(1) == Token::LParen {
                    let name = name.clone();
                    let start_span = self.peek_span();
                    self.advance(); // consume identifier
                    match self.parse_func_decl_rest(LslType::Void, name, start_span) {
                        Ok(f) => functions.push(f),
                        Err(()) => self.synchronize_top(),
                    }
                } else {
                    self.error("expected type, 'default', or 'state'");
                    self.synchronize_top();
                }
            } else {
                self.error("expected type, 'default', or 'state'");
                self.synchronize_top();
            }
        }

        if self.errors.is_empty() {
            Ok(Script {
                globals,
                functions,
                states,
            })
        } else {
            Err(self.errors)
        }
    }

    // --- helpers ---

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .map(|t| &t.token)
            .unwrap_or(&Token::Eof)
    }

    fn peek_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or_default()
    }

    fn peek_at(&self, offset: usize) -> &Token {
        self.tokens
            .get(self.pos + offset)
            .map(|t| &t.token)
            .unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> SpannedToken {
        let tok = self
            .tokens
            .get(self.pos)
            .cloned()
            .unwrap_or(SpannedToken {
                token: Token::Eof,
                span: Span::default(),
            });
        if tok.token != Token::Eof {
            self.pos += 1;
        }
        tok
    }

    fn check(&self, token: &Token) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(token)
    }

    fn at_end(&self) -> bool {
        *self.peek() == Token::Eof
    }

    fn expect(&mut self, token: Token) -> Result<SpannedToken, ()> {
        if self.check(&token) {
            Ok(self.advance())
        } else {
            self.error(&format!("expected {}, found {:?}", token_name(&token), self.peek()));
            Err(())
        }
    }

    fn error(&mut self, msg: &str) {
        self.errors.push(ParseError {
            message: msg.to_string(),
            span: self.peek_span(),
        });
    }

    fn synchronize_top(&mut self) {
        while !self.at_end() {
            match self.peek() {
                Token::KwDefault | Token::KwState => return,
                tok if tok.is_type_keyword() => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn synchronize_stmt(&mut self) {
        while !self.at_end() {
            match self.peek() {
                Token::Semi => {
                    self.advance();
                    return;
                }
                Token::RBrace => return,
                Token::KwIf
                | Token::KwFor
                | Token::KwWhile
                | Token::KwDo
                | Token::KwJump
                | Token::KwReturn
                | Token::KwState
                | Token::LBrace => return,
                tok if tok.is_type_keyword() => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn parse_type(&mut self) -> Result<LslType, ()> {
        match self.peek() {
            Token::KwInteger => { self.advance(); Ok(LslType::Integer) }
            Token::KwFloat => { self.advance(); Ok(LslType::Float) }
            Token::KwString => { self.advance(); Ok(LslType::String) }
            Token::KwKey => { self.advance(); Ok(LslType::Key) }
            Token::KwVector => { self.advance(); Ok(LslType::Vector) }
            Token::KwRotation => { self.advance(); Ok(LslType::Rotation) }
            Token::KwList => { self.advance(); Ok(LslType::List) }
            _ => {
                self.error("expected type keyword");
                Err(())
            }
        }
    }


    // --- top-level parsing ---

    fn parse_func_decl_rest(
        &mut self,
        return_type: LslType,
        name: String,
        start_span: Span,
    ) -> Result<FuncDecl, ()> {
        self.expect(Token::LParen)?;
        let params = self.parse_param_list()?;
        self.expect(Token::RParen)?;
        self.expect(Token::LBrace)?;
        let body = self.parse_stmt_list()?;
        self.expect(Token::RBrace)?;
        Ok(FuncDecl {
            return_type,
            name,
            params,
            body,
            span: start_span,
        })
    }

    fn parse_global_var_rest(
        &mut self,
        ty: LslType,
        name: String,
        start_span: Span,
    ) -> Result<GlobalVar, ()> {
        let init = if self.check(&Token::Eq) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(Token::Semi)?;
        Ok(GlobalVar {
            ty,
            name,
            init,
            span: start_span,
        })
    }

    fn parse_param_list(&mut self) -> Result<Vec<Param>, ()> {
        let mut params = Vec::new();
        if self.peek().is_type_keyword() {
            let span = self.peek_span();
            let ty = self.parse_type()?;
            let name = self.expect_ident()?;
            params.push(Param { ty, name, span });
            while self.check(&Token::Comma) {
                self.advance();
                let span = self.peek_span();
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                params.push(Param { ty, name, span });
            }
        }
        Ok(params)
    }

    fn expect_ident(&mut self) -> Result<String, ()> {
        if let Token::Ident(name) = self.peek().clone() {
            self.advance();
            Ok(name)
        } else {
            self.error("expected identifier");
            Err(())
        }
    }

    fn parse_state(&mut self) -> Result<State, ()> {
        let start = self.peek_span();
        let name = if self.check(&Token::KwDefault) {
            self.advance();
            "default".to_string()
        } else {
            self.expect(Token::KwState)?;
            self.expect_ident()?
        };
        self.expect(Token::LBrace)?;

        let mut events = Vec::new();
        while !self.check(&Token::RBrace) && !self.at_end() {
            match self.parse_event_handler() {
                Ok(eh) => events.push(eh),
                Err(()) => self.synchronize_stmt(),
            }
        }
        self.expect(Token::RBrace)?;

        Ok(State {
            name,
            events,
            span: start,
        })
    }

    fn parse_event_handler(&mut self) -> Result<EventHandler, ()> {
        let start = self.peek_span();
        let name = self.expect_ident()?;
        if !is_event_name(&name) {
            self.errors.push(ParseError {
                message: format!("unknown event name: '{name}'"),
                span: start,
            });
        }
        self.expect(Token::LParen)?;
        let params = self.parse_param_list()?;
        self.expect(Token::RParen)?;
        self.expect(Token::LBrace)?;
        let body = self.parse_stmt_list()?;
        self.expect(Token::RBrace)?;
        Ok(EventHandler {
            name,
            params,
            body,
            span: start,
        })
    }

    // --- statements ---

    fn parse_stmt_list(&mut self) -> Result<Vec<Stmt>, ()> {
        let mut stmts = Vec::new();
        while !self.check(&Token::RBrace) && !self.at_end() {
            if self.errors.len() > 1000 {
                break;
            }
            match self.parse_stmt() {
                Ok(s) => stmts.push(s),
                Err(()) => self.synchronize_stmt(),
            }
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ()> {
        let span = self.peek_span();
        match self.peek().clone() {
            Token::LBrace => {
                self.advance();
                let stmts = self.parse_stmt_list()?;
                self.expect(Token::RBrace)?;
                Ok(Stmt::Block(stmts, span))
            }
            Token::KwIf => self.parse_if_stmt(),
            Token::KwFor => self.parse_for_stmt(),
            Token::KwWhile => self.parse_while_stmt(),
            Token::KwDo => self.parse_do_while_stmt(),
            Token::KwJump => {
                self.advance();
                let label = self.expect_ident()?;
                self.expect(Token::Semi)?;
                Ok(Stmt::Jump { label, span })
            }
            Token::At => {
                self.advance();
                let name = self.expect_ident()?;
                self.expect(Token::Semi)?;
                Ok(Stmt::Label { name, span })
            }
            Token::KwReturn => {
                self.advance();
                let value = if self.check(&Token::Semi) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.expect(Token::Semi)?;
                Ok(Stmt::Return { value, span })
            }
            Token::KwState => {
                self.advance();
                let target = if self.check(&Token::KwDefault) {
                    self.advance();
                    "default".to_string()
                } else {
                    self.expect_ident()?
                };
                self.expect(Token::Semi)?;
                Ok(Stmt::StateChange { target, span })
            }
            Token::Semi => {
                self.advance();
                Ok(Stmt::Empty(span))
            }
            ref tok if tok.is_type_keyword() => {
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                let init = if self.check(&Token::Eq) {
                    self.advance();
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect(Token::Semi)?;
                Ok(Stmt::VarDecl {
                    ty,
                    name,
                    init,
                    span,
                })
            }
            _ => {
                let expr = self.parse_expr()?;
                let s = expr.span();
                self.expect(Token::Semi)?;
                Ok(Stmt::Expression(expr, s))
            }
        }
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt, ()> {
        let span = self.peek_span();
        self.expect(Token::KwIf)?;
        self.expect(Token::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(Token::RParen)?;
        let then_body = Box::new(self.parse_stmt()?);
        let else_body = if self.check(&Token::KwElse) {
            self.advance();
            Some(Box::new(self.parse_stmt()?))
        } else {
            None
        };
        Ok(Stmt::If {
            cond,
            then_body,
            else_body,
            span,
        })
    }

    fn parse_for_stmt(&mut self) -> Result<Stmt, ()> {
        let span = self.peek_span();
        self.expect(Token::KwFor)?;
        self.expect(Token::LParen)?;

        let mut init = Vec::new();
        if !self.check(&Token::Semi) {
            init.push(self.parse_expr()?);
            while self.check(&Token::Comma) {
                self.advance();
                init.push(self.parse_expr()?);
            }
        }
        self.expect(Token::Semi)?;

        let cond = if self.check(&Token::Semi) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(Token::Semi)?;

        let mut update = Vec::new();
        if !self.check(&Token::RParen) {
            update.push(self.parse_expr()?);
            while self.check(&Token::Comma) {
                self.advance();
                update.push(self.parse_expr()?);
            }
        }
        self.expect(Token::RParen)?;

        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::For {
            init,
            cond,
            update,
            body,
            span,
        })
    }

    fn parse_while_stmt(&mut self) -> Result<Stmt, ()> {
        let span = self.peek_span();
        self.expect(Token::KwWhile)?;
        self.expect(Token::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(Token::RParen)?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While { cond, body, span })
    }

    fn parse_do_while_stmt(&mut self) -> Result<Stmt, ()> {
        let span = self.peek_span();
        self.expect(Token::KwDo)?;
        let body = Box::new(self.parse_stmt()?);
        self.expect(Token::KwWhile)?;
        self.expect(Token::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(Token::RParen)?;
        self.expect(Token::Semi)?;
        Ok(Stmt::DoWhile { body, cond, span })
    }

    // --- expressions (Pratt / precedence climbing) ---

    fn parse_expr(&mut self) -> Result<Expr, ()> {
        self.parse_assignment()
    }

    fn parse_expr_no_angle(&mut self) -> Result<Expr, ()> {
        self.no_angle_depth += 1;
        let result = self.parse_assignment();
        self.no_angle_depth -= 1;
        result
    }

    fn parse_assignment(&mut self) -> Result<Expr, ()> {
        let lhs = self.parse_logic_or()?;

        let op = match self.peek() {
            Token::Eq => Some(AssignOp::Assign),
            Token::PlusEq => Some(AssignOp::AddAssign),
            Token::MinusEq => Some(AssignOp::SubAssign),
            Token::StarEq => Some(AssignOp::MulAssign),
            Token::SlashEq => Some(AssignOp::DivAssign),
            Token::PercentEq => Some(AssignOp::ModAssign),
            _ => None,
        };

        if let Some(op) = op {
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_assignment()?; // right-associative
            Ok(Expr::Assignment {
                op,
                target: Box::new(lhs),
                value: Box::new(rhs),
                span,
            })
        } else {
            Ok(lhs)
        }
    }

    fn parse_logic_or(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_logic_and()?;
        while self.check(&Token::PipePipe) {
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_logic_and()?;
            lhs = Expr::BinaryOp {
                op: BinOp::LogicOr,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_logic_and(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_bit_or()?;
        while self.check(&Token::AmpAmp) {
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_bit_or()?;
            lhs = Expr::BinaryOp {
                op: BinOp::LogicAnd,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_bit_or(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_bit_xor()?;
        while self.check(&Token::Pipe) {
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_bit_xor()?;
            lhs = Expr::BinaryOp {
                op: BinOp::BitOr,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_bit_xor(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_bit_and()?;
        while self.check(&Token::Caret) {
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_bit_and()?;
            lhs = Expr::BinaryOp {
                op: BinOp::BitXor,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_bit_and(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_equality()?;
        while self.check(&Token::Amp) {
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_equality()?;
            lhs = Expr::BinaryOp {
                op: BinOp::BitAnd,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_equality(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                Token::EqEq => BinOp::Eq,
                Token::BangEq => BinOp::Ne,
                _ => break,
            };
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_comparison()?;
            lhs = Expr::BinaryOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_shift()?;
        loop {
            let op = match self.peek() {
                Token::Lt => BinOp::Lt,
                Token::Gt if self.no_angle_depth == 0 => BinOp::Gt,
                Token::LtEq => BinOp::Le,
                Token::GtEq if self.no_angle_depth == 0 => BinOp::Ge,
                _ => break,
            };
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_shift()?;
            lhs = Expr::BinaryOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_shift(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                Token::LtLt => BinOp::Shl,
                Token::GtGt => BinOp::Shr,
                _ => break,
            };
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_additive()?;
            lhs = Expr::BinaryOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_additive(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_multiplicative()?;
            lhs = Expr::BinaryOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ()> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_unary()?;
            lhs = Expr::BinaryOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, ()> {
        let span = self.peek_span();
        match self.peek().clone() {
            Token::Minus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                    span,
                })
            }
            Token::Bang => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                    span,
                })
            }
            Token::Tilde => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::BitNot,
                    operand: Box::new(operand),
                    span,
                })
            }
            Token::PlusPlus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::PreInc,
                    operand: Box::new(operand),
                    span,
                })
            }
            Token::MinusMinus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::PreDec,
                    operand: Box::new(operand),
                    span,
                })
            }
            Token::LParen if self.is_type_cast() => {
                self.advance(); // (
                let target_type = self.parse_type()?;
                self.expect(Token::RParen)?;
                let expr = self.parse_unary()?;
                Ok(Expr::TypeCast {
                    target_type,
                    expr: Box::new(expr),
                    span,
                })
            }
            _ => self.parse_postfix(),
        }
    }

    fn is_type_cast(&self) -> bool {
        if !matches!(self.peek(), Token::LParen) {
            return false;
        }
        // Check: ( <type_keyword> )
        self.peek_at(1).is_type_keyword() && matches!(self.peek_at(2), Token::RParen)
    }

    fn parse_postfix(&mut self) -> Result<Expr, ()> {
        let mut expr = self.parse_primary()?;
        loop {
            let span = self.peek_span();
            match self.peek() {
                Token::PlusPlus => {
                    self.advance();
                    expr = Expr::PostfixOp {
                        op: PostfixOp::PostInc,
                        operand: Box::new(expr),
                        span,
                    };
                }
                Token::MinusMinus => {
                    self.advance();
                    expr = Expr::PostfixOp {
                        op: PostfixOp::PostDec,
                        operand: Box::new(expr),
                        span,
                    };
                }
                Token::Dot => {
                    self.advance();
                    let member = self.expect_ident()?;
                    expr = Expr::MemberAccess {
                        object: Box::new(expr),
                        member,
                        span,
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ()> {
        let span = self.peek_span();
        match self.peek().clone() {
            Token::IntegerLit(v) => {
                self.advance();
                Ok(Expr::IntegerLit(v, span))
            }
            Token::FloatLit(v) => {
                self.advance();
                Ok(Expr::FloatLit(v, span))
            }
            Token::StringLit(ref s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::StringLit(s, span))
            }
            Token::Ident(ref name) => {
                let name = name.clone();
                self.advance();
                if self.check(&Token::LParen) {
                    // function call
                    self.advance(); // (
                    let args = self.parse_arg_list()?;
                    self.expect(Token::RParen)?;
                    Ok(Expr::FuncCall { name, args, span })
                } else {
                    Ok(Expr::Ident(name, span))
                }
            }
            Token::Lt => {
                // vector or rotation literal: <expr, expr, expr[, expr]>
                // Use restricted expression parsing that stops at > and ,
                self.advance(); // <
                let e1 = self.parse_expr_no_angle()?;
                self.expect(Token::Comma)?;
                let e2 = self.parse_expr_no_angle()?;
                self.expect(Token::Comma)?;
                let e3 = self.parse_expr_no_angle()?;
                if self.check(&Token::Comma) {
                    self.advance();
                    let e4 = self.parse_expr_no_angle()?;
                    self.expect(Token::Gt)?;
                    Ok(Expr::RotationLit {
                        x: Box::new(e1),
                        y: Box::new(e2),
                        z: Box::new(e3),
                        s: Box::new(e4),
                        span,
                    })
                } else {
                    self.expect(Token::Gt)?;
                    Ok(Expr::VectorLit {
                        x: Box::new(e1),
                        y: Box::new(e2),
                        z: Box::new(e3),
                        span,
                    })
                }
            }
            Token::LBracket => {
                // list literal [expr, expr, ...]
                self.advance();
                let mut items = Vec::new();
                if !self.check(&Token::RBracket) {
                    items.push(self.parse_expr()?);
                    while self.check(&Token::Comma) {
                        self.advance();
                        if self.check(&Token::RBracket) {
                            break;
                        }
                        items.push(self.parse_expr()?);
                    }
                }
                self.expect(Token::RBracket)?;
                Ok(Expr::ListLit(items, span))
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Paren(Box::new(expr), span))
            }
            _ => {
                self.error(&format!("unexpected token in expression: {:?}", self.peek()));
                Err(())
            }
        }
    }

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, ()> {
        let mut args = Vec::new();
        if !self.check(&Token::RParen) {
            args.push(self.parse_expr()?);
            while self.check(&Token::Comma) {
                self.advance();
                args.push(self.parse_expr()?);
            }
        }
        Ok(args)
    }
}

fn token_name(token: &Token) -> &'static str {
    match token {
        Token::LBrace => "'{'",
        Token::RBrace => "'}'",
        Token::LParen => "'('",
        Token::RParen => "')'",
        Token::LBracket => "'['",
        Token::RBracket => "']'",
        Token::Semi => "';'",
        Token::Comma => "','",
        Token::Dot => "'.'",
        Token::At => "'@'",
        Token::Eq => "'='",
        Token::Lt => "'<'",
        Token::Gt => "'>'",
        Token::KwDefault => "'default'",
        Token::KwState => "'state'",
        Token::KwIf => "'if'",
        Token::KwElse => "'else'",
        Token::KwFor => "'for'",
        Token::KwWhile => "'while'",
        Token::KwDo => "'do'",
        Token::KwJump => "'jump'",
        Token::KwReturn => "'return'",
        Token::Eof => "end of file",
        _ => "<token>",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_script(src: &str) -> Result<Script, Vec<ParseError>> {
        let tokens = lex(src).unwrap();
        Parser::new(tokens).parse()
    }

    #[test]
    fn parse_minimal_script() {
        let script = parse_script("default { state_entry() {} }").unwrap();
        assert_eq!(script.states.len(), 1);
        assert_eq!(script.states[0].name, "default");
        assert_eq!(script.states[0].events.len(), 1);
        assert_eq!(script.states[0].events[0].name, "state_entry");
    }

    #[test]
    fn parse_global_var() {
        let script = parse_script("integer x = 42; default { state_entry() {} }").unwrap();
        assert_eq!(script.globals.len(), 1);
        assert_eq!(script.globals[0].name, "x");
    }

    #[test]
    fn parse_global_function() {
        let script =
            parse_script("integer add(integer a, integer b) { return a + b; } default { state_entry() {} }")
                .unwrap();
        assert_eq!(script.functions.len(), 1);
        assert_eq!(script.functions[0].name, "add");
        assert_eq!(script.functions[0].params.len(), 2);
    }

    #[test]
    fn parse_multiple_states() {
        let script = parse_script(
            r#"
            default { state_entry() { state active; } }
            state active { state_entry() { state default; } }
            "#,
        )
        .unwrap();
        assert_eq!(script.states.len(), 2);
        assert_eq!(script.states[0].name, "default");
        assert_eq!(script.states[1].name, "active");
    }

    #[test]
    fn parse_if_else() {
        let script = parse_script(
            r#"default { state_entry() { if (1) { return; } else { return; } } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        assert!(matches!(body[0], Stmt::If { else_body: Some(_), .. }));
    }

    #[test]
    fn parse_for_loop() {
        let script = parse_script(
            r#"default { state_entry() { integer i; for (i = 0; i < 10; i++) { } } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        assert!(matches!(body[1], Stmt::For { .. }));
    }

    #[test]
    fn parse_while_and_do_while() {
        let script = parse_script(
            r#"default { state_entry() { while (1) {} do {} while (1); } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        assert!(matches!(body[0], Stmt::While { .. }));
        assert!(matches!(body[1], Stmt::DoWhile { .. }));
    }

    #[test]
    fn parse_jump_label() {
        let script = parse_script(
            r#"default { state_entry() { jump done; @done; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        assert!(matches!(body[0], Stmt::Jump { .. }));
        assert!(matches!(body[1], Stmt::Label { .. }));
    }

    #[test]
    fn parse_vector_literal() {
        let script = parse_script(
            r#"default { state_entry() { vector v = <1.0, 2.0, 3.0>; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        if let Stmt::VarDecl { init: Some(expr), .. } = &body[0] {
            assert!(matches!(expr, Expr::VectorLit { .. }));
        } else {
            panic!("expected VarDecl with vector init");
        }
    }

    #[test]
    fn parse_rotation_literal() {
        let script = parse_script(
            r#"default { state_entry() { rotation r = <0.0, 0.0, 0.0, 1.0>; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        if let Stmt::VarDecl { init: Some(expr), .. } = &body[0] {
            assert!(matches!(expr, Expr::RotationLit { .. }));
        } else {
            panic!("expected VarDecl with rotation init");
        }
    }

    #[test]
    fn parse_list_literal() {
        let script = parse_script(
            r#"default { state_entry() { list l = [1, "hello", <0.0, 0.0, 0.0>]; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        if let Stmt::VarDecl { init: Some(Expr::ListLit(items, _)), .. } = &body[0] {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected VarDecl with list init");
        }
    }

    #[test]
    fn parse_function_call() {
        let script = parse_script(
            r#"default { state_entry() { llSay(0, "Hello"); } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        if let Stmt::Expression(Expr::FuncCall { name, args, .. }, _) = &body[0] {
            assert_eq!(name, "llSay");
            assert_eq!(args.len(), 2);
        } else {
            panic!("expected function call");
        }
    }

    #[test]
    fn parse_type_cast() {
        let script = parse_script(
            r#"default { state_entry() { string s = (string)42; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        if let Stmt::VarDecl { init: Some(Expr::TypeCast { target_type, .. }), .. } = &body[0] {
            assert_eq!(*target_type, LslType::String);
        } else {
            panic!("expected type cast");
        }
    }

    #[test]
    fn parse_member_access() {
        let script = parse_script(
            r#"default { state_entry() { vector v; float x = v.x; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        if let Stmt::VarDecl { init: Some(Expr::MemberAccess { member, .. }), .. } = &body[1] {
            assert_eq!(member, "x");
        } else {
            panic!("expected member access");
        }
    }

    #[test]
    fn parse_compound_assignment() {
        let script = parse_script(
            r#"default { state_entry() { integer x; x += 1; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        if let Stmt::Expression(Expr::Assignment { op, .. }, _) = &body[1] {
            assert_eq!(*op, AssignOp::AddAssign);
        } else {
            panic!("expected compound assignment");
        }
    }

    #[test]
    fn parse_operator_precedence() {
        let script = parse_script(
            r#"default { state_entry() { integer x = 1 + 2 * 3; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        if let Stmt::VarDecl { init: Some(Expr::BinaryOp { op: BinOp::Add, rhs, .. }), .. } = &body[0] {
            assert!(matches!(**rhs, Expr::BinaryOp { op: BinOp::Mul, .. }));
        } else {
            panic!("expected 1 + (2*3) precedence");
        }
    }

    #[test]
    fn parse_all_event_names() {
        let events = [
            ("state_entry()", "state_entry"),
            ("state_exit()", "state_exit"),
            ("touch_start(integer n)", "touch_start"),
            ("touch(integer n)", "touch"),
            ("touch_end(integer n)", "touch_end"),
            ("timer()", "timer"),
            ("listen(integer ch, string nm, string id, string msg)", "listen"),
            ("money(string id, integer amount)", "money"),
            ("collision_start(integer n)", "collision_start"),
            ("collision(integer n)", "collision"),
            ("collision_end(integer n)", "collision_end"),
            ("sensor(integer n)", "sensor"),
            ("no_sensor()", "no_sensor"),
            ("changed(integer ch)", "changed"),
            ("on_rez(integer sp)", "on_rez"),
            ("link_message(integer sn, integer n, string s, string id)", "link_message"),
            ("http_response(string rid, integer st, list md, string bd)", "http_response"),
        ];
        for (event_src, expected_name) in &events {
            let src = format!("default {{ {event_src} {{}} }}");
            let script = parse_script(&src).unwrap();
            assert_eq!(script.states[0].events[0].name, *expected_name);
        }
    }

    #[test]
    fn parse_nested_expressions() {
        let script = parse_script(
            r#"default { state_entry() { integer x = (1 + 2) * (3 - 4); } }"#,
        )
        .unwrap();
        assert_eq!(script.states[0].events[0].body.len(), 1);
    }

    #[test]
    fn parse_unary_prefix() {
        let script = parse_script(
            r#"default { state_entry() { integer x = -1; integer y = !0; integer z = ~0; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        assert!(matches!(
            &body[0],
            Stmt::VarDecl { init: Some(Expr::UnaryOp { op: UnaryOp::Neg, .. }), .. }
        ));
        assert!(matches!(
            &body[1],
            Stmt::VarDecl { init: Some(Expr::UnaryOp { op: UnaryOp::Not, .. }), .. }
        ));
        assert!(matches!(
            &body[2],
            Stmt::VarDecl { init: Some(Expr::UnaryOp { op: UnaryOp::BitNot, .. }), .. }
        ));
    }

    #[test]
    fn parse_pre_post_increment() {
        let script = parse_script(
            r#"default { state_entry() { integer x; ++x; x++; --x; x--; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        assert!(matches!(
            &body[1],
            Stmt::Expression(Expr::UnaryOp { op: UnaryOp::PreInc, .. }, _)
        ));
        assert!(matches!(
            &body[2],
            Stmt::Expression(Expr::PostfixOp { op: PostfixOp::PostInc, .. }, _)
        ));
        assert!(matches!(
            &body[3],
            Stmt::Expression(Expr::UnaryOp { op: UnaryOp::PreDec, .. }, _)
        ));
        assert!(matches!(
            &body[4],
            Stmt::Expression(Expr::PostfixOp { op: PostfixOp::PostDec, .. }, _)
        ));
    }

    #[test]
    fn parse_state_change_default() {
        let script = parse_script(
            r#"state active { state_entry() { state default; } } default { state_entry() { state active; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        if let Stmt::StateChange { target, .. } = &body[0] {
            assert_eq!(target, "default");
        } else {
            panic!("expected state change to default");
        }
    }

    #[test]
    fn parse_bitwise_operators() {
        let script = parse_script(
            r#"default { state_entry() { integer x = 1 & 2 | 3 ^ 4; } }"#,
        )
        .unwrap();
        assert_eq!(script.states[0].events[0].body.len(), 1);
    }

    #[test]
    fn parse_shift_operators() {
        let script = parse_script(
            r#"default { state_entry() { integer x = 1 << 2; integer y = 8 >> 1; } }"#,
        )
        .unwrap();
        let body = &script.states[0].events[0].body;
        assert!(matches!(
            &body[0],
            Stmt::VarDecl { init: Some(Expr::BinaryOp { op: BinOp::Shl, .. }), .. }
        ));
        assert!(matches!(
            &body[1],
            Stmt::VarDecl { init: Some(Expr::BinaryOp { op: BinOp::Shr, .. }), .. }
        ));
    }

    #[test]
    fn parse_complex_script() {
        let src = r#"
integer gCount = 0;
string gName = "test";

integer factorial(integer n) {
    if (n <= 1) { return 1; }
    return n * factorial(n - 1);
}

default {
    state_entry() {
        llSay(0, "Starting: " + gName);
        gCount = factorial(5);
        llSay(0, (string)gCount);
    }
    touch_start(integer num) {
        ++gCount;
        vector pos = llGetPos();
        float x = pos.x;
        list data = [1, 2, "three"];
        for (gCount = 0; gCount < 10; gCount++) {
            llSay(0, (string)gCount);
        }
        state active;
    }
}

state active {
    state_entry() {
        llSay(0, "Active");
    }
    touch_start(integer num) {
        state default;
    }
}
"#;
        let script = parse_script(src).unwrap();
        assert_eq!(script.globals.len(), 2);
        assert_eq!(script.functions.len(), 1);
        assert_eq!(script.states.len(), 2);
        assert_eq!(script.states[0].events.len(), 2);
        assert_eq!(script.states[1].events.len(), 2);
    }

    #[test]
    fn parse_void_function() {
        let script = parse_script(
            r#"myFunc() { llSay(0, "hello"); } default { state_entry() {} }"#,
        ).unwrap();
        assert_eq!(script.functions.len(), 1);
        assert_eq!(script.functions[0].name, "myFunc");
        assert_eq!(script.functions[0].return_type, LslType::Void);
    }

    #[test]
    fn parse_void_function_with_params() {
        let script = parse_script(
            r#"doStuff(string msg, integer n) { llSay(0, msg); } default { state_entry() {} }"#,
        ).unwrap();
        assert_eq!(script.functions.len(), 1);
        assert_eq!(script.functions[0].name, "doStuff");
        assert_eq!(script.functions[0].params.len(), 2);
        assert_eq!(script.functions[0].return_type, LslType::Void);
    }

    #[test]
    fn parse_void_function_with_typecast_no_oom() {
        // This was the OOM reproducer - a void function with a type cast
        let script = parse_script(
            r#"myFunc() { integer x = 5; llSay(0, (string)x); } default { state_entry() {} }"#,
        ).unwrap();
        assert_eq!(script.functions.len(), 1);
    }

    #[test]
    fn parse_mixed_void_and_typed_functions() {
        let script = parse_script(
            r#"
            integer add(integer a, integer b) { return a + b; }
            sayHello() { llSay(0, "hi"); }
            default { state_entry() {} }
            "#,
        ).unwrap();
        assert_eq!(script.functions.len(), 2);
        assert_eq!(script.functions[0].return_type, LslType::Integer);
        assert_eq!(script.functions[1].return_type, LslType::Void);
    }

    #[test]
    fn parse_error_recovery_bounded() {
        // Even with errors, should complete without OOM
        let result = parse_script("!!! ) ) 5 ; default { state_entry() {} }");
        // May succeed or error, but must not hang
        let _ = result;
    }
}
