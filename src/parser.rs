#![allow(dead_code)]
use crate::lexer::{
    KW_ACOS, KW_ACOSH, KW_ACOT, KW_ACOTH, KW_ACSC, KW_ACSCH, KW_ASEC, KW_ASECH, KW_ASIN, KW_ASINH,
    KW_ATAN, KW_ATANH, KW_COS, KW_COSH, KW_COT, KW_COTH, KW_CSC, KW_CSCH, KW_E, KW_LN, KW_LOG,
    KW_PI, KW_SEC, KW_SECH, KW_SIN, KW_SINH, KW_SQRT, KW_TAN, KW_TANH, Token,
};
use fast_float2 as fast_float;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenKind {
    Number = 0,
    Ident = 1,
    Plus = 2,
    Minus = 3,
    Multiply = 4,
    Divide = 5,
    Exponent = 6,
    LParen = 7,
    RParen = 8,
    EOF = 9,
    Equals = 10,
    _Count = 11,
}

/// The left-binding power (LBP) of each TokenKind.
static LBP: [u8; TokenKind::_Count as usize] = {
    let mut t = [0u8; TokenKind::_Count as usize];
    t[TokenKind::Equals as usize] = 5; // lowest infix; right-assoc: rbp = 4
    t[TokenKind::Plus as usize] = 10;
    t[TokenKind::Minus as usize] = 10;
    t[TokenKind::Multiply as usize] = 20;
    t[TokenKind::Divide as usize] = 20;
    t[TokenKind::LParen as usize] = 20; // implicit multiply: x(0) -> x*(0)
    t[TokenKind::Exponent as usize] = 30; // right-assoc: rbp = 29
    t
};

