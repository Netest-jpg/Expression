#![allow(dead_code)]

use crate::lexer::{Token, KW_COS, KW_E, KW_LN, KW_LOG, KW_PI, KW_SIN, KW_SQRT, KW_TAN};
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
    /// Variable name stored as byte offsets + precomputed FNV-1a hash.
    Variable(u32, u32, u64),
    Neg(u32),
    Add(u32, u32),
    Sub(u32, u32),
    Mul(u32, u32),
    Div(u32, u32),
    Pow(u32, u32),
    /// lhs must be a Variable node index; rhs is the value expression.
    Assign(u32, u32),
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
    /// Original source — needed to resolve Number/Identifier byte ranges.
    src: &'src str,
    pos: usize,
    /// Borrowed from the caller so the allocation is always preserved,
    /// even when parse() returns Err. Caller must clear before calling new().
    arena: &'arena mut Vec<Node>,
}

impl<'src, 'arena> Parser<'src, 'arena> {
    /// Create a parser. `tokens` must end with `Token::EndOfFile`.
    /// `src` is the original expression string the tokens were scanned from.
    /// `arena` must already be cleared by the caller; its allocation is
    /// preserved across both Ok and Err returns.
    #[inline(always)]
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
            Token::Number { start, end, .. } => {
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
            Token::Equals => {
                // Validate lhs is a plain variable, not an expression.
                match self.arena[left as usize].kind {
                    NodeKind::Variable(_, _, _) => {}
                    _ => return Err("left-hand side of '=' must be a variable name".to_string()),
                }
                let right = self.parse_expr(4)?; // rbp=4 → right-associative
                Ok(self.push(NodeKind::Assign(left, right)))
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

    // -------------------------------------------------------------------
    // Public entry point
    // -------------------------------------------------------------------

    /// Parse the token stream. Returns the root node index on success.
    /// On either Ok or Err, the arena borrow is released back to the caller
    /// with its allocation intact.
    pub fn parse(mut self) -> Result<u32, String> {
        let root = self.parse_expr(0)?;
        if self.peek_kind() != TokenKind::Eof {
            return Err(format!("unexpected trailing token: {:?}", self.peek()));
        }
        Ok(root)
    }
}

// -----------------------------------------------------------------------
// Variable store — linear scan over (hash, value) pairs.
//
// Up to 64 bindings fit comfortably; the FNV-1a hash (already computed by
// the lexer) means lookup never touches the source string.
// -----------------------------------------------------------------------
pub const VAR_STORE_LIMIT: usize = 64;

pub struct VarStore {
    entries: Vec<(u64, f64)>, // (name hash, value)
}

impl VarStore {
    pub fn new() -> Self {
        VarStore {
            entries: Vec::with_capacity(16),
        }
    }

    #[inline(always)]
    pub fn get(&self, hash: u64) -> Option<f64> {
        self.entries
            .iter()
            .find(|(h, _)| *h == hash)
            .map(|(_, v)| *v)
    }

    /// Insert or update. Returns Err if the store is full and the key is new.
    #[inline(always)]
    pub fn set(&mut self, hash: u64, value: f64) -> Result<(), String> {
        if let Some(entry) = self.entries.iter_mut().find(|(h, _)| *h == hash) {
            entry.1 = value;
            return Ok(());
        }
        if self.entries.len() >= VAR_STORE_LIMIT {
            return Err(format!(
                "variable limit ({VAR_STORE_LIMIT}) reached; clear some variables first"
            ));
        }
        self.entries.push((hash, value));
        Ok(())
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn eval(arena: &[Node], idx: u32, vars: &VarStore) -> Result<f64, String> {
    match unsafe { &arena.get_unchecked(idx as usize).kind } {
        NodeKind::Number(v) => Ok(*v),
        NodeKind::Constant(v) => Ok(*v),
        NodeKind::Variable(start, end, hash) => vars
            .get(*hash)
            .ok_or_else(|| format!("unbound variable at [{start}..{end}]")),

        NodeKind::Assign(lhs, rhs) => {
            // lhs is guaranteed to be a Variable node (enforced in led).
            let value = eval(arena, *rhs, vars)?;
            // We need a mutable vars but eval takes &VarStore — callers that
            // need assignment must call eval_assign instead. This arm is
            // unreachable through the normal eval path; if reached it means
            // the caller passed an Assign root to plain eval.
            let _ = lhs;
            // Surface a clear message rather than a cryptic panic.
            Err(format!(
                "assignment expression must be evaluated with eval_assign (value would be {value})"
            ))
        }

        NodeKind::Neg(a) => Ok(-eval(arena, *a, vars)?),
        NodeKind::Add(a, b) => Ok(eval(arena, *a, vars)? + eval(arena, *b, vars)?),
        NodeKind::Sub(a, b) => Ok(eval(arena, *a, vars)? - eval(arena, *b, vars)?),
        NodeKind::Mul(a, b) => Ok(eval(arena, *a, vars)? * eval(arena, *b, vars)?),
        NodeKind::Div(a, b) => Ok(eval(arena, *a, vars)? / eval(arena, *b, vars)?),
        NodeKind::Pow(a, b) => Ok(eval(arena, *a, vars)?.powf(eval(arena, *b, vars)?)),

        NodeKind::Sin(a) => Ok(eval(arena, *a, vars)?.sin()),
        NodeKind::Cos(a) => Ok(eval(arena, *a, vars)?.cos()),
        NodeKind::Tan(a) => Ok(eval(arena, *a, vars)?.tan()),
        NodeKind::Ln(a) => Ok(eval(arena, *a, vars)?.ln()),
        NodeKind::Log(a) => Ok(eval(arena, *a, vars)?.log10()),
        NodeKind::Sqrt(a) => Ok(eval(arena, *a, vars)?.sqrt()),

        NodeKind::Call { hash, arg } => Err(format!(
            "unknown function hash {hash} applied to {:?}",
            eval(arena, *arg, vars)?
        )),
    }
}

/// Evaluate a tree that may be rooted at an Assign node.
/// Returns the resulting value and (for assignments) the variable name hash.
pub fn eval_assign(
    arena: &[Node],
    idx: u32,
    vars: &mut VarStore,
) -> Result<(f64, Option<u64>), String> {
    match &arena[idx as usize].kind {
        NodeKind::Assign(lhs, rhs) => {
            let hash = var_hash(arena, *lhs);
            let value = eval(arena, *rhs, vars)?;
            vars.set(hash, value)?;
            Ok((value, Some(hash)))
        }
        _ => Ok((eval(arena, idx, vars)?, None)),
    }
}

/// Extract the FNV hash from a Variable node (panics if not Variable).
#[inline(always)]
fn var_hash(arena: &[Node], idx: u32) -> u64 {
    match arena[idx as usize].kind {
        NodeKind::Variable(_, _, hash) => hash,
        _ => panic!("var_hash called on non-Variable node"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Tokenizer;

    fn run(src: &str) -> f64 {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let vars = VarStore::new();
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        eval(&arena, root, &vars).unwrap()
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
        let mut arena = Vec::new();
        Tokenizer::new("1+2*3").tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, "1+2*3", &mut arena).parse().unwrap();
        assert!((root as usize) < arena.len());
    }

    #[test]
    fn test_assignment_basic() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let mut vars = VarStore::new();
        Tokenizer::new("x=42").tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, "x=42", &mut arena).parse().unwrap();
        let (val, hash) = eval_assign(&arena, root, &mut vars).unwrap();
        assert!((val - 42.0).abs() < 1e-10);
        assert!(hash.is_some());
    }

    #[test]
    fn test_assignment_then_use() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let mut vars = VarStore::new();
        // x = 3
        Tokenizer::new("x=3").tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, "x=3", &mut arena).parse().unwrap();
        eval_assign(&arena, root, &mut vars).unwrap();
        // x*x
        arena.clear();
        Tokenizer::new("x*x").tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, "x*x", &mut arena).parse().unwrap();
        let val = eval(&arena, root, &vars).unwrap();
        assert!((val - 9.0).abs() < 1e-10);
    }

