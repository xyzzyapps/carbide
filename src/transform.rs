//! AST Transformation pipeline for the Crust compiler.
//!
//! Applies passes to the AST:
//! 1. Type Substitution: Replace C primitives with core::ffi equivalents.
//! 2. Pointer Flipping: Transformed structurally via the AST and pretty-printed as prefix raw pointers.
//! 3. C-ABI Function Pass: Inject #[no_mangle] and extern "C".
//! 4. Implicit Unsafe Pass: Wrap bodies in unsafe blocks.
//! 5. Auto-Repr Struct Pass: Prepend #[repr(C)] to structs.

use crate::ast::*;

/// Transforms a Type by replacing C-style primitives with core::ffi equivalents.
pub fn transform_type(ty: &mut Type) {
    match ty {
        Type::Primitive(_) => {
            // RustPrimitive types are passed through unchanged.
        }
        Type::UserDefined(name) => {
            let mapped = match name.as_str() {
                // Single word C types
                "void" => Some("c_void"),
                "char" => Some("c_char"),
                "int" => Some("c_int"),
                "uint" => Some("c_uint"),
                "long" => Some("c_long"),
                "float" => Some("c_float"),
                "double" => Some("c_double"),
                "short" => Some("c_short"),

                // Multi-word C types
                "unsigned char" => Some("c_uchar"),
                "signed char" => Some("c_schar"),
                "unsigned short" => Some("c_ushort"),
                "signed short" => Some("c_short"),
                "short int" => Some("c_short"),
                "unsigned short int" => Some("c_ushort"),
                "signed short int" => Some("c_short"),
                "unsigned int" => Some("c_uint"),
                "signed int" => Some("c_int"),
                "unsigned long" => Some("c_ulong"),
                "signed long" => Some("c_long"),
                "long int" => Some("c_long"),
                "unsigned long int" => Some("c_ulong"),
                "signed long int" => Some("c_long"),
                "long long" => Some("c_longlong"),
                "unsigned long long" => Some("c_ulonglong"),
                "signed long long" => Some("c_longlong"),
                "long long int" => Some("c_longlong"),
                "unsigned long long int" => Some("c_ulonglong"),
                "signed long long int" => Some("c_longlong"),
                "long double" => Some("c_double"),

                // libc types
                "size_t" => Some("size_t"),
                "ssize_t" => Some("ssize_t"),
                "ptrdiff_t" => Some("ptrdiff_t"),
                "uintptr_t" => Some("uintptr_t"),
                "intptr_t" => Some("intptr_t"),
                "off_t" => Some("off_t"),
                "pid_t" => Some("pid_t"),

                _ => None,
            };
            if let Some(m) = mapped {
                *name = m.to_string();
            }
        }
        Type::Pointer { base, .. } => {
            transform_type(base);
        }
        Type::Reference { base, .. } => {
            transform_type(base);
        }
    }
}

/// Recursively walks and transforms statements in a block.
pub fn transform_block(block: &mut Block) {
    for stmt in &mut block.stmts {
        transform_stmt(stmt);
    }
}

/// Transforms a single statement.
pub fn transform_stmt(stmt: &mut Stmt) {
    match stmt {
        Stmt::Local { ty, init, .. } => {
            if let Some(t) = ty {
                transform_type(t);
            }
            if let Some(expr) = init {
                transform_expr(expr);
            }
        }
        Stmt::Expr(expr) | Stmt::Semi(expr) => {
            transform_expr(expr);
        }
    }
}

