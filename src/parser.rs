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

// -----------------------------------------------------------------------
// Variable store — linear scan over (hash, value) pairs.
//
// Up to 64 bindings; the FNV-1a hash (precomputed by the lexer) means
// lookup never touches the source string.
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

    #[inline(always)]
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Clone all current bindings and push one extra slot for `hash`.
    /// The unknown's value is left as 0.0; call `set_last` to update it.
    /// Used by the Newton solver: one allocation before the loop, then
    /// `set_last` updates the single f64 each iteration — no further allocs.
    pub fn clone_for_probe(&self, hash: u64) -> VarStore {
        let mut probe = VarStore {
            entries: self.entries.clone(),
        };
        // If the hash already exists (unlikely for a free var), update it;
        // otherwise push a new slot that set_last will overwrite.
        if let Some(e) = probe.entries.iter_mut().find(|(h, _)| *h == hash) {
            e.1 = 0.0;
        } else {
            probe.entries.push((hash, 0.0));
        }
        probe
    }

    /// Update the value of the last entry (the unknown slot created by
    /// `clone_for_probe`).  Panics if entries is empty.
    #[inline(always)]
    pub fn set_last(&mut self, value: f64) {
        self.entries.last_mut().unwrap().1 = value;
    }
}

// -----------------------------------------------------------------------
// Variable collection — walk the tree and gather unique identifiers.
// Returns Vec<(hash, start, end)> deduplicated by hash.
// -----------------------------------------------------------------------
pub fn collect_vars(arena: &[Node], root: u32) -> Vec<(u64, u32, u32)> {
    let mut out: Vec<(u64, u32, u32)> = Vec::new();
    collect_vars_inner(arena, root, &mut out);
    out
}

fn collect_vars_inner(arena: &[Node], idx: u32, out: &mut Vec<(u64, u32, u32)>) {
    match &arena[idx as usize].kind {
        NodeKind::Variable(start, end, hash) => {
            if !out.iter().any(|(h, _, _)| h == hash) {
                out.push((*hash, *start, *end));
            }
        }
        NodeKind::Number(_) | NodeKind::Constant(_) => {}
        NodeKind::Neg(a)
        | NodeKind::Sin(a)
        | NodeKind::Cos(a)
        | NodeKind::Tan(a)
        | NodeKind::Ln(a)
        | NodeKind::Log(a)
        | NodeKind::Sqrt(a) => {
            collect_vars_inner(arena, *a, out);
        }
        NodeKind::Call { arg, .. } => collect_vars_inner(arena, *arg, out),
        NodeKind::Add(a, b)
        | NodeKind::Sub(a, b)
        | NodeKind::Mul(a, b)
        | NodeKind::Div(a, b)
        | NodeKind::Pow(a, b)
        | NodeKind::Equation(a, b) => {
            collect_vars_inner(arena, *a, out);
            collect_vars_inner(arena, *b, out);
        }
    }
}

// -----------------------------------------------------------------------
// Eval — pure expression, all variables must be bound.
// -----------------------------------------------------------------------

/// Zero-allocation error type for eval.  Only converted to String at the
/// display boundary, so the Newton hot path never heap-allocates on errors.
#[derive(Debug)]
pub enum EvalError {
    /// A variable was looked up but had no binding in VarStore.
    UnboundVariable,
    /// The root node is an Equation — use evaluate_pending instead.
    IsEquation,
    /// An unknown function hash was encountered; carries the evaluated arg.
    UnknownFunction { hash: u64, arg_value: f64 },
}

impl EvalError {
    pub fn to_string_msg(&self) -> String {
        match self {
            EvalError::UnboundVariable => "unbound variable".to_string(),
            EvalError::IsEquation => "use 'evaluate' to evaluate an equation".to_string(),
            EvalError::UnknownFunction { hash, arg_value } => {
                format!("unknown function hash {hash} applied to {arg_value:?}")
            }
        }
    }
}

