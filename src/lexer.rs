#![allow(dead_code)] // public API items used by downstream consumers

// -----------------------------------------------------------------------
// FNV-1a constants (same as xnacly's C lexer)
// -----------------------------------------------------------------------
const FNV_OFFSET_BASIS: u64 = 14695981039346656037;
const FNV_PRIME: u64 = 1099511628211;

const fn keyword_hash(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    let mut i = 0;
    while i < bytes.len() {
        hash = (hash ^ bytes[i] as u64).wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

#[inline(always)]
fn fnv1a_update(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(FNV_PRIME)
}

// -----------------------------------------------------------------------
// O(1) character-class lookup table
// -----------------------------------------------------------------------
const fn build_ident_table() -> [bool; 256] {
    let mut t = [false; 256];
    let mut c = b'a';
    while c <= b'z' {
        t[c as usize] = true;
        c += 1;
    }
    let mut c = b'A';
    while c <= b'Z' {
        t[c as usize] = true;
        c += 1;
    }
    let mut c = b'0';
    while c <= b'9' {
        t[c as usize] = true;
        c += 1;
    }
    t[b'_' as usize] = true;
    t
}
static IS_IDENT: [bool; 256] = build_ident_table();

#[inline(always)]
fn is_ident_char(c: u8) -> bool {
    unsafe { *IS_IDENT.get_unchecked(c as usize) }
}

// -----------------------------------------------------------------------
// Dispatch table
// -----------------------------------------------------------------------
#[repr(u8)]
#[derive(Clone, Copy)]
enum Dispatch {
    Whitespace,
    Digit,
    Alpha,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Unknown,
}

const fn build_dispatch_table() -> [Dispatch; 256] {
    let mut t = [Dispatch::Unknown; 256];
    t[b' ' as usize] = Dispatch::Whitespace;
    t[b'\t' as usize] = Dispatch::Whitespace;
    t[b'\n' as usize] = Dispatch::Whitespace;
    t[b'\r' as usize] = Dispatch::Whitespace;
    let mut c = b'0';
    while c <= b'9' {
        t[c as usize] = Dispatch::Digit;
        c += 1;
    }
    t[b'.' as usize] = Dispatch::Digit;
    let mut c = b'a';
    while c <= b'z' {
        t[c as usize] = Dispatch::Alpha;
        c += 1;
    }
    let mut c = b'A';
    while c <= b'Z' {
        t[c as usize] = Dispatch::Alpha;
        c += 1;
    }
    t[b'_' as usize] = Dispatch::Alpha;
    t[b'+' as usize] = Dispatch::Plus;
    t[b'-' as usize] = Dispatch::Minus;
    t[b'*' as usize] = Dispatch::Star;
    t[b'/' as usize] = Dispatch::Slash;
    t[b'^' as usize] = Dispatch::Caret;
    t[b'(' as usize] = Dispatch::LParen;
    t[b')' as usize] = Dispatch::RParen;
    t
}
static DISPATCH: [Dispatch; 256] = build_dispatch_table();

// -----------------------------------------------------------------------
// Token
//
// Number and Identifier store byte offsets (u32) into the source string
// rather than borrowed slices. This makes Token<'static> (no lifetime
// parameter), so the tokens Vec can be declared outside the REPL loop and
// reuse its heap allocation across iterations.
//
// Call `.raw(src)` / `.name(src)` to recover the string slice when needed.
// -----------------------------------------------------------------------
#[derive(Clone, PartialEq)]
pub enum Token {
    /// Byte range in the source + FNV hash. Call `.raw(src)` when needed.
    Number {
        start: u32,
        end: u32,
        hash: u64,
    },
    /// Byte range in the source + FNV hash for O(1) keyword checks.
    Identifier {
        start: u32,
        end: u32,
        hash: u64,
    },
    Plus,
    Minus,
    Asterisk,
    ForwardSlash,
    Caret,
    LeftParenthesis,
    RightParenthesis,
    EndOfFile,
}

impl Token {
    /// Resolve the raw number text from the original source.
    #[inline(always)]
    pub fn raw<'src>(&self, src: &'src str) -> &'src str {
        match self {
            Token::Number { start, end, .. } => &src[*start as usize..*end as usize],
            _ => panic!("Token::raw called on non-Number token"),
        }
    }

    /// Resolve the identifier name from the original source.
    #[inline(always)]
    pub fn name<'src>(&self, src: &'src str) -> &'src str {
        match self {
            Token::Identifier { start, end, .. } => &src[*start as usize..*end as usize],
            _ => panic!("Token::name called on non-Identifier token"),
        }
    }

    /// Parse the numeric value on demand.
    pub fn as_f64(&self, src: &str) -> f64 {
        self.raw(src).parse().expect("invalid number")
    }

    /// Returns the identifier name, or None.
    pub fn as_ident<'src>(&self, src: &'src str) -> Option<&'src str> {
        match self {
            Token::Identifier { start, end, .. } => Some(&src[*start as usize..*end as usize]),
            _ => None,
        }
    }
}

