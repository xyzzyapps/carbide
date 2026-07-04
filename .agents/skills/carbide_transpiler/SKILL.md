---
name: carbide_transpiler
description: Reference guide for working with the Carbide dialect, maintaining the transpiler codebase, and using Carbide CLI/Cargo drivers.
---

## Carbide Dialect Overview

Carbide is a low-level dialect of Rust designed for seamless C ABI compatibility. It maps C-style primitive type keywords, postfix pointers, and attributes to standard FFI-compliant Rust code.

### 1. Naming & File Extensions
- **Dialect Name**: Carbide
- **File Extension**: `.carbide`
- **Compiler Binary**: `carbide` (runs as `carbide <FILE> [-o <OUT>] [-c]`)
- **Cargo Command**: `cargo-carbide` (runs as `cargo carbide build`)

### 2. Syntax Rules
- **Function/Procedure Declarations**:
  - `fn name(...)`: Declares a **safe** function by default. Transpiles to safe `pub extern "C" fn name(...)` without implicit unsafe wrapping.
  - `proc name(...)`: Declares an **unsafe** procedure by default. Transpiles to `pub unsafe extern "C" fn name(...)` and automatically wraps the body statements in an `unsafe {}` block.
  - Both `fn` and `proc` are automatically injected with the C calling convention (`extern "C"`) and `#[no_mangle]`.
- **C Primitive Type Mapping**:
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
- **libc Type Mapping**:
  - Aliases like `size_t`, `ssize_t`, `ptrdiff_t`, `uintptr_t`, `intptr_t`, `off_t`, and `pid_t` are transpiled to their `libc::` equivalents.
  - `use libc::*;` is only conditionally imported if one of these libc-specific types is referenced.
- **Postfix Pointer Syntax**:
  - `T*` $\rightarrow$ `*mut T` (Mutable raw pointer)
  - `T const*` $\rightarrow$ `*const T` (Constant raw pointer)
  - `T**` $\rightarrow$ `*mut *mut T` (Nested pointers)
- **Auto-Repr**:
  - Every struct definition is automatically prepended with `#[repr(C)]`.
- **Bare-Metal Attribute**:
  - Every transpiled file has `#![no_std]` prepended to ensure bare-metal FFI compatibility.
