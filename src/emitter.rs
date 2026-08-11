//! Code emitter: converts the transformed Carbide AST into Rust source text.
//!
//! Function bodies are emitted **verbatim** from `body_src` (the raw source
//! text captured by the parser).  Only the structural skeleton — function
//! signatures, struct definitions, headers — is constructed by the emitter.
//! This preserves all user formatting, comments, and whitespace exactly.

use crate::ast::*;

/// Emits a transformed Carbide AST as a Rust source string.
pub struct Emitter {
    output: String,
    indent_level: usize,
    no_std: bool,
}

impl Emitter {
    /// Create a new Emitter in standard library mode (does not emit `#![no_std]`).
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            no_std: false,
        }
    }

    /// Create a new Emitter with an explicit `no_std` configuration.
    pub fn with_no_std(no_std: bool) -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            no_std,
        }
    }

    /// Enable or disable `#![no_std]` emission.
    pub fn set_no_std(&mut self, no_std: bool) {
        self.no_std = no_std;
    }

    /// Retrieve the final emitted source.
    pub fn finish(self) -> String {
        self.output
    }

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    fn indent(&mut self) {
        self.output.push_str(&"    ".repeat(self.indent_level));
    }

    fn line(&mut self, s: &str) {
        self.indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    // -------------------------------------------------------------------------
    // Program
    // -------------------------------------------------------------------------

    /// Emit a complete program, prepending `#![no_std]` (if configured) and the required
    /// `use` imports.
    pub fn emit_program(&mut self, program: &Program) {
        // Emit items to a temporary buffer so we can scan for libc types
        let mut tmp = Emitter::new();
        for item in &program.items {
            tmp.emit_item(item);
            tmp.output.push('\n');
        }
        let items_src = tmp.finish();

        let libc_types = [
            "size_t",
            "ssize_t",
            "ptrdiff_t",
            "uintptr_t",
            "intptr_t",
            "off_t",
            "off64_t",
            "wchar_t",
            "pid_t",
            "uid_t",
            "gid_t",
            "id_t",
            "idtype_t",
            "mode_t",
            "dev_t",
            "ino_t",
            "ino64_t",
            "nlink_t",
            "blksize_t",
            "blkcnt_t",
            "FILE",
            "fpos_t",
            "DIR",
            "dirent",
            "stat",
            "stat64",
            "time_t",
            "clock_t",
            "clockid_t",
            "suseconds_t",
            "timespec",
            "timeval",
            "iovec",
            "pollfd",
            "nfds_t",
            "fd_set",
            "socklen_t",
            "sa_family_t",
            "sockaddr",
            "sockaddr_in",
            "sockaddr_in6",
            "sockaddr_storage",
            "sockaddr_un",
            "in_addr",
            "in6_addr",
            "in_addr_t",
            "in_port_t",
            "msghdr",
            "cmsghdr",
            "pthread_t",
            "pthread_mutex_t",
            "pthread_mutexattr_t",
            "pthread_cond_t",
            "pthread_condattr_t",
            "pthread_rwlock_t",
            "pthread_rwlockattr_t",
            "pthread_key_t",
            "pthread_once_t",
            "pthread_attr_t",
            "sigset_t",
            "siginfo_t",
            "sig_atomic_t",
            "rlimit",
            "rlimit64",
            "rusage",
            "rlim_t",
            "va_list",
            "Dl_info",
        ];
        let needs_libc = libc_types.iter().any(|t| contains_word(&items_src, t));

        let atomic_types = [
            "AtomicBool",
            "AtomicI32",
            "AtomicU32",
            "AtomicI64",
            "AtomicU64",
            "AtomicIsize",
            "AtomicUsize",
            "AtomicPtr",
            "Ordering",
        ];
        let needs_atomics = atomic_types.iter().any(|t| contains_word(&items_src, t));

        if self.no_std {
            self.output.push_str("#![no_std]\n");
        }
        // FFI bindings carry C-style names (snake_case types, SCREAMING
        // constants) - silence the style lints that would otherwise fire on
        // every generated binding (same hygiene as bindgen output).
        self.output.push_str("#![allow(non_camel_case_types)]\n");
        self.output.push_str("#![allow(non_snake_case)]\n");
        self.output
            .push_str("#![allow(non_upper_case_globals)]\n\n");
        self.output.push_str("use core::ffi::*;\n");
        if needs_libc {
            self.output.push_str("use libc::*;\n");
        }
        if needs_atomics {
            self.output.push_str("use core::sync::atomic::*;\n");
        }
        self.output.push('\n');
        self.output.push_str(&items_src);
    }

    // -------------------------------------------------------------------------
    // Item
    // -------------------------------------------------------------------------

    fn emit_item(&mut self, item: &Item) {
        match item {
            Item::Use(path) => {
                self.line(&format!("use {};", path));
            }

            Item::Struct(s) => {
                self.emit_struct(s);
            }

            Item::Fn(f) => {
                self.emit_fn(f);
            }

            Item::Enum { name, body } => {
                self.line(&format!("enum {} {{", name));
                // Body already contains the user's enum variants verbatim
                self.output.push_str(body);
                if !body.ends_with('\n') {
                    self.output.push('\n');
                }
                self.line("}");
            }

            Item::Impl { target, methods } => {
                self.line(&format!("impl {} {{", target));
                self.indent_level += 1;
                for m in methods {
                    self.emit_fn(m);
                    self.output.push('\n');
                }
                self.indent_level -= 1;
                self.line("}");
            }

            Item::TypeAlias { name, ty } => {
                self.indent();
                self.output.push_str(&format!("pub type {} = ", name));
                self.emit_type(ty);
                self.output.push_str(";\n");
            }

            Item::Raw { attrs, src } => {
                for a in attrs {
                    self.line(&format!("#[{}]", a.tokens));
                }
                self.indent();
                self.output.push_str(src);
                self.output.push('\n');
            }
        }
    }

    // -------------------------------------------------------------------------
    // Struct
    // -------------------------------------------------------------------------

    fn emit_struct(&mut self, s: &Struct) {
        for a in &s.attrs {
            self.line(&format!("#[{}]", a.tokens));
        }
        self.line(&format!("pub struct {} {{", s.name));
        self.indent_level += 1;
        for field in &s.fields {
            self.indent();
            self.output.push_str(&format!("pub {}: ", field.name));
            self.emit_type(&field.ty);
            self.output.push_str(",\n");
        }
        self.indent_level -= 1;
        self.line("}");
    }

    // -------------------------------------------------------------------------
    // Function
    // -------------------------------------------------------------------------

    fn emit_fn(&mut self, f: &Function) {
        for a in &f.attrs {
            self.line(&format!("#[{}]", a.tokens));
        }

        self.indent();
        self.output.push_str("pub ");
        if f.is_unsafe {
            self.output.push_str("unsafe ");
        }
        if let Some(ref abi) = f.abi {
            self.output.push_str(&format!("extern \"{}\" ", abi));
        }
        self.output.push_str(&format!("fn {}(", f.name));

        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.output.push_str(&format!("{}: ", p.name));
            self.emit_type(&p.ty);
        }
        self.output.push(')');

        if let Some(ref ret) = f.ret_type {
            if !is_void_type(ret) {
                self.output.push_str(" -> ");
                self.emit_type(ret);
            }
        }

        // Emit the body verbatim between `{` and `}`.
        // body_src is the raw text between the original braces — it already
        // carries the user's indentation and newlines.
        self.output.push_str(" {");
        self.output.push_str(&f.body_src);
        self.output.push_str("}\n");
    }

    // -------------------------------------------------------------------------
    // Type
    // -------------------------------------------------------------------------

    /// Emit a structured type node.  Only called for signature types; body
    /// types are emitted as part of the verbatim `body_src` string.
    pub fn emit_type(&mut self, ty: &Type) {
        match ty {
            Type::Primitive(PrimitiveType::RustPrimitive(s)) => {
                self.output.push_str(s);
            }
            Type::UserDefined(name) => {
                self.output.push_str(name);
            }
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
            Type::FnPointer { params, ret } => {
                // Function pointers are nullable → Option<…> with the
                // pointer-null optimisation, and calling into C / system is unsafe.
                self.output.push_str("Option<unsafe extern \"system\" fn(");
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&format!("{}: ", p.name));
                    self.emit_type(&p.ty);
                }
                self.output.push(')');
                if let Some(ret) = ret {
                    if !is_void_type(ret) {
                        self.output.push_str(" -> ");
                        self.emit_type(ret);
                    }
                }
                self.output.push('>');
            }
        }
    }
}

