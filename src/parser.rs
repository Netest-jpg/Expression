#![allow(dead_code)] // public API items used by downstream consumers

use crate::lexer::{Token, KW_COS, KW_E, KW_LN, KW_LOG, KW_PI, KW_SIN, KW_SQRT, KW_TAN};

// -----------------------------------------------------------------------
// Binding-power table (Pratt core)
// -----------------------------------------------------------------------
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
    _Count = 10,
}

/// Left binding power for each token kind.
static LBP: [u8; TokenKind::_Count as usize] = {
    let mut t = [0u8; TokenKind::_Count as usize];
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
        Token::EndOfFile => TokenKind::Eof,
    }
}

// -----------------------------------------------------------------------
// AST node — flat arena, children are u32 indices.
// -----------------------------------------------------------------------
#[derive(Debug, Clone)]
pub enum NodeKind<'src> {
    Number(f64),
    /// π or e — resolved at parse time.
    Constant(f64),
    /// Variable name as a borrowed slice into the source.
    Variable(&'src str),
    Neg(u32),
    Add(u32, u32),
    Sub(u32, u32),
    Mul(u32, u32),
    Div(u32, u32),
    Pow(u32, u32),
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
pub struct Node<'src> {
    pub kind: NodeKind<'src>,
}

// -----------------------------------------------------------------------
// Parser
// -----------------------------------------------------------------------
pub struct Parser<'src> {
    tokens: &'src [Token],
    /// Original source — needed to resolve Number/Identifier byte ranges.
    src: &'src str,
    pos: usize,
    arena: Vec<Node<'src>>,
}

impl<'src> Parser<'src> {
    /// Create a parser. `tokens` must end with `Token::EndOfFile`.
    /// `src` is the original expression string the tokens were scanned from.
    pub fn new(tokens: &'src [Token], src: &'src str) -> Self {
        Parser {
            tokens,
            src,
            pos: 0,
            arena: Vec::with_capacity(tokens.len()),
        }
    }

    // -------------------------------------------------------------------
    // Arena helpers
    // -------------------------------------------------------------------

    #[inline(always)]
    fn push(&mut self, kind: NodeKind<'src>) -> u32 {
        let idx = self.arena.len() as u32;
        self.arena.push(Node { kind });
        idx
    }

    // -------------------------------------------------------------------
    // Token stream helpers
    // -------------------------------------------------------------------

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

    // -------------------------------------------------------------------
    // Pratt expression parser
    // -------------------------------------------------------------------

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
            Token::Number { start, end, .. } => {
                let raw = &self.src[start as usize..end as usize];
                let v = raw
                    .parse::<f64>()
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

                let name = &self.src[start as usize..end as usize];
                Ok(self.push(NodeKind::Variable(name)))
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

    // -------------------------------------------------------------------
    // Public entry point
    // -------------------------------------------------------------------

    pub fn parse(mut self) -> Result<(u32, Vec<Node<'src>>), String> {
        let root = self.parse_expr(0)?;
        if self.peek_kind() != TokenKind::Eof {
            return Err(format!("unexpected trailing token: {:?}", self.peek()));
        }
        Ok((root, self.arena))
    }
}

// -----------------------------------------------------------------------
// Evaluator
// -----------------------------------------------------------------------
pub fn eval(arena: &[Node<'_>], idx: u32) -> f64 {
    match unsafe { &arena.get_unchecked(idx as usize).kind } {
        NodeKind::Number(v) => *v,
        NodeKind::Constant(v) => *v,
        NodeKind::Variable(n) => panic!("unbound variable: {n}"),

        NodeKind::Neg(a) => -eval(arena, *a),
        NodeKind::Add(a, b) => eval(arena, *a) + eval(arena, *b),
        NodeKind::Sub(a, b) => eval(arena, *a) - eval(arena, *b),
        NodeKind::Mul(a, b) => eval(arena, *a) * eval(arena, *b),
        NodeKind::Div(a, b) => eval(arena, *a) / eval(arena, *b),
        NodeKind::Pow(a, b) => eval(arena, *a).powf(eval(arena, *b)),

        NodeKind::Sin(a) => eval(arena, *a).sin(),
        NodeKind::Cos(a) => eval(arena, *a).cos(),
        NodeKind::Tan(a) => eval(arena, *a).tan(),
        NodeKind::Ln(a) => eval(arena, *a).ln(),
        NodeKind::Log(a) => eval(arena, *a).log10(),
        NodeKind::Sqrt(a) => eval(arena, *a).sqrt(),

        NodeKind::Call { hash, arg } => {
            panic!(
                "unknown function hash {hash} applied to {:?}",
                eval(arena, *arg)
            )
        }
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Tokenizer;

    fn run(src: &str) -> f64 {
        let mut tokens = Vec::new();
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let (root, arena) = Parser::new(&tokens, src).parse().unwrap();
        eval(&arena, root)
    }

    #[test]
    fn test_addition() {
        assert!((run("1+2") - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_precedence() {
        assert!((run("2+3*4") - 14.0).abs() < 1e-10);
    }

    #[test]
    fn test_right_assoc_pow() {
        assert!((run("2^3^2") - 512.0).abs() < 1e-10);
    }

    #[test]
    fn test_unary_minus() {
        assert!((run("-3+5") - 2.0).abs() < 1e-10);
        assert!((run("-(2+3)") - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_parentheses() {
        assert!((run("(2+3)*4") - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_implicit_multiply() {
        assert!((run("2*3") - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_constants() {
        let pi = run("pi");
        assert!((pi - std::f64::consts::PI).abs() < 1e-12);
        let e = run("e");
        assert!((e - std::f64::consts::E).abs() < 1e-12);
    }

    #[test]
    fn test_sin() {
        assert!(run("sin(0)").abs() < 1e-10);
    }

    #[test]
    fn test_cos() {
        assert!((run("cos(0)") - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_sqrt() {
        assert!((run("sqrt(9)") - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_nested_calls() {
        let v = run("sqrt(sin(0)^2+cos(0)^2)");
        assert!((v - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_complex_expression() {
        let v = run("2+3*(4-1)^2");
        assert!((v - 29.0).abs() < 1e-10);
    }

    #[test]
    fn test_arena_layout() {
        let mut tokens = Vec::new();
        Tokenizer::new("1+2*3").tokenize(&mut tokens).unwrap();
        let (root, arena) = Parser::new(&tokens, "1+2*3").parse().unwrap();
        assert!((root as usize) < arena.len());
    }

    #[test]
    fn test_division_and_float() {
        assert!((run("10.0/4.0") - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_decimal_edge_forms() {
        assert!((run(".5+.25") - 0.75).abs() < 1e-10);
        assert!((run("5.+.5") - 5.5).abs() < 1e-10);
    }

    #[test]
    fn test_ln() {
        assert!((run("ln(e)") - 1.0).abs() < 1e-10);
    }
}
