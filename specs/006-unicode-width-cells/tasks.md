# Tasks: Unicode Width-Aware Cells

**Branch**: `006-unicode-width-cells` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

- [x] T001 Read constitution, current `Terminal::print` / `CellFlags` / `get_selected_text`, and `build_vertices` in `src/renderer.rs`; confirm 89-test baseline
- [x] T002 Add `unicode-width = "0.2"` to `Cargo.toml` (justified in plan.md, Principle I)
- [x] T003 Add `CellFlags::WIDE_SPACER = 0b0001_0000`
- [x] T004 Rewrite `Terminal::print` to be width-aware: width 0 dropped, width 1 unchanged, width 2 writes head + spacer and advances by 2, last-column wide char wraps first via `linefeed()`, immediate-wrap semantics kept
- [x] T005 Implement overwrite hygiene: writing over a spacer blanks the wide char to its left; writing over a wide head blanks its spacer; blanking keeps colors
- [x] T006 Make `get_selected_text` skip spacer cells; verify the renderer needs no change (background fill precedes the `ch == ' '` glyph skip in `build_vertices`)
- [x] T007 Add terminal tests: CJK pair + copied attrs, emoji width 2, last-column wrap, overwrite spacer/head blanking (colors kept), U+200B dropped, mixed-line selection without extra spaces (plus immediate-wrap pin)
- [x] T008 Add parser test: UTF-8 CJK bytes through `AnsiParser` land in expected cells
- [x] T009 Run gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build`
- [x] T010 Commit on `006-unicode-width-cells` as `Add Unicode width-aware cells (006)` (no push, no merge, no Co-Authored-By trailer)
