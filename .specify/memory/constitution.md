# Wisp Constitution

## Core Principles

### I. Minimal by Default

Wisp is a minimalist terminal emulator: essential features only, clean codebase,
no bells and whistles. New features MUST justify why a terminal user needs them
daily; "nice to have" is grounds for rejection. Tabs, splits, themes, ligatures,
image rendering, and similar extras are out of scope unless a spec explicitly
amends this principle with a documented rationale. Prefer deleting or simplifying
code over adding abstraction; every new dependency MUST be justified in the plan.

### II. Terminal Correctness over Feature Count

Correct VT/xterm behavior outranks new features. Wisp MUST NOT advertise
capabilities it does not implement (via `TERM`, Device Attributes replies, or
documentation). Escape sequences that are not implemented MUST be consumed and
safely ignored — never rendered as garbage and never a panic. When behavior is
ambiguous, match xterm/ECMA-48 semantics and cite the reference in the spec or
plan.

### III. Test-First for the Terminal Core (NON-NEGOTIABLE)

The terminal core (`terminal`, `parser`, `input`) is a pure state machine and
MUST be covered by automated tests: feed byte sequences in, assert grid,
cursor, and attribute state out. Bug fixes in the core start with a failing
test that reproduces the bug. Renderer and PTY code, which need a GPU and a
live shell, are exempt from unit tests but MUST be exercised with the manual
checklist in TEST.md before merge.

### IV. Event-Driven Performance

Rendering is event-driven: an idle terminal MUST NOT busy-loop or redraw
continuously. Targets: 60 FPS while output is flowing, input latency under
10 ms, near-zero CPU when idle. Hot paths (per-frame rendering, per-byte
parsing, scrolling) MUST NOT allocate unboundedly or grow buffers without a
stated bound; scrollback and the glyph atlas stay capped.

### V. Safe Handling of Untrusted Output

Shell output is untrusted input. Code reachable from PTY bytes MUST NOT panic,
index out of bounds, or block indefinitely on malformed sequences, oversized
glyphs, or unexpected Unicode. Startup failures (GPU, font, PTY) may fail fast
with a clear message, but runtime surface errors (lost/outdated swapchain)
MUST be recovered from, not crashed on.

## Technology Constraints

- Single Rust binary (edition 2021); no plugin system, no scripting runtime.
- Stack: winit (windowing), wgpu (GPU rendering), portable-pty (shell),
  vte (ANSI parsing), fontdue (font rasterization). Replacing a stack
  component requires a spec and plan, not an opportunistic swap.
- macOS is the primary platform; changes MUST NOT knowingly break compilation
  on Linux or Windows.
- The monospace font ships embedded in the binary; there is no runtime asset
  or configuration requirement for a first run.

## Development Workflow & Quality Gates

Features follow the Spec Kit flow: constitution → specify → (clarify) → plan →
tasks → implement, with artifacts under `specs/NNN-<feature>/`. Small bug fixes
may skip a full spec but never the quality gates. Before merge, all of the
following MUST pass:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build --release`

Plans MUST include a constitution check confirming compliance with the
principles above, or record and justify any exception in the plan's
Complexity Tracking section.

## Governance

This constitution supersedes ad-hoc practices and stale project documents
(PLAN.md, STATUS.md, SUMMARY.md) where they conflict. Amendments require a
documented rationale in the PR that changes this file and a version bump:
MAJOR for removing or redefining a principle, MINOR for adding a principle or
gate, PATCH for clarifications. Reviewers MUST verify that specs and plans
comply; unjustified complexity is grounds for rejection.

**Version**: 1.0.0 | **Ratified**: 2026-10-14 | **Last Amended**: 2026-10-14