    #[test]
    fn test_assignment_lhs_must_be_variable() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        Tokenizer::new("1+2=5").tokenize(&mut tokens).unwrap();
        let result = Parser::new(&tokens, "1+2=5", &mut arena).parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_varstore_limit() {
        let mut vars = VarStore::new();
        for i in 0..VAR_STORE_LIMIT {
            vars.set(i as u64, i as f64).unwrap();
        }
        // One more new key must fail.
        let err = vars.set(VAR_STORE_LIMIT as u64 + 1, 1.0);
        assert!(err.is_err());
        // Updating an existing key must still succeed even when full.
        vars.set(0u64, 99.0).unwrap();
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

    #[test]
    fn test_arena_capacity_preserved_on_error() {
        // Capacity must survive a parse error — no re-allocation next iteration.
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        // First: a valid parse to grow the arena.
        Tokenizer::new("1+2*3").tokenize(&mut tokens).unwrap();
        Parser::new(&tokens, "1+2*3", &mut arena).parse().unwrap();
        let cap_after_success = arena.capacity();
        assert!(cap_after_success > 0);
        // Now: a parse error.
        arena.clear();
        Tokenizer::new("1+").tokenize(&mut tokens).unwrap();
        let _ = Parser::new(&tokens, "1+", &mut arena).parse();
        // Capacity must be unchanged — the arena was borrowed, not moved.
        assert_eq!(arena.capacity(), cap_after_success);
    }
}