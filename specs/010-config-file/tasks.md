# Tasks: TOML Configuration File

**Branch**: `010-config-file` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

- [x] T001 Read constitution and current `src/config.rs`, `src/window.rs`, `src/pty.rs`, terminal scrollback fields, renderer palette construction; create feature branch via Spec Kit scripts
- [x] T002 Add `serde` (derive) and `toml` to `Cargo.toml` (justified in plan.md, Principle I)
- [x] T003 Extend `Config` with serde defaults: `scrollback_lines`, `shell`, `cursor_blink`, `[colors]`; implement `from_toml_str`, `resolve_path` (env closure), `load()` (warn + defaults on error)
- [x] T004 Implement `#rrggbb` parser and `ColorPalette::from_config`; point `Renderer::new` at it
- [x] T005 Add `Terminal::set_scrollback_limit` (sets cap, trims) and `PtyManager::with_shell` (`new()` delegates with `None`)
- [x] T006 Wire `WispWindow::new`: `Config::load()`, clamped scrollback limit, configured shell
- [x] T007 Add tests: full/empty/partial/invalid TOML, hex parsing + rejections, path precedence via closure, palette overrides, scrollback trim
- [x] T008 Run gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build`
- [x] T009 Commit on `010-config-file` as `Add TOML configuration file support (010)` (no push, no merge, no Co-Authored-By trailer)
