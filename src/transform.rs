//! AST transformation passes for the Carbide transpiler.
//!
//! Two categories of transformation:
//!
//! 1. **Signature transforms** — operate on parsed `Type` nodes in function
//!    parameters, return types, and struct fields.  These are precise because
//!    we have a typed AST.
//!
//! 2. **Body transforms** — operate on raw source text (`body_src: String`).
//!    They use word-boundary string replacement so that C type keywords inside
//!    function bodies are correctly mapped without touching unrelated
//!    identifiers.

use crate::ast::*;

// ---------------------------------------------------------------------------
// Type name mappings applied everywhere (signatures + body text)
// ---------------------------------------------------------------------------

/// C primitive type → Rust FFI type mapping table.
///
/// Order matters: longer / more-specific multi-word entries must come first
/// so that `unsigned long long` is replaced before `unsigned long`.
const TYPE_MAP: &[(&str, &str)] = &[
    // Multi-word C types (must precede their prefixes)
    ("unsigned long long",  "c_ulonglong"),
    ("unsigned long int",   "c_ulong"),
    ("unsigned long",       "c_ulong"),
    ("unsigned short int",  "c_ushort"),
    ("unsigned short",      "c_ushort"),
    ("unsigned int",        "c_uint"),
    ("unsigned char",       "c_uchar"),
    ("signed long long",    "c_longlong"),
    ("signed long int",     "c_long"),
    ("signed long",         "c_long"),
    ("signed short int",    "c_short"),
    ("signed short",        "c_short"),
    ("signed int",          "c_int"),
    ("signed char",         "c_schar"),
    ("long long int",       "c_longlong"),
    ("long long",           "c_longlong"),
    ("long double",         "c_double"),
    ("long int",            "c_long"),
    ("short int",           "c_short"),
    // Single-word C primitives
    ("void",    "c_void"),
    ("int",     "c_int"),
    ("uint",    "c_uint"),
    ("long",    "c_long"),
    ("short",   "c_short"),
    ("float",   "c_float"),
    ("double",  "c_double"),
    // Fixed-width integer types (<stdint.h>)
    ("int8_t",          "i8"),
    ("int16_t",         "i16"),
    ("int32_t",         "i32"),
    ("int64_t",         "i64"),
    ("uint8_t",         "u8"),
    ("uint16_t",        "u16"),
    ("uint32_t",        "u32"),
    ("uint64_t",        "u64"),
    ("intmax_t",        "i64"),
    ("uintmax_t",       "u64"),
    ("int_least8_t",    "i8"),
    ("int_least16_t",   "i16"),
    ("int_least32_t",   "i32"),
    ("int_least64_t",   "i64"),
    ("uint_least8_t",   "u8"),
    ("uint_least16_t",  "u16"),
    ("uint_least32_t",  "u32"),
    ("uint_least64_t",  "u64"),
    ("int_fast8_t",     "i8"),
    ("int_fast16_t",    "i16"),
    ("int_fast32_t",    "i32"),
    ("int_fast64_t",    "i64"),
    ("uint_fast8_t",    "u8"),
    ("uint_fast16_t",   "u16"),
    ("uint_fast32_t",   "u32"),
    ("uint_fast64_t",   "u64"),
    ("char16_t",        "u16"),
    ("char32_t",        "u32"),
    // C Atomics mapping to Rust core::sync::atomic types
    ("atomic_bool",        "AtomicBool"),
    ("atomic_int",         "AtomicI32"),
    ("atomic_uint",        "AtomicU32"),
    ("atomic_long",        "AtomicI64"),
    ("atomic_ulong",       "AtomicU64"),
    ("atomic_size_t",      "AtomicUsize"),
    ("atomic_intptr_t",    "AtomicIsize"),
    ("atomic_uintptr_t",   "AtomicUsize"),
    // NB: `char` is intentionally omitted — it is also a Rust keyword and
    // replacing it blindly inside body text would break char literals.
    // Struct field / parameter `char` types are handled via map_type().
];

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Apply all Carbide transforms to a parsed program in place.
///
/// Transforms applied:
/// - Function signatures: C-ABI (`extern "system"`), `#[no_mangle]`, `pub`,
///   `unsafe` (for `proc`), type mapping, postfix-pointer flip.
/// - Struct definitions: `#[repr(C)]`, `pub`, type mapping.
/// - Function bodies: word-boundary type substitution + `as TYPE*` pointer flip.
pub fn transform_program(program: &mut Program) {
    for item in &mut program.items {
        transform_item(item);
    }
}

