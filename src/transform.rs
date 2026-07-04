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
        Type::Array { base, .. } => {
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
            if let Some(tokens) = init {
                transform_token_stream(tokens);
            }
        }
        Stmt::If { cond, then_branch, else_branch } => {
            transform_token_stream(cond);
            transform_block(then_branch);
            if let Some(eb) = else_branch {
                transform_block(eb);
            }
        }
        Stmt::Block(block) => {
            transform_block(block);
        }
        Stmt::Return(val) => {
            if let Some(tokens) = val {
                transform_token_stream(tokens);
            }
        }
        Stmt::Raw { tokens, .. } => {
            transform_token_stream(tokens);
        }
    }
}

use crate::lexer::Token;

/// Flat token stream type/pointer transformer.
pub fn transform_token_stream(tokens: &mut Vec<Token>) {
    let mut result = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == Token::As {
            result.push(Token::As);
            i += 1;
            
            // Parse the type following 'as'
            if i < tokens.len() {
                if let Some((base_str, consumed)) = parse_base_type_name_at(&tokens[i..]) {
                    let mapped_base = map_base_type_name(&base_str);
                    let mut next_idx = i + consumed;
                    
                    // Parse postfix pointer modifiers
                    let mut ptr_modifiers = Vec::new();
                    loop {
                        if next_idx < tokens.len() && tokens[next_idx] == Token::Star {
                            ptr_modifiers.push(false); // *mut
                            next_idx += 1;
                        } else if next_idx + 1 < tokens.len() && tokens[next_idx] == Token::Const && tokens[next_idx + 1] == Token::Star {
                            ptr_modifiers.push(true); // *const
                            next_idx += 2;
                        } else {
                            break;
                        }
                    }
                    
                    // Emit transformed type tokens
                    if !ptr_modifiers.is_empty() {
                        for is_const in ptr_modifiers {
                            result.push(Token::Star);
                            if is_const {
                                result.push(Token::Const);
                            } else {
                                result.push(Token::Mut);
                            }
                        }
                    }
                    result.push(Token::Ident(mapped_base));
                    i = next_idx;
                }
            }
        } else {
            // Global primitive type mappings (single-word and multi-word C types)
            if let Some((base_str, consumed)) = parse_base_type_name_at(&tokens[i..]) {
                let mapped = map_base_type_name(&base_str);
                // Only replace if it's a known C type to avoid replacing custom variable names
                if is_known_c_type(&base_str) {
                    result.push(Token::Ident(mapped));
                    i += consumed;
                    continue;
                }
            }
            
            result.push(tokens[i].clone());
            i += 1;
        }
    }
    *tokens = result;
}

fn parse_base_type_name_at(slice: &[Token]) -> Option<(String, usize)> {
    if slice.is_empty() {
        return None;
    }

    let get_ident = |tok: &Token| -> Option<String> {
        match tok {
            Token::Ident(s) => Some(s.clone()),
            Token::Int => Some("int".to_string()),
            Token::Char => Some("char".to_string()),
            Token::Void => Some("void".to_string()),
            Token::Uint => Some("uint".to_string()),
            Token::Long => Some("long".to_string()),
            _ => None,
        }
    };

    let w1 = get_ident(&slice[0]);
    let w2 = slice.get(1).and_then(get_ident);
    let w3 = slice.get(2).and_then(get_ident);
    let w4 = slice.get(3).and_then(get_ident);

    if let (Some(ref a), Some(ref b), Some(ref c), Some(ref d)) = (&w1, &w2, &w3, &w4) {
        let full = format!("{} {} {} {}", a, b, c, d);
        if is_known_c_type(&full) {
            return Some((full, 4));
        }
    }
    if let (Some(ref a), Some(ref b), Some(ref c)) = (&w1, &w2, &w3) {
        let full = format!("{} {} {}", a, b, c);
        if is_known_c_type(&full) {
            return Some((full, 3));
        }
    }
    if let (Some(ref a), Some(ref b)) = (&w1, &w2) {
        let full = format!("{} {}", a, b);
        if is_known_c_type(&full) {
            return Some((full, 2));
        }
    }
    if let Some(ref a) = w1 {
        if is_known_c_type(a) || is_rust_primitive(a) || matches!(slice[0], Token::Ident(_)) {
            return Some((a.clone(), 1));
        }
    }

    None
}

