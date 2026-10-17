# Feature Specification: Event-Driven Rendering and Cursor Blink

**Feature Branch**: `007-event-driven-rendering`
**Created**: 2026-10-17
**Status**: Implemented
**Input**: "Event-driven rendering and cursor blink"

## User Scenarios & Testing

### User Story 1 - Idle terminal uses near-zero CPU (Priority: P1)

As a terminal user leaving wisp open, the event loop sleeps instead of
busy-polling and redrawing every iteration, so an idle window costs
near-zero CPU. PTY output wakes the loop promptly via a user event.

**Independent Test**: manual — launch wisp, observe CPU in Activity Monitor
while idle, then run `ls` and confirm output appears without delay.

**Acceptance Scenarios**:

1. **Given** an idle prompt, **When** no input/output occurs, **Then** the loop is in `Wait`/`WaitUntil` (no `Poll`, no unconditional redraw).
2. **Given** the loop is sleeping, **When** the shell produces output, **Then** the PTY reader waker posts `PtyOutput`, the bytes are parsed, and a redraw is requested.
3. **Given** a resize, selection drag, scroll, or title change, **When** it occurs, **Then** exactly a redraw is requested for that change (no continuous redraw).

### User Story 2 - Cursor blinks and does not hide the glyph (Priority: P1)

As a user, the block cursor blinks with a 530 ms period, and while it is
shown the character under it remains readable.

**Independent Test**: manual — position the cursor over a character and
observe the 530 ms blink; the glyph stays visible in the background color
against the cursor-colored cell while the cursor is on.

**Acceptance Scenarios**:

1. **Given** a visible cursor, **When** 530 ms elapse, **Then** the blink phase flips and a redraw is requested.
2. **Given** the cursor over a non-space glyph while blink is on, **Then** the cell background is the cursor color and the glyph is drawn in the palette background color.
3. **Given** the cursor hidden via DEC `?25l`, **Then** no cursor cell is drawn regardless of blink phase.

### User Story 3 - Synchronized output avoids tearing (Priority: P2)

As a user of apps using DEC synchronized output (`?2026`, e.g. modern TUIs),
partial frames are not presented mid-update, without ever freezing if the
app fails to end the sync.

**Independent Test**: unit tests assert `CSI ?2026h/l` set/clear the
terminal flag; manual check with a sync-emitting app confirms no tearing.

**Acceptance Scenarios**:

1. **Given** `?2026` is set, **When** PTY bytes arrive, **Then** they are parsed but not presented.
2. **Given** `?2026` is set, **When** 100 ms pass without it clearing, **Then** the frame is presented anyway (failsafe).
3. **Given** a held frame, **When** `?2026` is cleared, **Then** the frame is presented promptly.

### User Story 4 - Robust, efficient rendering (Priority: P2)

As a user with large windows/grids, rendering does not corrupt geometry
past 65k vertices, does not reallocate GPU buffers every frame, and
recovers from swapchain loss instead of crashing.

**Acceptance Scenarios**:

1. **Given** 20,000 quads, **Then** `build_indices` ends at the correct u32 values without wrapping.
2. **Given** a lost/outdated surface, **Then** the renderer reconfigures and returns; timeout and out-of-memory are handled/logged without panicking.

### Edge Cases

- Empty frames still clear/present as before.
- Sync failsafe repeats: a frame held past 100 ms is presented, and a later hold gets a fresh 100 ms window.
- Buffer capacities only grow; smaller later frames reuse the same buffers via partial slices.

## Requirements

### Functional Requirements

- **FR-001**: The PTY reader thread MUST be able to wake the event loop without changing `PtyManager::new`'s signature: a shared waker cell, invoked after each successful send, set via `PtyManager::set_waker` by the window right after construction.
- **FR-002**: `main` MUST use `EventLoop::with_user_event` with `WispUserEvent::PtyOutput` and an `EventLoopProxy`; `WispWindow::new`'s event-loop type parameter MUST match.
- **FR-003**: The loop MUST use `Wait`/`WaitUntil`, never `Poll`; redraws occur only for PTY output, blink flips, resize, selection/scroll/title-relevant input, or sync-hold expiry.
- **FR-004**: Cursor blink MUST use a 530 ms period via `WaitUntil` deadlines tracked in the window, passed into rendering; the glyph under a shown cursor MUST render in the palette background color over the cursor-colored cell background.
- **FR-005**: `Terminal` MUST carry a `sync_output` flag set/cleared by DECSET/DECRST `?2026`; while set the window parses but does not present, with a 100 ms `WaitUntil` failsafe that presents anyway.
- **FR-006**: Renderer indices MUST be u32 (`IndexFormat::Uint32`); vertex/index buffers MUST persist on the renderer, be recreated only when capacity must grow, and be updated via `queue.write_buffer`; surface errors MUST be handled without panicking.
- **FR-007**: `WispWindow` MUST cache physical cell metrics (computed in `new`, refreshed in `resize`) so mouse positioning constructs no `FontManager` per move.

## Success Criteria

### Measurable Outcomes

- **SC-001**: New unit tests pass: parser sets/clears `?2026`; renderer `build_indices` for 20,000 quads ends at correct u32 values; all 98 pre-existing tests still pass.
- **SC-002**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -A deprecated`, `cargo test`, and `cargo build` all pass.
- **SC-003**: Manual verification (recorded in tasks): idle CPU near zero, output latency unchanged, 530 ms blink with visible glyph under cursor, resize/selection/scroll redraw correctly.

## Assumptions

- Blink wakes every 530 ms even when the cursor is hidden (flipping phase without a redraw when hidden) — negligible CPU and keeps the deadline logic single-sourced.
- Event-loop/blink behavior cannot be unit-tested without a display per constitution Principle III; it is manual-verification only. Pure logic (`?2026`, indices) is unit-tested.