/// Human-friendly Debug: resolves nothing (no src available), shows offsets.
impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Number { start, end, .. } => write!(f, "Number([{start}..{end}])"),
            Token::Identifier { start, end, .. } => write!(f, "Identifier([{start}..{end}])"),
            Token::Plus => write!(f, "Plus"),
            Token::Minus => write!(f, "Minus"),
            Token::Asterisk => write!(f, "Asterisk"),
            Token::ForwardSlash => write!(f, "ForwardSlash"),
            Token::Caret => write!(f, "Caret"),
            Token::LeftParenthesis => write!(f, "LeftParenthesis"),
            Token::RightParenthesis => write!(f, "RightParenthesis"),
            Token::EndOfFile => write!(f, "EndOfFile"),
        }
    }
}

// -----------------------------------------------------------------------
// Pre-computed keyword hashes.
// -----------------------------------------------------------------------
pub const KW_SIN: u64 = keyword_hash(b"sin");
pub const KW_COS: u64 = keyword_hash(b"cos");
pub const KW_TAN: u64 = keyword_hash(b"tan");
pub const KW_LN: u64 = keyword_hash(b"ln");
pub const KW_LOG: u64 = keyword_hash(b"log");
pub const KW_SQRT: u64 = keyword_hash(b"sqrt");
pub const KW_PI: u64 = keyword_hash(b"pi");
pub const KW_E: u64 = keyword_hash(b"e");

// -----------------------------------------------------------------------
// Tokenizer
// -----------------------------------------------------------------------
pub struct Tokenizer<'src> {
    src: &'src [u8],
    pos: usize,
}

impl<'src> Tokenizer<'src> {
    pub fn new(input: &'src str) -> Self {
        Tokenizer {
            src: input.as_bytes(),
            pos: 0,
        }
    }

    #[inline(always)]
    fn current(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    #[inline(always)]
    fn advance(&mut self) {
        self.pos += 1;
    }

    /// Tokenize the entire input into `tokens` (cleared first).
    pub fn tokenize(&mut self, tokens: &mut Vec<Token>) -> Result<(), String> {
        tokens.clear();
        tokens.reserve(self.src.len() + 1);
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

                Dispatch::Digit => {
                    let tok = self.read_number()?;
                    // implicit multiply: "2x" → Number Asterisk Identifier
                    let implicit = self.current().map_or(false, |b| IS_IDENT[b as usize]);
                    tokens.push(tok);
                    if implicit {
                        tokens.push(Token::Asterisk);
                    }
                }

                Dispatch::Alpha => {
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
                Dispatch::Star => {
                    self.advance();
                    tokens.push(Token::Asterisk);
                }
                Dispatch::Slash => {
                    self.advance();
                    tokens.push(Token::ForwardSlash);
                }
                Dispatch::Caret => {
                    self.advance();
                    tokens.push(Token::Caret);
                }
                Dispatch::LParen => {
                    self.advance();
                    tokens.push(Token::LeftParenthesis);
                }
                Dispatch::RParen => {
                    self.advance();
                    tokens.push(Token::RightParenthesis);
                }

                Dispatch::Unknown => {
                    return Err(format!("Unknown character: '{}'", byte as char));
                }
            }
        }
        Ok(())
    }

    fn read_number(&mut self) -> Result<Token, String> {
        let start = self.pos as u32;
        let mut hash = FNV_OFFSET_BASIS;
        let mut dot_seen = false;
        let mut digit_seen = false;

        while let Some(b) = self.current() {
            if b.is_ascii_digit() {
                digit_seen = true;
                hash = fnv1a_update(hash, b);
                self.advance();
            } else if b == b'.' && !dot_seen {
                dot_seen = true;
                hash = fnv1a_update(hash, b);
                self.advance();
            } else if b == b'.' {
                let so_far = std::str::from_utf8(&self.src[start as usize..self.pos]).unwrap();
                return Err(format!(
                    "Invalid number: unexpected second '.' in '{}'",
                    so_far
                ));
            } else {
                break;
            }
        }

        if !digit_seen {
            return Err("Invalid number: expected at least one digit".to_string());
        }

        Ok(Token::Number {
            start,
            end: self.pos as u32,
            hash,
        })
    }

    fn read_identifier(&mut self) -> Token {
        let start = self.pos as u32;
        let mut hash = FNV_OFFSET_BASIS;

        while let Some(b) = self.current() {
            if is_ident_char(b) {
                hash = fnv1a_update(hash, b);
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

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------
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
        assert!(matches!(tokens[2], Token::Asterisk));
        assert!(matches!(tokens[3], Token::ForwardSlash));
        assert!(matches!(tokens[4], Token::Caret));
        assert!(matches!(tokens[5], Token::LeftParenthesis));
        assert!(matches!(tokens[6], Token::RightParenthesis));
    }

    #[test]
    fn test_number_lazy_parse() {
        let src = "3.14";
        let (tokens, _) = tok(src);
        match tokens[0] {
            Token::Number { start, end, .. } => {
                assert_eq!(&src[start as usize..end as usize], "3.14");
                assert!((tokens[0].as_f64(src) - 3.14).abs() < 1e-10);
            }
            _ => panic!("expected Number"),
        }
    }

    #[test]
    fn test_bare_dot_is_not_number() {
        let mut tokens = Vec::new();
        let result = Tokenizer::new(".").tokenize(&mut tokens);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected at least one digit"));
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
        assert!(matches!(tokens[1], Token::Asterisk));
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
