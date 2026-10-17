# Feature Specification: Deferred wrap and resize reflow

**Feature Branch**: `011-deferred-wrap-reflow` **Status**: Implemented

## User Stories

- US1 (P1) Deferred (pending) wrap matching xterm: printing into the last
  cell leaves the cursor on that cell with `pending_wrap` set; the next
  printable wraps first. Explicit movement/linefeed/CR/backspace/tab,
  clears, scroll, insert/delete, alt switches and resize clear it.
  An exactly-cols line followed by LF produces no blank row.
- US2 (P1) Per-row wrapped (continuation) flags per grid, carried into
  scrollback as `(Vec<Cell>, bool)` on scroll-up; clears reset flags.
- US3 (P1) Resize reflow: join scrollback+grid to logical lines, trim
  trailing blank filler (space, default bg, no flags; spacers stay with
  their wide char), re-chunk to the new width never splitting wide/spacer
  (pad the gap), last `new_rows` lines become the grid and earlier lines
  scrollback (capped). Alt grid reflows independently, overflow dropped.
  Cursor remapped via its logical line and cell offset; region resets to
  full screen, `scroll_offset` and `pending_wrap` reset.

## Acceptance (unit tests in src/terminal.rs)

Deferred wrap (fill exactly/no scroll, next-char wraps, CR cancels,
exact-line+LF no blank, wide-at-edge pending); wrapped flags (continuation,
newline, into scrollback); reflow narrower/wider/scrollback/wide-no-split,
alt does not pollute scrollback, cursor remap, unchanged-size idempotency.

Trailing blank logical lines below the last non-blank/cursor line are free
space: they are dropped before re-chunking and the grid is blank-padded at
the bottom, so growing/shrinking does not push content behind filler.
