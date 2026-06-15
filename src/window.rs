use std::sync::Arc;
use winit::{
    dpi::{LogicalSize, PhysicalSize, PhysicalPosition},
    event_loop::EventLoop,
    window::{Window, WindowAttributes},
};

use crate::config::Config;
use crate::parser::AnsiParser;
use crate::pty::PtyManager;
use crate::renderer::Renderer;
use crate::terminal::Terminal;

pub struct WispWindow {
    window: Arc<Window>,
    renderer: Renderer,
    parser: AnsiParser,
    pty: PtyManager,
    config: Config,
    clipboard: Option<arboard::Clipboard>,
    mouse_pressed: bool,
    last_mouse_position: Option<PhysicalPosition<f64>>,
}

impl WispWindow {
    pub fn new(event_loop: &EventLoop<()>) -> Self {
        let config = Config::default();

        // Calculate initial window size based on terminal grid.
        // These are logical (point) dimensions; winit scales them to physical
        // pixels using the display's scale factor, and the renderer lays out
        // glyphs in physical pixels to match.
        let mut font_manager = crate::font::FontManager::new(config.font_size);
        let cell_width = font_manager.cell_width() as u32;
        let cell_height = font_manager.cell_height() as u32;
        let width = cell_width * config.cols as u32 + (config.padding * 2.0) as u32;
        let height = cell_height * config.rows as u32 + (config.padding * 2.0) as u32;

        let window_attrs = WindowAttributes::default()
            .with_title("Wisp Terminal")
            .with_inner_size(LogicalSize::new(width, height))
            .with_min_inner_size(LogicalSize::new(400, 300));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        // Enable IME for text input
        window.set_ime_allowed(true);

        let renderer = Renderer::new(window.clone(), &config);
        let terminal = Terminal::new(config.cols, config.rows);
        let parser = AnsiParser::new(terminal);
        let pty =
            PtyManager::new(config.cols as u16, config.rows as u16).expect("Failed to create PTY");

        let clipboard = arboard::Clipboard::new()
            .map_err(|e| eprintln!("Clipboard unavailable: {e}"))
            .ok();

        Self {
            window,
            renderer,
            parser,
            pty,
            config,
            clipboard,
            mouse_pressed: false,
            last_mouse_position: None,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.renderer.resize(new_size);

        // Calculate new grid size. new_size is in physical pixels, so the cell
        // metrics and padding must also be in physical pixels (scaled).
        let scale = self.window.scale_factor() as f32;
        let mut font_manager = crate::font::FontManager::new(self.config.font_size * scale);
        let cell_width = font_manager.cell_width();
        let cell_height = font_manager.cell_height();
        let padding = self.config.padding * scale * 2.0;

        let available_width = new_size.width as f32 - padding;
        let available_height = new_size.height as f32 - padding;

        let new_cols = (available_width / cell_width).floor() as usize;
        let new_rows = (available_height / cell_height).floor() as usize;

        // Ensure minimum size
        let new_cols = new_cols.max(10);
        let new_rows = new_rows.max(5);

        // Resize terminal and PTY
        self.parser.terminal_mut().resize(new_cols, new_rows);
        self.pty.resize(new_cols as u16, new_rows as u16);
    }

    pub fn render(&mut self) {
        // Read from PTY and process
        while let Some(data) = self.pty.try_read() {
            println!(
                "PTY output: {:?} '{}'",
                data,
                String::from_utf8_lossy(&data)
            );
            self.parser.advance(&data);
        }

        self.renderer.render(self.parser.terminal());
    }

    pub fn send_input(&mut self, data: &[u8]) {
        let _ = self.pty.write(data);
    }

    /// Paste the clipboard text into the shell. When the application has
    /// enabled bracketed paste (DEC 2004), wrap the text in the paste markers
    /// so multi-line content isn't executed line-by-line.
    pub fn paste(&mut self) {
        let Some(clipboard) = self.clipboard.as_mut() else {
            return;
        };
        let Ok(text) = clipboard.get_text() else {
            return;
        };
        if text.is_empty() {
            return;
        }

        if self.parser.terminal().bracketed_paste() {
            // Strip any embedded end marker so pasted content can't spoof the
            // end of the paste and inject commands.
            let sanitized = text.replace("\x1b[201~", "");
            let _ = self.pty.write(b"\x1b[200~");
            let _ = self.pty.write(sanitized.as_bytes());
            let _ = self.pty.write(b"\x1b[201~");
        } else {
            let _ = self.pty.write(text.as_bytes());
        }
    }

    /// Copy selected text to clipboard
    pub fn copy_selection(&mut self) {
        let text = self.parser.terminal().get_selected_text();
        if text.is_empty() {
            return;
        }

        if let Some(clipboard) = self.clipboard.as_mut() {
            let _ = clipboard.set_text(text);
        }
    }

    /// Handle mouse press for selection start
    pub fn mouse_press(&mut self) {
        self.mouse_pressed = true;
        
        // Convert physical position to grid coordinates
        if let Some(position) = self.last_mouse_position {
            if let Some((col, row)) = self.position_to_grid(position) {
                self.parser.terminal_mut().set_selection_start(col, row);
            }
        }
    }

    /// Handle mouse drag for selection update
    pub fn mouse_move(&mut self, position: PhysicalPosition<f64>) {
        self.last_mouse_position = Some(position);

        if !self.mouse_pressed {
            return;
        }

        // Convert physical position to grid coordinates
        if let Some((col, row)) = self.position_to_grid(position) {
            self.parser.terminal_mut().set_selection_end(col, row);
        }
    }

    /// Handle mouse release
    pub fn mouse_release(&mut self) {
        self.mouse_pressed = false;
    }

    /// Handle mouse wheel for scrolling
    pub fn mouse_wheel(&mut self, delta: f32) {
        // Positive delta = scroll up (show older content)
        // Negative delta = scroll down (show newer content)
        let lines = (delta.abs() / 10.0).ceil().max(1.0) as usize;
        
        if delta > 0.0 {
            self.parser.terminal_mut().scroll_viewport_up(lines);
        } else if delta < 0.0 {
            self.parser.terminal_mut().scroll_viewport_down(lines);
        }
    }

    /// Convert physical pixel position to grid coordinates (col, row)
    fn position_to_grid(&self, position: PhysicalPosition<f64>) -> Option<(usize, usize)> {
        let scale = self.window.scale_factor() as f32;
        let padding = self.config.padding * scale;

        // Calculate cell dimensions
        let mut font_manager = crate::font::FontManager::new(self.config.font_size * scale);
        let cell_width = font_manager.cell_width();
        let cell_height = font_manager.cell_height();

        // Adjust for padding
        let x = position.x as f32 - padding;
        let y = position.y as f32 - padding;

        if x < 0.0 || y < 0.0 {
            return None;
        }

        let col = (x / cell_width) as usize;
        let row = (y / cell_height) as usize;

        let terminal = self.parser.terminal();
        if col >= terminal.cols() || row >= terminal.rows() {
            return None;
        }

        Some((col, row))
    }
}
