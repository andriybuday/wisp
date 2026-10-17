# Implementation Plan: OSC 8 Hyperlinks and URL Opening

**Branch**: `013-hyperlinks` | **Date**: 2026-10-17 | **Spec**: [spec.md](./spec.md)

## Summary

`Cell` gains a `Copy`-preserving `link: u16` id into a bounded, deduplicated
intern table owned by `Terminal`. The parser handles OSC 8 only, the renderer
reuses its underline quad for linked cells, and `window::mouse_press` opens an
explicit link (or an implicitly detected plain URL via pure `find_url_at`) on
Super/Ctrl left-click before any selection or mouse reporting.

## Technical Context

- **Language/Version**: Rust, edition 2021
- **Primary Dependencies**: none added; `std::process::Command` for the opener, existing `log` for spawn failures
- **Testing**: `cargo test`; `#[cfg(test)]` in `terminal.rs`, `parser.rs`, `window.rs`
- **Target Platform**: macOS primary; opener selected with `cfg(target_os)` (`open` / `xdg-open` / `cmd /C start`, `xdg-open` fallback elsewhere)
- **Constraints**: only `src/terminal.rs`, `src/parser.rs`, `src/renderer.rs` (underline only), `src/window.rs`, and `specs/013-hyperlinks/`; `input.rs` untouched

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS. Clickable links are a daily terminal need (tool output is full of URLs and OSC 8 emitters); the implementation is one `u16` per cell, one bounded `Vec<String>`, one pure matcher, and the platform opener. No new dependencies, no hover UI, no theming.
- **II. Terminal Correctness** — PASS. OSC 8 implements the de-facto standard shape (`8;params;URI`, empty URI clears); malformed shapes are consumed and ignored; no `TERM`/DA capability is newly advertised.
- **III. Test-First for the Terminal Core** — PASS. Interning, validation, OSC 8 parsing, viewport resolution, and `find_url_at` are all covered by unit tests feeding bytes/strings in and asserting state out. Renderer underline and process spawning are GPU/OS effects exempt from unit tests per the constitution.
- **IV. Event-Driven Performance** — PASS. Interning is a linear search over at most 1024 short strings, done once per OSC 8 (not per cell or per frame); `print()` stamps a `u16`. Table growth is bounded at 1024 entries x 2048 bytes.
- **V. Safe Handling of Untrusted Output** — PASS. PTY-reachable paths never panic: scheme allow-list (`http://`, `https://`, `mailto:`) blocks `javascript:`/`file:` handler abuse, URIs are capped at 2048 bytes, lossy UTF-8 decoding tolerates invalid bytes, and over-cap links degrade to plain text.
- **Quality Gates** — fmt/clippy/test/build all run before commit.

Complexity Tracking: none.

## Project Structure

```text
specs/013-hyperlinks/{spec.md,plan.md,tasks.md}
src/terminal.rs   # Cell.link, intern table, set_hyperlink, hyperlink_at_viewport, print stamping, tests
src/parser.rs     # OSC 8 in osc_dispatch only, tests
src/renderer.rs   # underline quad when link != 0 (once if also UNDERLINE)
src/window.rs     # find_url_at + tests, open_url, Super/Ctrl-click handling in mouse_press
```

## Approach

1. Terminal: add `link` to `Cell`/`Default`, intern table (`hyperlinks: Vec<String>`, id = index + 1) plus `current_link: u16`; `set_hyperlink` validates scheme/length, dedupes, enforces the 1024 cap; `print()` stamps `current_link` (spacer copies it via `..cell`); `hyperlink_at_viewport` resolves via `get_viewport_cell`.
2. Parser: in `osc_dispatch`, exact 3-param OSC 8 shape maps non-empty URI to `set_hyperlink(Some(..))`, empty to `set_hyperlink(None)`, everything else ignored.
3. Renderer: underline condition becomes `UNDERLINE || link != 0` (single quad).
4. Window: `find_url_at` scans for `http://`/`https://`, ends at whitespace, trims trailing `).,;:!?`, returns the slice covering `byte_index`. Implicit lookup builds the viewport row string skipping `WIDE_SPACER` cells while mapping each cell to its character's byte index (spacer maps to its head). `mouse_press` checks Super/Ctrl left-clicks first (explicit wins), consumes hits, falls through on misses. `open_url` spawns the `cfg(target_os)` opener detached and logs failures.
5. Tests, four gates, commit (no push/merge).
