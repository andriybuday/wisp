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

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, c: char) {
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
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}
