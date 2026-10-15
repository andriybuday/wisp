# Tasks: Query Replies and Window Title

**Branch**: `004-query-replies-title` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

## Phase 1: Title storage

- [x] T001 Add `title: Option<String>` to `Terminal` in `src/terminal.rs` with `title() -> Option<&str>` and `set_title(String)` (title storage only, no other changes)

## Phase 2: Query replies and OSC title (parser)

- [x] T002 In `src/parser.rs`: add `replies: Vec<Vec<u8>>` to `AnsiPerformer` and `pub fn take_replies(&mut self) -> Vec<Vec<u8>>` on `AnsiParser` draining them
- [x] T003 In `src/parser.rs` `csi_dispatch`: `'c'` — no intermediate replies `ESC [ ? 6 c`, `>` replies `ESC [ > 0 ; 1 ; 0 c`, other intermediates ignored
- [x] T004 In `src/parser.rs` `csi_dispatch`: `'n'` — param 5 replies `ESC [ 0 n`, param 6 replies `ESC [ {row} ; {col} R` (1-based), other params ignored
- [x] T005 In `src/parser.rs` `csi_dispatch`: `'t'` — param 18 replies `ESC [ 8 ; {rows} ; {cols} t`, all other `CSI ... t` ignored
- [x] T006 In `src/parser.rs` `osc_dispatch`: `params[0]` of `b"0"`/`b"2"` sets title from `params[1]` (lossy UTF-8, control chars stripped, capped at 256 chars); all other OSC ignored

## Phase 3: Window wiring

- [x] T007 In `src/window.rs`: add `last_title: Option<String>`; in `render()`, after `parser.advance`, write every drained reply to the PTY ignoring write errors, and call `window.set_title` when the terminal title changes

## Phase 4: Tests

- [x] T008 Add `#[cfg(test)]` tests in `src/parser.rs`: `take_replies()` for DA, secondary DA, DSR 5, DSR 6 after `CSI H`, `CSI 18 t` for an 80x24 terminal; `OSC 2` title via BEL and via ST; non-UTF-8 title bytes do not panic
- [x] T009 Confirm all 33 pre-existing tests still pass (no pre-existing test modified)

## Phase 5: Verify & commit

- [x] T010 Verify: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build` all pass
- [x] T011 Commit on `004-query-replies-title` as "Add query replies and OSC window title (004)" (no Co-Authored-By, no push, no merge)
