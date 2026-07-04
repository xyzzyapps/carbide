# Learn Carbide in Y Minutes

Carbide is a low-level dialect of Rust tailored for C ABI compatibility, systems programming, and bare-metal FFI. It merges C-style types and pointer syntax with Rust's structure and safety, compiling directly to `#![no_std]` Rust code.

```rust
// A single-line comment.
/* A multi-line block comment. */

////////////////////////////////////////////////////////////////////////////////
// 1. Primitive C Types & Rust Primitives
////////////////////////////////////////////////////////////////////////////////

struct TypeShowcase {
    // Carbide maps C-style primitive type keywords to core::ffi equivalents:
    c_integer: int,                  // c_int (usually 32-bit signed)
    c_unsigned: unsigned int,        // c_uint (usually 32-bit unsigned)
    c_unsigned_short: unsigned short,// c_ushort
    c_signed_char: signed char,      // c_schar
    c_character: char,               // c_char
    
    // Multi-word integers:
    c_long_val: long,                // c_long
    c_ulong_val: unsigned long,      // c_ulong
    c_long_long: long long,          // c_longlong
    c_ulong_long: unsigned long long,// c_ulonglong
    
    // Floating point types:
    c_float_val: float,              // c_float
    c_double_val: double,            // c_double
    c_long_double: long double,      // c_double
    
    // Void type is written as void (typically used in pointer targets void* or return void)
    
    // 100% of standard Rust types bypass the mapping and remain untouched:
    rust_i32: i32,
    rust_u8: u8,
    rust_f64: f64,
    rust_bool: bool,
    rust_usize: usize
}

////////////////////////////////////////////////////////////////////////////////
// 2. Postfix Pointer Syntax
////////////////////////////////////////////////////////////////////////////////

struct PointerShowcase {
    // Carbide supports C-style postfix raw pointer syntax:
    mutable_ptr: int*,               // *mut c_int
    constant_ptr: int const*,        // *const c_int
    double_ptr: void**,              // *mut *mut c_void
    struct_ptr: TypeShowcase*        // *mut TypeShowcase
}

////////////////////////////////////////////////////////////////////////////////
// 3. Functions (fn) vs. Procedures (proc)
////////////////////////////////////////////////////////////////////////////////

// Carbide distinguishes safe functions from unsafe FFI procedures:

// A. safe fn: Safe by default
// - Emits standard `pub extern "C" fn` in Rust.
// - Body is safe (no implicit unsafe block).
fn add_safe(a: i32, b: i32) -> i32 {
    return a + b;
}

// B. unsafe proc: Unsafe by default
// - Emits `pub unsafe extern "C" fn` in Rust.
// - Body is implicitly wrapped in `unsafe {}`, permitting pointer arithmetic/dereferences.
proc compute_length(str_ptr: char const*) -> size_t {
    let mut len: size_t = 0;
    // Pointers can be dereferenced and navigated directly
    while *str_ptr != 0 {
        len = len + 1;
        str_ptr = str_ptr + 1; // Pointer arithmetic is allowed!
    }
    return len;
}

// C. Explicit unsafe fn: Works like proc (unsafe by default)
unsafe fn read_offset(ptr: int*, offset: usize) -> int {
    return *(ptr + offset);
}

////////////////////////////////////////////////////////////////////////////////
// 4. Struct Representation & memory layout
////////////////////////////////////////////////////////////////////////////////

// All struct definitions are automatically injected with #[repr(C)]
// ensuring stable memory layout compatible with C:
struct Point {
    x: float,
    y: float
}

proc scale_point(p: Point*, factor: float) -> void {
    (*p).x = (*p).x * factor;
    (*p).y = (*p).y * factor;
}

////////////////////////////////////////////////////////////////////////////////
// 5. libc Integration & no_std
////////////////////////////////////////////////////////////////////////////////

// - Every Carbide file is automatically injected with `#![no_std]`.
// - If libc-specific types (like size_t, pid_t, off_t, ssize_t) are used,
//   the transpiler automatically appends `use libc::*;` at the top of the file.
proc get_process_id() -> pid_t {
    // Transpiles to an FFI call that returns libc::pid_t
    return 0;
}
```

## Compilation and Tooling

Carbide transpiled files compile directly as standard Rust libraries or modules.

### 1. Direct Compilation via `carbide`
Transpile a `.carbide` file to standard `.rs` code:
```powershell
# Transpiles file.carbide -> file.rs
carbide file.carbide

# Transpiles and compiles directly with rustc into a static library
carbide file.carbide -c
```

### 2. Building Projects via `cargo carbide`
Build a Cargo package containing Carbide files under `src/`:
```powershell
# Compiles all Carbide code in your crate
cargo carbide build
```
This automatically manages a temporary FFI target in `target/carbide_workspace` and outputs compilation libraries (`.lib`, `.a`, `.dll`, `.so`) directly to your project's main target output folder.
