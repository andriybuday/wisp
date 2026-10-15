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

impl ColorPalette {
    /// Resolve a 256-color palette index to RGBA: 0-15 use the ANSI table,
    /// 16-231 use the 6x6x6 color cube, 232-255 use the grayscale ramp.
    pub fn ansi_256(&self, n: u8) -> [f32; 4] {
        match n {
            0..=15 => self.ansi[n as usize],
            16..=231 => {
                const STEPS: [f32; 6] = [0.0, 95.0, 135.0, 175.0, 215.0, 255.0];
                let n = n - 16;
                [
                    STEPS[(n / 36) as usize] / 255.0,
                    STEPS[((n / 6) % 6) as usize] / 255.0,
                    STEPS[(n % 6) as usize] / 255.0,
                    1.0,
                ]
            }
            232..=255 => {
                let v = (8.0 + 10.0 * (n - 232) as f32) / 255.0;
                [v, v, v, 1.0]
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_256_first_16_use_ansi_table() {
        let p = ColorPalette::default();
        assert_eq!(p.ansi_256(0), p.ansi[0]);
        assert_eq!(p.ansi_256(7), p.ansi[7]);
        assert_eq!(p.ansi_256(15), p.ansi[15]);
    }

    #[test]
    fn ansi_256_cube_values() {
        let p = ColorPalette::default();
        // 16 is cube origin (0,0,0); 231 is the far corner (255,255,255).
        assert_eq!(p.ansi_256(16), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(p.ansi_256(231), [1.0, 1.0, 1.0, 1.0]);
        // 196 = 16 + 36*5 -> (255, 0, 0); 21 = 16 + 5 -> (0, 0, 255).
        assert_eq!(p.ansi_256(196), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(p.ansi_256(21), [0.0, 0.0, 1.0, 1.0]);
        // Mid-cube: 59 = 16 + 36*1 + 6*1 + 1 -> (95, 95, 95).
        let v = 95.0 / 255.0;
        assert_eq!(p.ansi_256(59), [v, v, v, 1.0]);
    }

    #[test]
    fn ansi_256_grayscale_values() {
        let p = ColorPalette::default();
        let lo = 8.0 / 255.0;
        assert_eq!(p.ansi_256(232), [lo, lo, lo, 1.0]);
        let hi = 238.0 / 255.0; // 8 + 10*23
        assert_eq!(p.ansi_256(255), [hi, hi, hi, 1.0]);
        let mid = 128.0 / 255.0; // 8 + 10*12, n = 244
        assert_eq!(p.ansi_256(244), [mid, mid, mid, 1.0]);
    }
}
