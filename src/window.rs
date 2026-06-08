use std::sync::Arc;
use winit::{
    dpi::PhysicalSize,
    event_loop::EventLoop,
    window::{Window, WindowAttributes},
};

use crate::config::Config;
use crate::renderer::Renderer;
use crate::terminal::Terminal;
use crate::pty::PtyManager;
use crate::parser::AnsiParser;

pub struct WispWindow {
    window: Arc<Window>,
    renderer: Renderer,
    parser: AnsiParser,
    pty: PtyManager,
}

impl WispWindow {
    pub fn new(event_loop: &EventLoop<()>) -> Self {
        let config = Config::default();
        
        // Calculate initial window size based on terminal grid
        let cell_width = (config.font_size * 0.6) as u32;  // Approximate
        let cell_height = (config.font_size * 1.2) as u32;
        let width = cell_width * config.cols as u32 + (config.padding * 2.0) as u32;
        let height = cell_height * config.rows as u32 + (config.padding * 2.0) as u32;
        
        let window_attrs = WindowAttributes::default()
            .with_title("Wisp Terminal")
            .with_inner_size(PhysicalSize::new(width, height))
            .with_min_inner_size(PhysicalSize::new(400, 300));
        
        let window = Arc::new(event_loop.create_window(window_attrs)
            .expect("Failed to create window"));
        
        let renderer = Renderer::new(window.clone(), &config);
        let terminal = Terminal::new(config.cols, config.rows);
        let parser = AnsiParser::new(terminal);
        let pty = PtyManager::new(config.cols as u16, config.rows as u16)
            .expect("Failed to create PTY");
        
        Self {
            window,
            renderer,
            parser,
            pty,
        }
    }
    
    pub fn window(&self) -> &Window {
        &self.window
    }
    
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.renderer.resize(new_size);
        // TODO: Calculate new grid size and resize terminal
    }
    
    pub fn render(&mut self) {
        // Read from PTY and process
        while let Some(data) = self.pty.try_read() {
            self.parser.advance(&data);
        }
        
        self.renderer.render(self.parser.terminal());
    }
    
    pub fn send_input(&mut self, data: &[u8]) {
        let _ = self.pty.write(data);
    }
}