/// Check if a type represents the `void` / unit return type.
fn is_void_type(ty: &Type) -> bool {
    match ty {
        Type::UserDefined(name) => name == "void" || name == "c_void" || name == "()",
        Type::Primitive(PrimitiveType::RustPrimitive(name)) => name == "()",
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Word-boundary matching helper
// ---------------------------------------------------------------------------

fn contains_word(text: &str, word: &str) -> bool {
    let bytes = text.as_bytes();
    let word_bytes = word.as_bytes();
    let word_len = word_bytes.len();
    if word_len == 0 || bytes.len() < word_len {
        return false;
    }
    let mut i = 0;
    while i + word_len <= bytes.len() {
        if &bytes[i..i + word_len] == word_bytes {
            let left_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            let right_idx = i + word_len;
            let right_ok = right_idx == bytes.len() || !(bytes[right_idx].is_ascii_alphanumeric() || bytes[right_idx] == b'_');
            if left_ok && right_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::transform::transform_program;

    fn transpile(src: &str) -> String {
        let tokens = Lexer::new(src).tokenize_with_positions().unwrap();
        let mut program = Parser::new(src, tokens).parse_program().unwrap();
        transform_program(&mut program);
        let mut emitter = Emitter::new();
        emitter.emit_program(&program);
        emitter.finish()
    }

    fn transpile_no_std(src: &str) -> String {
        let tokens = Lexer::new(src).tokenize_with_positions().unwrap();
        let mut program = Parser::new(src, tokens).parse_program().unwrap();
        transform_program(&mut program);
        let mut emitter = Emitter::with_no_std(true);
        emitter.emit_program(&program);
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
                return (*p).x;
            }
        "#;
        let output = transpile(src);

        // By default, standard library mode is used (no #![no_std])
        assert!(!output.contains("#![no_std]"), "Unexpected no_std in default mode");
        assert!(output.contains("use core::ffi::*;"), "Missing ffi import");
        assert!(!output.contains("use libc::*;"), "Unexpected libc import");
        assert!(output.contains("#[repr(C)]"), "Missing repr(C)");
        assert!(output.contains("pub struct Point"), "Missing struct");
        assert!(output.contains("pub x: c_int"), "Missing c_int field");
        assert!(
            output.contains("pub y: *mut c_int"),
            "Missing pointer field"
        );
        assert!(output.contains("#[no_mangle]"), "Missing no_mangle");
        assert!(
            output.contains("pub unsafe extern \"system\" fn add(p: *const Point) -> c_int"),
            "Wrong function signature: {output}"
        );
        // Body is emitted verbatim — user's formatting preserved
        assert!(output.contains("return (*p).x;"), "Body not preserved");

        // With no_std enabled:
        let output_no_std = transpile_no_std(src);
        assert!(output_no_std.contains("#![no_std]"), "Missing no_std in no_std mode");
    }

    #[test]
    fn test_emitter_fn_pointer_and_type_alias() {
        let src = r#"
            type AudioCallback = fn(buffer: void*, frames: uint) -> void;

            struct Plugin {
                init: fn(plugin: Plugin const*) -> bool,
                destroy: fn(plugin: Plugin const*) -> void
            }
        "#;
        let output = transpile(src);

        // Type alias: C fn-pointer typedef → Option<unsafe extern "system" fn>
        assert!(
            output.contains("pub type AudioCallback = Option<unsafe extern \"system\" fn(buffer: *mut c_void, frames: c_uint)>;"),
            "Wrong AudioCallback alias: {output}"
        );

        // Struct fn-pointer fields: nullable + system ABI + void return omitted
        assert!(
            output.contains(
                "pub init: Option<unsafe extern \"system\" fn(plugin: *const Plugin) -> bool>,"
            ),
            "Wrong init field: {output}"
        );
        assert!(
            output.contains(
                "pub destroy: Option<unsafe extern \"system\" fn(plugin: *const Plugin)>,"
            ),
            "Wrong destroy field: {output}"
        );

        // Both structs/aliases get their usual attributes
        assert!(output.contains("#[repr(C)]"), "Missing repr(C) on struct");
    }

    #[test]
    fn test_emitter_atomics_import() {
        let src = r#"
            struct AtomicCounter {
                val: atomic_int,
                flag: atomic_bool
            }
        "#;
        let output = transpile(src);
        assert!(output.contains("use core::sync::atomic::*;"), "Missing core::sync::atomic import");
        assert!(output.contains("pub val: AtomicI32,"), "Missing AtomicI32 field");
        assert!(output.contains("pub flag: AtomicBool,"), "Missing AtomicBool field");
    }

    #[test]
    fn test_emitter_posix_libc_import() {
        let src = r#"
            struct FileInfo {
                owner: uid_t,
                group: gid_t,
                mode: mode_t,
                modified: time_t,
                fp: FILE*
            }
        "#;
        let output = transpile(src);
        assert!(output.contains("use libc::*;"), "Missing libc import for POSIX types");
        assert!(output.contains("pub owner: uid_t,"), "Missing uid_t field");
        assert!(output.contains("pub fp: *mut FILE,"), "Missing FILE* field");
    }

    #[test]
    fn test_emitter_sockets_pthreads_iovec_import() {
        let src = r#"
            struct ServerState {
                thread: pthread_t,
                lock: pthread_mutex_t,
                addr: sockaddr_in,
                buffer: iovec
            }
        "#;
        let output = transpile(src);
        assert!(output.contains("use libc::*;"), "Missing libc import for sockets/pthreads/iovec");
        assert!(output.contains("pub thread: pthread_t,"), "Missing pthread_t field");
        assert!(output.contains("pub lock: pthread_mutex_t,"), "Missing pthread_mutex_t field");
        assert!(output.contains("pub addr: sockaddr_in,"), "Missing sockaddr_in field");
        assert!(output.contains("pub buffer: iovec,"), "Missing iovec field");
    }
}
