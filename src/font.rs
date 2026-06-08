use fontdue::{Font, FontSettings};
use std::collections::HashMap;

pub struct FontManager {
    font: Font,
    font_size: f32,
    glyph_cache: HashMap<char, Glyph>,
}

#[derive(Clone)]
pub struct Glyph {
    pub bitmap: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub advance: f32,
    pub offset_x: i32,
    pub offset_y: i32,
}

impl FontManager {
    pub fn new(font_size: f32) -> Self {
        // Embedded monospace font (using default system font as fallback)
        let font_data = include_bytes!("../assets/SourceCodePro-Regular.ttf");
        let font = Font::from_bytes(font_data as &[u8], FontSettings::default())
            .expect("Failed to load font");
        
        Self {
            font,
            font_size,
            glyph_cache: HashMap::new(),
        }
    }
    
    pub fn get_glyph(&mut self, ch: char) -> &Glyph {
        if !self.glyph_cache.contains_key(&ch) {
            let (metrics, bitmap) = self.font.rasterize(ch, self.font_size);
            
            let glyph = Glyph {
                bitmap,
                width: metrics.width,
                height: metrics.height,
                advance: metrics.advance_width,
                offset_x: metrics.xmin,
                offset_y: metrics.ymin,
            };
            
            self.glyph_cache.insert(ch, glyph);
        }
        
        self.glyph_cache.get(&ch).unwrap()
    }
    
    pub fn cell_width(&mut self) -> f32 {
        self.get_glyph('M').advance
    }
    
    pub fn cell_height(&self) -> f32 {
        self.font_size * 1.3
    }
}
