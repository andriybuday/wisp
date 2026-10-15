# Implementation Plan: Query Replies and Window Title

**Branch**: `004-query-replies-title` | **Date**: 2026-10-14 | **Spec**: [spec.md](./spec.md)

## Summary

Teach the parser to answer terminal queries (primary/secondary DA, DSR 5/6,
text-area size report `CSI 18 t`) by accumulating reply byte strings that
`window.rs` drains and writes to the PTY each `render()`, and let the shell
set the OS window title via `OSC 0` / `OSC 2`, stored on `Terminal` and
applied in `render()` when it changes. References: ECMA-48 / xterm control
sequences for DA, DSR, `CSI 18 t`, and OSC title semantics.

## Technical Context

- **Language/Version**: Rust, edition 2021 (stable)
- **Primary Dependencies**: existing stack only (vte 0.13 for parsing,
  winit 0.30 `Window::set_title`, portable-pty via `PtyManager::write`);
  no new dependencies
- **Testing**: `cargo test` with `#[cfg(test)]` modules in `src/parser.rs`
  (byte-in, replies/title-out); `window.rs` is renderer/PTY code, exempt
  from unit tests per Constitution III
- **Target Platform**: macOS primary; must compile on Linux/Windows
- **Constraints**: No threads/channels for replies (plain `Vec` drain); PTY
  write errors ignored; title capped at 256 chars, control chars stripped;
  no other CSI behavior changes

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS. Only the four queries and OSC 0/2 title
  are implemented; all other `CSI ... t` window manipulations and all other
  OSC are explicitly out of scope and ignored. No new dependencies.
- **II. Terminal Correctness** — PASS. Primary DA advertises VT102
  (`ESC [ ? 6 c`) on purpose: wisp must not advertise capabilities it does
  not implement. Unknown DA intermediates, DSR params, and `CSI ... t`
  variants are consumed and safely ignored.
- **III. Test-First for the Terminal Core (NON-NEGOTIABLE)** — PASS. New
  parser tests feed byte sequences and assert `take_replies()` bytes plus
  `terminal.title()` for BEL/ST/non-UTF-8 cases. Window wiring is renderer
  code, covered by the manual checklist instead.
- **IV. Event-Driven Performance** — PASS. Replies are bounded byte strings
  drained once per `render()`; title is capped at 256 chars and applied
  only on change. No new loops or unbounded buffers.
- **V. Safe Handling of Untrusted Output** — PASS. Title decoding is lossy
  UTF-8 with control characters stripped and a hard 256-char cap; reply
  writes ignore errors; query handling indexes nothing and cannot panic on
  malformed sequences.
- **Quality Gates** — PASS with the project-wide recorded exception:
  clippy runs as `-D warnings -A deprecated` (winit deprecation migration
  is out of scope, see 001). `fmt --check`, `cargo test`, and `cargo build`
  apply unmodified.

Complexity Tracking: none (no exceptions beyond the recorded clippy flag).

## Project Structure

```text
specs/004-query-replies-title/
├── spec.md
├── plan.md              # this file
└── tasks.md
src/
├── parser.rs            # reply accumulation, take_replies(), DA/DSR/18t, OSC title, + tests
├── terminal.rs          # title storage only: title field, title(), set_title()
└── window.rs            # drain replies -> PTY; apply title changes in render()
```

## Approach

1. `terminal.rs`: add `title: Option<String>` (init `None`), `title() ->
   Option<&str>`, `set_title(String)`.
2. `parser.rs`: add `replies: Vec<Vec<u8>>` to `AnsiPerformer` and
   `AnsiParser::take_replies()` (`std::mem::take` drain). In
   `csi_dispatch` add `'c'` (empty intermediates -> VT102 reply,
   `[b'>']` -> secondary reply, anything else ignored), `'n'` (5 -> ready,
   6 -> 1-based cursor report), `'t'` (18 -> size report, else ignored).
   In `osc_dispatch`, `params[0]` of `b"0"`/`b"2"` sets the title from
   `params[1]` (lossy UTF-8, control chars filtered, `take(256)`).
3. `window.rs`: add `last_title: Option<String>`; in `render()`, after each
   `parser.advance`, write every `take_replies()` entry to the PTY
   (ignoring errors); after draining PTY output, compare
   `terminal.title()` with `last_title` and call `window.set_title` on
   change.
4. Add parser tests for each decision; run all four gates; commit.
