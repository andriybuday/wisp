# Testing Wisp Terminal

## Manual Testing Checklist

### Basic Functionality
- [ ] Window opens
- [ ] Shell prompt appears
- [ ] Can type characters
- [ ] Enter key sends command
- [ ] Output appears on screen

### Commands to Test
```bash
# Basic commands
echo "Hello, World!"
ls -la
pwd
date

# Color output
ls --color=auto

# Interactive programs
python3
>>> print("test")
>>> exit()

# Cursor movement
vim
# Press i, type, press ESC, :q!

# Clear screen
clear

# Long output (scrolling)
cat /etc/profile
```

### Keyboard Tests
- [ ] Arrow keys move cursor
- [ ] Backspace deletes characters
- [ ] Tab completion works
- [ ] Ctrl+C interrupts
- [ ] Enter submits command

### Expected Issues (Known)
- Colors may not display correctly
- Bold/italic text not visible
- No cursor visible
- Cannot select text
- Cannot copy/paste
- Some escape sequences may not work

## Automated Test

Run this one-liner to test basic functionality:
```bash
echo "=== Wisp Test ===" && \
echo "Date: $(date)" && \
echo "User: $USER" && \
echo "Shell: $SHELL" && \
echo "PWD: $(pwd)" && \
ls -1 | head -5
```

Expected: Should see all output formatted correctly in the terminal.
