# Feature Specification: 256-Color and Truecolor Support

**Feature Branch**: `003-color-256-truecolor`
**Created**: 2026-10-14
**Status**: Implemented
**Input**: "256-color and truecolor support"

## User Scenarios & Testing

### User Story 1 - 256-color and truecolor output renders correctly (Priority: P1)

As a terminal user, programs emitting SGR `38;5;n` / `48;5;n` (256-color) and
`38;2;r;g;b` / `48;2;r;g;b` (truecolor) display their intended colors instead
of being ignored or misinterpreted as simple SGR codes.

**Independent Test**: Feed SGR sequences into the parser and assert cell
`Color` values; assert `ColorPalette::ansi_256` resolution for cube/gray.

**Acceptance Scenarios**:

1. **Given** default state, **When** `ESC[38;5;196m` + char arrives, **Then** the cell fg is `Color::Indexed(196)`.
2. **Given** default state, **When** `ESC[38:5:196m` (colon form) arrives, **Then** the cell fg is `Color::Indexed(196)`.
3. **Given** default state, **When** `ESC[48;2;10;20;30m` arrives, **Then** the cell bg is `Color::Rgb(10,20,30)`; colon form `38:2::10:20:30` behaves the same.
4. **Given** existing 16-color output, **When** SGR 30-37/90-97/40-47/100-107/39/49/0 arrive, **Then** behavior is unchanged except colors are `Indexed`/`Default`.

### User Story 2 - Malformed extended colors are harmless (Priority: P1)

As a user, truncated sequences such as `38;5` or `38;2;1` never panic and
never corrupt the current color.

**Acceptance Scenarios**:

1. **Given** fg set to red, **When** `ESC[38;5m` arrives, **Then** fg is unchanged and no panic occurs.
2. **Given** fg set to red, **When** `ESC[38;2;1m` arrives, **Then** fg is unchanged and no panic occurs.

### Edge Cases

- RGB parameters arrive as `u16` from vte and are clamped to 255.
- Extended sequences may be mixed with simple codes in one SGR (`1;38;5;200m`).
- Bold brightens only `Indexed(0..=7)`; inverse swaps resolved RGBA colors.

## Requirements

- **FR-001**: Add `pub enum Color { Default, Indexed(u8), Rgb(u8,u8,u8) }` in `terminal.rs`; `Cell.fg/bg` and `current_fg/current_bg` become `Color`; `Cell::default` is `Default`/`Default`.
- **FR-002**: Simple SGR keeps working: 30-37/90-97 fg `Indexed(0-15)`, 40-47/100-107 bg `Indexed`, 39/49 `Default`, 0 resets colors to `Default`. Keep `Terminal::sgr(usize)` and add `set_fg_color`/`set_bg_color(Color)`.
- **FR-003**: Rewrite SGR parsing in `parser.rs` as stateful iteration over vte `Params` handling `38;5;n`, `48;5;n`, `38;2;r;g;b`, `48;2;r;g;b` in both semicolon and colon (subparam) forms; truncated/malformed sequences are consumed safely, no panic, color unchanged. No other CSI handlers change.
- **FR-004**: Add `ColorPalette::ansi_256(u8) -> [f32;4]` in `config.rs`: 0-15 use the ansi table, 16-231 use the 6x6x6 cube (steps 0,95,135,175,215,255 over 255), 232-255 use gray `(8+10*(n-232))/255`. Remove `#[allow(dead_code)]` from `foreground` and use it for `Default` fg; `Default` bg resolves to `background`.
- **FR-005**: Renderer resolves `Color` to RGBA; draws the background quad when `bg != Default || inverse || selected`; bold brightens only `Indexed(0..=7)` by +8; inverse swaps resolved colors.
- **FR-006**: Update 001-era SGR tests to the `Color` API and add tests for semicolon/colon 256-color, truecolor fg/bg, malformed sequences, and cube/grayscale resolution.

## Success Criteria

- **SC-001**: All 33 pre-existing tests pass (SGR tests updated for `Color` only) plus new color tests.
- **SC-002**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build` all pass.