pub fn eval(arena: &[Node], idx: u32, vars: &VarStore) -> Result<f64, EvalError> {
    match unsafe { &arena.get_unchecked(idx as usize).kind } {
        NodeKind::Number(v) => Ok(*v),
        NodeKind::Constant(v) => Ok(*v),

        NodeKind::Variable(_, _, hash) => vars.get(*hash).ok_or(EvalError::UnboundVariable),

        // Equation nodes are not evaluated by plain eval; use evaluate_pending.
        NodeKind::Equation(_, _) => Err(EvalError::IsEquation),

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

        NodeKind::Call { hash, arg } => {
            let arg_value = eval(arena, *arg, vars)?;
            Err(EvalError::UnknownFunction {
                hash: *hash,
                arg_value,
            })
        }
    }
}

// -----------------------------------------------------------------------
// Simple variable assignment: `x = <expr with no free vars>`.
//
// Recognises Equation(Variable, rhs) where rhs contains no unbound
// identifiers relative to `vars`. Returns (var_name_hash, value) on
// success, or None if the root is not this shape (caller treats it as a
// pending equation instead).
// -----------------------------------------------------------------------
pub fn try_simple_assign(
    arena: &[Node],
    root: u32,
    vars: &mut VarStore,
    src: &str,
) -> Result<Option<(String, f64)>, String> {
    let NodeKind::Equation(lhs, rhs) = &arena[root as usize].kind else {
        return Ok(None); // not an equation at all
    };
    let (lhs, rhs) = (*lhs, *rhs);

    // lhs must be a plain variable
    let (var_name, var_hash) = match &arena[lhs as usize].kind {
        NodeKind::Variable(start, end, hash) => {
            (src[*start as usize..*end as usize].to_string(), *hash)
        }
        _ => return Ok(None), // complex lhs → treat as equation
    };

    // rhs must be fully evaluable right now
    let value = eval(arena, rhs, vars).map_err(|e| e.to_string_msg())?;
    vars.set(var_hash, value)?;
    Ok(Some((var_name, value)))
}

// -----------------------------------------------------------------------
// Evaluate a pending equation against the VarStore.
//
// The root must be an Equation node.  Three cases:
//
//   • 0 free vars → verify lhs == rhs (within tolerance), print both sides.
//   • 1 free var  → solve f(x) = lhs(x) - rhs(x) = 0 numerically (Newton),
//                   print  `unknown = value`.
//   • >1 free vars → return Err listing which vars still need values.
// -----------------------------------------------------------------------
pub struct EvalResult {
    pub kind: EvalResultKind,
}

pub enum EvalResultKind {
    /// All vars bound: lhs value and rhs value (should be equal for an equation).
    Verified { lhs: f64, rhs: f64 },
    /// One free var solved numerically.
    Solved { name: String, value: f64 },
    /// Expression (no `=`) evaluated to a single value.
    Value(f64),
}

