# Feature Specification: Dim, Strikethrough and Underline Styles

**Feature Branch**: `012-text-attributes`
**Created**: 2026-10-17
**Status**: Implemented
**Input**: "Dim, strikethrough and underline styles"

## User Scenarios & Testing

### User Story 1 - Dim and strikethrough text renders as intended (Priority: P1)

As a terminal user, programs emitting SGR 2 (dim/faint) and SGR 9
(strikethrough) display dimmed or struck-through text instead of plain text.

**Independent Test**: Feed SGR sequences into the parser and terminal, assert
`Cell.flags` contains `DIM` / `STRIKETHROUGH` on printed cells.

**Acceptance Scenarios**:

1. **Given** default state, **When** `ESC[2m` + char arrives, **Then** the cell has `DIM`.
2. **Given** default state, **When** `ESC[9m` + char arrives, **Then** the cell has `STRIKETHROUGH`.
3. **Given** bold+dim set, **When** SGR 22 arrives, **Then** both `BOLD` and `DIM` are cleared (ECMA-48 normal intensity).
4. **Given** strikethrough set, **When** SGR 29 arrives, **Then** `STRIKETHROUGH` is cleared.
5. **Given** dim+strikethrough set, **When** SGR 0 arrives, **Then** both are cleared.

### User Story 2 - Underline styles and underline color do not corrupt state (Priority: P1)

As a user, programs emitting colon underline styles (`4:1`..`4:5`) get an
underline, `4:0` clears it, and underline-color sequences (SGR 58) are
consumed without changing fg/bg or leaking their operands as SGR codes.

**Independent Test**: Feed `ESC[4:2m`, `ESC[4:0m`, `ESC[58;5;196m`,
`ESC[58:2::255:0:0m` through the parser and assert flags and colors.

**Acceptance Scenarios**:

1. **Given** default state, **When** `ESC[4:2m` (double underline) + char arrives, **Then** the cell has `UNDERLINE` and not `DIM` (sub-style `2` is not re-applied as SGR 2).
2. **Given** underline set, **When** `ESC[4:0m` arrives, **Then** `UNDERLINE` is cleared.
3. **Given** fg/bg at non-default values, **When** `ESC[58;5;196m` or `ESC[58:2::255:0:0m` arrives, **Then** fg/bg are unchanged.

### Edge Cases

- Blink (SGR 5, 6) is parsed and deliberately ignored: no flag, no rendering
  change. Documented non-goal (minimal terminal; blinking text is a
  distraction and xterm allows disabling it).
- Double/curly/dotted/dashed underline styles (`4:2`–`4:5`) all render as the
  existing single underline. Documented simplification.
- Underline color (SGR 58, semicolon and colon forms) is consumed and ignored;
  underlines keep the (dimmed) foreground color.
- Truncated/malformed 58 sequences never panic and never change fg/bg.

## Requirements

- **FR-001**: `CellFlags` in `terminal.rs` gains `DIM = 0b0010_0000` and `STRIKETHROUGH = 0b0100_0000`; existing bits including `WIDE_SPACER` are unchanged.
- **FR-002**: `Terminal::sgr`: 2 sets `DIM`; 22 clears `BOLD` and `DIM`; 9 sets `STRIKETHROUGH`; 29 clears it; 5/6 parsed and ignored; 0 (via `reset_sgr`) clears both.
- **FR-003**: Parser SGR walker in `parser.rs`: a colon-subparam sequence starting with 4 sets underline for any sub-style except `4:0` which clears underline, and the sub-style number is never fed back as its own SGR code. Plain `4` keeps meaning underline.
- **FR-004**: Parser consumes SGR 58 (underline color) in semicolon forms (`58;5;n`, `58;2;r;g;b`) and colon forms (`58:...`), reusing the 38/48 skipping logic, ignoring it without affecting fg/bg. No other CSI handlers change.
- **FR-005**: Renderer `build_vertices`: `DIM` scales resolved foreground RGB by 0.6 (alpha unchanged); `STRIKETHROUGH` draws a 2px horizontal quad at the cell's vertical middle in the (dimmed) fg color, mirroring the underline quad; the underline quad also uses dimmed fg when `DIM` is set.

## Success Criteria

- **SC-001**: All 124 pre-existing tests still pass, except any test pinning bold-only clearing for SGR 22 is updated to the new bold+dim semantics (named in evidence).
- **SC-002**: New terminal and parser tests cover every decision above; renderer dim/strikethrough is manual-check only.
- **SC-003**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build` all pass.
