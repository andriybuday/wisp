use crate::terminal::{MouseTracking, Terminal};
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
                        Some(1000) => self
                            .terminal
                            .set_mouse_tracking(MouseTracking::Normal, enabled),
                        Some(1002) => self
                            .terminal
                            .set_mouse_tracking(MouseTracking::ButtonEvent, enabled),
                        Some(1003) => self
                            .terminal
                            .set_mouse_tracking(MouseTracking::AnyEvent, enabled),
                        Some(1004) => self.terminal.set_focus_reporting(enabled),
                        Some(1006) => self.terminal.set_sgr_mouse(enabled),
                        Some(2004) => self.terminal.set_bracketed_paste(enabled),
                        _ => {}
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
        assert_eq!(p.terminal().get_cell(2, 0).unwrap().fg, Color::Indexed(1));
        let p = run(b"ab\x1b[s\x1b[H\x1b[ux");
        assert_eq!(p.terminal().get_cell(2, 0).unwrap().ch, 'x');
    }

    #[test]
    fn oversized_counts_do_not_panic() {
        let p = run(b"\x1b[9999L\x1b[9999M\x1b[9999@\x1b[9999P\x1b[9999S\x1b[9999T\x1b[9999;9999r\x1b[9999G");
        assert_eq!(p.terminal().cursor().0, 9);
    }

    // Cross-feature checks added by the orchestrator after merging 002-005;
    // these exercise interactions no single feature branch could cover.

    #[test]
    fn merged_truecolor_survives_save_restore() {
        // 003 truecolor flowing through 002 save/restore (ESC 7 / ESC 8).
        let p = run(b"\x1b[38;2;10;20;30mab\x1b7\x1b[0m\x1b[H\x1b8x");
        assert_eq!(
            p.terminal().get_cell(2, 0).unwrap().fg,
            Color::Rgb(10, 20, 30)
        );
    }

    #[test]
    fn merged_cpr_reports_cursor_on_alt_screen() {
        // 004 cursor position report while 002 alternate screen is active.
        let mut p = run(b"\x1b[?1049h\x1b[2;3H\x1b[6n");
        assert_eq!(p.take_replies(), vec![b"\x1b[2;3R".to_vec()]);
        // Leaving the alt screen restores the main-screen cursor for CPR.
        let mut p = run(b"ab\x1b[?1049h\x1b[H\x1b[?1049l\x1b[6n");
        assert_eq!(p.take_replies(), vec![b"\x1b[1;3R".to_vec()]);
    }

    #[test]
    fn cjk_utf8_through_parser_lands_in_expected_cells() {
        // UTF-8 bytes for "a" + U+754C + "b": the wide char takes cells
        // 1-2 (head + spacer), so "b" lands in cell 3.
        let p = run("a\u{754C}b".as_bytes());
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().ch, 'a');
        assert_eq!(p.terminal().get_cell(1, 0).unwrap().ch, '\u{754C}');
        let spacer = p.terminal().get_cell(2, 0).unwrap();
        assert_eq!(spacer.ch, ' ');
        assert!(spacer.flags.contains(CellFlags::WIDE_SPACER));
        assert_eq!(p.terminal().get_cell(3, 0).unwrap().ch, 'b');
        assert_eq!(p.terminal().cursor(), (4, 0));
    }

    #[test]
    fn mouse_tracking_modes_set_and_reset() {
        use crate::terminal::MouseTracking;
        // Defaults: everything off.
        let p = run(b"");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::Off);
        assert!(!p.terminal().sgr_mouse());
        assert!(!p.terminal().focus_reporting());
        // Each tracking mode selects its level; resetting it turns tracking off.
        let p = run(b"\x1b[?1000h");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::Normal);
        let p = run(b"\x1b[?1000h\x1b[?1000l");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::Off);
        let p = run(b"\x1b[?1002h");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::ButtonEvent);
        let p = run(b"\x1b[?1002h\x1b[?1002l");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::Off);
        let p = run(b"\x1b[?1003h");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::AnyEvent);
        let p = run(b"\x1b[?1003h\x1b[?1003l");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::Off);
    }

    #[test]
    fn mouse_tracking_precedence_higher_wins_reset_drops_to_off() {
        use crate::terminal::MouseTracking;
        // Setting a higher mode replaces the lower one.
        let p = run(b"\x1b[?1000h\x1b[?1002h");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::ButtonEvent);
        let p = run(b"\x1b[?1002h\x1b[?1003h");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::AnyEvent);
        // Setting a lower mode does not downgrade a higher one.
        let p = run(b"\x1b[?1003h\x1b[?1000h");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::AnyEvent);
        let p = run(b"\x1b[?1002h\x1b[?1000h");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::ButtonEvent);
        // Resetting the active mode drops to Off (no fall-back to the
        // lower mode that was set earlier).
        let p = run(b"\x1b[?1000h\x1b[?1002h\x1b[?1002l");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::Off);
        // Resetting a mode that is not active is a no-op.
        let p = run(b"\x1b[?1003h\x1b[?1000l");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::AnyEvent);
        let p = run(b"\x1b[?1000h\x1b[?1003l");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::Normal);
        // Combined params in one sequence apply left to right.
        let p = run(b"\x1b[?1000;1006h");
        assert_eq!(p.terminal().mouse_tracking(), MouseTracking::Normal);
        assert!(p.terminal().sgr_mouse());
    }

    #[test]
    fn sgr_mouse_1006_and_focus_reporting_1004_set_and_reset() {
        let p = run(b"\x1b[?1006h");
        assert!(p.terminal().sgr_mouse());
        let p = run(b"\x1b[?1006h\x1b[?1006l");
        assert!(!p.terminal().sgr_mouse());
        let p = run(b"\x1b[?1004h");
        assert!(p.terminal().focus_reporting());
        let p = run(b"\x1b[?1004h\x1b[?1004l");
        assert!(!p.terminal().focus_reporting());
        // The flags are independent of the tracking level.
        let p = run(b"\x1b[?1006h\x1b[?1004h\x1b[?1000h\x1b[?1000l");
        assert!(p.terminal().sgr_mouse());
        assert!(p.terminal().focus_reporting());
        assert_eq!(
            p.terminal().mouse_tracking(),
            crate::terminal::MouseTracking::Off
        );
    }

    #[test]
    fn merged_256color_inside_scroll_region() {
        // 003 indexed-256 color printed inside a 002 scroll region.
        let p = run(b"\x1b[2;3r\x1b[38;5;196mX");
        // DECSTBM homes the cursor to (0,0); X lands there in color 196.
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().ch, 'X');
        assert_eq!(p.terminal().get_cell(0, 0).unwrap().fg, Color::Indexed(196));
    }
}
