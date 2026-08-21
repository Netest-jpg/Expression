#![allow(dead_code)]
use fast_float2 as fast_float;

const FNV_OFFSET_BASIS: u64 = 14695981039346656037;
const FNV_PRIME: u64 = 1099511628211;

/// Computes hash for a byte slice using the FNV-1a algorithm at compile time.\
/// **FNV-1a algorithm:**\
/// `hash = (hash ^ byte).wrapping_mul(FNV_PRIME)`\
/// **FNV_OFFSET_BASIS** is used as the initial hash value.
/// # Returns
/// The hash of the bytes.
const fn keyword_hash(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    let mut i = 0;
    while i < bytes.len() {
        hash = (hash ^ bytes[i] as u64).wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

// common keyword hashes stored as constants instead of static because the hashs are in u64.
pub const KW_LN: u64 = keyword_hash(b"ln");
pub const KW_LOG: u64 = keyword_hash(b"log");

pub const KW_SIN: u64 = keyword_hash(b"sin");
pub const KW_COS: u64 = keyword_hash(b"cos");
pub const KW_TAN: u64 = keyword_hash(b"tan");

pub const KW_SEC: u64 = keyword_hash(b"sec");
pub const KW_CSC: u64 = keyword_hash(b"csc");
pub const KW_COT: u64 = keyword_hash(b"cot");

pub const KW_SQRT: u64 = keyword_hash(b"sqrt");
pub const KW_PI: u64 = keyword_hash(b"pi");
pub const KW_E: u64 = keyword_hash(b"e");

pub const KW_ASIN: u64 = keyword_hash(b"asin");
pub const KW_ACOS: u64 = keyword_hash(b"acos");
pub const KW_ATAN: u64 = keyword_hash(b"atan");

pub const KW_ACSC: u64 = keyword_hash(b"acsc");
pub const KW_ASEC: u64 = keyword_hash(b"asec");
pub const KW_ACOT: u64 = keyword_hash(b"acot");

pub const KW_SINH: u64 = keyword_hash(b"sinh");
pub const KW_COSH: u64 = keyword_hash(b"cosh");
pub const KW_TANH: u64 = keyword_hash(b"tanh");

pub const KW_SECH: u64 = keyword_hash(b"sech");
pub const KW_COTH: u64 = keyword_hash(b"coth");
pub const KW_CSCH: u64 = keyword_hash(b"csch");

pub const KW_ASINH: u64 = keyword_hash(b"asinh");
pub const KW_ACOSH: u64 = keyword_hash(b"acosh");
pub const KW_ATANH: u64 = keyword_hash(b"atanh");

pub const KW_ACOTH: u64 = keyword_hash(b"acoth");
pub const KW_ACSCH: u64 = keyword_hash(b"acsch");
pub const KW_ASECH: u64 = keyword_hash(b"asech");

static IS_IDENT: [u8; 32] = build_ident_table();
static DISPATCH: [Dispatch; 256] = build_dispatch_table();
/// Builds a bitset of 32 bytes **at compile-time**, where each byte represents 8 characters.\
/// Each bit in the byte is set to **1** if the corresponding character is an identifier character.\
/// # Allowed characters:
/// - `0` .. `9`
/// - `A` .. `Z`
/// - `a` .. `z`
/// - `_`
const fn build_ident_table() -> [u8; 32] {
    let mut t = [0; 32];

    let mut c = b'0';
    while c <= b'9' {
        t[c as usize / 8] |= 1 << (c as usize % 8);
        c += 1;
    }

    let mut c = b'A';
    while c <= b'Z' {
        t[c as usize / 8] |= 1 << (c as usize % 8);
        c += 1;
    }

    t[b'_' as usize / 8] |= 1 << (b'_' as usize % 8);

    let mut c = b'a';
    while c <= b'z' {
        t[c as usize / 8] |= 1 << (c as usize % 8);
        c += 1;
    }

    t
}

/// Takes in a u8 and returns a boolean indicating whether it is a valid identifier character.
#[inline(always)]
fn is_ident_char(c: u8) -> bool {
    unsafe { (*IS_IDENT.get_unchecked(c as usize / 8) >> (c % 8)) & 1 != 0 }
}

/// Value types for the `DISPATCH` lookup table.
#[repr(u8)] // stores the enum's discriminant as an 8-bit unsigned integer
#[derive(Clone, Copy)]
enum Dispatch {
    Whitespace,
    Number,
    Identifier,
    Plus,
    Minus,
    Multiply,
    Divide,
    Exponent,
    LParen,
    RParen,
    Equals,
    Unknown,
}

/// Builds the `DISPATCH` lookup table at compile time.\
/// Maps every possible byte (0-255) to a `Dispatch` enum variant.\
/// - Whitespace: ` `, `\t`, `\n`, `\r`
/// - Digits: `0-9`, `.`
/// - Alpha: `a-z`, `A-Z`
/// - Operators: `+`, `-`, `*`, `/`, `^`
/// - Parentheses: `(` , `)`
/// - Equals: `=`
/// - Unknown: all other bytes
const fn build_dispatch_table() -> [Dispatch; 256] {
    let mut t = [Dispatch::Unknown; 256];
    t[b' ' as usize] = Dispatch::Whitespace;
    t[b'\t' as usize] = Dispatch::Whitespace;
    t[b'\n' as usize] = Dispatch::Whitespace;
    t[b'\r' as usize] = Dispatch::Whitespace;
    let mut c = b'0';
    while c <= b'9' {
        t[c as usize] = Dispatch::Number;
        c += 1;
    }
    t[b'.' as usize] = Dispatch::Number;
    let mut c = b'a';
    while c <= b'z' {
        t[c as usize] = Dispatch::Identifier;
        c += 1;
    }
    let mut c = b'A';
    while c <= b'Z' {
        t[c as usize] = Dispatch::Identifier;
        c += 1;
    }
    t[b'_' as usize] = Dispatch::Identifier;
    t[b'+' as usize] = Dispatch::Plus;
    t[b'-' as usize] = Dispatch::Minus;
    t[b'*' as usize] = Dispatch::Multiply;
    t[b'/' as usize] = Dispatch::Divide;
    t[b'^' as usize] = Dispatch::Exponent;
    t[b'(' as usize] = Dispatch::LParen;
    t[b')' as usize] = Dispatch::RParen;
    t[b'=' as usize] = Dispatch::Equals;
    t
}

#[derive(Clone, PartialEq)]
pub enum Token {
    /// Byte range in the source. Call `.raw(src)` when needed.
    Number {
        start: u32,
        end: u32,
    },
    /// Byte range in the source. Call `.name(src)` when needed.
    Identifier {
        start: u32,
        end: u32,
        hash: u64,
    },
    Plus,
    Minus,
    Multiply,
    Divide,
    Exponent,
    LeftParenthesis,
    RightParenthesis,
    Equals,
    EndOfFile,
}

impl Token {
    /// Returns the raw source text for the corresponding number token.\
    /// Panics if called on any other token variant.
    /// # Example:
    /// ```rust
    /// let src = "123 + 456";
    /// let token = Token::Number { start: 0, end: 3};
    /// assert_eq!(token.raw(src), "123");
    /// ```
    ///
    /// ```should_panic
    /// let src = "123 + 456";
    /// let token = Token::Plus;
    /// token.raw(src); // panics
    /// ```
    #[inline(always)]
    pub fn raw<'src>(&self, src: &'src str) -> &'src str {
        match self {
            Token::Number { start, end } => &src[*start as usize..*end as usize],
            _ => panic!("Token::raw called on non-Number token"),
        }
    }

    /// Returns the raw source text for the corresponding identifier token.\
    /// Panics if called on any other token variant.
    /// # Example:
    /// ```rust
    /// let src = "2x+3";
    /// let token = Token::Identifer { start: 1, end: 2};
    /// assert_eq!(token.name(src),"x");
    /// ```
    ///
    /// ```should_panic
    /// let src = "2x+3";
    /// let token = Tokenn::Plus;
    /// token.name(src); // panics
    /// ```
    #[inline(always)]
    pub fn name<'src>(&self, src: &'src str) -> &'src str {
        match self {
            Token::Identifier { start, end, .. } => &src[*start as usize..*end as usize],
            _ => panic!("Token::name called on non-Identifier token"),
        }
    }

    /// Parse a raw string literal to f64.\
    /// Returns an error either if the string literal is not a valid decimal number
    /// or if any characters are left remaining unparsed.
    /// # Example:
    /// ```rust
    /// let src = "3.14"
    /// let token = Token::Number { start: 0, end: 4}
    /// assert_eq!(token.as_f64(src), 3.14);
    /// ```
    #[inline(always)]
    pub fn as_f64(&self, src: &str) -> f64 {
        fast_float::parse::<f64, &str>(self.raw(src)).expect("invalid number")
    }

    /// Returns the raw source text as Some() for the corresponding identifier token.\
    /// Returns None if called on any other token variant.
    /// # Example:
    /// ```rust
    /// let src = "2x+3";
    /// let token = Token::Identifer { start: 1, end: 2};
    /// assert_eq!(token.as_ident(src),"x");
    /// ```
    #[inline(always)]
    pub fn as_ident<'src>(&self, src: &'src str) -> Option<&'src str> {
        match self {
            Token::Identifier { start, end, .. } => Some(&src[*start as usize..*end as usize]),
            _ => None,
        }
    }
}

