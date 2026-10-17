# Implementation Plan: Unicode Width-Aware Cells

**Branch**: `006-unicode-width-cells` | **Date**: 2026-10-17 | **Spec**: [spec.md](./spec.md)

## Summary

Make `Terminal::print` width-aware via `unicode-width` 0.2: width-0 chars are
dropped, width-1 chars are unchanged, and width-2 chars write a head cell
plus a `WIDE_SPACER` cell and advance the cursor by two (wrapping first from
the last column, keeping immediate-wrap semantics). Overwrites blank the
stranded half of a wide pair, and `get_selected_text` skips spacers. The
renderer needs no change. Combining/ZWJ composition, deferred wrap, and font
fallback are explicit non-goals (see spec.md).

## Technical Context

- **Language/Version**: Rust, edition 2021
- **Primary Dependencies**: NEW `unicode-width = "0.2"` (see the Principle I justification in the Constitution Check below); existing `vte` already delivers decoded `char`s to `Terminal::print`
- **Testing**: `cargo test`, `#[cfg(test)]` modules in `src/terminal.rs` and `src/parser.rs`
- **Target Platform**: macOS primary; width tables are platform-independent
- **Constraints**: changes confined to `Cargo.toml`/`Cargo.lock`, `src/terminal.rs`, one parser test in `src/parser.rs`, and `specs/006-unicode-width-cells/`; renderer untouched; width-1 behaviour byte-identical to before

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS, with the required dependency justification: correct CJK/emoji widths are daily-essential for any non-ASCII shell output, and applications already lay out text assuming the terminal uses these widths, so without this feature every wide char misaligns the rest of its line. The new dependency `unicode-width` (0.2) is justified because the width data is a large set of Unicode East Asian Width tables that would be error-prone and unmaintainable to hand-roll or vendor; the crate is tiny, has no dependencies of its own, is the de-facto standard `wcwidth` implementation in the Rust ecosystem, and exposes exactly one needed function (`UnicodeWidthChar::width`). No new modules, configuration, or UI surface are added; deferred wrap, grapheme composition, and font fallback are deliberately excluded to keep the change minimal.
- **II. Terminal Correctness over Feature Count** — PASS. Semantics match xterm/wcwidth: wide chars take two columns with a spacer, zero-width chars take none, a wide char that cannot fit in the last column wraps before printing. Nothing unimplemented is advertised (DA replies and `TERM` are untouched); the combining-mark/ZWJ limitation is documented as an explicit non-goal in spec.md rather than silently approximated.
- **III. Test-First for the Terminal Core (NON-NEGOTIABLE)** — PASS. `terminal` and `parser` are core; every new behaviour (wide pair + spacer flags/colors, emoji width, last-column wrap, both overwrite directions, zero-width drop, selection without inserted spaces, and UTF-8 through `AnsiParser`) gets a `#[cfg(test)]` assertion on grid/cursor state. All 89 pre-existing tests keep passing.
- **IV. Event-Driven Performance** — PASS. The per-char print path adds one table-driven width lookup and at most one extra cell write; no allocation is added to parsing, printing, or rendering, and scrollback/atlas bounds are unchanged.
- **V. Safe Handling of Untrusted Output** — PASS. `UnicodeWidthChar::width` is total over `char` (`None` for control chars is treated as 0 and dropped); grid indexing stays bounds-checked exactly as before, the wide path requires `col + 1 < cols` (degrading to one cell on a 1-column grid), and no panics/unwraps are added on PTY-reachable paths.
- **Quality Gates** — PASS. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated` (the documented winit exception from 001/CI), `cargo test`, and `cargo build` all run before commit.

No exceptions. Complexity Tracking: none.

## Project Structure

```text
specs/006-unicode-width-cells/
├── spec.md
├── plan.md              # this file
└── tasks.md
Cargo.toml               # + unicode-width = "0.2" (Cargo.lock updated by cargo)
src/
├── terminal.rs          # WIDE_SPACER flag; width-aware print + overwrite hygiene; get_selected_text skips spacers; tests
└── parser.rs            # one UTF-8 CJK test (no production change)
```

## Approach

1. Add `unicode-width = "0.2"` to `Cargo.toml`; add `CellFlags::WIDE_SPACER = 0b0001_0000` in `src/terminal.rs`.
2. Rewrite `Terminal::print`: `let width = UnicodeWidthChar::width(ch).unwrap_or(0)`; return immediately on 0 (no cell change, no cursor advance). For width ≥ 2 with at least 2 columns and the cursor in the last column, call the existing `linefeed()` first. Then run overwrite hygiene for each cell about to be written, write the cell (width 1: as today; width ≥ 2: head cell plus spacer `Cell { ch: ' ', ..head }` with `WIDE_SPACER` added), advance the cursor by the width, and apply today's immediate wrap (`linefeed()` when the cursor reaches/passes `cols`).
3. Add a private overwrite-hygiene helper: for a position being written, if its current cell is a spacer, blank the cell to its left; else if the next cell is a spacer (this cell is a wide head), blank that spacer. Blanking sets `ch = ' '`, removes `WIDE_SPACER`, and keeps fg/bg (and other flags). Applied to both written positions of a wide pair, this also cleans up a stranded spacer two cells over when a wide char overwrites the head of a shifted pair.
4. In `get_selected_text`, skip cells whose flags contain `WIDE_SPACER` instead of pushing their `' '`.
5. Renderer: make no change. Verified in `build_vertices` (`src/renderer.rs`): the background quad is emitted from the cell's resolved bg/inverse/selection state before the `if cell.ch == ' ' { continue; }` early-skip, so a spacer (whose `ch` is `' '` and whose fg/bg/flags copy the wide cell) already renders its background continuously and draws no glyph.
6. Add the terminal and parser tests listed in SC-001; run all four gates; commit on the feature branch (no push, no merge).
