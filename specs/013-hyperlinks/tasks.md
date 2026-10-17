# Tasks: OSC 8 Hyperlinks and URL Opening

**Branch**: `013-hyperlinks` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

- [x] T001 Read constitution and current `src/terminal.rs`, `src/parser.rs`, `src/renderer.rs`, `src/window.rs`; create feature via Spec Kit scripts
- [x] T002 Terminal: `Cell.link: u16` (Copy preserved, default 0), intern table + `current_link`, `set_hyperlink` (scheme allow-list, 2048-byte cap, dedupe, 1024-entry cap), `print()` stamps the id (spacers included), `hyperlink_at_viewport`
- [x] T003 Parser: OSC 8 in `osc_dispatch` (3-param shape, empty URI clears, lossy UTF-8, malformed ignored)
- [x] T004 Renderer: underline quad when `link != 0`, drawn once if `UNDERLINE` also set
- [x] T005 Window: pure `find_url_at` (http/https, whitespace end, trailing `).,;:!?` trim, no bare `www.`), implicit viewport-row lookup skipping spacers, `open_url` via `cfg(target_os)` `Command`, Super/Ctrl left-click handling in `mouse_press` (explicit wins, hits consumed, misses fall through to 008 behaviour)
- [x] T006 Tests: OSC 8 set/clear, `javascript:`/`file:` rejection, interning dedupe/distinctness, link 0 before/after span, viewport resolution (incl. scroll offset), `find_url_at` start/middle/end, `None` outside, punctuation trim, bare `www.`
- [x] T007 Run gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, `cargo build`
- [x] T008 Commit on `013-hyperlinks` as `Add OSC 8 hyperlinks with Cmd/Ctrl-click open (013)` (no push, no merge, no Co-Authored-By trailer)
