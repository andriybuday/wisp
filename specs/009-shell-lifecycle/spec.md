# Feature Specification: Shell Lifecycle and Window Close on Exit

**Feature Branch**: `009-shell-lifecycle`
**Created**: 2026-10-17
**Status**: Implemented
**Input**: "Shell lifecycle and window close on exit"

## User Scenarios & Testing

### User Story 1 - Window closes when the shell exits (Priority: P1)

As a terminal user who types `exit` (or whose shell otherwise terminates),
the Wisp window closes instead of lingering as a dead window that accepts
no input.

**Independent Test**: unit tests spawn `/bin/sh -c exit 0` through the
internal PTY helper and poll `has_exited` until true (2s deadline); the
window-close wiring in `main.rs`/`window.rs` is a one-line delegation
checked by reading and building.

**Acceptance Scenarios**:

1. **Given** a running shell, **When** the shell process exits, **Then** `PtyManager::has_exited()` returns true, `WispWindow::shell_exited()` delegates it, and `main.rs` calls `elwt.exit()` from `Event::AboutToWait`.
2. **Given** a still-running shell (`sleep 5`), **When** `has_exited()` is polled immediately, **Then** it returns false and the window stays open.

### User Story 2 - Shell can identify its terminal (Priority: P2)

As a shell or tool inspecting its environment, `TERM_PROGRAM=Wisp` and
`TERM_PROGRAM_VERSION` (from the crate version) identify the host terminal,
following the convention used by other terminals.

**Acceptance Scenarios**:

1. **Given** a shell spawned by Wisp, **When** it reads its environment, **Then** `TERM_PROGRAM` is `Wisp` and `TERM_PROGRAM_VERSION` equals `env!("CARGO_PKG_VERSION")`, alongside the existing `TERM=xterm-256color`.

### Edge Cases

- Reader-thread EOF alone must NOT count as exit: only the child process actually terminating (`try_wait()` returning `Ok(Some(_))`) counts. EOF can precede process reaping, and tying close to EOF could race.
- `try_wait()` errors count as not exited (and are logged), so a transient wait error never closes a live window.
- Repeated `has_exited()` calls after exit keep returning true (`std::process::Child::try_wait` caches the status).
- Closing the window while the shell still runs is unchanged: dropping the PTY master SIGHUPs the shell (see Non-Goals).

### Explicit Non-Goals

- **No Drop-time SIGKILL of the shell**: closing the master side of the PTY already delivers SIGHUP to the foreground process group; adding a kill in `Drop` would be redundant and risk killing a reused PID in edge cases.
- **No "press key to close" prompt**: the window closes immediately on shell exit, matching xterm-style behaviour; no confirmation UI.
- **Exit code is not surfaced in the UI**: the status from `try_wait()` is used only as a boolean; no title/message shows it.

## Requirements

- **FR-001**: `PtyManager` MUST retain the spawned child as `Box<dyn portable_pty::Child + Send>` instead of dropping the handle returned by `spawn_command`.
- **FR-002**: `PtyManager` MUST gain `pub fn has_exited(&mut self) -> bool` implemented via `child.try_wait()`: `Ok(Some(_))` → true, `Ok(None)` → false, `Err(e)` → false with the error logged. Reader-thread EOF MUST NOT factor into the result.
- **FR-003**: Spawning MUST be refactored into an internal helper taking a `portable_pty::CommandBuilder` (plus cols/rows), so tests — and a later shell-override feature — can spawn a custom command. `PtyManager::new(cols, rows)` MUST keep its exact signature and behaviour, building the user's shell (`$SHELL`, default `/bin/bash`) and delegating to the helper. No constructor parameters are added.
- **FR-004**: The spawned command's environment MUST include `TERM_PROGRAM=Wisp` and `TERM_PROGRAM_VERSION=env!("CARGO_PKG_VERSION")`, in addition to the existing `TERM=xterm-256color`.
- **FR-005**: `WispWindow` MUST gain `pub fn shell_exited(&mut self) -> bool` delegating to the PTY manager.
- **FR-006**: `main.rs` MUST check `shell_exited()` in `Event::AboutToWait` and call `elwt.exit()` when true, as the smallest possible insertion; `ControlFlow` handling MUST NOT be touched.

## Success Criteria

- **SC-001**: All 98 pre-existing tests still pass, plus new PTY tests: `/bin/sh -c exit 0` polls to `has_exited() == true` within 2s; `/bin/sh -c sleep 5` reports `has_exited() == false` immediately (child then killed); a spawned child sees `TERM_PROGRAM`/`TERM_PROGRAM_VERSION`/`TERM` in its environment. No pre-existing test is modified.
- **SC-002**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, and `cargo build` all pass.

## Assumptions

- `/bin/sh` exists on test hosts (macOS/Linux CI); tests are gated naturally by using it only in `#[cfg(test)]` and must bound all waiting so they cannot hang CI.
- Polling `has_exited()` in `AboutToWait` is sufficient because the event loop currently uses `ControlFlow::Poll` (a parallel feature reworking `AboutToWait` keeps the check reachable); no wakeup channel is added here.
