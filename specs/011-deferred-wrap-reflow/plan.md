# Plan: Deferred wrap and resize reflow (011)

Only `src/terminal.rs` changes (no parser/renderer/window/input).
State: `pending_wrap: bool`, `wrapped`/`alt_wrapped: Vec<bool>`,
scrollback becomes `Vec<(Vec<Cell>, bool)>`. `print` sets pending instead of
feeding at the last cell; `wrap_to_next_line` is the single wrap path and
marks the destination wrapped. Scroll/insert/delete shift flags with cells.
`resize` builds logical lines, re-chunks wide-safely, recomputes cursor
from (logical line, cell offset), resets region/offset/pending.

## Constitution Check

- I Minimal: no new dependency, no new file, core-only. PASS
- II Correctness: deferred wrap is xterm semantics; reflow preserves text
  and wide pairs. PASS
- III Test-first core: new `#[cfg(test)]` tests for every behaviour;
  tests pinning immediate wrap / resize-region clamping updated. PASS
- IV Performance: reflow allocates only on resize (not a hot path);
  scrollback stays capped. PASS
- V Safety: no panicking indexing on hostile sizes (zero-size guarded,
  offsets clamped). PASS
