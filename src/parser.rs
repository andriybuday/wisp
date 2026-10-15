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
            performer: AnsiPerformer {
                terminal,
                replies: Vec::new(),
            },
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

    /// Drain pending query replies (DA, DSR, size report) to be written
    /// back to the PTY.
    pub fn take_replies(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.performer.replies)
    }
}

struct AnsiPerformer {
    terminal: Terminal,
    replies: Vec<Vec<u8>>,
}

impl AnsiPerformer {
    /// Stateful SGR walk: simple codes go to `Terminal::sgr`, while extended
    /// colors (38/48) consume their operands in either semicolon form
    /// (separate params) or colon form (subparams in one param slice).
    /// Truncated or malformed extended sequences leave the color unchanged.
    fn sgr_dispatch(&mut self, params: &Params) {
        use crate::terminal::Color;
        let slices: Vec<&[u16]> = params.iter().collect();
        let clamp = |v: u16| v.min(255) as u8;
        let mut i = 0;
        while i < slices.len() {
            let slice = slices[i];
            let Some(&code) = slice.first() else {
                i += 1;
                continue;
            };
            if code != 38 && code != 48 {
                self.terminal.sgr(code as usize);
                i += 1;
                continue;
            }
            // Extended color: 38 = foreground, 48 = background.
            let apply = |t: &mut Terminal, color: Color| {
                if code == 38 {
                    t.set_fg_color(color);
                } else {
                    t.set_bg_color(color);
                }
            };
            if slice.len() > 1 {
                // Colon form: mode and operands are subparams of this slice.
                match slice[1] {
                    5 => {
                        if let Some(&n) = slice.get(2) {
                            apply(&mut self.terminal, Color::Indexed(clamp(n)));
                        }
                    }
                    2 => {
                        let rest = &slice[2..];
                        // Skip the optional (often empty) colorspace id.
                        let rgb = if rest.len() > 3 { &rest[1..] } else { rest };
                        if rgb.len() >= 3 {
                            apply(
                                &mut self.terminal,
                                Color::Rgb(clamp(rgb[0]), clamp(rgb[1]), clamp(rgb[2])),
                            );
                        }
                    }
                    _ => {}
                }
                i += 1;
            } else {
                // Semicolon form: mode and operands are following params.
                let next = |k: usize| slices.get(k).and_then(|s| s.first().copied());
                match next(i + 1) {
                    Some(5) => {
                        if let Some(n) = next(i + 2) {
                            apply(&mut self.terminal, Color::Indexed(clamp(n)));
                            i += 3;
                        } else {
                            i += 2; // consume "38;5", color unchanged
                        }
                    }
                    Some(2) => {
                        if let (Some(r), Some(g), Some(b)) = (next(i + 2), next(i + 3), next(i + 4))
                        {
                            apply(&mut self.terminal, Color::Rgb(clamp(r), clamp(g), clamp(b)));
                            i += 5;
                        } else {
                            i = slices.len(); // truncated: consume the rest
                        }
                    }
                    _ => i += 1,
                }
            }
        }
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

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        // OSC 0 / OSC 2: set window title. All other OSC is ignored.
        if (params.first() == Some(&b"0".as_slice()) || params.first() == Some(&b"2".as_slice()))
            && params.len() >= 2
        {
            let title = String::from_utf8_lossy(params[1])
                .chars()
                .filter(|c| !c.is_control())
                .take(256)
                .collect();
            self.terminal.set_title(title);
        }
    }

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
                    self.sgr_dispatch(params);
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
            'c' => {
                // Device Attributes. Primary DA (no intermediate) reports
                // VT102 on purpose: never advertise unimplemented features.
                // Secondary DA (`CSI > c`) reports version 1. Other
                // intermediates (e.g. `=`) are ignored.
                let reply: Option<&[u8]> = if intermediates.is_empty() {
                    Some(b"\x1b[?6c")
                } else if intermediates == [b'>'] {
                    Some(b"\x1b[>0;1;0c")
                } else {
                    None
                };
                if let Some(reply) = reply {
                    self.replies.push(reply.to_vec());
                }
            }
            'n' => {
                // Device Status Report.
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first().copied())
                    .unwrap_or(0);
                match n {
                    5 => self.replies.push(b"\x1b[0n".to_vec()),
                    6 => {
                        let (col, row) = self.terminal.cursor();
                        self.replies
                            .push(format!("\x1b[{};{}R", row + 1, col + 1).into_bytes());
                    }
                    _ => {}
                }
            }
            't' => {
                // Window manipulations. Only the text-area size report
                // (`CSI 18 t`) is implemented; all others are ignored.
                let n = params
                    .iter()
                    .next()
                    .and_then(|p| p.first().copied())
                    .unwrap_or(0);
                if n == 18 {
                    self.replies.push(
                        format!("\x1b[8;{};{}t", self.terminal.rows(), self.terminal.cols())
                            .into_bytes(),
                    );
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
    use crate::terminal::{CellFlags, Color};

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
        assert_eq!(a.fg, Color::Indexed(1));
        assert!(a.flags.contains(CellFlags::BOLD));
        let b = p.terminal().get_cell(1, 0).unwrap();
        assert_eq!(b.fg, Color::Default);
        assert!(b.flags.is_empty());
    }

    #[test]
    fn csi_m_256_color_semicolon() {
        let p = run(b"\x1b[38;5;196ma\x1b[48;5;34mb");
        let a = p.terminal().get_cell(0, 0).unwrap();
        assert_eq!((a.fg, a.bg), (Color::Indexed(196), Color::Default));
        let b = p.terminal().get_cell(1, 0).unwrap();
        assert_eq!((b.fg, b.bg), (Color::Indexed(196), Color::Indexed(34)));
    }

    #[test]
    fn csi_m_256_color_colon() {
        let p = run(b"\x1b[38:5:196ma\x1b[48:5:34mb");
        let a = p.terminal().get_cell(0, 0).unwrap();
        assert_eq!((a.fg, a.bg), (Color::Indexed(196), Color::Default));
        let b = p.terminal().get_cell(1, 0).unwrap();
        assert_eq!((b.fg, b.bg), (Color::Indexed(196), Color::Indexed(34)));
    }

    #[test]
    fn csi_m_truecolor_fg_and_bg() {
        let p = run(b"\x1b[38;2;10;20;30ma\x1b[48;2;200;100;50mb");
        let a = p.terminal().get_cell(0, 0).unwrap();
        assert_eq!((a.fg, a.bg), (Color::Rgb(10, 20, 30), Color::Default));
        let b = p.terminal().get_cell(1, 0).unwrap();
        assert_eq!(
            (b.fg, b.bg),
            (Color::Rgb(10, 20, 30), Color::Rgb(200, 100, 50))
        );
        // Colon form, with and without the empty colorspace field.
        let p = run(b"\x1b[38:2::1:2:3ma\x1b[48:2:4:5:6mb");
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().fg, Color::Rgb(1, 2, 3));
        assert_eq!(p.terminal().get_cell(1, 0).unwrap().bg, Color::Rgb(4, 5, 6));
    }

    #[test]
    fn csi_m_mixed_simple_and_extended() {
        let p = run(b"\x1b[1;38;5;200ma\x1b[39mb");
        let a = p.terminal().get_cell(0, 0).unwrap();
        assert_eq!(a.fg, Color::Indexed(200));
        assert!(a.flags.contains(CellFlags::BOLD));
        assert_eq!(p.terminal().get_cell(1, 0).unwrap().fg, Color::Default);
    }

    #[test]
    fn csi_m_malformed_extended_leaves_color_unchanged() {
        // Truncated 256-color and truecolor sequences: no panic, fg stays red.
        let p = run(b"\x1b[31ma\x1b[38;5mb\x1b[38;2;1mc");
        assert_eq!(p.terminal().get_cell(1, 0).unwrap().fg, Color::Indexed(1));
        assert_eq!(p.terminal().get_cell(2, 0).unwrap().fg, Color::Indexed(1));
        // Truncated colon forms behave the same way.
        let p = run(b"\x1b[31ma\x1b[38:5mb\x1b[38:2:1mc");
        assert_eq!(p.terminal().get_cell(1, 0).unwrap().fg, Color::Indexed(1));
        assert_eq!(p.terminal().get_cell(2, 0).unwrap().fg, Color::Indexed(1));
    }

    #[test]
    fn bracketed_paste_mode_2004_set_and_reset() {
        let p = run(b"\x1b[?2004h");
        assert!(p.terminal().bracketed_paste());
        let p = run(b"\x1b[?2004h\x1b[?2004l");
        assert!(!p.terminal().bracketed_paste());
    }

    #[test]
    fn primary_da_replies_vt102() {
        let mut p = run(b"\x1b[c");
        assert_eq!(p.take_replies(), vec![b"\x1b[?6c".to_vec()]);
        // Drained: a second take without new queries is empty.
        assert!(p.take_replies().is_empty());
    }

    #[test]
    fn secondary_da_replies_version() {
        let mut p = run(b"\x1b[>c");
        assert_eq!(p.take_replies(), vec![b"\x1b[>0;1;0c".to_vec()]);
    }

    #[test]
    fn dsr_5_replies_ready() {
        let mut p = run(b"\x1b[5n");
        assert_eq!(p.take_replies(), vec![b"\x1b[0n".to_vec()]);
    }

    #[test]
    fn dsr_6_reports_1_based_cursor() {
        // CSI H moves to (row 3, col 4) 1-based -> (3, 2) 0-based.
        let mut p = run(b"\x1b[3;4H\x1b[6n");
        assert_eq!(p.take_replies(), vec![b"\x1b[3;4R".to_vec()]);
    }

    #[test]
    fn csi_18t_reports_text_area_size() {
        let mut p = run(b"\x1b[18t");
        // run() uses a 10x4 terminal.
        assert_eq!(p.take_replies(), vec![b"\x1b[8;4;10t".to_vec()]);

        let mut p = AnsiParser::new(Terminal::new(80, 24));
        p.advance(b"\x1b[18t");
        assert_eq!(p.take_replies(), vec![b"\x1b[8;24;80t".to_vec()]);
    }

    #[test]
    fn osc_2_title_terminated_by_bell() {
        let p = run(b"\x1b]2;My Title\x07");
        assert_eq!(p.terminal().title(), Some("My Title"));
    }

    #[test]
    fn osc_2_title_terminated_by_st() {
        let p = run(b"\x1b]2;My Title\x1b\\");
        assert_eq!(p.terminal().title(), Some("My Title"));
    }

    #[test]
    fn osc_title_non_utf8_does_not_panic() {
        let p = run(b"\x1b]2;\xff\xfe title\x07");
        assert!(p.terminal().title().is_some());
        assert!(p.terminal().title().unwrap().contains("title"));
    }
}
