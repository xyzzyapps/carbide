//! Pretty-printer/emitter that converts the Crust AST back into standard Rust code.

use crate::ast::*;
use crate::lexer::Token;

/// Formatter for emitting AST structures as formatted Rust source code.
pub struct Emitter {
    output: String,
    indent_level: usize,
}

impl Emitter {
    /// Creates a new Emitter.
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
        }
    }

    /// Retrieve the final emitted code.
    pub fn finish(self) -> String {
        self.output
    }

    /// Insert indentation spaces based on the current level.
    fn indent(&mut self) {
        self.output.push_str(&"    ".repeat(self.indent_level));
    }

    /// Emit a newline.
    fn newline(&mut self) {
        self.output.push('\n');
    }

    /// Emit attributes.
    fn emit_attributes(&mut self, attrs: &[Attribute]) {
        for attr in attrs {
            self.indent();
            self.output.push_str(&format!("#[{}]\n", attr.tokens));
        }
    }

    /// Emit type.
    pub fn emit_type(&mut self, ty: &Type) {
        match ty {
            Type::Primitive(prim) => match prim {
                PrimitiveType::RustPrimitive(s) => self.output.push_str(s),
            },
            Type::UserDefined(name) => self.output.push_str(name),
            Type::Pointer { base, is_const } => {
                if *is_const {
                    self.output.push_str("*const ");
                } else {
                    self.output.push_str("*mut ");
                }
                self.emit_type(base);
            }
            Type::Reference { base, is_mut } => {
                if *is_mut {
                    self.output.push_str("&mut ");
                } else {
                    self.output.push('&');
                }
                self.emit_type(base);
            }
            Type::Array { base, len } => {
                self.output.push('[');
                self.emit_type(base);
                self.output.push_str(&format!("; {}]", len));
            }
        }
    }

    /// Emit entire program.
    pub fn emit_program(&mut self, program: &Program) {
        // Emit the items first to a temporary buffer to scan for libc types
        let mut items_emitter = Emitter::new();
        for item in &program.items {
            items_emitter.emit_item(item);
            items_emitter.newline();
        }
        let items_code = items_emitter.finish();

        // Scan items code for any libc types
        let libc_types = ["size_t", "ssize_t", "ptrdiff_t", "uintptr_t", "intptr_t", "off_t", "pid_t"];
        let needs_libc = libc_types.iter().any(|&t| items_code.contains(t));

        self.output.push_str("#![no_std]\n\n");
        self.output.push_str("use core::ffi::*;\n");
        if needs_libc {
            self.output.push_str("use libc::*;\n");
        }
        self.output.push('\n');
        self.output.push_str(&items_code);
    }

    /// Emit item.
    fn emit_item(&mut self, item: &Item) {
        match item {
            Item::Use(path) => {
                self.indent();
                self.output.push_str(&format!("use {};\n", path));
            }
            Item::Struct(strct) => {
                self.emit_attributes(&strct.attrs);
                self.indent();
                self.output.push_str(&format!("pub struct {} {{\n", strct.name));
                self.indent_level += 1;
                for field in &strct.fields {
                    self.indent();
                    self.output.push_str(&format!("pub {}: ", field.name));
                    self.emit_type(&field.ty);
                    self.output.push_str(",\n");
                }
                self.indent_level -= 1;
                self.indent();
                self.output.push_str("}\n");
            }
            Item::Fn(func) => {
                self.emit_attributes(&func.attrs);
                self.indent();
                self.output.push_str("pub ");
                if func.is_unsafe {
                    self.output.push_str("unsafe ");
                }
                if let Some(ref abi_name) = func.abi {
                    self.output.push_str(&format!("extern \"{}\" ", abi_name));
                }
                self.output.push_str(&format!("fn {}(", func.name));
                for (i, param) in func.params.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&format!("{}: ", param.name));
                    self.emit_type(&param.ty);
                }
                self.output.push(')');
                if let Some(ref ret) = func.ret_type {
                    self.output.push_str(" -> ");
                    self.emit_type(ret);
                }
                self.output.push(' ');
                self.emit_block(&func.body);
                self.newline();
            }
            Item::Enum { name, tokens } => {
                self.indent();
                self.output.push_str(&format!("enum {} {{\n", name));
                self.indent_level += 1;
                self.indent();
                self.emit_tokens(tokens);
                self.newline();
                self.indent_level -= 1;
                self.indent();
                self.output.push_str("}\n");
            }
            Item::Impl { target, methods } => {
                self.indent();
                self.output.push_str(&format!("impl {} {{\n", target));
                self.indent_level += 1;
                for method in methods {
                    self.emit_item(&Item::Fn(method.clone()));
                }
                self.indent_level -= 1;
                self.indent();
                self.output.push_str("}\n");
            }
            Item::Raw { attrs, tokens } => {
                self.emit_attributes(attrs);
                self.indent();
                self.emit_tokens(tokens);
                self.newline();
            }
        }
    }

    /// Emit statement block.
    fn emit_block(&mut self, block: &Block) {
        if block.is_unsafe {
            self.output.push_str("unsafe {\n");
        } else {
            self.output.push_str("{\n");
        }
        self.indent_level += 1;
        for stmt in &block.stmts {
            self.emit_stmt(stmt);
        }
        self.indent_level -= 1;
        self.indent();
        self.output.push('}');
    }

    /// Emit statement.
    fn emit_stmt(&mut self, stmt: &Stmt) {
        self.indent();
        match stmt {
            Stmt::Local { name, ty, init, is_mut } => {
                self.output.push_str("let ");
                if *is_mut {
                    self.output.push_str("mut ");
                }
                self.output.push_str(name);
                if let Some(ref t) = ty {
                    self.output.push_str(": ");
                    self.emit_type(t);
                }
                if let Some(ref tokens) = init {
                    self.output.push_str(" = ");
                    self.emit_tokens(tokens);
                }
                self.output.push_str(";\n");
            }
            Stmt::If { cond, then_branch, else_branch } => {
                self.output.push_str("if ");
                self.emit_tokens(cond);
                self.output.push(' ');
                self.emit_block(then_branch);
                if let Some(ref eb) = else_branch {
                    self.output.push_str(" else ");
                    self.emit_block(eb);
                }
                self.newline();
            }
            Stmt::Block(block) => {
                self.emit_block(block);
                self.newline();
            }
            Stmt::Return(val) => {
                self.output.push_str("return");
                if let Some(ref tokens) = val {
                    self.output.push(' ');
                    self.emit_tokens(tokens);
                }
                self.output.push_str(";\n");
            }
            Stmt::Raw { tokens, has_semi } => {
                self.emit_tokens(tokens);
                if *has_semi {
                    self.output.push(';');
                }
                self.newline();
            }
        }
    }

    /// Emit a slice of raw tokens with correct spacing.
    ///
    /// Rules:
    /// - Two adjacent identifier-like tokens get a space between them.
    /// - `as` always gets a trailing space (e.g. `as *mut T`).
    /// - `,` always gets a trailing space.
    /// - `}` gets a trailing newline+indent so the next statement starts fresh.
    fn emit_tokens(&mut self, tokens: &[Token]) {
        let mut prev_needs_space = false;
        let mut prev_was_as = false;
        let len = tokens.len();
        for (i, tok) in tokens.iter().enumerate() {
            let has_next = i + 1 < len;
            let s = match tok {
                Token::Fn => "fn".to_string(),
                Token::Proc => "proc".to_string(),
                Token::Struct => "struct".to_string(),
                Token::Let => "let".to_string(),
                Token::Mut => "mut".to_string(),
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
                Token::Ident(name) => name.clone(),
                Token::IntLit(val) => val.clone(),
                Token::StrLit(val) => format!("\"{}\"", val),
                Token::CharLit(val) => format!("'{}'", val),
                Token::Star => "*".to_string(),
                Token::Ampersand => "&".to_string(),
                Token::Arrow => "->".to_string(),
                Token::Colon => ":".to_string(),
                Token::DoubleColon => "::".to_string(),
                Token::Semicolon => ";".to_string(),
                Token::Comma => ",".to_string(),
                Token::Eq => "=".to_string(),
                Token::EqEq => "==".to_string(),
                Token::Lt => "<".to_string(),
                Token::Gt => ">".to_string(),
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
            };

            let cur_needs_space = matches!(
                tok,
                Token::Fn
                    | Token::Proc
                    | Token::Struct
                    | Token::Let
                    | Token::Mut
                    | Token::Const
                    | Token::Return
                    | Token::Extern
                    | Token::Unsafe
                    | Token::Use
                    | Token::Impl
                    | Token::As
                    | Token::If
                    | Token::Else
                    | Token::Void
                    | Token::Int
                    | Token::Uint
                    | Token::Long
                    | Token::Char
                    | Token::Ident(_)
                    | Token::IntLit(_)
            );

            // Space between two identifier-like tokens, or after `as` keyword.
            if (prev_needs_space && cur_needs_space) || prev_was_as {
                self.output.push(' ');
            }

            self.output.push_str(&s);

            // Trailing space after comma.
            if tok == &Token::Comma {
                self.output.push(' ');
            }

            // Newline + re-indent after `}` only when more tokens follow,
            // so that `return Struct { .. };` keeps `;` on the same line.
            if tok == &Token::CloseBrace && has_next {
                self.output.push('\n');
                self.indent();
            }

            prev_was_as = tok == &Token::As;
            prev_needs_space = cur_needs_space;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::transform::transform_program;

    fn transpile(src: &str) -> String {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let mut program = parser.parse_program().unwrap();
        transform_program(&mut program);
        let mut emitter = Emitter::new();
        emitter.emit_program(&mut program);
        emitter.finish()
    }

    #[test]
    fn test_emitter_output() {
        let src = r#"
            struct Point {
                x: int,
                y: int*
            }

            proc add(p: Point const*) -> int {
                return *p.x;
            }
        "#;
        let output = transpile(src);
        
        // Verify output contains FFI representations
        assert!(output.contains("#![no_std]"));
        assert!(output.contains("use core::ffi::*;"));
        assert!(!output.contains("use libc::*;"));
        assert!(output.contains("#[repr(C)]"));
        assert!(output.contains("pub struct Point"));
        assert!(output.contains("pub x: c_int"));
        assert!(output.contains("pub y: *mut c_int"));
        
        assert!(output.contains("#[no_mangle]"));
        assert!(output.contains("pub unsafe extern \"C\" fn add(p: *const Point) -> c_int"));
        assert!(output.contains("unsafe {"));
    }
}

