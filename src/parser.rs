//! Parser for the Carbide language.
//!
//! Parses a stream of `(Token, byte_pos)` pairs (produced by the lexer) into
//! the Carbide AST.
//!
//! ## Design philosophy
//!
//! Only the **structural skeleton** of a program is parsed into typed AST
//! nodes: top-level items, function signatures, and struct fields.  Everything
//! else — function bodies, enum variants, free-standing items — is stored as
//! a **verbatim slice of the original source string**.  This means arbitrary
//! `no_std` Rust inside a function body compiles without the parser knowing
//! anything about it.

use crate::ast::*;
use crate::lexer::Token;

/// A parser that transforms a `(Token, byte_pos)` stream into a Carbide AST.
///
/// `source` is the original source string.  Byte offsets stored alongside
/// tokens are used to slice out raw body text without loss of whitespace.
pub struct Parser<'s> {
    source: &'s str,
    tokens: Vec<(Token, usize)>,
    pos: usize,
}

impl<'s> Parser<'s> {
    /// Create a new Parser.
    ///
    /// `source` must be the same string that was tokenised to produce `tokens`.
    pub fn new(source: &'s str, tokens: Vec<(Token, usize)>) -> Self {
        Self {
            source,
            tokens,
            pos: 0,
        }
    }

    // -------------------------------------------------------------------------
    // Token navigation helpers
    // -------------------------------------------------------------------------

    /// Peek at the current token (without position).
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    /// Peek at the current token together with its source byte position.
    fn peek_with_pos(&self) -> Option<(&Token, usize)> {
        self.tokens.get(self.pos).map(|(t, p)| (t, *p))
    }

    /// Consume and return the current token.
    fn next_token(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].0.clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    /// Assert the current token matches `expected` and consume it.
    fn expect(&mut self, expected: Token) -> Result<(), String> {
        match self.next_token() {
            Some(tok) if tok == expected => Ok(()),
            Some(tok) => Err(format!("Expected {:?}, found {:?}", expected, tok)),
            None => Err(format!("Expected {:?}, found EOF", expected)),
        }
    }

    // -------------------------------------------------------------------------
    // Body capture
    // -------------------------------------------------------------------------

    /// Consume an optional `unsafe` keyword, then capture the body of a `{…}`
    /// block as a raw source-text slice.
    ///
    /// Returns `(is_unsafe, body_src)` where `body_src` is the text **between**
    /// the outer `{` and `}` (not including the braces themselves).
    fn capture_body(&mut self) -> Result<(bool, String), String> {
        let is_unsafe = if let Some(Token::Unsafe) = self.peek() {
            self.next_token();
            true
        } else {
            false
        };

        // Position of the opening `{` in source
        let open_pos = match self.peek_with_pos() {
            Some((Token::OpenBrace, p)) => p,
            other => return Err(format!("Expected '{{', found {:?}", other.map(|(t, _)| t))),
        };
        self.next_token(); // consume `{`

        let mut depth = 1usize;
        let mut close_pos = open_pos; // will be updated

        while self.pos < self.tokens.len() {
            let (tok, tok_pos) = {
                let (t, p) = &self.tokens[self.pos];
                (t.clone(), *p)
            };
            self.pos += 1;
            match tok {
                Token::OpenBrace => {
                    depth += 1;
                }
                Token::CloseBrace => {
                    depth -= 1;
                    if depth == 0 {
                        close_pos = tok_pos;
                        break;
                    }
                }
                _ => {}
            }
        }

        if depth != 0 {
            return Err("Unterminated brace block".to_string());
        }

        // Slice the source between the braces.  `open_pos` points to `{`
        // (one ASCII byte), `close_pos` points to `}`.
        let body = self.source[open_pos + 1..close_pos].to_string();
        Ok((is_unsafe, body))
    }

