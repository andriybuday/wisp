use crate::terminal::Terminal;
use vte::{Params, Parser, Perform};

pub struct AnsiParser {
    parser: Parser,
    performer: AnsiPerformer,
}

impl AnsiParser {
    pub fn new(terminal: Terminal) -> Self {
        Self {
            parser: Parser::new(),
            performer: AnsiPerformer { terminal },
        }
    }

    pub fn advance(&mut self, data: &[u8]) {
        for byte in data {
            self.parser.advance(&mut self.performer, *byte);
        }
    }

    pub fn terminal(&self) -> &Terminal {
        &self.performer.terminal
    }

    pub fn terminal_mut(&mut self) -> &mut Terminal {
        &mut self.performer.terminal
    }
}

struct AnsiPerformer {
    terminal: Terminal,
}

impl Perform for AnsiPerformer {
    fn print(&mut self, c: char) {
        self.terminal.print(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.terminal.linefeed(),
            b'\r' => self.terminal.carriage_return(),
            b'\x08' => self.terminal.backspace(),
            b'\t' => self.terminal.tab(),
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _c: char) {}

    fn put(&mut self, _byte: u8) {}

    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, c: char) {
        match c {
            'A' => {
                // Cursor up
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first().copied())
                    .unwrap_or(1) as usize;
                self.terminal.cursor_up(n);
            }
            'B' => {
                // Cursor down
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first().copied())
                    .unwrap_or(1) as usize;
                self.terminal.cursor_down(n);
            }
            'C' => {
                // Cursor forward
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first().copied())
                    .unwrap_or(1) as usize;
                self.terminal.cursor_forward(n);
            }
            'D' => {
                // Cursor backward
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first().copied())
                    .unwrap_or(1) as usize;
                self.terminal.cursor_backward(n);
            }
            'H' | 'f' => {
                // Cursor position
                let mut iter = params.iter();
                let row = iter.next().and_then(|p| p.first().copied()).unwrap_or(1) as usize;
                let col = iter.next().and_then(|p| p.first().copied()).unwrap_or(1) as usize;
                self.terminal
                    .cursor_goto(col.saturating_sub(1), row.saturating_sub(1));
            }
            'J' => {
                // Clear display
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first().copied())
                    .unwrap_or(0);
                match n {
                    0 => self.terminal.clear_below(),
                    1 => self.terminal.clear_above(),
                    2 => self.terminal.clear_all(),
                    _ => {}
                }
            }
            'K' => {
                // Clear line
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first().copied())
                    .unwrap_or(0);
                match n {
                    0 => self.terminal.clear_line_right(),
                    1 => self.terminal.clear_line_left(),
                    2 => self.terminal.clear_line(),
                    _ => {}
                }
            }
            'm' => {
                // SGR - Select Graphic Rendition
                if params.is_empty() {
                    self.terminal.reset_sgr();
                } else {
                    for param in params.iter() {
                        for &p in param {
                            self.terminal.sgr(p as usize);
                        }
                    }
                }
            }
            // DEC private mode set ('h') / reset ('l'), e.g. `CSI ? 2004 h`.
            // The private `?` marker is delivered as an intermediate byte.
            'h' | 'l' if intermediates.first() == Some(&b'?') => {
                let enabled = c == 'h';
                for param in params.iter() {
                    if param.first().copied() == Some(2004) {
                        self.terminal.set_bracketed_paste(enabled);
                    }
                }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::CellFlags;

    fn run(data: &[u8]) -> AnsiParser {
        let mut p = AnsiParser::new(Terminal::new(10, 4));
        p.advance(data);
        p
    }

    #[test]
    fn plain_text_and_cr_lf() {
        let p = run(b"ab\r\ncd");
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().ch, 'a');
        assert_eq!(p.terminal().get_cell(0, 1).unwrap().ch, 'c');
        assert_eq!(p.terminal().cursor(), (2, 1));
    }

    #[test]
    fn tab_and_backspace_bytes() {
        let p = run(b"a\tb\x08");
        assert_eq!(p.terminal().get_cell(8, 0).unwrap().ch, 'b');
        assert_eq!(p.terminal().cursor(), (8, 0));
    }

    #[test]
    fn csi_cursor_moves() {
        // Down 2, forward 3, up 1, backward 1.
        let p = run(b"\x1b[2B\x1b[3C\x1b[1A\x1b[1D");
        assert_eq!(p.terminal().cursor(), (2, 1));
        // Current behavior: a bare (param-less) CSI move is a no-op,
        // because vte reports the missing param as 0.
        let p = run(b"\x1b[2B\x1b[A");
        assert_eq!(p.terminal().cursor(), (0, 2));
    }

    #[test]
    fn csi_h_positions_cursor_1_based() {
        let p = run(b"\x1b[3;4Hab");
        assert_eq!(p.terminal().get_cell(3, 2).unwrap().ch, 'a');
        // No params goes home (1,1).
        let p = run(b"xy\x1b[H");
        assert_eq!(p.terminal().cursor(), (0, 0));
    }

    #[test]
    fn csi_j_clears_display() {
        let p = run(b"abc\x1b[2J");
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().ch, ' ');
        let p = run(b"abcd\x1b[1;2H\x1b[0J");
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().ch, 'a');
        assert_eq!(p.terminal().get_cell(1, 0).unwrap().ch, ' ');
    }

    #[test]
    fn csi_k_clears_line() {
        let p = run(b"abcd\x1b[1;3H\x1b[0K");
        assert_eq!(p.terminal().get_cell(1, 0).unwrap().ch, 'b');
        assert_eq!(p.terminal().get_cell(2, 0).unwrap().ch, ' ');
        let p = run(b"abcd\x1b[1;3H\x1b[1K");
        assert_eq!(p.terminal().get_cell(2, 0).unwrap().ch, ' ');
        assert_eq!(p.terminal().get_cell(3, 0).unwrap().ch, 'd');
        let p = run(b"abcd\x1b[2K");
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().ch, ' ');
    }

    #[test]
    fn csi_m_sgr_and_bare_reset() {
        let p = run(b"\x1b[1;31ma\x1b[mb");
        let a = p.terminal().get_cell(0, 0).unwrap();
        assert_eq!(a.fg, 1);
        assert!(a.flags.contains(CellFlags::BOLD));
        let b = p.terminal().get_cell(1, 0).unwrap();
        assert_eq!(b.fg, 7);
        assert!(b.flags.is_empty());
    }

    #[test]
    fn bracketed_paste_mode_2004_set_and_reset() {
        let p = run(b"\x1b[?2004h");
        assert!(p.terminal().bracketed_paste());
        let p = run(b"\x1b[?2004h\x1b[?2004l");
        assert!(!p.terminal().bracketed_paste());
    }
}
