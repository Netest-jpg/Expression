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

    /// Parses the full token stream into a single expression, consuming the parser.
    /// Returns the arena index of the root node.
    ///
    /// # Errors:
    /// Returns `Err` if the tokens don't form a valid expression or if any non-`Token::EndOfFile` tokens remain after the expression is parsed.
    ///
    /// E.g., "2+3 4" - here, 4 is a trailing input with no connecting operator
    ///
    /// # Example:
    /// ```rust
    /// use expression::lexer::Tokenizer;
    /// use expression::parser::{Node, Parser};
    ///
    /// let mut tokens = Vec::new();
    /// let mut arena = Vec::new();
    /// let src = "x^2+2*x=2*x-3";
    /// Tokenizer::new(src).tokenize(&mut tokens).unwrap();
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
    //         Token::Number { start, end } | Token::Variable { start, end, .. } => unsafe {
    //             self.src.get_unchecked(*start as usize..*end as usize)
    //         },
    //         _ => "",
    //     }
    // }

    /// Parses an expression, consuming infix operators while their `lbp`
    /// exceeds `rbp`.
    ///
    /// `rbp` is the minimum binding power the caller will accept — pass the
    /// current operator's `lbp` for left-associative recursion, or a value
    /// less than its `lbp` for right-associative recursion (see
    /// [`Token::rbp`]). Pass `0` to parse a full expression with no upper
    /// bound on what gets consumed.
    ///
    /// # Errors
    /// Returns `Err` if the token stream doesn't form a valid expression
    /// from the current position.
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

    /// Parses a primary expression: a number, variable/constant, unary minus,
    /// function call, implicit-multiply call, or parenthesized group.
    ///
    /// Handles:
    /// 1. `Token::Number` — parses the literal into `Node::Number`.
    ///
    /// 2. `Token::Variable` — resolves as follows:
    ///   - `pi` / `e` → the corresponding constant.
    ///   - followed by `(`:
    ///     - name starts with `log_` → parses a `log_<base>(...)` call.
    ///     - name matches a known function (e.g. `sin`) → a function call.
    ///     - otherwise → implicit multiplication (`x(3)` → `x*3`).
    ///   - otherwise → a plain variable reference.
    ///
    /// 3. `Token::Minus` — unary negation, binds tighter than `*` or `/` but
    ///   looser than `^` (see the `25` binding power).
    ///
    /// 4. `Token::LeftParenthesis` — parses a fully parenthesized sub-expression.
    ///
    /// # Errors
    /// Returns `Err` if:
    /// 1. The current token can't start an expression (e.g. an infix operator
    ///   or `EndOfFile` in prefix position).
    /// 2. A number literal fails to parse.
    /// 3. A `log_` call's base is invalid.
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

            Token::Variable { start, end, hash } => {
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
                        let arg = self.pratt_parse(0)?;
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

    /// Returns `Token` at the current `position` and advances `position` by 1.
    ///
    /// # Safety
    /// This function does not perform bounds checking.
    /// The caller must ensure that `position` is less than the number of tokens.
    /// Otherwise, this function invokes undefined behavior.
    #[inline(always)]
    fn consume(&mut self) -> Token {
        let token = unsafe { self.tokens.get_unchecked(self.position) }.clone();
        self.position += 1;
        token
    }

    /// Parses the base out of a `log_<base>` identifier and returns its
    /// arena index.
    ///
    /// `start`/`end` are the byte range of the full identifier (e.g. `log_2`);
    /// the base is everything after the `log_` prefix (4 bytes). The base
    /// text is re-tokenized and parsed as its own expression, with resulting
    /// token offsets adjusted to stay consistent with the original source
    /// positions (so error messages point at the right place in `self.src`).
    ///
    /// # Errors
    /// - Returns `Err` if there's no text after the `log_` prefix.
    /// - Returns `Err` if the base fails to tokenize.
    /// - Returns `Err` if the base fails to parse as a valid expression.
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

    /// Stores a `Node` in the parser's `arena` and returns the index of the newly appended `Node`.
    ///
    /// Arena indices are `u32`; parsing an expression that produces more than `u32::MAX` nodes will silently wrap.
    #[inline(always)]
    fn push(&mut self, node: Node) -> u32 {
        let index = self.arena.len() as u32;
        self.arena.push(node);
        index
    }

    /// Consumes the current token if it's `Token::RightParenthesis`
    ///
    /// # Errors
    /// Returns `Err` if the current token is not `Token::RightParenthesis`.
    #[inline(always)]
    fn expect_rparen(&mut self) -> Result<(), String> {
        if !matches!(self.peek(), Token::RightParenthesis) {
            return Err(expected_rparen_error(self.peek()));
        }
        self.skip();
        Ok(())
    }

    /// Returns `Token` at the current `position`.
    ///
    /// # Safety
    /// This function doesn't perform bounds checking.
    /// The caller must ensure that `position` must be less than the number of tokens.
    #[inline(always)]
    fn peek(&self) -> &Token {
        unsafe { self.tokens.get_unchecked(self.position) }
    }

    /// Skips to the next position.
    #[inline(always)]
    fn skip(&mut self) {
        self.position += 1;
    }

    /// Parses the infix continuation of an expression whose left operand is
    /// already parsed, using the just-consumed token as the operator.
    ///
    /// Handles:
    /// - `Token::Equals` — right-associative; builds `Node::Equation`.
    /// - `Token::Plus` / `Token::Minus` / `Token::Multiply` / `Token::Divide`
    ///   — left-associative; build the corresponding arithmetic node.
    /// - `Token::Exponent` — right-associative; builds `Node::Pow`.
    /// - `Token::LeftParenthesis` — implicit multiplication (`x(3)` → `x*3`);
    ///   parses the parenthesized right operand and builds `Node::Mul`.
    ///
    /// # Errors
    /// - The right operand fails to parse.
    /// - `Token::LeftParenthesis` is not followed by a matching `)`.
    /// - The consumed token isn't a valid infix operator.
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

/// Shifts every `Token::Number` and `Token::Variable`'s `start`/`end`
/// span by `offset`, in place.
///
/// Used after tokenizing a source substring (e.g. a `log_<base>` slice)
/// so its tokens' spans line up with the original full source instead
/// of being relative to the substring.
///
/// Tokens with no span (operators, parentheses, `EndOfFile`, etc.) are
/// left unchanged.
#[inline(always)]
fn offset_tokens(tokens: &mut [Token], offset: u32) {
    for token in tokens {
        match token {
            Token::Number { start, end } | Token::Variable { start, end, .. } => {
                *start += offset;
                *end += offset;
            }
            _ => {}
        }
    }
}

// Error Handles
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
