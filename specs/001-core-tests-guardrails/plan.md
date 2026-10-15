# Implementation Plan: Core Tests and CI Guardrails

**Branch**: `001-core-tests-guardrails` | **Date**: 2026-10-14 | **Spec**: [spec.md](./spec.md)

## Summary

Pin current terminal-core behavior with in-module unit tests, route app
diagnostics through `log`, clean fmt/clippy (with the documented winit
`deprecated` exception), and add GitHub Actions CI. No new runtime features.

## Technical Context

- **Language/Version**: Rust, edition 2021 (stable toolchain in CI)
- **Primary Dependencies**: winit 0.30, wgpu 23, vte 0.13, portable-pty 0.8, fontdue 0.9; new: `log` (facade for the already-present `env_logger`)
- **Testing**: `cargo test` with `#[cfg(test)]` modules in terminal/parser/input
- **Target Platform**: macOS primary; CI on `ubuntu-latest`
- **Constraints**: Tests pin CURRENT behavior; no `lib.rs` extraction; no winit API migration; no runtime behavior change beyond logging/fmt/clippy.

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS. No new features; only new dependency is `log`, justified: `env_logger` is already initialized but the `log` facade is required to emit records.
- **II. Terminal Correctness** — PASS. Tests pin existing VT behavior; nothing new is advertised.
- **III. Test-First for the Terminal Core (NON-NEGOTIABLE)** — PASS / this feature *is* the compliance work: `terminal`, `parser`, `input` get byte-in/state-out tests. Renderer/PTY remain exempt (manual TEST.md).
- **IV. Event-Driven Performance** — PASS. No hot-path changes; removing per-frame `println!` in the renderer reduces work.
- **V. Safe Handling of Untrusted Output** — PASS. No parser behavior change except the semantics-identical `collapsible_match` guard rewrite.
- **Quality Gates** — PASS with one recorded exception: clippy runs as `-D warnings -A deprecated` because migrating off deprecated winit `EventLoop::run`/`create_window` is explicitly out of scope for 001. All other gates (`fmt --check`, `cargo test`, release build in CI) apply unmodified.

No other exceptions. Complexity Tracking: none.

## Project Structure

```text
specs/001-core-tests-guardrails/
├── spec.md
├── plan.md              # this file
└── tasks.md
src/
├── terminal.rs          # + #[cfg(test)] tests, dead-code allows
├── parser.rs            # + #[cfg(test)] tests, collapsible_match fix
├── input.rs             # + #[cfg(test)] tests
├── main.rs, window.rs, renderer.rs, pty.rs   # println/eprintln -> log
└── renderer.rs          # - never-read fields terminal_config/texture_view
.github/workflows/ci.yml # new
Cargo.toml               # + log dependency
Cargo.lock               # now tracked (.gitignore entry removed)
```

## Approach

1. Write tests against current code first; verify they pass before other edits.
2. Swap logging call sites 1:1 (`debug!` for diagnostics, `warn!` for errors).
3. Apply clippy/fmt fixes; grep before removing renderer fields.
4. Add CI, track Cargo.lock, run all four gates, commit on feature branch.