fn transform_item(item: &mut Item) {
    match item {
        Item::Fn(f)          => transform_fn(f, true),
        Item::Struct(s)      => transform_struct(s),
        Item::Impl { methods, .. } => {
            for m in methods { transform_fn(m, false); }
        }
        Item::TypeAlias { ty, .. } => transform_type(ty),
        // Enum, Use, Raw — no signature to transform; body text is left as-is
        // (enum variants don't contain type keywords in signature positions).
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Function transformation
// ---------------------------------------------------------------------------

fn transform_fn(func: &mut Function, is_top_level: bool) {
    // Inject C-ABI attributes and calling convention only on top-level functions/procs (excluding main)
    if is_top_level && func.name != "main" {
        func.attrs.insert(0, Attribute { tokens: "no_mangle".to_string() });
        if func.abi.is_none() {
            func.abi = Some("system".to_string());
        }
    }

    // Map parameter types
    for param in &mut func.params {
        transform_type(&mut param.ty);
    }

    // Map return type
    if let Some(ref mut ty) = func.ret_type {
        transform_type(ty);
    }

    // Apply text-level substitutions to the body
    func.body_src = apply_body_transforms(&func.body_src);
}

// ---------------------------------------------------------------------------
// Struct transformation
// ---------------------------------------------------------------------------

fn transform_struct(strct: &mut Struct) {
    strct.attrs.insert(0, Attribute { tokens: "repr(C)".to_string() });
    for field in &mut strct.fields {
        transform_type(&mut field.ty);
    }
}

// ---------------------------------------------------------------------------
// Type node transformation (signature-level, precise)
// ---------------------------------------------------------------------------

/// Recursively map a parsed `Type` node using the C→FFI type table.
fn transform_type(ty: &mut Type) {
    match ty {
        Type::UserDefined(name) => {
            *name = map_type_name(name);
        }
        Type::Pointer { base, .. } | Type::Reference { base, .. } | Type::Array { base, .. } => {
            transform_type(base);
        }
        Type::FnPointer { params, ret } => {
            for param in params.iter_mut() {
                transform_type(&mut param.ty);
            }
            if let Some(ret) = ret {
                transform_type(ret);
            }
        }
        Type::Primitive(_) => {} // Rust primitives pass through unchanged
    }
}

/// Look up a type-name string in the mapping table.
fn map_type_name(name: &str) -> String {
    // `char` is mapped at the signature level only — the lexer tokenises it
    // as Token::Char so there is no ambiguity with Rust's `char` keyword.
    if name == "char" { return "c_char".to_string(); }
    for (from, to) in TYPE_MAP {
        if *from == name {
            return to.to_string();
        }
    }
    // libc / platform types and user-defined types pass through unchanged
    name.to_string()
}

// ---------------------------------------------------------------------------
// Body text transformation (string-level)
// ---------------------------------------------------------------------------

/// Apply all text-level substitutions to a function body source string.
///
/// 1. Multi-word and single-word C type keywords → FFI equivalents
///    (word-boundary aware, so `point` is not mangled by `int` → `c_int`).
/// 2. Postfix pointer casts: `as TYPE*` → `as *mut TYPE`,
///                           `as TYPE const*` → `as *const TYPE`.
pub fn apply_body_transforms(src: &str) -> String {
    let mut s = src.to_string();

    // Multi-word entries first (longer match wins)
    for (from, to) in TYPE_MAP {
        s = replace_word(&s, from, to);
    }

    // Postfix pointer cast flip: `as WORD*` and `as WORD const*`
    s = flip_as_pointer_casts(&s);

    // Postfix pointer and reference type annotations in body: `: WORD*`, `: WORD&`, etc.
    s = flip_colon_type_annotations(&s);

    // Prefix mut& borrow expressions: `mut& expr` -> `&mut expr`
    s = flip_prefix_mut_ref_expressions(&s);

    s
}

// ---------------------------------------------------------------------------
// Word-boundary string replacement (no regex crate needed)
// ---------------------------------------------------------------------------

/// Replace all word-boundary occurrences of `word` with `replacement`.
///
/// A match is a "word-boundary" match when the character immediately before
/// the match (if any) is not alphanumeric or `_`, and the character
/// immediately after the match (if any) is not alphanumeric or `_`.
///
/// This prevents `int` from matching inside `point`, `hint`, etc.
fn replace_word(src: &str, word: &str, replacement: &str) -> String {
    if word.is_empty() { return src.to_string(); }
    let mut result = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(pos) = rest.find(word) {
        let before   = &rest[..pos];
        let after    = &rest[pos + word.len()..];
        let word_end = pos + word.len();

        let before_ok = before.chars().last()
            .map_or(true, |c| !c.is_alphanumeric() && c != '_');
        let after_ok  = after.chars().next()
            .map_or(true, |c| !c.is_alphanumeric() && c != '_');

        result.push_str(before);
        if before_ok && after_ok {
            result.push_str(replacement);
        } else {
            result.push_str(word);
        }
        rest = &rest[word_end..];
    }
    result.push_str(rest);
    result
}

fn is_modifier_keyword_ahead(bytes: &[u8], pos: usize) -> bool {
    let len = bytes.len();
    if pos + 5 <= len && &bytes[pos..pos+5] == b"const" {
        let after = pos + 5;
        let mut m = after;
        while m < len && bytes[m].is_ascii_whitespace() { m += 1; }
        if m < len && (bytes[m] == b'*' || bytes[m] == b'&') {
            return true;
        }
    }
    if pos + 3 <= len && &bytes[pos..pos+3] == b"mut" {
        let after = pos + 3;
        let mut m = after;
        while m < len && bytes[m].is_ascii_whitespace() { m += 1; }
        if m < len && (bytes[m] == b'*' || bytes[m] == b'&') {
            return true;
        }
    }
    if pos < len && (bytes[pos] == b'*' || bytes[pos] == b'&') {
        return true;
    }
    false
}

/// Flip `as TYPE*`, `as TYPE const*`, `as TYPE&`, and `as TYPE const&` postfix casts in source text.
fn flip_as_pointer_casts(src: &str) -> String {
    let mut result = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let len   = bytes.len();
    let mut i = 0;

    while i < len {
        // Look for `as` followed by a word boundary
        if i + 2 < len && &bytes[i..i+2] == b"as" {
            let before_ok = i == 0 || !(bytes[i-1].is_ascii_alphanumeric() || bytes[i-1] == b'_');
            let after_as  = i + 2;
            let after_ok  = after_as < len &&
                (bytes[after_as] == b' ' || bytes[after_as] == b'\t' || bytes[after_as] == b'\n');

            if before_ok && after_ok {
                let mut j = after_as;
                // Skip whitespace
                while j < len && bytes[j].is_ascii_whitespace() { j += 1; }
                // Read typename identifier(s) (including :: and multi-word types)
                let type_start = j;
                while j < len && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b':' || bytes[j] == b' ') {
                    if bytes[j] == b' ' {
                        let mut next_w = j + 1;
                        while next_w < len && bytes[next_w].is_ascii_whitespace() { next_w += 1; }
                        if is_modifier_keyword_ahead(bytes, next_w) {
                            break;
                        }
                        if next_w < len && (bytes[next_w].is_ascii_alphanumeric() || bytes[next_w] == b'_') {
                            j = next_w;
                            continue;
                        } else {
                            break;
                        }
                    }
                    j += 1;
                }
                let type_name = std::str::from_utf8(&bytes[type_start..j]).unwrap_or("").trim();

                if !type_name.is_empty() {
                    // Try parsing one or more pointer/reference modifiers
                    let mut modifiers = Vec::new();
                    let mut scan_pos = j;

                    let mut k = scan_pos;
                    while k < len && bytes[k].is_ascii_whitespace() {
                        k += 1;
                    }

                    while k < len {
                        // Check for optional `const`
                        if k + 5 <= len && &bytes[k..k+5] == b"const" {
                            let after_const = k + 5;
                            let const_boundary = after_const >= len ||
                                bytes[after_const].is_ascii_whitespace() || bytes[after_const] == b'*' || bytes[after_const] == b'&';
                            if const_boundary {
                                let mut m = after_const;
                                while m < len && bytes[m].is_ascii_whitespace() { m += 1; }
                                if m < len && bytes[m] == b'*' {
                                    modifiers.push(Modifier::ConstPtr);
                                    k = m + 1;
                                    scan_pos = k;
                                    while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                                    continue;
                                } else if m < len && bytes[m] == b'&' {
                                    modifiers.push(Modifier::ConstRef);
                                    k = m + 1;
                                    scan_pos = k;
                                    while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                                    continue;
                                }
                            }
                        } else if k + 3 <= len && &bytes[k..k+3] == b"mut" {
                            let after_mut = k + 3;
                            let mut_boundary = after_mut >= len ||
                                bytes[after_mut].is_ascii_whitespace() || bytes[after_mut] == b'*' || bytes[after_mut] == b'&';
                            if mut_boundary {
                                let mut m = after_mut;
                                while m < len && bytes[m].is_ascii_whitespace() { m += 1; }
                                if m < len && bytes[m] == b'*' {
                                    modifiers.push(Modifier::MutPtr);
                                    k = m + 1;
                                    scan_pos = k;
                                    while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                                    continue;
                                } else if m < len && bytes[m] == b'&' {
                                    modifiers.push(Modifier::MutRef);
                                    k = m + 1;
                                    scan_pos = k;
                                    while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                                    continue;
                                }
                            }
                        } else if bytes[k] == b'*' {
                            modifiers.push(Modifier::MutPtr);
                            k += 1;
                            scan_pos = k;
                            while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                            continue;
                        } else if bytes[k] == b'&' {
                            modifiers.push(Modifier::MutRef);
                            k += 1;
                            scan_pos = k;
                            while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                            continue;
                        }
                        break;
                    }

                    if !modifiers.is_empty() {
                        // Look at the character immediately after modifiers (skipping whitespace)
                        // If it is followed by an expression operand, then `*` was binary multiplication!
                        let is_multiplication = if k < len {
                            let next_c = bytes[k] as char;
                            let is_operand_start = next_c.is_alphabetic() || next_c == '_' || next_c.is_numeric() ||
                                next_c == '"' || next_c == '\'' || next_c == '(' || next_c == '[';
                            is_operand_start && (modifiers.len() == 1 && matches!(modifiers[0], Modifier::MutPtr))
                        } else {
                            false
                        };

                        if !is_multiplication {
                            let mut ptr_prefix = String::new();
                            for m in modifiers.iter().rev() {
                                match m {
                                    Modifier::ConstPtr => ptr_prefix.push_str("*const "),
                                    Modifier::MutPtr => ptr_prefix.push_str("*mut "),
                                    Modifier::ConstRef => ptr_prefix.push_str("&"),
                                    Modifier::MutRef => ptr_prefix.push_str("&mut "),
                                }
                            }
                            let final_type_name = if type_name == "char" {
                                "c_char"
                            } else {
                                type_name
                            };
                            result.push_str(&format!("as {}{}", ptr_prefix, final_type_name));
                            i = scan_pos;
                            continue;
                        }
                    }
                }
            }
        }

        result.push(bytes[i] as char);
        i += 1;
    }

    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Modifier {
    MutPtr,
    ConstPtr,
    MutRef,
    ConstRef,
}

/// Flip postfix pointer and reference type annotations in source text (e.g. `let x: int* = ...` or `let r: int& = ...`).
fn flip_colon_type_annotations(src: &str) -> String {
    let mut result = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let len   = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b':' {
            let is_double_colon = (i > 0 && bytes[i-1] == b':') || (i + 1 < len && bytes[i+1] == b':');
            if !is_double_colon {
                let mut j = i + 1;
                while j < len && bytes[j].is_ascii_whitespace() { j += 1; }

                let type_start = j;
                while j < len && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b':' || bytes[j] == b' ') {
                    if bytes[j] == b' ' {
                        let mut next_w = j + 1;
                        while next_w < len && bytes[next_w].is_ascii_whitespace() { next_w += 1; }
                        if is_modifier_keyword_ahead(bytes, next_w) {
                            break;
                        }
                        if next_w < len && (bytes[next_w].is_ascii_alphanumeric() || bytes[next_w] == b'_') {
                            j = next_w;
                            continue;
                        } else {
                            break;
                        }
                    }
                    j += 1;
                }
                let type_name = std::str::from_utf8(&bytes[type_start..j]).unwrap_or("").trim();

                if !type_name.is_empty() {
                    let mut modifiers = Vec::new();
                    let mut scan_pos = j;

                    let mut k = scan_pos;
                    while k < len && bytes[k].is_ascii_whitespace() { k += 1; }

                    while k < len {
                        if k + 5 <= len && &bytes[k..k+5] == b"const" {
                            let after_const = k + 5;
                            let const_boundary = after_const >= len ||
                                bytes[after_const].is_ascii_whitespace() || bytes[after_const] == b'*' || bytes[after_const] == b'&';
                            if const_boundary {
                                let mut m = after_const;
                                while m < len && bytes[m].is_ascii_whitespace() { m += 1; }
                                if m < len && bytes[m] == b'*' {
                                    modifiers.push(Modifier::ConstPtr);
                                    k = m + 1;
                                    scan_pos = k;
                                    while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                                    continue;
                                } else if m < len && bytes[m] == b'&' {
                                    modifiers.push(Modifier::ConstRef);
                                    k = m + 1;
                                    scan_pos = k;
                                    while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                                    continue;
                                }
                            }
                        } else if k + 3 <= len && &bytes[k..k+3] == b"mut" {
                            let after_mut = k + 3;
                            let mut_boundary = after_mut >= len ||
                                bytes[after_mut].is_ascii_whitespace() || bytes[after_mut] == b'*' || bytes[after_mut] == b'&';
                            if mut_boundary {
                                let mut m = after_mut;
                                while m < len && bytes[m].is_ascii_whitespace() { m += 1; }
                                if m < len && bytes[m] == b'*' {
                                    modifiers.push(Modifier::MutPtr);
                                    k = m + 1;
                                    scan_pos = k;
                                    while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                                    continue;
                                } else if m < len && bytes[m] == b'&' {
                                    modifiers.push(Modifier::MutRef);
                                    k = m + 1;
                                    scan_pos = k;
                                    while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                                    continue;
                                }
                            }
                        } else if bytes[k] == b'*' {
                            modifiers.push(Modifier::MutPtr);
                            k += 1;
                            scan_pos = k;
                            while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                            continue;
                        } else if bytes[k] == b'&' {
                            modifiers.push(Modifier::MutRef);
                            k += 1;
                            scan_pos = k;
                            while k < len && bytes[k].is_ascii_whitespace() { k += 1; }
                            continue;
                        }
                        break;
                    }

                    if !modifiers.is_empty() {
                        let is_valid_type_term = if k < len {
                            let next_c = bytes[k] as char;
                            next_c == '=' || next_c == ';' || next_c == ',' || next_c == ')' || next_c == ']' || next_c == '}' || next_c == '\n'
                        } else {
                            true
                        };

                        if is_valid_type_term {
                            let mut ptr_prefix = String::new();
                            for m in modifiers.iter().rev() {
                                match m {
                                    Modifier::ConstPtr => ptr_prefix.push_str("*const "),
                                    Modifier::MutPtr => ptr_prefix.push_str("*mut "),
                                    Modifier::ConstRef => ptr_prefix.push_str("&"),
                                    Modifier::MutRef => ptr_prefix.push_str("&mut "),
                                }
                            }
                            let final_type_name = if type_name == "char" {
                                "c_char"
                            } else {
                                type_name
                            };
                            result.push_str(&format!(": {}{}", ptr_prefix, final_type_name));
                            i = scan_pos;
                            continue;
                        }
                    }
                }
            }
        }

        result.push(bytes[i] as char);
        i += 1;
    }

    result
}

