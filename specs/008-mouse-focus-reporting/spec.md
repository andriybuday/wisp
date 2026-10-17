# Feature Specification: SGR Mouse and Focus Reporting

**Feature Branch**: `008-mouse-focus-reporting`
**Created**: 2026-10-17
**Status**: Implemented
**Input**: "SGR mouse and focus reporting"

## User Scenarios & Testing

### User Story 1 - Mouse reporting for full-screen apps (Priority: P1)

As a user of mouse-aware programs (vim, tmux, htop, less), clicks, drags, and
wheel events are reported to the application in xterm SGR (or legacy) mouse
encoding whenever the application has enabled mouse tracking, instead of
being swallowed by local text selection.

**Independent Test**: encoder unit tests assert exact bytes for SGR/legacy
press, release, motion, wheel, and modifier combinations; parser tests assert
the DEC private modes flip the terminal flags.

**Acceptance Scenarios**:

1. **Given** an app sent `CSI ? 1000 h` and `CSI ? 1006 h`, **When** the user presses the left button at cell (0,0), **Then** the PTY receives `ESC [ < 0 ; 1 ; 1 M` and no local selection starts.
2. **Given** tracking is on without `?1006`, **When** the button is released, **Then** the legacy form `ESC [ M` + three offset bytes is sent, with button value 3.
3. **Given** tracking is on, **When** the wheel scrolls up, **Then** button 64 is reported at the cursor cell and the local viewport does not scroll; wheel down reports 65.
4. **Given** `?1002` (button-event) mode, **When** the cursor moves with a button held, **Then** motion reports (button + 32) are sent; **Given** `?1003` (any-event), motion is always reported, with 35 when no button is held.
5. **Given** tracking is on but Shift is held, **When** the user drags or scrolls, **Then** today's local selection/scroll behavior applies unchanged (the standard user override).

### User Story 2 - Focus in/out reporting (Priority: P2)

As a user of apps that request focus reporting (`?1004`, e.g. editors that
dim UI or pause on blur), gaining focus sends `ESC [ I` and losing focus
sends `ESC [ O` to the application.

**Acceptance Scenarios**:

1. **Given** `CSI ? 1004 h`, **When** the window gains/loses focus, **Then** `ESC [ I` / `ESC [ O` is written to the PTY; when the mode is off nothing is sent.

### Edge Cases

- Mouse coordinates clamp into the grid for reporting: an event in the padding or past the last cell reports the nearest edge cell instead of being dropped.
- Legacy encoding clamps each byte to 255, so large grids cannot overflow a byte.
- Mode precedence: `?1000`/`?1002`/`?1003` form one tracking level (Off < Normal < ButtonEvent < AnyEvent). Setting a higher mode replaces a lower one; setting a lower mode never downgrades a higher one; resetting the active mode drops tracking to Off (no fall-back to a previously set lower mode).
- Motion is reported once per cell change (xterm behavior), not per pixel event.

### Explicit Non-Goals

- No UTF-8 (`?1005`) or URXVT (`?1015`) mouse encodings; SGR (`?1006`) and legacy only.
- No change to Device Attributes replies: mouse support is not advertised via DA (xterm does not advertise it there either).
- Pixel-position (`?1016`, SGR-Pixels) reporting is out of scope.

## Requirements

- **FR-001**: `Terminal` MUST store mouse tracking as a `MouseTracking` enum (`Off`/`Normal`/`ButtonEvent`/`AnyEvent`), an `sgr_mouse` flag (`?1006`), and a `focus_reporting` flag (`?1004`), all defaulting to off, with getters for each.
- **FR-002**: The parser's DEC private mode handler MUST wire `?1000`, `?1002`, `?1003`, `?1004`, and `?1006` for both set (`h`) and reset (`l`).
- **FR-003**: `input.rs` MUST provide a pure encoder `encode_mouse(button: u32, col: usize, row: usize, pressed: bool, sgr: bool) -> Vec<u8>`. SGR form: `ESC [ < {button} ; {col+1} ; {row+1}` + `M` (press) or `m` (release). Legacy form: `ESC [ M` + three bytes, each `value + 32` clamped to 255, using button 3 for release.
- **FR-004**: Button codes MUST follow xterm: left 0, middle 1, right 2, wheel up 64, wheel down 65, motion adds 32, modifiers add Shift 4 / Alt 8 / Ctrl 16; no-button motion uses 35.
- **FR-005**: While tracking is not Off and Shift is not held, press/release/wheel/motion (per tracking level) MUST be reported to the PTY and MUST NOT start selection or scroll the viewport. While Shift is held or tracking is Off, existing selection/scroll behavior MUST apply unchanged.
- **FR-006**: Reported coordinates MUST clamp into the grid (a clamped helper alongside `position_to_grid`), including events `position_to_grid` would return `None` for.
- **FR-007**: `main.rs` MUST pass the current `ModifiersState` and the set of pressed mouse buttons through to the window handlers, and forward `WindowEvent::Focused` so focus events send `ESC [ I` / `ESC [ O` when `?1004` is enabled.

## Success Criteria

- **SC-001**: All 98 pre-existing tests still pass, plus new tests covering: SGR press/release bytes, legacy press/release bytes (release = button 3), motion +32, wheel 64/65, modifier bits 4/8/16, legacy clamping at 255, and parser set/reset of `?1000`/`?1002`/`?1003`/`?1006`/`?1004` including precedence (higher replaces lower, lower does not downgrade, resetting the active mode drops to Off).
- **SC-002**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, and `cargo build` all pass.

## Assumptions

- Encodings and mode numbers follow xterm's control sequences documentation (constitution Principle II).
- One wheel report per `MouseWheel` event, matching the existing one-line scroll step for typical deltas.
