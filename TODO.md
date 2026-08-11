# Carbide Tasks

- [x] 1. Update `src/parser.rs` to support postfix references (`T&` -> `&mut T`, `T const&` -> `&T`).
- [x] 2. Update `src/transform.rs`:
  - Set default function ABI to `extern "system"`.
  - Ensure `void` in return type position is treated as unit `()`.
  - Fix `flip_as_pointer_casts` so binary multiplication expressions like `e1 as usize * e2` are preserved.
- [x] 3. Update `src/emitter.rs`:
  - Omit return type arrow `-> ...` when function or fn-pointer return type is `void`, `c_void`, or `()`.
  - Use `extern "system"` in fn-pointer types.
- [x] 4. Update unit tests in `src/parser.rs`, `src/transform.rs`, `src/emitter.rs`.
- [x] 5. Update fixture tests in `tests/fixture_tests.rs` and integration tests in `tests/integration_tests.rs`.
- [x] 6. Run all tests to verify correctness (`cargo test`).
- [x] 7. Update `SPEC.md`, `README.md`, `Tutorial.md`, and `.agents/skills/carbide_transpiler/SKILL.md`.
- [x] 8. Update `src/emitter.rs` to make `no_std: false` the default, only emitting `#![no_std]` when `no_std` is true.
- [x] 9. Update `src/main.rs` to add `--no-std` and `--std` flags.
- [x] 10. Update `src/parser.rs` to reject prefix `*mut T`, `*const T`, `&mut T`, `&T` with clear errors, enforcing C++-style postfix notation.
- [x] 11. Update unit tests, fixture tests (`tests/fixture_tests.rs`), and integration tests (`tests/integration_tests.rs`).
- [x] 12. Run all tests to verify correctness (`cargo test`).
- [x] 13. Update `SPEC.md`, `README.md`, `Tutorial.md`, and `.agents/skills/carbide_transpiler/SKILL.md`.