    /// Capture raw source text up to (but not including) the **first** token
    /// for which `stop_fn` returns true, respecting balanced braces/parens.
    /// The stop token itself is NOT consumed.
    ///
    /// Used for enum bodies and raw top-level items.
    fn capture_raw_until<F>(&mut self, stop_fn: F) -> String
    where
        F: Fn(&Token) -> bool,
    {
        if self.tokens.is_empty() || self.pos >= self.tokens.len() {
            return String::new();
        }
        // Start of the raw region: just after the previous token end, or the
        // start of the first token to capture.
        let start = self.tokens[self.pos].1;
        let mut depth_brace = 0usize;
        let mut depth_paren = 0usize;

        loop {
            match self.peek() {
                None => break,
                Some(tok) => {
                    if depth_brace == 0 && depth_paren == 0 && stop_fn(tok) {
                        break;
                    }
                    match tok {
                        Token::OpenBrace => {
                            depth_brace += 1;
                        }
                        Token::CloseBrace => {
                            if depth_brace > 0 {
                                depth_brace -= 1;
                            } else {
                                break;
                            }
                        }
                        Token::OpenParen => {
                            depth_paren += 1;
                        }
                        Token::CloseParen => {
                            if depth_paren > 0 {
                                depth_paren -= 1;
                            }
                        }
                        _ => {}
                    }
                    self.pos += 1;
                }
            }
        }

        // `self.pos` now points at the stop token (or EOF).
        // The end of the captured region is the start of that token.
        let end = self
            .tokens
            .get(self.pos)
            .map(|(_, p)| *p)
            .unwrap_or(self.source.len());
        self.source[start..end].trim().to_string()
    }

    // -------------------------------------------------------------------------
    // Top-level parsing
    // -------------------------------------------------------------------------

