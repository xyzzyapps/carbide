---
name: carbide_transpiler
description: Reference guide for working with the Carbide dialect, maintaining the transpiler codebase, and using Carbide CLI/Cargo drivers.
---

## Carbide Dialect Overview

Carbide is a low-level dialect of Rust designed for seamless C/C++ ABI compatibility. It maps C-style primitive type keywords, postfix pointers (`*`) and references (`&`), function-pointer callbacks, and attributes to standard FFI-compliant Rust code.

### 1. Naming & File Extensions
- **Dialect Name**: Carbide
- **File Extension**: `.carbide`
- **Compiler Binary**: `carbide` (runs as `carbide <FILE> [-o <OUT>] [-c] [--std] [--no-std]`)
- **Cargo Command**: `cargo-carbide` (runs as `cargo carbide build [--no-std]`)

### 2. Syntax Rules
- **Function/Procedure Declarations**:
  - `fn name(...)`: Declares a **safe** function by default. Transpiles to safe `pub extern "system" fn name(...)` without implicit unsafe wrapping.
  - `proc name(...)`: Declares an **unsafe** procedure by default. Transpiles to `pub unsafe extern "system" fn name(...)` and automatically wraps the body statements in an `unsafe {}` block.
  - Both `fn` and `proc` default to `extern "system"` calling convention (or preserved explicit ABI) and `#[no_mangle]`.
  - Functions returning `void` omit the return type arrow `-> ...` in Rust (transpiling to unit `()`).
- **C Primitive Type Mapping**:
  - `void` $\rightarrow$ `core::ffi::c_void` (under raw pointers like `void*` $\rightarrow$ `*mut c_void`).
  - `void` return $\rightarrow$ unit `()` (omitted `-> ...` arrow).
  - `char` $\rightarrow$ `core::ffi::c_char`
  - `signed char` $\rightarrow$ `core::ffi::c_schar`
  - `unsigned char` $\rightarrow$ `core::ffi::c_uchar`
  - `short` $\rightarrow$ `core::ffi::c_short`
  - `unsigned short` $\rightarrow$ `core::ffi::c_ushort`
  - `int` $\rightarrow$ `core::ffi::c_int`
  - `unsigned int` / `unsigned` / `uint` $\rightarrow$ `core::ffi::c_uint`
  - `long` $\rightarrow$ `core::ffi::c_long`
  - `unsigned long` $\rightarrow$ `core::ffi::c_ulong`
  - `long long` $\rightarrow$ `core::ffi::c_longlong`
  - `unsigned long long` $\rightarrow$ `core::ffi::c_ulonglong`
  - `float` $\rightarrow$ `core::ffi::c_float`
  - `double` $\rightarrow$ `core::ffi::c_double`
  - `long double` $\rightarrow$ `core::ffi::c_double`
- **libc Type Mapping**:
  - Aliases like `size_t`, `ssize_t`, `ptrdiff_t`, `uintptr_t`, `intptr_t`, `off_t`, and `pid_t` are transpiled to their `libc::` equivalents.
  - `use libc::*;` is only conditionally imported if one of these libc-specific types is referenced.
- **Postfix Pointer & Reference Syntax (C/C++ Conventions Exclusively)**:
  - `T*` or `T mut*` $\rightarrow$ `*mut T` (Mutable raw pointer)
  - `T const*` $\rightarrow$ `*const T` (Constant raw pointer)
  - `T&` or `T mut&` $\rightarrow$ `&mut T` (Mutable reference / borrow)
  - `T const&` $\rightarrow$ `&T` (Constant reference / borrow)
  - `T**` $\rightarrow$ `*mut *mut T` (Nested pointers)
  - Prefix Rust syntax (`*const T`, `*mut T`, `&T`, `&mut T`) in type positions is disallowed in Carbide.
- **Function Pointer Types (C callbacks)**:
  - Written with `fn` in type position (struct fields, params, returns, aliases): `cb: fn(a: int, b: void*) -> bool`
  - Transpiles to `Option<unsafe extern "system" fn(a: c_int, b: *mut c_void) -> bool>` — nullable C callback, param names preserved.
  - Callbacks returning `void` omit `-> ...`.
- **Type Aliases (C typedefs)**:
  - `type Name = Type;` parses the RHS as a full Carbide type with mapping/pointer flips: `type AudioCallback = fn(buffer: void*, frames: uint) -> void;` $\rightarrow$ `pub type AudioCallback = Option<unsafe extern "system" fn(buffer: *mut c_void, frames: c_uint)>;`
- **Expressions**:
  - Binary multiplication following a type cast (`e1 as usize * e2`) is preserved as arithmetic and not rewritten into a pointer cast.
- **Auto-Repr**:
  - Every struct definition is automatically prepended with `#[repr(C)]`.
- **Standard Library Default & `--no-std` Option**:
  - Standard library mode (`--std`) is the default and does not emit `#![no_std]`.
  - Pass `--no-std` to explicitly emit `#![no_std]` for bare-metal FFI targets.
- **FFI style lints**:
  - Generated files carry `#![allow(non_camel_case_types)]`, `#![allow(non_snake_case)]`, and `#![allow(non_upper_case_globals)]`.
- **Raw items**:
  - `const` / `static` / other unparsed top-level items pass through verbatim **including their terminating `;`**.

### 3. Reference Bindings (fixtures)

- `tests/fixtures/clap_audio.carbide` — CLAP audio plugin ABI (free-audio/clap 1.2): version/descriptor/plugin/host/process/events, params/state/audio-ports/note-ports/log extensions, factory, `clap_entry` static.
- `tests/fixtures/raylib_api.carbide` — raylib API surface: math/colour/texture/camera/audio structs, `AudioCallback` fn-pointer typedef, window/draw/audio procedure stubs (`unimplemented!()` bodies).
- `tests/fixtures/rust_syntax.carbide` — syntax test covering postfix references, pointers, and expressions.

### 4. Testing

- Unit tests live inline in `src/*.rs` (`cargo test`).
- `tests/fixture_tests.rs` transpiles every fixture in `--std`, `--no-std`, and default modes and asserts emitted content.
- `tests/integration_tests.rs` transpiles `ffi_compute`, `clap_audio`, `raylib_api`, `rust_syntax` and compiles the generated Rust with `rustc --crate-type=lib` in both std and `--no-std` modes.
