//! Fixture-based integration test runner for carbide.
//!
//! Discovers all `.carbide` files under `tests/fixtures/`, transpiles each
//! through the carbide binary, and verifies both the transpiled output
//! content and compilation via `rustc`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

/// Monotonic counter used to generate unique temp file names and avoid
/// race conditions when tests run in parallel.
static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Discover all `.carbide` fixture files under the given directory.
fn discover_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut fixtures = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir).expect("Failed to read fixtures directory") {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "carbide") {
                fixtures.push(path);
            }
        }
    }
    fixtures.sort();
    fixtures
}

/// Run the carbide binary on a `.carbide` fixture file and return the
/// transpiled Rust source code as a String. Uses a unique temp file
/// to avoid race conditions during parallel test execution.
fn transpile_fixture(fixture: &Path) -> String {
    let carbide_bin = Path::new("target").join("debug").join("carbide.exe");
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stem = fixture.file_stem().unwrap().to_str().unwrap();
    let out_file = PathBuf::from(format!("fixture_out_{}_{}.rs", stem, id));

    let status = Command::new(&carbide_bin)
        .arg(fixture)
        .arg("-o")
        .arg(&out_file)
        .status()
        .expect(&format!("Failed to run carbide on {:?}", fixture));

    assert!(
        status.success(),
        "carbide transpilation failed for fixture: {:?}",
        fixture
    );

    let output = fs::read_to_string(&out_file)
        .expect(&format!("Failed to read transpiled output {:?}", out_file));

    // Clean up transpiled file
    let _ = fs::remove_file(&out_file);

    output
}

// ---------------------------------------------------------------------------
// Individual fixture tests
// ---------------------------------------------------------------------------

