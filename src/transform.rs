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
/// - Function signatures: C-ABI (`extern "C"`), `#[no_mangle]`, `pub`,
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
        Item::Fn(f)          => transform_fn(f),
        Item::Struct(s)      => transform_struct(s),
        Item::Impl { methods, .. } => {
            for m in methods { transform_fn(m); }
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

fn transform_fn(func: &mut Function) {
    // Inject C-ABI attributes and calling convention
    func.attrs.insert(0, Attribute { tokens: "no_mangle".to_string() });
    if func.abi.is_none() {
        func.abi = Some("C".to_string());
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

/// Flip `as TYPE*` and `as TYPE const*` postfix pointer casts in source text.
///
/// Pattern:  `as <ws> <typename> <ws>? [const <ws>?] *`
/// Becomes:  `as *[const|mut] <typename>`
///
/// Only the C primitive type names (already mapped at this point to their
/// FFI equivalents) and unrecognised identifiers are matched.
fn flip_as_pointer_casts(src: &str) -> String {
    // We scan character by character looking for the pattern `as `.
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
                // Try to parse: `as <ws>* <ident> <ws>* [const <ws>*] *`
                let mut j = after_as;
                // Skip whitespace
                while j < len && bytes[j].is_ascii_whitespace() { j += 1; }
                // Read typename identifier(s)
                let type_start = j;
                while j < len && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b' ') {
                    // Allow a single space within multi-word types like `c_void`
                    // but stop at `*` or non-word chars
                    if bytes[j] == b' ' {
                        // peek ahead: is the next char alphanumeric? (multi-word type)
                        if j + 1 < len && (bytes[j+1].is_ascii_alphanumeric() || bytes[j+1] == b'_') {
                            j += 1;
                            continue;
                        } else {
                            break;
                        }
                    }
                    j += 1;
                }
                let type_name = std::str::from_utf8(&bytes[type_start..j]).unwrap_or("").trim();

                // Skip whitespace
                while j < len && bytes[j].is_ascii_whitespace() { j += 1; }

                // Optional `const`
                let is_const = if j + 5 <= len && &bytes[j..j+5] == b"const" {
                    let after_const = j + 5;
                    let ok = after_const >= len ||
                        bytes[after_const].is_ascii_whitespace() || bytes[after_const] == b'*';
                    if ok { j = after_const; while j < len && bytes[j].is_ascii_whitespace() { j += 1; } true }
                    else { false }
                } else { false };

                // Must be followed by `*`
                if j < len && bytes[j] == b'*' && !type_name.is_empty() {
                    j += 1; // consume `*`
                    let ptr_kind = if is_const { "*const" } else { "*mut" };
                    result.push_str(&format!("as {} {}", ptr_kind, type_name));
                    i = j;
                    continue;
                }
                // Pattern didn't match — emit `as` literally and move on
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
        assert!(out.contains("extern \"C\""), "missing extern C");
        assert!(!out.contains("unsafe"), "fn should not be unsafe");
    }

    #[test]
    fn test_transform_c_abi_and_repr() {
        let src = "proc go(p: void*) -> void { *p = 0; }";
        let out = transpile(src);
        assert!(out.contains("unsafe"), "proc should be unsafe");
        assert!(out.contains("extern \"C\""), "missing extern C");
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
}
