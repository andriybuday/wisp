# Feature Specification: OSC 8 Hyperlinks and URL Opening

**Feature Branch**: `013-hyperlinks`
**Created**: 2026-10-17
**Status**: Implemented
**Input**: "OSC 8 hyperlinks and URL opening"

## User Scenarios & Testing

### User Story 1 - Open links emitted by programs (Priority: P1)

Programs such as `ls --hyperlink`, `grep`, and GitHub CLI emit OSC 8
hyperlinks. The linked text is underlined, and Cmd/Ctrl-clicking it opens the
URI in the system handler instead of forcing a copy-paste.

**Acceptance Scenarios**:

1. **Given** `OSC 8;;https://example.com ST text OSC 8;; ST`, **When** parsed, **Then** the cells of `text` carry the interned link id, cells before/after the span carry link `0`, and the empty-URI OSC 8 clears the active link.
2. **Given** a cell with a link, **When** rendered, **Then** the existing underline quad is drawn (once, even if SGR underline is also set).
3. **Given** a Super/Ctrl left-click on a linked viewport cell, **When** handled, **Then** the URI opens via the OS opener and the click neither starts a selection nor is reported to the application.

### User Story 2 - Open plain URLs printed as text (Priority: P1)

Most output contains bare `http://`/`https://` URLs with no OSC 8 markup.

**Acceptance Scenarios**:

1. **Given** a viewport row containing a plain URL and no explicit link on the clicked cell, **When** Super/Ctrl-clicked on a cell covered by the URL, **Then** that URL opens.
2. **Given** a URL followed by sentence punctuation (`).,;:!?`), **Then** the trailing punctuation is not part of the opened URL.
3. **Given** bare `www.example.com` text, **Then** it is never treated as a URL (explicit scheme required).

### User Story 3 - Untrusted output cannot create dangerous links (Priority: P1)

**Acceptance Scenarios**:

1. **Given** OSC 8 with `javascript:`, `file:`, or any scheme other than `http://`, `https://`, `mailto:`, **Then** it is rejected like an empty URI (link cleared/ignored).
2. **Given** a URI longer than 2048 bytes or a malformed OSC 8 shape, **Then** it is consumed and ignored without panic.
3. **Given** more than 1024 distinct URIs, **Then** further new links are not interned and their text prints without a link (bounded table).

### Edge Cases

- The same URI interned twice yields one id (linear-search dedupe); distinct URIs get distinct ids.
- Wide-character spacer cells carry the same link id as their head cell; implicit detection skips spacer cells when building row text and maps a click on a spacer to its head character.
- `hyperlink_at_viewport(col, row)` resolves through `get_viewport_cell`, so scrollback offset is honoured.
- Opening spawns the platform opener detached (`open` on macOS, `xdg-open` on Linux, `cmd /C start` on Windows); spawn errors are logged and ignored.
- Malformed OSC 8 (wrong parameter count) is ignored; the URI is decoded with `from_utf8_lossy`.

### Click-Semantics Decision (recorded per feature 008 interaction)

- Only a **left-button click with Super or Ctrl held** attempts link opening, and it is checked **before** mouse reporting and selection in `mouse_press`.
- If that click **hits** an explicit hyperlink or an implicit URL, the URL opens and the click is consumed: it does **not** start a selection and is **not** reported to the application (even when mouse tracking is active).
- If that click **misses** every link, it falls through to the existing 008 behaviour unchanged (reported to the application when tracking is active, otherwise starts a local selection).
- Plain clicks (no Super/Ctrl) keep 008 behaviour exactly unchanged.

## Requirements

- **FR-001**: `Cell` stays `Copy` and gains `pub link: u16` (0 = no link); `Cell::default()` uses 0.
- **FR-002**: `Terminal` owns an intern table `Vec<String>` capped at 1024 entries. `set_hyperlink(Option<&str>)` validates (scheme `http://`/`https://`/`mailto:` only, max 2048 bytes, non-empty), dedupes by linear search, and stores the current link id that `print()` stamps into every cell (spacers included). Invalid/over-cap/`None`/empty input clears the current link.
- **FR-003**: `hyperlink_at_viewport(col, row) -> Option<&str>` resolves a viewport cell's link id through `get_viewport_cell`.
- **FR-004**: Parser `osc_dispatch`: params `["8", _params, uri]` call `set_hyperlink(Some(uri))` for non-empty `uri`, `set_hyperlink(None)` for empty; other shapes are ignored; URI decoded with `from_utf8_lossy`. No other OSC behaviour changes.
- **FR-005**: Renderer draws the existing underline quad for a cell with `link != 0` even without the `UNDERLINE` flag, drawn once if both apply. No other renderer change.
- **FR-006**: `window::mouse_press` implements the click-semantics decision above; explicit hyperlink wins over implicit detection on the same cell.
- **FR-007**: Pure `find_url_at(row: &str, byte_index: usize) -> Option<&str>` in `window.rs` finds `http://`/`https://` URLs (terminated by whitespace or end of row), trims trailing `).,;:!?`, returns the URL only when `byte_index` is covered, and never matches bare `www.` text.
- **FR-008**: Opening uses `std::process::Command` with `cfg(target_os)` (`open` / `xdg-open` / `cmd /C start`), spawned detached; errors are logged via `log::warn!` and otherwise ignored.
- **FR-009**: Mouse reporting encoders in `input.rs` are not modified.

## Success Criteria

- **SC-001**: All 124 pre-existing tests pass unchanged; new tests cover OSC 8 set/clear, scheme rejection, interning dedupe/distinctness, link-0 outside the span, viewport resolution, and `find_url_at` (http/https at start/middle/end, `None` outside, trailing-punctuation trim, bare `www.` rejection).
- **SC-002**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, and `cargo build` pass.

## Assumptions

- OSC 8 follows the de-facto standard (`OSC 8 ; params ; URI ST`, empty URI ends the link); the `id=` parameter is accepted and ignored because interning by URI already gives span identity the behaviour users need.
- Scheme allow-listing is the security boundary: shell output is untrusted, so only web/mail handlers may be invoked.
- No hover styling, tooltips, or link context menus are in scope (minimal by default).
