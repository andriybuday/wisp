/// Represents a single cell in the terminal grid
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub fg: u8, // Foreground color index (0-15)
    pub bg: u8, // Background color index (0-15)
    pub flags: CellFlags,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: 7, // Default white
            bg: 0, // Default black
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
    current_fg: u8,
    /// Current background color
    current_bg: u8,
    /// Current text flags
    current_flags: CellFlags,
    /// Cursor visible
    cursor_visible: bool,
    /// Bracketed paste mode (DEC private mode 2004) is enabled
    bracketed_paste: bool,
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
            current_fg: 7, // Default white
            current_bg: 0, // Default black
            current_flags: CellFlags::empty(),
            cursor_visible: true,
            bracketed_paste: false,
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

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cursor(&self) -> (usize, usize) {
        self.cursor
    }

    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

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
        self.current_fg = 7; // Default white
        self.current_bg = 0; // Default black
        self.current_flags = CellFlags::empty();
    }

    pub fn sgr(&mut self, param: usize) {
        // Handle SGR parameters (colors, bold, etc.)
        match param {
            0 => self.reset_sgr(),                                  // Reset all
            1 => self.current_flags.insert(CellFlags::BOLD),        // Bold
            3 => self.current_flags.insert(CellFlags::ITALIC),      // Italic
            4 => self.current_flags.insert(CellFlags::UNDERLINE),   // Underline
            7 => self.current_flags.insert(CellFlags::INVERSE),     // Inverse
            22 => self.current_flags.remove(CellFlags::BOLD),       // Normal intensity
            23 => self.current_flags.remove(CellFlags::ITALIC),     // Not italic
            24 => self.current_flags.remove(CellFlags::UNDERLINE),  // Not underline
            27 => self.current_flags.remove(CellFlags::INVERSE),    // Not inverse
            30..=37 => self.current_fg = (param - 30) as u8,        // Foreground colors 0-7
            40..=47 => self.current_bg = (param - 40) as u8,        // Background colors 0-7
            90..=97 => self.current_fg = (param - 90 + 8) as u8,    // Bright foreground 8-15
            100..=107 => self.current_bg = (param - 100 + 8) as u8, // Bright background 8-15
            39 => self.current_fg = 7,                              // Default foreground
            49 => self.current_bg = 0,                              // Default background
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

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn selection(&self) -> Option<(usize, usize, usize, usize)> {
        self.selection
    }

    pub fn is_cell_selected(&self, col: usize, row: usize) -> bool {
        if let Some((start_col, start_row, end_col, end_row)) = self.selection {
            // Normalize the selection (start before end)
            let (start_col, start_row, end_col, end_row) = if start_row < end_row
                || (start_row == end_row && start_col <= end_col)
            {
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
                self.scrollback.get(absolute_row).and_then(|line| line.get(col))
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
            let (start_col, start_row, end_col, end_row) = if start_row < end_row
                || (start_row == end_row && start_col <= end_col)
            {
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
