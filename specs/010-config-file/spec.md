# Feature Specification: TOML Configuration File

**Feature Branch**: `010-config-file`
**Created**: 2026-10-17
**Status**: Implemented
**Input**: "TOML configuration file"

## User Scenarios & Testing

### User Story 1 - Tune basics without recompiling (Priority: P1)

As a daily terminal user, I set font size, initial grid size, padding,
scrollback length, and shell in one TOML file instead of editing Rust code.

**Acceptance Scenarios**:

1. **Given** a file setting every field, **When** `Config::from_toml_str` parses it, **Then** every field is applied.
2. **Given** an empty/missing file, **When** config loads, **Then** today's defaults apply (font 14, 80x24, padding 4, scrollback 1000, shell from `$SHELL`, cursor blink on).
3. **Given** a partial file, **When** parsed, **Then** unset fields keep defaults.

### User Story 2 - Broken config never breaks startup (Priority: P1)

**Acceptance Scenarios**:

1. **Given** invalid TOML, **When** `Config::load()` runs, **Then** it logs `log::warn!` with path+error and returns full defaults (no partial application, no panic).
2. **Given** `from_toml_str` invalid input, **Then** it returns `Err(toml::de::Error)`.

### User Story 3 - Override base colours (Priority: P2)

**Acceptance Scenarios**:

1. **Given** `[colors]` with `background`/`foreground`/`cursor` as `#rrggbb`, **When** the renderer builds its palette via `ColorPalette::from_config`, **Then** those three colours (alpha 1.0) replace the defaults; ANSI 16 and unset entries are unchanged.
2. **Given** a malformed colour string, **Then** the pure hex parser rejects it (`None`) and defaults are kept.

### Edge Cases

- Path precedence: `$WISP_CONFIG` > `$XDG_CONFIG_HOME/wisp/config.toml` > `$HOME/.config/wisp/config.toml`; no path resolvable (no env at all) returns defaults.
- `scrollback_lines` is clamped to `100..=100000` when applied to the terminal.
- `cursor_blink` is parsed and stored now but **consumed by feature 007 (event-driven rendering)**; it changes no behaviour in this feature.
- No file is required for a first run; no UI for editing config is added.

## Requirements

- **FR-001**: Add `serde` (derive) and `toml` deps, justified in plan.md (Principle I). `Config` derives `Deserialize` with `#[serde(default)]` so every field is optional: `font_size`, `cols`, `rows`, `padding`, `scrollback_lines` (1000), `shell: Option<String>`, `cursor_blink: bool = true`, `[colors]` (`background`/`foreground`/`cursor: Option<String>`).
- **FR-002**: `Config::from_toml_str(&str) -> Result<Config, toml::de::Error>` is the testable core; `Config::load()` reads the resolved path, returns defaults on missing file silently, warns + full defaults on any error.
- **FR-003**: Pure `resolve_path` takes an env-lookup closure (tests never mutate process env), implementing the precedence above.
- **FR-004**: Pure `#rrggbb` parser returns `[f32;4]` (alpha 1.0) or rejects; `ColorPalette::from_config(&Config)` layers overrides on `Default`; `Renderer::new` uses it.
- **FR-005**: `Terminal::set_scrollback_limit(limit)` sets the cap (callers clamp) and trims existing scrollback from the front; the window calls it after `Terminal::new` with the clamped config value.
- **FR-006**: `PtyManager::with_shell(cols, rows, Option<String>)` spawns the override shell (falling back to `$SHELL`/`/bin/bash`); `new()` delegates with `None`.
- **FR-007**: `WispWindow::new` uses `Config::load()` and `with_shell(.., config.shell.clone())`.

## Success Criteria

- **SC-001**: All 98 pre-existing tests pass; new tests cover full/empty/partial/invalid TOML, hex parsing (`#000000`, `#ffffff`, `#ff8000`, rejections), path precedence via closure, palette overrides, and scrollback trimming.
- **SC-002**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build` pass.

## Assumptions

- TOML is the Rust-ecosystem standard for hand-edited config; serde defaults make forward-compatible optional fields trivial.
- `cursor_blink` exists solely for feature 007; shipping it parsed-but-unused avoids a second config-format change later.
