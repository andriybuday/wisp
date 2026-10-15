/// A terminal color: the palette default, a 256-color palette index,
/// or a 24-bit truecolor value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

/// Represents a single cell in the terminal grid
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub flags: CellFlags,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Default,
            bg: Color::Default,
            flags: CellFlags::empty(),
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct CellFlags: u8 {
        const BOLD = 0b0001;
        const ITALIC = 0b0010;
        const UNDERLINE = 0b0100;
        const INVERSE = 0b1000;
    }
}

/// Terminal state
pub struct Terminal {
    /// Character grid
    grid: Vec<Cell>,
    /// Number of columns
    cols: usize,
    /// Number of rows
    rows: usize,
    /// Cursor position (col, row)
    cursor: (usize, usize),
    /// Scrollback buffer
    scrollback: Vec<Vec<Cell>>,
    scrollback_limit: usize,
    /// Current foreground color
    current_fg: Color,
    /// Current background color
    current_bg: Color,
    /// Current text flags
    current_flags: CellFlags,
    /// Cursor visible
    cursor_visible: bool,
    /// Bracketed paste mode (DEC private mode 2004) is enabled
    bracketed_paste: bool,
    /// Window title set via OSC 0 / OSC 2
    title: Option<String>,
    /// Text selection range: (start_col, start_row, end_col, end_row)
    selection: Option<(usize, usize, usize, usize)>,
    /// Scrollback offset (0 = at bottom, >0 = scrolled up)
    scroll_offset: usize,
}

impl Terminal {
    pub fn new(cols: usize, rows: usize) -> Self {
        let grid = vec![Cell::default(); cols * rows];

        Self {
            grid,
            cols,
            rows,
            cursor: (0, 0),
            scrollback: Vec::new(),
            scrollback_limit: 1000,
            current_fg: Color::Default,
            current_bg: Color::Default,
            current_flags: CellFlags::empty(),
            cursor_visible: true,
            bracketed_paste: false,
            title: None,
            selection: None,
            scroll_offset: 0,
        }
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn bracketed_paste(&self) -> bool {
        self.bracketed_paste
    }

    pub fn set_bracketed_paste(&mut self, enabled: bool) {
        self.bracketed_paste = enabled;
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cursor(&self) -> (usize, usize) {
        self.cursor
    }

    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    // Kept for upcoming Spec Kit features.
    #[allow(dead_code)]
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        let mut new_grid = vec![Cell::default(); new_cols * new_rows];

        // Copy existing content
        let copy_cols = self.cols.min(new_cols);
        let copy_rows = self.rows.min(new_rows);

        for row in 0..copy_rows {
            for col in 0..copy_cols {
                new_grid[row * new_cols + col] = self.grid[row * self.cols + col];
            }
        }

        self.grid = new_grid;
        self.cols = new_cols;
        self.rows = new_rows;

        // Adjust cursor position
        self.cursor.0 = self.cursor.0.min(new_cols.saturating_sub(1));
        self.cursor.1 = self.cursor.1.min(new_rows.saturating_sub(1));
    }

    pub fn get_cell(&self, col: usize, row: usize) -> Option<&Cell> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        Some(&self.grid[row * self.cols + col])
    }

    // Kept for upcoming Spec Kit features.
    #[allow(dead_code)]
    pub fn set_cell(&mut self, col: usize, row: usize, cell: Cell) {
        if col < self.cols && row < self.rows {
            self.grid[row * self.cols + col] = cell;
        }
    }

    // ANSI control functions

    pub fn print(&mut self, ch: char) {
        // Reset scroll offset when new content arrives
        self.scroll_offset = 0;

        let (col, row) = self.cursor;
        if col < self.cols && row < self.rows {
            self.grid[row * self.cols + col] = Cell {
                ch,
                fg: self.current_fg,
                bg: self.current_bg,
                flags: self.current_flags,
            };
            self.cursor.0 += 1;
            if self.cursor.0 >= self.cols {
                self.linefeed();
            }
        }
    }

    pub fn linefeed(&mut self) {
        self.cursor.1 += 1;
        if self.cursor.1 >= self.rows {
            self.scroll_up();
            self.cursor.1 = self.rows - 1;
        }
        self.cursor.0 = 0;
    }

