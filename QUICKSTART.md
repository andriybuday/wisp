# Wisp Terminal - Quick Start

## Installation

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Clone the repository**:
   ```bash
   git clone https://github.com/andriybuday/wisp.git
   cd wisp
   ```

3. **Build and run**:
   ```bash
   cargo run --release
   ```

## First Steps

When the terminal opens:
1. You'll see your shell prompt
2. Type commands as normal
3. Try: `ls`, `echo "Hello"`, `clear`
4. Press Cmd+Q (or close window) to quit

## What Works Now

- ✅ Full shell integration
- ✅ Text rendering
- ✅ Keyboard input (including arrows)
- ✅ Basic ANSI sequences
- ✅ Auto-scrolling

## What's Coming

- ⏳ Colors
- ⏳ Text selection
- ⏳ Copy/paste
- ⏳ Cursor rendering

## Troubleshooting

**Q: Nothing appears when I type**  
A: Make sure the window has focus. Click on it first.

**Q: Colors don't work**  
A: Color rendering is not fully implemented yet.

**Q: Text looks weird**  
A: Some complex ANSI sequences aren't supported yet.

## Need Help?

See [STATUS.md](STATUS.md) for detailed feature status  
See [TEST.md](TEST.md) for testing guide  
See [ARCHITECTURE.md](ARCHITECTURE.md) for technical details
