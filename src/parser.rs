#![allow(dead_code)]
use crate::lexer::{
    KW_ACOS, KW_ACOSH, KW_ACOT, KW_ACOTH, KW_ACSC, KW_ACSCH, KW_ASEC, KW_ASECH, KW_ASIN, KW_ASINH,
    KW_ATAN, KW_ATANH, KW_COS, KW_COSH, KW_COT, KW_COTH, KW_CSC, KW_CSCH, KW_E, KW_LN, KW_LOG,
    KW_PI, KW_SEC, KW_SECH, KW_SIN, KW_SINH, KW_SQRT, KW_TAN, KW_TANH, Token, Tokenizer,
};
use fast_float2 as fast_float;

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
    LogBase(u32, u32),
    Sqrt(u32),

    Equation(u32, u32),
}

/// Resolves a function hash to its corresponding `Node`.
///
/// The supplied `arg` is stored in the resulting `Node`.
///
/// Returns `None` if the supplied `hash` doesn't match any of the supported functions' hash.
///
/// # Examples
/// ```ignore
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
    pub fn new(tokens: &'src [Token], src: &'src str, arena: &'arena mut Vec<Node>) -> Self {
        Parser {
            tokens,
            src,
            position: 0,
            arena,
        }
    }

    /// Parses the entire expression and returns the root node.
    ///
    /// Returns an `Err` if the last `Token` is not `Token::EndOfFile`.
    ///
    /// # Example:
    /// ```rust
    /// use expression::lexer::Tokenizer;
    /// use expression::parser::{Node, Parser};
    ///
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
        if !matches!(self.peek(), Token::EndOfFile) {
            return self.unexpected_trailing_token();
        }
        Ok(root)
    }
    #[cold]
    fn unexpected_trailing_token(&self) -> Result<u32, String> {
        Err(format!("Unexpected trailing token: {:?}", self.peek()))
        // Err(format!(
        //     "Unexpected trailing token: {:?}",
        //     self.token_text(self.peek())
        // ))
    }

    // #[inline(always)]
    // fn token_text(&self, token: &Token) -> &str {
    //     match token {
    //         Token::Number { start, end } | Token::Identifier { start, end, .. } => unsafe {
    //             self.src.get_unchecked(*start as usize..*end as usize)
    //         },
    //         _ => "",
    //     }
    // }

    /// Parses an expression using Pratt (operator-precedence) parsing with the
    /// supplied right-binding power (`rbp`).
    ///
    /// Parses the initial expression with [`Self::nud`], then repeatedly consumes
    /// infix operators whose left-binding power ([`LBP`]) is greater than `rbp`,
    /// combining each operator and its right-hand operand with [`Self::led`].
    /// Parsing stops as soon as an operator's LBP is `<= rbp`, or when
    /// [`TokenKind::EOF`] or [`TokenKind::RParen`] is reached (both have an
    /// LBP of `0`).
    ///
    /// A lower `rbp` allows operators of lower precedence to be consumed;
    /// this is how precedence climbing and right-associativity (e.g. `^`,
    /// unary `-`, `=`) are implemented — see the `rbp` values passed from
    /// [`Self::led`] and [`Self::nud`].
    ///
    /// # Example:
    /// ```rust
    /// use expression::lexer::Tokenizer;
    /// use expression::parser::{Node, Parser};
    /// let mut tokens = Vec::new();
    /// let mut arena = Vec::new();
    /// let src = "2+3*4";
    ///
    /// Tokenizer::new(src).tokenize(&mut tokens).unwrap();
    ///
    /// let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
    /// assert!(matches!(arena[root as usize], Node::Add(_, _)));
    /// ```
    fn pratt_parse(&mut self, rbp: u8) -> Result<u32, String> {
        let mut left = self.nud()?;
        loop {
            let lbp = self.peek().lbp();
            if lbp <= rbp {
                break;
            }
            left = self.led(left)?;
        }
        Ok(left)
    }

    /// The "null denotation" step of Pratt parsing: consumes the current
    /// token and parses it as the start of an expression (a literal,
    /// identifier, unary prefix operator, or parenthesized group), with no
    /// left operand.
    ///
    /// Handles: numbers, identifiers (constants `pi`/`e`, known functions
    /// like `sin(...)`, implicit multiplication like `x(0)` for unknown
    /// identifiers, and bare variables), unary minus, and parenthesized
    /// sub-expressions. Returns an error for any other token, since those
    /// cannot begin an expression.
    ///
    /// # Example:
    /// ```ignore
    /// // nud is private; see the `test_fn_parse` unit test, which exercises
    /// // nud indirectly through Parser::parse.
    /// let src = "-3";
    /// let mut tokens = Vec::new();
    /// let mut arena = Vec::new();
    /// Tokenizer::new(src).tokenize(&mut tokens).unwrap();
    /// let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
    /// assert!(matches!(arena[root as usize], Node::Neg(_)));
    /// ```
    #[inline(always)]
    fn nud(&mut self) -> Result<u32, String> {
        let token = self.consume();
        match token {
            Token::Number { start, end } => {
                let raw = &self.src[start as usize..end as usize];
                let value = fast_float::parse::<f64, &str>(raw)
                    .map_err(|e| invalid_number_error(raw, &e))?;
                Ok(self.push(Node::Number(value)))
            }

            Token::Identifier { start, end, hash } => {
                if hash == KW_PI {
                    return Ok(self.push(Node::Constant(std::f64::consts::PI)));
                }
                if hash == KW_E {
                    return Ok(self.push(Node::Constant(std::f64::consts::E)));
                }

                if matches!(self.peek(), Token::LeftParenthesis) {
                    if self.src[start as usize..end as usize].starts_with("log_") {
                        let base = self.parse_log_base(start, end)?;
                        self.skip(); // eat '('
                        let arg = self.pratt_parse(token.rbp())?;
                        self.expect_rparen()?;
                        return Ok(self.push(Node::LogBase(base, arg)));
                    }

                    self.skip(); // eat '('
                    let arg = self.pratt_parse(0)?;
                    self.expect_rparen()?;
                    if let Some(kind) = known_function_kind(hash, arg) {
                        return Ok(self.push(kind));
                    }
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

            other => unexpected_token_in_expression(&other),
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

    #[inline(always)]
    fn parse_log_base(&mut self, start: u32, end: u32) -> Result<u32, String> {
        let base_start = start + 4;
        if base_start >= end {
            return Err("Expected base after log_".to_string());
        }

        let base_src = &self.src[base_start as usize..end as usize];
        let mut tokens = Vec::new();
        Tokenizer::new(base_src)
            .tokenize(&mut tokens)
            .map_err(|e| invalid_log_base_error(&e))?;
        offset_tokens(&mut tokens, base_start);

        Parser::new(&tokens, self.src, self.arena).parse()
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

    /// Expect the current token to be an rparen.\
    /// Returns an `Error` if it is not [`TokenKind::RParen`] and skips if it is.
    #[inline(always)]
    fn expect_rparen(&mut self) -> Result<(), String> {
        if !matches!(self.peek(), Token::RightParenthesis) {
            return Err(expected_rparen_error(self.peek()));
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

    /// The "left denotation" step of Pratt parsing: consumes the current
    /// (infix) token and combines it with the already-parsed `left` operand
    /// to build a binary [`Node`], recursively parsing the right-hand side
    /// via [`Self::pratt_parse`] with the operator's right-binding power.
    ///
    /// Handles `=`, `+`, `-`, `*`, `/`, `^`, and implicit multiplication via
    /// a following `(`, e.g. `2(3+4)` parses as `Mul(2, Add(3, 4))`. `^` and
    /// `=` recurse with an rbp one less than their LBP, making them
    /// right-associative; the others recurse with an rbp equal to their LBP,
    /// making them left-associative. Returns an error for any other token,
    /// since those cannot appear in infix position.
    ///
    /// # Example:
    /// ```ignore
    /// // led is private; see the `test_fn_parse` unit test, which exercises
    /// // led indirectly through Parser::parse.
    /// let src = "2+3";
    /// let mut tokens = Vec::new();
    /// let mut arena = Vec::new();
    /// Tokenizer::new(src).tokenize(&mut tokens).unwrap();
    /// let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
    /// assert!(matches!(arena[root as usize], Node::Add(_, _)));
    /// ```
    #[inline(always)]
    fn led(&mut self, left: u32) -> Result<u32, String> {
        let token = self.consume();
        match token {
            Token::Equals => {
                let right = self.pratt_parse(token.rbp())?; // rbp=4 → right-associative
                Ok(self.push(Node::Equation(left, right)))
            }
            Token::Plus => {
                let right = self.pratt_parse(token.rbp())?;
                Ok(self.push(Node::Add(left, right)))
            }
            Token::Minus => {
                let right = self.pratt_parse(token.rbp())?;
                Ok(self.push(Node::Sub(left, right)))
            }
            Token::Multiply => {
                let right = self.pratt_parse(token.rbp())?;
                Ok(self.push(Node::Mul(left, right)))
            }
            Token::Divide => {
                let right = self.pratt_parse(token.rbp())?;
                Ok(self.push(Node::Div(left, right)))
            }
            Token::Exponent => {
                let right = self.pratt_parse(token.rbp())?;
                Ok(self.push(Node::Pow(left, right)))
            }
            Token::LeftParenthesis => {
                let right = self.pratt_parse(token.rbp())?;
                self.expect_rparen()?;
                Ok(self.push(Node::Mul(left, right)))
            }
            other => unexpected_infix_token(&other),
        }
    }
}
#[inline(always)]
fn offset_tokens(tokens: &mut [Token], offset: u32) {
    for token in tokens {
        match token {
            Token::Number { start, end } | Token::Identifier { start, end, .. } => {
                *start += offset;
                *end += offset;
            }
            _ => {}
        }
    }
}
#[cold]
#[inline(never)]
fn invalid_number_error(raw: &str, e: &impl std::fmt::Display) -> String {
    format!("Invalid number '{}': {}", raw, e)
}

#[cold]
#[inline(never)]
fn unexpected_token_in_expression(token: &Token) -> Result<u32, String> {
    Err(format!("Unexpected token in expression: {:?}", token))
}

#[cold]
#[inline(never)]
fn unexpected_infix_token(token: &Token) -> Result<u32, String> {
    Err(format!("Unexpected token: {:?}", token))
}

#[cold]
#[inline(never)]
fn expected_rparen_error(found: &Token) -> String {
    format!("Expected ')', found {:?}", found)
}

#[cold]
#[inline(never)]
fn invalid_log_base_error(e: impl std::fmt::Display) -> String {
    format!("Invalid log base: {e}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Tokenizer;

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
        use std::assert_matches;
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "x^2+2*x=2*x-3";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();
        assert_matches!(arena[root as usize], Node::Equation(_, _));
    }

    #[test]
    fn test_log_base_number_parse() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "log_2(8)";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();

        let Node::LogBase(base, arg) = arena[root as usize] else {
            panic!("Expected LogBase");
        };
        assert_eq!(arena[base as usize], Node::Number(2.0));
        assert_eq!(arena[arg as usize], Node::Number(8.0));
    }

    #[test]
    fn test_log_base_variables_parse() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "log_b(k)";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();

        let Node::LogBase(base, arg) = arena[root as usize] else {
            panic!("Expected LogBase");
        };
        match arena[base as usize] {
            Node::Variable(start, end, _) => assert_eq!(&src[start as usize..end as usize], "b"),
            ref other => panic!("Expected base variable, got {other:?}"),
        }
        match arena[arg as usize] {
            Node::Variable(start, end, _) => assert_eq!(&src[start as usize..end as usize], "k"),
            ref other => panic!("Expected argument variable, got {other:?}"),
        }
    }

    #[test]
    fn test_log_base_symbolic_parse() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "log_x(k)";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();

        assert!(matches!(arena[root as usize], Node::LogBase(_, _)));
    }

    #[test]
    fn test_plain_log_stays_base_ten_parse() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "log(100)";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();

        assert!(matches!(arena[root as usize], Node::Log(_)));
    }

    #[test]
    fn test_empty_log_base_errors() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "log_(8)";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let err = Parser::new(&tokens, src, &mut arena).parse().unwrap_err();

        assert!(err.contains("Expected base after log_"));
    }

    #[test]
    fn test_grouped_log_base_errors() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "log_(2+1)(27)";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let err = Parser::new(&tokens, src, &mut arena).parse().unwrap_err();

        assert!(err.contains("Expected base after log_"));
    }

    #[test]
    fn test_empty_log_argument_errors() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "log_2()";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let err = Parser::new(&tokens, src, &mut arena).parse().unwrap_err();

        assert!(err.contains("Unexpected token in expression"));
    }

    #[test]
    fn test_bare_log_base_remains_variable() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "log_2";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let root = Parser::new(&tokens, src, &mut arena).parse().unwrap();

        assert!(matches!(arena[root as usize], Node::Variable(_, _, _)));
    }

    #[test]
    fn test_log_base_without_parentheses_errors() {
        let mut tokens = Vec::new();
        let mut arena = Vec::new();
        let src = "log_2 8";
        Tokenizer::new(src).tokenize(&mut tokens).unwrap();
        let err = Parser::new(&tokens, src, &mut arena).parse().unwrap_err();

        assert!(err.contains("Unexpected trailing token"));
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
