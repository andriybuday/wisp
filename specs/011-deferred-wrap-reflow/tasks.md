# Tasks: Deferred wrap and resize reflow (011)

- [x] T001 [US1] `pending_wrap` in print + clear on all explicit ops (src/terminal.rs)
- [x] T002 [US1] Update immediate-wrap tests to deferred semantics
- [x] T003 [US2] Wrapped flags per grid + scrollback `(Vec<Cell>, bool)` + shifts/clears
- [x] T004 [US3] Reflow resize for main (scrollback) and alt (drop overflow) + cursor remap
- [x] T005 [US1-3] New unit tests listed in spec.md
- [x] T006 Gates: cargo fmt --check, clippy -D warnings -A deprecated, test, build
