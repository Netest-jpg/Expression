#![allow(dead_code)]

use crate::lexer::{KW_COS, KW_E, KW_LN, KW_LOG, KW_PI, KW_SIN, KW_SQRT, KW_TAN, Token};
use fast_float2 as fast_float;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Number = 0,
    Ident = 1,
    Plus = 2,
    Minus = 3,
    Asterisk = 4,
    Slash = 5,
    Caret = 6,
    LParen = 7,
    RParen = 8,
    Eof = 9,
    Equals = 10,
    _Count = 11,
}

static LBP: [u8; TokenKind::_Count as usize] = {
    let mut t = [0u8; TokenKind::_Count as usize];
    t[TokenKind::Equals as usize] = 5; // lowest infix; right-assoc: rbp = 4
    t[TokenKind::Plus as usize] = 10;
    t[TokenKind::Minus as usize] = 10;
    t[TokenKind::Asterisk as usize] = 20;
    t[TokenKind::Slash as usize] = 20;
    t[TokenKind::Caret as usize] = 30; // right-assoc: rbp = 29
    t
};

#[inline(always)]
fn kind_of(tok: &Token) -> TokenKind {
    match tok {
        Token::Number { .. } => TokenKind::Number,
        Token::Identifier { .. } => TokenKind::Ident,
        Token::Plus => TokenKind::Plus,
        Token::Minus => TokenKind::Minus,
        Token::Asterisk => TokenKind::Asterisk,
        Token::ForwardSlash => TokenKind::Slash,
        Token::Caret => TokenKind::Caret,
        Token::LeftParenthesis => TokenKind::LParen,
        Token::RightParenthesis => TokenKind::RParen,
        Token::Equals => TokenKind::Equals,
        Token::EndOfFile => TokenKind::Eof,
    }
}

// -----------------------------------------------------------------------
// AST node — flat arena, children are u32 indices.
// -----------------------------------------------------------------------
#[derive(Debug, Clone)]
pub enum NodeKind {
    Number(f64),
    /// π or e — resolved at parse time.
    Constant(f64),
    /// Variable: byte offsets + precomputed FNV-1a hash (no src needed at lookup).
    Variable(u32, u32, u64),
    Neg(u32),
    Add(u32, u32),
    Sub(u32, u32),
    Mul(u32, u32),
    Div(u32, u32),
    Pow(u32, u32),
    /// General equation: lhs_expr = rhs_expr.
    /// Both sides are arbitrary expression trees (no restriction on lhs).
    Equation(u32, u32),
    Sin(u32),
    Cos(u32),
    Tan(u32),
    Ln(u32),
    Log(u32),
    Sqrt(u32),
    Call {
        hash: u64,
        arg: u32,
    },
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
}

// -----------------------------------------------------------------------
// Parser
// -----------------------------------------------------------------------
pub struct Parser<'src, 'arena> {
    tokens: &'src [Token],
    src: &'src str,
    pos: usize,
    arena: &'arena mut Vec<Node>,
}

impl<'src, 'arena> Parser<'src, 'arena> {
    pub fn new(tokens: &'src [Token], src: &'src str, arena: &'arena mut Vec<Node>) -> Self {
        Parser {
            tokens,
            src,
            pos: 0,
            arena,
        }
    }

    #[inline(always)]
    fn push(&mut self, kind: NodeKind) -> u32 {
        let idx = self.arena.len() as u32;
        self.arena.push(Node { kind });
        idx
    }

    #[inline(always)]
    fn peek(&self) -> &Token {
        unsafe { self.tokens.get_unchecked(self.pos) }
    }

    #[inline(always)]
    fn peek_kind(&self) -> TokenKind {
        kind_of(unsafe { self.tokens.get_unchecked(self.pos) })
    }

    #[inline(always)]
    fn advance(&mut self) -> Token {
        let tok = unsafe { self.tokens.get_unchecked(self.pos) }.clone();
        self.pos += 1;
        tok
    }

    #[inline(always)]
    fn skip(&mut self) {
        self.pos += 1;
    }

    #[inline(always)]
    fn expect_rparen(&mut self) -> Result<(), String> {
        if self.peek_kind() != TokenKind::RParen {
            return Err(format!("expected ')', found {:?}", self.peek()));
        }
        self.skip();
        Ok(())
    }

