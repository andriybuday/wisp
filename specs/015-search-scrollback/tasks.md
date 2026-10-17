# Tasks: Search in Scrollback

**Branch**: `015-search-scrollback` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

- [x] T001 Read constitution and the 014 absolute-line helpers in `src/terminal.rs`, title handling in `src/window.rs` `process_pty`, `render(terminal, blink_on)` in `src/renderer.rs`, and copy/paste interception in `src/main.rs`; create feature via Spec Kit scripts
- [x] T002 Terminal `find_matches` (case-insensitive, per physical line, spacer-skipping column map, non-overlapping left-to-right, empty query empty) and `scroll_to_absolute_line` (top-row unless in live grid, clamped, alt no-op) plus `abs_line_to_viewport_row`
- [x] T003 Window search state (`search_active`, query, cached matches, current index) with start/end/push_char/backspace/next/prev (wrap), title feedback `Search: {query} (n/total)` / `(no matches)`, OSC titles recorded but not applied while searching, title restored on end, refresh + re-scroll after `process_pty` and on resize
- [x] T004 Renderer highlights parameter (`&[(viewport_row, start_col, end_col)]` + current index): dark-yellow matches, orange current, applied after selection; window converts absolute matches to viewport rows each frame
- [x] T005 Main key routing: Cmd+F / Ctrl+Shift+F starts search (consumed); while searching Escape/Enter/Shift+Enter/Backspace/text go to search, copy/paste still work, IME commits append to query; unchanged otherwise (manual check)
- [x] T006 Tests: case-insensitive; multiple matches one line with correct cols; scrollback matches with absolute lines; match after wide CJK char at post-spacer col; empty and absent query empty; non-overlapping policy (`aa` in `aaa`/`aaaa`); wrapped-spanning needle not found; alt-screen line space; `scroll_to_absolute_line` makes `get_viewport_cell` show the target. Key routing/title/highlight are manual-check only
- [x] T007 Run gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build`
- [x] T008 Commit on `015-search-scrollback` as `Add scrollback search with match highlighting (015)` (no push, no merge, no Co-Authored-By trailer)