/// Formats a token as a human-readable string for debugging.
impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Number { start, end } => write!(f, "Number([{start}..{end}])"),
            Token::Identifier { start, end, .. } => write!(f, "Identifier([{start}..{end}])"),
            Token::Plus => write!(f, "Plus"),
            Token::Minus => write!(f, "Minus"),
            Token::Multiply => write!(f, "Multiply"),
            Token::Divide => write!(f, "Divide"),
            Token::Exponent => write!(f, "Exponent"),
            Token::LeftParenthesis => write!(f, "LeftParenthesis"),
            Token::RightParenthesis => write!(f, "RightParenthesis"),
            Token::Equals => write!(f, "Equals"),
            Token::EndOfFile => write!(f, "EndOfFile"),
        }
    }
}

pub struct Tokenizer<'src> {
    src: &'src [u8],
    pos: usize,
}

impl<'src> Tokenizer<'src> {
    /// Creates a new [`Tokenizer`] with the supplied string literal.\
    /// Stores the string literal as a byte slice
    /// and sets the pos 0.
    pub fn new(input: &'src str) -> Self {
        Tokenizer {
            src: input.as_bytes(),
            pos: 0,
        }
    }

    /// Takes in a mutable vector of Token,
    /// clears out the pre-existing elements inside the vector,
    /// makes a rough estimate to set for token capacity (`hint = self.src.len()/2+2`),
    /// reserves capacity for `hint - tokens.len()`
    /// tokenizes the src's byte slice into a vector of `Token`s and pushes them into the vector.
    /// # Example:
    /// ```rust
    /// let src = "2x+3";
    /// let mut expression = Tokenizer::new(src);
    /// let mut tokens = Vec::new();
    /// expression.tokenize(&mut tokens).unwrap();
    /// assert_eq!(tokens.len(), 6);
    /// assert_eq!(tokens[0], Token::Number { start: 0, end: 1 });
    /// assert_eq!(tokens[1], Token::Multiply);
    /// assert_eq!(tokens[3], Token::Plus);
    /// assert_eq!(tokens[4], Token::Number { start: 3, end: 4 });
    /// assert_eq!(tokens[5], Token::EndOfFile);

