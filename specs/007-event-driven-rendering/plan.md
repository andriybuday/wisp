# Implementation Plan: Event-Driven Rendering and Cursor Blink

**Branch**: `007-event-driven-rendering` | **Date**: 2026-10-17 | **Spec**: [spec.md](./spec.md)

## Summary

Replace `ControlFlow::Poll` + unconditional `AboutToWait` redraw with
`Wait`/`WaitUntil` driven by three wake sources: PTY output (reader thread
→ shared waker → `EventLoopProxy` user event), a 530 ms cursor-blink
deadline, and a 100 ms synchronized-output (`?2026`) failsafe deadline.
Redraws are requested only on real changes. The renderer gains u32 indices,
persistent grow-only vertex/index buffers updated with `write_buffer`, and
panic-free surface-error handling. The shown cursor becomes the cell
background with its glyph drawn in the palette background color. Cell
metrics are cached in the window.

## Technical Context

- **Language/Version**: Rust, edition 2021
- **Primary Dependencies**: existing `winit` 0.30 (`EventLoop::with_user_event().build()`, `EventLoopProxy::send_event`, `ControlFlow::WaitUntil(Instant)`), `wgpu` 23 (`SurfaceError`, `IndexFormat::Uint32`), `vte`; no new dependencies
- **Testing**: `cargo test`; `#[cfg(test)]` in `src/parser.rs` (`?2026`), `src/terminal.rs` (flag), `src/renderer.rs` (pure `build_indices`); event-loop/blink behavior is manual-verification only (see tasks)
- **Target Platform**: macOS primary; winit/wgpu APIs used are cross-platform
- **Constraints**: changes confined to `src/main.rs`, `src/window.rs`, `src/pty.rs`, `src/renderer.rs`, `?2026` wiring in `src/terminal.rs`/`src/parser.rs`, and `specs/007-event-driven-rendering/`. `PtyManager::new` signature unchanged (a parallel feature edits PTY lifecycle): waker installed via new `set_waker`.

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS. No new dependencies, modules, config, or UI surface. The change mostly removes the busy loop; blink and `?2026` are daily-essential terminal behaviors (cursor visibility; tear-free TUI repaint), not extras.
- **II. Terminal Correctness over Feature Count** — PASS. `?2026` follows the DEC synchronized-output semantics used by xterm-compatible terminals (hold presentation between set and reset, with a timeout so a stuck app cannot freeze the display); unknown modes keep being ignored exactly as before, and nothing new is advertised in DA/`TERM`.
- **III. Test-First for the Terminal Core (NON-NEGOTIABLE)** — PASS. The core addition (`?2026` in `parser`/`terminal`) is unit-tested (set and clear, including via the flag accessors). Renderer `build_indices` is made a pure associated function so the 20,000-quad u32 case is unit-tested without a GPU. Windowing/PTY/blink paths are exempt renderer/PTY-adjacent code requiring a display and live shell; they are covered by the manual checklist in tasks.md.
- **IV. Event-Driven Performance** — PASS (this feature implements the principle). Idle loop sleeps in `WaitUntil` to the next deadline; per-frame GPU buffer allocation is replaced by persistent grow-only buffers; mouse-move no longer constructs a `FontManager` (embedded-font parse) per event. Blink wakes at most every 530 ms; sync hold caps at 100 ms.
- **V. Safe Handling of Untrusted Output** — PASS. `?2026` is a plain bool; sync can never block presentation beyond 100 ms. Surface `Lost`/`Outdated` reconfigure and return, `Timeout` returns, `OutOfMemory` logs and returns — the old `.expect()` on `get_current_texture` is gone. The waker cell clone avoids holding its lock during the callback, and reader-thread lock poisoning cannot panic the reader.
- **Quality Gates** — PASS. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated` (documented winit exception), `cargo test`, `cargo build` run before commit; all 98 pre-existing tests keep passing, no pre-existing test is modified.

No exceptions. Complexity Tracking: none.

## Project Structure

```text
specs/007-event-driven-rendering/
├── spec.md
├── plan.md              # this file
└── tasks.md
src/
├── main.rs              # with_user_event loop, Wait/WaitUntil, targeted redraws
├── window.rs            # WispUserEvent, waker install, blink + sync deadlines, process_pty/render split, cached metrics
├── pty.rs               # shared waker cell + set_waker (new signature unchanged)
├── renderer.rs          # u32 indices, persistent buffers, surface-error handling, cursor-cell glyph, blink param
├── terminal.rs          # sync_output flag + accessors
└── parser.rs            # ?2026 in DECSET/DECRST handling + tests
```

## Approach

1. **PTY waker (`pty.rs`)**: add `waker: Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>`; create it in `new` (signature unchanged), clone into the reader thread, and after each successful `sender.send` clone the fn out from under the lock and call it. Add `pub fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>)`.
2. **Window (`window.rs`)**: define `pub enum WispUserEvent { PtyOutput }`; `new(&EventLoop<WispUserEvent>)`. After `PtyManager::new`, install a waker closing over `event_loop.create_proxy()` that sends `PtyOutput`. Cache physical `cell_width`/`cell_height`/`padding` in `new` (post-window, using the real scale factor) and refresh them in `resize`; `position_to_grid` uses the cache only. Track `blink_on` (starts on), `next_blink: Instant`, and `sync_since: Option<Instant>`. Split the old `render` into `process_pty() -> bool` (drain channel, parse, forward query replies, apply title; note first-seen sync hold) and `render()` (present via `renderer.render(terminal, blink_on)`). Add `sync_holding()` (flag set and inside the 100 ms failsafe), `next_deadline()` (min of blink and armed sync failsafe), and `tick() -> bool` (flip blink when due; when a sync hold expires, clear the hold timer so the frame presents and any later hold re-arms). Mouse press/move(while dragging)/wheel and resize request redraws from within the window methods.
3. **Main (`main.rs`)**: build `EventLoop::<WispUserEvent>::with_user_event().build()`. On `UserEvent(PtyOutput)`, process PTY and request redraw unless `sync_holding()`. On `AboutToWait`, call `tick()` and request redraw if it reports a change (blink flip with a visible cursor, or sync expiry); also opportunistically drain pending PTY bytes there so a sync release arriving between events presents. After every event set `elwt.set_control_flow(ControlFlow::WaitUntil(wisp_window.next_deadline()))`. Resized/mouse handlers rely on the window's own redraw requests; `RedrawRequested` presents.
4. **Sync output (`terminal.rs`/`parser.rs`)**: `sync_output: bool` with `sync_output()`/`set_sync_output(bool)`; parser `h`/`l` private-mode handling maps `2026` to it.
5. **Renderer (`renderer.rs`)**: `build_indices(quad_count) -> Vec<u32>` as a pure associated function; `IndexFormat::Uint32`. Hold `vertex_buffer`/`index_buffer: Option<Buffer>` plus capacities; (re)create only when the needed byte size exceeds capacity, `queue.write_buffer` the frame data, and bind sub-slices of the exact frame size. `render(terminal, blink_on)`: match `get_current_texture()` errors (`Lost`/`Outdated` → reconfigure + return, `Timeout` → return, `OutOfMemory` → log + return). In `build_vertices`, when the cursor is shown (blink on, `cursor_visible`, not scrolled) the cursor cell's background is the cursor color and its glyph/underline color is the palette background; the old separate cursor block that hid the glyph is removed.
6. **Tests**: parser set/clear `?2026`; terminal flag default/accessors; renderer `build_indices(20_000)` boundary values (first quad, quad at the old u16 wrap point, last quad). Run all four gates; commit on the feature branch (no push, no merge).
