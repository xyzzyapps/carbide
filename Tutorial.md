# Learn Carbide in Y Minutes

Ah, Carbide. The language of modern high-performance FFI and C/C++-style Rust systems programming.

Carbide is a low-level dialect of Rust designed to merge C/C++-style types, atomic types, postfix pointer (`*`), and postfix reference (`&`) semantics directly into standard, FFI-compliant Rust. It gives you the raw feel of C/C++ notation combined with the structure and compiler benefits of Rust, managing memory layouts automatically. By default, it targets the standard library, while bare-metal projects can pass `--no-std`.

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
// Carbide will output it as a public extern "system" function with #[no_mangle].
fn main() -> int {
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

    // C Atomics (use core::sync::atomic::*; is automatically imported)
    let mut flag: atomic_bool = AtomicBool::new(true);
    let mut counter: atomic_int = AtomicI32::new(0);

    // String literals are represented using standard Rust double quotes.
    let mut a_string: char const* = "This is a string";

    ///////////////////////////////////////
    // Operators
    ///////////////////////////////////////

    // Arithmetic is straightforward
    let mut sum: int = 1 + 2;   // => 3
    let mut diff: int = 2 - 1;  // => 1
    let mut prod: int = 2 * 1;  // => 2
    let mut quot: int = 1 / 2;  // => 0 (truncated towards 0)
    let mut rem: int = 7 % 3;   // => 1 (modulo)

    // Bitwise operators
    let mut bitwise_or: int = 1 | 2;   // => 3
    let mut bitwise_xor: int = 3 ^ 1;  // => 2

    // Closures
    let add_ten = |n: int| n + 10;
    let computed: int = add_ten(5); // => 15

    // Comparisons return standard boolean flags in Rust:
    let mut is_equal: bool = 3 == 2; // => false
    let mut is_greater: bool = 3 > 2; // => true

    // Logic works on bool flags:
    let mut result: bool = !true; // => false
    let mut logical_and: bool = true && false; // => false

    ///////////////////////////////////////
    // Control Structures
    ///////////////////////////////////////

    // Any standard Rust control flow works inside bodies:
    if x_int == 0 {
        x_int = 10;
    } else {
        x_int = 20;
    }

    ///////////////////////////////////////
    // Pointers & References (C/C++ Conventions Exclusively)
    ///////////////////////////////////////

    // Pointers are declared with postfix `*` (or `mut*` / `const*`):
    let mut x: int = 42;
    let mut px: int* = &mut x; // Transpiles to `*mut c_int`
    let pcx: int const* = &x;   // Transpiles to `*const c_int`

    // References / borrows use postfix `&` (or `mut&` / `const&`):
    let r_mut: int& = &mut x;       // Transpiles to `&mut c_int`
    let r_const: int const& = &x;   // Transpiles to `&c_int`

    // Dereferencing uses the prefix `*` operator:
    let mut val: int = *px; // => 42

    *px = 5; // x is now 5

    // Pointer Arithmetic:
    let mut px_offset: int* = px.offset(1);

    // Expressions: binary multiplication following type cast is preserved:
    let e1: u32 = 10;
    let e2: usize = 20;
    let product: usize = e1 as usize * e2;

    return 0;
}

///////////////////////////////////////
// Functions (fn) & Procedures (proc)
///////////////////////////////////////

// Carbide provides two keyword types for FFI code generation:

// 1. safe fn: Safe by default
// - Transpiles to safe Rust `pub extern "system" fn`.
// - Body is in safe Rust scope.
fn add_two_ints(x1: int, x2: int) -> int {
    return x1 + x2;
}

// 2. unsafe proc: Unsafe by default
// - Transpiles to `pub unsafe extern "system" fn`.
// - Functions returning `void` omit the Rust return type arrow (unit `()`).
// - Body is automatically wrapped in an implicit `unsafe {}` block.
proc swapTwoNumbers(a: int*, b: int*) -> void {
    let temp: int = *a;
    *a = *b;
    *b = temp;
}

// 3. Explicit unsafe fn: Works like proc
unsafe fn read_value(ptr: int*) -> int {
    return *ptr;
}

///////////////////////////////////////
// Function Pointers & Callbacks
///////////////////////////////////////

// C/system callbacks in type position transpile to `Option<unsafe extern "system" fn(...)>`:
type AudioCallback = fn(buffer: void*, frames: uint) -> void;

///////////////////////////////////////
// Structs & Memory Layout
///////////////////////////////////////

// Every struct definition is automatically prepended with #[repr(C)],
// guaranteeing C-compatible memory layout:
struct Point {
    x: float,
    y: float
}

impl Point {
    // Methods inside impl blocks emit as standard Rust methods
    fn new(x: float, y: float) -> Point {
        return Point { x, y };
    }

    proc shift(self: Point&, dx: float, dy: float) -> void {
        self.x = self.x + dx;
        self.y = self.y + dy;
    }
}
```
