# Feature Specification: Selection over Absolute Lines with Word and Line Select

**Feature Branch**: `014-selection-correctness`
**Created**: 2026-10-17
**Status**: Implemented
**Input**: "Selection over absolute lines with word and line select"

## Problem

Selection coordinates were viewport rows. `is_cell_selected` compared them
against the current viewport and `get_selected_text` read the live grid, so
selecting while scrolled up (or after new output scrolled) copied the wrong
text, and scrollback text could never be copied.

## Decisions

### Absolute-line selection model

Selection is stored as `(start_col, start_abs_line, end_col, end_abs_line)`
and compared lexicographically by `(line, col)`. An absolute line index
addresses scrollback lines followed by grid rows. The public Terminal API the
window uses keeps taking viewport rows (`set_selection_start(col,
viewport_row)`, `set_selection_end`, `is_cell_selected(col, viewport_row)`)
but converts to/from absolute lines internally using the same math as
`get_viewport_cell`.

`get_selected_text` iterates absolute lines via an internal absolute-line
cell accessor (`get_absolute_cell`) next to `get_viewport_cell`, skips
spacer cells (already the behaviour since 006), and trims trailing spaces on
every copied line (inner spaces are kept).

### Stability under scrolling and trimming

- When scrollback trims its oldest line (limit reached in scroll push or in
  `set_scrollback_limit`), stored selection lines shift down accordingly
  (saturating at 0).
- Switching to or from the alternate screen clears the selection.
- Resize already clears the selection since 011; that is kept.
- Normal scrolling that pushes lines into scrollback does NOT shift absolute
  indices: the selected text keeps the same absolute position in the combined
  scrollback+grid space, so a bottom selection still copies the same text
  after lines enter scrollback.

### Alternate-screen choice

While on the alternate screen, selections made there address alt-grid rows
via the same absolute scheme over (no scrollback + rows): selections on the
alt screen work within the alt grid, and leaving the alt screen clears them.
This is the simplest correct choice: the alt grid has no scrollback, so its
absolute lines are just its rows. All three alt-screen entry/exit paths
(?1049, ?47, ?1047) clear the selection whenever the active grid actually
switches (and ?1047 also clears when leaving, since it wipes the alt grid).

### Word and line select

- `Terminal::select_word_at(col, viewport_row)` expands left/right over word
  characters defined as alphanumeric or one of `_-./~:@` (a spacer belongs
  to its wide char). Clicking a space selects the contiguous space run.
- `Terminal::select_line_at(viewport_row)` selects columns `0..cols-1` of
  that absolute line.

### Click handling in window.rs

`mouse_press` tracks last click time and cell; a press within 400 ms on the
same viewport cell as the previous press increments a click count (reset
otherwise). Count 1 starts a drag selection as today; count 2 calls
`select_word_at`; count 3 calls `select_line_at`. Counts above 3 keep line
select. A plain click released on the same cell it started (no drag) clears
the selection in `mouse_release` (via a Terminal helper).

Mouse reporting precedence from 008 (Shift bypass) and hyperlink
Cmd/Ctrl-click from 013 are preserved exactly: click counting applies only
on the local-selection path, after hyperlink and mouse-reporting checks.

## User Scenarios & Testing

### User Story 1 - Copy scrollback text while scrolled up (Priority: P1)

A user scrolls up, drags over old output, and copies. The clipboard must
contain the text that was under the mouse, not live-grid text.

**Acceptance Scenarios**:

1. **Given** lines pushed into scrollback and the viewport scrolled up, **When** the visible scrollback text is selected, **Then** `get_selected_text` returns that scrollback text.
2. **Given** a selection made at the bottom, **When** enough new lines push it into scrollback, **Then** `get_selected_text` still returns the same text.

### User Story 2 - Double-click word, triple-click line (Priority: P1)

**Acceptance Scenarios**:

1. **Given** a word among delimiters, **When** double-clicked, **Then** the selection covers exactly the maximal run of word characters (alphanumeric or `_-./~:@`), stopping at other characters.
2. **Given** a double-click on a space, **Then** the contiguous space run is selected.
3. **Given** a double-click on a wide character (or its spacer), **Then** the word includes the wide char and its spacer cell.
4. **Given** a triple-click, **Then** the full line is selected; copied text has trailing spaces trimmed.

### User Story 3 - Clean copied text and stability (Priority: P2)

**Acceptance Scenarios**:

1. **Given** copied lines with trailing blank cells, **Then** trailing spaces are trimmed per line while inner spaces are kept.
2. **Given** an alt-screen switch, **Then** the selection is cleared.
3. **Given** scrollback trimmed to its limit, **Then** selection lines shift without panicking.

### Edge Cases

- Out-of-range columns/rows are clamped; empty columns/rows grids are no-ops.
- A single-cell selection cleared on a no-drag click-release does not leave a phantom selection.
- Shift-click extension, rectangular selection, and copy-on-select are out of scope.

## Requirements

- **FR-001**: Selection storage is `(start_col, start_abs_line, end_col, end_abs_line)`, normalized lexicographically by `(line, col)` at read time. Public selection methods keep taking viewport rows.
- **FR-002**: `viewport_to_abs_line(viewport_row)` converts using the same math as `get_viewport_cell` (alt screen: the row itself; no scroll offset: `scrollback_len + row`; scrolled: `viewport_start + row` clamped to the combined line count).
- **FR-003**: `get_absolute_cell(col, abs_line)` resolves scrollback vs active grid; on the alt screen absolute lines are alt rows.
- **FR-004**: `get_selected_text` iterates absolute lines via `get_absolute_cell`, skips `WIDE_SPACER` cells, trims trailing spaces per line, joins lines with `\n`.
- **FR-005**: Scrollback trims (scroll-push limit and `set_scrollback_limit`) shift selection lines down saturating; alt-screen switches and resize clear the selection.
- **FR-006**: `select_word_at(col, viewport_row)` expands over word chars (alphanumeric or `_-./~:@`; spacers belong to their wide char); a space click selects the space run; other characters fall into their own same-char-kind handling (non-word, non-space characters expand as a run of the same character only when clicked? see FR-007 note).
- **FR-007**: Clicking a character that is neither a word char nor a space selects just that cell (plus its spacer when wide). Neighbours of the same literal character are not gathered.
- **FR-008**: `select_line_at(viewport_row)` selects `0..cols-1` of the absolute line.
- **FR-009**: `window::mouse_press` counts clicks (400 ms, same cell) only on the local-selection path after the 013 hyperlink check and the 008 mouse-reporting check; count 1 = drag start, 2 = word, 3+ = line. `mouse_release` clears a no-drag single-cell selection via `Terminal::clear_selection_if_single_cell(col, viewport_row)`.
- **FR-010**: No public signature changes for renderer/input paths; `selection()` continues to return the stored tuple (now in absolute lines; it is `#[allow(dead_code)]` and unused outside the terminal).

## Success Criteria

- **SC-001**: All 159 pre-existing tests pass (existing selection tests changed only where the trim behaviour requires it, named in evidence); new tests cover every acceptance scenario above.
- **SC-002**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, and `cargo build` pass.

## Assumptions

- Absolute indices are relative to the current scrollback buffer, not a global ever-growing line counter; trimming shifts them explicitly, which keeps the model O(1) and matches `get_viewport_cell` math.
- Word characters match the set common in Unix paths and identifiers (`_-./~:@` plus alphanumerics) so paths and `user@host` select as one word.