    /// Parse a complete program.
    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut items = Vec::new();
        while self.peek().is_some() {
            items.push(self.parse_item()?);
        }
        Ok(Program { items })
    }

    /// Parse one top-level item.
    fn parse_item(&mut self) -> Result<Item, String> {
        let mut attrs = Vec::new();

        // Optional leading attributes: `#[…]`
        while let Some(Token::Pound) = self.peek() {
            self.next_token(); // `#`
            self.expect(Token::OpenBracket)?;
            let mut inner = String::new();
            while let Some(tok) = self.peek() {
                if tok == &Token::CloseBracket {
                    break;
                }
                // Reconstruct attribute text from tokens
                inner.push_str(&token_to_str(&self.next_token().unwrap()));
            }
            self.expect(Token::CloseBracket)?;
            attrs.push(Attribute {
                tokens: inner.trim().to_string(),
            });
        }

        match self.peek() {
            Some(Token::Use) => {
                if !attrs.is_empty() {
                    return Err("Attributes on 'use' not supported".to_string());
                }
                self.next_token(); // `use`
                let mut path = String::new();
                while let Some(tok) = self.peek() {
                    if tok == &Token::Semicolon {
                        break;
                    }
                    path.push_str(&token_to_str(&self.next_token().unwrap()));
                }
                self.expect(Token::Semicolon)?;
                Ok(Item::Use(path))
            }

            Some(Token::Struct) => Ok(Item::Struct(self.parse_struct(attrs)?)),

            Some(Token::Impl) => {
                if !attrs.is_empty() {
                    return Err("Attributes on 'impl' not supported".to_string());
                }
                self.next_token(); // `impl`
                let target = match self.next_token() {
                    Some(Token::Ident(n)) => n,
                    other => return Err(format!("Expected impl target name, found {:?}", other)),
                };
                self.expect(Token::OpenBrace)?;
                let mut methods = Vec::new();
                while let Some(tok) = self.peek() {
                    if tok == &Token::CloseBrace {
                        break;
                    }
                    let mut method_attrs = Vec::new();
                    while let Some(Token::Pound) = self.peek() {
                        self.next_token();
                        self.expect(Token::OpenBracket)?;
                        let mut inner = String::new();
                        while let Some(tok) = self.peek() {
                            if tok == &Token::CloseBracket {
                                break;
                            }
                            inner.push_str(&token_to_str(&self.next_token().unwrap()));
                        }
                        self.expect(Token::CloseBracket)?;
                        method_attrs.push(Attribute {
                            tokens: inner.trim().to_string(),
                        });
                    }
                    methods.push(self.parse_fn(method_attrs)?);
                }
                self.expect(Token::CloseBrace)?;
                Ok(Item::Impl { target, methods })
            }

            // `enum` is lexed as `Ident("enum")` because we didn't add a keyword token for it
            Some(Token::Ident(ref n)) if n == "enum" => {
                if !attrs.is_empty() {
                    return Err("Attributes on enum not supported yet".to_string());
                }
                self.next_token(); // `enum`
                let name = match self.next_token() {
                    Some(Token::Ident(n)) => n,
                    other => return Err(format!("Expected enum name, found {:?}", other)),
                };
                // Capture body between { and } as raw source
                let open_pos = match self.peek_with_pos() {
                    Some((Token::OpenBrace, p)) => p,
                    other => return Err(format!("Expected '{{' for enum body, found {:?}", other)),
                };
                self.next_token(); // `{`
                let mut depth = 1usize;
                let mut close_pos = open_pos;
                while self.pos < self.tokens.len() {
                    let (tok, tok_pos) = {
                        let (t, p) = &self.tokens[self.pos];
                        (t.clone(), *p)
                    };
                    self.pos += 1;
                    match tok {
                        Token::OpenBrace => depth += 1,
                        Token::CloseBrace => {
                            depth -= 1;
                            if depth == 0 {
                                close_pos = tok_pos;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let body = self.source[open_pos + 1..close_pos].to_string();
                Ok(Item::Enum { name, body })
            }

            // `type Name = Type;` alias declaration (`type` is lexed as
            // `Ident("type")`, mirroring how `enum` is handled above).
            Some(Token::Ident(ref n)) if n == "type" => {
                if !attrs.is_empty() {
                    return Err("Attributes on 'type' aliases not supported".to_string());
                }
                self.next_token(); // `type`
                let name = match self.next_token() {
                    Some(Token::Ident(n)) => n,
                    other => return Err(format!("Expected type alias name, found {:?}", other)),
                };
                self.expect(Token::Eq)?;
                let ty = self.parse_type()?;
                self.expect(Token::Semicolon)?;
                Ok(Item::TypeAlias { name, ty })
            }

            Some(Token::Fn) | Some(Token::Proc) | Some(Token::Unsafe) | Some(Token::Extern) => {
                Ok(Item::Fn(self.parse_fn(attrs)?))
            }

            Some(_) => {
                // Unknown item — capture raw source text up to `;`
                let mut src = self.capture_raw_until(|t| *t == Token::Semicolon);
                // Re-attach the terminating `;`: capture_raw_until() stops
                // *before* the stop token, so without this every `const` /
                // `static` / `type`-style raw item would lose its semicolon.
                if let Some(Token::Semicolon) = self.peek() {
                    self.next_token();
                    src.push(';');
                }
                Ok(Item::Raw { attrs, src })
            }

            None => Err("Unexpected EOF while parsing item".to_string()),
        }
    }

    // -------------------------------------------------------------------------
    // Struct parsing
    // -------------------------------------------------------------------------

    /// Parse `struct Name { field: Type, … }`
    fn parse_struct(&mut self, attrs: Vec<Attribute>) -> Result<Struct, String> {
        self.expect(Token::Struct)?;
        let name = match self.next_token() {
            Some(Token::Ident(n)) => n,
            other => return Err(format!("Expected struct name, found {:?}", other)),
        };
        self.expect(Token::OpenBrace)?;
        let mut fields = Vec::new();
        while let Some(tok) = self.peek() {
            if tok == &Token::CloseBrace {
                break;
            }
            let field_name = match self.next_token() {
                Some(Token::Ident(n)) => n,
                other => return Err(format!("Expected field name, found {:?}", other)),
            };
            self.expect(Token::Colon)?;
            let ty = self.parse_type()?;
            fields.push(Field {
                name: field_name,
                ty,
            });
            if let Some(Token::Comma) = self.peek() {
                self.next_token();
            }
        }
        self.expect(Token::CloseBrace)?;
        Ok(Struct {
            name,
            fields,
            attrs,
        })
    }

    // -------------------------------------------------------------------------
    // Function / procedure parsing
    // -------------------------------------------------------------------------

    /// Parse `[unsafe] [extern "ABI"] fn|proc name(params) [-> Type] { body }`
    fn parse_fn(&mut self, attrs: Vec<Attribute>) -> Result<Function, String> {
        let mut is_unsafe = false;
        let mut abi: Option<String> = None;

        // Optional `unsafe`
        if let Some(Token::Unsafe) = self.peek() {
            self.next_token();
            is_unsafe = true;
        }
        // Optional `extern "ABI"`
        if let Some(Token::Extern) = self.peek() {
            self.next_token();
            if let Some(Token::StrLit(a)) = self.peek().cloned() {
                self.next_token();
                abi = Some(a);
            }
        }
        // `fn` or `proc`
        let is_proc = match self.next_token() {
            Some(Token::Fn) => false,
            Some(Token::Proc) => true,
            other => return Err(format!("Expected 'fn' or 'proc', found {:?}", other)),
        };
        if is_proc {
            is_unsafe = true;
        }

        let name = match self.next_token() {
            Some(Token::Ident(n)) => n,
            other => return Err(format!("Expected function name, found {:?}", other)),
        };

        // Parameter list
        self.expect(Token::OpenParen)?;
        let mut params = Vec::new();
        while let Some(tok) = self.peek() {
            if tok == &Token::CloseParen {
                break;
            }
            let param_name = match self.next_token() {
                Some(Token::Ident(n)) => n,
                other => return Err(format!("Expected parameter name, found {:?}", other)),
            };
            self.expect(Token::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param {
                name: param_name,
                ty,
            });
            if let Some(Token::Comma) = self.peek() {
                self.next_token();
            }
        }
        self.expect(Token::CloseParen)?;

        // Optional return type
        let ret_type = if let Some(Token::Arrow) = self.peek() {
            self.next_token();
            Some(self.parse_type()?)
        } else {
            None
        };

        // Body: captured as verbatim source text.
        // `capture_body` handles optional leading `unsafe` (already consumed
        // above, but a user could still write `unsafe { … }` as a bare block —
        // we ignore that and just capture verbatim).
        let (_, body_src) = self.capture_body()?;

        Ok(Function {
            name,
            params,
            ret_type,
            body_src,
            is_unsafe,
            abi,
            attrs,
        })
    }

    // -------------------------------------------------------------------------
    // Type parsing
    // -------------------------------------------------------------------------

    /// Parse a Carbide type expression.
    ///
    /// Supports:
    /// - `[T; N]`          — array
    /// - `&[mut] T`        — prefix reference
    /// - `T&`              — mutable reference (postfix)
    /// - `T const&`        — const reference (postfix)
    /// - `T*`              — mutable pointer (postfix)
    /// - `T const*`        — const pointer (postfix)
    /// - `int`, `void`, …  — C primitive keywords
    /// - `i32`, `u8`, …    — Rust primitives
    /// - Any `Ident`       — user-defined type
    pub fn parse_type(&mut self) -> Result<Type, String> {
        let mut ty = self.parse_base_type()?;

        // Postfix pointer (`*`, `const*`, `mut*`) and reference (`&`, `const&`, `mut&`) modifiers
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.next_token();
                    ty = Type::Pointer {
                        base: Box::new(ty),
                        is_const: false,
                    };
                }
                Some(Token::Ampersand) => {
                    self.next_token();
                    ty = Type::Reference {
                        base: Box::new(ty),
                        is_mut: true,
                    };
                }
                Some(Token::Mut) => {
                    self.next_token();
                    match self.peek() {
                        Some(Token::Star) => {
                            self.next_token();
                            ty = Type::Pointer {
                                base: Box::new(ty),
                                is_const: false,
                            };
                        }
                        Some(Token::Ampersand) => {
                            self.next_token();
                            ty = Type::Reference {
                                base: Box::new(ty),
                                is_mut: true,
                            };
                        }
                        other => {
                            return Err(format!(
                                "Expected '*' or '&' after 'mut', found {:?}",
                                other
                            ));
                        }
                    }
                }
                Some(Token::Const) => {
                    self.next_token();
                    match self.peek() {
                        Some(Token::Star) => {
                            self.next_token();
                            ty = Type::Pointer {
                                base: Box::new(ty),
                                is_const: true,
                            };
                        }
                        Some(Token::Ampersand) => {
                            self.next_token();
                            ty = Type::Reference {
                                base: Box::new(ty),
                                is_mut: false,
                            };
                        }
                        other => {
                            return Err(format!(
                                "Expected '*' or '&' after 'const', found {:?}",
                                other
                            ));
                        }
                    }
                }
                _ => break,
            }
        }
        Ok(ty)
    }

    /// Parse a base type (before postfix pointer modifiers).
    fn parse_base_type(&mut self) -> Result<Type, String> {
        match self.peek() {
            // `[T; N]` array
            Some(Token::OpenBracket) => {
                self.next_token();
                let elem_ty = self.parse_type()?;
                self.expect(Token::Semicolon)?;
                let len = match self.next_token() {
                    Some(Token::IntLit(n)) => n,
                    other => return Err(format!("Expected array length, found {:?}", other)),
                };
                self.expect(Token::CloseBracket)?;
                return Ok(Type::Array {
                    base: Box::new(elem_ty),
                    len,
                });
            }
            // Disallow Rust prefix reference syntax (`&T`, `&mut T`)
            Some(Token::Ampersand) => {
                return Err("Prefix reference syntax ('&T' / '&mut T') is not allowed in Carbide; use C++-style postfix reference syntax ('T&' / 'T const&' / 'T mut&') instead".to_string());
            }
            // Disallow C-style prefix const type syntax (`const T*`, `const T&`)
            Some(Token::Const) => {
                return Err("Prefix 'const' type syntax ('const T*') is not allowed in Carbide; use C++-style postfix syntax ('T const*') instead".to_string());
            }
            // C primitive keywords
            Some(Token::Void) => {
                self.next_token();
                return Ok(Type::UserDefined("void".to_string()));
            }
            Some(Token::Int) => {
                self.next_token();
                return Ok(Type::UserDefined("int".to_string()));
            }
            Some(Token::Uint) => {
                self.next_token();
                return Ok(Type::UserDefined("uint".to_string()));
            }
            Some(Token::Long) => {
                self.next_token();
                // long long, long double, long int
                match self.peek() {
                    Some(Token::Long) => {
                        self.next_token();
                        return Ok(Type::UserDefined("long long".to_string()));
                    }
                    Some(Token::Ident(ref n)) if n == "double" => {
                        self.next_token();
                        return Ok(Type::UserDefined("long double".to_string()));
                    }
                    Some(Token::Int) => {
                        self.next_token();
                        return Ok(Type::UserDefined("long int".to_string()));
                    }
                    _ => {}
                }
                return Ok(Type::UserDefined("long".to_string()));
            }
            Some(Token::Char) => {
                self.next_token();
                return Ok(Type::UserDefined("char".to_string()));
            }
            // `fn(param: Type, …) -> Ret` function pointer type (C callback).
            // Emitted as `Option<unsafe extern "system" fn(…) -> Ret>`; the parser
            // consumes the full parenthesised parameter list and the arrow
            // return type before returning.
            Some(Token::Fn) => {
                self.next_token(); // `fn`
                self.expect(Token::OpenParen)?;
                let mut params = Vec::new();
                while let Some(tok) = self.peek() {
                    if tok == &Token::CloseParen {
                        break;
                    }
                    let param_name = match self.next_token() {
                        Some(Token::Ident(n)) => n,
                        other => return Err(format!("Expected parameter name, found {:?}", other)),
                    };
                    self.expect(Token::Colon)?;
                    let ty = self.parse_type()?;
                    params.push(Param {
                        name: param_name,
                        ty,
                    });
                    if let Some(Token::Comma) = self.peek() {
                        self.next_token();
                    }
                }
                self.expect(Token::CloseParen)?;
                let ret = if let Some(Token::Arrow) = self.peek() {
                    self.next_token();
                    Some(Box::new(self.parse_type()?))
                } else {
                    None
                };
                return Ok(Type::FnPointer { params, ret });
            }
            // Disallow Rust prefix pointer syntax (`*mut T`, `*const T`)
            Some(Token::Star) => {
                return Err("Prefix pointer syntax ('*mut T' / '*const T') is not allowed in Carbide; use C++-style postfix pointer syntax ('T*' / 'T const*' / 'T mut*') instead".to_string());
            }
            Some(Token::Ident(_)) => {}
            Some(other) => return Err(format!("Expected type, found {:?}", other)),
            None => return Err("Expected type, found EOF".to_string()),
        }

        // Identifier — check for multi-word C types and Rust primitives
        let n = match self.next_token() {
            Some(Token::Ident(n)) => n,
            _ => unreachable!(),
        };

        match n.as_str() {
            "unsigned" => {
                // unsigned [char|short|int|long|long long|…]
                match self.peek().cloned() {
                    Some(Token::Char) => {
                        self.next_token();
                        return Ok(Type::UserDefined("unsigned char".to_string()));
                    }
                    Some(Token::Long) => {
                        self.next_token();
                        // unsigned long long
                        if let Some(Token::Long) = self.peek() {
                            self.next_token();
                            return Ok(Type::UserDefined("unsigned long long".to_string()));
                        }
                        // unsigned long int
                        if let Some(Token::Int) = self.peek() {
                            self.next_token();
                        }
                        return Ok(Type::UserDefined("unsigned long".to_string()));
                    }
                    Some(Token::Int) => {
                        self.next_token();
                        return Ok(Type::UserDefined("unsigned int".to_string()));
                    }
                    Some(Token::Ident(ref k)) if k == "short" => {
                        self.next_token();
                        if let Some(Token::Int) = self.peek() {
                            self.next_token();
                        }
                        return Ok(Type::UserDefined("unsigned short".to_string()));
                    }
                    _ => return Ok(Type::UserDefined("unsigned int".to_string())),
                }
            }
            "signed" => {
                match self.peek().cloned() {
                    Some(Token::Char) => {
                        self.next_token();
                        return Ok(Type::UserDefined("signed char".to_string()));
                    }
                    Some(Token::Long) => {
                        self.next_token();
                        // signed long long
                        if let Some(Token::Long) = self.peek() {
                            self.next_token();
                            return Ok(Type::UserDefined("signed long long".to_string()));
                        }
                        if let Some(Token::Int) = self.peek() {
                            self.next_token();
                        }
                        return Ok(Type::UserDefined("signed long".to_string()));
                    }
                    Some(Token::Ident(ref k)) if k == "short" => {
                        self.next_token();
                        if let Some(Token::Int) = self.peek() {
                            self.next_token();
                        }
                        return Ok(Type::UserDefined("signed short".to_string()));
                    }
                    _ => return Ok(Type::UserDefined("signed int".to_string())),
                }
            }
            "short" => {
                if let Some(Token::Int) = self.peek() {
                    self.next_token();
                }
                return Ok(Type::UserDefined("short".to_string()));
            }
            "double" => return Ok(Type::UserDefined("double".to_string())),
            "float" => return Ok(Type::UserDefined("float".to_string())),
            // Standard Rust primitives pass through as-is
            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
            | "u128" | "usize" | "f32" | "f64" | "bool" | "str" => {
                return Ok(Type::Primitive(PrimitiveType::RustPrimitive(n)));
            }
            _ => return Ok(Type::UserDefined(n)),
        }
    }
}

