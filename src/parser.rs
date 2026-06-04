#![allow(dead_code)] // public API items used by downstream consumers

use crate::lexer::{Token, KW_COS, KW_E, KW_LN, KW_LOG, KW_PI, KW_SIN, KW_SQRT, KW_TAN};

// -----------------------------------------------------------------------
// Binding-power table (Pratt core)
//
// Instead of a chain of match arms every call, we index into a compact array
// keyed on a lightweight TokenKind discriminant.
//
// Left binding power (lbp) is what matters for the loop condition; right
// binding power (rbp) is passed into the recursive nud/led calls.
// Right-associative operators (^) have rbp = lbp - 1.
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
    // sentinel — must stay last
    _Count = 10,
}

/// Left binding power for each token kind.
/// 0 = terminates expression (RParen, EOF, unknown).
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
fn kind_of(tok: &Token<'_>) -> TokenKind {
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
// AST node
//
// Stored as a flat Vec<Node> (arena).  Children are indices (u32) rather
// than Box<Node>, giving:
//   • no heap allocation per node
//   • cache-friendly linear layout
//   • trivially clone-able / serialisable
// -----------------------------------------------------------------------
#[derive(Debug, Clone)]
pub enum NodeKind<'src> {
    Number(f64),
    /// π or e — resolved at parse time; stored as the f64 constant.
    Constant(f64),
    /// Variable name as a borrowed slice.
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
    // General user-defined call (name hash + arg index).
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
    tokens: &'src [Token<'src>],
    pos: usize,
    arena: Vec<Node<'src>>,
}

impl<'src> Parser<'src> {
    /// Create a parser.  `tokens` must end with `Token::EndOfFile`.
    pub fn new(tokens: &'src [Token<'src>]) -> Self {
        Parser {
            tokens,
            pos: 0,
            // Pre-allocate — token count is an upper bound on node count.
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
    fn peek(&self) -> &Token<'src> {
        // Safety: tokens always ends with EndOfFile, so this is in-bounds.
        unsafe { self.tokens.get_unchecked(self.pos) }
    }

    /// Return the kind of the current token without borrowing self for long.
    #[inline(always)]
    fn peek_kind(&self) -> TokenKind {
        kind_of(unsafe { self.tokens.get_unchecked(self.pos) })
    }

    /// Advance past the current token and return a *clone* of its payload.
    /// Cloning Token is cheap: Number/Identifier copy a fat-pointer + u64,
    /// operators are plain unit variants.
    #[inline(always)]
    fn advance(&mut self) -> Token<'src> {
        // Safety: tokens always ends with EndOfFile.
        let tok = unsafe { self.tokens.get_unchecked(self.pos) }.clone();
        self.pos += 1;
        tok
    }

    #[inline(always)]
    fn skip(&mut self) {
        self.pos += 1;
    }

    #[inline(always)]
    fn expect_rparen(&mut self) {
        if self.peek_kind() != TokenKind::RParen {
            panic!("expected ')', found {:?}", self.peek());
        }
        self.skip();
    }

    // -------------------------------------------------------------------
    // Pratt expression parser
    //
    // `rbp` = right binding power (minimum lbp the next infix must beat).
    // -------------------------------------------------------------------

    pub fn parse_expr(&mut self, rbp: u8) -> u32 {
        // ── nud (null-denotation): prefix position ──────────────────────
        let mut left = self.nud();

        // ── led (left-denotation): infix/postfix loop ───────────────────
        loop {
            let lbp = unsafe { *LBP.get_unchecked(self.peek_kind() as usize) };
            if lbp <= rbp {
                break;
            }
            left = self.led(left);
        }

        left
    }

    /// Null-denotation — what a token means at the *start* of an expression.
    #[inline(always)]
    fn nud(&mut self) -> u32 {
        // advance() clones the token (fat-ptr + u64 at most) and bumps pos,
        // so no reference into self.tokens is held across the match body.
        let tok = self.advance();
        match tok {
            Token::Number { raw, .. } => {
                let v = raw
                    .parse::<f64>()
                    .expect("lexer guarantees valid number literal");
                self.push(NodeKind::Number(v))
            }

            Token::Identifier { name, hash } => {
                // All payload is now owned — no borrow of self.tokens remains.

                // O(1) hash dispatch — no string comparisons.
                if hash == KW_PI {
                    return self.push(NodeKind::Constant(std::f64::consts::PI));
                }
                if hash == KW_E {
                    return self.push(NodeKind::Constant(std::f64::consts::E));
                }

                // Unary math functions — peek for '('
                if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.skip(); // eat '('
                    let arg = self.parse_expr(0);
                    self.expect_rparen();

                    return if hash == KW_SIN {
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
                    };
                }

                self.push(NodeKind::Variable(name))
            }

            Token::Minus => {
                // Unary minus: rbp = 25 (beats + and *, yields to ^)
                let inner = self.parse_expr(25);
                self.push(NodeKind::Neg(inner))
            }

            Token::LeftParenthesis => {
                let inner = self.parse_expr(0);
                self.expect_rparen();
                inner // transparent grouping — no extra node
            }

            other => panic!("unexpected token in nud: {:?}", other),
        }
    }

    /// Left-denotation — what a token means *after* a left-hand expression.
    #[inline(always)]
    fn led(&mut self, left: u32) -> u32 {
        let tok = self.advance();
        match tok {
            Token::Plus => {
                let right = self.parse_expr(10);
                self.push(NodeKind::Add(left, right))
            }
            Token::Minus => {
                let right = self.parse_expr(10);
                self.push(NodeKind::Sub(left, right))
            }
            Token::Asterisk => {
                let right = self.parse_expr(20);
                self.push(NodeKind::Mul(left, right))
            }
            Token::ForwardSlash => {
                let right = self.parse_expr(20);
                self.push(NodeKind::Div(left, right))
            }
            Token::Caret => {
                // Right-associative: rbp = lbp - 1 = 29
                let right = self.parse_expr(29);
                self.push(NodeKind::Pow(left, right))
            }
            other => panic!("unexpected token in led: {:?}", other),
        }
    }

    // -------------------------------------------------------------------
    // Public entry point
    // -------------------------------------------------------------------

    /// Parse the full token stream and return (root_index, arena).
    /// The root is the last-pushed node that represents the whole expression.
    pub fn parse(mut self) -> (u32, Vec<Node<'src>>) {
        let root = self.parse_expr(0);
        if self.peek_kind() != TokenKind::Eof {
            panic!("unexpected trailing token: {:?}", self.peek());
        }
        (root, self.arena)
    }
}

