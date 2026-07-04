//! Pretty-printer/emitter that converts the Crust AST back into standard Rust code.

use crate::ast::*;

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
                if let Some(ref expr) = init {
                    self.output.push_str(" = ");
                    self.emit_expr(expr);
                }
                self.output.push_str(";\n");
            }
            Stmt::Expr(expr) => {
                self.emit_expr(expr);
                self.newline();
            }
            Stmt::Semi(expr) => {
                self.emit_expr(expr);
                self.output.push_str(";\n");
            }
        }
    }

    /// Get the operator precedence of an expression.
    fn get_precedence(expr: &Expr) -> i32 {
        match expr {
            Expr::Ident(_) | Expr::IntLit(_) | Expr::StrLit(_) | Expr::CharLit(_) | Expr::Call { .. } | Expr::Block(_) | Expr::If { .. } => 10,
            Expr::Binary { op, .. } if op == "." => 9,
            Expr::Deref(_) | Expr::AddrOf { .. } | Expr::Unary { .. } => 8,
            Expr::Binary { op, .. } if op == "*" || op == "/" => 7,
            Expr::Binary { op, .. } if op == "+" || op == "-" => 6,
            Expr::Binary { op, .. } if op == "==" || op == "<" || op == ">" => 5,
            Expr::Binary { .. } => 5, // fallback for any other binary operator
            Expr::Assign { .. } => 4,
            Expr::Return(_) => 3,
        }
    }

    /// Emit expression, wrapping in parentheses if its precedence is less than parent precedence.
    fn emit_expr_with_precedence(&mut self, expr: &Expr, parent_prec: i32) {
        let prec = Self::get_precedence(expr);
        if prec < parent_prec {
            self.output.push('(');
            self.emit_expr_node(expr);
            self.output.push(')');
        } else {
            self.emit_expr_node(expr);
        }
    }

    /// Emit expression.
    fn emit_expr(&mut self, expr: &Expr) {
        self.emit_expr_with_precedence(expr, 0);
    }

    /// Emit the core of an expression node.
    fn emit_expr_node(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(name) => self.output.push_str(name),
            Expr::IntLit(val) => self.output.push_str(val),
            Expr::StrLit(val) => self.output.push_str(&format!("\"{}\"", val)),
            Expr::CharLit(val) => self.output.push_str(&format!("'{}'", val)),
            Expr::Binary { left, op, right } => {
                let prec = Self::get_precedence(expr);
                if op == "." {
                    self.emit_expr_with_precedence(left, prec);
                    self.output.push('.');
                    self.emit_expr_with_precedence(right, prec);
                } else {
                    self.emit_expr_with_precedence(left, prec);
                    self.output.push_str(&format!(" {} ", op));
                    self.emit_expr_with_precedence(right, prec + 1);
                }
            }
            Expr::Unary { op, expr } => {
                let prec = Self::get_precedence(expr);
                self.output.push_str(op);
                self.emit_expr_with_precedence(expr, prec);
            }
            Expr::Call { name, args } => {
                self.output.push_str(name);
                self.output.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.emit_expr(arg);
                }
                self.output.push(')');
            }
            Expr::Assign { target, value } => {
                let prec = Self::get_precedence(expr);
                self.emit_expr_with_precedence(target, prec);
                self.output.push_str(" = ");
                self.emit_expr_with_precedence(value, prec);
            }
            Expr::Deref(expr) => {
                let prec = Self::get_precedence(expr);
                self.output.push_str("*");
                self.emit_expr_with_precedence(expr, prec);
            }
            Expr::AddrOf { expr, is_mut } => {
                let prec = Self::get_precedence(expr);
                if *is_mut {
                    self.output.push_str("&mut ");
                } else {
                    self.output.push_str("&");
                }
                self.emit_expr_with_precedence(expr, prec);
            }
            Expr::Block(block) => {
                self.emit_block(block);
            }
            Expr::If { cond, then_branch, else_branch } => {
                self.output.push_str("if ");
                self.emit_expr(cond);
                self.output.push(' ');
                self.emit_block(then_branch);
                if let Some(ref eb) = else_branch {
                    self.output.push_str(" else ");
                    self.emit_block(eb);
                }
            }
            Expr::Return(val) => {
                self.output.push_str("return");
                if let Some(ref v) = val {
                    self.output.push(' ');
                    self.emit_expr(v);
                }
            }
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

