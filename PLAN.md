# Wisp Terminal Emulator - MVP Plan

## Project Overview
Wisp is a minimalist terminal emulator focusing on simplicity and core functionality.

## MVP Goals
Create a functional terminal emulator with essential features only - no bells and whistles.

## Tech Stack Options
- **macOS**: Swift + AppKit (native) or Electron/Tauri (cross-platform)
- **Cross-platform**: Rust + Tauri or Electron + TypeScript
- **Recommendation**: Start with native macOS (Swift) for best performance

## Core Features for MVP (Iteration 1)

### 1. Basic Window Management
- [ ] Single window application
- [ ] Resizable window with minimum size constraints
- [ ] Basic menu bar (File, Edit, Window, Help)
- [ ] Quit command

### 2. Terminal Display
- [ ] Monospace text rendering
- [ ] Fixed color scheme (dark background, light text)
- [ ] Scrollback buffer (1000 lines)
- [ ] Text selection with mouse
- [ ] Copy to clipboard

### 3. PTY Integration
- [ ] Spawn shell process (default user shell)
- [ ] Bidirectional communication with shell
- [ ] Handle PTY resize on window resize
- [ ] Proper cleanup on exit

### 4. Input Handling
- [ ] Keyboard input to shell
- [ ] Special keys (arrows, backspace, delete, etc.)
- [ ] Ctrl/Cmd key combinations
- [ ] Paste from clipboard (Cmd+V)

### 5. Basic Terminal Emulation
- [ ] ANSI escape sequence parsing
- [ ] Basic colors (16 colors)
- [ ] Cursor positioning
- [ ] Clear screen
- [ ] Text formatting (bold, italic, underline)

## Non-Goals for MVP
- Tabs/splits
- Themes/customization
- GPU acceleration
- Ligatures
- Image rendering
- Hyperlinks
- Search functionality
- Configuration file

## Implementation Phases

### Phase 1: Window + Text Display (Week 1)
- Create basic window
- Render static text in monospace font
- Handle window resize

### Phase 2: PTY Integration (Week 1-2)
- Spawn shell process
- Display shell output
- Handle basic input

### Phase 3: Terminal Emulation (Week 2-3)
- Implement ANSI parser
- Handle colors and formatting
- Cursor movement

### Phase 4: Polish (Week 3-4)
- Text selection
- Copy/paste
- Scrollback
- Bug fixes

## Success Criteria
- Can run basic commands (ls, cd, vim, etc.)
- Text is readable and properly formatted
- Colors work correctly
- Can scroll through history
- Stable (no crashes during normal use)

## Future Iterations (Post-MVP)
- Multiple tabs
- Split panes
- Theme support
- Configuration file
- Font selection
- Performance optimization
- Cross-platform support
