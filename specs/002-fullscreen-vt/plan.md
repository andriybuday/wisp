# Implementation Plan: Full-screen VT Support (002)

**Branch**: `002-fullscreen-vt` | **Spec**: [spec.md](spec.md)

## Summary

Add alternate screen, DECSTBM scroll region, index/reverse-index, insert/
delete line/char, scroll up/down, save/restore cursor, cursor visibility
(?25), and CHA/VPA/CNL/CPL, all in `src/terminal.rs` (state) and
`src/parser.rs` (dispatch). No new dependencies.

## Technical Approach

- `Terminal` gains `alt_grid: Vec<Cell>`, `on_alt: bool`, `region_top/bottom:
  usize` (0-based inclusive), `saved_cursor: (usize,usize,u8,u8,CellFlags)`,
  and `alt_saved_cursor` for ?1049. Active-grid helpers return the right grid.
- Region scrolling generalizes `scroll_up`: `scroll_region_up/down(n)` move
  rows within `[top,bottom]`; scrollback push only when region is full-height
  and screen is main.
- `linefeed` becomes region-aware: at bottom margin scroll region; below
  region clamp at `rows-1`; otherwise advance. IND reuses linefeed without CR
  (separate `index()`), RI scrolls down at top margin, NEL = CR+IND.
- Parser: new CSI arms `@ P X L M S T G d E F r s u`, extend `?h/?l` for
  25/47/1047/1049, `esc_dispatch` for `D M E 7 8`. Param 0 defaults to 1 for
  count-based ops (xterm semantics); all counts clamped.

## Constitution Check

- I. Minimal: full-screen apps are daily-use; no new deps, no abstractions
  beyond a second grid. PASS.
- II. Correctness: matches xterm ctlseqs semantics (?1049/1047/47, DECSTBM,
  DECSC); unknown sequences ignored. PASS.
- III. Test-first: new #[cfg(test)] tests in both files per FR. PASS.
- IV. Performance: region scrolls copy bounded rows; scrollback capped. PASS.
- V. Safety: saturating/clamped indexing everywhere; no panics. PASS.

## Complexity Tracking

No exceptions.
