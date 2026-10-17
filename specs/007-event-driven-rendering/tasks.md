# Tasks: Event-Driven Rendering and Cursor Blink

**Branch**: `007-event-driven-rendering` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

- [x] T001 Read constitution and current `main.rs`/`window.rs`/`pty.rs`/`renderer.rs`/`terminal.rs`/`parser.rs`; confirm 98-test baseline
- [x] T002 `pty.rs`: shared waker cell cloned into the reader thread and invoked after each successful send; add `set_waker`; `PtyManager::new` signature unchanged
- [x] T003 `terminal.rs` + `parser.rs`: `sync_output` flag with accessors, wired to DECSET/DECRST `?2026`; unit tests for set/clear
- [x] T004 `renderer.rs`: u32 indices (`build_indices -> Vec<u32>`, `IndexFormat::Uint32`) with 20,000-quad unit test; persistent grow-only vertex/index buffers via `queue.write_buffer`; panic-free surface-error handling
- [x] T005 `renderer.rs`: `render(terminal, blink_on)`; shown cursor renders as cursor-colored cell background with the glyph in the palette background color (separate hiding block removed)
- [x] T006 `window.rs`: `WispUserEvent`, typed `new`, proxy waker installed after PTY construction; blink (530 ms) and sync (100 ms failsafe) deadlines with `process_pty`/`render`/`tick`/`next_deadline`; cached physical cell metrics (no per-move `FontManager`); redraw requests on resize/selection/scroll
- [x] T007 `main.rs`: `EventLoop::with_user_event`, `Wait`/`WaitUntil` control flow (no `Poll`), redraws only on PTY output / blink flip / resize / selection / scroll / sync expiry
- [x] T008 Run gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build`
- [ ] T009 Manual verification (required before merge; event-loop/blink behavior is manual-verification only and cannot be unit-tested without a display): idle CPU near zero; shell output appears promptly; 530 ms blink with glyph visible under the shown cursor; `?25l` hides the cursor; resize, selection drag, and scroll redraw; a `?2026`-emitting TUI shows no tearing and never freezes
- [x] T010 Commit on `007-event-driven-rendering` as `Make rendering event-driven with cursor blink (007)` (no push, no merge, no Co-Authored-By trailer)