/// Transforms type annotations and sub-blocks within an expression.
pub fn transform_expr(expr: &mut Expr) {
    match expr {
        Expr::Binary { left, right, .. } => {
            transform_expr(left);
            transform_expr(right);
        }
        Expr::Unary { expr, .. } => {
            transform_expr(expr);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                transform_expr(arg);
            }
        }
        Expr::Assign { target, value } => {
            transform_expr(target);
            transform_expr(value);
        }
        Expr::Deref(e) => {
            transform_expr(e);
        }
        Expr::AddrOf { expr: e, .. } => {
            transform_expr(e);
        }
        Expr::Block(block) => {
            transform_block(block);
        }
        Expr::If { cond, then_branch, else_branch } => {
            transform_expr(cond);
            transform_block(then_branch);
            if let Some(eb) = else_branch {
                transform_block(eb);
            }
        }
        Expr::Return(val) => {
            if let Some(v) = val {
                transform_expr(v);
            }
        }
        Expr::Ident(_) | Expr::IntLit(_) | Expr::StrLit(_) | Expr::CharLit(_) => {}
    }
}

/// Applies all transformation passes to a Function.
pub fn transform_fn(func: &mut Function) {
    // 1. C-ABI Pass: Inject #[no_mangle]
    if !func.attrs.iter().any(|a| a.tokens == "no_mangle") {
        func.attrs.insert(0, Attribute { tokens: "no_mangle".to_string() });
    }
    // Set ABI calling convention to "C"
    func.abi = Some("C".to_string());

    // 2. Type substitution on parameters and return type
    for param in &mut func.params {
        transform_type(&mut param.ty);
    }
    if let Some(ret) = &mut func.ret_type {
        transform_type(ret);
    }

    // 3. Type substitution within function body statements
    transform_block(&mut func.body);

    // 4. Implicit Unsafe Pass: Wrap body in unsafe block
    let original_stmts = std::mem::take(&mut func.body.stmts);
    let unsafe_block = Block {
        stmts: original_stmts,
        is_unsafe: true,
    };
    func.body.stmts = vec![Stmt::Expr(Expr::Block(unsafe_block))];
}

/// Applies all transformation passes to a Struct.
pub fn transform_struct(strct: &mut Struct) {
    // 1. Auto-Repr Pass: Prepend #[repr(C)]
    if !strct.attrs.iter().any(|a| a.tokens == "repr(C)") {
        strct.attrs.insert(0, Attribute { tokens: "repr(C)".to_string() });
    }

    // 2. Type substitution on field types
    for field in &mut strct.fields {
        transform_type(&mut field.ty);
    }
}

/// Applies all AST transformation passes to the entire Program.
pub fn transform_program(program: &mut Program) {
    for item in &mut program.items {
        match item {
            Item::Fn(func) => {
                transform_fn(func);
            }
            Item::Struct(strct) => {
                transform_struct(strct);
            }
            Item::Use(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn parse_and_transform(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let mut program = parser.parse_program().unwrap();
        transform_program(&mut program);
        program
    }

    #[test]
    fn test_transform_c_abi_and_repr() {
        let src = r#"
            struct Point { x: int, y: int* }
            fn add(a: int, b: int) -> int {
                return a + b;
            }
        "#;
        let program = parse_and_transform(src);
        assert_eq!(program.items.len(), 2);

        // Verify struct has repr(C) and int -> c_int
        if let Item::Struct(s) = &program.items[0] {
            assert_eq!(s.name, "Point");
            assert!(s.attrs.iter().any(|a| a.tokens == "repr(C)"));
            assert_eq!(s.fields[0].ty, Type::UserDefined("c_int".to_string()));
        } else {
            panic!("Expected struct");
        }

        // Verify fn has no_mangle, extern "C", and int -> c_int
        if let Item::Fn(f) = &program.items[1] {
            assert_eq!(f.name, "add");
            assert!(f.attrs.iter().any(|a| a.tokens == "no_mangle"));
            assert_eq!(f.abi, Some("C".to_string()));
            if let Stmt::Expr(Expr::Block(b)) = &f.body.stmts[0] {
                assert!(b.is_unsafe);
            } else {
                panic!("Expected unsafe block");
            }
            assert_eq!(f.params[0].ty, Type::UserDefined("c_int".to_string()));
            assert_eq!(f.ret_type, Some(Type::UserDefined("c_int".to_string())));
        } else {
            panic!("Expected function");
        }
    }
}


