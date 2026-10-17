# Implementation Plan: TOML Configuration File

**Branch**: `010-config-file` | **Date**: 2026-10-17 | **Spec**: [spec.md](./spec.md)

## Summary

`Config` becomes serde-deserializable with per-field defaults and is loaded
at startup from `$WISP_CONFIG` / XDG / `$HOME/.config`; any error warns and
falls back to full defaults. Scrollback limit, shell override, and three base
colours flow into the existing terminal/PTY/renderer construction paths.
`cursor_blink` is parsed for feature 007 and unused here.

## Technical Context

- **Language/Version**: Rust, edition 2021
- **Primary Dependencies**: NEW `serde` (derive) + `toml` (see Principle I justification); existing `log` for warnings
- **Testing**: `cargo test`, `#[cfg(test)]` in `src/config.rs` and one test in `src/terminal.rs`
- **Target Platform**: macOS primary; env/path logic uses `PathBuf` only
- **Constraints**: only `src/config.rs`, `src/window.rs`, palette construction in `src/renderer.rs`, scrollback setter in `src/terminal.rs`, shell constructor in `src/pty.rs`, `Cargo.toml`/`Cargo.lock`, `specs/010-config-file/`

## Constitution Check

Against `.specify/memory/constitution.md` v1.0.0:

- **I. Minimal by Default** — PASS with dependency justification: font size, grid size, scrollback, and shell are settings a daily terminal user currently cannot change without recompiling, so a config file is essential, not a bell/whistle; scope is capped at those fields plus three colours (no theming system, no hot-reload, no settings UI). `serde`+`toml` are justified because hand-rolling a TOML parser would be large, error-prone, and unsafe against malformed input, whereas serde's `#[serde(default)]` gives exactly the "every field optional" semantics in a few lines; both crates are the de-facto maintained standards for Rust config parsing. One narrow exception noted: the constitution lists "themes" as out of scope — this feature is explicitly *not* a theme system (three fixed colour slots, no named themes), and this spec explicitly amends scope for those three slots as Principle I permits ("unless a spec explicitly amends this principle"); recorded here as the rationale.
- **II. Terminal Correctness** — PASS. No VT behaviour, DA reply, or `TERM` value changes; the shell override only changes which program is spawned.
- **III. Test-First for the Terminal Core** — PASS. Parsing, hex colours, path resolution, palette layering are pure functions with unit tests; the new `set_scrollback_limit` core method gets a trim test. Renderer/PTY remain test-exempt per the constitution; their changes are one-line wirings.
- **IV. Event-Driven Performance** — PASS. Config is parsed once at startup; no hot path changes; scrollback stays capped (clamped `100..=100000`).
- **V. Safe Handling of Untrusted Output** — PASS. Config is local-user input, but parsing is still total: any read/parse error warns and yields defaults, colour parsing rejects malformed strings, scrollback is clamped; nothing PTY-reachable changes.
- **Quality Gates** — fmt/clippy/test/build all run before commit.

Complexity Tracking: none beyond the two justified dependencies.

## Project Structure

```text
specs/010-config-file/{spec.md,plan.md,tasks.md}
Cargo.toml / Cargo.lock     # + serde (derive), toml
src/config.rs               # Deserialize Config/Colors, load/from_toml_str/resolve_path/parse_hex_color, from_config, tests
src/window.rs               # Config::load(); set_scrollback_limit; PtyManager::with_shell
src/renderer.rs             # ColorPalette::from_config(terminal_config)
src/terminal.rs             # set_scrollback_limit + test
src/pty.rs                  # with_shell constructor; new() delegates with None
```

## Approach

1. Deps + `Config`/`Colors` serde types: struct field defaults via a `Config::default()`-backed `#[serde(default)]` per field (helper default fns), so partial and empty files work.
2. `resolve_path(env: impl Fn(&str) -> Option<String>) -> Option<PathBuf>`; `load()` passes `|k| std::env::var(k).ok()`, reads the file, maps any error to warn + `Self::default()`.
3. `parse_hex_color(&str) -> Option<[f32;4]>`: require `#` + 6 hex digits.
4. `ColorPalette::from_config`: start from `default()`, apply parsed overrides that parse successfully.
5. Terminal/PTY/window/renderer wiring per FR-005..007; `with_shell` keeps the existing spawn body, taking the shell from the argument first.
6. Tests, four gates, commit (no push/merge).
