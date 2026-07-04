//! Lexer for the Crust language.
//!
//! Converts a Crust source string into a stream of tokens, recognizing C-style
//! primitive type keywords and pointer-related symbols.

/// Represent all possible tokens in the Crust language dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    // Keywords
    Fn,
    Struct,
    Let,
    Mut,
    Const,
    Return,
    Extern,
    Unsafe,
    Use,
    Impl,
    As,

    // C Primitive Keywords
    Void,
    Int,
    Uint,
    Long,
    Char,

    // Identifiers & Literals
    Ident(String),
    IntLit(String),
    StrLit(String),
    CharLit(char),

    // Symbols & Operators
    Star,               // `*`
    Ampersand,          // `&`
    Arrow,              // `->`
    Colon,              // `:`
    DoubleColon,        // `::`
    Semicolon,          // `;`
    Comma,              // `,`
    Eq,                 // `=`
    EqEq,               // `==`
    Plus,               // `+`
    Minus,              // `-`
    Slash,              // `/`
    Pound,              // `#`
    OpenParen,          // `(`
    CloseParen,         // `)`
    OpenBrace,          // `{`
    CloseBrace,         // `}`
    OpenBracket,        // `[`
    CloseBracket,       // `]`
    Dot,                // `.`
    Bang,               // `!`
}

