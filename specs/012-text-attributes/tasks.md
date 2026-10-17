# Tasks: Dim, Strikethrough and Underline Styles

**Branch**: `012-text-attributes` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

## Phase 1: Terminal core

- [x] T001 Add `DIM`/`STRIKETHROUGH` bits to `CellFlags` in `src/terminal.rs`; SGR 2/9/22/29 (22 clears bold+dim)
- [x] T002 Terminal tests: `sgr(2)`/`sgr(9)` set flags, `sgr(22)` clears bold+dim, `sgr(29)` clears strike, `sgr(0)` clears both

## Phase 2: Parser

- [x] T003 SGR walker in `src/parser.rs`: colon `4:n` underline styles without re-dispatching sub-style; consume-and-ignore `58` (semicolon + colon) reusing 38/48 skipping
- [x] T004 Parser tests: `ESC[4:2m` sets UNDERLINE not DIM, `ESC[4:0m` clears underline, `ESC[58;5;196m` and `ESC[58:2::255:0:0m` leave fg/bg untouched, `ESC[2m`/`ESC[9m` set flags on printed cells

## Phase 3: Renderer (manual check only)

- [x] T005 Dim fg (×0.6 RGB) and 2px mid-cell strikethrough quad in `src/renderer.rs` `build_vertices`; underline uses dimmed fg. Visual rendering is manual-check only (GPU-exempt per constitution III): verify with `printf '\e[2mdim\e[0m \e[9mstrike\e[0m\n'`

## Phase 4: Verify & commit

- [x] T006 Verify: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build` all pass
- [x] T007 Commit on `012-text-attributes` as "Add dim, strikethrough and underline-style SGR support (012)" (no Co-Authored-By)
