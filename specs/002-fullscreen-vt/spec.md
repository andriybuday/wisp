# Feature Specification: Full-screen VT Support

**Feature Branch**: `002-fullscreen-vt`
**Created**: 2026-10-14
**Status**: Implemented
**Input**: "Full-screen VT support"

## User Scenarios & Testing

### User Story 1 - Full-screen apps use alternate screen (Priority: P1)

As a user running vim/less/htop, the app switches to a clean alternate
screen and on exit my shell screen and cursor return exactly as before.

**Independent Test**: Feed `?1049h` via parser, print, `?1049l`; assert main
grid/cursor restored, alternate content gone. Same for `?47`/`?1047`
(no cursor save; `?1047` exit clears alternate).

**Acceptance Scenarios**:

1. **Given** text on main screen, **When** ?1049 set, print, reset, **Then** main text and cursor restored.
2. **Given** alternate active, **When** output scrolls, **Then** scrollback does not grow and viewport ignores scroll_offset.

### User Story 2 - Scroll regions and index operations (Priority: P1)

As a user, split-screen curses apps scroll only their region (DECSTBM),
with region-aware IND/RI, insert/delete lines and scroll up/down.

**Independent Test**: Set region rows 2–4, linefeed at bottom margin;
assert only region scrolled and lines outside untouched.

**Acceptance Scenarios**:

1. **Given** region set, **When** linefeed at bottom margin, **Then** only region scrolls.
2. **Given** cursor below region, **When** linefeed, **Then** cursor clamps at last row without scrolling.

### User Story 3 - Editing and cursor extras (Priority: P2)

As a user, line editing works: insert/delete/erase chars, CNL/CPL/CHA/VPA,
save/restore cursor (ESC 7/8, CSI s/u), ESC D/M/E, and hide/show cursor (?25).

**Acceptance Scenarios**:

1. **Given** a line of text, **When** CSI @/P/X applied, **Then** chars shift/erase as in xterm.
2. **Given** saved cursor with red bold attrs, **When** restored, **Then** position and attrs return.

### Edge Cases

- Invalid DECSTBM params (0, reversed, out of range) mean full screen.
- Oversized counts (e.g. insert 9999 lines) clamp, never panic.
- Unrecognized sequences consumed and ignored.
- resize resizes both grids and clamps the scroll region.

## Requirements

- **FR-001**: DECSET/DECRST ?1049 (save cursor, switch+clear alt; switch back, restore cursor), ?1047/?47 (switch without cursor save; ?1047 exit clears alt, enter does not clear). Second grid in `Terminal`; get_cell/print/scroll use active grid. Alternate scrolling MUST NOT push scrollback; `get_viewport_cell` ignores scroll_offset on alternate.
- **FR-002**: CSI Pt;Pb r DECSTBM (1-based inclusive; invalid = full screen; cursor → 0,0). Linefeed at bottom margin scrolls only region; below region clamps; scrollback only from full-height region on main screen.
- **FR-003**: ESC D (IND), ESC M (RI, region-aware), ESC E (NEL) via `esc_dispatch`.
- **FR-004**: Region-aware CSI L/M, line-local CSI @/P/X, region-aware CSI S/T.
- **FR-005**: ESC 7/8 and CSI s/u save/restore cursor position + fg/bg/flags.
- **FR-006**: ?25 wired to `set_cursor_visible`.
- **FR-007**: CSI G (CHA), CSI d (VPA), CSI E/F (CNL/CPL with column 0).
- **FR-008**: `resize()` resizes both grids, clamps scroll region.
- **FR-009**: No panics on parser input (saturating/clamped indexing). Files: `src/terminal.rs`, `src/parser.rs` only.

## Out of Scope

Origin mode (?6), DECCKM (?1), mouse reporting. Color/SGR changes,
input/window/renderer changes.

## Success Criteria

- **SC-001**: All 33 pre-existing tests pass; new tests cover every FR.
- **SC-002**: fmt, clippy `-D warnings -A deprecated`, test, build pass.
