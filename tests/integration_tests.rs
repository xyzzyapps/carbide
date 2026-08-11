//! Integration test for carbide transpilation and rustc compilation.
//!
//! Reads a .carbide fixture file, transpiles it, verifies the output,
//! and compiles the result with rustc to validate correctness.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

/// Monotonic counter used to generate unique temp file names and avoid
/// race conditions when tests run in parallel.
static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Transpile `fixture` with the carbide binary and compile the generated
/// Rust with rustc as a library. Panics on any failure, printing the
/// compiler diagnostics.
fn transpile_and_compile_with_args(fixture: &Path, expected_snippets: &[&str], args: &[&str]) {
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let carbide_bin = Path::new("target").join("debug").join("carbide.exe");
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stem = fixture.file_stem().unwrap().to_str().unwrap();
    let rs_file = format!("integration_{}_{}.rs", stem, id);
    let lib_out = format!("libintegration_{}_{}.rlib", stem, id);

    // 1. Invoke our carbide transpiler binary on the fixture file
    let mut cmd = Command::new(&carbide_bin);
    cmd.arg(fixture).arg("-o").arg(&rs_file);
    for arg in args {
        cmd.arg(arg);
    }
    let status = cmd.status().expect("Failed to run carbide binary");

    assert!(
        status.success(),
        "carbide transpiler failed for {:?}",
        fixture
    );

    // 2. Verify transpiled file contents
    let transpiled_code = fs::read_to_string(&rs_file).expect("Failed to read transpiled rs file");
    let expects_no_std = args.contains(&"--no-std");
    if expects_no_std {
        assert!(transpiled_code.contains("#![no_std]"), "Missing #![no_std] in no_std mode");
    } else {
        assert!(!transpiled_code.contains("#![no_std]"), "Unexpected #![no_std] in default mode");
    }
    assert!(
        transpiled_code.contains("use core::ffi::*;"),
        "Missing core::ffi import"
    );
    for snippet in expected_snippets {
        assert!(
            transpiled_code.contains(snippet),
            "Missing expected output `{snippet}` in {}:\n{transpiled_code}",
            fixture.display()
        );
    }

    // 3. Run rustc on the generated code to compile as lib
    let rustc_status = Command::new("rustc")
        .arg("--edition=2021")
        .arg("--crate-type=lib")
        .arg(&rs_file)
        .arg("-o")
        .arg(&lib_out)
        .status()
        .expect("Failed to run rustc");

    assert!(
        rustc_status.success(),
        "rustc compilation of transpiled code failed for {:?}",
        fixture
    );

    // Clean up temporary files
    let _ = fs::remove_file(&rs_file);
    let _ = fs::remove_file(&lib_out);
}

fn transpile_and_compile(fixture: &Path, expected_snippets: &[&str]) {
    transpile_and_compile_with_args(fixture, expected_snippets, &[]);
}

#[test]
fn test_integration_transpile_and_compile() {
    transpile_and_compile(
        Path::new("tests/fixtures/ffi_compute.carbide"),
        &[
            "#[repr(C)]",
            "pub struct FfiStruct",
            "pub val: c_int",
            "pub ptr: *mut c_int",
            "#[no_mangle]",
            "pub unsafe extern \"system\" fn compute(s: *const FfiStruct) -> c_int",
        ],
    );
}

#[test]
fn test_integration_clap_audio_compiles() {
    transpile_and_compile(
        Path::new("tests/fixtures/clap_audio.carbide"),
        &[
            // Type aliases (C typedefs)
            "pub type clap_id = u32;",
            "pub type clap_process_status = i32;",
            // Version constants keep their semicolons
            "const CLAP_VERSION_MAJOR: u32 = 1;",
            // Function pointer fields → nullable C fn pointers
            "pub init: Option<unsafe extern \"system\" fn(plugin: *const clap_plugin) -> bool>,",
            // Nested const pointer
            "pub features: *const *const c_char,",
            // Entry point static with struct literal
            "pub static clap_entry: clap_plugin_entry = clap_plugin_entry {",
            "    get_factory: None\n};",
            // Helpers
            "pub extern \"system\" fn clap_version_is_compatible",
            "pub unsafe extern \"system\" fn plugin_has_init",
        ],
    );
}

#[test]
fn test_integration_raylib_api_compiles() {
    transpile_and_compile(
        Path::new("tests/fixtures/raylib_api.carbide"),
        &[
            // C typedef aliases
            "pub type Texture2D = Texture;",
            "pub type AudioCallback = Option<unsafe extern \"system\" fn(buffer: *mut c_void, frames: c_uint)>;",
            // Struct layout mapping
            "pub struct Color",
            "pub r: c_uchar,",
            "pub recs: *mut Rectangle,",
            // Opaque handles
            "pub struct rAudioBuffer {",
            // Stub procedures with real signatures
            "pub unsafe extern \"system\" fn InitWindow(width: c_int, height: c_int, title: *const c_char)",
            "pub unsafe extern \"system\" fn LoadSound(file_name: *const c_char) -> Sound",
            "pub unsafe extern \"system\" fn SetAudioStreamCallback(stream: AudioStream, callback: AudioCallback)",
            // Helpers with real bodies
            "pub extern \"system\" fn raylib_color",
        ],
    );
}

#[test]
fn test_integration_rust_syntax_compiles() {
    transpile_and_compile(
        Path::new("tests/fixtures/rust_syntax.carbide"),
        &[
            "pub unsafe extern \"system\" fn shift",
            "pub unsafe extern \"system\" fn test_rust_syntax",
            "let prod: usize = e1 as usize * e2;",
        ],
    );
}

#[test]
fn test_integration_no_std_flag_compiles() {
    transpile_and_compile_with_args(
        Path::new("tests/fixtures/ffi_compute.carbide"),
        &[
            "#![no_std]",
            "pub unsafe extern \"system\" fn compute",
        ],
        &["--no-std"],
    );
}
