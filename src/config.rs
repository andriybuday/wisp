/// Basic configuration for the terminal
#[derive(Clone)]
pub struct Config {
    pub font_size: f32,
    pub cols: usize,
    pub rows: usize,
    pub padding: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            cols: 80,
            rows: 24,
            padding: 4.0,
        }
    }
}

/// Terminal color palette
#[derive(Debug, Clone, Copy)]
pub struct ColorPalette {
    pub background: [f32; 4],
    pub foreground: [f32; 4],
    pub cursor: [f32; 4],
    pub ansi: [[f32; 4]; 16],
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self {
            background: [0.08, 0.08, 0.08, 1.0],
            foreground: [0.9, 0.9, 0.9, 1.0],
            cursor: [0.5, 0.7, 1.0, 1.0],
            // Basic 16 ANSI colors
            ansi: [
                // Normal colors
                [0.0, 0.0, 0.0, 1.0], // Black
                [0.8, 0.0, 0.0, 1.0], // Red
                [0.0, 0.8, 0.0, 1.0], // Green
                [0.8, 0.8, 0.0, 1.0], // Yellow
                [0.0, 0.0, 0.8, 1.0], // Blue
                [0.8, 0.0, 0.8, 1.0], // Magenta
                [0.0, 0.8, 0.8, 1.0], // Cyan
                [0.8, 0.8, 0.8, 1.0], // White
                // Bright colors
                [0.4, 0.4, 0.4, 1.0], // Bright Black
                [1.0, 0.0, 0.0, 1.0], // Bright Red
                [0.0, 1.0, 0.0, 1.0], // Bright Green
                [1.0, 1.0, 0.0, 1.0], // Bright Yellow
                [0.0, 0.0, 1.0, 1.0], // Bright Blue
                [1.0, 0.0, 1.0, 1.0], // Bright Magenta
                [0.0, 1.0, 1.0, 1.0], // Bright Cyan
                [1.0, 1.0, 1.0, 1.0], // Bright White
            ],
        }
    }
}
