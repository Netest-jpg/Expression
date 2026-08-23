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

/// The left-binding power (LBP) of each TokenKind.\
/// \
/// [`TokenKind::Number`] = 0
/// \
/// [`TokenKind::Ident`] = 0
/// \
/// [`TokenKind::Plus`] = 10
/// \
/// [`TokenKind::Minus`] = 10
/// \
/// [`TokenKind::Multiply`] = 20
/// \
/// [`TokenKind::Divide`] = 20
/// \
/// [`TokenKind::Exponent`] = 30
/// \
/// [`TokenKind::LParen`] = 20
/// \
/// [`TokenKind::RParen`] = 0
/// \
/// [`TokenKind::EOF`] = 0
/// \
/// [`TokenKind::Equals`] = 5

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
/// ```ignore
/// // kind_of and TokenKind are private; see the `test_fn_kind_of` unit test.
/// assert_eq!(kind_of(&Token::Plus), TokenKind::Plus);
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

const KW_TABLE_SIZE: usize = 64; // power of 2; 27 entries → ~42% load factor
const KW_MASK: u64 = (KW_TABLE_SIZE as u64) - 1;

/// (keyword hash, tag). tag identifies which Node variant to build.
/// hash == 0 marks an empty slot (no real FNV-1a hash of these keywords is 0).
static KW_TABLE: [(u64, u8); KW_TABLE_SIZE] = build_kw_table();

const fn build_kw_table() -> [(u64, u8); KW_TABLE_SIZE] {
    let mut t = [(0u64, 0u8); KW_TABLE_SIZE];
    const ENTRIES: [(u64, u8); 27] = [
        (KW_SIN, 0),
        (KW_COS, 1),
        (KW_TAN, 2),
        (KW_LN, 3),
        (KW_LOG, 4),
        (KW_SQRT, 5),
        (KW_SEC, 6),
        (KW_CSC, 7),
        (KW_COT, 8),
        (KW_ASIN, 9),
        (KW_ACOS, 10),
        (KW_ATAN, 11),
        (KW_ACSC, 12),
        (KW_ASEC, 13),
        (KW_ACOT, 14),
        (KW_SINH, 15),
        (KW_COSH, 16),
        (KW_TANH, 17),
        (KW_SECH, 18),
        (KW_CSCH, 19),
        (KW_COTH, 20),
        (KW_ASINH, 21),
        (KW_ACOSH, 22),
        (KW_ATANH, 23),
        (KW_ASECH, 24),
        (KW_ACSCH, 25),
        (KW_ACOTH, 26),
    ];
    let mut i = 0;
    while i < ENTRIES.len() {
        let (hash, tag) = ENTRIES[i];
        let mut slot = (hash & KW_MASK) as usize;
        while t[slot].0 != 0 {
            slot = (slot + 1) % KW_TABLE_SIZE;
        }
        t[slot] = (hash, tag);
        i += 1;
    }
    t
}

