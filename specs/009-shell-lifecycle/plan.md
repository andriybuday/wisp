# Implementation Plan: Shell Lifecycle and Window Close on Exit

**Branch**: `009-shell-lifecycle` | **Date**: 2026-10-17 | **Spec**: [spec.md](./spec.md)

## Summary

Retain the PTY child handle in `PtyManager`, expose a non-blocking
`has_exited()` via `try_wait()`, delegate it as `WispWindow::shell_exited()`,
and exit the event loop from `AboutToWait` when the shell is gone. Spawning
is refactored into a `CommandBuilder`-taking helper (`spawn_with_command`,
`pub(crate)`) so tests and a later shell-override feature can reuse it
without changing `PtyManager::new`'s signature. The shell environment gains
`TERM_PROGRAM`/`TERM_PROGRAM_VERSION`. Non-goals (no Drop SIGKILL, no
close prompt, no exit-code UI) are recorded in spec.md.

## Technical Context

- **Language/Version**: Rust, edition 2021
- **Primary Dependencies**: existing `portable-pty` 0.8 (`Child::try_wait`, `ChildKiller::kill`, `CommandBuilder::env`); no new dependencies
- **Testing**: `cargo test`; new `#[cfg(test)]` module in `src/pty.rs` spawning real (but trivial and bounded) `/bin/sh` commands through the helper
- **Target Platform**: macOS primary; `try_wait`/`kill` are portable-pty cross-platform APIs, tests rely on `/bin/sh` (Unix test hosts)
- **Constraints**: changes confined to `src/pty.rs`, one accessor in `src/window.rs`, the smallest `AboutToWait` insertion in `src/main.rs`, and `specs/009-shell-lifecycle/`; `ControlFlow` untouched; `PtyManager::new(cols, rows)` signature unchanged (parallel features touch pty wakeups via a setter and will reuse the spawn helper for a shell override)

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS. Closing on shell exit is daily-essential terminal behaviour (a window whose shell died is unusable). The change is ~30 lines in the PTY layer plus two delegations; no new dependency, module, config, or UI. The explicit non-goals (no prompt, no exit-code display, no Drop kill) keep it minimal.
- **II. Terminal Correctness over Feature Count** — PASS. Exit is detected from the child process itself (`try_wait`), not from reader EOF, matching how terminals distinguish "no more output" from "session ended". `TERM_PROGRAM`/`TERM_PROGRAM_VERSION` follow the established cross-terminal convention and advertise only identity, no unimplemented capability; `TERM` and DA replies are untouched.
- **III. Test-First for the Terminal Core (NON-NEGOTIABLE)** — PASS with note. The terminal core (`terminal`/`parser`/`input`) is untouched. PTY code is normally exempt from unit tests under this principle, but the new lifecycle logic is pure process-state polling and is cheaply testable headlessly (real `/bin/sh`, no GPU/window), so it gets bounded unit tests anyway; the window/event-loop wiring remains manual-checklist territory and is kept to trivial delegations.
- **IV. Event-Driven Performance** — PASS. `try_wait()` is a non-blocking waitpid poll once per `AboutToWait` turn; no allocation, no new threads, no busy-loop added (loop pacing/`ControlFlow` is another feature's concern and is not touched).
- **V. Safe Handling of Untrusted Output** — PASS. No PTY-byte-reachable path changes. `has_exited` never panics: wait errors are logged and treated as "not exited", so a transient error cannot close a live window or crash the app.
- **Quality Gates** — PASS. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated` (documented winit exception), `cargo test`, and `cargo build` run before commit.

No exceptions. Complexity Tracking: none.

## Project Structure

```text
specs/009-shell-lifecycle/
├── spec.md
├── plan.md              # this file
└── tasks.md
src/
├── pty.rs     # retain child; spawn_with_command helper; has_exited; env vars; tests
├── window.rs  # shell_exited() accessor only
└── main.rs    # exit check in AboutToWait only
```

## Approach

1. In `src/pty.rs`, add a `child: Box<dyn Child + Send>` field to `PtyManager` and store the handle `spawn_command` returns (currently dropped after logging its PID).
2. Extract `pub(crate) fn spawn_with_command(cols, rows, mut cmd: CommandBuilder) -> Result<Self, ...>` containing the openpty/spawn/reader-thread logic from `new`. The helper sets `TERM=xterm-256color`, `TERM_PROGRAM=Wisp`, and `TERM_PROGRAM_VERSION=env!("CARGO_PKG_VERSION")` on the command so every spawn path (default shell, tests, a future override) gets the same environment. `new` builds `CommandBuilder::new($SHELL or /bin/bash)` and delegates. No signature change to `new`.
3. Add `pub fn has_exited(&mut self) -> bool`: `match self.child.try_wait() { Ok(Some(_)) => true, Ok(None) => false, Err(e) => { log::warn!(...); false } }`. The reader thread/EOF path is deliberately not consulted.
4. Add `pub fn shell_exited(&mut self) -> bool { self.pty.has_exited() }` to `WispWindow`; in `main.rs` `Event::AboutToWait`, `if wisp_window.shell_exited() { elwt.exit(); }` before the existing `request_redraw()`, leaving `ControlFlow` alone.
5. Tests in `src/pty.rs`: spawn `/bin/sh -c "exit 0"` via the helper, poll with a 2s `Instant` deadline and short sleeps until `has_exited()`; spawn `/bin/sh -c "sleep 5"`, assert `!has_exited()` immediately, then kill the child through the stored handle so nothing outlives the test. No unbounded waits.
6. Run all four gates; commit on the feature branch (no push, no merge).
