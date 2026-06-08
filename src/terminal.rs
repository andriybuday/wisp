/// Represents a single cell in the terminal grid
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub fg: u8,  // Foreground color index (0-15)
    pub bg: u8,  // Background color index (0-15)
    pub flags: CellFlags,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: 7,  // Default white
            bg: 0,  // Default black
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
        }
    }
    
    pub fn cols(&self) -> usize {
        self.cols
    }
    
    pub fn rows(&self) -> usize {
        self.rows
    }
    
    pub fn cursor(&self) -> (usize, usize) {
        self.cursor
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
        let (col, row) = self.cursor;
        if col < self.cols && row < self.rows {
            self.grid[row * self.cols + col] = Cell {
                ch,
                fg: 7,
                bg: 0,
                flags: CellFlags::empty(),
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
        // Reset to default colors
    }
    
    pub fn sgr(&mut self, param: usize) {
        // Handle SGR parameters (colors, bold, etc.)
        // For MVP, we'll implement basic colors
        match param {
            0 => {}, // Reset
            30..=37 => {}, // Foreground colors
            40..=47 => {}, // Background colors
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
}