    /// assert_eq!(tokens[0].raw(src), "2");
    /// assert_eq!(tokens[2].name(src), "x");
    /// assert_eq!(tokens[4].raw(src), "3");
    /// ```
    pub fn tokenize(&mut self, tokens: &mut Vec<Token>) -> Result<(), String> {
        tokens.clear();
        // Typical token is 2-3 chars; src.len()/2+2 avoids the large
        // over-allocation that src.len()+1 causes for identifier-heavy input.
        let hint = self.src.len() / 2 + 2;
        if tokens.capacity() < hint {
            tokens.reserve(hint - tokens.len());
        }
        loop {
            let byte = match self.current() {
                None => {
                    tokens.push(Token::EndOfFile);
                    break;
                }
                Some(b) => b,
            };

            match unsafe { *DISPATCH.get_unchecked(byte as usize) } {
                Dispatch::Whitespace => {
                    self.advance();
                }

                Dispatch::Number => {
                    let token = self.read_number()?;
                    // implicit multiply: "2x" → Number Asterisk Identifier
                    let implicit = self.current().is_some_and(is_ident_char);
                    tokens.push(token);
                    if implicit {
                        tokens.push(Token::Multiply);
                    }
                }

                Dispatch::Identifier => {
                    tokens.push(self.read_identifier());
                }

                Dispatch::Plus => {
                    self.advance();
                    tokens.push(Token::Plus);
                }

                Dispatch::Minus => {
                    self.advance();
                    tokens.push(Token::Minus);
                }

                Dispatch::Multiply => {
                    self.advance();
                    tokens.push(Token::Multiply);
                }

                Dispatch::Divide => {
                    self.advance();
                    tokens.push(Token::Divide);
                }

                Dispatch::Exponent => {
                    self.advance();
                    tokens.push(Token::Exponent);
                }

                Dispatch::LParen => {
                    self.advance();
                    tokens.push(Token::LeftParenthesis);
                }

                Dispatch::RParen => {
                    self.advance();
                    tokens.push(Token::RightParenthesis);
                }

                Dispatch::Equals => {
                    self.advance();
                    tokens.push(Token::Equals);
                }

                Dispatch::Unknown => {
                    return Err(format!("Unknown character: '{}'", byte as char));
                }
            }
        }
        Ok(())
    }

