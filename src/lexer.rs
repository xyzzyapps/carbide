//! Lexer for the Carbide language.
//!
//! Converts a Carbide source string into a flat stream of tokens.  The lexer
//! only needs to recognise tokens that appear in *signatures* (function
//! parameters, return types, struct fields) and at the structural top level
//! (`fn`, `proc`, `struct`, `enum`, `impl`, `use`, `#[…]`).  Everything
//! inside a function body is captured as verbatim source text by the parser,
//! so the lexer does NOT need to understand operators, control-flow keywords,
//! or expression syntax.
//!
//! The companion [`Lexer::tokenize_with_positions`] method returns each token
//! together with its **start byte offset** in the source string.  The parser
//! uses these offsets to slice out raw body text without re-scanning.

/// All tokens produced by the Carbide lexer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    // Keywords
    Fn,
    Proc,
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
    If,
    Else,

    // C primitive type keywords
    Void,
    Int,
    Uint,
    Long,
    Char,

    // Identifiers & literals
    Ident(String),
    IntLit(String),
    StrLit(String),
    CharLit(char),

    // Symbols & operators
    Star,           // `*`
    Ampersand,      // `&`
    Arrow,          // `->`
    Colon,          // `:`
    DoubleColon,    // `::`
    Semicolon,      // `;`
    Comma,          // `,`
    Eq,             // `=`
    EqEq,           // `==`
    Plus,           // `+`
    Minus,          // `-`
    Slash,          // `/`
    Pound,          // `#`
    OpenParen,      // `(`
    CloseParen,     // `)`
    OpenBrace,      // `{`
    CloseBrace,     // `}`
    OpenBracket,    // `[`
    CloseBracket,   // `]`
    Dot,            // `.`
    Bang,           // `!`
    Lt,             // `<`
    Gt,             // `>`
}

/// Lexer for a Carbide source string.
pub struct Lexer<'a> {
    /// Character iterator that also yields byte positions.
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
}

impl<'a> Lexer<'a> {
    /// Create a new Lexer for `input`.
    pub fn new(input: &'a str) -> Self {
        Self { chars: input.char_indices().peekable() }
    }

    /// Advance past the current character, returning it.
    fn next_char(&mut self) -> Option<(usize, char)> {
        self.chars.next()
    }

    /// Peek at the current character without consuming it.
    fn peek_char(&mut self) -> Option<(usize, char)> {
        self.chars.peek().copied()
    }

