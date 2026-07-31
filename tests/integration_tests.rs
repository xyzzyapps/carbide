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
fn transpile_and_compile(fixture: &Path, expected_snippets: &[&str]) {
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let carbide_bin = Path::new("target").join("debug").join("carbide.exe");
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stem = fixture.file_stem().unwrap().to_str().unwrap();
    let rs_file = format!("integration_{}_{}.rs", stem, id);
    let lib_out = format!("libintegration_{}_{}.rlib", stem, id);

    // 1. Invoke our carbide transpiler binary on the fixture file
    let status = Command::new(&carbide_bin)
        .arg(fixture)
        .arg("-o")
        .arg(&rs_file)
        .status()
        .expect("Failed to run carbide binary");

    assert!(
        status.success(),
        "carbide transpiler failed for {:?}",
        fixture
    );

    // 2. Verify transpiled file contents
    let transpiled_code = fs::read_to_string(&rs_file).expect("Failed to read transpiled rs file");
    assert!(transpiled_code.contains("#![no_std]"), "Missing #![no_std]");
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
            "pub unsafe extern \"C\" fn compute(s: *const FfiStruct) -> c_int",
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
            "pub init: Option<unsafe extern \"C\" fn(plugin: *const clap_plugin) -> bool>,",
            // Nested const pointer
            "pub features: *const *const c_char,",
            // Entry point static with struct literal
            "pub static clap_entry: clap_plugin_entry = clap_plugin_entry {",
            "    get_factory: None\n};",
            // Helpers
            "pub extern \"C\" fn clap_version_is_compatible",
            "pub unsafe extern \"C\" fn plugin_has_init",
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
            "pub type AudioCallback = Option<unsafe extern \"C\" fn(buffer: *mut c_void, frames: c_uint) -> c_void>;",
            // Struct layout mapping
            "pub struct Color",
            "pub r: c_uchar,",
            "pub recs: *mut Rectangle,",
            // Opaque handles
            "pub struct rAudioBuffer {",
            // Stub procedures with real signatures
            "pub unsafe extern \"C\" fn InitWindow(width: c_int, height: c_int, title: *const c_char) -> c_void",
            "pub unsafe extern \"C\" fn LoadSound(file_name: *const c_char) -> Sound",
            "pub unsafe extern \"C\" fn SetAudioStreamCallback(stream: AudioStream, callback: AudioCallback) -> c_void",
            // Helpers with real bodies
            "pub extern \"C\" fn raylib_color",
        ],
    );
}