    /// Returns the current byte, else returns `None`.
    #[inline(always)]
    fn current(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    /// Advances the tokenizer to the next byte.\
    /// i.e. it sets the Tokenizer's pos to the next byte's index.
    #[inline(always)]
    fn advance(&mut self) {
        self.pos += 1;
    }

    /// Reads a numeric literal from the current position.
    /// The number may should contain digits and at most one decimal point.\
    /// \
    /// Returns `Token::Number` spanning the consumed source range,
    /// **i.e.**, until the number is finished.\
    /// \
    /// Returns an Error if there are multiple decimal points or if there is no digit
    ///
    /// # Example:
    /// ```rust
    /// ```
    #[inline(always)]
    fn read_number(&mut self) -> Result<Token, String> {
        let start = self.pos as u32;
        let mut dot_seen = false;
        let mut digit_seen = false;

        while let Some(byte) = self.current() {
            if byte.is_ascii_digit() {
                digit_seen = true;
                self.advance();
            } else if byte == b'.' && !dot_seen {
                dot_seen = true;
                self.advance();
            } else if byte == b'.' {
                let invalid_number =
                    std::str::from_utf8(&self.src[start as usize..=self.pos]).unwrap();
                return Err(format!(
                    "Invalid number: Unexpected second '.' in '{}'",
                    invalid_number
                ));
            } else {
                break;
            }
        }

        if !digit_seen {
            return Err("Invalid number: Expected at least one digit".to_string());
        }

        Ok(Token::Number {
            start,
            end: self.pos as u32,
        })
    }

    /// Reads an identifier from the input string and returns it as a [`Token::Identifier`].
    #[inline(always)]
    fn read_identifier(&mut self) -> Token {
        let start = self.pos as u32;
        let mut hash = FNV_OFFSET_BASIS;

        while let Some(byte) = self.current() {
            if is_ident_char(byte) {
                hash = fnv1a_update(hash, byte);
                self.advance();
            } else {
                break;
            }
        }

        Token::Identifier {
            start,
            end: self.pos as u32,
            hash,
        }
    }
}

/// Computes hash for one byte using the FNV-1a algorithm.\
/// **FNV-1a algorithm:**\
/// `hash = (hash ^ byte).wrapping_mul(FNV_PRIME)`\
/// # Returns
/// The hash of a byte.
#[inline(always)]
fn fnv1a_update(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(FNV_PRIME)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(s: &str) -> (Vec<Token>, String) {
        let mut tokens = Vec::new();
        Tokenizer::new(s).tokenize(&mut tokens).unwrap();
        (tokens, s.to_string())
    }

    #[test]
    fn test_basic_operators() {
        let (tokens, _) = tok("+ - * / ^ ( )");
        assert!(matches!(tokens[0], Token::Plus));
        assert!(matches!(tokens[1], Token::Minus));
        assert!(matches!(tokens[2], Token::Multiply));
        assert!(matches!(tokens[3], Token::Divide));
        assert!(matches!(tokens[4], Token::Exponent));
        assert!(matches!(tokens[5], Token::LeftParenthesis));
        assert!(matches!(tokens[6], Token::RightParenthesis));
    }

    #[test]
    fn test_fn_raw() {
        let src = "123 + 456";
        let token = Token::Number { start: 0, end: 3 };
        assert_eq!(token.raw(src), "123");
    }

    #[test]
    #[should_panic(expected = "Token::raw called on non-Number token")]
    fn test_fn_raw_panics_on_non_number() {
        let src = "123 + 456";
        let token = Token::Plus;

        token.raw(src);
    }

    #[test]
    fn test_fn_name() {
        let src = "2x+3";
        let token = Token::Identifier {
            start: 1,
            end: 2,
            hash: 111111,
        };
        assert_eq!(token.name(src), "x");
    }

    #[test]
    #[should_panic(expected = "Token::name called on non-Identifier token")]
    fn test_fn_name_panics_on_non_identifier() {
        let src = "2x+3";
        let token = Token::Plus;

        token.name(src);
    }

    #[test]
    fn test_fn_to_f64() {
        let src = "3.14";
        let token = Token::Number { start: 0, end: 4 };
        assert_eq!(token.as_f64(src), 3.14);
    }

    #[test]
    fn test_fn_tokenize() {
        let src = "2x+3";
        let mut expression = Tokenizer::new(src);
        let mut tokens = Vec::new();
        expression.tokenize(&mut tokens).unwrap();
        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0], Token::Number { start: 0, end: 1 });
        assert_eq!(tokens[1], Token::Multiply);
        assert_eq!(tokens[3], Token::Plus);
        assert_eq!(tokens[4], Token::Number { start: 3, end: 4 });
        assert_eq!(tokens[5], Token::EndOfFile);

