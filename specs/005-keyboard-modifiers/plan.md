# Implementation Plan: Complete Keyboard Modifier Input

**Branch**: `005-keyboard-modifiers` | **Date**: 2026-10-14 | **Spec**: [spec.md](./spec.md)

## Summary

Thread `ModifiersState` into `key_to_bytes` and implement xterm modifier
encoding: Super→None, modifier parameter `m` in CSI sequences for
arrows/navigation/function keys, Shift+Tab backtab, the full Ctrl
control-byte table, and Alt-as-Meta ESC prefixing. DECCKM is out of scope.

## Technical Context

- **Language/Version**: Rust, edition 2021
- **Primary Dependencies**: winit 0.30 (`ModifiersState` bitflags: `SHIFT`/`CONTROL`/`ALT`/`SUPER`, accessors `shift_key()`/`control_key()`/`alt_key()`/`super_key()`); no new dependencies
- **Testing**: `cargo test`, `#[cfg(test)]` module in `src/input.rs`
- **Target Platform**: macOS primary; encoding is platform-independent
- **Constraints**: only `src/input.rs` + one call site in `src/main.rs`; unmodified sequences byte-identical to 001

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS. Ctrl/Alt/F-key input is daily-essential terminal behaviour, not an extra; no new dependencies, no new modules, no configuration.
- **II. Terminal Correctness over Feature Count** — PASS. Encodings match xterm/ECMA-48: modifier parameter `1+shift+2·alt+4·ctrl`, SS3 for unmodified F1-F4, `CSI Z` backtab, C0 control bytes for Ctrl combos. Nothing unimplemented is advertised; DECCKM explicitly out of scope in the spec.
- **III. Test-First for the Terminal Core (NON-NEGOTIABLE)** — PASS. `input` is core; every new behaviour gets a `#[cfg(test)]` assertion feeding key+modifiers in and asserting exact bytes out. All 33 pre-existing tests keep passing.
- **IV. Event-Driven Performance** — PASS. Per-keypress path only; a handful of branch checks and one small bounded `Vec<u8>` (≤8 bytes) per key, as before. No rendering/parse hot-loop changes.
- **V. Safe Handling of Untrusted Output** — PASS. Input path only; no PTY-byte-reachable code changes; no panics/unwraps added (char arithmetic on checked ASCII ranges only).
- **Quality Gates** — PASS. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated` (the documented 001 winit exception), `cargo test`, `cargo build` all run before commit.

No exceptions. Complexity Tracking: none.

## Project Structure

```text
specs/005-keyboard-modifiers/
├── spec.md
├── plan.md              # this file
└── tasks.md
src/
├── input.rs             # key_to_bytes gains ModifiersState; modifier/Ctrl/Alt/F-key encoding + tests
└── main.rs              # single call site passes tracked `modifiers`
```

## Approach

1. Change signature to `key_to_bytes(key, text, logical_key, modifiers: ModifiersState)`; update `main.rs` call and 001 tests.
2. Early-return `None` on `super_key()`. Compute `m`, `shift`, `alt`, `ctrl`.
3. Dispatch physical special keys first (Shift+Tab before Tab; Ctrl+Backspace; arrows/Home/End; Insert/Delete/PgUp/PgDn; F1-F12; Enter/Escape/Backspace/Tab bases with Alt ESC-prefix where specified) so modified CSI forms win over the Ctrl-char table.
4. Then Ctrl-char table over candidate chars (text, then logical `Character`, then physical key code as a fallback for platforms that give no text under Ctrl), so Ctrl wins over plain text.
5. Then Alt-without-Ctrl ESC prefix for printable text/logical characters; finally the unchanged plain text/logical fallback.
6. Add tests for every spec bullet; run all four gates; commit on the feature branch.
