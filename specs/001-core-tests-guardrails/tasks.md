# Tasks: Core Tests and CI Guardrails

**Branch**: `001-core-tests-guardrails` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

## Phase 1: Tests (pin current behavior)

- [x] T001 Add `#[cfg(test)]` tests in `src/terminal.rs`: print, wrap, scroll_up + scrollback limit, cursor moves, clear ops, SGR 16-color/bright/bold/underline/inverse/reset, selection text
- [x] T002 Add `#[cfg(test)]` tests in `src/parser.rs`: CSI A/B/C/D/H/J/K/m, bracketed-paste mode 2004, tab/backspace/CR/LF
- [x] T003 Add `#[cfg(test)]` tests in `src/input.rs` for every existing key mapping + text/character fallback

## Phase 2: Logging & hygiene

- [x] T004 Add `log` dependency to `Cargo.toml` and replace `println!`/`eprintln!` with `log::debug!`/`log::warn!` in `src/main.rs`, `src/window.rs`, `src/renderer.rs`, `src/pty.rs`
- [x] T005 Fix clippy `collapsible_match` in `src/parser.rs`; remove never-read renderer fields after grepping all uses; add targeted `#[allow(dead_code)]` with one-line comment to the six unused Terminal methods
- [x] T006 Run `cargo fmt` on the whole repo

## Phase 3: CI & lockfile

- [x] T007 Add `.github/workflows/ci.yml` (push/PR to main, ubuntu-latest, stable + rustfmt/clippy, cargo cache, apt libxkbcommon-dev/libwayland-dev/libx11-dev, fmt-check / clippy -D warnings -A deprecated / test / build --release)
- [x] T008 Remove `Cargo.lock` from `.gitignore` and track `Cargo.lock`

## Phase 4: Verify & commit

- [x] T009 Verify: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build` all pass
- [x] T010 Commit on `001-core-tests-guardrails` as "Add core tests and CI guardrails (001)" (no Co-Authored-By)
