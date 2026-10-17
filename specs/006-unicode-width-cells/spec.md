# Feature Specification: Unicode Width-Aware Cells

**Feature Branch**: `006-unicode-width-cells`
**Created**: 2026-10-17
**Status**: Implemented
**Input**: "Unicode width-aware cells"

## User Scenarios & Testing

### User Story 1 - Wide characters occupy two cells (Priority: P1)

As a terminal user printing CJK text or emoji, each wide character occupies
two grid columns and advances the cursor by two, so following text, cursor
position reports, and line wrapping line up with what applications (which
compute widths with the same Unicode tables) expect.

**Independent Test**: unit tests print U+754C (界) and U+1F980 (🦀) into a
`Terminal` and assert the head cell, the spacer cell, and the cursor.

**Acceptance Scenarios**:

1. **Given** an empty grid, **When** 界 is printed, **Then** cell 0 holds 界, cell 1 is a spacer (`' '` + `WIDE_SPACER`), and the cursor is at column 2.
2. **Given** the cursor in the last column, **When** a width-2 char is printed, **Then** the terminal wraps to the next line first (same linefeed path) and writes the char at column 0.
3. **Given** a width-2 char written at the second-to-last column, **When** the cursor advances by 2 past the end, **Then** today's immediate-wrap semantics apply (cursor moves to the next line start right away).

### User Story 2 - Zero-width characters are dropped (Priority: P1)

As a user whose output contains zero-width characters (e.g. U+200B), they
leave the grid and cursor untouched instead of occupying a misleading cell.

**Acceptance Scenarios**:

1. **Given** `a` printed, **When** U+200B is printed, **Then** no cell changes and the cursor does not advance; a following `b` lands immediately after `a`.

### User Story 3 - Overwrites never leave half a wide char (Priority: P1)

As a user of full-screen apps that redraw cells, writing over either half of
a wide pair blanks the other half, so no orphan spacer or dangling wide glyph
remains on screen.

**Acceptance Scenarios**:

1. **Given** a wide pair, **When** a width-1 char is written over the spacer, **Then** the wide char to its left is blanked.
2. **Given** a wide pair, **When** a width-1 char is written over the wide char, **Then** its spacer is blanked.
3. **Given** a blanked cell, **Then** it holds `' '` without `WIDE_SPACER` but keeps its colors.

### User Story 4 - Copied text has no inserted spaces (Priority: P2)

As a user copying a line mixing ASCII and wide chars, the selection text
contains exactly the printed characters, with no space contributed by spacer
cells.

**Acceptance Scenarios**:

1. **Given** `a界b` on a line, **When** the whole line is selected, **Then** `get_selected_text` returns `a界b`.

### Edge Cases

- Width-1 characters behave exactly as before this feature, including immediate wrap at the last column.
- The spacer cell copies the wide cell's fg/bg/flags, so backgrounds and inverse video render continuously across both columns.
- A 1-column terminal cannot hold a wide pair; the char degrades to a single cell without panicking or indexing out of bounds.

### Explicit Non-Goals

- **Combining marks and ZWJ sequences do not compose**: width-0 characters (combining accents, zero-width joiner/space, etc.) are dropped entirely rather than composed onto the previous cell. Grapheme-cluster rendering is a possible later feature, not this one.
- **Deferred (pending) wrap** is not implemented; today's immediate-wrap semantics are kept. Deferred wrap is a later feature.
- **Glyph coverage is out of scope**: whether the embedded font has a glyph for a given wide char is unchanged; no fonts are added and there is no font fallback. Only grid/cursor width accounting changes.

## Requirements

- **FR-001**: The `unicode-width` crate (version 0.2) MUST be added as a dependency; `Terminal::print` MUST compute each char's column width with `UnicodeWidthChar::width`, treating `None` as 0.
- **FR-002**: Width-0 chars MUST be dropped entirely: no cell change, no cursor advance.
- **FR-003**: Width-1 chars MUST work exactly as before (write cell with current fg/bg/flags, advance by 1, immediate wrap via the existing linefeed path at the end of the line).
- **FR-004**: Width-2 chars MUST write the char in the cursor cell and a spacer in the next cell, then advance the cursor by 2 with the same immediate-wrap semantics when the cursor reaches the end.
- **FR-005**: If the cursor is in the last column when a width-2 char arrives, `print` MUST wrap to the next line first (same linefeed path), then write the pair.
- **FR-006**: The spacer MUST be represented by a new `CellFlags` bit `WIDE_SPACER` (`0b0001_0000`); the spacer cell MUST hold `ch == ' '` and copy the wide cell's fg/bg/flags (plus `WIDE_SPACER`) so backgrounds and inverse render continuously.
- **FR-007**: Overwrite hygiene: writing into a cell that is a spacer MUST blank the wide char to its left; writing a width-1 char over a wide char MUST blank its spacer. Blanking sets `ch` to `' '` and removes `WIDE_SPACER` but keeps colors.
- **FR-008**: `get_selected_text` MUST skip spacer cells so copied text contains no inserted spaces.
- **FR-009**: The renderer MUST NOT need a glyph change: a spacer's `ch == ' '` means the existing background fill runs and the glyph pass is already skipped (verified by reading `build_vertices`).

## Success Criteria

- **SC-001**: All 89 pre-existing tests still pass, plus new tests covering: CJK char occupies two cells with a spacer and advances by 2 (including copied fg/bg/flags); emoji is width 2; wide char at the last column wraps; overwriting the spacer blanks the wide char and vice versa (keeping colors); U+200B changes nothing; mixed ASCII+wide selection copies without extra spaces; UTF-8 CJK bytes fed through `AnsiParser` land in the expected cells.
- **SC-002**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, and `cargo build` all pass.

## Assumptions

- Column widths follow Unicode East Asian Width / wcwidth semantics as implemented by `unicode-width` 0.2 (constitution Principle II: match xterm behaviour, which uses the same tables).
- `vte` already decodes UTF-8 from the PTY into `char`s for `Perform::print`, so no parser change is needed beyond tests; all width logic lives in `Terminal::print`.