fn map_base_type_name(name: &str) -> String {
    let mapped = match name {
        "void" => Some("c_void"),
        "char" => Some("c_char"),
        "int" => Some("c_int"),
        "uint" => Some("c_uint"),
        "long" => Some("c_long"),
        "float" => Some("c_float"),
        "double" => Some("c_double"),
        "short" => Some("c_short"),

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
        "unsigned" => Some("c_uint"),
        "signed" => Some("c_int"),

        "size_t" => Some("size_t"),
        "ssize_t" => Some("ssize_t"),
        "ptrdiff_t" => Some("ptrdiff_t"),
        "uintptr_t" => Some("uintptr_t"),
        "intptr_t" => Some("intptr_t"),
        "off_t" => Some("off_t"),
        "pid_t" => Some("pid_t"),

        _ => None,
    };
    mapped.unwrap_or(name).to_string()
}

fn is_known_c_type(s: &str) -> bool {
    matches!(
        s,
        "void" | "char" | "int" | "uint" | "long" | "float" | "double" | "short"
            | "unsigned char"
            | "signed char"
            | "unsigned short"
            | "signed short"
            | "short int"
            | "unsigned short int"
            | "signed short int"
            | "unsigned int"
            | "signed int"
            | "unsigned long"
            | "signed long"
            | "long int"
            | "unsigned long int"
            | "signed long int"
            | "long long"
            | "unsigned long long"
            | "signed long long"
            | "long long int"
            | "unsigned long long int"
            | "signed long long int"
            | "long double"
            | "unsigned"
            | "signed"
            | "size_t"
            | "ssize_t"
            | "ptrdiff_t"
            | "uintptr_t"
            | "intptr_t"
            | "off_t"
            | "pid_t"
    )
}

fn is_rust_primitive(s: &str) -> bool {
    matches!(
        s,
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
            | "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
            | "f32" | "f64" | "bool" | "str"
    )
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

    // 4. Implicit Unsafe Pass: Wrap body in unsafe block only if the function is unsafe
    if func.is_unsafe {
        let original_stmts = std::mem::take(&mut func.body.stmts);
        let unsafe_block = Block {
            stmts: original_stmts,
            is_unsafe: true,
        };
        func.body.stmts = vec![Stmt::Block(unsafe_block)];
    }
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
            Item::Enum { tokens, .. } => {
                transform_token_stream(tokens);
            }
            Item::Impl { methods, .. } => {
                for method in methods {
                    transform_fn(method);
                }
            }
            Item::Raw { tokens, .. } => {
                transform_token_stream(tokens);
            }
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
            proc add(a: int, b: int) -> int {
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

        // Verify proc has no_mangle, extern "C", and int -> c_int
        if let Item::Fn(f) = &program.items[1] {
            assert_eq!(f.name, "add");
            assert!(f.attrs.iter().any(|a| a.tokens == "no_mangle"));
            assert_eq!(f.abi, Some("C".to_string()));
            assert!(f.is_unsafe); // proc is unsafe by default
            if let Stmt::Block(b) = &f.body.stmts[0] {
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

    #[test]
    fn test_transform_fn_remains_safe() {
        let src = r#"
            fn add_safe(a: int, b: int) -> int {
                return a + b;
            }
        "#;
        let program = parse_and_transform(src);
        assert_eq!(program.items.len(), 1);

        if let Item::Fn(f) = &program.items[0] {
            assert_eq!(f.name, "add_safe");
            assert!(!f.is_unsafe); // fn is safe by default
            // The body should NOT contain a nested unsafe block
            if let Stmt::Return(_) = &f.body.stmts[0] {
                // Statements are directly in the body, not nested in unsafe block
            } else {
                panic!("Expected return statement directly in body");
            }
        } else {
            panic!("Expected function");
        }
    }
}