// -------------------------------------------------------------------------
// Helper: reconstruct minimal source text from a single token
// (used only for attribute and use-path reconstruction where we need a
//  string but lost the original whitespace — acceptable because these are
//  structural, not user-formatted code)
// -------------------------------------------------------------------------

fn token_to_str(tok: &Token) -> String {
    match tok {
        Token::Fn => "fn".to_string(),
        Token::Proc => "proc".to_string(),
        Token::Struct => "struct".to_string(),
        Token::Let => "let".to_string(),
        Token::Mut => "mut ".to_string(),
        Token::Const => "const".to_string(),
        Token::Return => "return".to_string(),
        Token::Extern => "extern".to_string(),
        Token::Unsafe => "unsafe".to_string(),
        Token::Use => "use".to_string(),
        Token::Impl => "impl".to_string(),
        Token::As => "as".to_string(),
        Token::If => "if".to_string(),
        Token::Else => "else".to_string(),
        Token::Void => "void".to_string(),
        Token::Int => "int".to_string(),
        Token::Uint => "uint".to_string(),
        Token::Long => "long".to_string(),
        Token::Char => "char".to_string(),
        Token::Ident(s) => s.clone(),
        Token::IntLit(s) => s.clone(),
        Token::StrLit(s) => format!("\"{}\"", s),
        Token::CharLit(c) => format!("'{}'", c),
        Token::Star => "*".to_string(),
        Token::Ampersand => "&".to_string(),
        Token::Arrow => "->".to_string(),
        Token::Colon => ":".to_string(),
        Token::DoubleColon => "::".to_string(),
        Token::Semicolon => ";".to_string(),
        Token::Comma => ", ".to_string(),
        Token::Eq => "=".to_string(),
        Token::EqEq => "==".to_string(),
        Token::Plus => "+".to_string(),
        Token::Minus => "-".to_string(),
        Token::Slash => "/".to_string(),
        Token::Pound => "#".to_string(),
        Token::OpenParen => "(".to_string(),
        Token::CloseParen => ")".to_string(),
        Token::OpenBrace => "{".to_string(),
        Token::CloseBrace => "}".to_string(),
        Token::OpenBracket => "[".to_string(),
        Token::CloseBracket => "]".to_string(),
        Token::Dot => ".".to_string(),
        Token::Bang => "!".to_string(),
        Token::Lt => "<".to_string(),
        Token::Gt => ">".to_string(),
        Token::Pipe => "|".to_string(),
        Token::Percent => "%".to_string(),
        Token::Caret => "^".to_string(),
        Token::Question => "?".to_string(),
        Token::Tilde => "~".to_string(),
        Token::At => "@".to_string(),
        Token::Dollar => "$".to_string(),
    }
}