    /// Tokenize the source and return tokens together with their **start byte
    /// offsets** in the original source string.
    ///
    /// The parser uses the offsets to slice raw function-body text verbatim.
    pub fn tokenize_with_positions(mut self) -> Result<Vec<(Token, usize)>, String> {
        let mut tokens: Vec<(Token, usize)> = Vec::new();

        while let Some((pos, c)) = self.peek_char() {
            // Whitespace
            if c.is_whitespace() {
                self.next_char();
                continue;
            }

            // Line and block comments
            if c == '/' {
                self.next_char();
                if let Some((_, '/')) = self.peek_char() {
                    self.next_char();
                    while let Some((_, nc)) = self.next_char() {
                        if nc == '\n' { break; }
                    }
                    continue;
                } else if let Some((_, '*')) = self.peek_char() {
                    self.next_char();
                    let mut closed = false;
                    while let Some((_, nc)) = self.next_char() {
                        if nc == '*' {
                            if let Some((_, '/')) = self.peek_char() {
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
                    tokens.push((Token::Slash, pos));
                    continue;
                }
            }

            // Identifiers and keywords
            if c.is_alphabetic() || c == '_' {
                let start = pos;
                let mut ident = String::new();
                while let Some((_, nc)) = self.peek_char() {
                    if nc.is_alphanumeric() || nc == '_' {
                        ident.push(self.next_char().unwrap().1);
                    } else {
                        break;
                    }
                }
                let tok = match ident.as_str() {
                    "fn"     => Token::Fn,
                    "proc"   => Token::Proc,
                    "struct" => Token::Struct,
                    "let"    => Token::Let,
                    "mut"    => Token::Mut,
                    "const"  => Token::Const,
                    "return" => Token::Return,
                    "extern" => Token::Extern,
                    "unsafe" => Token::Unsafe,
                    "use"    => Token::Use,
                    "impl"   => Token::Impl,
                    "as"     => Token::As,
                    "if"     => Token::If,
                    "else"   => Token::Else,
                    "void"   => Token::Void,
                    "int"    => Token::Int,
                    "uint"   => Token::Uint,
                    "long"   => Token::Long,
                    "char"   => Token::Char,
                    _        => Token::Ident(ident),
                };
                tokens.push((tok, start));
                continue;
            }

            // Integer literals
            if c.is_ascii_digit() {
                let start = pos;
                let mut num = String::new();
                while let Some((_, nc)) = self.peek_char() {
                    if nc.is_ascii_digit() {
                        num.push(self.next_char().unwrap().1);
                    } else {
                        break;
                    }
                }
                tokens.push((Token::IntLit(num), start));
                continue;
            }

            // String literals
            if c == '"' {
                let start = pos;
                self.next_char();
                let mut s = String::new();
                let mut escaped = false;
                let mut closed = false;
                while let Some((_, nc)) = self.next_char() {
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
                tokens.push((Token::StrLit(s), start));
                continue;
            }

            // Char literals
            if c == '\'' {
                let start = pos;
                self.next_char();
                let val = match self.next_char() {
                    Some((_, '\\')) => match self.next_char() {
                        Some((_, 'n'))  => '\n',
                        Some((_, 'r'))  => '\r',
                        Some((_, 't'))  => '\t',
                        Some((_, '0'))  => '\0',
                        Some((_, esc))  => esc,
                        None => return Err("Unterminated char literal escape".to_string()),
                    },
                    Some((_, other)) => other,
                    None => return Err("Empty or unterminated char literal".to_string()),
                };
                if self.next_char().map(|(_, c)| c) != Some('\'') {
                    return Err("Expected closing single quote for char literal".to_string());
                }
                tokens.push((Token::CharLit(val), start));
                continue;
            }

            // Single-character and two-character operators / punctuation
            let (cur_pos, current) = self.next_char().unwrap();
            match current {
                ':' => {
                    if let Some((_, ':')) = self.peek_char() {
                        self.next_char();
                        tokens.push((Token::DoubleColon, cur_pos));
                    } else {
                        tokens.push((Token::Colon, cur_pos));
                    }
                }
                '-' => {
                    if let Some((_, '>')) = self.peek_char() {
                        self.next_char();
                        tokens.push((Token::Arrow, cur_pos));
                    } else {
                        tokens.push((Token::Minus, cur_pos));
                    }
                }
                '=' => {
                    if let Some((_, '=')) = self.peek_char() {
                        self.next_char();
                        tokens.push((Token::EqEq, cur_pos));
                    } else {
                        tokens.push((Token::Eq, cur_pos));
                    }
                }
                '*'  => tokens.push((Token::Star,         cur_pos)),
                '&'  => tokens.push((Token::Ampersand,    cur_pos)),
                ';'  => tokens.push((Token::Semicolon,    cur_pos)),
                ','  => tokens.push((Token::Comma,        cur_pos)),
                '+'  => tokens.push((Token::Plus,         cur_pos)),
                '#'  => tokens.push((Token::Pound,        cur_pos)),
                '('  => tokens.push((Token::OpenParen,    cur_pos)),
                ')'  => tokens.push((Token::CloseParen,   cur_pos)),
                '{'  => tokens.push((Token::OpenBrace,    cur_pos)),
                '}'  => tokens.push((Token::CloseBrace,   cur_pos)),
                '['  => tokens.push((Token::OpenBracket,  cur_pos)),
                ']'  => tokens.push((Token::CloseBracket, cur_pos)),
                '.'  => tokens.push((Token::Dot,          cur_pos)),
                '!'  => tokens.push((Token::Bang,         cur_pos)),
                '<'  => tokens.push((Token::Lt,           cur_pos)),
                '>'  => tokens.push((Token::Gt,           cur_pos)),
                other => return Err(format!("Unexpected character: '{}'", other)),
            }
        }

        Ok(tokens)
    }

    /// Tokenize the source, returning only the tokens (positions discarded).
    ///
    /// Used by unit tests that don't need source-slicing.
    pub fn tokenize(self) -> Result<Vec<Token>, String> {
        Ok(self.tokenize_with_positions()?.into_iter().map(|(t, _)| t).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_primitives_and_pointers() {
        let source = "int* p; char const* s; void** q;";
        let tokens = Lexer::new(source).tokenize().unwrap();
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
        let tokens = Lexer::new(source).tokenize().unwrap();
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
        let tokens = Lexer::new(source).tokenize().unwrap();
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

    #[test]
    fn test_lex_proc() {
        let source = "proc run() {}";
        let tokens = Lexer::new(source).tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Proc,
                Token::Ident("run".to_string()),
                Token::OpenParen,
                Token::CloseParen,
                Token::OpenBrace,
                Token::CloseBrace,
            ]
        );
    }
}
