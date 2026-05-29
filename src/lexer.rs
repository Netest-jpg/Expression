pub mod lexer {

    #[derive(Debug, Clone, PartialEq)]
    pub enum Token {
        Number(f64),
        Identifier(String),
        Plus,
        Minus,
        Asterisk,
        ForwardSlash,
        Caret,
        LeftParenthesis,
        RightParenthesis,
        EndOfFile,
    }

    pub struct Tokenizer {
        input: Vec<char>,
        position: usize,
    }

    impl Tokenizer {
        pub fn new(input: &str) -> Tokenizer {
            Tokenizer {
                input: input.chars().collect(),
                position: 0,
            }
        }

        fn current(&self) -> Option<char> {
            if self.position < self.input.len() {
                Some(self.input[self.position])
            } else {
                None
            }
        }

        fn advance(&mut self) -> char {
            let character = self.input[self.position];
            self.position += 1;
            character
        }

        pub fn tokenize(&mut self) -> Vec<Token> {
            let mut tokens = Vec::new();

            loop {
                match self.current() {
                    None => {
                        tokens.push(Token::EndOfFile);
                        break;
                    }

                    Some(character) => match character {
                        ' ' | '\t' => {
                            self.advance();
                        }

                        '0'..='9' | '.' => {
                            let number = self.read_number();
                            tokens.push(Token::Number(number));
                            // if a letter immediately follows a number, insert a * automatically
                            if let Some(character) = self.current() {
                                if character.is_alphabetic() {
                                    tokens.push(Token::Asterisk);
                                }
                            }
                        }

                        'a'..='z' | 'A'..='Z' => {
                            let name = self.read_identifier();
                            tokens.push(Token::Identifier(name));
                        }

                        '+' => {
                            self.advance();
                            tokens.push(Token::Plus);
                        }
                        '-' => {
                            self.advance();
                            tokens.push(Token::Minus);
                        }
                        '*' => {
                            self.advance();
                            tokens.push(Token::Asterisk);
                        }
                        '/' => {
                            self.advance();
                            tokens.push(Token::ForwardSlash);
                        }
                        '^' => {
                            self.advance();
                            tokens.push(Token::Caret);
                        }
                        '(' => {
                            self.advance();
                            tokens.push(Token::LeftParenthesis);
                        }
                        ')' => {
                            self.advance();
                            tokens.push(Token::RightParenthesis);
                        }

                        other => {
                            panic!("Unknown character: '{}'", other);
                        }
                    },
                }
            }

            tokens
        }

        fn read_number(&mut self) -> f64 {
            let mut accumulated_text = String::new();
            let mut dot_seen = false;

            while let Some(character) = self.current() {
                if character.is_ascii_digit() {
                    accumulated_text.push(character);
                    self.advance();
                } else if character == '.' && !dot_seen {
                    dot_seen = true;
                    accumulated_text.push(character);
                    self.advance();
                } else if character == '.' && dot_seen {
                    panic!(
                        "Invalid number: unexpected second '.' in '{}'",
                        accumulated_text
                    );
                } else {
                    break;
                }
            }

            accumulated_text
                .parse()
                .unwrap_or_else(|_| panic!("Invalid number: '{}'", accumulated_text))
        }

        fn read_identifier(&mut self) -> String {
            let mut accumulated_text = String::new();

            while let Some(character) = self.current() {
                if character.is_ascii_alphanumeric() {
                    accumulated_text.push(character);
                    self.advance();
                } else {
                    break;
                }
            }

            accumulated_text
        }
    }
}