pub fn evaluate_pending(
    arena: &[Node],
    root: u32,
    vars: &VarStore,
    src: &str,
) -> Result<EvalResult, String> {
    // Plain expression (no Equation node at root)
    let (lhs_idx, rhs_idx) = match &arena[root as usize].kind {
        NodeKind::Equation(l, r) => (*l, *r),
        _ => {
            let v = eval(arena, root, vars).map_err(|e| e.to_string_msg())?;
            return Ok(EvalResult {
                kind: EvalResultKind::Value(v),
            });
        }
    };

    // Collect free variables (unbound in VarStore).
    // Reuse a single Vec: collect_vars fills it, retain filters in place.
    let mut free = collect_vars(arena, root);
    free.retain(|(hash, _, _)| vars.get(*hash).is_none());

    match free.len() {
        0 => {
            // All bound — evaluate both sides.
            let lhs_val = eval(arena, lhs_idx, vars).map_err(|e| e.to_string_msg())?;
            let rhs_val = eval(arena, rhs_idx, vars).map_err(|e| e.to_string_msg())?;
            Ok(EvalResult {
                kind: EvalResultKind::Verified {
                    lhs: lhs_val,
                    rhs: rhs_val,
                },
            })
        }

        1 => {
            let (unknown_hash, start, end) = free[0];
            let name = src[start as usize..end as usize].to_string();

            // f(x) = lhs(x) - rhs(x); we want f(x) = 0.
            //
            // Build a probe VarStore once (one clone of entries + one push),
            // then update only the unknown's slot each Newton step via set_last.
            // Zero heap allocations inside the Newton loop.
            let mut probe = vars.clone_for_probe(unknown_hash);
            let mut f = |x: f64| -> Result<f64, String> {
                probe.set_last(x);
                let l = eval(arena, lhs_idx, &probe).map_err(|e| e.to_string_msg())?;
                let r = eval(arena, rhs_idx, &probe).map_err(|e| e.to_string_msg())?;
                Ok(l - r)
            };

            // Newton's method, max 64 iterations, starting at x=1.0 then x=0.0 if needed.
            let solution = newton(&mut f, 1.0)
                .or_else(|_| newton(&mut f, 0.0))
                .or_else(|_| newton(&mut f, -1.0))
                .or_else(|_| newton(&mut f, 10.0))
                .map_err(|_| {
                    format!("could not solve for '{name}'; try assigning an initial guess manually")
                })?;

            Ok(EvalResult {
                kind: EvalResultKind::Solved {
                    name,
                    value: solution,
                },
            })
        }

        _ => {
            // Build error message without a Vec<String> intermediate.
            let mut msg = format!(
                "cannot evaluate: {} variable{} still unassigned: ",
                free.len(),
                if free.len() == 1 { "" } else { "s" },
            );
            for (i, (_, start, end)) in free.iter().enumerate() {
                if i > 0 {
                    msg.push_str(", ");
                }
                msg.push_str(&src[*start as usize..*end as usize]);
            }
            msg.push_str(
                ".\nAssign values with  name=value  or leave exactly one free for solving.",
            );
            Err(msg)
        }
    }
}

