# Feature Specification: Complete Keyboard Modifier Input

**Feature Branch**: `005-keyboard-modifiers`
**Created**: 2026-10-14
**Status**: Implemented
**Input**: "Complete keyboard modifier input"

## User Scenarios & Testing

### User Story 1 - Ctrl combinations reach the shell (Priority: P1)

As a terminal user, Ctrl+C/Ctrl+D/Ctrl+L and friends send the traditional
control bytes (1-26 and friends) instead of literal letters, so shell line
editing, process signalling, and full-screen apps work.

**Independent Test**: unit tests call `key_to_bytes` with CONTROL and assert
bytes for lower/upper-case letters, Space, and `]`.

**Acceptance Scenarios**:

1. **Given** Ctrl held, **When** `c` or `C` is pressed, **Then** byte 3 is sent.
2. **Given** Ctrl held, **When** Space is pressed, **Then** byte 0 is sent.
3. **Given** Ctrl held, **When** `]` is pressed, **Then** byte 29 is sent.
4. **Given** no modifiers, **When** a plain key is pressed, **Then** output is byte-identical to feature 001 behaviour.

### User Story 2 - Modified navigation and function keys (Priority: P1)

As a user of vim/tmux/htop, modified arrows, Home/End, Insert/Delete/PgUp/PgDn
and F1-F12 emit xterm CSI sequences carrying the modifier parameter, so apps
can distinguish e.g. Ctrl+Right (word jump) from Right.

**Independent Test**: unit tests assert F1/F5/F12 sequences and
Ctrl+Shift+Right → `CSI 1;6C` (m = 1+1+4 = 6).

**Acceptance Scenarios**:

1. **Given** Ctrl+Shift held, **When** Right is pressed, **Then** `ESC [ 1 ; 6 C` is sent.
2. **Given** no modifiers, **When** F1/F5/F12/Insert is pressed, **Then** `ESC O P` / `CSI 15~` / `CSI 24~` / `CSI 2~` is sent.
3. **Given** Shift held, **When** Tab is pressed, **Then** `CSI Z` (backtab) is sent.

### User Story 3 - Alt as Meta, Cmd never leaks (Priority: P2)

As a user, Alt+key sends ESC-prefixed bytes (Meta) for emacs/readline, while
Cmd (Super) combinations are reserved for the OS/app and never reach the shell.

**Acceptance Scenarios**:

1. **Given** Alt held, **When** `x` is pressed, **Then** `ESC x` is sent.
2. **Given** Super held, **When** any key is pressed, **Then** nothing is sent (`None`).

### Edge Cases

- Unmodified forms of every pre-existing key stay byte-identical to today; plain Backspace keeps emitting `0x7f`.
- Ctrl wins over the plain text path (a Ctrl+letter never leaks the letter).
- **Application cursor keys (DECCKM) are explicitly out of scope**: arrows always emit normal-mode CSI sequences regardless of any future mode flag.
- Keypad application mode, Kitty keyboard protocol, and modifyOtherKeys are out of scope.

## Requirements

- **FR-001**: `key_to_bytes` MUST accept `winit` `ModifiersState` by value as a new parameter; the single `main.rs` call site MUST pass the tracked `modifiers` variable; 001-era input tests MUST be updated to the new signature. If `modifiers.super_key()`, return `None`.
- **FR-002**: Modifier parameter `m = 1 + (shift?1:0) + (alt?2:0) + (ctrl?4:0)`. For `m>1`: arrows/Home/End emit `CSI 1;{m}A/B/C/D/H/F`; Insert/Delete/PageUp/PageDown emit `CSI 2;{m}~ / 3;{m}~ / 5;{m}~ / 6;{m}~`. Unmodified forms byte-identical to today; Insert added as `CSI 2~`.
- **FR-003**: F1-F4: `ESC O P/Q/R/S` unmodified, `CSI 1;{m}P/Q/R/S` modified. F5-F12: `CSI 15/17/18/19/20/21/23/24 ~`, with `;{m}` inserted before `~` when modified.
- **FR-004**: Shift+Tab MUST emit `CSI Z`, checked before plain Tab.
- **FR-005**: Ctrl combos: letters a-z (case-insensitive, from text or logical key) → 1-26; Space/`2` → 0; `[`/`3` → 27; `\`/`4` → 28; `]`/`5` → 29; `^`/`6` → 30; `_`/`7` → 31; `?`/`8`/Ctrl+Backspace → 127. Ctrl MUST win over the plain text path. Plain Backspace stays `0x7f`.
- **FR-006**: Alt (without Ctrl) on printable text MUST prefix ESC to the text bytes; Alt on Enter/Escape MUST prefix ESC to the base byte. Other special keys use the modified CSI forms of FR-002/FR-003.
- **FR-007**: Only `src/input.rs` and the single `key_to_bytes` call site in `src/main.rs` may change (plus `specs/005-keyboard-modifiers/`). Terminal/parser/renderer untouched.

## Success Criteria

- **SC-001**: All 33 pre-existing tests still pass (input tests updated only for the new signature and the newly-mapped F1 key), plus new tests covering: ctrl+letter (upper+lower), ctrl+space, ctrl+], alt+char, F1, F5, F12, ctrl+shift+Right (m=6), shift+tab, insert, super→None, plain keys unchanged.
- **SC-002**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, and `cargo build` all pass.

## Assumptions

- Modifier encoding follows xterm/ECMA-48 conventions (constitution Principle II).
- `main.rs` already tracks `ModifiersState` from `ModifiersChanged` events and already intercepts Cmd/Ctrl+Shift+C/V for copy/paste before calling `key_to_bytes`; the Super→None rule in `key_to_bytes` is defence in depth so Cmd combos can never leak to the shell through any future call path.
