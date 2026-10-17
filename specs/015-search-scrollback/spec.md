# Feature Specification: Search in Scrollback

**Feature Branch**: `015-search-scrollback`
**Created**: 2026-10-17
**Status**: Implemented
**Input**: "Search in scrollback"

## Problem

There is no way to find earlier output. A user who needs a command, error
message, or path printed long ago must scroll and eyeball the whole
scrollback. Searching history is a daily terminal need.

## Decisions

### Terminal search core (absolute lines, as in 014)

`Terminal::find_matches(query) -> Vec<(usize, usize, usize)>` returns
`(absolute_line, start_col, end_col_exclusive)` in ascending order.
Absolute lines address scrollback lines followed by grid rows (on the
alternate screen, just alt-grid rows), the same line space as the 014
selection model, resolved through the existing `get_absolute_cell`
accessor.

- Matching is case-insensitive: each line's text and the query are
  compared via `to_lowercase`.
- Matching is per physical line. A match that would span a wrapped
  continuation onto the next physical row is NOT found; this is a
  documented limitation (joining logical lines could come later, but
  physical-line search already finds the overwhelmingly common case).
- Each line's text is built skipping `WIDE_SPACER` cells, and match
  positions are mapped back to cell columns around wide characters: a
  wide head cell spans two columns, so a match starting after a wide
  char reports the post-spacer column, and a match that ends on a wide
  char ends after its spacer.
- Occurrences are non-overlapping, scanned left to right: after a match
  the scan resumes at the first character after it (`"aa"` in `"aaa"`
  matches once).
- An empty query returns no matches.

`Terminal::scroll_to_absolute_line(abs_line)` sets `scroll_offset` so the
line becomes visible: at the top viewport row for scrollback lines, and
with `scroll_offset` clamped to 0 for lines already in the live grid
(they are visible somewhere on screen). Out-of-range lines are clamped.
On the alternate screen it is a no-op: the whole alt grid is always the
viewport and the scroll offset is ignored there.

`Terminal::abs_line_to_viewport_row(abs_line) -> Option<usize>` converts
an absolute line back to a viewport row under the current scroll offset
(`None` when not visible), for highlight rendering.

### Search state in the window

`WispWindow` owns search state: an active flag, the query string, the
cached matches, and the current match index. Methods: `start_search`,
`end_search`, `search_active`, `search_push_char`, `search_backspace`,
`search_next`, `search_prev`.

- Editing the query recomputes matches, keeps the current index clamped
  into range, and jumps (scrolls) to that match. `search_next` /
  `search_prev` wrap around the match list.
- While searching, the window title is feedback:
  `Search: {query} ({current+1}/{total})` when there are matches,
  `Search: {query} (no matches)` when a non-empty query has none, and
  `Search: ` for an empty query.
- `process_pty` must not stomp the search title with an OSC title while
  search is active; it still records `last_title`. `end_search`
  restores the recorded title, or `Wisp Terminal` when none was set.
- When `process_pty` processes new output while search is active,
  matches are refreshed and the viewport re-scrolls to the current
  match (new output may push lines into scrollback, moving absolute
  lines). A resize while searching likewise refreshes matches.

### Key routing

- `Cmd+F` (physical `KeyF` with Super) or `Ctrl+Shift+F` starts search
  and is consumed, mirroring the existing copy/paste combos.
- While search is active, keyboard input goes to the search instead of
  the PTY: `Escape` ends search, `Enter` goes to the next match
  (`Shift+Enter` previous), `Backspace` deletes a query character,
  other text appends to the query. Copy/paste combos keep working.
  IME commits append to the query while searching.
- Everything is unchanged when not searching.

### Highlight rendering

`Renderer::render` gains a highlights parameter: a slice of
`(viewport_row, start_col, end_col_exclusive)` plus the index of the
current match within that slice (`Option<usize>`). The window converts
the absolute matches to viewport rows for the current scroll offset
each frame. Matched cells get a dark-yellow background, the current
match an orange background. Search highlighting is applied after
selection, so it wins over the selection background; the cursor cell
keeps its cursor rendering.

### Out of scope

- Regex search.
- A copy-mode cursor (vim-style navigation/selection by keyboard).
- Searching the main screen's scrollback while on the alternate screen.
  Search always operates on the active absolute-line space: on the alt
  screen the matches come from the alt grid's rows as its own line
  space (`find_matches` and the scroll/highlight helpers behave sanely
  there — all alt rows are visible and scrolling is a no-op).
- Match highlighting UI beyond background colors (no search bar widget;
  the title carries the feedback, keeping the UI minimal).

## User Scenarios & Testing

### User Story 1 - Find text anywhere in history (Priority: P1)

