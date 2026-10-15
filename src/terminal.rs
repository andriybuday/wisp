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
    /// Alternate screen grid (full-screen apps)
    alt_grid: Vec<Cell>,
    /// Whether the alternate screen is active
    on_alt: bool,
    /// Scroll region top row (0-based, inclusive)
    region_top: usize,
    /// Scroll region bottom row (0-based, inclusive)
    region_bottom: usize,
    /// Saved cursor: (col, row, fg, bg, flags) for ESC 7 / CSI s
    saved_cursor: (usize, usize, Color, Color, CellFlags),
    /// Cursor saved on entering the alternate screen via ?1049
    alt_entry_cursor: (usize, usize),
}

impl Terminal {
    pub fn new(cols: usize, rows: usize) -> Self {
        let grid = vec![Cell::default(); cols * rows];
        let alt_grid = vec![Cell::default(); cols * rows];

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
            alt_grid,
            on_alt: false,
            region_top: 0,
            region_bottom: rows.saturating_sub(1),
            saved_cursor: (0, 0, Color::Default, Color::Default, CellFlags::empty()),
            alt_entry_cursor: (0, 0),
        }
    }

    fn active_grid(&self) -> &Vec<Cell> {
        if self.on_alt {
            &self.alt_grid
        } else {
            &self.grid
        }
    }

    fn active_grid_mut(&mut self) -> &mut Vec<Cell> {
        if self.on_alt {
            &mut self.alt_grid
        } else {
            &mut self.grid
        }
    }

    // Used by tests; renderer adoption comes later.
    #[allow(dead_code)]
    pub fn on_alternate_screen(&self) -> bool {
        self.on_alt
    }

    /// DECSET/DECRST ?1049: enter saves cursor, switches and clears alt;
    /// exit switches back and restores cursor.
    pub fn set_alt_screen_1049(&mut self, enabled: bool) {
        if enabled {
            if !self.on_alt {
                self.alt_entry_cursor = self.cursor;
                self.on_alt = true;
                for cell in &mut self.alt_grid {
                    *cell = Cell::default();
                }
            }
        } else if self.on_alt {
            self.on_alt = false;
            self.cursor = (
                self.alt_entry_cursor.0.min(self.cols.saturating_sub(1)),
                self.alt_entry_cursor.1.min(self.rows.saturating_sub(1)),
            );
        }
    }

    /// DECSET/DECRST ?47: switch buffers, no cursor save, no clear.
    pub fn set_alt_screen_47(&mut self, enabled: bool) {
        self.on_alt = enabled;
    }

    /// DECSET/DECRST ?1047: switch buffers, no cursor save; entering does
    /// not clear, leaving clears the alternate grid.
    pub fn set_alt_screen_1047(&mut self, enabled: bool) {
        if enabled {
            self.on_alt = true;
        } else {
            for cell in &mut self.alt_grid {
                *cell = Cell::default();
            }
            self.on_alt = false;
        }
    }

    /// DECSTBM scroll region (0-based, inclusive).
    #[allow(dead_code)]
    pub fn scroll_region(&self) -> (usize, usize) {
        (self.region_top, self.region_bottom)
    }

    /// Set scroll region from 1-based CSI params (0 = default).
    /// Invalid params select the full screen. Cursor moves to 0,0.
    pub fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        let t = if top == 0 { 1 } else { top };
        let b = if bottom == 0 { self.rows } else { bottom };
        if t < 1 || b > self.rows || t >= b {
            self.region_top = 0;
            self.region_bottom = self.rows.saturating_sub(1);
        } else {
            self.region_top = t - 1;
            self.region_bottom = b - 1;
        }
        self.cursor = (0, 0);
    }

    pub fn save_cursor(&mut self) {
        self.saved_cursor = (
            self.cursor.0,
            self.cursor.1,
            self.current_fg,
            self.current_bg,
            self.current_flags,
        );
    }

    pub fn restore_cursor(&mut self) {
        let (col, row, fg, bg, flags) = self.saved_cursor;
        self.cursor = (
            col.min(self.cols.saturating_sub(1)),
            row.min(self.rows.saturating_sub(1)),
        );
        self.current_fg = fg;
        self.current_bg = bg;
        self.current_flags = flags;
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

    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        fn resized(
            old: &[Cell],
            old_cols: usize,
            old_rows: usize,
            new_cols: usize,
            new_rows: usize,
        ) -> Vec<Cell> {
            let mut new_grid = vec![Cell::default(); new_cols * new_rows];
            let copy_cols = old_cols.min(new_cols);
            let copy_rows = old_rows.min(new_rows);
            for row in 0..copy_rows {
                for col in 0..copy_cols {
                    new_grid[row * new_cols + col] = old[row * old_cols + col];
                }
            }
            new_grid
        }

        self.grid = resized(&self.grid, self.cols, self.rows, new_cols, new_rows);
        self.alt_grid = resized(&self.alt_grid, self.cols, self.rows, new_cols, new_rows);
        self.cols = new_cols;
        self.rows = new_rows;

        // Adjust cursor position
        self.cursor.0 = self.cursor.0.min(new_cols.saturating_sub(1));
        self.cursor.1 = self.cursor.1.min(new_rows.saturating_sub(1));

        // Clamp the scroll region to the new height
        let last = new_rows.saturating_sub(1);
        self.region_bottom = self.region_bottom.min(last);
        self.region_top = self.region_top.min(self.region_bottom);
    }

    pub fn get_cell(&self, col: usize, row: usize) -> Option<&Cell> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        Some(&self.active_grid()[row * self.cols + col])
    }

    // Kept for upcoming Spec Kit features.
    #[allow(dead_code)]
    pub fn set_cell(&mut self, col: usize, row: usize, cell: Cell) {
        if col < self.cols && row < self.rows {
            let cols = self.cols;
            self.active_grid_mut()[row * cols + col] = cell;
        }
    }

    // ANSI control functions

    pub fn print(&mut self, ch: char) {
        // Reset scroll offset when new content arrives
        self.scroll_offset = 0;

        let (col, row) = self.cursor;
        if col < self.cols && row < self.rows {
            let cell = Cell {
                ch,
                fg: self.current_fg,
                bg: self.current_bg,
                flags: self.current_flags,
            };
            let cols = self.cols;
            self.active_grid_mut()[row * cols + col] = cell;
            self.cursor.0 += 1;
            if self.cursor.0 >= self.cols {
                self.linefeed();
            }
        }
    }

    /// ESC D (IND): move down, scrolling the region at the bottom margin.
    pub fn index(&mut self) {
        if self.cursor.1 == self.region_bottom {
            self.scroll_region_up(1);
        } else if self.cursor.1 < self.rows.saturating_sub(1) {
            self.cursor.1 += 1;
        }
    }

    /// ESC M (RI): move up, scrolling the region at the top margin.
    pub fn reverse_index(&mut self) {
        if self.cursor.1 == self.region_top {
            self.scroll_region_down(1);
        } else {
            self.cursor.1 = self.cursor.1.saturating_sub(1);
        }
    }

    /// ESC E (NEL): carriage return + index.
    pub fn next_line(&mut self) {
        self.cursor.0 = 0;
        self.index();
    }

    pub fn linefeed(&mut self) {
        self.index();
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
        self.cursor.0 = col.min(self.cols.saturating_sub(1));
        self.cursor.1 = row.min(self.rows.saturating_sub(1));
    }

    /// CSI G (CHA): move to absolute column (0-based).
    pub fn cursor_to_column(&mut self, col: usize) {
        self.cursor.0 = col.min(self.cols.saturating_sub(1));
    }

    /// CSI d (VPA): move to absolute row (0-based).
    pub fn cursor_to_row(&mut self, row: usize) {
        self.cursor.1 = row.min(self.rows.saturating_sub(1));
    }

    /// CSI E (CNL): down n lines, column 0.
    pub fn cursor_next_line(&mut self, n: usize) {
        self.cursor_down(n);
        self.cursor.0 = 0;
    }

    /// CSI F (CPL): up n lines, column 0.
    pub fn cursor_prev_line(&mut self, n: usize) {
        self.cursor_up(n);
        self.cursor.0 = 0;
    }

    pub fn clear_all(&mut self) {
        for cell in self.active_grid_mut().iter_mut() {
            *cell = Cell::default();
        }
    }

    pub fn clear_below(&mut self) {
        let (col, row) = self.cursor;
        let cols = self.cols;
        let rows = self.rows;
        let grid = self.active_grid_mut();
        for c in col..cols {
            grid[row * cols + c] = Cell::default();
        }
        for r in (row + 1)..rows {
            for c in 0..cols {
                grid[r * cols + c] = Cell::default();
            }
        }
    }

    pub fn clear_above(&mut self) {
        let (col, row) = self.cursor;
        let cols = self.cols;
        let grid = self.active_grid_mut();
        for r in 0..row {
            for c in 0..cols {
                grid[r * cols + c] = Cell::default();
            }
        }
        for c in 0..=col.min(cols.saturating_sub(1)) {
            grid[row * cols + c] = Cell::default();
        }
    }

    pub fn clear_line(&mut self) {
        let row = self.cursor.1;
        let cols = self.cols;
        let grid = self.active_grid_mut();
        for c in 0..cols {
            grid[row * cols + c] = Cell::default();
        }
    }

    pub fn clear_line_right(&mut self) {
        let (col, row) = self.cursor;
        let cols = self.cols;
        let grid = self.active_grid_mut();
        for c in col..cols {
            grid[row * cols + c] = Cell::default();
        }
    }

    pub fn clear_line_left(&mut self) {
        let (col, row) = self.cursor;
        let cols = self.cols;
        let grid = self.active_grid_mut();
        for c in 0..=col.min(cols.saturating_sub(1)) {
            grid[row * cols + c] = Cell::default();
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

    fn region_height(&self) -> usize {
        self.region_bottom
            .saturating_sub(self.region_top)
            .saturating_add(1)
    }

    /// Scroll the region up by n lines. Lines only enter scrollback when
    /// scrolling the full-height region on the main screen.
    fn scroll_region_up(&mut self, n: usize) {
        let n = n.min(self.region_height());
        if n == 0 || self.cols == 0 || self.rows == 0 {
            return;
        }
        let full_height = self.region_top == 0 && self.region_bottom == self.rows.saturating_sub(1);
        if full_height && !self.on_alt {
            for i in 0..n {
                let top = self.region_top + i;
                let line: Vec<Cell> = self.grid[top * self.cols..(top + 1) * self.cols].to_vec();
                self.scrollback.push(line);
                if self.scrollback.len() > self.scrollback_limit {
                    self.scrollback.remove(0);
                }
            }
        }
        let (top, bottom, cols) = (self.region_top, self.region_bottom, self.cols);
        let grid = self.active_grid_mut();
        if n < bottom + 1 - top {
            for row in top..=(bottom - n) {
                for col in 0..cols {
                    grid[row * cols + col] = grid[(row + n) * cols + col];
                }
            }
        }
        for row in (bottom + 1 - n)..=bottom {
            for col in 0..cols {
                grid[row * cols + col] = Cell::default();
            }
        }
    }

    /// Scroll the region down by n lines (never touches scrollback).
    fn scroll_region_down(&mut self, n: usize) {
        let n = n.min(self.region_height());
        if n == 0 || self.cols == 0 || self.rows == 0 {
            return;
        }
        let (top, bottom, cols) = (self.region_top, self.region_bottom, self.cols);
        let grid = self.active_grid_mut();
        for row in ((top + n)..=bottom).rev() {
            for col in 0..cols {
                grid[row * cols + col] = grid[(row - n) * cols + col];
            }
        }
        for row in top..(top + n) {
            for col in 0..cols {
                grid[row * cols + col] = Cell::default();
            }
        }
    }

    /// CSI S (SU): scroll the region up n lines.
    pub fn csi_scroll_up(&mut self, n: usize) {
        self.scroll_region_up(n);
    }

    /// CSI T (SD): scroll the region down n lines.
    pub fn csi_scroll_down(&mut self, n: usize) {
        self.scroll_region_down(n);
    }

    /// CSI L (IL): insert n blank lines at the cursor row (region-aware).
    pub fn insert_lines(&mut self, n: usize) {
        let row = self.cursor.1;
        if row < self.region_top || row > self.region_bottom {
            return;
        }
        let n = n.min(self.region_bottom + 1 - row);
        if n == 0 {
            return;
        }
        let (bottom, cols) = (self.region_bottom, self.cols);
        let grid = self.active_grid_mut();
        for r in ((row + n)..=bottom).rev() {
            for col in 0..cols {
                grid[r * cols + col] = grid[(r - n) * cols + col];
            }
        }
        for r in row..(row + n) {
            for col in 0..cols {
                grid[r * cols + col] = Cell::default();
            }
        }
    }

    /// CSI M (DL): delete n lines at the cursor row (region-aware).
    pub fn delete_lines(&mut self, n: usize) {
        let row = self.cursor.1;
        if row < self.region_top || row > self.region_bottom {
            return;
        }
        let n = n.min(self.region_bottom + 1 - row);
        if n == 0 {
            return;
        }
        let (bottom, cols) = (self.region_bottom, self.cols);
        let grid = self.active_grid_mut();
        if n < bottom + 1 - row {
            for r in row..=(bottom - n) {
                for col in 0..cols {
                    grid[r * cols + col] = grid[(r + n) * cols + col];
                }
            }
        }
        for r in (bottom + 1 - n)..=bottom {
            for col in 0..cols {
                grid[r * cols + col] = Cell::default();
            }
        }
    }

    /// CSI @ (ICH): insert n blank chars at the cursor, shifting right.
    pub fn insert_chars(&mut self, n: usize) {
        let (col, row) = self.cursor;
        if col >= self.cols || row >= self.rows {
            return;
        }
        let n = n.min(self.cols - col);
        if n == 0 {
            return;
        }
        let cols = self.cols;
        let grid = self.active_grid_mut();
        for c in ((col + n)..cols).rev() {
            grid[row * cols + c] = grid[row * cols + c - n];
        }
        for c in col..(col + n) {
            grid[row * cols + c] = Cell::default();
        }
    }

    /// CSI P (DCH): delete n chars at the cursor, shifting left.
    pub fn delete_chars(&mut self, n: usize) {
        let (col, row) = self.cursor;
        if col >= self.cols || row >= self.rows {
            return;
        }
        let n = n.min(self.cols - col);
        if n == 0 {
            return;
        }
        let cols = self.cols;
        let grid = self.active_grid_mut();
        for c in col..(cols - n) {
            grid[row * cols + c] = grid[row * cols + c + n];
        }
        for c in (cols - n)..cols {
            grid[row * cols + c] = Cell::default();
        }
    }

    /// CSI X (ECH): erase n chars starting at the cursor.
    pub fn erase_chars(&mut self, n: usize) {
        let (col, row) = self.cursor;
        if col >= self.cols || row >= self.rows {
            return;
        }
        let end = (col + n).min(self.cols);
        let cols = self.cols;
        let grid = self.active_grid_mut();
        for c in col..end {
            grid[row * cols + c] = Cell::default();
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

        if self.on_alt || self.scroll_offset == 0 {
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
    #[test]
    fn alt_screen_1049_saves_and_restores_cursor_and_grid() {
        let mut t = term(5, 3);
        print_str(&mut t, "hi");
        t.set_alt_screen_1049(true);
        assert!(t.on_alternate_screen());
        assert_eq!(t.get_cell(0, 0).unwrap().ch, ' ');
        t.cursor_goto(0, 0);
        print_str(&mut t, "ALT");
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'A');
        t.set_alt_screen_1049(false);
        assert!(!t.on_alternate_screen());
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'h');
        assert_eq!(t.cursor(), (2, 0));
    }

    #[test]
    fn alt_screen_1049_reentry_clears() {
        let mut t = term(5, 2);
        t.set_alt_screen_47(true);
        print_str(&mut t, "zz");
        t.set_alt_screen_47(false);
        t.set_alt_screen_1049(true);
        assert_eq!(t.get_cell(0, 0).unwrap().ch, ' ');
    }

    #[test]
    fn alt_screen_47_preserves_both_grids_no_cursor_save() {
        let mut t = term(5, 2);
        print_str(&mut t, "ab");
        t.set_alt_screen_47(true);
        print_str(&mut t, "XY");
        assert_eq!(t.get_cell(2, 0).unwrap().ch, 'X');
        t.set_alt_screen_47(false);
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
        assert_eq!(t.cursor(), (4, 0));
        t.set_alt_screen_47(true);
        assert_eq!(t.get_cell(2, 0).unwrap().ch, 'X');
    }

    #[test]
    fn alt_screen_1047_exit_clears_alt() {
        let mut t = term(5, 2);
        t.set_alt_screen_1047(true);
        print_str(&mut t, "zz");
        t.set_alt_screen_1047(false);
        t.set_alt_screen_1047(true);
        assert_eq!(t.get_cell(0, 0).unwrap().ch, ' ');
    }

    #[test]
    fn alt_screen_scroll_does_not_push_scrollback() {
        let mut t = term(3, 2);
        t.set_alt_screen_1049(true);
        for _ in 0..5 {
            t.print('x');
            t.linefeed();
        }
        assert_eq!(t.scrollback.len(), 0);
    }

    #[test]
    fn alt_viewport_ignores_scroll_offset() {
        let mut t = term(3, 2);
        print_str(&mut t, "ab");
        t.linefeed();
        print_str(&mut t, "cd");
        t.linefeed();
        assert_eq!(t.scrollback.len(), 1);
        t.scroll_viewport_up(1);
        assert_eq!(t.get_viewport_cell(0, 0).unwrap().ch, 'a');
        t.set_alt_screen_47(true);
        assert_eq!(t.get_viewport_cell(0, 0).unwrap().ch, ' ');
    }

    #[test]
    fn scroll_region_linefeed_scrolls_only_region() {
        let mut t = term(3, 5);
        for r in 0..5 {
            t.cursor_goto(0, r);
            t.print((b'a' + r as u8) as char);
        }
        t.set_scroll_region(2, 4);
        assert_eq!(t.scroll_region(), (1, 3));
        assert_eq!(t.cursor(), (0, 0));
        t.cursor_goto(0, 3);
        t.linefeed();
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
        assert_eq!(t.get_cell(0, 1).unwrap().ch, 'c');
        assert_eq!(t.get_cell(0, 3).unwrap().ch, ' ');
        assert_eq!(t.get_cell(0, 4).unwrap().ch, 'e');
        assert_eq!(t.scrollback.len(), 0);
        assert_eq!(t.cursor(), (0, 3));
    }

    #[test]
    fn scroll_region_invalid_means_full_screen() {
        let mut t = term(3, 5);
        t.set_scroll_region(2, 4);
        t.set_scroll_region(4, 2);
        assert_eq!(t.scroll_region(), (0, 4));
        t.set_scroll_region(0, 99);
        assert_eq!(t.scroll_region(), (0, 4));
        t.set_scroll_region(0, 0);
        assert_eq!(t.scroll_region(), (0, 4));
    }

    #[test]
    fn linefeed_below_region_clamps_without_scroll() {
        let mut t = term(3, 4);
        t.set_scroll_region(1, 2);
        t.cursor_goto(0, 3);
        t.print('z');
        t.cursor_goto(0, 3);
        t.linefeed();
        assert_eq!(t.cursor(), (0, 3));
        assert_eq!(t.get_cell(0, 3).unwrap().ch, 'z');
        assert_eq!(t.scrollback.len(), 0);
    }

    #[test]
    fn index_and_reverse_index_region_aware() {
        let mut t = term(3, 4);
        for r in 0..4 {
            t.cursor_goto(0, r);
            t.print((b'a' + r as u8) as char);
        }
        t.set_scroll_region(2, 3);
        t.cursor_goto(2, 1);
        t.reverse_index();
        assert_eq!(t.cursor(), (2, 1));
        assert_eq!(t.get_cell(0, 1).unwrap().ch, ' ');
        assert_eq!(t.get_cell(0, 2).unwrap().ch, 'b');
        t.cursor_goto(1, 2);
        t.index();
        assert_eq!(t.cursor(), (1, 2));
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
        assert_eq!(t.get_cell(0, 3).unwrap().ch, 'd');
    }

    #[test]
    fn next_line_does_cr_and_index() {
        let mut t = term(5, 3);
        t.cursor_goto(3, 0);
        t.next_line();
        assert_eq!(t.cursor(), (0, 1));
    }

    #[test]
    fn insert_and_delete_lines_region_aware() {
        let mut t = term(3, 4);
        for r in 0..4 {
            t.cursor_goto(0, r);
            t.print((b'a' + r as u8) as char);
        }
        t.set_scroll_region(1, 3);
        t.cursor_goto(0, 0);
        t.insert_lines(1);
        assert_eq!(t.get_cell(0, 0).unwrap().ch, ' ');
        assert_eq!(t.get_cell(0, 1).unwrap().ch, 'a');
        assert_eq!(t.get_cell(0, 3).unwrap().ch, 'd');
        t.delete_lines(1);
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
        t.cursor_goto(0, 3);
        t.insert_lines(1);
        assert_eq!(t.get_cell(0, 3).unwrap().ch, 'd');
    }

    #[test]
    fn insert_delete_erase_chars() {
        let mut t = term(6, 2);
        print_str(&mut t, "abcd");
        t.cursor_goto(1, 0);
        t.insert_chars(2);
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
        assert_eq!(t.get_cell(1, 0).unwrap().ch, ' ');
        assert_eq!(t.get_cell(3, 0).unwrap().ch, 'b');
        t.delete_chars(2);
        assert_eq!(t.get_cell(1, 0).unwrap().ch, 'b');
        t.cursor_goto(2, 0);
        t.erase_chars(2);
        assert_eq!(t.get_cell(2, 0).unwrap().ch, ' ');
        assert_eq!(t.get_cell(1, 0).unwrap().ch, 'b');
    }

    #[test]
    fn csi_scroll_up_down_region() {
        let mut t = term(3, 4);
        for r in 0..4 {
            t.cursor_goto(0, r);
            t.print((b'a' + r as u8) as char);
        }
        t.set_scroll_region(2, 4);
        t.csi_scroll_up(1);
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
        assert_eq!(t.get_cell(0, 1).unwrap().ch, 'c');
        t.csi_scroll_down(1);
        assert_eq!(t.get_cell(0, 1).unwrap().ch, ' ');
        assert_eq!(t.get_cell(0, 2).unwrap().ch, 'c');
        t.csi_scroll_up(999);
        assert_eq!(t.get_cell(0, 1).unwrap().ch, ' ');
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
    }

    #[test]
    fn save_restore_cursor_with_attrs() {
        let mut t = term(10, 3);
        t.sgr(31);
        t.sgr(1);
        t.cursor_goto(3, 2);
        t.save_cursor();
        t.sgr(0);
        t.cursor_goto(0, 0);
        t.restore_cursor();
        assert_eq!(t.cursor(), (3, 2));
        t.print('x');
        let cell = t.get_cell(3, 2).unwrap();
        assert_eq!(cell.fg, Color::Indexed(1));
        assert!(cell.flags.contains(CellFlags::BOLD));
    }

    #[test]
    fn resize_resizes_alt_and_clamps_region() {
        let mut t = term(5, 5);
        t.set_scroll_region(2, 5);
        t.set_alt_screen_47(true);
        t.cursor_goto(0, 0);
        print_str(&mut t, "alt");
        t.resize(3, 3);
        assert_eq!(t.scroll_region(), (1, 2));
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
        assert!(t.get_cell(0, 4).is_none());
    }
}