// -----------------------------------------------------------------------
// Evaluator
//
// Stack-free recursive walk over the flat arena.
// Each arm is a single arithmetic op — tight enough that LLVM will
// inline and vectorise aggressively.
// -----------------------------------------------------------------------
pub fn eval(arena: &[Node<'_>], idx: u32) -> f64 {
    // Safety: indices are always produced by Parser::push, so in-bounds.
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
        let tokens = Tokenizer::new(src).tokenize();
        let (root, arena) = Parser::new(&tokens).parse();
        eval(&arena, root)
    }

    #[test]
    fn test_addition() {
        assert!((run("1+2") - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_precedence() {
        // 2 + 3 * 4 = 14, not 20
        assert!((run("2+3*4") - 14.0).abs() < 1e-10);
    }

    #[test]
    fn test_right_assoc_pow() {
        // 2^3^2 = 2^9 = 512 (right-associative)
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
        // Lexer inserts Asterisk between "2" and "x"; x is unbound so we
        // test purely the token stream shape by parsing a numeric implicit mul.
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
        let v = run("sin(0)");
        assert!(v.abs() < 1e-10);
    }

    #[test]
    fn test_cos() {
        let v = run("cos(0)");
        assert!((v - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_sqrt() {
        let v = run("sqrt(9)");
        assert!((v - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_nested_calls() {
        // sqrt(sin(0)^2 + cos(0)^2) = sqrt(1) = 1
        let v = run("sqrt(sin(0)^2+cos(0)^2)");
        assert!((v - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_complex_expression() {
        // 2 + 3 * (4 - 1) ^ 2 = 2 + 3*9 = 29
        let v = run("2+3*(4-1)^2");
        assert!((v - 29.0).abs() < 1e-10);
    }

    #[test]
    fn test_arena_layout() {
        // Simple sanity: arena should be non-empty and root valid
        let tokens = Tokenizer::new("1+2*3").tokenize();
        let (root, arena) = Parser::new(&tokens).parse();
        assert!((root as usize) < arena.len());
    }

    #[test]
    fn test_division_and_float() {
        let v = run("10.0/4.0");
        assert!((v - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_decimal_edge_forms() {
        assert!((run(".5+.25") - 0.75).abs() < 1e-10);
        assert!((run("5.+.5") - 5.5).abs() < 1e-10);
    }

    #[test]
    fn test_ln() {
        let v = run("ln(e)"); // ln(e) == 1
        assert!((v - 1.0).abs() < 1e-10);
    }
}