#[inline(always)]
fn known_function_kind(hash: u64, arg: u32) -> Option<Node> {
    let mut slot = (hash & KW_MASK) as usize;
    loop {
        let (h, tag) = unsafe { *KW_TABLE.get_unchecked(slot) };
        if h == 0 {
            return None; // empty slot — not found, check this first
        }
        if h == hash {
            return Some(match tag {
                0 => Node::Sin(arg),
                1 => Node::Cos(arg),
                2 => Node::Tan(arg),
                3 => Node::Ln(arg),
                4 => Node::Log(arg),
                5 => Node::Sqrt(arg),
                6 => Node::Sec(arg),
                7 => Node::Csc(arg),
                8 => Node::Cot(arg),
                9 => Node::Asin(arg),
                10 => Node::Acos(arg),
                11 => Node::Atan(arg),
                12 => Node::Acsc(arg),
                13 => Node::Asec(arg),
                14 => Node::Acot(arg),
                15 => Node::Sinh(arg),
                16 => Node::Cosh(arg),
                17 => Node::Tanh(arg),
                18 => Node::Sech(arg),
                19 => Node::Csch(arg),
                20 => Node::Coth(arg),
                21 => Node::Asinh(arg),
                22 => Node::Acosh(arg),
                23 => Node::Atanh(arg),
                24 => Node::Asech(arg),
                25 => Node::Acsch(arg),
                26 => Node::Acoth(arg),
                _ => unreachable!(),
            });
        }
        slot = (slot + 1) % KW_TABLE_SIZE;
    }
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
    /// Returns an `Error` if the last [`TokenKind`] is not [`TokenKind::EOF`].
    /// # Example:
    /// ```rust
    /// use expression::lexer::Tokenizer;
    /// use expression::parser::{Node, Parser};
    /// let mut tokens = Vec::new();
    /// let mut arena = Vec::new();
    /// let src = "x^2+2*x=2*x-3";
    ///
    /// Tokenizer::new(src).tokenize(&mut tokens).unwrap();
    ///
    /// let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
    ///
    /// assert!(matches!(arena[root as usize], Node::Equation(_, _)));
    /// ```
    pub fn parse(mut self) -> Result<u32, String> {
        let root = self.pratt_parse(0)?;

        if self.peek_tokenkind() != TokenKind::EOF {
            return self.unexpected_trailing_token();
        }

        Ok(root)
    }
    #[cold]
    fn unexpected_trailing_token(&self) -> Result<u32, String> {
        Err(format!("unexpected trailing token: {:?}", self.peek()))
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
            let lbp = unsafe { *LBP.get_unchecked(self.peek_tokenkind() as usize) };
            if lbp <= rbp {
                break;
            }
            left = self.led(left)?;
        }
        Ok(left)
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
        let token = self.consume();
        match token {
            Token::Number { start, end } => {
                let raw = &self.src[start as usize..end as usize];
                let value = fast_float::parse::<f64, &str>(raw)
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
                if matches!(self.peek_tokenkind(), TokenKind::LParen) {
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

    /// Returns the token at the current [`position`] and advances [`position`] by 1.
    ///
    /// # Safety
    ///
    /// This function does not perform bounds checking.
    /// The caller must ensure that [`position`] is less than the number of tokens.
    /// Otherwise, this function invokes undefined behavior.
    ///
    /// # Example:
    /// ```ignore
    /// // consume and position are private; see the `test_fn_consume` unit test.
    /// let src = "2x+3";
    /// let mut tokens = Vec::new();
    /// let mut arena = Vec::new();
    /// Tokenizer::new(src).tokenize(&mut tokens).unwrap();
    /// let mut parser = Parser::new(&tokens, src, &mut arena);
    ///
    /// assert_eq!(parser.consume(), Token::Number { start: 0, end: 1 });
    /// assert_eq!(parser.position, 1);
    /// ```
    #[inline(always)]
    fn consume(&mut self) -> Token {
        let token = unsafe { self.tokens.get_unchecked(self.position) }.clone();
        self.position += 1;
        token
    }

    /// Stores a [`Node`] in the parser's [`arena`] and
    /// returns the index of the newly appended [`Node`].
    ///
    /// # Example:
    /// ```ignore
    /// // push and arena are private; see the `test_fn_push` unit test.
    /// let src = "";
    /// let mut tokens = Vec::new();
    /// let mut arena = Vec::new();
    /// Tokenizer::new(src).tokenize(&mut tokens).unwrap();
    /// let mut parser = Parser::new(&tokens, src, &mut arena);
    ///
    /// let index = parser.push(Node::Number(42.0));
    /// assert_eq!(index, 0);
    /// assert_eq!(parser.arena.len(), 1);
    /// ```
    #[inline(always)]
    fn push(&mut self, node: Node) -> u32 {
        let index = self.arena.len() as u32;
        self.arena.push(node);
        index
    }

    /// Returns [`TokenKind`] at the current [`position`].
    /// # Safety
    /// This function does not perform bounds checking.
    /// The caller must ensure that [`position`] must be less than the number of tokens.
    #[inline(always)]
    fn peek_tokenkind(&self) -> TokenKind {
        kind_of(unsafe { self.tokens.get_unchecked(self.position) })
    }

    /// Expect the current token to be an rparen.\
    /// Returns an `Error` if it is not [`TokenKind::RParen`] and skips if it is.
    #[inline(always)]
    fn expect_rparen(&mut self) -> Result<(), String> {
        if self.peek_tokenkind() != TokenKind::RParen {
            return Err(format!("expected ')', found {:?}", self.peek()));
        }
        self.skip();
        Ok(())
    }

    /// Returns [`Token`] at the current [`position`].
    /// # Safety
    /// This function doesn't perform bounds checking.
    /// The caller must ensure that [`position`] must be less than the number of tokens.
    #[inline(always)]
    fn peek(&self) -> &Token {
        unsafe { self.tokens.get_unchecked(self.position) }
    }

    /// Skips to the next position.
    #[inline(always)]
    fn skip(&mut self) {
        self.position += 1;
    }

    /// Parse a binary operator as the LED (left denotation) of an expression.
    #[inline(always)]
    fn led(&mut self, left: u32) -> Result<u32, String> {
        let token = self.consume();
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
    fn test_fn_consume() {
        let src = "2x+3";
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let mut parser = Parser::new(&tokens, src, &mut arena);
        assert_eq!(parser.consume(), Token::Number { start: 0, end: 1 });
        assert_eq!(parser.position, 1);
    }

    #[test]
    fn test_fn_push() {
        let src = "";
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let mut parser = Parser::new(&tokens, src, &mut arena);

        let index = parser.push(Node::Number(42.0));
        assert_eq!(index, 0);
        assert_eq!(parser.arena.len(), 1);
    }

    #[test]
    fn test_fn_parse() {
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
