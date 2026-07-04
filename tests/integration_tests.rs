//! Integration test for carbide transpilation and rustc compilation.
//!
//! Reads a .carbide fixture file, transpiles it, verifies the output,
//! and compiles the result with rustc to validate correctness.

use std::fs;
use std::process::Command;
use std::path::Path;

#[test]
fn test_integration_transpile_and_compile() {
    let fixture = Path::new("tests/fixtures/ffi_compute.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let rs_file = "integration_test_temp.rs";

    // 1. Invoke our carbide transpiler binary on the fixture file
    let carbide_bin = Path::new("target").join("debug").join("carbide.exe");
    let status = Command::new(&carbide_bin)
        .arg(fixture)
        .arg("-o")
        .arg(rs_file)
        .status()
        .expect("Failed to run carbide binary");

    assert!(status.success(), "carbide transpiler failed");

    // 2. Verify transpiled file contents
    let transpiled_code = fs::read_to_string(rs_file).expect("Failed to read transpiled rs file");
    assert!(transpiled_code.contains("#![no_std]"));
    assert!(transpiled_code.contains("use core::ffi::*;"));
    assert!(!transpiled_code.contains("use libc::*;"));
    assert!(transpiled_code.contains("#[repr(C)]"));
    assert!(transpiled_code.contains("pub struct FfiStruct"));
    assert!(transpiled_code.contains("pub val: c_int"));
    assert!(transpiled_code.contains("pub ptr: *mut c_int"));
    assert!(transpiled_code.contains("#[no_mangle]"));
    assert!(transpiled_code.contains("pub unsafe extern \"C\" fn compute(s: *const FfiStruct) -> c_int"));
    assert!(transpiled_code.contains("unsafe {"));

    // 3. Run rustc on the generated code to compile as lib
    let lib_out = "libintegration_test.rlib";
    let rustc_status = Command::new("rustc")
        .arg("--edition=2021")
        .arg("--crate-type=lib")
        .arg(rs_file)
        .arg("-o")
        .arg(lib_out)
        .status()
        .expect("Failed to run rustc");

    assert!(rustc_status.success(), "rustc compilation of transpiled code failed");

    // Clean up temporary files
    let _ = fs::remove_file(rs_file);
    let _ = fs::remove_file(lib_out);
}