        assert_eq!(tokens[0].raw(src), "2");
        assert_eq!(tokens[2].name(src), "x");
        assert_eq!(tokens[4].raw(src), "3");
    }

    #[test]
    fn test_fn_read_number() {
        let src = "1234";
        let mut expression = Tokenizer::new(src);
        let result = expression.read_number();
        assert_eq!(result, Ok(Token::Number { start: 0, end: 4 }));
    }

    #[test]
    fn test_number_lazy_parse() {
        let src = "3.14";
        let (tokens, _) = tok(src);
        match tokens[0] {
            Token::Number { start, end } => {
                assert_eq!(&src[start as usize..end as usize], "3.14");
                assert!((tokens[0].as_f64(src) - 314.0 / 100.0).abs() < 1e-10);
            }
            _ => panic!("expected Number"),
        }
    }

    #[test]
    fn test_bare_dot_is_not_number() {
        let mut tokens = Vec::new();
        let result = Tokenizer::new(".").tokenize(&mut tokens);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected at least one digit"));
    }

    #[test]
    fn test_identifier_hash_stability() {
        let (t1, _) = tok("foo");
        let (t2, _) = tok("foo");
        match (&t1[0], &t2[0]) {
            (Token::Identifier { hash: h1, .. }, Token::Identifier { hash: h2, .. }) => {
                assert_eq!(h1, h2);
            }
            _ => panic!("expected Identifiers"),
        }
    }

    #[test]
    fn test_implicit_multiply() {
        let (tokens, _) = tok("2x");
        assert!(matches!(tokens[0], Token::Number { .. }));
        assert!(matches!(tokens[1], Token::Multiply));
        assert!(matches!(tokens[2], Token::Identifier { .. }));
    }

    #[test]
    fn test_keyword_hash_lookup() {
        let (tokens, _) = tok("sin");
        match tokens[0] {
            Token::Identifier { hash, .. } => assert_eq!(hash, KW_SIN),
            _ => panic!("expected Identifier"),
        }
    }

    #[test]
    fn test_whitespace_skipped() {
        let (tokens, _) = tok("  \t1\n+\t2  ");
        // Number(1), Plus, Number(2), EndOfFile
        assert_eq!(tokens.len(), 4);
    }

    #[test]
    fn test_debug_no_hash() {
        let (tokens, _) = tok("3x+3");
        // Debug shows offsets, not hash
        let s = format!("{:?}", tokens[0]);
        assert!(s.starts_with("Number("));
        let s = format!("{:?}", tokens[2]);
        assert!(s.starts_with("Identifier("));
    }
}
