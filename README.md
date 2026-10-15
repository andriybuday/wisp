# Wisp

A minimalist terminal emulator.

## Status
🚧 Work in progress - MVP development

### Current Progress
- ✅ Basic window creation
- ✅ GPU-accelerated rendering (wgpu)
- ✅ Text rendering with monospace font
- ⏳ PTY integration (shell spawning)
- ⏳ ANSI escape sequence parsing
- ⏳ Keyboard input handling
- ⏳ Text selection & clipboard

## Goals
- Simple and focused
- Fast and lightweight
- Clean codebase
- Essential features only

## Building

### Prerequisites
- Rust (install from https://rustup.rs)
- macOS/Linux/Windows

### Build and Run
```bash
cargo build --release
cargo run --release
```

### Development
```bash
# Quick build (debug mode)
cargo build

# Run
cargo run

# Check for errors
cargo check
```

### Spec-Driven Development (GitHub Spec Kit)

This project uses [GitHub Spec Kit](https://github.com/github/spec-kit) for
spec-driven development. The governing principles live in
[.specify/memory/constitution.md](.specify/memory/constitution.md); feature
artifacts go under `specs/NNN-<feature>/`.

Prerequisite: the `specify` CLI (`specify check` verifies your setup).
Spec Kit skills are installed for Claude Code under `.claude/skills/`;
add another agent later with `specify integration install <agent>`.

Workflow for a new feature, from your coding agent:

1. `/speckit-specify` — create the feature specification
2. `/speckit-clarify` — (optional) resolve ambiguities before planning
3. `/speckit-plan` — create the implementation plan
4. `/speckit-tasks` — generate actionable tasks
5. `/speckit-analyze` — (optional) check cross-artifact consistency
6. `/speckit-implement` — execute the implementation

## Current Features

✅ Basic window and rendering  
✅ Shell integration (PTY)  
✅ Keyboard input  
✅ ANSI escape sequences  
✅ Scrollback buffer  
✅ Color rendering (16 ANSI colors + bright variants)  
✅ Text attributes (bold, underline, inverse)  
✅ Visual cursor  
✅ PTY resize on window resize  
✅ Text selection (mouse drag)  
✅ Copy/paste (Cmd+C / Cmd+V on macOS)  
✅ Scrollback navigation (mouse wheel)  

See [STATUS.md](STATUS.md) for detailed progress.

## Tech Stack
- Rust
- wgpu (GPU rendering)
- winit (windowing)
- portable-pty (shell integration)
- vte (ANSI parsing)

## License
MIT
