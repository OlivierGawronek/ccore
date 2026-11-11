use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::token::{Span, Token, TokenKind};

type PResult<T> = Result<T, Diagnostic>;

pub fn parse(tokens: Vec<Token>) -> PResult<Program> {
    Parser { tokens, pos: 0 }.parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn peek_nth_kind(&self, n: usize) -> &TokenKind {
        let idx = (self.pos + n).min(self.tokens.len() - 1);
        &self.tokens[idx].kind
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        if tok.kind != TokenKind::Eof {
            self.pos += 1;
        }
        tok
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek_kind() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind) -> PResult<Token> {
        if *self.peek_kind() == kind {
            Ok(self.advance())
        } else {
            Err(self.unexpected(&kind.to_string()))
        }
    }

    fn expect_ident(&mut self) -> PResult<(String, Span)> {
        match self.peek_kind().clone() {
            TokenKind::Ident(name) => {
                let span = self.advance().span;
                Ok((name, span))
            }
            _ => Err(self.unexpected("identifier")),
        }
    }

    fn unexpected(&self, expected: &str) -> Diagnostic {
        let tok = self.peek();
        Diagnostic::new(format!("expected {expected}, found {}", tok.kind), tok.span)
    }

    fn prev_end(&self) -> usize {
        self.tokens[self.pos.saturating_sub(1)].span.end
    }

    fn parse_program(&mut self) -> PResult<Program> {
        let mut functions = Vec::new();
        while *self.peek_kind() != TokenKind::Eof {
            if *self.peek_kind() != TokenKind::Fn {
                return Err(self.unexpected("`fn`"));
            }
            functions.push(self.parse_function()?);
        }
        Ok(Program { functions })
    }

    fn parse_function(&mut self) -> PResult<Function> {
        self.expect(TokenKind::Fn)?;
        let (name, name_span) = self.expect_ident()?;

        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        while *self.peek_kind() != TokenKind::RParen {
            let (pname, pspan) = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param {
                name: pname,
                span: Span {
                    start: pspan.start,
                    end: ty.span.end,
                },
                ty,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;

        let ret = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        Ok(Function {
            name,
            name_span,
            params,
            ret,
            body,
        })
    }

    fn parse_type(&mut self) -> PResult<TypeName> {
        match self.peek_kind().clone() {
            TokenKind::Ident(name) => {
                let span = self.advance().span;
                Ok(TypeName { name, span })
            }
            _ => Err(self.unexpected("type")),
        }
    }

    fn parse_block(&mut self) -> PResult<Block> {
        let start = self.expect(TokenKind::LBrace)?.span.start;
        let mut stmts = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        let end = self.expect(TokenKind::RBrace)?.span.end;
        Ok(Block {
            stmts,
            span: Span { start, end },
        })
    }

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        let start = self.peek().span.start;
        let kind = match self.peek_kind() {
            TokenKind::Let => self.parse_let()?,
            TokenKind::If => return self.parse_if(),
            TokenKind::While => {
                self.advance();
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                StmtKind::While { cond, body }
            }
            TokenKind::Return => {
                self.advance();
                let value = if *self.peek_kind() == TokenKind::Semicolon {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.expect(TokenKind::Semicolon)?;
                StmtKind::Return(value)
            }
            TokenKind::LBrace => StmtKind::Block(self.parse_block()?),
            TokenKind::Ident(_) if *self.peek_nth_kind(1) == TokenKind::Eq => {
                let (name, name_span) = self.expect_ident()?;
                self.expect(TokenKind::Eq)?;
                let value = self.parse_expr()?;
                self.expect(TokenKind::Semicolon)?;
                StmtKind::Assign {
                    name,
                    name_span,
                    value,
                }
            }
            _ => {
                let expr = self.parse_expr()?;
                self.expect(TokenKind::Semicolon)?;
                StmtKind::Expr(expr)
            }
        };
        Ok(Stmt {
            kind,
            span: Span {
                start,
                end: self.prev_end(),
            },
        })
    }

    fn parse_let(&mut self) -> PResult<StmtKind> {
        self.expect(TokenKind::Let)?;
        let mutable = self.eat(&TokenKind::Mut);
        let (name, _) = self.expect_ident()?;
        let ty = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::Semicolon)?;
        Ok(StmtKind::Let {
            name,
            mutable,
            ty,
            value,
        })
    }

    fn parse_if(&mut self) -> PResult<Stmt> {
        let start = self.expect(TokenKind::If)?.span.start;
        let cond = self.parse_expr()?;
        let then_block = self.parse_block()?;

        let else_block = if self.eat(&TokenKind::Else) {
            if *self.peek_kind() == TokenKind::If {
                let nested = self.parse_if()?;
                Some(Block {
                    span: nested.span,
                    stmts: vec![nested],
                })
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };

        Ok(Stmt {
            kind: StmtKind::If {
                cond,
                then_block,
                else_block,
            },
            span: Span {
                start,
                end: self.prev_end(),
            },
        })
    }

    pub fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_binary(0)
    }

    /// Precedence climbing. Comparisons share one level and don't chain,
    /// so `a < b < c` is rejected the same way Rust does it.
    fn parse_binary(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut lhs = self.parse_unary()?;
        let mut seen_comparison = false;

        while let Some((op, prec)) = binary_op(self.peek_kind()) {
            if prec < min_prec {
                break;
            }
            let op_tok = self.advance();
            if op.is_comparison() {
                if seen_comparison {
                    return Err(Diagnostic::new(
                        "comparison operators cannot be chained; use parentheses",
                        op_tok.span,
                    ));
                }
                seen_comparison = true;
            }

            let rhs = self.parse_binary(prec + 1)?;
            let span = Span {
                start: lhs.span.start,
                end: rhs.span.end,
            };
            lhs = Expr::new(
                ExprKind::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            );
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        let op = match self.peek_kind() {
            TokenKind::Minus => UnaryOp::Neg,
            TokenKind::Bang => UnaryOp::Not,
            _ => return self.parse_primary(),
        };
        let start = self.advance().span.start;
        let operand = self.parse_unary()?;
        let span = Span {
            start,
            end: operand.span.end,
        };
        Ok(Expr::new(
            ExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
            span,
        ))
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let tok = self.peek().clone();
        let kind = match tok.kind {
            TokenKind::Int(n) => ExprKind::Int(n),
            TokenKind::True => ExprKind::Bool(true),
            TokenKind::False => ExprKind::Bool(false),
            TokenKind::Str(s) => ExprKind::Str(s),
            TokenKind::Ident(name) => {
                self.advance();
                if *self.peek_kind() != TokenKind::LParen {
                    return Ok(Expr::new(ExprKind::Var(name), tok.span));
                }
                return self.parse_call(name, tok.span);
            }
            TokenKind::LParen => {
                self.advance();
                let mut inner = self.parse_expr()?;
                let end = self.expect(TokenKind::RParen)?.span.end;
                inner.span = Span {
                    start: tok.span.start,
                    end,
                };
                return Ok(inner);
            }
            _ => return Err(self.unexpected("expression")),
        };
        self.advance();
        Ok(Expr::new(kind, tok.span))
    }

    fn parse_call(&mut self, callee: String, callee_span: Span) -> PResult<Expr> {
        self.expect(TokenKind::LParen)?;
        let mut args = Vec::new();
        while *self.peek_kind() != TokenKind::RParen {
            args.push(self.parse_expr()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(TokenKind::RParen)?.span.end;
        Ok(Expr::new(
            ExprKind::Call { callee, args },
            Span {
                start: callee_span.start,
                end,
            },
        ))
    }
}

fn binary_op(kind: &TokenKind) -> Option<(BinaryOp, u8)> {
    let op = match kind {
        TokenKind::OrOr => (BinaryOp::Or, 1),
        TokenKind::AndAnd => (BinaryOp::And, 2),
        TokenKind::EqEq => (BinaryOp::Eq, 3),
        TokenKind::BangEq => (BinaryOp::Ne, 3),
        TokenKind::Lt => (BinaryOp::Lt, 3),
        TokenKind::LtEq => (BinaryOp::Le, 3),
        TokenKind::Gt => (BinaryOp::Gt, 3),
        TokenKind::GtEq => (BinaryOp::Ge, 3),
        TokenKind::Plus => (BinaryOp::Add, 4),
        TokenKind::Minus => (BinaryOp::Sub, 4),
        TokenKind::Star => (BinaryOp::Mul, 5),
        TokenKind::Slash => (BinaryOp::Div, 5),
        TokenKind::Percent => (BinaryOp::Rem, 5),
        _ => return None,
    };
    Some(op)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_src(src: &str) -> PResult<Program> {
        parse(Lexer::new(src).tokenize().unwrap())
    }

    fn parse_expr_src(src: &str) -> Expr {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser { tokens, pos: 0 }.parse_expr().unwrap()
    }

    /// Prints the expression fully parenthesized, which makes precedence easy to assert on.
    fn sexpr(e: &Expr) -> String {
        match &e.kind {
            ExprKind::Int(n) => n.to_string(),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Str(s) => format!("{s:?}"),
            ExprKind::Var(name) => name.clone(),
            ExprKind::Unary { op, operand } => {
                let sym = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                };
                format!("({sym}{})", sexpr(operand))
            }
            ExprKind::Binary { op, lhs, rhs } => {
                format!("({} {} {})", sexpr(lhs), op.symbol(), sexpr(rhs))
            }
            ExprKind::Call { callee, args } => {
                let args: Vec<String> = args.iter().map(sexpr).collect();
                format!("{callee}({})", args.join(", "))
            }
        }
    }

    #[test]
    fn precedence() {
        assert_eq!(sexpr(&parse_expr_src("1 + 2 * 3")), "(1 + (2 * 3))");
        assert_eq!(sexpr(&parse_expr_src("1 - 2 - 3")), "((1 - 2) - 3)");
        assert_eq!(
            sexpr(&parse_expr_src("a < b + 1 && !c || d == e")),
            "(((a < (b + 1)) && (!c)) || (d == e))"
        );
        assert_eq!(sexpr(&parse_expr_src("-(1 + 2) * 3")), "((-(1 + 2)) * 3)");
        assert_eq!(
            sexpr(&parse_expr_src("f(1, g(x), 2 + 3)")),
            "f(1, g(x), (2 + 3))"
        );
    }

    #[test]
    fn chained_comparison_is_rejected() {
        let tokens = Lexer::new("a < b < c").tokenize().unwrap();
        let err = Parser { tokens, pos: 0 }.parse_expr().unwrap_err();
        assert!(err.message.contains("cannot be chained"));

        assert_eq!(sexpr(&parse_expr_src("(a < b) == c")), "((a < b) == c)");
    }

    #[test]
    fn function_with_statements() {
        let program = parse_src(
            "fn main() -> i32 {
                let mut x: i32 = 0;
                while x < 10 { x = x + 1; }
                if x == 10 { return 0; } else if x > 10 { return 1; } else { return 2; }
            }",
        )
        .unwrap();

        let main = &program.functions[0];
        assert_eq!(main.name, "main");
        assert_eq!(main.ret.as_ref().unwrap().name, "i32");
        assert_eq!(main.body.stmts.len(), 3);

        let StmtKind::If { else_block, .. } = &main.body.stmts[2].kind else {
            panic!("expected if statement");
        };
        let else_block = else_block.as_ref().unwrap();
        assert!(matches!(else_block.stmts[0].kind, StmtKind::If { .. }));
    }

    #[test]
    fn params_and_trailing_comma() {
        let program = parse_src("fn add(a: i32, b: i64,) { }").unwrap();
        let names: Vec<&str> = program.functions[0]
            .params
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, ["a", "b"]);
    }

    #[test]
    fn missing_semicolon() {
        let err = parse_src("fn main() { let x = 1 }").unwrap_err();
        assert_eq!(err.message, "expected `;`, found `}`");
    }

    #[test]
    fn top_level_must_be_fn() {
        let err = parse_src("let x = 1;").unwrap_err();
        assert_eq!(err.message, "expected `fn`, found `let`");
    }
}
