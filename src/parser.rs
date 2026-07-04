//! Parser for the Crust language.
//!
//! Parses a stream of tokens into the Abstract Syntax Tree (AST).

use crate::ast::*;
use crate::lexer::Token;

/// A parser that transforms a token stream into a Crust AST.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    /// Creates a new Parser for the given token stream.
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Helper to peek at the current token.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// Helper to consume and return the current token.
    fn next_token(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    /// Helper to assert and consume a specific token.
    fn expect(&mut self, expected: Token) -> Result<(), String> {
        match self.next_token() {
            Some(tok) if tok == expected => Ok(()),
            Some(tok) => Err(format!("Expected {:?}, found {:?}", expected, tok)),
            None => Err(format!("Expected {:?}, found EOF", expected)),
        }
    }

    /// Parse a complete program.
    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut items = Vec::new();
        while self.peek().is_some() {
            items.push(self.parse_item()?);
        }
        Ok(Program { items })
    }

    /// Parse a top-level item (fn, struct, use, or attribute-decorated item).
    fn parse_item(&mut self) -> Result<Item, String> {
        let mut attrs = Vec::new();

        // Parse optional attributes e.g. #[attribute]
        while let Some(Token::Pound) = self.peek() {
            self.next_token(); // consume '#'
            self.expect(Token::OpenBracket)?;
            let mut attr_toks = String::new();
            while let Some(tok) = self.peek() {
                if tok == &Token::CloseBracket {
                    break;
                }
                let t = self.next_token().unwrap();
                attr_toks.push_str(&format!("{:?} ", t));
            }
            self.expect(Token::CloseBracket)?;
            attrs.push(Attribute { tokens: attr_toks.trim().to_string() });
        }

        match self.peek() {
            Some(Token::Use) => {
                if !attrs.is_empty() {
                    return Err("Attributes are not allowed on 'use' imports".to_string());
                }
                self.next_token(); // consume 'use'
                let mut path = String::new();
                while let Some(tok) = self.peek() {
                    if tok == &Token::Semicolon {
                        break;
                    }
                    let t = self.next_token().unwrap();
                    match t {
                        Token::Ident(name) => path.push_str(&name),
                        Token::DoubleColon => path.push_str("::"),
                        Token::Star => path.push('*'),
                        other => return Err(format!("Unexpected token in use path: {:?}", other)),
                    }
                }
                self.expect(Token::Semicolon)?;
                Ok(Item::Use(path))
            }
            Some(Token::Struct) => {
                let s = self.parse_struct(attrs)?;
                Ok(Item::Struct(s))
            }
            Some(Token::Fn) | Some(Token::Proc) | Some(Token::Unsafe) | Some(Token::Extern) => {
                let f = self.parse_fn(attrs)?;
                Ok(Item::Fn(f))
            }
            Some(other) => Err(format!("Unexpected top-level token: {:?}", other)),
            None => Err("Unexpected EOF while parsing item".to_string()),
        }
    }

    /// Parse a struct definition: `struct Name { field1: Type, ... }`
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
            fields.push(Field { name: field_name, ty });

            if let Some(Token::Comma) = self.peek() {
                self.next_token();
            }
        }
        self.expect(Token::CloseBrace)?;

        Ok(Struct { name, fields, attrs })
    }

    /// Parse a function: `fn name(params) [-> ret_ty] { body }`
    fn parse_fn(&mut self, attrs: Vec<Attribute>) -> Result<Function, String> {
        let mut is_unsafe = false;
        let mut abi = None;

        if let Some(Token::Unsafe) = self.peek() {
            self.next_token();
            is_unsafe = true;
        }

        if let Some(Token::Extern) = self.peek() {
            self.next_token();
            if let Some(Token::StrLit(s)) = self.peek() {
                abi = Some(s.clone());
                self.next_token();
            } else {
                abi = Some("C".to_string());
            }
        }

        let is_proc = match self.peek() {
            Some(Token::Fn) => {
                self.next_token();
                false
            }
            Some(Token::Proc) => {
                self.next_token();
                true
            }
            other => return Err(format!("Expected fn or proc, found {:?}", other)),
        };

        if is_proc {
            is_unsafe = true;
        }

        let name = match self.next_token() {
            Some(Token::Ident(n)) => n,
            other => return Err(format!("Expected function name, found {:?}", other)),
        };

        self.expect(Token::OpenParen)?;
        let mut params = Vec::new();
        while let Some(tok) = self.peek() {
            if tok == &Token::CloseParen {
                break;
            }

            let is_mut = if let Some(Token::Mut) = self.peek() {
                self.next_token();
                true
            } else {
                false
            };

            let param_name = match self.next_token() {
                Some(Token::Ident(n)) => {
                    if is_mut {
                        format!("mut {}", n)
                    } else {
                        n
                    }
                }
                other => return Err(format!("Expected parameter name, found {:?}", other)),
            };

            self.expect(Token::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param { name: param_name, ty });

            if let Some(Token::Comma) = self.peek() {
                self.next_token();
            }
        }
        self.expect(Token::CloseParen)?;

        let mut ret_type = None;
        if let Some(Token::Arrow) = self.peek() {
            self.next_token();
            ret_type = Some(self.parse_type()?);
        }

        let body = self.parse_block()?;

        Ok(Function {
            name,
            params,
            ret_type,
            body,
            is_unsafe,
            abi,
            attrs,
        })
    }

    /// Parse a type signature, including postfix `*` and `const*` pointer symbols.
    pub fn parse_type(&mut self) -> Result<Type, String> {
        let mut ty = self.parse_base_type()?;

        // Parse trailing postfix pointer symbols loop
        loop {
            match self.peek() {
                Some(Token::Const) => {
                    self.next_token(); // consume 'const'
                    self.expect(Token::Star)?; // must be followed by '*'
                    ty = Type::Pointer {
                        base: Box::new(ty),
                        is_const: true,
                    };
                }
                Some(Token::Star) => {
                    self.next_token(); // consume '*'
                    ty = Type::Pointer {
                        base: Box::new(ty),
                        is_const: false,
                    };
                }
                _ => break,
            }
        }

        Ok(ty)
    }

    /// Parse base type without postfix modifiers.
    fn parse_base_type(&mut self) -> Result<Type, String> {
        match self.peek() {
            Some(Token::Void) => {
                self.next_token();
                Ok(Type::UserDefined("void".to_string()))
            }
            Some(Token::Int) => {
                self.next_token();
                Ok(Type::UserDefined("int".to_string()))
            }
            Some(Token::Uint) => {
                self.next_token();
                Ok(Type::UserDefined("uint".to_string()))
            }
            Some(Token::Char) => {
                self.next_token();
                Ok(Type::UserDefined("char".to_string()))
            }
            Some(Token::Long) => {
                self.next_token(); // consume 'long'
                // Check if followed by another 'long' (long long)
                if let Some(Token::Long) = self.peek() {
                    self.next_token(); // consume second 'long'
                    if let Some(Token::Int) = self.peek() {
                        self.next_token(); // consume 'int'
                        return Ok(Type::UserDefined("long long int".to_string()));
                    }
                    return Ok(Type::UserDefined("long long".to_string()));
                }
                // Check if followed by 'double' (long double)
                if let Some(Token::Ident(name)) = self.peek() {
                    if name == "double" {
                        self.next_token(); // consume 'double'
                        return Ok(Type::UserDefined("long double".to_string()));
                    }
                }
                // Check if followed by 'int' (long int)
                if let Some(Token::Int) = self.peek() {
                    self.next_token();
                    return Ok(Type::UserDefined("long int".to_string()));
                }
                Ok(Type::UserDefined("long".to_string()))
            }
            Some(Token::Ampersand) => {
                self.next_token(); // consume '&'
                let is_mut = if let Some(Token::Mut) = self.peek() {
                    self.next_token();
                    true
                } else {
                    false
                };
                let base = self.parse_type()?;
                Ok(Type::Reference {
                    base: Box::new(base),
                    is_mut,
                })
            }
            Some(Token::Ident(name)) => {
                let n = name.clone();
                if n == "unsigned" || n == "signed" {
                    self.next_token(); // consume 'unsigned' / 'signed'
                    if let Some(next_tok) = self.peek() {
                        match next_tok {
                            Token::Char => {
                                self.next_token();
                                return Ok(Type::UserDefined(format!("{} char", n)));
                            }
                            Token::Int => {
                                self.next_token();
                                return Ok(Type::UserDefined(format!("{} int", n)));
                            }
                            Token::Long => {
                                self.next_token();
                                // Check if followed by second 'long' (Token::Long)
                                if let Some(Token::Long) = self.peek() {
                                    self.next_token();
                                    if let Some(Token::Int) = self.peek() {
                                        self.next_token();
                                        return Ok(Type::UserDefined(format!("{} long long int", n)));
                                    }
                                    return Ok(Type::UserDefined(format!("{} long long", n)));
                                }
                                // Check if followed by 'int'
                                if let Some(Token::Int) = self.peek() {
                                    self.next_token();
                                    return Ok(Type::UserDefined(format!("{} long int", n)));
                                }
                                return Ok(Type::UserDefined(format!("{} long", n)));
                            }
                            Token::Ident(sub_name) => {
                                let s = sub_name.clone();
                                if s == "short" {
                                    self.next_token();
                                    if let Some(Token::Int) = self.peek() {
                                        self.next_token();
                                        return Ok(Type::UserDefined(format!("{} short int", n)));
                                    }
                                    return Ok(Type::UserDefined(format!("{} short", n)));
                                }
                            }
                            _ => {}
                        }
                    }
                    // Defaults to unsigned int / signed int
                    return Ok(Type::UserDefined(format!("{} int", n)));
                } else if n == "short" {
                    self.next_token();
                    if let Some(Token::Int) = self.peek() {
                        self.next_token();
                        return Ok(Type::UserDefined("short int".to_string()));
                    }
                    return Ok(Type::UserDefined("short".to_string()));
                } else if n == "double" {
                    self.next_token();
                    return Ok(Type::UserDefined("double".to_string()));
                } else if n == "float" {
                    self.next_token();
                    return Ok(Type::UserDefined("float".to_string()));
                } else {
                    self.next_token(); // consume standard identifier
                    match n.as_str() {
                        // Standard Rust primitives
                        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" |
                        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" |
                        "f32" | "f64" | "bool" | "str" => {
                            Ok(Type::Primitive(PrimitiveType::RustPrimitive(n)))
                        }
                        _ => Ok(Type::UserDefined(n)),
                    }
                }
            }
            Some(other) => Err(format!("Expected type, found {:?}", other)),
            None => Err("Expected type, found EOF".to_string()),
        }
    }

    /// Parse a block of code enclosed in `{}`.
    fn parse_block(&mut self) -> Result<Block, String> {
        let mut is_unsafe = false;
        if let Some(Token::Unsafe) = self.peek() {
            self.next_token();
            is_unsafe = true;
        }

        self.expect(Token::OpenBrace)?;
        let mut stmts = Vec::new();
        while let Some(tok) = self.peek() {
            if tok == &Token::CloseBrace {
                break;
            }
            stmts.push(self.parse_stmt()?);
        }
        self.expect(Token::CloseBrace)?;

        Ok(Block { stmts, is_unsafe })
    }

    /// Parse a single statement.
    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        if let Some(Token::Let) = self.peek() {
            self.next_token(); // consume 'let'
            let is_mut = if let Some(Token::Mut) = self.peek() {
                self.next_token();
                true
            } else {
                false
            };

            let name = match self.next_token() {
                Some(Token::Ident(n)) => n,
                other => return Err(format!("Expected variable name, found {:?}", other)),
            };

            let mut ty = None;
            if let Some(Token::Colon) = self.peek() {
                self.next_token();
                ty = Some(self.parse_type()?);
            }

            let mut init = None;
            if let Some(Token::Eq) = self.peek() {
                self.next_token();
                init = Some(self.parse_expr()?);
            }

            self.expect(Token::Semicolon)?;
            Ok(Stmt::Local {
                name,
                ty,
                init,
                is_mut,
            })
        } else {
            let expr = self.parse_expr()?;
            if let Some(Token::Semicolon) = self.peek() {
                self.next_token();
                Ok(Stmt::Semi(expr))
            } else {
                Ok(Stmt::Expr(expr))
            }
        }
    }

    /// Parse an expression.
    pub fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_expr_with_precedence(0)
    }

    /// Operator precedence map.
    fn get_op_precedence(op: &str) -> i32 {
        match op {
            "=" => 1,
            "==" => 2,
            "+" | "-" => 3,
            "*" | "/" => 4,
            "." => 9,
            _ => 0,
        }
    }

    /// Parse expression using Pratt precedence-climbing.
    fn parse_expr_with_precedence(&mut self, min_precedence: i32) -> Result<Expr, String> {
        let mut left = self.parse_primary_expr()?;

        while let Some(tok) = self.peek() {
            let op = match tok {
                Token::Eq => "=".to_string(),
                Token::EqEq => "==".to_string(),
                Token::Plus => "+".to_string(),
                Token::Minus => "-".to_string(),
                Token::Star => "*".to_string(),
                Token::Slash => "/".to_string(),
                Token::Dot => ".".to_string(),
                _ => break,
            };

            let prec = Self::get_op_precedence(&op);
            if prec < min_precedence {
                break;
            }

            self.next_token(); // consume operator

            // Right-associative assignment vs left-associative operators
            let next_min_precedence = if op == "=" { prec } else { prec + 1 };
            let right = self.parse_expr_with_precedence(next_min_precedence)?;

            if op == "=" {
                left = Expr::Assign {
                    target: Box::new(left),
                    value: Box::new(right),
                };
            } else {
                left = Expr::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                };
            }
        }

        Ok(left)
    }

    /// Parse basic primary expression.
    fn parse_primary_expr(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(Token::IntLit(val)) => {
                let v = val.clone();
                self.next_token();
                Ok(Expr::IntLit(v))
            }
            Some(Token::StrLit(val)) => {
                let v = val.clone();
                self.next_token();
                Ok(Expr::StrLit(v))
            }
            Some(Token::CharLit(val)) => {
                let v = *val;
                self.next_token();
                Ok(Expr::CharLit(v))
            }
            Some(Token::Ident(name)) => {
                let n = name.clone();
                self.next_token();
                if let Some(Token::OpenParen) = self.peek() {
                    self.next_token(); // consume '('
                    let mut args = Vec::new();
                    while let Some(t) = self.peek() {
                        if t == &Token::CloseParen {
                            break;
                        }
                        args.push(self.parse_expr()?);
                        if let Some(Token::Comma) = self.peek() {
                            self.next_token();
                        }
                    }
                    self.expect(Token::CloseParen)?;
                    Ok(Expr::Call { name: n, args })
                } else {
                    Ok(Expr::Ident(n))
                }
            }
            Some(Token::Star) => {
                self.next_token(); // consume '*' for dereference
                let expr = self.parse_expr_with_precedence(8)?;
                Ok(Expr::Deref(Box::new(expr)))
            }
            Some(Token::Bang) => {
                self.next_token(); // consume '!'
                let expr = self.parse_expr_with_precedence(8)?;
                Ok(Expr::Unary {
                    op: "!".to_string(),
                    expr: Box::new(expr),
                })
            }
            Some(Token::Ampersand) => {
                self.next_token(); // consume '&'
                let is_mut = if let Some(Token::Mut) = self.peek() {
                    self.next_token();
                    true
                } else {
                    false
                };
                let expr = self.parse_expr_with_precedence(8)?;
                Ok(Expr::AddrOf {
                    expr: Box::new(expr),
                    is_mut,
                })
            }
            Some(Token::OpenParen) => {
                self.next_token(); // consume '('
                let expr = self.parse_expr()?;
                self.expect(Token::CloseParen)?;
                Ok(expr)
            }
            Some(Token::OpenBrace) | Some(Token::Unsafe) => {
                let block = self.parse_block()?;
                Ok(Expr::Block(block))
            }
            Some(Token::Return) => {
                self.next_token(); // consume 'return'
                let val = if let Some(tok) = self.peek() {
                    if tok == &Token::Semicolon || tok == &Token::CloseBrace || tok == &Token::Comma {
                        None
                    } else {
                        Some(Box::new(self.parse_expr()?))
                    }
                } else {
                    None
                };
                Ok(Expr::Return(val))
            }
            Some(other) => Err(format!("Unexpected token in expression: {:?}", other)),
            None => Err("Expected expression, found EOF".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_str_program(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_program().unwrap()
    }

    #[test]
    fn test_parse_type_pointer() {
        let tokens = Lexer::new("int const*").tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let ty = parser.parse_type().unwrap();
        assert_eq!(
            ty,
            Type::Pointer {
                base: Box::new(Type::UserDefined("int".to_string())),
                is_const: true,
            }
        );

        let tokens = Lexer::new("void**").tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let ty = parser.parse_type().unwrap();
        assert_eq!(
            ty,
            Type::Pointer {
                base: Box::new(Type::Pointer {
                    base: Box::new(Type::UserDefined("void".to_string())),
                    is_const: false,
                }),
                is_const: false,
            }
        );
    }

    #[test]
    fn test_parse_struct() {
        let src = "struct Point { x: int, y: int* }";
        let program = parse_str_program(src);
        assert_eq!(program.items.len(), 1);
        if let Item::Struct(s) = &program.items[0] {
            assert_eq!(s.name, "Point");
            assert_eq!(s.fields.len(), 2);
            assert_eq!(s.fields[0].name, "x");
            assert_eq!(s.fields[0].ty, Type::UserDefined("int".to_string()));
            assert_eq!(s.fields[1].name, "y");
            assert_eq!(
                s.fields[1].ty,
                Type::Pointer {
                    base: Box::new(Type::UserDefined("int".to_string())),
                    is_const: false,
                }
            );
        } else {
            panic!("Expected Struct");
        }
    }

    #[test]
    fn test_parse_expr_precedence() {
        let tokens = Lexer::new("a = b + c * d").tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let expr = parser.parse_expr().unwrap();
        assert_eq!(
            expr,
            Expr::Assign {
                target: Box::new(Expr::Ident("a".to_string())),
                value: Box::new(Expr::Binary {
                    left: Box::new(Expr::Ident("b".to_string())),
                    op: "+".to_string(),
                    right: Box::new(Expr::Binary {
                        left: Box::new(Expr::Ident("c".to_string())),
                        op: "*".to_string(),
                        right: Box::new(Expr::Ident("d".to_string())),
                    })
                })
            }
        );
    }

    #[test]
    fn test_parse_function() {
        let src = r#"
            fn test(p: int*) -> void {
                let x: int = *p;
                *p = x + 1;
            }
        "#;
        let program = parse_str_program(src);
        assert_eq!(program.items.len(), 1);
        if let Item::Fn(f) = &program.items[0] {
            assert_eq!(f.name, "test");
            assert_eq!(f.params.len(), 1);
            assert_eq!(f.ret_type, Some(Type::UserDefined("void".to_string())));
            assert_eq!(f.body.stmts.len(), 2);
        } else {
            panic!("Expected Function");
        }
    }

    #[test]
    fn test_parse_multiword_c_types() {
        // Helper to parse a single type from source
        fn parse_type_str(src: &str) -> Type {
            let tokens = Lexer::new(src).tokenize().unwrap();
            let mut parser = Parser::new(tokens);
            parser.parse_type().unwrap()
        }

        // Single-word C types
        assert_eq!(parse_type_str("int"), Type::UserDefined("int".to_string()));
        assert_eq!(parse_type_str("char"), Type::UserDefined("char".to_string()));
        assert_eq!(parse_type_str("void"), Type::UserDefined("void".to_string()));
        assert_eq!(parse_type_str("long"), Type::UserDefined("long".to_string()));

        // Ident-based C types
        assert_eq!(parse_type_str("float"), Type::UserDefined("float".to_string()));
        assert_eq!(parse_type_str("double"), Type::UserDefined("double".to_string()));
        assert_eq!(parse_type_str("short"), Type::UserDefined("short".to_string()));

        // Multi-word signed/unsigned types
        assert_eq!(parse_type_str("unsigned int"), Type::UserDefined("unsigned int".to_string()));
        assert_eq!(parse_type_str("signed int"), Type::UserDefined("signed int".to_string()));
        assert_eq!(parse_type_str("unsigned char"), Type::UserDefined("unsigned char".to_string()));
        assert_eq!(parse_type_str("signed char"), Type::UserDefined("signed char".to_string()));
        assert_eq!(parse_type_str("unsigned long"), Type::UserDefined("unsigned long".to_string()));
        assert_eq!(parse_type_str("unsigned short"), Type::UserDefined("unsigned short".to_string()));

        // Multi-word long long types
        assert_eq!(parse_type_str("long long"), Type::UserDefined("long long".to_string()));
        assert_eq!(parse_type_str("unsigned long long"), Type::UserDefined("unsigned long long".to_string()));

        // long double
        assert_eq!(parse_type_str("long double"), Type::UserDefined("long double".to_string()));

        // bare unsigned/signed defaults to unsigned int / signed int
        assert_eq!(parse_type_str("unsigned"), Type::UserDefined("unsigned int".to_string()));
        assert_eq!(parse_type_str("signed"), Type::UserDefined("signed int".to_string()));
    }

    #[test]
    fn test_parse_rust_primitives() {
        fn parse_type_str(src: &str) -> Type {
            let tokens = Lexer::new(src).tokenize().unwrap();
            let mut parser = Parser::new(tokens);
            parser.parse_type().unwrap()
        }

        // All Rust integer types
        for ty_name in &["i8", "i16", "i32", "i64", "i128", "isize",
                         "u8", "u16", "u32", "u64", "u128", "usize"] {
            assert_eq!(
                parse_type_str(ty_name),
                Type::Primitive(PrimitiveType::RustPrimitive(ty_name.to_string())),
                "Failed for Rust type: {}",
                ty_name
            );
        }

        // Rust float types
        assert_eq!(parse_type_str("f32"), Type::Primitive(PrimitiveType::RustPrimitive("f32".to_string())));
        assert_eq!(parse_type_str("f64"), Type::Primitive(PrimitiveType::RustPrimitive("f64".to_string())));

        // Rust bool
        assert_eq!(parse_type_str("bool"), Type::Primitive(PrimitiveType::RustPrimitive("bool".to_string())));

        // Rust str
        assert_eq!(parse_type_str("str"), Type::Primitive(PrimitiveType::RustPrimitive("str".to_string())));
    }
}