    pub fn parse_expr(&mut self, rbp: u8) -> Result<u32, String> {
        let mut left = self.nud()?;
        loop {
            let lbp = unsafe { *LBP.get_unchecked(self.peek_kind() as usize) };
            if lbp <= rbp {
                break;
            }
            left = self.led(left)?;
        }
        Ok(left)
    }

    #[inline(always)]
    fn nud(&mut self) -> Result<u32, String> {
        let tok = self.advance();
        match tok {
            Token::Number { start, end } => {
                let raw = &self.src[start as usize..end as usize];
                let v = fast_float::parse::<f64, _>(raw)
                    .map_err(|e| format!("invalid number '{}': {}", raw, e))?;
                Ok(self.push(NodeKind::Number(v)))
            }

            Token::Identifier { start, end, hash } => {
                if hash == KW_PI {
                    return Ok(self.push(NodeKind::Constant(std::f64::consts::PI)));
                }
                if hash == KW_E {
                    return Ok(self.push(NodeKind::Constant(std::f64::consts::E)));
                }

                if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.skip(); // eat '('
                    let arg = self.parse_expr(0)?;
                    self.expect_rparen()?;
                    return Ok(if hash == KW_SIN {
                        self.push(NodeKind::Sin(arg))
                    } else if hash == KW_COS {
                        self.push(NodeKind::Cos(arg))
                    } else if hash == KW_TAN {
                        self.push(NodeKind::Tan(arg))
                    } else if hash == KW_LN {
                        self.push(NodeKind::Ln(arg))
                    } else if hash == KW_LOG {
                        self.push(NodeKind::Log(arg))
                    } else if hash == KW_SQRT {
                        self.push(NodeKind::Sqrt(arg))
                    } else {
                        self.push(NodeKind::Call { hash, arg })
                    });
                }

                Ok(self.push(NodeKind::Variable(start, end, hash)))
            }

            Token::Minus => {
                let inner = self.parse_expr(25)?;
                Ok(self.push(NodeKind::Neg(inner)))
            }

            Token::LeftParenthesis => {
                let inner = self.parse_expr(0)?;
                self.expect_rparen()?;
                Ok(inner)
            }

            other => Err(format!("unexpected token in expression: {:?}", other)),
        }
    }

    #[inline(always)]
    fn led(&mut self, left: u32) -> Result<u32, String> {
        let tok = self.advance();
        match tok {
            // '=' is now a general equation separator — both sides can be
            // arbitrary expressions. No lhs restriction at parse time.
            Token::Equals => {
                let right = self.parse_expr(4)?; // rbp=4 → right-associative
                Ok(self.push(NodeKind::Equation(left, right)))
            }
            Token::Plus => {
                let right = self.parse_expr(10)?;
                Ok(self.push(NodeKind::Add(left, right)))
            }
            Token::Minus => {
                let right = self.parse_expr(10)?;
                Ok(self.push(NodeKind::Sub(left, right)))
            }
            Token::Asterisk => {
                let right = self.parse_expr(20)?;
                Ok(self.push(NodeKind::Mul(left, right)))
            }
            Token::ForwardSlash => {
                let right = self.parse_expr(20)?;
                Ok(self.push(NodeKind::Div(left, right)))
            }
            Token::Caret => {
                let right = self.parse_expr(29)?;
                Ok(self.push(NodeKind::Pow(left, right)))
            }
            other => Err(format!("unexpected token: {:?}", other)),
        }
    }

    pub fn parse(mut self) -> Result<u32, String> {
        let root = self.parse_expr(0)?;
        if self.peek_kind() != TokenKind::Eof {
            return Err(format!("unexpected trailing token: {:?}", self.peek()));
        }
        Ok(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Tokenizer;

    #[test]
    fn test_equation_both_sides() {
        // x^2+2x=2x-3  should parse without error (complex lhs is fine now)
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "x^2+2*x=2*x-3";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        matches!(arena[root as usize].kind, NodeKind::Equation(_, _));
    }

    #[test]
    fn test_arena_capacity_preserved_on_error() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        Tokenizer::new("1+2*3").tokenize(&mut tokens).unwrap();
        Parser::new(&tokens, "1+2*3", &mut arena).parse().unwrap();
        let cap = arena.capacity();
        arena.clear();
        Tokenizer::new("1+").tokenize(&mut tokens).unwrap();
        let _ = Parser::new(&tokens, "1+", &mut arena).parse();
        assert_eq!(arena.capacity(), cap);
    }
}
