# Implementation Plan: Selection over Absolute Lines with Word and Line Select

**Branch**: `014-selection-correctness` | **Date**: 2026-10-17 | **Spec**: [spec.md](./spec.md)

## Summary

Selection moves from viewport rows to absolute lines (scrollback lines
followed by grid rows). The public Terminal API keeps taking viewport rows
and converts internally with the same math as `get_viewport_cell`.
`get_selected_text` reads absolute lines via a new internal
`get_absolute_cell` accessor, skips spacers, and trims trailing spaces per
line. Word select expands over alphanumerics plus `_-./~:@` (spacers belong
to their wide char; a space click selects the space run), line select covers
`0..cols-1`. The window counts clicks (400 ms, same cell) on the
local-selection path only, preserving the 008 mouse-reporting and 013
hyperlink precedence exactly. Alt-screen switches clear the selection;
scrollback trims shift stored lines down saturating.

## Technical Context

- **Language/Version**: Rust, edition 2021
- **Primary Dependencies**: none added; `std::time::Instant`/`Duration` already used in `window.rs`
- **Testing**: `cargo test`; `#[cfg(test)]` in `terminal.rs` (selection model, word/line select, stability)
- **Target Platform**: macOS primary; no platform-specific code
- **Constraints**: only `src/terminal.rs` (selection model), `src/window.rs` (click counting only), and `specs/014-selection-correctness/`

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS. Correct copy from scrollback and double/triple-click select are daily terminal needs; the change reuses the existing selection tuple and viewport math with no new dependencies, no new UI, and no rectangular/copy-on-select extras.
- **II. Terminal Correctness** — PASS. Selection is a local UI behaviour; no `TERM`/DA capability changes, no escape-sequence changes. Mouse-reporting (008) and hyperlink-click (013) semantics are preserved exactly, so application-visible behaviour is unchanged.
- **III. Test-First for the Terminal Core** — PASS. The absolute-line model, copied-text trimming, word/line expansion, and stability rules are pure terminal-core behaviour covered by unit tests (feed bytes/selections in, assert text out). The window-side click counting follows the manual-checklist exemption for windowing code, with its decision logic kept thin over tested Terminal methods.
- **IV. Event-Driven Performance** — PASS. Conversions are O(1); `get_selected_text` iterates only the selected range; word expansion is bounded by the line width. No per-frame or per-byte allocation changes.
- **V. Safe Handling of Untrusted Output** — PASS. All indexes are clamped/saturating; wide-char/spacer handling never indexes out of bounds; trimmed scrollback shifts selection lines with `saturating_sub`.
- **Quality Gates** — fmt/clippy/test/build all run before commit.

Complexity Tracking: none.

## Project Structure

```text
specs/014-selection-correctness/{spec.md,plan.md,tasks.md}
src/terminal.rs   # absolute-line selection, get_absolute_cell, select_word_at, select_line_at, trim, stability, tests
src/window.rs     # click counting (last_click_time/cell/count) in mouse_press, no-drag clear in mouse_release
```

## Approach

1. Helpers: `viewport_to_abs_line(viewport_row)` mirroring `get_viewport_cell` (alt: row; unscrolled: `scrollback_len + row`; scrolled: `viewport_start + row`, clamped), `get_absolute_cell(col, abs_line)` (scrollback vs active grid), `normalized_selection()`, and `shift_selection_lines(n)` for trim sites. Add a `clamp_col_row` guard for degenerate grids.
2. `set_selection_start/end` convert viewport rows to absolute lines; `is_cell_selected` converts the queried viewport row and compares `(line, col)` lexicographically against the normalized selection.
3. `get_selected_text` iterates absolute lines with per-line start/end columns, builds each line skipping spacers, trims trailing `' '`, joins with `\n`.
4. `select_word_at`: classify the clicked cell (spacer → its head): word char → expand over word cells (wide heads consume their spacer, spacer-as-neighbour stops at its head); space → expand over plain space cells (spacers stop the run); other → single cell (plus spacer if wide). `select_line_at`: `(0, abs) .. (cols-1, abs)`. `clear_selection_if_single_cell(col, viewport_row)` clears only when start == end == that cell.
5. Stability: `set_scrollback_limit` excess drain and the scroll-push `remove(0)` call `shift_selection_lines`; `set_alt_screen_1049/47/1047` clear selection when the grid switches (1047-leave also clears, since it wipes the alt grid).
6. Window: after the hyperlink and mouse-reporting early returns, count clicks (400 ms + same `(col,row)` increments, else resets to 1); 1 → `set_selection_start` + `mouse_pressed`, 2 → `select_word_at`, 3+ → `select_line_at`. In `mouse_release` (local path, left button), clear a no-drag single-cell selection at the release cell.
7. Tests, four gates, commit (no push/merge).
