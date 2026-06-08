# Setup Instructions

## Install Rust

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

## Build and Run

```bash
cargo build
cargo run
```

## Development

```bash
# Check code
cargo check

# Run with release optimizations
cargo run --release

# Run tests
cargo test
```

## Dependencies

The project uses:
- **winit** - Cross-platform window creation
- **wgpu** - GPU-accelerated rendering
- **fontdue** - Font rasterization
- **portable-pty** - PTY (pseudoterminal) handling
- **vte** - ANSI escape sequence parser

## Platform Requirements

- **macOS**: Should work out of the box
- **Linux**: May need `libxkbcommon-dev` and `libwayland-dev`
- **Windows**: Should work out of the box
