# Tasks: Selection over Absolute Lines with Word and Line Select

**Branch**: `014-selection-correctness` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

- [x] T001 Read constitution and current `src/terminal.rs` / `src/window.rs` selection and mouse paths (008 reporting, 013 hyperlink precedence); create feature via Spec Kit scripts
- [x] T002 Terminal selection model: store `(start_col, start_abs_line, end_col, end_abs_line)`; `viewport_to_abs_line` mirroring `get_viewport_cell`; `get_absolute_cell`; `normalized_selection`; `is_cell_selected` lexicographic; `get_selected_text` absolute iteration, spacer skip, per-line trailing-space trim
- [x] T003 Stability: `shift_selection_lines` on scroll-push trim and `set_scrollback_limit` drain; alt-screen switches (?1049/?47/?1047) clear selection; resize clear kept
- [x] T004 Terminal `select_word_at` (word chars alphanumeric + `_-./~:@`, spacer-with-head, space-run, single-cell fallback), `select_line_at`, `clear_selection_if_single_cell`
- [x] T005 Window click counting only: `last_click_time/cell/count` (400 ms, same cell) in `mouse_press` after hyperlink/reporting checks (1 drag, 2 word, 3+ line); no-drag single-cell clear in `mouse_release`
- [x] T006 Tests: scrolled-up copy of scrollback text; bottom selection survives push into scrollback; trailing trim with inner spaces kept; word expansion over letters/digits/punctuation set stopping at others; word select on wide char includes spacer; line select full line; alt-screen switch clears; trimmed-to-limit scrollback shifts without panic; space-run select
- [x] T007 Run gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build`
- [x] T008 Commit on `014-selection-correctness` as `Rebase selection on absolute lines with word/line select (014)` (no push, no merge, no Co-Authored-By trailer)
