# Tasks: Shell Lifecycle and Window Close on Exit

**Branch**: `009-shell-lifecycle` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

- [x] T001 Read constitution, current `src/pty.rs` spawn path, `src/window.rs`, and `AboutToWait` in `src/main.rs`; confirm 98-test baseline and the portable-pty `Child::try_wait`/`kill` API
- [x] T002 Refactor `PtyManager::new` spawning into `pub(crate) fn spawn_with_command(cols, rows, CommandBuilder)`; `new(cols, rows)` keeps its signature and builds the `$SHELL` (default `/bin/bash`) command
- [x] T003 Store the spawned child (`Box<dyn Child + Send>`) in `PtyManager` instead of dropping it
- [x] T004 Set `TERM_PROGRAM=Wisp` and `TERM_PROGRAM_VERSION=env!("CARGO_PKG_VERSION")` (plus existing `TERM`) in the spawn helper
- [x] T005 Add `pub fn has_exited(&mut self) -> bool` via `child.try_wait()` (`Ok(Some(_))` only; errors logged, treated as not exited; EOF not consulted)
- [x] T006 Add `WispWindow::shell_exited()` delegation and the `elwt.exit()` check in `Event::AboutToWait` in `src/main.rs` (no `ControlFlow` change)
- [x] T007 Add PTY tests: `/bin/sh -c exit 0` polls to exited within 2s; `/bin/sh -c sleep 5` not exited immediately, then killed via the stored handle; spawned child sees `TERM_PROGRAM`/`TERM_PROGRAM_VERSION`/`TERM`
- [x] T008 Run gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build`
- [x] T009 Commit on `009-shell-lifecycle` as `Close window when the shell exits (009)` (no push, no merge, no Co-Authored-By trailer)