/// A Lexer that processes the source string.
pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Lexer<'a> {
    /// Creates a new Lexer for the given input.
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
        }
    }

    /// Read next character.
    fn next_char(&mut self) -> Option<char> {
        self.chars.next()
    }

    /// Peek next character.
    fn peek_char(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    /// Scan all tokens.
    pub fn tokenize(mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        while let Some(&c) = self.peek_char() {
            if c.is_whitespace() {
                self.next_char();
                continue;
            }

            // Handle comments
            if c == '/' {
                self.next_char();
                if let Some(&'/') = self.peek_char() {
                    // Line comment
                    self.next_char();
                    while let Some(nc) = self.next_char() {
                        if nc == '\n' {
                            break;
                        }
                    }
                    continue;
                } else if let Some(&'*') = self.peek_char() {
                    // Block comment
                    self.next_char();
                    let mut closed = false;
                    while let Some(nc) = self.next_char() {
                        if nc == '*' {
                            if let Some(&'/') = self.peek_char() {
                                self.next_char();
                                closed = true;
                                break;
                            }
                        }
                    }
                    if !closed {
                        return Err("Unterminated block comment".to_string());
                    }
                    continue;
                } else {
                    tokens.push(Token::Slash);
                    continue;
                }
            }

            // Handle identifiers and keywords
            if c.is_alphabetic() || c == '_' {
                let mut ident = String::new();
                ident.push(self.next_char().unwrap());
                while let Some(&nc) = self.peek_char() {
                    if nc.is_alphanumeric() || nc == '_' {
                        ident.push(self.next_char().unwrap());
                    } else {
                        break;
                    }
                }

                let token = match ident.as_str() {
                    "fn" => Token::Fn,
                    "struct" => Token::Struct,
                    "let" => Token::Let,
                    "mut" => Token::Mut,
                    "const" => Token::Const,
                    "return" => Token::Return,
                    "extern" => Token::Extern,
                    "unsafe" => Token::Unsafe,
                    "use" => Token::Use,
                    "impl" => Token::Impl,
                    "as" => Token::As,
                    "void" => Token::Void,
                    "int" => Token::Int,
                    "uint" => Token::Uint,
                    "long" => Token::Long,
                    "char" => Token::Char,
                    _ => Token::Ident(ident),
                };
                tokens.push(token);
                continue;
            }

            // Handle digits (integer literals)
            if c.is_ascii_digit() {
                let mut num = String::new();
                while let Some(&nc) = self.peek_char() {
                    if nc.is_ascii_digit() {
                        num.push(self.next_char().unwrap());
                    } else {
                        break;
                    }
                }
                tokens.push(Token::IntLit(num));
                continue;
            }

            // Handle string literals
            if c == '"' {
                self.next_char(); // Consume opening quote
                let mut s = String::new();
                let mut escaped = false;
                let mut closed = false;
                while let Some(nc) = self.next_char() {
                    if escaped {
                        s.push(nc);
                        escaped = false;
                    } else if nc == '\\' {
                        escaped = true;
                    } else if nc == '"' {
                        closed = true;
                        break;
                    } else {
                        s.push(nc);
                    }
                }
                if !closed {
                    return Err("Unterminated string literal".to_string());
                }
                tokens.push(Token::StrLit(s));
                continue;
            }

            // Handle char literals
            if c == '\'' {
                self.next_char(); // Consume opening single quote
                let val = match self.next_char() {
                    Some('\\') => match self.next_char() {
                        Some('n') => '\n',
                        Some('r') => '\r',
                        Some('t') => '\t',
                        Some('0') => '\0',
                        Some(escaped) => escaped,
                        None => return Err("Unterminated char literal escape".to_string()),
                    },
                    Some(other) => other,
                    None => return Err("Empty or unterminated char literal".to_string()),
                };
                if self.next_char() != Some('\'') {
                    return Err("Expected closing single quote for char literal".to_string());
                }
                tokens.push(Token::CharLit(val));
                continue;
            }

            // Handle operators and punctuation symbols
            let current = self.next_char().unwrap();
            match current {
                ':' => {
                    if let Some(&':') = self.peek_char() {
                        self.next_char();
                        tokens.push(Token::DoubleColon);
                    } else {
                        tokens.push(Token::Colon);
                    }
                }
                '-' => {
                    if let Some(&'>') = self.peek_char() {
                        self.next_char();
                        tokens.push(Token::Arrow);
                    } else {
                        tokens.push(Token::Minus);
                    }
                }
                '=' => {
                    if let Some(&'=') = self.peek_char() {
                        self.next_char();
                        tokens.push(Token::EqEq);
                    } else {
                        tokens.push(Token::Eq);
                    }
                }
                '*' => tokens.push(Token::Star),
                '&' => tokens.push(Token::Ampersand),
                ';' => tokens.push(Token::Semicolon),
                ',' => tokens.push(Token::Comma),
                '+' => tokens.push(Token::Plus),
                '#' => tokens.push(Token::Pound),
                '(' => tokens.push(Token::OpenParen),
                ')' => tokens.push(Token::CloseParen),
                '{' => tokens.push(Token::OpenBrace),
                '}' => tokens.push(Token::CloseBrace),
                '[' => tokens.push(Token::OpenBracket),
                ']' => tokens.push(Token::CloseBracket),
                '.' => tokens.push(Token::Dot),
                '!' => tokens.push(Token::Bang),
                other => return Err(format!("Unexpected character: '{}'", other)),
            }
        }

        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_primitives_and_pointers() {
        let source = "int* p; char const* s; void** q;";
        let lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Int,
                Token::Star,
                Token::Ident("p".to_string()),
                Token::Semicolon,
                Token::Char,
                Token::Const,
                Token::Star,
                Token::Ident("s".to_string()),
                Token::Semicolon,
                Token::Void,
                Token::Star,
                Token::Star,
                Token::Ident("q".to_string()),
                Token::Semicolon,
            ]
        );
    }

    #[test]
    fn test_lex_function() {
        let source = "fn add(x: int, y: int*) -> int { return x + *y; }";
        let lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Fn,
                Token::Ident("add".to_string()),
                Token::OpenParen,
                Token::Ident("x".to_string()),
                Token::Colon,
                Token::Int,
                Token::Comma,
                Token::Ident("y".to_string()),
                Token::Colon,
                Token::Int,
                Token::Star,
                Token::CloseParen,
                Token::Arrow,
                Token::Int,
                Token::OpenBrace,
                Token::Return,
                Token::Ident("x".to_string()),
                Token::Plus,
                Token::Star,
                Token::Ident("y".to_string()),
                Token::Semicolon,
                Token::CloseBrace,
            ]
        );
    }

    #[test]
    fn test_comments_and_strings() {
        let source = r#"
            // This is a comment
            let x = "hello world";
            /* block comment */
            let c = 'a';
        "#;
        let lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("x".to_string()),
                Token::Eq,
                Token::StrLit("hello world".to_string()),
                Token::Semicolon,
                Token::Let,
                Token::Ident("c".to_string()),
                Token::Eq,
                Token::CharLit('a'),
                Token::Semicolon,
            ]
        );
    }
}

