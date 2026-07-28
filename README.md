# Carbide: C-Style Rust Transpiler

<p align="center">
  <img src="assets/logo_transparent.png" alt="Carbide Logo" width="220" />
</p>

`carbide` is a transpiler and compiler frontend that compiles a custom dialect of Rust (`.carbide`) featuring C-style keywords and postfix pointer syntax into standard, FFI-compliant Rust code. The transpiler applies well-defined syntactic transformations and passes all other Rust code through unchanged, so **any valid `no_std` Rust code works in a `.carbide` file**. It includes an integrated driver to call `rustc` directly and a custom Cargo subcommand (`cargo-carbide`) that automatically manages FFI compilation targets and compiles pure static/dynamic libraries.

## Architecture

The compiler is structured as a transformation pipeline. The parser only deeply parses the structural skeleton (top-level items, function signatures, struct fields). Statement bodies are captured as **raw token streams** and passed through a lightweight token-level transformer. This design means any valid `no_std` Rust inside a function body passes through verbatim after type substitutions:

```mermaid
graph TD
    Source["Source Code (.carbide)"] --> Lexer[Lexer / Tokenizer]
    Lexer --> Tokens[Token Stream]
    Tokens --> Parser["Token-Stream Parser (structural skeleton only)"]
    Parser --> AST["AST (items + raw token bodies)"]
    AST --> Transform[Transformation Pipeline]

    subgraph Transformation Passes
        Transform --> Pass1[Type Substitution int/void/char -> c_int/c_void/c_char]
        Pass1 --> Pass2[Postfix Pointer Flip T* -> *mut T]
        Pass2 --> Pass3[C-ABI fn signature + no_mangle]
        Pass3 --> Pass4["proc -> unsafe fn + implicit unsafe{} body"]
        Pass4 --> Pass5[Auto #repr(C) on structs]
    end

    Pass5 --> TransAST[Transformed AST]
    TransAST --> Emitter[Code Generator / Emitter]
    Emitter --> Output["Rust Code (.rs)"]

    Output --> Driver[rustc / Cargo Compilation]
```

---

## Dialect Specification (The Grammar)

Carbide extends Rust by permitting C-style declarations and pointers in signatures:

### 1. Primitive C Types
The transpiler automatically maps C-style primitive type keywords:
- `void` $\rightarrow$ `core::ffi::c_void`
- `char` $\rightarrow$ `core::ffi::c_char`
- `signed char` $\rightarrow$ `core::ffi::c_schar`
- `unsigned char` $\rightarrow$ `core::ffi::c_uchar`
- `short` $\rightarrow$ `core::ffi::c_short`
- `unsigned short` $\rightarrow$ `core::ffi::c_ushort`
- `int` $\rightarrow$ `core::ffi::c_int`
- `unsigned int` / `unsigned` $\rightarrow$ `core::ffi::c_uint`
- `long` $\rightarrow$ `core::ffi::c_long`
- `unsigned long` $\rightarrow$ `core::ffi::c_ulong`
- `long long` $\rightarrow$ `core::ffi::c_longlong`
- `unsigned long long` $\rightarrow$ `core::ffi::c_ulonglong`
- `float` $\rightarrow$ `core::ffi::c_float`
- `double` $\rightarrow$ `core::ffi::c_double`
- `long double` $\rightarrow$ `core::ffi::c_double`
- Standard Rust types (e.g. `i32`, `u8`, `f32`, `bool`, `usize`) pass through unmodified.
- Any valid `no_std` Rust control flow (`while`, `loop`, `match`, `if`/`else`, `break`, `continue`, closures, etc.) is captured as a raw token stream and emitted verbatim after type substitutions. The Rust compiler handles all semantic checking.

### 2. libc Integration
Standard `libc` types are supported:
- `size_t` $\rightarrow$ `libc::size_t`
- `ssize_t` $\rightarrow$ `libc::ssize_t`
- `ptrdiff_t` $\rightarrow$ `libc::ptrdiff_t`
- `uintptr_t` $\rightarrow$ `libc::uintptr_t`
- `intptr_t` $\rightarrow$ `libc::intptr_t`
- `off_t` $\rightarrow$ `libc::off_t`
- `pid_t` $\rightarrow$ `libc::pid_t`

