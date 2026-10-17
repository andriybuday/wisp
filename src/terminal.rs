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
    /// Interned OSC 8 hyperlink id (index + 1 into the terminal's
    /// hyperlink table); 0 means no link.
    pub link: u16,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Default,
            bg: Color::Default,
            flags: CellFlags::empty(),
            link: 0,
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
        /// Second cell of a wide (width-2) character; `ch` is `' '`.
        const WIDE_SPACER = 0b0001_0000;
        const DIM = 0b0010_0000;
        const STRIKETHROUGH = 0b0100_0000;
    }
}

/// Mouse tracking level requested by the application (DEC private modes
/// ?1000 / ?1002 / ?1003). Declaration order is the precedence order:
/// a higher level replaces a lower one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MouseTracking {
    /// No mouse reporting (?1000/?1002/?1003 all reset).
    Off,
    /// ?1000: report button press and release only.
    Normal,
    /// ?1002: press/release plus motion while a button is held.
    ButtonEvent,
    /// ?1003: press/release plus all motion.
    AnyEvent,
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
    /// Scrollback buffer; each entry carries the row's wrapped flag.
    scrollback: Vec<(Vec<Cell>, bool)>,
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
    /// Mouse tracking level (DEC private modes 1000/1002/1003)
    mouse_tracking: MouseTracking,
    /// SGR mouse encoding (DEC private mode 1006) is enabled
    sgr_mouse: bool,
    /// Focus in/out reporting (DEC private mode 1004) is enabled
    focus_reporting: bool,
    /// Synchronized output (DEC private mode 2026) is enabled: the window
    /// holds presentation until the mode is reset (or its failsafe fires).
    sync_output: bool,
    /// Window title set via OSC 0 / OSC 2
    title: Option<String>,
    /// Text selection range in absolute lines:
    /// (start_col, start_abs_line, end_col, end_abs_line). Absolute line
    /// indices address scrollback lines followed by grid rows (on the
    /// alternate screen, just alt-grid rows).
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
    /// Interned OSC 8 hyperlink URIs; a cell's `link` id is index + 1.
    hyperlinks: Vec<String>,
    /// Link id stamped into newly printed cells (0 = no link).
    current_link: u16,
    /// Deferred wrap: last cell written, wrap on next printable.
    pending_wrap: bool,
    /// Per-row continuation flags for the main grid.
    wrapped: Vec<bool>,
    /// Per-row continuation flags for the alternate grid.
    alt_wrapped: Vec<bool>,
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
            mouse_tracking: MouseTracking::Off,
            sgr_mouse: false,
            focus_reporting: false,
            sync_output: false,
            title: None,
            selection: None,
            scroll_offset: 0,
            alt_grid,
            on_alt: false,
            region_top: 0,
            region_bottom: rows.saturating_sub(1),
            saved_cursor: (0, 0, Color::Default, Color::Default, CellFlags::empty()),
            alt_entry_cursor: (0, 0),
            hyperlinks: Vec::new(),
            current_link: 0,
            pending_wrap: false,
            wrapped: vec![false; rows],
            alt_wrapped: vec![false; rows],
        }
    }

    /// Set the scrollback cap (from config), trimming the oldest lines.
    pub fn set_scrollback_limit(&mut self, limit: usize) {
        self.scrollback_limit = limit;
        if self.scrollback.len() > limit {
            let excess = self.scrollback.len() - limit;
            self.scrollback.drain(..excess);
            self.shift_selection_lines(excess);
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

    #[allow(dead_code)] // used by tests; renderer adoption later
    fn active_wrapped(&self) -> &Vec<bool> {
        if self.on_alt {
            &self.alt_wrapped
        } else {
            &self.wrapped
        }
    }

    fn active_wrapped_mut(&mut self) -> &mut Vec<bool> {
        if self.on_alt {
            &mut self.alt_wrapped
        } else {
            &mut self.wrapped
        }
    }

    fn set_wrapped(&mut self, row: usize, value: bool) {
        let flags = self.active_wrapped_mut();
        if row < flags.len() {
            flags[row] = value;
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
        self.pending_wrap = false;
        if enabled {
            if !self.on_alt {
                self.selection = None;
                self.alt_entry_cursor = self.cursor;
                self.on_alt = true;
                for cell in &mut self.alt_grid {
                    *cell = Cell::default();
                }
                for flag in &mut self.alt_wrapped {
                    *flag = false;
                }
            }
        } else if self.on_alt {
            self.selection = None;
            self.on_alt = false;
            self.cursor = (
                self.alt_entry_cursor.0.min(self.cols.saturating_sub(1)),
                self.alt_entry_cursor.1.min(self.rows.saturating_sub(1)),
            );
        }
    }

    /// DECSET/DECRST ?47: switch buffers, no cursor save, no clear.
    pub fn set_alt_screen_47(&mut self, enabled: bool) {
        self.pending_wrap = false;
        if enabled != self.on_alt {
            self.selection = None;
        }
        self.on_alt = enabled;
    }

    /// DECSET/DECRST ?1047: switch buffers, no cursor save; entering does
    /// not clear, leaving clears the alternate grid.
    pub fn set_alt_screen_1047(&mut self, enabled: bool) {
        self.pending_wrap = false;
        if enabled {
            if !self.on_alt {
                self.selection = None;
            }
            self.on_alt = true;
        } else {
            if self.on_alt {
                self.selection = None;
            }
            for cell in &mut self.alt_grid {
                *cell = Cell::default();
            }
            for flag in &mut self.alt_wrapped {
                *flag = false;
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
        self.pending_wrap = false;
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
        self.pending_wrap = false;
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

    pub fn mouse_tracking(&self) -> MouseTracking {
        self.mouse_tracking
    }

    /// DECSET/DECRST for one tracking mode (?1000/?1002/?1003). Enabling a
    /// mode at or above the current level selects it (a lower mode never
    /// downgrades a higher one); disabling the active mode drops tracking
    /// to Off, and disabling any other mode is a no-op.
    pub fn set_mouse_tracking(&mut self, mode: MouseTracking, enabled: bool) {
        if enabled {
            if mode >= self.mouse_tracking {
                self.mouse_tracking = mode;
            }
        } else if mode == self.mouse_tracking {
            self.mouse_tracking = MouseTracking::Off;
        }
    }

    pub fn sgr_mouse(&self) -> bool {
        self.sgr_mouse
    }

    pub fn set_sgr_mouse(&mut self, enabled: bool) {
        self.sgr_mouse = enabled;
    }

    pub fn focus_reporting(&self) -> bool {
        self.focus_reporting
    }

    pub fn set_focus_reporting(&mut self, enabled: bool) {
        self.focus_reporting = enabled;
    }
    pub fn sync_output(&self) -> bool {
        self.sync_output
    }

    pub fn set_sync_output(&mut self, enabled: bool) {
        self.sync_output = enabled;
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }

    /// OSC 8: set (or clear, with `None`/empty/invalid) the hyperlink
    /// applied to subsequently printed cells. Only `http://`, `https://`,
    /// and `mailto:` URIs of at most 2048 bytes are accepted; anything
    /// else clears the link like an empty URI. URIs are deduplicated by
    /// linear search into a table capped at 1024 entries; beyond the cap
    /// new links are not interned and text prints without a link.
    pub fn set_hyperlink(&mut self, uri: Option<&str>) {
        const MAX_URI_BYTES: usize = 2048;
        const MAX_HYPERLINKS: usize = 1024;
        let Some(uri) = uri else {
            self.current_link = 0;
            return;
        };
        let accepted = !uri.is_empty()
            && uri.len() <= MAX_URI_BYTES
            && (uri.starts_with("http://")
                || uri.starts_with("https://")
                || uri.starts_with("mailto:"));
        if !accepted {
            self.current_link = 0;
            return;
        }
        if let Some(idx) = self.hyperlinks.iter().position(|u| u == uri) {
            self.current_link = (idx + 1) as u16;
            return;
        }
        if self.hyperlinks.len() >= MAX_HYPERLINKS {
            self.current_link = 0;
            return;
        }
        self.hyperlinks.push(uri.to_owned());
        self.current_link = self.hyperlinks.len() as u16;
    }

    /// Resolve the hyperlink URI of a viewport cell (scroll offset
    /// honoured via `get_viewport_cell`), if any.
    pub fn hyperlink_at_viewport(&self, col: usize, row: usize) -> Option<&str> {
        let cell = self.get_viewport_cell(col, row)?;
        if cell.link == 0 {
            return None;
        }
        self.hyperlinks
            .get((cell.link - 1) as usize)
            .map(|s| s.as_str())
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
        fn is_filler(cell: &Cell) -> bool {
            // Link-carrying blanks are content, not filler: trimming them
            // would shorten an OSC 8 span's trailing spaces on resize.
            cell.ch == ' ' && cell.bg == Color::Default && cell.flags.is_empty() && cell.link == 0
        }

        // Join physical rows into logical lines, trimming trailing
        // blank filler. Spacers (non-empty flags) stay with their char.
        fn logical_lines(physical: &[(Vec<Cell>, bool)]) -> Vec<Vec<Cell>> {
            let mut lines: Vec<Vec<Cell>> = Vec::new();
            for (cells, wrapped) in physical {
                if *wrapped && !lines.is_empty() {
                    lines.last_mut().unwrap().extend_from_slice(cells);
                } else {
                    lines.push(cells.clone());
                }
            }
            for line in &mut lines {
                while line.last().map(is_filler).unwrap_or(false) {
                    line.pop();
                }
            }
            lines
        }

        // Re-chunk one logical line; never split a wide/spacer pair: the
        // pair moves to the next chunk and the gap gets a blank pad.
        fn chunk(line: &[Cell], new_cols: usize) -> Vec<(Vec<Cell>, bool)> {
            let mut out: Vec<Vec<Cell>> = Vec::new();
            let mut cur: Vec<Cell> = Vec::new();
            let flush = |out: &mut Vec<Vec<Cell>>, cur: &mut Vec<Cell>| {
                while cur.len() < new_cols {
                    cur.push(Cell::default());
                }
                out.push(std::mem::take(cur));
            };
            let mut i = 0;
            while i < line.len() {
                let wide = i + 1 < line.len()
                    && line[i + 1].flags.contains(CellFlags::WIDE_SPACER)
                    && !line[i].flags.contains(CellFlags::WIDE_SPACER);
                if wide && new_cols >= 2 {
                    if cur.len() + 2 > new_cols {
                        flush(&mut out, &mut cur);
                    }
                    cur.push(line[i]);
                    cur.push(line[i + 1]);
                    i += 2;
                } else if wide {
                    // Degenerate 1-column target: keep head only.
                    if !cur.is_empty() {
                        flush(&mut out, &mut cur);
                    }
                    let mut head = line[i];
                    head.flags.remove(CellFlags::WIDE_SPACER);
                    cur.push(head);
                    i += 2;
                } else {
                    if cur.len() >= new_cols {
                        flush(&mut out, &mut cur);
                    }
                    cur.push(line[i]);
                    i += 1;
                }
            }
            flush(&mut out, &mut cur);
            out.into_iter()
                .enumerate()
                .map(|(idx, cells)| (cells, idx > 0))
                .collect()
        }

        // Reflow joined logical lines. `keep_up_to` bounds retained
        // lines; trailing blanks past it are free space. Returns
        // (physical rows, first physical row index per logical line).
        fn reflow(
            physical: &[(Vec<Cell>, bool)],
            keep_up_to: Option<usize>,
            new_cols: usize,
        ) -> (Vec<(Vec<Cell>, bool)>, Vec<usize>) {
            let mut lines = logical_lines(physical);
            let mut last = lines.iter().rposition(|l| !l.is_empty()).unwrap_or(0);
            if let Some(k) = keep_up_to {
                last = last.max(k.min(lines.len().saturating_sub(1)));
            }
            if lines.is_empty() {
                lines.push(Vec::new());
                last = 0;
            }
            lines.truncate(last + 1);
            let mut rows = Vec::new();
            let mut firsts = Vec::new();
            for line in &lines {
                firsts.push(rows.len());
                rows.extend(chunk(line, new_cols));
            }
            (rows, firsts)
        }

        if new_cols == 0 || new_rows == 0 || self.cols == 0 || self.rows == 0 {
            self.pending_wrap = false;
            return;
        }
        let old_cols = self.cols;
        let grid_rows = |g: &[Cell], w: &[bool], rows: usize| -> Vec<(Vec<Cell>, bool)> {
            (0..rows)
                .map(|r| {
                    (
                        g[r * old_cols..(r + 1) * old_cols].to_vec(),
                        w.get(r).copied().unwrap_or(false),
                    )
                })
                .collect()
        };

        // Locate the cursor's logical line and cell offset (active grid).
        let locate = |physical: &[(Vec<Cell>, bool)], phys_idx: usize, col: usize| {
            let mut start = phys_idx.min(physical.len().saturating_sub(1));
            while start > 0 && physical[start].1 {
                start -= 1;
            }
            // Count of logical-line starts before `start`.
            let logical_idx = physical[..start]
                .iter()
                .enumerate()
                .filter(|(i, (_, w))| *i == 0 || !*w)
                .count();
            let offset = (phys_idx - start) * old_cols + col;
            (logical_idx, offset)
        };

        // Main grid, with scrollback prepended.
        let main_phys: Vec<(Vec<Cell>, bool)> = self
            .scrollback
            .iter()
            .cloned()
            .chain(grid_rows(&self.grid, &self.wrapped, self.rows))
            .collect();
        let alt_phys = grid_rows(&self.alt_grid, &self.alt_wrapped, self.rows);
        let (active_phys, active_idx) = if self.on_alt {
            (&alt_phys, self.cursor.1)
        } else {
            (&main_phys, self.scrollback.len() + self.cursor.1)
        };
        let (cur_logical, cur_offset) = locate(active_phys, active_idx, self.cursor.0);

        let (main_rows, main_firsts) = reflow(
            &main_phys,
            if self.on_alt { None } else { Some(cur_logical) },
            new_cols,
        );
        let (alt_rows, alt_firsts) = reflow(
            &alt_phys,
            if self.on_alt { Some(cur_logical) } else { None },
            new_cols,
        );

        let take_grid = |rows: &[(Vec<Cell>, bool)]| -> (Vec<Cell>, Vec<bool>, usize) {
            let start = rows.len().saturating_sub(new_rows);
            let mut g = vec![Cell::default(); new_cols * new_rows];
            let mut w = vec![false; new_rows];
            for (ri, (cells, wrapped)) in rows[start..].iter().enumerate() {
                g[ri * new_cols..(ri + 1) * new_cols].copy_from_slice(&cells[..new_cols]);
                w[ri] = *wrapped;
            }
            (g, w, start)
        };
        let (new_grid, new_wrapped, main_start) = take_grid(&main_rows);
        let (new_alt, new_alt_wrapped, alt_start) = take_grid(&alt_rows);
        let mut overflow: Vec<(Vec<Cell>, bool)> = main_rows[..main_start].to_vec();
        if overflow.len() > self.scrollback_limit {
            overflow.drain(..overflow.len() - self.scrollback_limit);
        }
        let (firsts, start) = if self.on_alt {
            (&alt_firsts, alt_start)
        } else {
            (&main_firsts, main_start)
        };
        let first = firsts.get(cur_logical).copied().unwrap_or(0);
        let abs_row = first + cur_offset / new_cols;
        let grid_row = abs_row.saturating_sub(start).min(new_rows - 1);
        self.scrollback = overflow;
        self.grid = new_grid;
        self.wrapped = new_wrapped;
        self.alt_grid = new_alt;
        self.alt_wrapped = new_alt_wrapped;
        self.cols = new_cols;
        self.rows = new_rows;
        self.cursor = ((cur_offset % new_cols).min(new_cols - 1), grid_row);
        self.region_top = 0;
        self.region_bottom = new_rows - 1;
        self.scroll_offset = 0;
        self.pending_wrap = false;
        self.selection = None;
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

    /// Blank one cell: `ch` becomes `' '` and `WIDE_SPACER` is removed,
    /// but colors (and other flags) are kept.
    fn blank_cell_at(&mut self, col: usize, row: usize) {
        let cols = self.cols;
        let cell = &mut self.active_grid_mut()[row * cols + col];
        cell.ch = ' ';
        cell.flags.remove(CellFlags::WIDE_SPACER);
    }

    /// Overwrite hygiene for one cell about to be written: if it is a
    /// spacer, blank the wide char to its left; else if the next cell is
    /// a spacer (this cell is a wide head), blank that spacer. This
    /// keeps half of a wide pair from being stranded by an overwrite.
    fn blank_wide_neighbor(&mut self, col: usize, row: usize) {
        let idx = row * self.cols + col;
        if self.active_grid()[idx]
            .flags
            .contains(CellFlags::WIDE_SPACER)
        {
            if col > 0 {
                self.blank_cell_at(col - 1, row);
            }
        } else if col + 1 < self.cols
            && self.active_grid()[idx + 1]
                .flags
                .contains(CellFlags::WIDE_SPACER)
        {
            self.blank_cell_at(col + 1, row);
        }
    }

    /// The wrap path: linefeed, marking the destination row wrapped.
    fn wrap_to_next_line(&mut self) {
        self.pending_wrap = false;
        let before = self.cursor.1;
        let scrolled = before == self.region_bottom;
        self.index();
        self.cursor.0 = 0;
        if scrolled || self.cursor.1 != before {
            let row = self.cursor.1;
            self.set_wrapped(row, true);
        }
    }

    pub fn print(&mut self, ch: char) {
        use unicode_width::UnicodeWidthChar;

        // Width-0 chars (combining marks, ZWJ/ZWSP, controls reported as
        // None) are dropped entirely: no cell change, no cursor advance.
        // Grapheme composition is a documented non-goal (see 006 spec).
        let width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width == 0 {
            return;
        }

        // Reset scroll offset when new content arrives
        self.scroll_offset = 0;

        if self.cols == 0 || self.rows == 0 {
            return;
        }

        // Deferred wrap: the next printable performs the wrap first.
        if self.pending_wrap {
            self.wrap_to_next_line();
        } else if width >= 2 && self.cols >= 2 && self.cursor.0 == self.cols - 1 {
            // A wide char cannot fit in the last column: wrap first.
            self.wrap_to_next_line();
        }

        let (col, row) = self.cursor;
        if col >= self.cols || row >= self.rows {
            return;
        }

        let cell = Cell {
            ch,
            fg: self.current_fg,
            bg: self.current_bg,
            flags: self.current_flags,
            link: self.current_link,
        };
        let cols = self.cols;
        if width >= 2 && col + 1 < cols {
            self.blank_wide_neighbor(col, row);
            self.blank_wide_neighbor(col + 1, row);
            // The spacer copies fg/bg/flags so backgrounds and inverse
            // video render continuously across both columns.
            let spacer = Cell {
                ch: ' ',
                flags: cell.flags | CellFlags::WIDE_SPACER,
                ..cell
            };
            let grid = self.active_grid_mut();
            grid[row * cols + col] = cell;
            grid[row * cols + col + 1] = spacer;
            if col + 1 == cols - 1 {
                self.cursor.0 = cols - 1;
                self.pending_wrap = true;
            } else {
                self.cursor.0 += 2;
            }
        } else {
            // Width 1 (also the degenerate 1-column fallback for a
            // wide char, which has no room for a spacer).
            self.blank_wide_neighbor(col, row);
            self.active_grid_mut()[row * cols + col] = cell;
            if col == cols - 1 {
                self.pending_wrap = true;
            } else {
                self.cursor.0 += 1;
            }
        }
    }

    /// ESC D (IND): move down, scrolling the region at the bottom margin.
    pub fn index(&mut self) {
        self.pending_wrap = false;
        if self.cursor.1 == self.region_bottom {
            self.scroll_region_up(1);
        } else if self.cursor.1 < self.rows.saturating_sub(1) {
            self.cursor.1 += 1;
        }
    }

    /// ESC M (RI): move up, scrolling the region at the top margin.
    pub fn reverse_index(&mut self) {
        self.pending_wrap = false;
        if self.cursor.1 == self.region_top {
            self.scroll_region_down(1);
        } else {
            self.cursor.1 = self.cursor.1.saturating_sub(1);
        }
    }

    /// ESC E (NEL): carriage return + index.
    pub fn next_line(&mut self) {
        self.pending_wrap = false;
        self.cursor.0 = 0;
        self.index();
    }

    pub fn linefeed(&mut self) {
        self.pending_wrap = false;
        self.index();
        self.cursor.0 = 0;
    }

    pub fn carriage_return(&mut self) {
        self.pending_wrap = false;
        self.cursor.0 = 0;
    }

    pub fn backspace(&mut self) {
        self.pending_wrap = false;
        if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
        }
    }

    pub fn tab(&mut self) {
        self.pending_wrap = false;
        self.cursor.0 = ((self.cursor.0 / 8) + 1) * 8;
        if self.cursor.0 >= self.cols {
            self.linefeed();
        }
    }

    pub fn cursor_up(&mut self, n: usize) {
        self.pending_wrap = false;
        self.cursor.1 = self.cursor.1.saturating_sub(n);
    }

    pub fn cursor_down(&mut self, n: usize) {
        self.pending_wrap = false;
        self.cursor.1 = (self.cursor.1 + n).min(self.rows - 1);
    }

    pub fn cursor_forward(&mut self, n: usize) {
        self.pending_wrap = false;
        self.cursor.0 = (self.cursor.0 + n).min(self.cols - 1);
    }

    pub fn cursor_backward(&mut self, n: usize) {
        self.pending_wrap = false;
        self.cursor.0 = self.cursor.0.saturating_sub(n);
    }

    pub fn cursor_goto(&mut self, col: usize, row: usize) {
        self.pending_wrap = false;
        self.cursor.0 = col.min(self.cols.saturating_sub(1));
        self.cursor.1 = row.min(self.rows.saturating_sub(1));
    }

    /// CSI G (CHA): move to absolute column (0-based).
    pub fn cursor_to_column(&mut self, col: usize) {
        self.pending_wrap = false;
        self.cursor.0 = col.min(self.cols.saturating_sub(1));
    }

    /// CSI d (VPA): move to absolute row (0-based).
    pub fn cursor_to_row(&mut self, row: usize) {
        self.pending_wrap = false;
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
        self.pending_wrap = false;
        for cell in self.active_grid_mut().iter_mut() {
            *cell = Cell::default();
        }
        for flag in self.active_wrapped_mut().iter_mut() {
            *flag = false;
        }
    }

    pub fn clear_below(&mut self) {
        self.pending_wrap = false;
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
        let flags = self.active_wrapped_mut();
        if row + 1 < flags.len() {
            flags[(row + 1)..].fill(false);
        }
    }

    pub fn clear_above(&mut self) {
        self.pending_wrap = false;
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
        let flags = self.active_wrapped_mut();
        let n = row.min(flags.len());
        for f in flags.iter_mut().take(n) {
            *f = false;
        }
    }

    pub fn clear_line(&mut self) {
        self.pending_wrap = false;
        let row = self.cursor.1;
        let cols = self.cols;
        {
            let grid = self.active_grid_mut();
            for c in 0..cols {
                grid[row * cols + c] = Cell::default();
            }
        }
        self.set_wrapped(row, false);
    }

    pub fn clear_line_right(&mut self) {
        self.pending_wrap = false;
        let (col, row) = self.cursor;
        let cols = self.cols;
        let grid = self.active_grid_mut();
        for c in col..cols {
            grid[row * cols + c] = Cell::default();
        }
    }

    pub fn clear_line_left(&mut self) {
        self.pending_wrap = false;
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
            0 => self.reset_sgr(),                                // Reset all
            1 => self.current_flags.insert(CellFlags::BOLD),      // Bold
            2 => self.current_flags.insert(CellFlags::DIM),       // Dim/faint
            3 => self.current_flags.insert(CellFlags::ITALIC),    // Italic
            4 => self.current_flags.insert(CellFlags::UNDERLINE), // Underline
            // 5/6 (blink) fall through to the ignore arm by design.
            7 => self.current_flags.insert(CellFlags::INVERSE), // Inverse
            9 => self.current_flags.insert(CellFlags::STRIKETHROUGH), // Strikethrough
            22 => self.current_flags.remove(CellFlags::BOLD | CellFlags::DIM), // Normal intensity
            23 => self.current_flags.remove(CellFlags::ITALIC), // Not italic
            24 => self.current_flags.remove(CellFlags::UNDERLINE), // Not underline
            27 => self.current_flags.remove(CellFlags::INVERSE), // Not inverse
            29 => self.current_flags.remove(CellFlags::STRIKETHROUGH), // Not strikethrough
            30..=37 => self.current_fg = Color::Indexed((param - 30) as u8), // FG 0-7
            40..=47 => self.current_bg = Color::Indexed((param - 40) as u8), // BG 0-7
            90..=97 => self.current_fg = Color::Indexed((param - 90 + 8) as u8), // FG 8-15
            100..=107 => self.current_bg = Color::Indexed((param - 100 + 8) as u8), // BG 8-15
            39 => self.current_fg = Color::Default,             // Default FG
            49 => self.current_bg = Color::Default,             // Default BG
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
                let flag = self.wrapped.get(top).copied().unwrap_or(false);
                self.scrollback.push((line, flag));
                if self.scrollback.len() > self.scrollback_limit {
                    self.scrollback.remove(0);
                    self.shift_selection_lines(1);
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
        let flags = self.active_wrapped_mut();
        if flags.len() > bottom {
            if n < bottom + 1 - top {
                for row in top..=(bottom - n) {
                    flags[row] = flags[row + n];
                }
            }
            flags[(bottom + 1 - n)..=bottom].fill(false);
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
        let flags = self.active_wrapped_mut();
        if flags.len() > bottom {
            for row in ((top + n)..=bottom).rev() {
                flags[row] = flags[row - n];
            }
            flags[top..(top + n)].fill(false);
        }
    }

    /// CSI S (SU): scroll the region up n lines.
    pub fn csi_scroll_up(&mut self, n: usize) {
        self.pending_wrap = false;
        self.scroll_region_up(n);
    }

    /// CSI T (SD): scroll the region down n lines.
    pub fn csi_scroll_down(&mut self, n: usize) {
        self.pending_wrap = false;
        self.scroll_region_down(n);
    }

    /// CSI L (IL): insert n blank lines at the cursor row (region-aware).
    pub fn insert_lines(&mut self, n: usize) {
        self.pending_wrap = false;
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
        let flags = self.active_wrapped_mut();
        if flags.len() > bottom {
            for r in ((row + n)..=bottom).rev() {
                flags[r] = flags[r - n];
            }
            flags[row..(row + n)].fill(false);
        }
    }

    /// CSI M (DL): delete n lines at the cursor row (region-aware).
    pub fn delete_lines(&mut self, n: usize) {
        self.pending_wrap = false;
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
        let flags = self.active_wrapped_mut();
        if flags.len() > bottom {
            if n < bottom + 1 - row {
                for r in row..=(bottom - n) {
                    flags[r] = flags[r + n];
                }
            }
            flags[(bottom + 1 - n)..=bottom].fill(false);
        }
    }

    /// CSI @ (ICH): insert n blank chars at the cursor, shifting right.
    pub fn insert_chars(&mut self, n: usize) {
        self.pending_wrap = false;
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
        self.pending_wrap = false;
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
        self.pending_wrap = false;
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

    // Selection methods (absolute lines; public API takes viewport rows)

    /// Convert a viewport row to an absolute line, using the same math
    /// as `get_viewport_cell`.
    fn viewport_to_abs_line(&self, viewport_row: usize) -> usize {
        if self.on_alt || self.scroll_offset == 0 {
            let base = if self.on_alt {
                0
            } else {
                self.scrollback.len()
            };
            base + viewport_row.min(self.rows.saturating_sub(1))
        } else {
            let total_rows = self.scrollback.len() + self.rows;
            let viewport_start = total_rows.saturating_sub(self.rows + self.scroll_offset);
            (viewport_start + viewport_row).min(total_rows.saturating_sub(1))
        }
    }

    /// Cell at an absolute line (scrollback first, then the active grid).
    fn get_absolute_cell(&self, col: usize, abs_line: usize) -> Option<&Cell> {
        if col >= self.cols {
            return None;
        }
        if self.on_alt {
            self.get_cell(col, abs_line)
        } else if abs_line < self.scrollback.len() {
            self.scrollback
                .get(abs_line)
                .and_then(|line| line.0.get(col))
        } else {
            self.get_cell(col, abs_line - self.scrollback.len())
        }
    }

    fn normalized_selection(&self) -> Option<(usize, usize, usize, usize)> {
        let (sc, sl, ec, el) = self.selection?;
        if (sl, sc) <= (el, ec) {
            Some((sc, sl, ec, el))
        } else {
            Some((ec, el, sc, sl))
        }
    }

    /// Shift stored selection lines down by `n` (oldest lines trimmed).
    fn shift_selection_lines(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        if let Some((sc, sl, ec, el)) = self.selection {
            self.selection = Some((sc, sl.saturating_sub(n), ec, el.saturating_sub(n)));
        }
    }

    fn clamp_col_row(&self, col: usize, row: usize) -> (usize, usize) {
        (
            col.min(self.cols.saturating_sub(1)),
            row.min(self.rows.saturating_sub(1)),
        )
    }

    pub fn set_selection_start(&mut self, col: usize, row: usize) {
        if self.cols == 0 || self.rows == 0 {
            self.selection = None;
            return;
        }
        let (col, row) = self.clamp_col_row(col, row);
        let abs = self.viewport_to_abs_line(row);
        self.selection = Some((col, abs, col, abs));
    }

    pub fn set_selection_end(&mut self, col: usize, row: usize) {
        if self.cols == 0 || self.rows == 0 {
            return;
        }
        if let Some((start_col, start_abs, _, _)) = self.selection {
            let (col, row) = self.clamp_col_row(col, row);
            let abs = self.viewport_to_abs_line(row);
            self.selection = Some((start_col, start_abs, col, abs));
        }
    }

    #[allow(dead_code)]
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Clear the selection only when it is a single cell at the given
    /// viewport position (a no-drag click released where it started).
    pub fn clear_selection_if_single_cell(&mut self, col: usize, row: usize) {
        if self.cols == 0 || self.rows == 0 {
            return;
        }
        let (col, row) = self.clamp_col_row(col, row);
        let abs = self.viewport_to_abs_line(row);
        if self.selection == Some((col, abs, col, abs)) {
            self.selection = None;
        }
    }

    #[allow(dead_code)]
    pub fn selection(&self) -> Option<(usize, usize, usize, usize)> {
        self.selection
    }

    pub fn is_cell_selected(&self, col: usize, row: usize) -> bool {
        let Some((sc, sl, ec, el)) = self.normalized_selection() else {
            return false;
        };
        if self.cols == 0 || self.rows == 0 {
            return false;
        }
        let abs = self.viewport_to_abs_line(row);
        (sl, sc) <= (abs, col) && (abs, col) <= (el, ec)
    }

    /// Select the word at a viewport cell: expand over word characters
    /// (alphanumeric or `_-./~:@`); a spacer belongs to its wide char.
    /// Clicking a space selects the contiguous space run; clicking any
    /// other character selects just that cell (plus spacer when wide).
    pub fn select_word_at(&mut self, col: usize, viewport_row: usize) {
        if self.cols == 0 || self.rows == 0 {
            self.selection = None;
            return;
        }
        let (col, viewport_row) = self.clamp_col_row(col, viewport_row);
        let abs = self.viewport_to_abs_line(viewport_row);
        let cell_at = |c: usize| self.get_absolute_cell(c, abs).copied();
        let Some(clicked) = cell_at(col) else {
            self.selection = None;
            return;
        };
        // A clicked spacer belongs to the wide char on its left.
        let (head, head_cell) = if clicked.flags.contains(CellFlags::WIDE_SPACER) && col > 0 {
            (col - 1, cell_at(col - 1).unwrap_or(clicked))
        } else {
            (col, clicked)
        };
        let wide_head = |c: usize| -> bool {
            cell_at(c + 1)
                .map(|cell| cell.flags.contains(CellFlags::WIDE_SPACER))
                .unwrap_or(false)
        };
        let cell_end = |c: usize| if wide_head(c) { c + 1 } else { c };
        let is_word = |ch: char| ch.is_alphanumeric() || "_-./~:@".contains(ch);
        let is_word_cell =
            |cell: &Cell| !cell.flags.contains(CellFlags::WIDE_SPACER) && is_word(cell.ch);
        let is_space_cell =
            |cell: &Cell| cell.ch == ' ' && !cell.flags.contains(CellFlags::WIDE_SPACER);

        let (start, end) = if is_word(head_cell.ch) {
            let mut start = head;
            while start > 0 {
                let prev = start - 1;
                let Some(pc) = cell_at(prev) else { break };
                if pc.flags.contains(CellFlags::WIDE_SPACER) {
                    // prev is the spacer of the cell before it.
                    if prev == 0 {
                        break;
                    }
                    let Some(hc) = cell_at(prev - 1) else { break };
                    if !is_word_cell(&hc) {
                        break;
                    }
                    start = prev - 1;
                } else if is_word_cell(&pc) {
                    start = prev;
                } else {
                    break;
                }
            }
            let mut end = cell_end(head);
            while let Some(nc) = cell_at(end + 1) {
                if nc.flags.contains(CellFlags::WIDE_SPACER) || !is_word_cell(&nc) {
                    break;
                }
                end = cell_end(end + 1);
            }
            (start, end)
        } else if is_space_cell(&head_cell) {
            let mut start = head;
            while start > 0 {
                let Some(pc) = cell_at(start - 1) else { break };
                if !is_space_cell(&pc) {
                    break;
                }
                start -= 1;
            }
            let mut end = head;
            while let Some(nc) = cell_at(end + 1) {
                if !is_space_cell(&nc) {
                    break;
                }
                end += 1;
            }
            (start, end)
        } else {
            (head, cell_end(head))
        };
        self.selection = Some((start, abs, end, abs));
    }

    /// Select the full absolute line under a viewport row.
    pub fn select_line_at(&mut self, viewport_row: usize) {
        if self.cols == 0 || self.rows == 0 {
            self.selection = None;
            return;
        }
        let row = viewport_row.min(self.rows.saturating_sub(1));
        let abs = self.viewport_to_abs_line(row);
        self.selection = Some((0, abs, self.cols - 1, abs));
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
                    .and_then(|line| line.0.get(col))
            } else {
                // Get from main grid
                let grid_row = absolute_row - self.scrollback.len();
                self.get_cell(col, grid_row)
            }
        }
    }

    pub fn get_selected_text(&self) -> String {
        let Some((start_col, start_line, end_col, end_line)) = self.normalized_selection() else {
            return String::new();
        };
        if self.cols == 0 {
            return String::new();
        }
        let mut lines: Vec<String> = Vec::new();
        for line in start_line..=end_line {
            let line_start = if line == start_line { start_col } else { 0 };
            let line_end = if line == end_line {
                end_col.min(self.cols - 1)
            } else {
                self.cols - 1
            };
            if line_start > line_end {
                lines.push(String::new());
                continue;
            }
            let mut text = String::new();
            for col in line_start..=line_end {
                if let Some(cell) = self.get_absolute_cell(col, line) {
                    // Spacers are the second half of a wide char;
                    // copied text must not gain an inserted space.
                    if !cell.flags.contains(CellFlags::WIDE_SPACER) {
                        text.push(cell.ch);
                    }
                }
            }
            while text.ends_with(' ') {
                text.pop();
            }
            lines.push(text);
        }
        lines.join("\n")
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
        // Deferred wrap: filling the row leaves the cursor on the
        // last cell; the next printable performs the wrap.
        let mut t = term(3, 3);
        print_str(&mut t, "abc");
        assert_eq!(t.cursor(), (2, 0));
        assert!(t.pending_wrap);
        t.print('d');
        assert_eq!(t.get_cell(2, 0).unwrap().ch, 'c');
        assert_eq!(t.get_cell(0, 1).unwrap().ch, 'd');
        assert_eq!(t.cursor(), (1, 1));
        assert!(!t.pending_wrap);
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
        assert_eq!(t.scrollback[0].0[0].ch, 'a');
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
    fn set_scrollback_limit_trims() {
        let mut t = term(2, 2);
        for ch in ['a', 'b', 'c', 'd', 'e'] {
            t.print(ch);
            t.linefeed();
        }
        assert!(t.scrollback.len() > 2);
        let newest = t.scrollback.last().unwrap().0[0].ch;
        t.set_scrollback_limit(2);
        assert_eq!(t.scrollback_limit, 2);
        assert_eq!(t.scrollback.len(), 2);
        assert_eq!(t.scrollback.last().unwrap().0[0].ch, newest); // oldest dropped
        t.set_scrollback_limit(10);
        assert_eq!(t.scrollback.len(), 2); // raising the cap keeps content
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
    fn sgr_dim_and_strikethrough_set_and_clear() {
        let mut t = term(10, 2);
        t.sgr(2);
        t.sgr(9);
        t.print('a');
        let f = t.get_cell(0, 0).unwrap().flags;
        assert!(f.contains(CellFlags::DIM | CellFlags::STRIKETHROUGH));
        t.sgr(29);
        t.print('b');
        let f = t.get_cell(1, 0).unwrap().flags;
        assert!(f.contains(CellFlags::DIM));
        assert!(!f.contains(CellFlags::STRIKETHROUGH));
    }

    #[test]
    fn sgr_22_clears_bold_and_dim() {
        let mut t = term(10, 2);
        t.sgr(1);
        t.sgr(2);
        t.sgr(4);
        t.sgr(22);
        t.print('a');
        let f = t.get_cell(0, 0).unwrap().flags;
        assert!(!f.contains(CellFlags::BOLD));
        assert!(!f.contains(CellFlags::DIM));
        // Normal intensity leaves other attributes alone.
        assert!(f.contains(CellFlags::UNDERLINE));
    }

    #[test]
    fn sgr_reset_clears_dim_and_strikethrough_and_blink_ignored() {
        let mut t = term(10, 2);
        t.sgr(2);
        t.sgr(9);
        t.sgr(0);
        t.print('a');
        assert!(t.get_cell(0, 0).unwrap().flags.is_empty());
        // Blink (5/6) is parsed and deliberately ignored.
        t.sgr(5);
        t.sgr(6);
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
    fn selection_scrolled_up_copies_scrollback_text() {
        let mut t = term(5, 2);
        print_str(&mut t, "ab");
        t.linefeed();
        print_str(&mut t, "cd");
        t.linefeed();
        assert_eq!(t.scrollback.len(), 1);
        t.scroll_viewport_up(1);
        // Viewport row 0 now shows the scrollback line "ab".
        t.set_selection_start(0, 0);
        t.set_selection_end(1, 0);
        assert!(t.is_cell_selected(0, 0));
        assert!(!t.is_cell_selected(0, 1));
        assert_eq!(t.get_selected_text(), "ab");
        // Scrolling back down does not change what was selected.
        t.scroll_viewport_down(1);
        assert!(!t.is_cell_selected(0, 0));
        assert_eq!(t.get_selected_text(), "ab");
    }

    #[test]
    fn selection_survives_push_into_scrollback() {
        let mut t = term(5, 2);
        print_str(&mut t, "ab");
        t.set_selection_start(0, 0);
        t.set_selection_end(1, 0);
        assert_eq!(t.get_selected_text(), "ab");
        t.linefeed();
        print_str(&mut t, "cd");
        t.linefeed();
        assert_eq!(t.scrollback.len(), 1);
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'c');
        assert_eq!(t.get_selected_text(), "ab");
    }

    #[test]
    fn selected_text_trims_trailing_spaces_keeps_inner() {
        let mut t = term(10, 3);
        print_str(&mut t, "a b");
        t.linefeed();
        print_str(&mut t, "cd");
        t.set_selection_start(0, 0);
        t.set_selection_end(9, 1);
        assert_eq!(t.get_selected_text(), "a b\ncd");
    }

    #[test]
    fn word_select_expands_over_word_chars_and_stops() {
        let mut t = term(24, 2);
        print_str(&mut t, "  foo@bar-baz/qux1.txt z");
        t.select_word_at(5, 0);
        assert_eq!(t.get_selected_text(), "foo@bar-baz/qux1.txt");
        assert!(!t.is_cell_selected(1, 0));
        // Stops at characters outside the word set.
        let mut t = term(10, 2);
        print_str(&mut t, "(hi) *x");
        t.select_word_at(2, 0);
        assert_eq!(t.get_selected_text(), "hi");
        // A non-word, non-space character selects just itself.
        t.select_word_at(0, 0);
        assert_eq!(t.get_selected_text(), "(");
    }

    #[test]
    fn word_select_on_space_selects_space_run() {
        let mut t = term(10, 2);
        print_str(&mut t, "ab   cd");
        t.select_word_at(3, 0);
        assert_eq!(t.selection(), Some((2, 0, 4, 0)));
        assert!(t.is_cell_selected(2, 0));
        assert!(t.is_cell_selected(4, 0));
        assert!(!t.is_cell_selected(1, 0));
        assert!(!t.is_cell_selected(5, 0));
    }

    #[test]
    fn word_select_on_wide_char_includes_spacer() {
        let mut t = term(10, 2);
        t.print('a');
        t.print('\u{754C}');
        t.print('b');
        // Click the spacer cell (col 2) of the wide char at col 1.
        t.select_word_at(2, 0);
        assert!(t.is_cell_selected(1, 0));
        assert!(t.is_cell_selected(2, 0));
        assert_eq!(t.get_selected_text(), "a\u{754C}b");
        t.select_word_at(1, 0);
        assert!(t.is_cell_selected(2, 0));
        assert_eq!(t.get_selected_text(), "a\u{754C}b");
    }

    #[test]
    fn line_select_covers_full_line() {
        let mut t = term(5, 3);
        print_str(&mut t, "ab");
        t.linefeed();
        print_str(&mut t, "cd");
        t.select_line_at(0);
        assert!(t.is_cell_selected(0, 0));
        assert!(t.is_cell_selected(4, 0));
        assert!(!t.is_cell_selected(0, 1));
        assert_eq!(t.get_selected_text(), "ab");
    }

    #[test]
    fn alt_screen_switch_clears_selection() {
        let mut t = term(5, 2);
        print_str(&mut t, "ab");
        t.set_selection_start(0, 0);
        t.set_selection_end(1, 0);
        t.set_alt_screen_1049(true);
        assert_eq!(t.selection(), None);
        assert_eq!(t.get_selected_text(), "");
        // Selections work within the alt grid, then leaving clears.
        t.cursor_goto(0, 0);
        print_str(&mut t, "xy");
        t.set_selection_start(0, 0);
        t.set_selection_end(1, 0);
        assert_eq!(t.get_selected_text(), "xy");
        t.set_alt_screen_1049(false);
        assert_eq!(t.selection(), None);
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
    }

    #[test]
    fn scrollback_trim_shifts_selection_without_panic() {
        // Trim via set_scrollback_limit: same text stays selected.
        let mut t = term(5, 2);
        print_str(&mut t, "aa");
        t.linefeed();
        print_str(&mut t, "bb");
        t.linefeed();
        print_str(&mut t, "cc");
        t.linefeed();
        assert_eq!(t.scrollback.len(), 2);
        t.set_selection_start(0, 0);
        t.set_selection_end(1, 0);
        assert_eq!(t.get_selected_text(), "cc");
        t.set_scrollback_limit(1);
        assert_eq!(t.scrollback.len(), 1);
        assert_eq!(t.get_selected_text(), "cc");

        // Trim via scroll push past the limit saturates, never panics.
        let mut t = term(5, 2);
        t.scrollback_limit = 1;
        print_str(&mut t, "ab");
        t.linefeed();
        print_str(&mut t, "cd");
        t.linefeed();
        t.scroll_viewport_up(1);
        t.set_selection_start(0, 0);
        t.set_selection_end(1, 0);
        assert_eq!(t.get_selected_text(), "ab");
        t.scroll_viewport_down(1);
        print_str(&mut t, "ef");
        t.linefeed();
        // "ab" was trimmed; the saturated selection now covers "cd".
        assert_eq!(t.get_selected_text(), "cd");
    }

    #[test]
    fn clear_selection_if_single_cell_only_clears_no_drag() {
        let mut t = term(5, 2);
        print_str(&mut t, "ab");
        t.set_selection_start(1, 0);
        t.clear_selection_if_single_cell(1, 0);
        assert_eq!(t.selection(), None);
        t.set_selection_start(0, 0);
        t.set_selection_end(1, 0);
        t.clear_selection_if_single_cell(1, 0);
        assert_eq!(t.get_selected_text(), "ab");
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
        // 011: resize resets the scroll region to the full screen.
        assert_eq!(t.scroll_region(), (0, 2));
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
        assert!(t.get_cell(0, 4).is_none());
    }

    #[test]
    fn wide_char_occupies_two_cells_with_spacer() {
        // U+754C is a wide CJK char: head cell + spacer, cursor +2, and
        // the spacer copies fg/bg/flags from the wide cell.
        let mut t = term(10, 3);
        t.sgr(31);
        t.sgr(42);
        t.sgr(7);
        t.print('\u{754C}');
        let head = t.get_cell(0, 0).unwrap();
        assert_eq!(head.ch, '\u{754C}');
        assert_eq!((head.fg, head.bg), (Color::Indexed(1), Color::Indexed(2)));
        assert!(head.flags.contains(CellFlags::INVERSE));
        assert!(!head.flags.contains(CellFlags::WIDE_SPACER));
        let spacer = t.get_cell(1, 0).unwrap();
        assert_eq!(spacer.ch, ' ');
        assert_eq!(
            (spacer.fg, spacer.bg),
            (Color::Indexed(1), Color::Indexed(2))
        );
        assert!(spacer
            .flags
            .contains(CellFlags::INVERSE | CellFlags::WIDE_SPACER));
        assert_eq!(t.cursor(), (2, 0));
    }

    #[test]
    fn emoji_is_width_two() {
        // U+1F980 (crab emoji) is width 2.
        let mut t = term(10, 2);
        t.print('\u{1F980}');
        assert_eq!(t.get_cell(0, 0).unwrap().ch, '\u{1F980}');
        assert!(t
            .get_cell(1, 0)
            .unwrap()
            .flags
            .contains(CellFlags::WIDE_SPACER));
        assert_eq!(t.cursor(), (2, 0));
    }

    #[test]
    fn wide_char_at_last_column_wraps_to_next_line() {
        let mut t = term(4, 3);
        print_str(&mut t, "abc");
        assert_eq!(t.cursor(), (3, 0));
        t.print('\u{754C}');
        assert_eq!(t.get_cell(3, 0).unwrap().ch, ' ');
        assert_eq!(t.get_cell(0, 1).unwrap().ch, '\u{754C}');
        assert!(t
            .get_cell(1, 1)
            .unwrap()
            .flags
            .contains(CellFlags::WIDE_SPACER));
        assert_eq!(t.cursor(), (2, 1));
    }

    #[test]
    fn wide_char_filling_line_end_sets_pending_wrap() {
        // Deferred wrap (011, replaces immediate-wrap): a wide char
        // ending exactly at the edge leaves the cursor on its spacer
        // cell with a pending wrap; the next printable wraps first.
        let mut t = term(4, 3);
        t.cursor_goto(2, 0);
        t.print('\u{754C}');
        assert_eq!(t.get_cell(2, 0).unwrap().ch, '\u{754C}');
        assert!(t
            .get_cell(3, 0)
            .unwrap()
            .flags
            .contains(CellFlags::WIDE_SPACER));
        assert_eq!(t.cursor(), (3, 0));
        assert!(t.pending_wrap);
        t.print('a');
        assert_eq!(t.get_cell(0, 1).unwrap().ch, 'a');
        assert_eq!(t.cursor(), (1, 1));
    }

    #[test]
    fn overwriting_spacer_blanks_wide_char() {
        let mut t = term(10, 2);
        t.sgr(31);
        t.print('\u{754C}');
        t.sgr(32);
        t.cursor_goto(1, 0);
        t.print('x');
        let blanked = t.get_cell(0, 0).unwrap();
        assert_eq!(blanked.ch, ' ');
        // Blanking keeps the wide cell's colors.
        assert_eq!(blanked.fg, Color::Indexed(1));
        let x = t.get_cell(1, 0).unwrap();
        assert_eq!(x.ch, 'x');
        assert_eq!(x.fg, Color::Indexed(2));
        assert!(!x.flags.contains(CellFlags::WIDE_SPACER));
        assert_eq!(t.cursor(), (2, 0));
    }

    #[test]
    fn overwriting_wide_char_blanks_spacer() {
        let mut t = term(10, 2);
        t.sgr(31);
        t.sgr(42);
        t.print('\u{754C}');
        t.sgr(32);
        t.cursor_goto(0, 0);
        t.print('x');
        let x = t.get_cell(0, 0).unwrap();
        assert_eq!(x.ch, 'x');
        assert_eq!(x.fg, Color::Indexed(2));
        let blanked = t.get_cell(1, 0).unwrap();
        assert_eq!(blanked.ch, ' ');
        assert!(!blanked.flags.contains(CellFlags::WIDE_SPACER));
        // Blanking keeps the spacer's (copied) colors.
        assert_eq!(
            (blanked.fg, blanked.bg),
            (Color::Indexed(1), Color::Indexed(2))
        );
        assert_eq!(t.cursor(), (1, 0));
    }

    #[test]
    fn zero_width_space_is_dropped() {
        // U+200B changes no cell and does not advance the cursor.
        let mut t = term(10, 2);
        t.print('a');
        t.print('\u{200B}');
        assert_eq!(t.cursor(), (1, 0));
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'a');
        assert_eq!(t.get_cell(1, 0).unwrap().ch, ' ');
        assert!(t.get_cell(1, 0).unwrap().flags.is_empty());
        t.print('b');
        assert_eq!(t.get_cell(1, 0).unwrap().ch, 'b');
        assert_eq!(t.cursor(), (2, 0));
    }

    #[test]
    fn sync_output_defaults_off_and_toggles() {
        let mut t = term(10, 2);
        assert!(!t.sync_output());
        t.set_sync_output(true);
        assert!(t.sync_output());
        t.set_sync_output(false);
        assert!(!t.sync_output());
    }

    fn row_str(t: &Terminal, row: usize) -> String {
        (0..t.cols())
            .filter_map(|c| t.get_cell(c, row))
            .filter(|cell| !cell.flags.contains(CellFlags::WIDE_SPACER))
            .map(|cell| cell.ch)
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    fn all_text(t: &Terminal) -> String {
        let mut s = String::new();
        for (cells, _) in &t.scrollback {
            for cell in cells {
                if !cell.flags.contains(CellFlags::WIDE_SPACER) {
                    s.push(cell.ch);
                }
            }
        }
        for r in 0..t.rows() {
            s.push_str(&row_str(t, r));
        }
        s.trim_end().to_string()
    }

    #[test]
    fn deferred_fill_exact_stays_and_does_not_scroll() {
        let mut t = term(3, 2);
        print_str(&mut t, "abc");
        assert_eq!(t.cursor(), (2, 0));
        print_str(&mut t, "def");
        assert_eq!(t.cursor(), (2, 1));
        assert_eq!(t.scrollback.len(), 0);
        assert_eq!(row_str(&t, 0), "abc");
        t.print('g'); // now the wrap scrolls
        assert_eq!(t.scrollback.len(), 1);
        assert_eq!(t.scrollback[0].0[0].ch, 'a');
        assert_eq!(row_str(&t, 1), "g");
    }

    #[test]
    fn deferred_cr_cancels_pending_wrap() {
        let mut t = term(3, 2);
        print_str(&mut t, "abc");
        t.carriage_return();
        assert!(!t.pending_wrap);
        t.print('x');
        assert_eq!(t.get_cell(0, 0).unwrap().ch, 'x');
        assert_eq!(t.get_cell(0, 1).unwrap().ch, ' ');
        assert_eq!(t.cursor(), (1, 0));
    }

    #[test]
    fn deferred_exact_line_then_linefeed_no_blank_row() {
        let mut t = term(3, 3);
        print_str(&mut t, "abc");
        t.linefeed();
        t.print('d');
        assert_eq!(t.get_cell(0, 1).unwrap().ch, 'd');
        assert_eq!(t.get_cell(0, 2).unwrap().ch, ' ');
        assert_eq!(t.cursor(), (1, 1));
    }

    #[test]
    fn wrapped_flags_continuation_vs_newline() {
        let mut t = term(3, 4);
        print_str(&mut t, "abcd");
        assert!(!t.active_wrapped()[0]);
        assert!(t.active_wrapped()[1]);
        t.linefeed();
        t.print('e');
        assert_eq!(t.get_cell(0, 2).unwrap().ch, 'e');
        assert!(!t.active_wrapped()[2]);
    }

    #[test]
    fn wrapped_flag_survives_into_scrollback() {
        let mut t = term(3, 2);
        print_str(&mut t, "abcdefg");
        print_str(&mut t, "hi");
        t.linefeed();
        assert_eq!(t.scrollback.len(), 2);
        assert!(!t.scrollback[0].1);
        assert!(t.scrollback[1].1);
        assert_eq!(t.scrollback[1].0[0].ch, 'd');
    }

    #[test]
    fn reflow_narrower_splits_preserving_order() {
        let mut t = term(4, 4);
        print_str(&mut t, "abcdefgh");
        t.resize(2, 4);
        assert_eq!(row_str(&t, 0), "ab");
        assert_eq!(row_str(&t, 3), "gh");
        assert_eq!(all_text(&t), "abcdefgh");
        assert_eq!(t.active_wrapped(), &vec![false, true, true, true]);
        assert_eq!(t.scrollback.len(), 0);
    }

    #[test]
    fn reflow_wider_joins_wrapped_rows() {
        let mut t = term(2, 4);
        print_str(&mut t, "abcdefgh");
        t.resize(4, 4);
        assert_eq!(row_str(&t, 0), "abcd");
        assert_eq!(row_str(&t, 1), "efgh");
        assert_eq!(t.active_wrapped(), &vec![false, true, false, false]);
    }

    #[test]
    fn reflow_includes_scrollback_content() {
        let mut t = term(4, 2);
        print_str(&mut t, "aaaa");
        t.linefeed();
        print_str(&mut t, "bbbb");
        t.linefeed();
        print_str(&mut t, "cc");
        assert_eq!(t.scrollback.len(), 1);
        t.resize(2, 2);
        assert_eq!(t.scrollback.len(), 3);
        assert_eq!(all_text(&t), "aaaabbbbcc");
        assert_eq!(row_str(&t, 1), "cc");
    }

    #[test]
    fn reflow_never_splits_wide_char() {
        let mut t = term(5, 2);
        print_str(&mut t, "ab");
        t.print('\u{754C}');
        t.print('c');
        t.resize(3, 3);
        assert_eq!(t.get_cell(2, 0).unwrap().ch, ' ');
        assert!(!t
            .get_cell(2, 0)
            .unwrap()
            .flags
            .contains(CellFlags::WIDE_SPACER));
        assert_eq!(t.get_cell(0, 1).unwrap().ch, '\u{754C}');
        assert!(t
            .get_cell(1, 1)
            .unwrap()
            .flags
            .contains(CellFlags::WIDE_SPACER));
        assert_eq!(t.get_cell(2, 1).unwrap().ch, 'c');
        assert!(t.active_wrapped()[1]);
    }

    #[test]
    fn reflow_alt_overflow_dropped_not_scrollback() {
        let mut t = term(4, 2);
        print_str(&mut t, "main");
        t.set_alt_screen_1049(true);
        t.cursor_goto(0, 0);
        print_str(&mut t, "abcdefgh");
        t.resize(2, 2);
        assert_eq!(t.scrollback.len(), 0);
        assert_eq!(row_str(&t, 0), "ef");
        assert_eq!(row_str(&t, 1), "gh");
        t.set_alt_screen_1049(false);
        assert_eq!(row_str(&t, 0), "ma");
        assert_eq!(row_str(&t, 1), "in");
    }

    #[test]
    fn reflow_cursor_remapped_through_logical_line() {
        let mut t = term(4, 4);
        print_str(&mut t, "abcdefgh");
        assert_eq!(t.cursor(), (3, 1)); // offset 7
        t.resize(2, 4);
        assert_eq!(t.cursor(), (1, 3)); // 7 / 2, 7 % 2
        let mut t = term(2, 4);
        print_str(&mut t, "abcde");
        assert_eq!(t.cursor(), (1, 2)); // offset 5
        t.resize(4, 4);
        assert_eq!(t.cursor(), (1, 1)); // 5 / 4, 5 % 4
    }

    #[test]
    fn deferred_wide_after_pending_and_empty_resize() {
        let mut t = term(3, 3);
        print_str(&mut t, "abc");
        t.print('\u{754C}'); // wraps first, then wide on row 1
        assert_eq!(t.get_cell(0, 1).unwrap().ch, '\u{754C}');
        assert!(t.active_wrapped()[1]);
        let mut empty = term(4, 2);
        empty.resize(2, 3);
        assert_eq!(empty.cursor(), (0, 0));
        assert_eq!(empty.get_cell(0, 0).unwrap().ch, ' ');
        assert_eq!(empty.scrollback.len(), 0);
    }

    #[test]
    fn reflow_rows_only_and_unchanged_idempotent() {
        let mut t = term(4, 3);
        print_str(&mut t, "abcdefg");
        t.resize(4, 3);
        let once = (row_str(&t, 0), row_str(&t, 1), t.active_wrapped().clone());
        t.resize(4, 3);
        assert_eq!(row_str(&t, 0), once.0);
        assert_eq!(row_str(&t, 1), once.1);
        assert_eq!(t.active_wrapped(), &once.2);
        let mut t = term(4, 3);
        print_str(&mut t, "ab");
        t.linefeed();
        print_str(&mut t, "cd");
        t.linefeed();
        print_str(&mut t, "ef");
        t.resize(4, 2);
        assert_eq!(t.scrollback.len(), 1);
        assert_eq!(row_str(&t, 0), "cd");
        assert_eq!(row_str(&t, 1), "ef");
    }

    #[test]
    fn selected_text_skips_wide_spacers() {
        let mut t = term(10, 2);
        t.print('a');
        t.print('\u{754C}');
        t.print('b');
        assert_eq!(t.cursor(), (4, 0));
        t.set_selection_start(0, 0);
        t.set_selection_end(3, 0);
        assert_eq!(t.get_selected_text(), "a\u{754C}b");
    }

    #[test]
    fn hyperlink_stamps_cells_and_clears() {
        let mut t = term(10, 2);
        print_str(&mut t, "ab");
        t.set_hyperlink(Some("https://example.com"));
        print_str(&mut t, "cd");
        t.set_hyperlink(None);
        print_str(&mut t, "ef");
        // Before/after the span: no link. Inside: same non-zero id.
        assert_eq!(t.get_cell(0, 0).unwrap().link, 0);
        assert_eq!(t.get_cell(1, 0).unwrap().link, 0);
        let id = t.get_cell(2, 0).unwrap().link;
        assert_ne!(id, 0);
        assert_eq!(t.get_cell(3, 0).unwrap().link, id);
        assert_eq!(t.get_cell(4, 0).unwrap().link, 0);
        assert_eq!(t.get_cell(5, 0).unwrap().link, 0);
        // Empty URI also clears.
        t.set_hyperlink(Some("https://example.com"));
        t.set_hyperlink(Some(""));
        t.print('g');
        assert_eq!(t.get_cell(6, 0).unwrap().link, 0);
        assert_eq!(t.hyperlink_at_viewport(2, 0), Some("https://example.com"));
        assert_eq!(t.hyperlink_at_viewport(0, 0), None);
        assert_eq!(t.hyperlink_at_viewport(99, 0), None);
    }

    #[test]
    fn hyperlink_rejects_dangerous_and_unknown_schemes() {
        let mut t = term(10, 2);
        for bad in ["javascript:alert(1)", "file:///etc/passwd", "ftp://x", ""] {
            t.set_hyperlink(Some("https://example.com"));
            t.set_hyperlink(Some(bad));
            t.print('x');
            // The invalid URI cleared the previous link.
            let (col, _) = t.cursor();
            assert_eq!(t.get_cell(col - 1, 0).unwrap().link, 0, "for {bad:?}");
        }
        // mailto: is accepted; overlong URIs are not.
        t.set_hyperlink(Some("mailto:a@example.com"));
        t.print('m');
        let (col, _) = t.cursor();
        assert_ne!(t.get_cell(col - 1, 0).unwrap().link, 0);
        let long = format!("https://example.com/{}", "a".repeat(2048));
        t.set_hyperlink(Some(&long));
        t.print('y');
        let (col, _) = t.cursor();
        assert_eq!(t.get_cell(col - 1, 0).unwrap().link, 0);
    }

    #[test]
    fn hyperlink_interning_dedupes_and_distinguishes() {
        let mut t = term(20, 2);
        t.set_hyperlink(Some("https://a.example"));
        t.print('a');
        t.set_hyperlink(Some("https://b.example"));
        t.print('b');
        t.set_hyperlink(Some("https://a.example"));
        t.print('c');
        let a = t.get_cell(0, 0).unwrap().link;
        let b = t.get_cell(1, 0).unwrap().link;
        let c = t.get_cell(2, 0).unwrap().link;
        assert_ne!(a, 0);
        assert_ne!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn hyperlink_spacer_shares_link_id() {
        let mut t = term(10, 2);
        t.set_hyperlink(Some("https://example.com"));
        t.print('\u{754C}');
        let head = t.get_cell(0, 0).unwrap();
        let spacer = t.get_cell(1, 0).unwrap();
        assert!(spacer.flags.contains(CellFlags::WIDE_SPACER));
        assert_ne!(head.link, 0);
        assert_eq!(spacer.link, head.link);
    }

    #[test]
    fn hyperlink_at_viewport_resolves_through_scroll_offset() {
        let mut t = term(10, 2);
        t.set_hyperlink(Some("https://example.com"));
        print_str(&mut t, "link");
        t.set_hyperlink(None);
        // Push the linked line into scrollback, then scroll up to it.
        t.linefeed();
        t.carriage_return();
        print_str(&mut t, "next");
        t.linefeed();
        t.carriage_return();
        print_str(&mut t, "last");
        assert_eq!(t.scrollback_len(), 1);
        assert_eq!(
            t.hyperlink_at_viewport(0, 0),
            None,
            "at bottom the link scrolled away"
        );
        t.scroll_viewport_up(1);
        assert_eq!(t.hyperlink_at_viewport(0, 0), Some("https://example.com"));
        assert_eq!(t.hyperlink_at_viewport(0, 1), None);
    }

    #[test]
    fn hyperlink_table_cap_prints_without_link() {
        let mut t = term(10, 2);
        for i in 0..1024 {
            t.set_hyperlink(Some(&format!("https://example.com/{i}")));
        }
        t.set_hyperlink(Some("https://example.com/overflow"));
        t.print('x');
        assert_eq!(t.get_cell(0, 0).unwrap().link, 0);
        // An already-interned URI still resolves at the cap.
        t.set_hyperlink(Some("https://example.com/0"));
        t.print('y');
        assert_eq!(t.get_cell(1, 0).unwrap().link, 1);
    }
}
