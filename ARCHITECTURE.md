# Wisp Architecture

## Overview
Wisp is built with Rust for performance and safety, using a modular architecture.

## Components

### 1. Window Manager (`window.rs`)
- Creates and manages the application window
- Handles resize events
- Manages the render surface

### 2. Renderer (`renderer.rs`)
- GPU-accelerated text rendering using wgpu
- Manages glyph cache
- Renders terminal grid with colors and formatting
- Handles cursor drawing

### 3. Terminal State (`terminal.rs`)
- Maintains the character grid
- Stores scrollback buffer
- Tracks cursor position
- Manages text attributes (colors, bold, etc.)

### 4. PTY Manager (`pty.rs`)
- Spawns shell process
- Handles bidirectional I/O with shell
- Manages PTY resize
- Process cleanup

### 5. ANSI Parser (`parser.rs`)
- Parses ANSI escape sequences using vte crate
- Updates terminal state based on parsed commands
- Handles CSI, OSC, and control sequences

### 6. Input Handler (`input.rs`)
- Keyboard input processing
- Mouse events (selection)
- Clipboard integration
- Maps keys to appropriate sequences

### 7. Selection Manager (`selection.rs`)
- Text selection logic
- Copy to clipboard
- Mouse-based selection

## Data Flow

```
User Input → Input Handler → PTY Manager → Shell
                                             ↓
Renderer ← Terminal State ← ANSI Parser ← Shell Output
    ↓
  Window
```

## Module Structure

```
src/
├── main.rs           # Entry point, event loop
├── window.rs         # Window management
├── renderer.rs       # GPU rendering
├── terminal.rs       # Terminal state
├── pty.rs           # PTY handling
├── parser.rs        # ANSI parsing
├── input.rs         # Input handling
├── selection.rs     # Text selection
└── config.rs        # Basic configuration
```

## Key Design Decisions

1. **GPU Rendering**: Use wgpu for cross-platform GPU acceleration
2. **Immediate Mode**: Render entire grid each frame (simple, fast enough)
3. **Grid-based**: Fixed-width characters in a 2D grid
4. **Async I/O**: Non-blocking PTY communication
5. **Single Window**: One terminal per process (MVP simplicity)

## Performance Targets

- 60 FPS rendering
- < 10ms input latency
- Support 80x24 up to 500x200 grid sizes
- 1000 line scrollback buffer
