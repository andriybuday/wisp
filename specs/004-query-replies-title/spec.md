# Feature Specification: Query Replies and Window Title

**Feature Branch**: `004-query-replies-title`
**Created**: 2026-10-14
**Status**: Implemented
**Input**: "Query replies and window title"

## User Scenarios & Testing

### User Story 1 - Terminal answers identity and status queries (Priority: P1)

As a terminal user, applications querying the terminal (Device Attributes,
Device Status Reports, text-area size) receive correct replies so TUIs and
shells that probe capabilities initialize correctly instead of hanging or
guessing.

**Independent Test**: Feed the parser byte strings for each query and assert
`take_replies()` returns the exact reply bytes.

**Acceptance Scenarios**:

1. **Given** a fresh parser, **When** `CSI c` is received, **Then** the reply
   is exactly `ESC [ ? 6 c` (VT102, conservative).
2. **Given** a fresh parser, **When** `CSI > c` is received, **Then** the
   reply is exactly `ESC [ > 0 ; 1 ; 0 c`.
3. **Given** a fresh parser, **When** `CSI 5 n` is received, **Then** the
   reply is exactly `ESC [ 0 n`.
4. **Given** the cursor moved with `CSI H`, **When** `CSI 6 n` is received,
   **Then** the reply is `ESC [ {row} ; {col} R` (1-based cursor position).
5. **Given** an 80x24 terminal, **When** `CSI 18 t` is received, **Then** the
   reply is exactly `ESC [ 8 ; 24 ; 80 t`.

### User Story 2 - Shell can set the window title (Priority: P1)

As a terminal user, when a shell or application sets the title via
`OSC 0` / `OSC 2`, the OS window title updates so I can tell windows apart.

**Independent Test**: Feed `OSC 2 ; title BEL` and `OSC 2 ; title ST` and
assert `terminal.title()` returns the decoded title.

**Acceptance Scenarios**:

1. **Given** a fresh parser, **When** `OSC 2 ; My Title BEL` is received,
   **Then** `terminal.title()` is `Some("My Title")`.
2. **Given** a fresh parser, **When** `OSC 2 ; My Title ST` is received,
   **Then** `terminal.title()` is `Some("My Title")`.
3. **Given** `OSC 2` with non-UTF-8 title bytes, **When** parsed, **Then**
   parsing does not panic and the title is decoded lossily.
4. **Given** the terminal title changes, **When** `render()` runs, **Then**
   the window title is updated via `window.set_title`.

### Edge Cases

- Title text: decoded lossily as UTF-8, control characters stripped, length
  capped at 256 chars (shell output is untrusted; Constitution V).
- Replies accumulate internally in the performer and are drained by
  `AnsiParser::take_replies()`; a second drain without new queries is empty.
- PTY write errors for replies are ignored (never panic, never block).
- Malformed or unknown queries produce no reply and no panic.

## Requirements

- **FR-001**: `AnsiPerformer` accumulates pending reply byte strings
  internally; `AnsiParser` exposes `pub fn take_replies(&mut self) ->
  Vec<Vec<u8>>` draining them (no threads/channels). In `window.rs`
  `render()`, after `parser.advance`, every drained reply is written to the
  PTY, ignoring write errors.
- **FR-002**: Primary DA — `CSI c` with no intermediate `>` or `=` replies
  exactly `ESC [ ? 6 c` (VT102; conservative per Constitution II — never
  advertise unimplemented features). Secondary DA `CSI > c` replies
  `ESC [ > 0 ; 1 ; 0 c`. Other intermediates are ignored.
- **FR-003**: DSR — `CSI 5 n` replies `ESC [ 0 n`; `CSI 6 n` replies
  `ESC [ {row} ; {col} R` with the terminal cursor converted to 1-based.
  Other DSR params are ignored.
- **FR-004**: `CSI 18 t` replies `ESC [ 8 ; {rows} ; {cols} t`. All other
  `CSI ... t` window manipulations are ignored; this exclusion is explicit
  scope for this feature (no move/resize/iconify etc. is implemented).
- **FR-005**: OSC — in `osc_dispatch`, `params[0]` of `b"0"` or `b"2"` sets
  the title from `params[1]`, decoded lossily as UTF-8 with control
  characters stripped and length capped at 256 chars; all other OSC is
  ignored. `Terminal` stores `title: Option<String>` with
  `title() -> Option<&str>` and `set_title()`. `window.rs` tracks the last
  applied title and calls `window.set_title` when the terminal title
  changes (done in `render()`).
- **FR-006**: No other CSI behavior changes. All 33 pre-existing tests still
  pass; new `#[cfg(test)]` tests cover every new behavior.

## Success Criteria

- **SC-001**: Parser tests assert `take_replies()` contents for primary DA,
  secondary DA, DSR 5, DSR 6 (after `CSI H`), and `CSI 18 t` (80x24).
- **SC-002**: Parser tests assert `terminal.title()` for `OSC 2` terminated
  by BEL and by ST, and non-UTF-8 title bytes do not panic.
- **SC-003**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings
  -A deprecated`, `cargo test`, and `cargo build` all pass.