### 3. Postfix Pointer Notation
Carbide supports C-style postfix pointer syntax:
- `T*` $\rightarrow$ `*mut T` (Mutable pointer)
- `T const*` $\rightarrow$ `*const T` (Constant pointer)
- `T**` $\rightarrow$ `*mut *mut T` (Nested pointers)

### 4. Automatic FFI Attributes & Crate Directives
- **no_std**: The `#![no_std]` crate attribute is automatically prepended to every generated file, ensuring low-level and bare-metal compilation compatibility.
- **Conditional libc**: `use libc::*;` is conditionally imported only if libc-specific types (e.g., `size_t`, `pid_t`) are referenced in the source AST, keeping simple transpiled files free from external dependencies.
- **C-ABI**: Every top-level function or procedure is automatically injected with the `extern "C"` ABI calling convention and the `#[no_mangle]` attribute.
- **Function/Procedure Safety (`fn` vs `proc`)**:
  - `fn` declarations: Safe by default (emits standard `fn` in Rust, does **not** implicitly wrap body statements in unsafe).
  - `proc` declarations: Unsafe by default (emits `unsafe fn` in Rust, and automatically wraps all body statements in an implicit `unsafe {}` block, permitting low-level pointer dereferencing and arithmetic).
  - Explicit `unsafe fn` declarations are also supported and behave like `proc` (unsafe with implicit body wrapping).
- **Auto-Repr**: Every struct definition in the AST is automatically prepended with the `#[repr(C)]` attribute to ensure stable C memory layout.

---

## Workspace Layout

- `src/main.rs`: Command Line Interface parser and Cargo subcommand router.
- `src/lexer.rs`: Token definitions and tokenizer logic. Handles C primitive keywords, postfix `*` symbols, and all standard Rust tokens needed for pass-through.
- `src/ast.rs`: Intermediate representation nodes. Function and struct signatures are fully parsed; statement bodies are stored as `Vec<Token>` raw token streams.
- `src/parser.rs`: Hand-written recursive descent parser. Deeply parses structural items (functions, structs, enums, impls). Statement bodies are captured with balanced-brace tracking and stored as flat token streams.
- `src/transform.rs`: Runs structural mutation passes over the AST. Applies type substitutions and postfix-to-prefix pointer rewrites on token streams.
- `src/emitter.rs`: Formats the transformed AST back into compliant Rust code. Includes spacing-aware raw token emitter that handles `as *mut T` spacing and `}` newlines correctly.
- `tests/integration_tests.rs`: Multi-stage pipeline verification test (transpile + `rustc` compile).
- `tests/fixture_tests.rs`: Fixture-based runner that transpiles all `.carbide` files under `tests/fixtures/` and verifies output.

---

## Usage

### 1. Direct Compilation
Run the transpiler directly on a `.carbide` file:
```powershell
# Transpile only (generates main.rs)
carbide main.carbide

# Transpile to custom output path
carbide main.carbide -o output.rs

# Transpile and immediately compile using rustc
carbide main.carbide -c
```

### 2. Cargo subcommand Integration
To build a static library or dynamic library:
```powershell
# Transpile all .carbide files in src/ and compile the project
cargo carbide build
```
The subcommand:
1. Creates a temporary compilation workspace under `target/carbide_workspace`.
2. Automatically generates/injects FFI crate targets:
   ```toml
   [lib]
   crate-type = ["staticlib", "cdylib"]
   ```
3. Performs the build and copies FFI static/dynamic libraries (`carbide.lib`, `libcarbide.a`, `carbide.dll`) back to the main project's `target/debug/` directory.

## License

MIT
 
## Signature

Original Research by Xyzzy, built with assistance from **Gemini 3.5**.  
Specification target: the implementation described in [SPEC.md](SPEC.md).
