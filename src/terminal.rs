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
        
        let mut terminal = Self {
            grid,
            cols,
            rows,
            cursor: (0, 0),
            scrollback: Vec::new(),
            scrollback_limit: 1000,
        };
        
        // Add some test content
        terminal.write_test_content();
        
        terminal
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
    
    /// Temporary test content to verify rendering
    fn write_test_content(&mut self) {
        let test_lines = [
            "Wisp Terminal Emulator - MVP",
            "",
            "Testing basic text rendering...",
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            "abcdefghijklmnopqrstuvwxyz",
            "0123456789 !@#$%^&*()",
            "",
            "Grid size: {}x{}",
        ];
        
        for (row, line) in test_lines.iter().enumerate() {
            let content = if line.contains("{}") {
                line.replace("{}", &format!("{}", self.cols))
                    .replace("{}", &format!("{}", self.rows))
            } else {
                line.to_string()
            };
            
            for (col, ch) in content.chars().enumerate() {
                if col < self.cols && row < self.rows {
                    self.grid[row * self.cols + col] = Cell {
                        ch,
                        fg: 7,
                        bg: 0,
                        flags: CellFlags::empty(),
                    };
                }
            }
        }
        
        self.cursor = (0, test_lines.len());
    }
}
