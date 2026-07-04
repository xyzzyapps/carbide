# Learn Carbide in Y Minutes

Ah, Carbide. The language of modern high-performance FFI and C-style Rust systems programming.

Carbide is a low-level dialect of Rust designed to merge C-style types and pointer semantics directly into standard, FFI-compliant Rust. It gives you the raw feel of C pointer notation combined with the structure and compiler benefits of Rust, prefixing files with `#![no_std]` and managing memory layouts automatically.

Just be aware of Carbide's specific safe (`fn`) vs unsafe (`proc`) declarations, and it will take you as far as you need to go in systems FFI.

```rust
// Single-line comments start with //

/*
Multi-line comments look like this.
*/

// Declarations of functions or procedures can be placed at the top of
// your file or in advance.
fn add_two_ints(x1: int, x2: int) -> int; // Function prototype

// Your entry point can be a function called "main".
// Carbide will output it as a public extern "C" function with #[no_mangle].
fn main() -> int {
    // print output using printf-like external declarations or bindings
    
    ///////////////////////////////////////
    // Types
    ///////////////////////////////////////

    // ints are mapped to core::ffi::c_int (usually 4 bytes)
    let mut x_int: int = 0;

    // shorts are mapped to core::ffi::c_short (usually 2 bytes)
    let mut x_short: short = 0;

    // chars are mapped to core::ffi::c_char (usually 1 byte)
    let mut x_char: char = 0;
    let mut y_char: char = 'y'; // Char literals are quoted with ''

    // longs are mapped to c_long (usually 4 to 8 bytes);
    // long longs are mapped to c_longlong (guaranteed to be at least 8 bytes)
    let mut x_long: long = 0;
    let mut x_long_long: long long = 0;

    // floats are mapped to core::ffi::c_float (usually 32-bit floating point)
    let mut x_float: float = 0.0;

    // doubles are mapped to core::ffi::c_double (64-bit floating-point)
    let mut x_double: double = 0.0;

    // integer types may be unsigned (greater than or equal to zero)
    let mut ux_short: unsigned short = 0;
    let mut ux_int: unsigned int = 0;
    let mut ux_long_long: unsigned long long = 0;

    // size_t is an unsigned integer type used to represent sizes
    // (If size_t or other libc types are used, use libc::*; is automatically imported)
    let mut size: size_t = 0;

    // String literals are represented using standard Rust double quotes.
    // In Carbide, they are raw strings:
    let mut a_string: char const* = "This is a string";

    ///////////////////////////////////////
    // Operators
    ///////////////////////////////////////

    // Arithmetic is straightforward
    let mut sum: int = 1 + 2;   // => 3
    let mut diff: int = 2 - 1;  // => 1
    let mut prod: int = 2 * 1;  // => 2
    let mut quot: int = 1 / 2;  // => 0 (truncated towards 0)

    // Comparisons return standard boolean flags in Rust:
    let mut is_equal: bool = 3 == 2; // => false
    let mut is_greater: bool = 3 > 2; // => true

    // Logic works on bool flags:
    let mut result: bool = !true; // => false
    let mut logical_and: bool = true && false; // => false

    ///////////////////////////////////////
    // Control Structures
    ///////////////////////////////////////

    // If/Else statements:
    if x_int == 0 {
        x_int = 10;
    } else {
        x_int = 20;
    }

    // NOTE: Carbide does not define loops (like while, for, do-while) in its AST.
    // Instead, repetitive FFI tasks are written recursively or offloaded to Rust bindings.

    ///////////////////////////////////////
    // Pointers
    ///////////////////////////////////////

    // A pointer is declared with a postfix star `*`:
    let mut x: int = 0;
    let mut px: int* = &mut x; // Retrieve address of x

    // Dereferencing a pointer uses the prefix `*` operator:
    let mut val: int = *px; // => 0 (the value of x)

    // You can also change the value the pointer is pointing to:
    *px = 5; // x is now 5

    // Pointer Arithmetic:
    // Raw pointer offsets in Carbide use the standard Rust .offset() method,
    // which compiles cleanly within implicit unsafe blocks:
    let mut px_offset: int* = px.offset(1);

    return 0;
}

///////////////////////////////////////
// Functions (fn) & Procedures (proc)
///////////////////////////////////////

// Carbide has two keyword types for FFI code generation:

// 1. safe fn: Safe by default
// - Transpiles to safe Rust `pub extern "C" fn`.
// - Body is parsed in safe Rust scope (no implicit unsafe block wrapper).
fn add_two_ints(x1: int, x2: int) -> int {
    return x1 + x2;
}

// 2. unsafe proc: Unsafe by default
// - Transpiles to `pub unsafe extern "C" fn`.
// - Body is automatically wrapped in an implicit `unsafe {}` block,
//   allowing pointer dereferences and offsets without manual wrapping.
proc swapTwoNumbers(a: int*, b: int*) -> void {
    let temp: int = *a;
    *a = *b;
    *b = temp;
}

// 3. Explicit unsafe fn: Works like proc (unsafe by default)
unsafe fn read_value(ptr: int*) -> int {
    return *ptr;
}

///////////////////////////////////////
// Structs & memory layout
///////////////////////////////////////

// Every struct definition is automatically prepended with #[repr(C)],
// guaranteeing C-compatible memory layout:
struct Point {
    x: float,
    y: float
}
```
