---
name: carbide_transpiler
description: Reference guide for working with the Carbide dialect, maintaining the transpiler codebase, and using Carbide CLI/Cargo drivers.
---

## Carbide Dialect Overview

Carbide is a low-level dialect of Rust designed for seamless C/C++ ABI compatibility. It maps C-style primitive type keywords, postfix pointers (`*`) and references (`&`), C atomics, function-pointer callbacks, and attributes to standard FFI-compliant Rust code.

### 1. Naming & File Extensions
- **Dialect Name**: Carbide
- **File Extension**: `.carbide`
- **Compiler Binary**: `carbide` (runs as `carbide <FILE> [-o <OUT>] [-c] [--std] [--no-std]`)
- **Cargo Command**: `cargo-carbide` (runs as `cargo carbide build [--no-std]`)

### 2. Syntax Rules
- **Function/Procedure Declarations**:
  - `fn name(...)`: Declares a **safe** function by default. Top-level free functions transpile to `#[no_mangle] pub extern "system" fn name(...)`.
  - `proc name(...)`: Declares an **unsafe** procedure by default. Top-level free procedures transpile to `#[no_mangle] pub unsafe extern "system" fn name(...)` and automatically wrap body statements in an `unsafe {}` block.
  - `impl` methods emit standard Rust methods (`pub fn` or `pub unsafe fn`) without `#[no_mangle]` to avoid global symbol collisions.
  - Functions returning `void` omit the return type arrow `-> ...` in Rust (transpiling to unit `()`).
- **C Primitive Type Mapping**:
  - `void` $\rightarrow$ `core::ffi::c_void` (under raw pointers like `void*` $\rightarrow$ `*mut c_void`).
  - `void` return $\rightarrow$ unit `()` (omitted `-> ...` arrow).
  - `char` $\rightarrow$ `core::ffi::c_char` (and `char*` $\rightarrow$ `*mut c_char`).
  - `signed char` $\rightarrow$ `core::ffi::c_schar`, `unsigned char` $\rightarrow$ `core::ffi::c_uchar`.
  - `short` $\rightarrow$ `core::ffi::c_short`, `unsigned short` $\rightarrow$ `core::ffi::c_ushort`.
  - `int` $\rightarrow$ `core::ffi::c_int`, `unsigned int` / `unsigned` / `uint` $\rightarrow$ `core::ffi::c_uint`.
  - `long` $\rightarrow$ `core::ffi::c_long`, `unsigned long` $\rightarrow$ `core::ffi::c_ulong`.
  - `long long` $\rightarrow$ `core::ffi::c_longlong`, `unsigned long long` $\rightarrow$ `core::ffi::c_ulonglong`.
  - `float` $\rightarrow$ `core::ffi::c_float`, `double` / `long double` $\rightarrow$ `core::ffi::c_double`.
- **C/C++ Atomics & libc Types**:
  - `atomic_bool` $\rightarrow$ `core::sync::atomic::AtomicBool`
  - `atomic_int` $\rightarrow$ `core::sync::atomic::AtomicI32`, `atomic_uint` $\rightarrow$ `core::sync::atomic::AtomicU32`
  - `atomic_long` $\rightarrow$ `core::sync::atomic::AtomicI64`, `atomic_ulong` $\rightarrow$ `core::sync::atomic::AtomicU64`
  - `atomic_size_t` / `atomic_uintptr_t` $\rightarrow$ `core::sync::atomic::AtomicUsize`
  - `atomic_intptr_t` $\rightarrow$ `core::sync::atomic::AtomicIsize`
  - `use core::sync::atomic::*;` is automatically imported when atomic types or `Ordering` are used.
  - `use libc::*;` is conditionally imported for libc types (`size_t`, `off_t`, etc.).
- **Postfix Pointer & Reference Syntax (C/C++ Conventions Exclusively)**:
  - `T*` or `T mut*` $\rightarrow$ `*mut T` (Mutable raw pointer)
  - `T const*` $\rightarrow$ `*const T` (Constant raw pointer)
  - `T&` or `T mut&` $\rightarrow$ `&mut T` (Mutable reference / borrow)
  - `T const&` $\rightarrow$ `&T` (Constant reference / borrow)
  - `T**` $\rightarrow$ `*mut *mut T` (Nested pointers)
  - In expressions: `mut& num` $\rightarrow$ `&mut num` (mutable borrow).
  - Prefix syntax (`*const T`, `*mut T`, `&T`, `&mut T`, `const T*`, `const T&`) in type positions is disallowed.
- **Function Pointer Types (C callbacks)**:
  - `cb: fn(a: int, b: void*) -> bool` $\rightarrow$ `Option<unsafe extern "system" fn(a: c_int, b: *mut c_void) -> bool>` (nullable C callback).
- **Auto-Repr**:
  - Struct definitions automatically get `#[repr(C)]`.
- **Standard Library Default & `--no-std` Option**:
  - Default mode (`--std`) omits `#![no_std]`.
  - Pass `--no-std` to explicitly emit `#![no_std]` for bare-metal targets.

### 3. Reference Bindings & Fixtures
- `tests/fixtures/atomics_operators.carbide` — Atomics, closures, operators, and `impl` methods.
- `tests/fixtures/clap_audio.carbide` — CLAP audio plugin ABI (free-audio/clap 1.2).
- `tests/fixtures/raylib_api.carbide` — raylib API surface.
- `tests/fixtures/rust_syntax.carbide` — Syntax, expressions, and postfix references.