// -------------------------------------------------------------------------
// Unit tests
// -------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenize_with_positions().unwrap();
        Parser::new(src, tokens).parse_program().unwrap()
    }

    #[test]
    fn test_parse_function() {
        let src = "fn add(x: int, y: int) -> int { return x + y; }";
        let prog = parse(src);
        assert_eq!(prog.items.len(), 1);
        if let Item::Fn(f) = &prog.items[0] {
            assert_eq!(f.name, "add");
            assert_eq!(f.params.len(), 2);
            assert!(!f.is_unsafe);
            assert!(f.body_src.contains("return x + y;"));
        } else {
            panic!("Expected Fn item");
        }
    }

    #[test]
    fn test_parse_struct() {
        let src = "struct Point { x: int, y: int }";
        let prog = parse(src);
        assert_eq!(prog.items.len(), 1);
        if let Item::Struct(s) = &prog.items[0] {
            assert_eq!(s.name, "Point");
            assert_eq!(s.fields.len(), 2);
        } else {
            panic!("Expected Struct item");
        }
    }

    #[test]
    fn test_parse_type_pointer() {
        let src = "fn f(p: int*) -> void {}";
        let prog = parse(src);
        if let Item::Fn(f) = &prog.items[0] {
            assert!(matches!(
                &f.params[0].ty,
                Type::Pointer {
                    is_const: false,
                    ..
                }
            ));
        } else {
            panic!("Expected Fn");
        }
    }

    #[test]
    fn test_parse_rust_primitives() {
        let src = "fn f(x: i32, y: u64) -> bool {}";
        let prog = parse(src);
        if let Item::Fn(f) = &prog.items[0] {
            assert!(
                matches!(&f.params[0].ty, Type::Primitive(PrimitiveType::RustPrimitive(s)) if s == "i32")
            );
            assert!(
                matches!(&f.params[1].ty, Type::Primitive(PrimitiveType::RustPrimitive(s)) if s == "u64")
            );
        } else {
            panic!("Expected Fn");
        }
    }

    #[test]
    fn test_parse_multiword_c_types() {
        let src = "fn f(a: unsigned long, b: signed char) -> void {}";
        let prog = parse(src);
        if let Item::Fn(f) = &prog.items[0] {
            assert!(matches!(&f.params[0].ty, Type::UserDefined(s) if s == "unsigned long"));
            assert!(matches!(&f.params[1].ty, Type::UserDefined(s) if s == "signed char"));
        } else {
            panic!("Expected Fn");
        }
    }

    #[test]
    fn test_parse_raw_statement() {
        // Body contains arbitrary Rust — parser stores it verbatim.
        let src = "fn f() -> int { let x = if a > b { a } else { b }; x }";
        let prog = parse(src);
        if let Item::Fn(f) = &prog.items[0] {
            assert!(f.body_src.contains("let x = if a > b"));
            assert!(f.body_src.contains("else"));
        } else {
            panic!("Expected Fn");
        }
    }

    #[test]
    fn test_parse_fn_pointer_type() {
        // C callback fields: fn(name: Type, …) -> Ret inside a struct.
        let src = r#"
            struct Plugin {
                init: fn(plugin: Plugin const*) -> bool,
                destroy: fn(plugin: Plugin const*) -> void,
                process: fn(plugin: Plugin const*, frames: uint) -> int*
            }
        "#;
        let prog = parse(src);
        if let Item::Struct(s) = &prog.items[0] {
            assert_eq!(s.fields.len(), 3);
            let init = &s.fields[0].ty;
            match init {
                Type::FnPointer { params, ret } => {
                    assert_eq!(params.len(), 1);
                    assert_eq!(params[0].name, "plugin");
                    assert!(matches!(params[0].ty, Type::Pointer { is_const: true, .. }));
                    let ret = ret.as_ref().expect("fn pointer should have a return type");
                    // `bool` is a Rust primitive, preserved verbatim
                    assert!(
                        matches!(ret.as_ref(), Type::Primitive(PrimitiveType::RustPrimitive(n)) if n == "bool")
                    );
                }
                other => panic!("Expected FnPointer type, got {:?}", other),
            }
            // Multi-param + pointer return: process field
            let process = &s.fields[2].ty;
            match process {
                Type::FnPointer { params, ret } => {
                    assert_eq!(params.len(), 2);
                    let ret = ret.as_ref().expect("return type");
                    assert!(matches!(
                        ret.as_ref(),
                        Type::Pointer {
                            is_const: false,
                            ..
                        }
                    ));
                }
                other => panic!("Expected FnPointer type, got {:?}", other),
            }
        } else {
            panic!("Expected Struct");
        }
    }

    #[test]
    fn test_parse_fn_pointer_param_and_return() {
        // fn pointer as a function parameter and return type
        let src = "proc register(cb: fn(a: int) -> int) -> fn(b: int) -> int { return cb; }";
        let prog = parse(src);
        if let Item::Fn(f) = &prog.items[0] {
            assert!(matches!(f.params[0].ty, Type::FnPointer { .. }));
            let ret = f.ret_type.as_ref().expect("return type");
            assert!(matches!(ret, Type::FnPointer { .. }));
        } else {
            panic!("Expected Fn");
        }
    }

    #[test]
    fn test_parse_type_alias() {
        let src = "type AudioCallback = fn(buffer: void*, frames: uint) -> void;";
        let prog = parse(src);
        if let Item::TypeAlias { name, ty } = &prog.items[0] {
            assert_eq!(name, "AudioCallback");
            assert!(matches!(ty, Type::FnPointer { .. }));
        } else {
            panic!("Expected TypeAlias item, got {:?}", prog.items[0]);
        }
    }

    #[test]
    fn test_parse_postfix_references() {
        let src = r#"
            struct RefStruct {
                a: int&,
                b: int const&,
                c: int mut&,
                d: int mut*,
                e: int const*
            }
        "#;
        let prog = parse(src);
        if let Item::Struct(s) = &prog.items[0] {
            assert_eq!(s.fields.len(), 5);
            assert!(matches!(s.fields[0].ty, Type::Reference { is_mut: true, .. }));
            assert!(matches!(s.fields[1].ty, Type::Reference { is_mut: false, .. }));
            assert!(matches!(s.fields[2].ty, Type::Reference { is_mut: true, .. }));
            assert!(matches!(s.fields[3].ty, Type::Pointer { is_const: false, .. }));
            assert!(matches!(s.fields[4].ty, Type::Pointer { is_const: true, .. }));
        } else {
            panic!("Expected Struct");
        }
    }

    #[test]
    fn test_reject_prefix_pointers_and_references() {
        let src_ref = "struct S { a: &int }";
        let tokens = Lexer::new(src_ref).tokenize_with_positions().unwrap();
        assert!(Parser::new(src_ref, tokens).parse_program().is_err());

        let src_mut_ref = "struct S { a: &mut int }";
        let tokens = Lexer::new(src_mut_ref).tokenize_with_positions().unwrap();
        assert!(Parser::new(src_mut_ref, tokens).parse_program().is_err());

        let src_ptr = "struct S { a: *const int }";
        let tokens = Lexer::new(src_ptr).tokenize_with_positions().unwrap();
        assert!(Parser::new(src_ptr, tokens).parse_program().is_err());

        let src_mut_ptr = "struct S { a: *mut int }";
        let tokens = Lexer::new(src_mut_ptr).tokenize_with_positions().unwrap();
        assert!(Parser::new(src_mut_ptr, tokens).parse_program().is_err());

        let src_prefix_const_ptr = "struct S { a: const int* }";
        let tokens = Lexer::new(src_prefix_const_ptr).tokenize_with_positions().unwrap();
        assert!(Parser::new(src_prefix_const_ptr, tokens).parse_program().is_err());

        let src_prefix_const_ref = "struct S { a: const int& }";
        let tokens = Lexer::new(src_prefix_const_ref).tokenize_with_positions().unwrap();
        assert!(Parser::new(src_prefix_const_ref, tokens).parse_program().is_err());
    }
}
