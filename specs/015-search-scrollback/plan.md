# Implementation Plan: Search in Scrollback

**Branch**: `015-search-scrollback` | **Date**: 2026-10-17 | **Spec**: [spec.md](./spec.md)

## Summary

Case-insensitive plain-text search over the 014 absolute-line space
(scrollback followed by grid rows; alt-grid rows on the alternate
screen). `Terminal::find_matches` builds each physical line's text
skipping spacer cells, lowercases line and query, scans
non-overlapping left-to-right, and maps match spans back to cell
columns (wide heads span two columns). `scroll_to_absolute_line` sets
`scroll_offset` so a match line is visible (top row for scrollback
lines, clamped). `WispWindow` owns the search session (query, cached
matches, current index), gives title feedback, refreshes matches after
new PTY output, and never lets an OSC title stomp the search title.
`main.rs` routes Cmd/Ctrl+Shift+F and, while searching, all keys to the
search instead of the PTY. The renderer takes viewport highlights plus
the current-match index and paints dark-yellow/orange backgrounds after
selection.

## Technical Context

- **Language/Version**: Rust, edition 2021
- **Primary Dependencies**: none added
- **Testing**: `cargo test`; `#[cfg(test)]` in `terminal.rs` (find API, scroll-to-line). Key routing, title feedback, and highlight rendering are manual-check only.
- **Target Platform**: macOS primary; combos mirror the existing Cmd / Ctrl+Shift copy/paste handling on all platforms
- **Constraints**: only `src/terminal.rs`, `src/window.rs`, `src/main.rs`, `src/renderer.rs`, and `specs/015-search-scrollback/`

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS. Finding earlier output is a daily terminal need; the UI is just title feedback plus highlight colors, with no new widget, dependency, regex engine, or copy-mode. The search core reuses the 014 absolute-line accessor.
- **II. Terminal Correctness** — PASS. Search is a local UI behaviour; no `TERM`/DA changes and no escape-sequence changes. While searching, keys are intentionally consumed locally (standard find behaviour), and copy/paste precedence is preserved.
- **III. Test-First for the Terminal Core** — PASS. `find_matches` and `scroll_to_absolute_line` are pure terminal-core behaviour covered by unit tests (case, columns, scrollback lines, wide chars, overlap policy, visibility). Window/renderer/main routing follows the manual-checklist exemption, kept thin over the tested core.
- **IV. Event-Driven Performance** — PASS. Search runs on query edits and PTY batches, never per frame; its cost is bounded by the capped scrollback times the line width (a few thousand short lines). Per frame, the window converts only the matches intersecting the viewport and the renderer scans the viewport-sized highlight slice per cell, as it already does for selection. No idle redraws are added: search actions request single redraws.
- **V. Safe Handling of Untrusted Output** — PASS. Line/col indexes go through the bounds-checked `get_absolute_cell`; lowercase expansion is handled char-by-char with an explicit column map; degenerate grids and out-of-range lines clamp or return empty. Query text comes only from the local keyboard, never the PTY.
- **Quality Gates** — fmt/clippy/test/build all run before commit.

Complexity Tracking: none.

## Project Structure

```text
specs/015-search-scrollback/{spec.md,plan.md,tasks.md}
src/terminal.rs   # find_matches, scroll_to_absolute_line, abs_line_to_viewport_row, tests
src/window.rs     # search state + methods, search title, process_pty refresh, highlight conversion
src/main.rs       # Cmd/Ctrl+Shift+F, search key routing, IME commit routing
src/renderer.rs   # render highlights param, dark-yellow/orange backgrounds after selection
```

## Approach

1. Terminal: add a `total_abs_lines` helper (alt: `rows`; main: `scrollback.len() + rows`). `find_matches`: early-return on empty query/degenerate grid; lowercase the query; per absolute line, build the lowered text char-by-char from non-spacer cells, recording `(start_col, end_col_exclusive)` per lowered char (wide head: end = col + 2 when the next cell is its spacer); scan with `str::find`, resuming at the match end (non-overlapping), mapping byte spans to char indices to columns. `scroll_to_absolute_line`: no-op on alt/degenerate; clamp the line; `scroll_offset = scrollback.len().saturating_sub(abs_line)`. `abs_line_to_viewport_row`: subtract `viewport_to_abs_line(0)`, keep rows `< rows`.
2. Window: add `search_active`, `search_query`, `search_matches`, `search_current`. A private `refresh_search` recomputes matches from the terminal, clamps the current index, scrolls to the current match, updates the search title, and requests a redraw; `search_push_char`/`search_backspace` edit then refresh; `search_next`/`search_prev` wrap the index then jump/title/redraw. `start_search` resets the session; `end_search` restores `last_title` or `Wisp Terminal`.
3. `process_pty`: record OSC titles in `last_title` always, apply with `set_title` only when search is inactive; after processing while searching, `refresh_search`. `resize` refreshes an active search after the terminal resize.
4. Rendering path: `WispWindow::render` builds viewport highlights by converting each absolute match via `abs_line_to_viewport_row`, tracking the position of `search_current`, and passes them to `Renderer::render(terminal, blink_on, &highlights, current)`.
5. Renderer: thread the highlights through `build_vertices`; per cell, compute match/current flags from the slice, set the background (dark yellow `[0.45, 0.38, 0.0, 1.0]`, orange `[0.85, 0.45, 0.0, 1.0]`) after the selection override and before the cursor override, and force the background quad for highlighted cells.
6. Main: in the pressed-key handler, check copy, paste, then find (`KeyF` + Super or Ctrl+Shift → `start_search`), then — when `search_active` — Escape/Enter/Shift+Enter/Backspace/plain text; otherwise the existing PTY path. Route IME commits to the query while searching.
7. Tests, four gates, commit (no push/merge).