/// Converts a [`Token`] to a [`TokenKind`].\
/// # Example:
/// ```rust
/// assert_eq!(kind_of(&Token::Plus ), TokenKind::Plus);
/// ```
#[inline(always)]
fn kind_of(token: &Token) -> TokenKind {
    match token {
        Token::Number { .. } => TokenKind::Number,
        Token::Identifier { .. } => TokenKind::Ident,
        Token::Plus => TokenKind::Plus,
        Token::Minus => TokenKind::Minus,
        Token::Multiply => TokenKind::Multiply,
        Token::Divide => TokenKind::Divide,
        Token::Exponent => TokenKind::Exponent,
        Token::LeftParenthesis => TokenKind::LParen,
        Token::RightParenthesis => TokenKind::RParen,
        Token::Equals => TokenKind::Equals,
        Token::EndOfFile => TokenKind::EOF,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Number(f64),
    Constant(f64),
    Variable(u32, u32, u64),

    Neg(u32),

    Add(u32, u32),
    Sub(u32, u32),
    Mul(u32, u32),
    Div(u32, u32),
    Pow(u32, u32),

    Sin(u32),
    Cos(u32),
    Tan(u32),

    Sec(u32),
    Csc(u32),
    Cot(u32),

    Asin(u32),
    Acos(u32),
    Atan(u32),

    Acsc(u32),
    Asec(u32),
    Acot(u32),

    Sinh(u32),
    Cosh(u32),
    Tanh(u32),

    Sech(u32),
    Csch(u32),
    Coth(u32),

    Asinh(u32),
    Acosh(u32),
    Atanh(u32),

    Asech(u32),
    Acsch(u32),
    Acoth(u32),

    Ln(u32),
    Log(u32),
    Sqrt(u32),

    Equation(u32, u32),
}

/// Resolves a function hash to its corresponding [`Node`].
///
/// The supplied `arg` is stored in the resulting [`Node`].\
/// Returns `None` if the supplied `hash` doesn't match any of the supported functions' hash.
///
/// # Examples
///
/// ```rust
/// let node = known_function_kind(KW_SIN, 30);
///
/// assert_eq!(node, Some(Node::Sin(30)));
/// assert_eq!(known_function_kind(0, 30), None);
/// ```
#[inline(always)]
fn known_function_kind(hash: u64, arg: u32) -> Option<Node> {
    Some(match hash {
        KW_SIN => Node::Sin(arg),
        KW_COS => Node::Cos(arg),
        KW_TAN => Node::Tan(arg),
        KW_LN => Node::Ln(arg),
        KW_LOG => Node::Log(arg),
        KW_SQRT => Node::Sqrt(arg),
        KW_SEC => Node::Sec(arg),
        KW_CSC => Node::Csc(arg),
        KW_COT => Node::Cot(arg),
        KW_ASIN => Node::Asin(arg),
        KW_ACOS => Node::Acos(arg),
        KW_ATAN => Node::Atan(arg),
        KW_ACSC => Node::Acsc(arg),
        KW_ASEC => Node::Asec(arg),
        KW_ACOT => Node::Acot(arg),
        KW_SINH => Node::Sinh(arg),
        KW_COSH => Node::Cosh(arg),
        KW_TANH => Node::Tanh(arg),
        KW_SECH => Node::Sech(arg),
        KW_CSCH => Node::Csch(arg),
        KW_COTH => Node::Coth(arg),
        KW_ASINH => Node::Asinh(arg),
        KW_ACOSH => Node::Acosh(arg),
        KW_ATANH => Node::Atanh(arg),
        KW_ASECH => Node::Asech(arg),
        KW_ACSCH => Node::Acsch(arg),
        KW_ACOTH => Node::Acoth(arg),
        _ => return None,
    })
}

pub struct Parser<'src, 'arena> {
    tokens: &'src [Token],
    src: &'src str,
    position: usize,
    arena: &'arena mut Vec<Node>,
}

impl<'src, 'arena> Parser<'src, 'arena> {
    /// Creates a parser over `tokens` and `src`, using `arena` to store the parsed nodes.\
    /// The parser starts at index 0 of the `tokens` array.
    /// # Arguments
    ///
    /// * `tokens` - The tokens to parse.
    /// * `src` - The original source text from which `tokens` was produced.
    /// * `arena` - A mutable vector used to store the parsed nodes.
    pub fn new(tokens: &'src [Token], src: &'src str, arena: &'arena mut Vec<Node>) -> Self {
        Parser {
            tokens,
            src,
            position: 0,
            arena,
        }
    }

    /// Parse the entire expression, returning the root node.
    pub fn parse(mut self) -> Result<u32, String> {
        let root = self.pratt_parse(0)?;
        if self.peek_kind() != TokenKind::EOF {
            return Err(format!("unexpected trailing token: {:?}", self.peek()));
        }
        Ok(root)
    }

    /// Parses an expression using Pratt parsing with the supplied right-binding power (rbp).
    ///
    /// Parses the initial expression with [`Self::nud`], then repeatedly consumes
    /// infix operators whose left-binding power is greater than `rbp`, combining
    /// each operator and its right-hand operand with [`Self::led`].
    ///
    /// A lower `rbp` allows operators with lower precedence to be consumed.
    fn pratt_parse(&mut self, rbp: u8) -> Result<u32, String> {
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
    fn push(&mut self, node: Node) -> u32 {
        let index = self.arena.len() as u32;
        self.arena.push(node);
        index
    }

    #[inline(always)]
    fn peek(&self) -> &Token {
        unsafe { self.tokens.get_unchecked(self.position) }
    }

    /// Peek at the kind of the current token.
    #[inline(always)]
    fn peek_kind(&self) -> TokenKind {
        kind_of(unsafe { self.tokens.get_unchecked(self.position) })
    }

    #[inline(always)]
    fn advance(&mut self) -> Token {
        let token = unsafe { self.tokens.get_unchecked(self.position) }.clone();
        self.position += 1;
        token
    }

    #[inline(always)]
    fn skip(&mut self) {
        self.position += 1;
    }

    /// Expect the current token to be an rparen, and skip it if it is.
    #[inline(always)]
    fn expect_rparen(&mut self) -> Result<(), String> {
        if self.peek_kind() != TokenKind::RParen {
            return Err(format!("expected ')', found {:?}", self.peek()));
        }
        self.skip();
        Ok(())
    }

    /// Parses a token in null-denotation (NUD) position.
    ///
    /// In Pratt parsing, NUD handles tokens that appear at the beginning of an
    /// expression, such as literals, identifiers, prefix operators, and
    /// parenthesized expressions. It consumes the token and returns the index of
    /// the resulting node in the arena.
    ///
    /// # Examples
    ///
    /// ```text
    /// 42       → Node::Number(42)
    /// pi       → Node::Constant(π)
    /// x        → Node::Variable(...)
    /// -x       → Node::Neg(...)
    /// (x + 1)  → Node::Add(...)
    /// sin(x)   → Node::Sin(...)
    /// ```
    ///
    /// Unknown identifiers followed by `(` are treated as implicit
    /// multiplication, so `x(2)` becomes `x * 2`.
    #[inline(always)]
    fn nud(&mut self) -> Result<u32, String> {
        let token = self.advance();
        match token {
            Token::Number { start, end } => {
                let raw = &self.src[start as usize..end as usize];
                let value = fast_float::parse::<f64, _>(raw)
                    .map_err(|e| format!("invalid number '{}': {}", raw, e))?;
                Ok(self.push(Node::Number(value)))
            }

            Token::Identifier { start, end, hash } => {
                if hash == KW_PI {
                    return Ok(self.push(Node::Constant(std::f64::consts::PI)));
                }
                if hash == KW_E {
                    return Ok(self.push(Node::Constant(std::f64::consts::E)));
                }

                // Cheap check first: only bother resolving which function
                // this is (if any) when a '(' actually follows. Plain
                // variables, and identifiers not followed by '(', skip the
                // hash dispatch entirely.
                if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.skip(); // eat '('
                    let arg = self.pratt_parse(0)?;
                    self.expect_rparen()?;
                    if let Some(kind) = known_function_kind(hash, arg) {
                        return Ok(self.push(kind));
                    }
                    // Not a recognized function: identifier '(' ... ')' is
                    // implicit multiplication, e.g. "x(0)" -> x * (0).
                    let var = self.push(Node::Variable(start, end, hash));
                    return Ok(self.push(Node::Mul(var, arg)));
                }

                Ok(self.push(Node::Variable(start, end, hash)))
            }

            Token::Minus => {
                let inner = self.pratt_parse(25)?;
                Ok(self.push(Node::Neg(inner)))
            }

            Token::LeftParenthesis => {
                let inner = self.pratt_parse(0)?;
                self.expect_rparen()?;
                Ok(inner)
            }

            other => Err(format!("unexpected token in expression: {:?}", other)),
        }
    }

    /// Parse a binary operator as the LED (left denotation) of an expression.
    #[inline(always)]
    fn led(&mut self, left: u32) -> Result<u32, String> {
        let token = self.advance();
        match token {
            // '=' is now a general equation separator — both sides can be
            // arbitrary expressions. No lhs restriction at parse time.
            Token::Equals => {
                let right = self.pratt_parse(4)?; // rbp=4 → right-associative
                Ok(self.push(Node::Equation(left, right)))
            }
            Token::Plus => {
                let right = self.pratt_parse(10)?;
                Ok(self.push(Node::Add(left, right)))
            }
            Token::Minus => {
                let right = self.pratt_parse(10)?;
                Ok(self.push(Node::Sub(left, right)))
            }
            Token::Multiply => {
                let right = self.pratt_parse(20)?;
                Ok(self.push(Node::Mul(left, right)))
            }
            Token::Divide => {
                let right = self.pratt_parse(20)?;
                Ok(self.push(Node::Div(left, right)))
            }
            Token::Exponent => {
                let right = self.pratt_parse(29)?;
                Ok(self.push(Node::Pow(left, right)))
            }
            // Implicit multiply: "x(0)" -> x * (0). The '(' was just
            // consumed by advance(); parse the inner expression as a normal
            // parenthesized group, then multiply.
            Token::LeftParenthesis => {
                let right = self.pratt_parse(0)?;
                self.expect_rparen()?;
                Ok(self.push(Node::Mul(left, right)))
            }
            other => Err(format!("unexpected token: {:?}", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Tokenizer;

    #[test]
    fn test_fn_kind_of() {
        assert_eq!(kind_of(&Token::Plus), TokenKind::Plus);
    }

    #[test]
    fn test_fn_known_function_kind() {
        let node = known_function_kind(KW_SIN, 30);
        assert_eq!(node, Some(Node::Sin(30)));
        assert_eq!(known_function_kind(0, 30), None);
    }

    #[test]
    fn test_equation_both_sides() {
        // x^2+2x=2x-3  should parse without error (complex lhs is fine now)
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "x^2+2*x=2*x-3";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        matches!(arena[root as usize], Node::Equation(_, _));
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
