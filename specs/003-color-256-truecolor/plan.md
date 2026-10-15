# Implementation Plan: 256-Color and Truecolor Support

**Branch**: `003-color-256-truecolor` | **Date**: 2026-10-14 | **Spec**: [spec.md](./spec.md)

## Summary

Replace `u8` color indices with a `Color` enum (Default/Indexed/Rgb), parse
extended SGR colors in both semicolon and colon forms, resolve the full
256-color palette plus truecolor in the renderer, and pin everything with
unit tests. References: ECMA-48 SGR 38/48 extended colors, xterm ctlseqs.

## Technical Context

- **Language/Version**: Rust, edition 2021
- **Primary Dependencies**: winit 0.30, wgpu 23, vte 0.13 (`Params` yields `&[u16]` per param; colon-separated values arrive as subparams in one slice), portable-pty, fontdue; no new dependencies
- **Testing**: `cargo test` with `#[cfg(test)]` modules in terminal/parser/config
- **Target Platform**: macOS primary; must not break Linux/Windows compilation
- **Constraints**: Only the SGR `m` CSI handler changes in `parser.rs`; untrusted output must never panic

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS. 256-color/truecolor is daily-driver terminal behavior (ls, vim, git, prompts); no new dependency, one small enum replaces two `u8` fields.
- **II. Terminal Correctness** — PASS. Matches ECMA-48/xterm SGR 38/48 semantics in both separator forms; malformed sequences are consumed and ignored, never rendered or panicked on.
- **III. Test-First for the Terminal Core** — PASS. Parser/terminal tests assert byte-in / `Color`-out; palette resolution is a pure function with unit tests. Renderer wiring is GPU-exempt per the constitution but the resolution logic lives in testable `config.rs`.
- **IV. Event-Driven Performance** — PASS. No new allocations in the parse or render hot paths; `ansi_256` is constant-time arithmetic.
- **V. Safe Handling of Untrusted Output** — PASS. All extended-param reads are bounds-checked, `u16` RGB values clamp to 255, truncated sequences leave color unchanged.
- **Quality Gates** — PASS. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated` (the `deprecated` exception is inherited from 001 for winit), `cargo test`, `cargo build`.

No exceptions. Complexity Tracking: none.

## Project Structure

```text
specs/003-color-256-truecolor/
├── spec.md
├── plan.md              # this file
└── tasks.md
src/
├── terminal.rs          # Color enum; Cell/current colors; set_fg/bg_color
├── parser.rs            # stateful SGR 'm' iteration only
├── renderer.rs          # Color -> RGBA resolution, bg-quad rule
└── config.rs            # ColorPalette::ansi_256, foreground un-dead-coded
```

## Approach

1. Add `Color` and migrate `Cell`, `current_fg/bg`, `reset_sgr`, `sgr`; update 001 SGR tests.
2. Replace the flat SGR loop with a two-level walker (param list + subparams): on 38/48, read the mode (5/2) from the same slice when present (colon form) or the next param (semicolon form), then consume the index/RGB operands from the same stream; any shortfall aborts that color without mutation.
3. Implement `ansi_256` and renderer resolution (`Default` → palette foreground/background, bold +8 for `Indexed(0..=7)`, inverse swaps resolved colors, selection still overrides bg).
4. Add tests, run all four gates, commit on the feature branch.
