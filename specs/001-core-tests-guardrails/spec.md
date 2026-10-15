# Feature Specification: Core Tests and CI Guardrails

**Feature Branch**: `001-core-tests-guardrails`
**Created**: 2026-10-14
**Status**: Implemented
**Input**: "Core tests and CI guardrails"

## User Scenarios & Testing

### User Story 1 - Terminal core is pinned by unit tests (Priority: P1)

As a maintainer, I can run `cargo test` and get fast assertions over current
terminal/parser/input behavior so refactors cannot silently change grid,
cursor, SGR, or key encodings.

**Independent Test**: `cargo test` runs >=20 tests covering print/wrap/scroll,
cursor, clears, SGR, selection, parser CSI A/B/C/D/H/J/K/m + mode 2004,
tab/backspace/CR/LF, and every `input.rs` mapping.

**Acceptance Scenarios**:

1. **Given** a fresh Terminal, **When** bytes/chars are fed in, **Then** tests assert grid/cursor/attrs match current behavior.
2. **Given** an input key event, **When** `key_to_bytes` is called, **Then** the exact current byte sequence is returned.

### User Story 2 - Quiet logging and clean lints (Priority: P1)

As a user, normal runs do not spam stdout/stderr; diagnostic output goes
through `log`/`env_logger` and appears only with `RUST_LOG` set.

**Acceptance Scenarios**:

1. **Given** default env, **When** the app runs, **Then** former `println!` sites emit nothing.
2. **Given** `RUST_LOG=debug`, **When** the app runs, **Then** the same diagnostics appear as log records.

### User Story 3 - CI enforces gates on every change (Priority: P2)

As a contributor, every push/PR to `main` runs fmt, clippy, tests, and a
release build on Linux so broken code is caught before merge.

**Acceptance Scenarios**:

1. **Given** a PR to main, **When** CI runs, **Then** fmt-check, clippy `-D warnings -A deprecated`, `cargo test`, and `cargo build --release` execute.

### Edge Cases

- Deprecated winit `EventLoop::run`/`create_window` warnings are an explicit exception (`-A deprecated`); no API migration in this feature.
- Unused-but-planned Terminal APIs are kept, not deleted.
- No runtime behavior changes beyond logging/fmt/clippy fixes.

## Requirements

- **FR-001**: Add `#[cfg(test)]` unit tests inside `src/terminal.rs`, `src/parser.rs`, `src/input.rs` only (no `lib.rs`), at least 20 tests pinning current behavior.
- **FR-002**: Replace `println!`/`eprintln!` in `main.rs`, `window.rs`, `renderer.rs`, `pty.rs` with `log::debug!`/`log::warn!`; add `log` dependency.
- **FR-003**: Run `cargo fmt` repo-wide.
- **FR-004**: Fix clippy `collapsible_match` in `parser.rs`; remove never-read renderer fields if safe; add targeted `#[allow(dead_code)]` (with one-line "upcoming features use them" comment) to the six unused Terminal methods instead of deleting them.
- **FR-005**: Add `.github/workflows/ci.yml` (push/PR to main, ubuntu-latest, stable + rustfmt/clippy, caching, apt `libxkbcommon-dev libwayland-dev libx11-dev`) running fmt-check, clippy `-D warnings -A deprecated`, test, release build.
- **FR-006**: Remove `Cargo.lock` from `.gitignore` and commit `Cargo.lock`.
- **FR-007**: No runtime behavior change beyond logging/fmt/clippy fixes.

## Success Criteria

- **SC-001**: `cargo test` passes with >=20 new tests.
- **SC-002**: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings -A deprecated` pass.
- **SC-003**: `cargo build` passes; `Cargo.lock` is tracked.
