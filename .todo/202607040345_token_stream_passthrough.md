# Session Archive: Token-Stream Pass-Through Parser

**Date**: 2026-07-04
**Previous Commit**: f16d466
**Session Commit**: 75577a3

## Tasks Completed

- [x] Redefine AST in `src/ast.rs` - remove `Expr`, update `Stmt` to hold `Vec<Token>` raw token streams
- [x] Refactor Parser in `src/parser.rs` - balanced token-stream statement capture (`read_balanced_tokens`), add `Item::Impl`, `Item::Enum`, `Item::Raw`
- [x] Refactor Transform in `src/transform.rs` - flat token-stream type substitution and postfix-to-prefix pointer rewrites
- [x] Refactor Emitter in `src/emitter.rs` - spacing-aware raw token emitter; fix `as *mut T` space, fix `}` newline only when tokens follow
- [x] Add `tests/fixtures/rust_syntax.carbide` - fixture covering `while`/`loop`/`match`/arrays/type-casts/`impl`/`enum`
- [x] All 19 tests passing (13 unit + 5 fixture + 1 integration)
- [x] Update README - architecture diagram, workspace layout, `no_std` compatibility note

## Key Design Decision

User directed: the parser should NOT build a deep expression AST for function bodies.
Instead, capture statement bodies as raw `Vec<Token>` streams and apply only the
well-defined Carbide transformations (type substitution, pointer flip). Let `rustc`
handle all semantic checking. This makes any valid `no_std` Rust work transparently
inside `.carbide` files.
