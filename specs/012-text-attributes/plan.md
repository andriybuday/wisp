# Implementation Plan: Dim, Strikethrough and Underline Styles

**Branch**: `012-text-attributes` | **Date**: 2026-10-17 | **Spec**: [spec.md](./spec.md)

## Summary

Add `DIM` and `STRIKETHROUGH` cell flags with SGR 2/9/22/29 handling, teach
the parser's SGR walker about colon underline styles (`4:n`) and underline
color (`58`), and render dim (fg × 0.6) plus a strikethrough quad. References:
ECMA-48 SGR, xterm ctlseqs (underline styles `4:0`–`4:5`, underline color 58).

## Technical Context

- **Language/Version**: Rust, edition 2021
- **Primary Dependencies**: winit 0.30, wgpu 23, vte 0.13 (`Params` yields `&[u16]` per param; colon values are subparams in one slice), portable-pty, fontdue; no new dependencies
- **Testing**: `cargo test` with `#[cfg(test)]` modules in terminal/parser; renderer GPU-exempt, manual check
- **Target Platform**: macOS primary; must not break Linux/Windows compilation
- **Constraints**: Only the SGR `m` walker changes in `parser.rs`; untrusted output must never panic; stay in assigned files

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS. Dim/strikethrough are emitted daily by git, ls, prompts, and markdown/TUI tools; two flag bits and one quad, no new dependency. Blink (5/6) and distinct underline styles/colors are deliberately not rendered (documented in spec).
- **II. Terminal Correctness** — PASS. Matches ECMA-48/xterm SGR 2/9/22/29 and `4:n`/`58` consumption; unimplemented parts (blink, underline color, distinct styles) are consumed and safely ignored, never rendered as garbage. No capability is newly advertised (DA replies unchanged).
- **III. Test-First for the Terminal Core** — PASS. Terminal/parser byte-in / flag-and-color-out tests for every decision; renderer dim/strikethrough quads are manual-check per the constitution (noted in tasks).
- **IV. Event-Driven Performance** — PASS. No new allocations or redraw triggers; dim is constant-time arithmetic in `build_vertices`, strikethrough adds at most one quad per flagged cell.
- **V. Safe Handling of Untrusted Output** — PASS. Colon/semicolon walks use bounds-checked `get`/`first`; truncated 58/`4:` sequences leave state unchanged except specified underline set/clear.
- **Quality Gates** — PASS. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build`.

No exceptions. Complexity Tracking: none.

## Project Structure

```text
specs/012-text-attributes/
├── spec.md
├── plan.md              # this file
└── tasks.md
src/
├── terminal.rs          # CellFlags DIM/STRIKETHROUGH; sgr 2/9/22/29
├── parser.rs            # SGR walker: 4:n colon styles, 58 consumption
└── renderer.rs          # dim fg scaling; strikethrough quad
```

## Approach

1. Add flag bits and SGR arms in `terminal.rs`; make 22 clear `BOLD | DIM`. `reset_sgr` already clears all flags, so SGR 0 needs no change. Update any test pinning bold-only clearing.
2. In `parser.rs` `sgr_dispatch`, before the generic simple-code arm:
   - `code == 4 && slice.len() > 1`: colon underline style. `slice[1] == 0` → clear underline via `sgr(24)`, else set via `sgr(4)`. Never dispatch the sub-style as a code.
   - `code == 58`: consume exactly like 38/48 (colon: one slice; semicolon: `58;5;n` → 3 params, `58;2;r;g;b` → 5 params, truncated → consume safely) but apply nothing.
   Keep 38/48 handling untouched; share the operand-count logic rather than duplicating color application.
3. In `renderer.rs` `build_vertices`, after resolving fg/bg and inverse swap,
   scale fg RGB by 0.6 when `DIM` (before cursor override so underline and
   strikethrough inherit the dimmed fg; cursor cell keeps its readable
   override). After the glyph quad, draw the strikethrough quad (2px at
   vertical middle) mirroring the underline quad.
4. Add tests, run all four gates, commit on the feature branch.