    pub fn carriage_return(&mut self) {
        self.cursor.0 = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
        }
    }

    pub fn tab(&mut self) {
        self.cursor.0 = ((self.cursor.0 / 8) + 1) * 8;
        if self.cursor.0 >= self.cols {
            self.linefeed();
        }
    }

    pub fn cursor_up(&mut self, n: usize) {
        self.cursor.1 = self.cursor.1.saturating_sub(n);
    }

    pub fn cursor_down(&mut self, n: usize) {
        self.cursor.1 = (self.cursor.1 + n).min(self.rows - 1);
    }

    pub fn cursor_forward(&mut self, n: usize) {
        self.cursor.0 = (self.cursor.0 + n).min(self.cols - 1);
    }

    pub fn cursor_backward(&mut self, n: usize) {
        self.cursor.0 = self.cursor.0.saturating_sub(n);
    }

    pub fn cursor_goto(&mut self, col: usize, row: usize) {
        self.cursor.0 = col.min(self.cols - 1);
        self.cursor.1 = row.min(self.rows - 1);
    }

    pub fn clear_all(&mut self) {
        for cell in &mut self.grid {
            *cell = Cell::default();
        }
    }

    pub fn clear_below(&mut self) {
        let (col, row) = self.cursor;
        for c in col..self.cols {
            self.grid[row * self.cols + c] = Cell::default();
        }
        for r in (row + 1)..self.rows {
            for c in 0..self.cols {
                self.grid[r * self.cols + c] = Cell::default();
            }
        }
    }

    pub fn clear_above(&mut self) {
        let (col, row) = self.cursor;
        for r in 0..row {
            for c in 0..self.cols {
                self.grid[r * self.cols + c] = Cell::default();
            }
        }
        for c in 0..=col {
            self.grid[row * self.cols + c] = Cell::default();
        }
    }

    pub fn clear_line(&mut self) {
        let row = self.cursor.1;
        for c in 0..self.cols {
            self.grid[row * self.cols + c] = Cell::default();
        }
    }

    pub fn clear_line_right(&mut self) {
        let (col, row) = self.cursor;
        for c in col..self.cols {
            self.grid[row * self.cols + c] = Cell::default();
        }
    }

    pub fn clear_line_left(&mut self) {
        let (col, row) = self.cursor;
        for c in 0..=col {
            self.grid[row * self.cols + c] = Cell::default();
        }
    }

    pub fn reset_sgr(&mut self) {
        // Reset to default colors and flags
        self.current_fg = Color::Default;
        self.current_bg = Color::Default;
        self.current_flags = CellFlags::empty();
    }

    pub fn set_fg_color(&mut self, color: Color) {
        self.current_fg = color;
    }

    pub fn set_bg_color(&mut self, color: Color) {
        self.current_bg = color;
    }

    pub fn sgr(&mut self, param: usize) {
        // Handle SGR parameters (colors, bold, etc.)
        match param {
            0 => self.reset_sgr(),                                           // Reset all
            1 => self.current_flags.insert(CellFlags::BOLD),                 // Bold
            3 => self.current_flags.insert(CellFlags::ITALIC),               // Italic
            4 => self.current_flags.insert(CellFlags::UNDERLINE),            // Underline
            7 => self.current_flags.insert(CellFlags::INVERSE),              // Inverse
            22 => self.current_flags.remove(CellFlags::BOLD),                // Normal intensity
            23 => self.current_flags.remove(CellFlags::ITALIC),              // Not italic
            24 => self.current_flags.remove(CellFlags::UNDERLINE),           // Not underline
            27 => self.current_flags.remove(CellFlags::INVERSE),             // Not inverse
            30..=37 => self.current_fg = Color::Indexed((param - 30) as u8), // FG 0-7
            40..=47 => self.current_bg = Color::Indexed((param - 40) as u8), // BG 0-7
            90..=97 => self.current_fg = Color::Indexed((param - 90 + 8) as u8), // FG 8-15
            100..=107 => self.current_bg = Color::Indexed((param - 100 + 8) as u8), // BG 8-15
            39 => self.current_fg = Color::Default,                          // Default FG
            49 => self.current_bg = Color::Default,                          // Default BG
            _ => {}
        }
    }

    fn scroll_up(&mut self) {
        // Save first line to scrollback
        let first_line: Vec<Cell> = self.grid[0..self.cols].to_vec();
        self.scrollback.push(first_line);
        if self.scrollback.len() > self.scrollback_limit {
            self.scrollback.remove(0);
        }

        // Shift all lines up
        for row in 1..self.rows {
            for col in 0..self.cols {
                self.grid[(row - 1) * self.cols + col] = self.grid[row * self.cols + col];
            }
        }

        // Clear last line
        for col in 0..self.cols {
            self.grid[(self.rows - 1) * self.cols + col] = Cell::default();
        }
    }

    // Selection methods

    pub fn set_selection_start(&mut self, col: usize, row: usize) {
        self.selection = Some((col, row, col, row));
    }

    pub fn set_selection_end(&mut self, col: usize, row: usize) {
        if let Some((start_col, start_row, _, _)) = self.selection {
            self.selection = Some((start_col, start_row, col, row));
        }
    }

    // Kept for upcoming Spec Kit features.
    #[allow(dead_code)]
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    // Kept for upcoming Spec Kit features.
    #[allow(dead_code)]
    pub fn selection(&self) -> Option<(usize, usize, usize, usize)> {
        self.selection
    }

    pub fn is_cell_selected(&self, col: usize, row: usize) -> bool {
        if let Some((start_col, start_row, end_col, end_row)) = self.selection {
            // Normalize the selection (start before end)
            let (start_col, start_row, end_col, end_row) =
                if start_row < end_row || (start_row == end_row && start_col <= end_col) {
                    (start_col, start_row, end_col, end_row)
                } else {
                    (end_col, end_row, start_col, start_row)
                };

            let cell_pos = row * self.cols + col;
            let start_pos = start_row * self.cols + start_col;
            let end_pos = end_row * self.cols + end_col;

            cell_pos >= start_pos && cell_pos <= end_pos
        } else {
            false
        }
    }

    // Scrollback methods

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    // Kept for upcoming Spec Kit features.
    #[allow(dead_code)]
    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    pub fn scroll_viewport_up(&mut self, lines: usize) {
        let max_scroll = self.scrollback.len();
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
    }

    pub fn scroll_viewport_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    // Kept for upcoming Spec Kit features.
    #[allow(dead_code)]
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Get a cell from the visible viewport (accounting for scroll offset)
    pub fn get_viewport_cell(&self, col: usize, row: usize) -> Option<&Cell> {
        if col >= self.cols {
            return None;
        }

        if self.scroll_offset == 0 {
            // Normal view - get from main grid
            self.get_cell(col, row)
        } else {
            // Scrolled view - get from scrollback or grid
            let total_rows = self.scrollback.len() + self.rows;
            let viewport_start = total_rows.saturating_sub(self.rows + self.scroll_offset);
            let absolute_row = viewport_start + row;

            if absolute_row < self.scrollback.len() {
                // Get from scrollback
                self.scrollback
                    .get(absolute_row)
                    .and_then(|line| line.get(col))
            } else {
                // Get from main grid
                let grid_row = absolute_row - self.scrollback.len();
                self.get_cell(col, grid_row)
            }
        }
    }

    pub fn get_selected_text(&self) -> String {
        if let Some((start_col, start_row, end_col, end_row)) = self.selection {
            // Normalize the selection (start before end)
            let (start_col, start_row, end_col, end_row) =
                if start_row < end_row || (start_row == end_row && start_col <= end_col) {
                    (start_col, start_row, end_col, end_row)
                } else {
                    (end_col, end_row, start_col, start_row)
                };

            let mut text = String::new();

            for row in start_row..=end_row {
                if row >= self.rows {
                    break;
                }

                let line_start = if row == start_row { start_col } else { 0 };
                let line_end = if row == end_row {
                    end_col
                } else {
                    self.cols - 1
                };

                for col in line_start..=line_end {
                    if col >= self.cols {
                        break;
                    }
                    if let Some(cell) = self.get_cell(col, row) {
                        text.push(cell.ch);
                    }
                }

                // Add newline if not on the last row
                if row < end_row {
                    text.push('\n');
                }
            }

            text
        } else {
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(cols: usize, rows: usize) -> Terminal {
        Terminal::new(cols, rows)
    }

    fn print_str(t: &mut Terminal, s: &str) {
        for ch in s.chars() {
            t.print(ch);
        }
    }

    #[test]
    fn print_writes_char_and_advances_cursor() {
        let mut t = term(10, 3);
        t.print('a');
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
        assert_eq!(t.cursor(), (1, 0));
    }

    #[test]
    fn print_uses_default_colors() {
        let mut t = term(10, 3);
        t.print('x');
        let cell = t.get_cell(0, 0).unwrap();
        assert_eq!((cell.fg, cell.bg), (Color::Default, Color::Default));
        assert!(cell.flags.is_empty());
    }

    #[test]
    fn print_wraps_at_last_column() {
        let mut t = term(3, 3);
        print_str(&mut t, "abcd");
        assert_eq!(t.get_cell(2, 0).unwrap().ch, 'c');
        assert_eq!(t.get_cell(0, 1).unwrap().ch, 'd');
        assert_eq!(t.cursor(), (1, 1));
    }

    #[test]
    fn linefeed_at_bottom_scrolls_up() {
        let mut t = term(5, 2);
        print_str(&mut t, "ab");
        t.linefeed();
        print_str(&mut t, "cd");
        t.linefeed();
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'c');
        assert_eq!(t.get_cell(0, 1).unwrap().ch, ' ');
        assert_eq!(t.cursor(), (0, 1));
        assert_eq!(t.scrollback.len(), 1);
        assert_eq!(t.scrollback[0][0].ch, 'a');
    }

    #[test]
    fn scrollback_respects_limit() {
        let mut t = term(2, 2);
        t.scrollback_limit = 2;
        for ch in ['a', 'b', 'c', 'd', 'e'] {
            t.print(ch);
            t.linefeed();
        }
        assert_eq!(t.scrollback.len(), 2);
    }

    #[test]
    fn cursor_moves_clamp_to_grid() {
        let mut t = term(5, 3);
        t.cursor_down(10);
        t.cursor_forward(10);
        assert_eq!(t.cursor(), (4, 2));
        t.cursor_up(1);
        t.cursor_backward(2);
        assert_eq!(t.cursor(), (2, 1));
        t.cursor_up(99);
        t.cursor_backward(99);
        assert_eq!(t.cursor(), (0, 0));
    }

    #[test]
    fn cursor_goto_clamps() {
        let mut t = term(5, 3);
        t.cursor_goto(3, 2);
        assert_eq!(t.cursor(), (3, 2));
        t.cursor_goto(99, 99);
        assert_eq!(t.cursor(), (4, 2));
    }

    #[test]
    fn carriage_return_and_backspace() {
        let mut t = term(10, 2);
        print_str(&mut t, "abc");
        t.carriage_return();
        assert_eq!(t.cursor(), (0, 0));
        t.backspace();
        assert_eq!(t.cursor(), (0, 0));
        t.cursor_forward(2);
        t.backspace();
        assert_eq!(t.cursor(), (1, 0));
    }

    #[test]
    fn tab_advances_to_next_multiple_of_8() {
        let mut t = term(20, 2);
        t.tab();
        assert_eq!(t.cursor(), (8, 0));
        t.print('x');
        t.tab();
        assert_eq!(t.cursor(), (16, 0));
    }

    #[test]
    fn tab_beyond_last_column_wraps() {
        let mut t = term(10, 3);
        t.cursor_forward(9);
        t.tab();
        assert_eq!(t.cursor(), (0, 1));
    }

    #[test]
    fn clear_all_keeps_cursor() {
        let mut t = term(5, 2);
        print_str(&mut t, "hi");
        t.clear_all();
        assert_eq!(t.get_cell(0, 0).unwrap().ch, ' ');
        assert_eq!(t.cursor(), (2, 0));
    }

    #[test]
    fn clear_below_and_above() {
        let mut t = term(4, 3);
        print_str(&mut t, "abcd");
        print_str(&mut t, "efgh");
        t.cursor_goto(1, 0);
        t.clear_below();
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
        assert_eq!(t.get_cell(1, 0).unwrap().ch, ' ');
        assert_eq!(t.get_cell(0, 1).unwrap().ch, ' ');
        let mut t = term(4, 3);
        print_str(&mut t, "abcd");
        print_str(&mut t, "efgh");
        t.cursor_goto(1, 1);
        t.clear_above();
        assert_eq!(t.get_cell(0, 0).unwrap().ch, ' ');
        assert_eq!(t.get_cell(1, 1).unwrap().ch, ' ');
        assert_eq!(t.get_cell(2, 1).unwrap().ch, 'g');
    }

    #[test]
    fn clear_line_variants() {
        let mut t = term(4, 2);
        print_str(&mut t, "abcd");
        t.cursor_goto(2, 0);
        t.clear_line_right();
        assert_eq!(t.get_cell(1, 0).unwrap().ch, 'b');
        assert_eq!(t.get_cell(2, 0).unwrap().ch, ' ');
        t.clear_line_left();
        assert_eq!(t.get_cell(0, 0).unwrap().ch, ' ');
        let mut t = term(4, 2);
        print_str(&mut t, "abcd");
        t.cursor_goto(1, 0);
        t.clear_line();
        assert_eq!(t.get_cell(0, 0).unwrap().ch, ' ');
        assert_eq!(t.get_cell(3, 0).unwrap().ch, ' ');
    }

    #[test]
    fn sgr_normal_and_bright_colors() {
        let mut t = term(10, 2);
        t.sgr(31);
        t.sgr(42);
        t.print('a');
        assert_eq!(
            (t.get_cell(0, 0).unwrap().fg, t.get_cell(0, 0).unwrap().bg),
            (Color::Indexed(1), Color::Indexed(2))
        );
        t.sgr(91);
        t.sgr(103);
        t.print('b');
        assert_eq!(
            (t.get_cell(1, 0).unwrap().fg, t.get_cell(1, 0).unwrap().bg),
            (Color::Indexed(9), Color::Indexed(11))
        );
    }

    #[test]
    fn sgr_bold_underline_inverse_and_removals() {
        let mut t = term(10, 2);
        t.sgr(1);
        t.sgr(4);
        t.sgr(7);
        t.print('a');
        let f = t.get_cell(0, 0).unwrap().flags;
        assert!(f.contains(CellFlags::BOLD | CellFlags::UNDERLINE | CellFlags::INVERSE));
        t.sgr(22);
        t.sgr(24);
        t.sgr(27);
        t.print('b');
        assert!(t.get_cell(1, 0).unwrap().flags.is_empty());
    }

    #[test]
    fn sgr_reset_and_defaults() {
        let mut t = term(10, 2);
        t.sgr(31);
        t.sgr(1);
        t.sgr(0);
        t.print('a');
        let cell = t.get_cell(0, 0).unwrap();
        assert_eq!((cell.fg, cell.bg), (Color::Default, Color::Default));
        assert!(cell.flags.is_empty());
        t.sgr(34);
        t.sgr(45);
        t.sgr(39);
        t.sgr(49);
        t.print('b');
        assert_eq!(
            (t.get_cell(1, 0).unwrap().fg, t.get_cell(1, 0).unwrap().bg),
            (Color::Default, Color::Default)
        );
    }

    #[test]
    fn extended_colors_via_setters_and_reset() {
        let mut t = term(10, 2);
        t.set_fg_color(Color::Indexed(196));
        t.set_bg_color(Color::Rgb(10, 20, 30));
        t.print('a');
        let cell = t.get_cell(0, 0).unwrap();
        assert_eq!(cell.fg, Color::Indexed(196));
        assert_eq!(cell.bg, Color::Rgb(10, 20, 30));
        t.sgr(0);
        t.print('b');
        let cell = t.get_cell(1, 0).unwrap();
        assert_eq!((cell.fg, cell.bg), (Color::Default, Color::Default));
    }

    #[test]
    fn selection_text_single_and_multi_row() {
        let mut t = term(4, 3);
        print_str(&mut t, "abcd");
        print_str(&mut t, "efgh");
        t.set_selection_start(1, 0);
        t.set_selection_end(2, 0);
        assert_eq!(t.get_selected_text(), "bc");
        t.set_selection_start(3, 1);
        t.set_selection_end(2, 0);
        assert_eq!(t.get_selected_text(), "cd\nefgh");
    }

    #[test]
    fn empty_selection_text_is_empty() {
        let t = term(4, 2);
        assert_eq!(t.get_selected_text(), "");
        assert!(!t.is_cell_selected(0, 0));
    }
}
