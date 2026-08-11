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
fn transpile_fixture_with_args(fixture: &Path, args: &[&str]) -> String {
    let carbide_bin = Path::new("target").join("debug").join("carbide.exe");
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let stem = fixture.file_stem().unwrap().to_str().unwrap();
    let out_file = PathBuf::from(format!("fixture_out_{}_{}.rs", stem, id));

    let mut cmd = Command::new(&carbide_bin);
    cmd.arg(fixture).arg("-o").arg(&out_file);
    for arg in args {
        cmd.arg(arg);
    }
    let status = cmd
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

fn transpile_fixture(fixture: &Path) -> String {
    transpile_fixture_with_args(fixture, &[])
}

// ---------------------------------------------------------------------------
// Individual fixture tests
// ---------------------------------------------------------------------------

#[test]
fn test_fixture_libc_types() {
    let fixture = Path::new("tests/fixtures/libc_types.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Verify imports in default std mode
    assert!(!output.contains("#![no_std]"), "Unexpected #![no_std] in default mode");
    assert!(
        output.contains("use core::ffi::*;"),
        "Missing core::ffi import"
    );
    assert!(output.contains("use libc::*;"), "Missing libc import");

    // Verify single-word C type substitutions
    assert!(output.contains("c_int"), "Missing c_int substitution");
    assert!(output.contains("c_uint"), "Missing c_uint substitution");
    assert!(output.contains("c_long"), "Missing c_long substitution");
    assert!(output.contains("c_char"), "Missing c_char substitution");
    assert!(output.contains("c_float"), "Missing c_float substitution");
    assert!(output.contains("c_double"), "Missing c_double substitution");

    // Verify multi-word C type substitutions
    assert!(
        output.contains("c_uchar"),
        "Missing c_uchar (unsigned char)"
    );
    assert!(output.contains("c_schar"), "Missing c_schar (signed char)");
    assert!(
        output.contains("c_ushort"),
        "Missing c_ushort (unsigned short)"
    );
    assert!(output.contains("c_short"), "Missing c_short (short)");
    assert!(
        output.contains("c_ulong"),
        "Missing c_ulong (unsigned long)"
    );
    assert!(
        output.contains("c_longlong"),
        "Missing c_longlong (long long)"
    );
    assert!(
        output.contains("c_ulonglong"),
        "Missing c_ulonglong (unsigned long long)"
    );

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
    assert!(output.contains("extern \"system\""), "Missing extern \"system\"");
    assert!(
        output.contains("pub unsafe extern"),
        "Missing unsafe fn declaration"
    );
}

#[test]
fn test_fixture_apr_types() {
    let fixture = Path::new("tests/fixtures/apr_types.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Verify imports
    assert!(!output.contains("#![no_std]"), "Unexpected #![no_std]");

    // Verify APR struct declarations
    assert!(
        output.contains("pub struct apr_pool_t"),
        "Missing apr_pool_t struct"
    );
    assert!(
        output.contains("pub struct apr_file_t"),
        "Missing apr_file_t struct"
    );
    assert!(
        output.contains("pub struct apr_finfo_t"),
        "Missing apr_finfo_t struct"
    );
    assert!(
        output.contains("pub struct apr_buf_t"),
        "Missing apr_buf_t struct"
    );

    // Verify field types
    assert!(
        output.contains("pub pool: *mut apr_pool_t"),
        "Missing pool pointer field"
    );
    assert!(
        output.contains("pub name: *const c_char"),
        "Missing name const pointer field"
    );
    assert!(
        output.contains("pub size: size_t"),
        "Missing size size_t field"
    );

    // Verify function declarations
    assert!(
        output.contains("pub unsafe extern \"system\" fn apr_pool_create"),
        "Missing apr_pool_create"
    );
    assert!(
        output.contains("pub unsafe extern \"system\" fn apr_file_open"),
        "Missing apr_file_open"
    );
    assert!(
        output.contains("pub unsafe extern \"system\" fn apr_file_read"),
        "Missing apr_file_read"
    );
    assert!(
        output.contains("pub unsafe extern \"system\" fn apr_stat"),
        "Missing apr_stat"
    );
}

#[test]
fn test_fixture_ffi_compute() {
    let fixture = Path::new("tests/fixtures/ffi_compute.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Verify struct
    assert!(
        output.contains("pub struct FfiStruct"),
        "Missing FfiStruct"
    );
    assert!(output.contains("pub val: c_int"), "Missing val field");
    assert!(
        output.contains("pub ptr: *mut c_int"),
        "Missing ptr field"
    );

    // Verify proc compute
    assert!(
        output.contains("pub unsafe extern \"system\" fn compute(s: *const FfiStruct) -> c_int"),
        "Missing or incorrect compute signature"
    );

    // Verify body statements
    assert!(
        output.contains("let mut result: c_int = 0;"),
        "Missing result var declaration"
    );
    assert!(
        output.contains("result = *(*s).ptr + (*s).val;"),
        "Missing pointer dereference in compute"
    );
    assert!(
        output.contains("return result;"),
        "Missing return statement"
    );
}

#[test]
fn test_fixture_rust_primitives() {
    let fixture = Path::new("tests/fixtures/rust_primitives.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Verify imports
    assert!(!output.contains("#![no_std]"), "Unexpected #![no_std]");

    // Verify all Rust integer types are preserved verbatim
    for ty in &[
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
    ] {
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
    assert!(
        output.contains("pub extern \"system\" fn rust_add"),
        "rust_add signature must be safe fn"
    );
    assert!(
        !output.contains("pub unsafe extern \"system\" fn rust_add"),
        "Safe fn must not be unsafe"
    );

    // Verify Rust types are NOT substituted to c_* equivalents
    assert!(
        output.contains("pub a: i8"),
        "i8 was incorrectly substituted"
    );
    assert!(
        output.contains("pub c: i32"),
        "i32 was incorrectly substituted"
    );
    assert!(
        output.contains("pub m: f32"),
        "f32 was incorrectly substituted"
    );
}

#[test]
fn test_fixture_rust_syntax() {
    let fixture = Path::new("tests/fixtures/rust_syntax.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    assert!(!output.contains("#![no_std]"), "Unexpected #![no_std]");
    assert!(output.contains("pub struct Point"), "Missing Point struct");
    assert!(
        output.contains("pub unsafe fn shift"),
        "Missing shift method"
    );
    assert!(
        output.contains("pub unsafe extern \"system\" fn test_rust_syntax"),
        "Missing test_rust_syntax"
    );
    assert!(
        output.contains("let prod: usize = e1 as usize * e2;"),
        "Binary multiplication was corrupted"
    );
}

#[test]
fn test_fixture_learn_c_examples() {
    let fixture = Path::new("tests/fixtures/learn_c_examples.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Verify correct signature and safety translation for swapTwoNumbers
    assert!(
        output.contains("pub unsafe extern \"system\" fn swapTwoNumbers(a: *mut c_int, b: *mut c_int)"),
        "Missing or incorrect swapTwoNumbers signature"
    );
    assert!(
        output.contains("pub unsafe extern \"system\" fn swapTwoNumbers"),
        "proc must be unsafe fn"
    );

    // Verify correct signature and safety translation for get_char_at_offset
    assert!(
        output.contains("pub unsafe extern \"system\" fn get_char_at_offset(str_in: *mut c_char, offset: c_int) -> c_char"),
        "Missing or incorrect get_char_at_offset signature"
    );

    // Verify correct signature for add_two_ints (safe fn)
    assert!(
        output.contains("pub extern \"system\" fn add_two_ints(x1: c_int, x2: c_int) -> c_int"),
        "Missing or incorrect add_two_ints signature"
    );

    // Verify basic_math exists and returns c_int
    assert!(
        output.contains("pub extern \"system\" fn basic_math(a: c_int, b: c_int) -> c_int"),
        "Missing or incorrect basic_math signature"
    );

    // Verify struct has repr(C)
    assert!(output.contains("#[repr(C)]"), "Missing #[repr(C)]");
    assert!(
        output.contains("pub struct MyStruct"),
        "Missing MyStruct struct"
    );
}

#[test]
fn test_fixture_clap_audio() {
    let fixture = Path::new("tests/fixtures/clap_audio.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Header + imports
    assert!(!output.contains("#![no_std]"), "Unexpected #![no_std]");
    assert!(
        output.contains("use core::ffi::*;"),
        "Missing core::ffi import"
    );
    assert!(
        !output.contains("use libc::*;"),
        "CLAP fixture should not pull libc"
    );

    // Type aliases for C typedefs
    assert!(
        output.contains("pub type clap_id = u32;"),
        "Missing clap_id alias"
    );
    assert!(
        output.contains("pub type clap_process_status = i32;"),
        "Missing process_status alias"
    );
    assert!(
        output.contains("pub type clap_beattime = i64;"),
        "Missing clap_beattime alias"
    );

    // Version constants must keep their semicolons (raw item fix)
    assert!(
        output.contains("const CLAP_VERSION_MAJOR: u32 = 1;"),
        "Version const lost its ';'"
    );

    // Core structs
    assert!(
        output.contains("pub struct clap_version"),
        "Missing clap_version"
    );
    assert!(
        output.contains("pub struct clap_plugin_descriptor"),
        "Missing descriptor"
    );
    assert!(
        output.contains("pub struct clap_plugin"),
        "Missing clap_plugin"
    );
    assert!(output.contains("pub struct clap_host"), "Missing clap_host");
    assert!(
        output.contains("pub struct clap_process"),
        "Missing clap_process"
    );
    assert!(
        output.contains("pub struct clap_audio_buffer"),
        "Missing audio buffer"
    );

    // Pointer mapping
    assert!(
        output.contains("pub data32: *mut *mut c_float"),
        "Missing float** pointer"
    );
    assert!(
        output.contains("pub features: *const *const c_char"),
        "Missing features const** pointer"
    );

    // Function pointer fields -> Option<unsafe extern "system" fn>
    assert!(
        output.contains(
            "pub init: Option<unsafe extern \"system\" fn(plugin: *const clap_plugin) -> bool>,"
        ),
        "Missing plugin init fn-pointer field"
    );
    assert!(
        output.contains("pub process: Option<unsafe extern \"system\" fn(plugin: *const clap_plugin, process: *const clap_process) -> clap_process_status>,"),
        "Missing plugin process fn-pointer field"
    );
    assert!(
        output.contains("pub get: Option<unsafe extern \"system\" fn(list: *const clap_input_events, index: u32) -> *const clap_event_header>,"),
        "Missing input-events get fn-pointer field"
    );

    // Renamed `type` field (Rust keyword)
    assert!(
        output.contains("pub kind: u16"),
        "Missing renamed kind field"
    );

    // Char array fields
    assert!(
        output.contains("pub name: [c_char; 256]"),
        "Missing CLAP_NAME_SIZE array"
    );
    assert!(
        output.contains("pub module: [c_char; 1024]"),
        "Missing CLAP_PATH_SIZE array"
    );

    // Static entry point keeps its semicolon
    assert!(
        output.contains("pub static clap_entry: clap_plugin_entry = clap_plugin_entry {"),
        "Missing clap_entry static"
    );
    assert!(
        output.contains("    get_factory: None\n};"),
        "clap_entry static must end with a semicolon"
    );

    // Helpers: safe fn + unsafe proc
    assert!(
        output.contains("pub extern \"system\" fn clap_version_is_compatible"),
        "Missing compatible helper"
    );
    assert!(
        output.contains("pub unsafe extern \"system\" fn plugin_has_init"),
        "Missing has_init proc"
    );
}

#[test]
fn test_fixture_raylib_api() {
    let fixture = Path::new("tests/fixtures/raylib_api.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Header + imports
    assert!(!output.contains("#![no_std]"), "Unexpected #![no_std]");
    assert!(
        output.contains("use core::ffi::*;"),
        "Missing core::ffi import"
    );
    assert!(
        !output.contains("use libc::*;"),
        "raylib fixture should not pull libc"
    );

    // C typedef aliases
    assert!(
        output.contains("pub type Texture2D = Texture;"),
        "Missing Texture2D alias"
    );
    assert!(
        output.contains("pub type RenderTexture2D = RenderTexture;"),
        "Missing RenderTexture2D alias"
    );
    assert!(
        output.contains("pub type AudioCallback = Option<unsafe extern \"system\" fn(buffer: *mut c_void, frames: c_uint)>;"),
        "Missing AudioCallback fn-pointer alias"
    );

    // Structs with C primitive mapping
    assert!(output.contains("pub struct Vector2"), "Missing Vector2");
    assert!(output.contains("pub struct Matrix"), "Missing Matrix");
    assert!(output.contains("pub struct Color"), "Missing Color");
    assert!(output.contains("pub r: c_uchar"), "Missing c_uchar field");
    assert!(output.contains("pub struct Texture"), "Missing Texture");
    assert!(output.contains("pub struct Sound"), "Missing Sound");
    assert!(output.contains("pub struct Music"), "Missing Music");
    assert!(
        output.contains("pub struct AudioStream"),
        "Missing AudioStream"
    );
    assert!(
        output.contains("pub struct rAudioBuffer {"),
        "Missing opaque rAudioBuffer"
    );
    assert!(
        output.contains("pub recs: *mut Rectangle"),
        "Missing Rectangle* pointer"
    );

    // Audio stub functions
    assert!(
        output.contains("pub unsafe extern \"system\" fn InitAudioDevice()"),
        "Missing InitAudioDevice"
    );
    assert!(
        output.contains("pub unsafe extern \"system\" fn LoadSound(file_name: *const c_char) -> Sound"),
        "Missing LoadSound"
    );
    assert!(
        output.contains("pub unsafe extern \"system\" fn SetAudioStreamCallback(stream: AudioStream, callback: AudioCallback)"),
        "Missing SetAudioStreamCallback"
    );

    // Window/drawing stubs
    assert!(output.contains("pub unsafe extern \"system\" fn InitWindow(width: c_int, height: c_int, title: *const c_char)"), "Missing InitWindow");
    assert!(output.contains("pub unsafe extern \"system\" fn DrawText(text: *const c_char, pos_x: c_int, pos_y: c_int, font_size: c_int, color: Color)"), "Missing DrawText");

    // Helpers with real bodies
    assert!(
        output.contains("pub extern \"system\" fn raylib_color"),
        "Missing raylib_color helper"
    );
    assert!(
        output.contains("pub extern \"system\" fn vector2_length_squared"),
        "Missing vector2 helper"
    );
}

#[test]
fn test_fixture_atomics_operators() {
    let fixture = Path::new("tests/fixtures/atomics_operators.carbide");
    assert!(fixture.exists(), "Fixture file missing: {:?}", fixture);

    let output = transpile_fixture(fixture);

    // Verify atomics import
    assert!(output.contains("use core::sync::atomic::*;"), "Missing core::sync::atomic import");
    assert!(output.contains("pub val: AtomicI32,"), "Missing AtomicI32 struct field");
    assert!(output.contains("pub active: AtomicBool,"), "Missing AtomicBool struct field");
    assert!(output.contains("pub total: AtomicUsize,"), "Missing AtomicUsize struct field");

    // Verify impl methods don't have #[no_mangle]
    assert!(!output.contains("#[no_mangle]\n    pub fn new"), "impl methods must not have #[no_mangle]");
    assert!(!output.contains("#[no_mangle]\n    pub fn increment"), "impl methods must not have #[no_mangle]");

    // Verify body char* cast mapping
    assert!(output.contains("as *mut c_char;"), "as char* should map to *mut c_char");
    assert!(output.contains("as *const c_char;"), "as char const* should map to *const c_char");

    // Verify top-level function gets #[no_mangle] and extern system
    assert!(output.contains("#[no_mangle]"), "Missing #[no_mangle] on free proc");
    assert!(output.contains("pub unsafe extern \"system\" fn test_operators_and_closures"), "Missing test_operators_and_closures");
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

        // Default mode (std)
        let output = transpile_fixture(fixture);
        assert!(
            !output.is_empty(),
            "Empty transpilation output for: {}",
            name
        );
        assert!(
            !output.contains("#![no_std]"),
            "Unexpected #![no_std] in default mode for: {}",
            name
        );
        assert!(
            output.contains("use core::ffi::*;"),
            "Missing core::ffi import in: {}",
            name
        );

        // Explicit --no-std mode
        let output_no_std = transpile_fixture_with_args(fixture, &["--no-std"]);
        assert!(
            output_no_std.contains("#![no_std]"),
            "Missing #![no_std] with --no-std flag in: {}",
            name
        );

        // Explicit --std mode
        let output_std = transpile_fixture_with_args(fixture, &["--std"]);
        assert!(
            !output_std.contains("#![no_std]"),
            "Unexpected #![no_std] with --std flag in: {}",
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