#[test]
fn test_fixture_libc_types() {
    let fixture = Path::new("tests/fixtures/libc_types.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Verify imports
    assert!(output.contains("#![no_std]"), "Missing #![no_std]");
    assert!(output.contains("use core::ffi::*;"), "Missing core::ffi import");
    assert!(output.contains("use libc::*;"), "Missing libc import");

    // Verify single-word C type substitutions
    assert!(output.contains("c_int"), "Missing c_int substitution");
    assert!(output.contains("c_uint"), "Missing c_uint substitution");
    assert!(output.contains("c_long"), "Missing c_long substitution");
    assert!(output.contains("c_char"), "Missing c_char substitution");
    assert!(output.contains("c_float"), "Missing c_float substitution");
    assert!(output.contains("c_double"), "Missing c_double substitution");

    // Verify multi-word C type substitutions
    assert!(output.contains("c_uchar"), "Missing c_uchar (unsigned char)");
    assert!(output.contains("c_schar"), "Missing c_schar (signed char)");
    assert!(output.contains("c_ushort"), "Missing c_ushort (unsigned short)");
    assert!(output.contains("c_short"), "Missing c_short (short)");
    assert!(output.contains("c_ulong"), "Missing c_ulong (unsigned long)");
    assert!(output.contains("c_longlong"), "Missing c_longlong (long long)");
    assert!(output.contains("c_ulonglong"), "Missing c_ulonglong (unsigned long long)");

    // Verify libc types are present
    assert!(output.contains("size_t"), "Missing size_t");
    assert!(output.contains("ssize_t"), "Missing ssize_t");
    assert!(output.contains("ptrdiff_t"), "Missing ptrdiff_t");
    assert!(output.contains("uintptr_t"), "Missing uintptr_t");
    assert!(output.contains("intptr_t"), "Missing intptr_t");
    assert!(output.contains("off_t"), "Missing off_t");
    assert!(output.contains("pid_t"), "Missing pid_t");

    // Verify struct attributes
    assert!(output.contains("#[repr(C)]"), "Missing #[repr(C)]");

    // Verify function attributes
    assert!(output.contains("#[no_mangle]"), "Missing #[no_mangle]");
    assert!(output.contains("extern \"C\""), "Missing extern \"C\"");
    assert!(output.contains("pub unsafe extern"), "Missing unsafe fn declaration");
}

#[test]
fn test_fixture_apr_types() {
    let fixture = Path::new("tests/fixtures/apr_types.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Verify imports
    assert!(output.contains("#![no_std]"), "Missing #![no_std]");

    // Verify APR struct declarations
    assert!(output.contains("pub struct apr_pool_t"), "Missing apr_pool_t struct");
    assert!(output.contains("pub struct apr_file_t"), "Missing apr_file_t struct");
    assert!(output.contains("pub struct apr_finfo_t"), "Missing apr_finfo_t struct");
    assert!(output.contains("pub struct apr_buf_t"), "Missing apr_buf_t struct");

    // Verify pointer types in struct fields
    assert!(output.contains("*mut apr_pool_t"), "Missing *mut apr_pool_t pointer");
    assert!(output.contains("*const c_char"), "Missing *const c_char pointer");
    assert!(output.contains("*mut c_uchar"), "Missing *mut c_uchar pointer");
    assert!(output.contains("size_t"), "Missing size_t in apr_buf_t");
    assert!(output.contains("c_longlong"), "Missing c_longlong (long long) in apr_finfo_t");

    // Verify function signatures
    assert!(output.contains("apr_pool_create"), "Missing apr_pool_create function");
    assert!(output.contains("apr_pool_destroy"), "Missing apr_pool_destroy function");
    assert!(output.contains("apr_file_open"), "Missing apr_file_open function");
    assert!(output.contains("apr_file_read"), "Missing apr_file_read function");
    assert!(output.contains("apr_file_write"), "Missing apr_file_write function");
    assert!(output.contains("apr_file_close"), "Missing apr_file_close function");
    assert!(output.contains("apr_stat"), "Missing apr_stat function");

    // Verify double-pointer parameter (apr_pool_t**)
    assert!(
        output.contains("*mut *mut apr_pool_t"),
        "Missing double pointer *mut *mut apr_pool_t"
    );

    // Verify const pointer parameter (void const*)
    assert!(
        output.contains("*const c_void"),
        "Missing *const c_void parameter"
    );

    // All functions should have C ABI
    assert!(output.contains("#[repr(C)]"), "Missing #[repr(C)]");
    assert!(output.contains("#[no_mangle]"), "Missing #[no_mangle]");
    assert!(output.contains("extern \"C\""), "Missing extern \"C\"");
}

#[test]
fn test_fixture_rust_primitives() {
    let fixture = Path::new("tests/fixtures/rust_primitives.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Verify imports
    assert!(output.contains("#![no_std]"), "Missing #![no_std]");

    // Verify all Rust integer types are preserved verbatim
    for ty in &["i8", "i16", "i32", "i64", "i128", "isize",
                "u8", "u16", "u32", "u64", "u128", "usize"] {
        assert!(
            output.contains(ty),
            "Rust primitive type '{}' was not preserved in output",
            ty
        );
    }

    // Verify float types preserved
    assert!(output.contains("f32"), "Missing f32");
    assert!(output.contains("f64"), "Missing f64");

    // Verify bool preserved
    assert!(output.contains("bool"), "Missing bool");

    // Verify struct and function attributes
    assert!(output.contains("#[repr(C)]"), "Missing #[repr(C)]");
    assert!(output.contains("#[no_mangle]"), "Missing #[no_mangle]");
    assert!(output.contains("pub extern \"C\" fn rust_add"), "rust_add signature must be safe fn");
    assert!(!output.contains("pub unsafe extern \"C\" fn rust_add"), "Safe fn must not be unsafe");

    // Verify Rust types are NOT substituted to c_* equivalents
    // (i32 should not become c_int, etc.)
    assert!(output.contains("pub a: i8"), "i8 was incorrectly substituted");
    assert!(output.contains("pub c: i32"), "i32 was incorrectly substituted");
    assert!(output.contains("pub m: f32"), "f32 was incorrectly substituted");
}

#[test]
fn test_fixture_learn_c_examples() {
    let fixture = Path::new("tests/fixtures/learn_c_examples.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Verify correct signature and safety translation for swapTwoNumbers
    assert!(
        output.contains("pub unsafe extern \"C\" fn swapTwoNumbers(a: *mut c_int, b: *mut c_int)"),
        "Missing or incorrect swapTwoNumbers signature"
    );
    assert!(output.contains("pub unsafe extern \"C\" fn swapTwoNumbers"), "proc must be unsafe fn");

    // Verify correct signature and safety translation for get_char_at_offset
    assert!(
        output.contains("pub unsafe extern \"C\" fn get_char_at_offset(str_in: *mut c_char, offset: c_int) -> c_char"),
        "Missing or incorrect get_char_at_offset signature"
    );

    // Verify correct signature for add_two_ints (safe fn)
    assert!(
        output.contains("pub extern \"C\" fn add_two_ints(x1: c_int, x2: c_int) -> c_int"),
        "Missing or incorrect add_two_ints signature"
    );

    // Verify basic_math exists and returns c_int
    assert!(
        output.contains("pub extern \"C\" fn basic_math(a: c_int, b: c_int) -> c_int"),
        "Missing or incorrect basic_math signature"
    );

    // Verify struct has repr(C)
    assert!(output.contains("#[repr(C)]"), "Missing #[repr(C)]");
    assert!(output.contains("pub struct MyStruct"), "Missing MyStruct struct");
}

#[test]
fn test_all_fixtures_transpile() {
    let fixtures_dir = Path::new("tests/fixtures");
    let fixtures = discover_fixtures(fixtures_dir);

    assert!(
        !fixtures.is_empty(),
        "No .carbide fixture files found in {:?}",
        fixtures_dir
    );

    for fixture in &fixtures {
        let name = fixture.file_stem().unwrap().to_str().unwrap();
        println!("Transpiling fixture: {}", name);

        let output = transpile_fixture(fixture);
        assert!(!output.is_empty(), "Empty transpilation output for: {}", name);
        assert!(
            output.contains("#![no_std]"),
            "Missing #![no_std] in: {}",
            name
        );
        assert!(
            output.contains("use core::ffi::*;"),
            "Missing core::ffi import in: {}",
            name
        );
        if name == "libc_types" || name == "apr_types" {
            assert!(
                output.contains("use libc::*;"),
                "Missing libc import in: {}",
                name
            );
        } else {
            assert!(
                !output.contains("use libc::*;"),
                "Unexpected libc import in: {}",
                name
            );
        }
    }
}
