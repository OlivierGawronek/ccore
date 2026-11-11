use crate::diagnostic::Diagnostic;
use crate::token::{Span, Token, TokenKind};

pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    /// Returns all tokens in the source, always ending with `Eof`.
    pub fn tokenize(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                return Ok(tokens);
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<u8> {
        self.bytes.get(self.pos + 1).copied()
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_ascii_whitespace() => self.pos += 1,
                Some(b'/') if self.peek_next() == Some(b'/') => {
                    while self.peek().is_some_and(|c| c != b'\n') {
                        self.pos += 1;
                    }
                }
                _ => return,
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, Diagnostic> {
        self.skip_whitespace_and_comments();
        let start = self.pos;

        let Some(c) = self.peek() else {
            return Ok(Token {
                kind: TokenKind::Eof,
                span: Span { start, end: start },
            });
        };

        let kind = if c.is_ascii_digit() {
            self.lex_number()?
        } else if c.is_ascii_alphabetic() || c == b'_' {
            self.lex_ident_or_keyword()
        } else if c == b'"' {
            self.lex_string()?
        } else {
            self.lex_symbol()?
        };

        Ok(Token {
            kind,
            span: Span {
                start,
                end: self.pos,
            },
        })
    }

    fn lex_number(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if !c.is_ascii_digit() && c != b'_' {
                break;
            }
            self.pos += 1;
        }

        let text = &self.src[start..self.pos];
        match text.replace('_', "").parse::<i64>() {
            Ok(value) => Ok(TokenKind::Int(value)),
            Err(_) => Err(Diagnostic::new(
                format!("integer literal out of range: {text}"),
                Span {
                    start,
                    end: self.pos,
                },
            )),
        }
    }

    fn lex_ident_or_keyword(&mut self) -> TokenKind {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            self.pos += 1;
        }

        match &self.src[start..self.pos] {
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            name => TokenKind::Ident(name.to_string()),
        }
    }

    fn lex_string(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        self.pos += 1; // opening quote

        let mut value = Vec::new();
        loop {
            match self.peek() {
                None | Some(b'\n') => {
                    return Err(Diagnostic::new(
                        "unterminated string literal",
                        Span {
                            start,
                            end: self.pos,
                        },
                    ));
                }
                Some(b'"') => {
                    self.pos += 1;
                    break;
                }
                Some(b'\\') => {
                    let escape_start = self.pos;
                    self.pos += 1;
                    let escaped = match self.peek() {
                        Some(b'n') => b'\n',
                        Some(b't') => b'\t',
                        Some(b'r') => b'\r',
                        Some(b'0') => b'\0',
                        Some(b'\\') => b'\\',
                        Some(b'"') => b'"',
                        _ => {
                            let escaped_len = self.src[self.pos..]
                                .chars()
                                .next()
                                .map_or(0, char::len_utf8);
                            return Err(Diagnostic::new(
                                "unknown escape sequence",
                                Span {
                                    start: escape_start,
                                    end: self.pos + escaped_len,
                                },
                            ));
                        }
                    };
                    value.push(escaped);
                    self.pos += 1;
                }
                Some(c) => {
                    value.push(c);
                    self.pos += 1;
                }
            }
        }

        // We only split the source at ASCII bytes, so multi-byte characters stay intact.
        Ok(TokenKind::Str(String::from_utf8(value).unwrap()))
    }

    fn lex_symbol(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        let c = self.bytes[self.pos];
        let next = self.peek_next();

        // Two-char operators have to be matched before their one-char prefixes.
        let (kind, len) = match (c, next) {
            (b'-', Some(b'>')) => (TokenKind::Arrow, 2),
            (b'=', Some(b'=')) => (TokenKind::EqEq, 2),
            (b'!', Some(b'=')) => (TokenKind::BangEq, 2),
            (b'<', Some(b'=')) => (TokenKind::LtEq, 2),
            (b'>', Some(b'=')) => (TokenKind::GtEq, 2),
            (b'&', Some(b'&')) => (TokenKind::AndAnd, 2),
            (b'|', Some(b'|')) => (TokenKind::OrOr, 2),
            (b'(', _) => (TokenKind::LParen, 1),
            (b')', _) => (TokenKind::RParen, 1),
            (b'{', _) => (TokenKind::LBrace, 1),
            (b'}', _) => (TokenKind::RBrace, 1),
            (b',', _) => (TokenKind::Comma, 1),
            (b':', _) => (TokenKind::Colon, 1),
            (b';', _) => (TokenKind::Semicolon, 1),
            (b'+', _) => (TokenKind::Plus, 1),
            (b'-', _) => (TokenKind::Minus, 1),
            (b'*', _) => (TokenKind::Star, 1),
            (b'/', _) => (TokenKind::Slash, 1),
            (b'%', _) => (TokenKind::Percent, 1),
            (b'=', _) => (TokenKind::Eq, 1),
            (b'!', _) => (TokenKind::Bang, 1),
            (b'<', _) => (TokenKind::Lt, 1),
            (b'>', _) => (TokenKind::Gt, 1),
            _ => {
                let ch = self.src[start..].chars().next().unwrap();
                return Err(Diagnostic::new(
                    format!("unexpected character '{ch}'"),
                    Span {
                        start,
                        end: start + ch.len_utf8(),
                    },
                ));
            }
        };

        self.pos += len;
        Ok(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        Lexer::new(src)
            .tokenize()
            .expect("lexer failed")
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn empty_source_gives_eof() {
        assert_eq!(kinds(""), vec![Eof]);
        assert_eq!(kinds("   \n\t  "), vec![Eof]);
    }

    #[test]
    fn single_char_symbols() {
        assert_eq!(
            kinds("( ) { } , : ; + - * / %"),
            vec![
                LParen, RParen, LBrace, RBrace, Comma, Colon, Semicolon, Plus, Minus, Star, Slash,
                Percent, Eof
            ]
        );
    }

    #[test]
    fn two_char_symbols_win_over_one_char() {
        assert_eq!(
            kinds("-> == != <= >= && || = ! < > -"),
            vec![
                Arrow, EqEq, BangEq, LtEq, GtEq, AndAnd, OrOr, Eq, Bang, Lt, Gt, Minus, Eof
            ]
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(
            kinds("0 42 1_000_000"),
            vec![Int(0), Int(42), Int(1_000_000), Eof]
        );
    }

    #[test]
    fn keywords_and_identifiers() {
        assert_eq!(
            kinds("fn let mut if else while return true false"),
            vec![Fn, Let, Mut, If, Else, While, Return, True, False, Eof]
        );
        assert_eq!(
            kinds("x _tmp count2 letter iffy"),
            vec![
                Ident("x".into()),
                Ident("_tmp".into()),
                Ident("count2".into()),
                Ident("letter".into()),
                Ident("iffy".into()),
                Eof
            ]
        );
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(kinds("1 // a comment\n2"), vec![Int(1), Int(2), Eof]);
        assert_eq!(kinds("// just a comment"), vec![Eof]);
    }

    #[test]
    fn strings() {
        assert_eq!(
            kinds(r#""Hello, world!" "a\n\"b\"" "zażółć""#),
            vec![
                Str("Hello, world!".into()),
                Str("a\n\"b\"".into()),
                Str("zażółć".into()),
                Eof
            ]
        );
    }

    #[test]
    fn unterminated_string_is_an_error() {
        let err = Lexer::new("let s = \"oops").tokenize().unwrap_err();
        assert_eq!(err.message, "unterminated string literal");
        assert_eq!(err.span, Span { start: 8, end: 13 });
    }

    #[test]
    fn unknown_escape_covers_the_whole_character() {
        let err = Lexer::new(r#""\ż""#).tokenize().unwrap_err();
        assert_eq!(err.message, "unknown escape sequence");
        assert_eq!(err.span, Span { start: 1, end: 4 });
    }

    #[test]
    fn spans_point_at_source() {
        let tokens = Lexer::new("let x == 42").tokenize().unwrap();
        let spans: Vec<(usize, usize)> =
            tokens.iter().map(|t| (t.span.start, t.span.end)).collect();
        assert_eq!(spans, vec![(0, 3), (4, 5), (6, 8), (9, 11), (11, 11)]);
    }

    #[test]
    fn unknown_character_is_an_error() {
        let err = Lexer::new("let x = 5 @ 3;").tokenize().unwrap_err();
        assert_eq!(err.span, Span { start: 10, end: 11 });

        let err = Lexer::new("ą").tokenize().unwrap_err();
        assert_eq!(err.message, "unexpected character 'ą'");
        assert_eq!(err.span, Span { start: 0, end: 2 });
    }

    #[test]
    fn whole_function() {
        let src = "fn add(a: i32, b: i32) -> i32 {\n    return a + b;\n}";
        assert_eq!(
            kinds(src),
            vec![
                Fn,
                Ident("add".into()),
                LParen,
                Ident("a".into()),
                Colon,
                Ident("i32".into()),
                Comma,
                Ident("b".into()),
                Colon,
                Ident("i32".into()),
                RParen,
                Arrow,
                Ident("i32".into()),
                LBrace,
                Return,
                Ident("a".into()),
                Plus,
                Ident("b".into()),
                Semicolon,
                RBrace,
                Eof
            ]
        );
    }
}
