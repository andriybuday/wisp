# Implementation Plan: SGR Mouse and Focus Reporting

**Branch**: `008-mouse-focus-reporting` | **Date**: 2026-10-17 | **Spec**: [spec.md](./spec.md)

## Summary

Add xterm mouse tracking (`?1000`/`?1002`/`?1003`) with SGR (`?1006`) and
legacy encodings, plus focus in/out reporting (`?1004`). `Terminal` stores the
mode state, the parser wires the five DEC private modes, `input.rs` gains a
pure byte encoder, and `window.rs`/`main.rs` forward mouse and focus events
to the PTY while tracking is active and Shift is not held (Shift restores
today's selection/scroll behavior, the standard user override).

## Technical Context

- **Language/Version**: Rust, edition 2021
- **Primary Dependencies**: none new; existing `winit` already delivers `MouseInput`, `CursorMoved`, `MouseWheel`, `Focused`, and `ModifiersState`
- **Testing**: `cargo test`, `#[cfg(test)]` modules in `src/input.rs` (encoder) and `src/parser.rs` (mode wiring); window/main event forwarding exercised via the manual checklist in TEST.md (GPU/window code, per constitution)
- **Target Platform**: macOS primary; encodings are platform-independent
- **Constraints**: changes confined to `src/terminal.rs`, `src/parser.rs`, `src/input.rs`, `src/window.rs`, `src/main.rs`, and `specs/008-mouse-focus-reporting/`; no renderer or PTY changes

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS. Mouse reporting is daily-essential: vim, tmux, htop, and less are effectively mouse-blind without it, and focus reporting is a two-sequence addition on the same mode plumbing. Only the two encodings modern apps negotiate (SGR + legacy fallback) are implemented; `?1005`/`?1015`/`?1016` are explicit non-goals. No new dependency, module, or configuration.
- **II. Terminal Correctness over Feature Count** — PASS. Mode numbers, button codes (wheel 64/65, motion +32, modifiers 4/8/16, no-button 35), SGR `M`/`m` finals, legacy release button 3, per-cell motion dedupe, and `ESC [ I`/`ESC [ O` all follow xterm's documented semantics. Nothing is advertised beyond what is implemented: DA replies are untouched (xterm does not advertise mouse in DA either).
- **III. Test-First for the Terminal Core (NON-NEGOTIABLE)** — PASS. `terminal`, `parser`, and `input` are core: the encoder is a pure function with byte-exact tests (SGR/legacy press and release, motion, wheel, modifiers, 255 clamping), and mode wiring/precedence is tested by feeding escape sequences through `AnsiParser`. `window.rs`/`main.rs` forwarding is window/GPU-adjacent code, exempt from unit tests and covered by the TEST.md manual checklist. All 98 pre-existing tests keep passing.
- **IV. Event-Driven Performance** — PASS. Reporting happens only on input events; motion is deduped per cell change and each report is a single small bounded allocation written straight to the PTY. No polling, timers, or unbounded buffers are added; the idle path is untouched.
- **V. Safe Handling of Untrusted Output** — PASS. PTY-reachable code only flips enum/bool mode state in the parser (unknown modes remain ignored, as today). The encoder uses saturating arithmetic and clamps legacy bytes to 255, so no coordinate or button value can overflow or panic; grid coordinates are clamped before use.
- **Quality Gates** — PASS. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated` (the documented winit exception from 001/CI), `cargo test`, and `cargo build` all run before commit.

No exceptions. Complexity Tracking: none.

## Project Structure

```text
specs/008-mouse-focus-reporting/
├── spec.md
├── plan.md              # this file
└── tasks.md
src/
├── terminal.rs          # MouseTracking enum + sgr_mouse / focus_reporting flags, getters/setters
├── parser.rs            # 'h'/'l' wiring for ?1000 ?1002 ?1003 ?1004 ?1006; tests
├── input.rs             # encode_mouse + button-code constants/helpers; tests
├── window.rs            # reporting-aware mouse handlers, clamped coords, focus_changed
└── main.rs              # passes modifiers + pressed buttons; forwards Focused
```

## Approach

1. In `src/terminal.rs`, add `MouseTracking { Off, Normal, ButtonEvent, AnyEvent }` (ordered, so precedence is a rank comparison) plus `sgr_mouse` and `focus_reporting` flags. `set_mouse_tracking(mode, enabled)`: enabling applies only when `mode >= current` (higher replaces lower, lower never downgrades); disabling the currently active mode drops to `Off`, disabling any other mode is a no-op.
2. In `src/parser.rs`, extend the existing DEC private `h`/`l` match with 1000/1002/1003 → `set_mouse_tracking`, 1004 → `set_focus_reporting`, 1006 → `set_sgr_mouse`.
3. In `src/input.rs`, add button-code constants, a `mouse_modifiers(ModifiersState) -> u32` helper (Shift 4 / Alt 8 / Ctrl 16), and the pure `encode_mouse` per FR-003/FR-004. Legacy release keeps modifier/motion bits but replaces the low button bits with 3 (xterm cannot identify the released button in legacy form).
4. In `src/window.rs`, add `position_to_grid_clamped` (same math as `position_to_grid`, clamping into the grid instead of returning `None`), a `last_reported_cell` for per-cell motion dedupe, and reporting branches in `mouse_press`/`mouse_release`/`mouse_move`/`mouse_wheel`: when tracking is not `Off` and Shift is not held, encode and write to the PTY (wheel = 64/65 at the cursor cell, one report per event; motion per tracking level, 35 for no-button AnyEvent); otherwise run today's selection/scroll code verbatim. Add `focus_changed(bool)` writing `ESC [ I`/`ESC [ O` when `?1004` is on.
5. In `src/main.rs`, track pressed `MouseButton`s in a small `Vec`, pass `modifiers` and the pressed set into the window handlers for all buttons (not just left), and forward `WindowEvent::Focused` to `focus_changed`.
6. Add the encoder and parser tests from SC-001; run all four gates; commit on the feature branch (no push, no merge).