A user presses Cmd+F / Ctrl+Shift+F, types a needle, and the viewport
jumps to a highlighted occurrence in scrollback or on screen.

**Acceptance Scenarios**:

1. **Given** output that has scrolled into scrollback, **When** the user searches for text on a scrollback line, **Then** the match carries that line's absolute index and the viewport scrolls so the line is visible.
2. **Given** mixed-case output, **When** the query is a different case, **Then** it still matches (case-insensitive).
3. **Given** several occurrences on one line, **Then** each is reported with correct columns, including after a wide CJK character (post-spacer column).
4. **Given** an empty query or a query that does not occur, **Then** there are no matches.

### User Story 2 - Step through matches (Priority: P1)

**Acceptance Scenarios**:

1. **Given** an active search with matches, **When** the user presses Enter repeatedly, **Then** the current match advances and wraps from the last match to the first; Shift+Enter steps backwards, also wrapping.
2. **Given** an active search, **Then** the title shows the query and `current/total`, or `(no matches)`.
3. **Given** an active search, **When** the user presses Escape, **Then** search ends, highlights clear, and the OSC/last title (or `Wisp Terminal`) is restored.

### User Story 3 - Search stays correct while output flows (Priority: P2)

**Acceptance Scenarios**:

1. **Given** an active search, **When** new PTY output arrives, **Then** matches are recomputed and the viewport re-scrolls to the current match.
2. **Given** an active search, **When** the shell sets an OSC title, **Then** the search title is not overwritten, and ending search restores the OSC title.

### Edge Cases

- Overlapping occurrences: non-overlapping left-to-right (`"aa"` in
  `"aaaa"` matches at columns 0 and 2).
- A needle spanning a wrapped continuation is not found (documented
  limitation above).
- Degenerate grids (0 cols/rows) and out-of-range absolute lines never
  panic; they return no matches / clamp.
- Highlighting, title feedback, and key routing are windowing/renderer
  behaviour and are manual-check only (constitution III exemption).

## Requirements

- **FR-001**: `find_matches(&self, query: &str) -> Vec<(usize, usize, usize)>` returns `(absolute_line, start_col, end_col_exclusive)` ascending; case-insensitive via `to_lowercase`; per physical line; spacers skipped with correct column mapping around wide chars; empty query yields no matches; occurrences non-overlapping left-to-right.
- **FR-002**: `scroll_to_absolute_line(&mut self, abs_line: usize)` sets `scroll_offset = scrollback_len.saturating_sub(clamped abs_line)` on the main screen (target at the top row unless it is in the live grid); no-op on the alternate screen.
- **FR-003**: `abs_line_to_viewport_row(&self, abs_line: usize) -> Option<usize>` inverts `viewport_to_abs_line` under the current scroll offset.
- **FR-004**: `WispWindow` holds `search_active`, `search_query`, cached `search_matches`, and `search_current`, with `start_search` / `end_search` / `search_active` / `search_push_char` / `search_backspace` / `search_next` / `search_prev`. Query edits recompute matches, clamp the current index, jump to it, update the title, and request a redraw. Next/prev wrap.
- **FR-005**: The search title is `Search: {query} ({current+1}/{total})`, `Search: {query} (no matches)` for a non-empty query with no matches, `Search: ` for an empty query. `process_pty` records OSC titles in `last_title` but applies them to the window only when search is inactive; `end_search` restores `last_title` or `Wisp Terminal`. After processing new output while searching, matches refresh and re-scroll to the current match.
- **FR-006**: In `main.rs`, physical `KeyF` with Super or Ctrl+Shift calls `start_search` and is consumed. While `search_active`, pressed keys route to search (Escape end, Enter next, Shift+Enter previous, Backspace delete, other plain text appends) instead of the PTY; copy/paste combos are checked first and keep working. IME commits append to the query while searching.
- **FR-007**: `Renderer::render(terminal, blink_on, highlights: &[(usize, usize, usize)], current_highlight: Option<usize>)` draws dark-yellow backgrounds for highlighted cells and orange for the current match, applied after selection (search wins over selection background). The window builds the viewport highlights from the absolute matches each frame.

## Success Criteria

- **SC-001**: All 169 pre-existing tests pass unmodified; new terminal-core tests cover FR-001/FR-002 acceptance scenarios (case-insensitivity, multiple matches per line, scrollback absolute lines, wide-char columns, empty/absent query, non-overlapping policy, scroll-to-line visibility).
- **SC-002**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, and `cargo build` pass.

## Assumptions

- Keeping the current index clamped (rather than re-anchoring to the
  viewport top) on every query edit is the simplest correct stepping
  behaviour; the first typed character jumps to the first match.
- The window title is sufficient search feedback for a minimal terminal;
  no overlay widget is added.
