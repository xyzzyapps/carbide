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
            "pub unsafe fn shift",
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

#[test]
fn test_integration_atomics_operators_compiles() {
    transpile_and_compile(
        Path::new("tests/fixtures/atomics_operators.carbide"),
        &[
            "use core::sync::atomic::*;",
            "pub val: AtomicI32,",
            "pub active: AtomicBool,",
            "pub total: AtomicUsize,",
            "pub fn new(init: c_int) -> Counter",
            "pub fn increment(self: &mut Counter, step: c_int) -> c_int",
            "pub unsafe extern \"system\" fn test_operators_and_closures",
        ],
    );
}

#[test]
fn test_compile_cdylib_dll() {
    let fixture = Path::new("tests/fixtures/ffi_compute.carbide");
    let carbide_bin = Path::new("target").join("debug").join("carbide.exe");
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dll_out = format!("target/test_output_{}.dll", id);

    let status = Command::new(&carbide_bin)
        .arg(fixture)
        .arg("--dll")
        .arg("-o")
        .arg(&dll_out)
        .status()
        .expect("Failed to run carbide binary");

    assert!(status.success(), "carbide --dll compilation failed");
    let out_path = Path::new(&dll_out);
    assert!(out_path.exists(), "Expected DLL output file {:?}", dll_out);
    let _ = fs::remove_file(out_path);
}

#[test]
fn test_compile_staticlib() {
    let fixture = Path::new("tests/fixtures/ffi_compute.carbide");
    let carbide_bin = Path::new("target").join("debug").join("carbide.exe");
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let lib_out = format!("target/test_output_{}.lib", id);

    let status = Command::new(&carbide_bin)
        .arg(fixture)
        .arg("--static")
        .arg("-o")
        .arg(&lib_out)
        .status()
        .expect("Failed to run carbide binary");

    assert!(status.success(), "carbide --static compilation failed");
    let out_path = Path::new(&lib_out);
    assert!(out_path.exists(), "Expected static lib output file {:?}", lib_out);
    let _ = fs::remove_file(out_path);
}

#[test]
fn test_compile_executable() {
    let carbide_bin = Path::new("target").join("debug").join("carbide.exe");
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let src_file = format!("target/test_app_{}.carbide", id);
    let exe_out = format!("target/test_app_{}.exe", id);

    let app_src = "fn main() { let x: int = 10; }";
    fs::write(&src_file, app_src).expect("Failed to write app carbide file");

    let status = Command::new(&carbide_bin)
        .arg(&src_file)
        .arg("--exe")
        .arg("-o")
        .arg(&exe_out)
        .status()
        .expect("Failed to run carbide binary");

    assert!(status.success(), "carbide --exe compilation failed");
    let exe_path = Path::new(&exe_out);
    assert!(exe_path.exists(), "Expected exe output file {:?}", exe_out);

    // Run the generated executable
    let run_status = Command::new(exe_path)
        .status()
        .expect("Failed to run generated executable");
    assert!(run_status.success(), "Generated executable failed with {:?}", run_status);

    let _ = fs::remove_file(&src_file);
    let _ = fs::remove_file(format!("target/test_app_{}.rs", id));
    let _ = fs::remove_file(exe_path);
    let _ = fs::remove_file(format!("target/test_app_{}.pdb", id));
}
