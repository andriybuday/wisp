# Tasks: Complete Keyboard Modifier Input

**Branch**: `005-keyboard-modifiers` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

- [x] T001 Read constitution, current `src/input.rs`, and the `main.rs` call site / modifier tracking
- [x] T002 Change `key_to_bytes` to accept `ModifiersState` by value; update `main.rs` call to pass tracked `modifiers`; update 001-era input tests to the new signature
- [x] T003 Implement Super→None and modifier parameter `m = 1 + shift + 2·alt + 4·ctrl`
- [x] T004 Implement modified CSI for arrows/Home/End and Insert/Delete/PageUp/PageDown; add Insert `CSI 2~`; keep unmodified forms byte-identical
- [x] T005 Implement F1-F4 (SS3 / modified CSI) and F5-F12 (CSI n~ / CSI n;m~)
- [x] T006 Implement Shift+Tab → `CSI Z` before plain Tab
- [x] T007 Implement Ctrl control-byte table (letters, Space/2, [/3, \/4, ]/5, ^/6, _/7, ?/8, Ctrl+Backspace) winning over plain text
- [x] T008 Implement Alt-without-Ctrl ESC prefix for printable text and for Enter/Escape base bytes
- [x] T009 Add tests: ctrl+letter upper/lower, ctrl+space, ctrl+], alt+char, F1, F5, F12, ctrl+shift+Right (m=6), shift+tab, insert, super→None, plain unchanged (plus modified F-keys/arrows, alt+enter/escape, ctrl punctuation/digits)
- [x] T010 Run gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build`
- [x] T011 Commit on `005-keyboard-modifiers` (no push, no merge, no Co-Authored-By trailer)
