# Tasks: Full-screen VT Support (002)

- [x] T001 Alternate screen (?1049/?1047/?47), active-grid routing, scrollback/viewport rules in src/terminal.rs
- [x] T002 DECSTBM scroll region + region-aware linefeed/scrollback in src/terminal.rs
- [x] T003 index/reverse_index/next_line + save/restore cursor in src/terminal.rs
- [x] T004 insert/delete lines, insert/delete/erase chars, scroll up/down in src/terminal.rs
- [x] T005 resize both grids + clamp region in src/terminal.rs
- [x] T006 Parser dispatch: CSI @/P/X/L/M/S/T/G/d/E/F/r/s/u, ?25/?47/?1047/?1049, ESC D/M/E/7/8 in src/parser.rs
- [x] T007 New #[cfg(test)] tests for every behavior in both files
- [x] T008 Gates: cargo fmt --check, clippy, test, build
