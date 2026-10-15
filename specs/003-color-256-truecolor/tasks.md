# Tasks: 256-Color and Truecolor Support

**Branch**: `003-color-256-truecolor` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

## Phase 1: Terminal core

- [x] T001 Add `Color` enum in `src/terminal.rs`; migrate `Cell.fg/bg`, `current_fg/current_bg`, `Cell::default`, `reset_sgr`, `sgr`; add `set_fg_color`/`set_bg_color`
- [x] T002 Update 001-era SGR tests in `src/terminal.rs`/`src/parser.rs` to the `Color` API; add terminal-level extended-color tests

## Phase 2: Parser

- [x] T003 Rewrite SGR `m` handling in `src/parser.rs` as stateful `Params` iteration supporting semicolon and colon forms for 38;5/48;5/38;2/48;2, clamping RGB to 255
- [x] T004 Add parser tests: semicolon and colon 256-color, truecolor fg and bg, malformed `38;5` and `38;2;1`

## Phase 3: Palette & renderer

- [x] T005 Add `ColorPalette::ansi_256` in `src/config.rs` (16 colors, 6x6x6 cube, grayscale); remove `#[allow(dead_code)]` from `foreground`; add cube/grayscale unit tests
- [x] T006 Resolve `Color` to RGBA in `src/renderer.rs`; background quad when `bg != Default || inverse || selected`; bold +8 only for `Indexed(0..=7)`; inverse swaps resolved colors

## Phase 4: Verify & commit

- [x] T007 Verify: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build` all pass
- [x] T008 Commit on `003-color-256-truecolor` as "Add 256-color and truecolor support (003)" (no Co-Authored-By)
