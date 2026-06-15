use std::sync::Arc;
use winit::{
    dpi::{LogicalSize, PhysicalSize},
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

        Self {
            window,
            renderer,
            parser,
            pty,
            config,
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
}
