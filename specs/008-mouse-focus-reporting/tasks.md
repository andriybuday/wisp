# Tasks: SGR Mouse and Focus Reporting

**Branch**: `008-mouse-focus-reporting` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

- [x] T001 Read constitution, `Terminal` mode state, parser `h`/`l` handling, `input.rs` encoders, and the `window.rs`/`main.rs` event paths; confirm 98-test baseline
- [x] T002 Add `MouseTracking` enum and `sgr_mouse`/`focus_reporting` flags with getters/setters to `src/terminal.rs`
- [x] T003 Wire `?1000`/`?1002`/`?1003`/`?1004`/`?1006` in the parser `h`/`l` handler in `src/parser.rs`
- [x] T004 Add mouse button-code constants, `mouse_modifiers`, and pure `encode_mouse` (SGR + legacy) to `src/input.rs`
- [x] T005 Add clamped coordinate helper and reporting-aware `mouse_press`/`mouse_release`/`mouse_move`/`mouse_wheel` plus `focus_changed` to `src/window.rs`
- [x] T006 Pass `ModifiersState` and pressed buttons from `src/main.rs` into the window handlers; forward `WindowEvent::Focused`
- [x] T007 Add encoder tests: SGR press/release, legacy press/release, motion +32, wheel 64/65, modifier bits, legacy 255 clamping, no-button motion 35
- [x] T008 Add parser tests: `?1000`/`?1002`/`?1003`/`?1006`/`?1004` set/reset with precedence (higher replaces, lower no downgrade, reset active → Off)
- [x] T009 Run gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build`
- [x] T010 Commit on `008-mouse-focus-reporting` as `Add SGR mouse and focus reporting (008)` (no push, no merge, no Co-Authored-By trailer)
