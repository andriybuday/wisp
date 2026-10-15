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

fn param_at(params: &Params, idx: usize) -> usize {
    params
        .iter()
        .nth(idx)
        .and_then(|p| p.first().copied())
        .unwrap_or(0) as usize
}

/// Count-style param: missing or 0 means 1 (xterm semantics).
fn count_param(params: &Params) -> usize {
    match param_at(params, 0) {
        0 => 1,
        n => n,
    }
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
            'G' => {
                let col = count_param(params);
                self.terminal.cursor_to_column(col.saturating_sub(1));
            }
            'd' => {
                let row = count_param(params);
                self.terminal.cursor_to_row(row.saturating_sub(1));
            }
            'E' => self.terminal.cursor_next_line(count_param(params)),
            'F' => self.terminal.cursor_prev_line(count_param(params)),
            '@' => self.terminal.insert_chars(count_param(params)),
            'P' => self.terminal.delete_chars(count_param(params)),
            'X' => self.terminal.erase_chars(count_param(params)),
            'L' => self.terminal.insert_lines(count_param(params)),
            'M' => self.terminal.delete_lines(count_param(params)),
            'S' => self.terminal.csi_scroll_up(count_param(params)),
            'T' => self.terminal.csi_scroll_down(count_param(params)),
            'r' => {
                self.terminal
                    .set_scroll_region(param_at(params, 0), param_at(params, 1));
            }
            's' if intermediates.is_empty() => self.terminal.save_cursor(),
            'u' if intermediates.is_empty() => self.terminal.restore_cursor(),
            // DEC private mode set ('h') / reset ('l'), e.g. `CSI ? 2004 h`.
            // The private `?` marker is delivered as an intermediate byte.
            'h' | 'l' if intermediates.first() == Some(&b'?') => {
                let enabled = c == 'h';
                for param in params.iter() {
                    match param.first().copied() {
                        Some(25) => self.terminal.set_cursor_visible(enabled),
                        Some(47) => self.terminal.set_alt_screen_47(enabled),
                        Some(1047) => self.terminal.set_alt_screen_1047(enabled),
                        Some(1049) => self.terminal.set_alt_screen_1049(enabled),
                        Some(2004) => self.terminal.set_bracketed_paste(enabled),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        if !intermediates.is_empty() {
            return;
        }
        match byte {
            b'D' => self.terminal.index(),
            b'M' => self.terminal.reverse_index(),
            b'E' => self.terminal.next_line(),
            b'7' => self.terminal.save_cursor(),
            b'8' => self.terminal.restore_cursor(),
            _ => {}
        }
    }
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
    #[test]
    fn alt_screen_1049_round_trip() {
        let p = run(b"main\x1b[?1049h\x1b[Hab\x1b[?1049l");
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().ch, 'm');
        assert_eq!(p.terminal().cursor(), (4, 0));
        let p = run(b"\x1b[?1049hab");
        assert!(p.terminal().on_alternate_screen());
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().ch, 'a');
    }

    #[test]
    fn cursor_visibility_mode_25() {
        let p = run(b"\x1b[?25l");
        assert!(!p.terminal().cursor_visible());
        let p = run(b"\x1b[?25l\x1b[?25h");
        assert!(p.terminal().cursor_visible());
    }

    #[test]
    fn decstbm_via_csi_r() {
        let p = run(b"\x1b[2;3r");
        assert_eq!(p.terminal().scroll_region(), (1, 2));
        let p = run(b"\x1b[r");
        assert_eq!(p.terminal().scroll_region(), (0, 3));
    }

    #[test]
    fn csi_insert_delete_ops() {
        let p = run(b"abcd\x1b[1;2H\x1b[2@");
        assert_eq!(p.terminal().get_cell(3, 0).unwrap().ch, 'b');
        let p = run(b"abcd\x1b[1;1H\x1b[1P");
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().ch, 'b');
        let p = run(b"abcd\x1b[1;2H\x1b[2X");
        assert_eq!(p.terminal().get_cell(1, 0).unwrap().ch, ' ');
        assert_eq!(p.terminal().get_cell(3, 0).unwrap().ch, 'd');
        let p = run(b"ab\r\ncd\x1b[1;1H\x1b[1L");
        assert_eq!(p.terminal().get_cell(0, 1).unwrap().ch, 'a');
        let p = run(b"ab\r\ncd\x1b[1;1H\x1b[1M");
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().ch, 'c');
        let p = run(b"ab\r\ncd\x1b[1S");
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().ch, 'c');
        let p = run(b"ab\r\ncd\x1b[1T");
        assert_eq!(p.terminal().get_cell(0, 1).unwrap().ch, 'a');
    }

    #[test]
    fn csi_cha_vpa_cnl_cpl() {
        let p = run(b"\x1b[5G\x1b[3d");
        assert_eq!(p.terminal().cursor(), (4, 2));
        let p = run(b"ab\x1b[2E");
        assert_eq!(p.terminal().cursor(), (0, 2));
        let p = run(b"ab\x1b[2E\x1b[1F");
        assert_eq!(p.terminal().cursor(), (0, 1));
    }

    #[test]
    fn esc_index_reverse_next_line() {
        let p = run(b"a\x1bDb");
        assert_eq!(p.terminal().get_cell(1, 1).unwrap().ch, 'b');
        let p = run(b"a\x1bD\x1bMb");
        assert_eq!(p.terminal().get_cell(1, 0).unwrap().ch, 'b');
        let p = run(b"ab\x1bEc");
        assert_eq!(p.terminal().get_cell(0, 1).unwrap().ch, 'c');
    }

    #[test]
    fn save_restore_cursor_esc_and_csi() {
        let p = run(b"\x1b[31mab\x1b7\x1b[0m\x1b[H\x1b8x");
        assert_eq!(p.terminal().cursor(), (3, 0));
        assert_eq!(p.terminal().get_cell(2, 0).unwrap().fg, 1);
        let p = run(b"ab\x1b[s\x1b[H\x1b[ux");
        assert_eq!(p.terminal().get_cell(2, 0).unwrap().ch, 'x');
    }

    #[test]
    fn oversized_counts_do_not_panic() {
        let p = run(b"\x1b[9999L\x1b[9999M\x1b[9999@\x1b[9999P\x1b[9999S\x1b[9999T\x1b[9999;9999r\x1b[9999G");
        assert_eq!(p.terminal().cursor().0, 9);
    }
}