// -----------------------------------------------------------------------
// Newton's method: find x such that f(x) ≈ 0.
// -----------------------------------------------------------------------
fn newton(f: &mut impl FnMut(f64) -> Result<f64, String>, x0: f64) -> Result<f64, String> {
    const MAX_ITER: usize = 64;
    const TOL: f64 = 1e-10;
    const H: f64 = 1e-7;

    let mut x = x0;
    for _ in 0..MAX_ITER {
        let fx = f(x)?;
        if fx.abs() < TOL {
            return Ok(x);
        }
        let fpx = (f(x + H)? - f(x - H)?) / (2.0 * H);
        if fpx.abs() < 1e-14 {
            return Err("derivative too small".to_string());
        }
        let x_new = x - fx / fpx;
        if (x_new - x).abs() < TOL {
            return Ok(x_new);
        }
        x = x_new;
    }
    Err("did not converge".to_string())
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Tokenizer;

    fn parse_and_eval(src: &str) -> f64 {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let vars = VarStore::new();
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        eval(&arena, root, &vars).unwrap()
    }

    #[test]
    fn test_addition() {
        assert!((parse_and_eval("1+2") - 3.0).abs() < 1e-10);
    }
    #[test]
    fn test_precedence() {
        assert!((parse_and_eval("2+3*4") - 14.0).abs() < 1e-10);
    }
    #[test]
    fn test_right_assoc_pow() {
        assert!((parse_and_eval("2^3^2") - 512.0).abs() < 1e-10);
    }
    #[test]
    fn test_parentheses() {
        assert!((parse_and_eval("(2+3)*4") - 20.0).abs() < 1e-10);
    }
    #[test]
    fn test_division() {
        assert!((parse_and_eval("10.0/4.0") - 2.5).abs() < 1e-10);
    }
    #[test]
    fn test_decimal_edges() {
        assert!((parse_and_eval(".5+.25") - 0.75).abs() < 1e-10);
        assert!((parse_and_eval("5.+.5") - 5.5).abs() < 1e-10);
    }
    #[test]
    fn test_unary_minus() {
        assert!((parse_and_eval("-3+5") - 2.0).abs() < 1e-10);
        assert!((parse_and_eval("-(2+3)") - (-5.0)).abs() < 1e-10);
    }
    #[test]
    fn test_constants() {
        assert!((parse_and_eval("pi") - std::f64::consts::PI).abs() < 1e-12);
        assert!((parse_and_eval("e") - std::f64::consts::E).abs() < 1e-12);
    }
    #[test]
    fn test_sin() {
        assert!(parse_and_eval("sin(0)").abs() < 1e-10);
    }
    #[test]
    fn test_cos() {
        assert!((parse_and_eval("cos(0)") - 1.0).abs() < 1e-10);
    }
    #[test]
    fn test_sqrt() {
        assert!((parse_and_eval("sqrt(9)") - 3.0).abs() < 1e-10);
    }
    #[test]
    fn test_ln() {
        assert!((parse_and_eval("ln(e)") - 1.0).abs() < 1e-10);
    }
    #[test]
    fn test_nested_calls() {
        assert!((parse_and_eval("sqrt(sin(0)^2+cos(0)^2)") - 1.0).abs() < 1e-10);
    }
    #[test]
    fn test_complex() {
        assert!((parse_and_eval("2+3*(4-1)^2") - 29.0).abs() < 1e-10);
    }

    #[test]
    fn test_simple_assign() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let mut vars = VarStore::new();
        let src = "x=42";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        let result = try_simple_assign(&arena, root, &mut vars, src).unwrap();
        assert!(result.is_some());
        let (name, val) = result.unwrap();
        assert_eq!(name, "x");
        assert!((val - 42.0).abs() < 1e-10);
    }

    #[test]
    fn test_assign_then_use() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let mut vars = VarStore::new();
        // Assign x=3
        let src = "x=3";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        try_simple_assign(&arena, root, &mut vars, src).unwrap();
        // Eval x*x
        arena.clear();
        let src2 = "x*x";
        Tokenizer::new(src2).tokenize(&mut tokens).unwrap();
        let root2 = Parser::new(&tokens, src2, &mut arena).parse().unwrap();
        assert!((eval(&arena, root2, &vars).unwrap() - 9.0).abs() < 1e-10);
    }

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
    fn test_solve_linear() {
        // x+2=5  →  x=3
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let vars = VarStore::new();
        let src = "x+2=5";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        let result = evaluate_pending(&arena, root, &vars, src).unwrap();
        if let EvalResultKind::Solved { name, value } = result.kind {
            assert_eq!(name, "x");
            assert!((value - 3.0).abs() < 1e-8);
        } else {
            panic!("expected Solved");
        }
    }

    #[test]
    fn test_solve_quadratic() {
        // x^2=9  →  x=3 or x=-3 (Newton from x0=1 → 3)
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let vars = VarStore::new();
        let src = "x^2=9";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        let result = evaluate_pending(&arena, root, &vars, src).unwrap();
        if let EvalResultKind::Solved { value, .. } = result.kind {
            assert!((value.abs() - 3.0).abs() < 1e-8);
        } else {
            panic!("expected Solved");
        }
    }

    #[test]
    fn test_too_many_free_vars_error() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let vars = VarStore::new();
        let src = "x+y=5";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        assert!(evaluate_pending(&arena, root, &vars, src).is_err());
    }

    #[test]
    fn test_varstore_limit() {
        let mut vars = VarStore::new();
        for i in 0..VAR_STORE_LIMIT {
            vars.set(i as u64, i as f64).unwrap();
        }
        assert!(vars.set(VAR_STORE_LIMIT as u64 + 1, 1.0).is_err());
        vars.set(0u64, 99.0).unwrap(); // update existing: always ok
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