/// Flip `mut& expr` prefix mutable borrow expressions in body source text to `&mut expr`.
fn flip_prefix_mut_ref_expressions(src: &str) -> String {
    let mut result = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if i + 4 <= len && &bytes[i..i+4] == b"mut&" {
            let before_ok = i == 0 || !(bytes[i-1].is_ascii_alphanumeric() || bytes[i-1] == b'_');
            if before_ok {
                result.push_str("&mut ");
                i += 4;
                while i < len && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }

    result
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn transpile(src: &str) -> String {
        let tokens = Lexer::new(src).tokenize_with_positions().unwrap();
        let mut program = Parser::new(src, tokens).parse_program().unwrap();
        transform_program(&mut program);
        // Quick structural check: emit just enough to verify transforms
        let mut out = String::new();
        for item in &program.items {
            if let Item::Fn(f) = item {
                for a in &f.attrs { out.push_str(&format!("#[{}]\n", a.tokens)); }
                if f.is_unsafe { out.push_str("unsafe "); }
                if let Some(ref abi) = f.abi { out.push_str(&format!("extern \"{}\" ", abi)); }
                out.push_str(&format!("fn {}(", f.name));
                for (i, p) in f.params.iter().enumerate() {
                    if i > 0 { out.push_str(", "); }
                    // Quick type display
                    out.push_str(&format!("{}: {:?}", p.name, p.ty));
                }
                out.push_str(")\n");
                out.push_str(&f.body_src);
                out.push('\n');
            }
        }
        out
    }

    #[test]
    fn test_transform_fn_remains_safe() {
        let src = "fn safe(x: int) -> int { return x; }";
        let out = transpile(src);
        assert!(out.contains("extern \"system\""), "missing extern system");
        assert!(!out.contains("unsafe"), "fn should not be unsafe");
    }

    #[test]
    fn test_transform_system_abi_and_repr() {
        let src = "proc go(p: void*) -> void { *p = 0; }";
        let out = transpile(src);
        assert!(out.contains("unsafe"), "proc should be unsafe");
        assert!(out.contains("extern \"system\""), "missing extern system");
    }

    #[test]
    fn test_replace_word() {
        assert_eq!(replace_word("int x = 0;", "int", "c_int"), "c_int x = 0;");
        assert_eq!(replace_word("point.x", "int", "c_int"), "point.x"); // no false match
        assert_eq!(replace_word("hint", "int", "c_int"), "hint");        // no false match
    }

    #[test]
    fn test_apply_body_transforms() {
        let src = "let x: int = 0; let p: void* = &x as void*;";
        let out = apply_body_transforms(src);
        assert!(out.contains("c_int"), "int should become c_int");
        assert!(out.contains("c_void"), "void should become c_void");
        assert!(out.contains("as *mut c_void"), "postfix pointer cast should flip");
    }

    #[test]
    fn test_binary_multiplication_not_flipped() {
        let src = "let res = e1 as usize * e2;";
        let out = apply_body_transforms(src);
        assert_eq!(out, "let res = e1 as usize * e2;");

        let src2 = "let total = a as int * 5;";
        let out2 = apply_body_transforms(src2);
        assert_eq!(out2, "let total = a as c_int * 5;");

        let src3 = "let total2 = a as i32 * 5;";
        let out3 = apply_body_transforms(src3);
        assert_eq!(out3, "let total2 = a as i32 * 5;");
    }

    #[test]
    fn test_char_pointer_and_atomics_and_impl_methods() {
        let src = "let s = p as char*; let c = p as char const*; let a: atomic_int = 0;";
        let out = apply_body_transforms(src);
        assert!(out.contains("as *mut c_char"), "char* should become *mut c_char");
        assert!(out.contains("as *const c_char"), "char const* should become *const c_char");
        assert!(out.contains("AtomicI32"), "atomic_int should become AtomicI32");

        let impl_src = "impl Point { fn new(x: float, y: float) -> Point { return Point { x, y }; } }";
        let tokens = Lexer::new(impl_src).tokenize_with_positions().unwrap();
        let mut program = Parser::new(impl_src, tokens).parse_program().unwrap();
        transform_program(&mut program);
        if let Item::Impl { methods, .. } = &program.items[0] {
            assert!(!methods[0].attrs.iter().any(|a| a.tokens == "no_mangle"), "impl methods must not have #[no_mangle]");
        } else {
            panic!("Expected Impl");
        }
    }

    #[test]
    fn test_mut_ref_expression_flip() {
        let src = "let raw = (mut& num as int*) as void*; let r = mut& x;";
        let out = apply_body_transforms(src);
        assert!(out.contains("(&mut num as *mut c_int) as *mut c_void"), "mut& cast should flip properly: {out}");
        assert!(out.contains("let r = &mut x;"), "mut& x should flip to &mut x");
    }

    #[test]
    fn test_stdint_fixed_width_types_mapping() {
        let src = "let a: int8_t = 1; let b: uint16_t = 2; let c: int32_t = 3; let d: uint64_t = 4; let e: intmax_t = 5; let f: char16_t = 6;";
        let out = apply_body_transforms(src);
        assert!(out.contains("let a: i8 = 1;"), "int8_t failed: {out}");
        assert!(out.contains("let b: u16 = 2;"), "uint16_t failed: {out}");
        assert!(out.contains("let c: i32 = 3;"), "int32_t failed: {out}");
        assert!(out.contains("let d: u64 = 4;"), "uint64_t failed: {out}");
        assert!(out.contains("let e: i64 = 5;"), "intmax_t failed: {out}");
        assert!(out.contains("let f: u16 = 6;"), "char16_t failed: {out}");
    }
}
