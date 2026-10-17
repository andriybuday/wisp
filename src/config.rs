use serde::Deserialize;
use std::path::PathBuf;

fn default_font_size() -> f32 {
    14.0
}
fn default_cols() -> usize {
    80
}
fn default_rows() -> usize {
    24
}
fn default_padding() -> f32 {
    4.0
}
fn default_scrollback_lines() -> usize {
    1000
}
fn default_cursor_blink() -> bool {
    true
}

/// Optional base-colour overrides from the `[colors]` table, each `#rrggbb`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Colors {
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub cursor: Option<String>,
}

/// Basic configuration for the terminal. Every field is optional in the
/// TOML file; unset fields take the `Default` values below.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_cols")]
    pub cols: usize,
    #[serde(default = "default_rows")]
    pub rows: usize,
    #[serde(default = "default_padding")]
    pub padding: f32,
    /// Scrollback cap; clamped to `100..=100000` when applied.
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: usize,
    /// Shell to spawn; `None` falls back to `$SHELL`, then `/bin/bash`.
    pub shell: Option<String>,
    /// Consumed by feature 007 (event-driven rendering); unused for now.
    #[allow(dead_code)]
    #[serde(default = "default_cursor_blink")]
    pub cursor_blink: bool,
    pub colors: Colors,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            cols: default_cols(),
            rows: default_rows(),
            padding: default_padding(),
            scrollback_lines: default_scrollback_lines(),
            shell: None,
            cursor_blink: default_cursor_blink(),
            colors: Colors::default(),
        }
    }
}

impl Config {
    /// Parse a config from TOML text (the testable core of [`Config::load`]).
    pub fn from_toml_str(s: &str) -> Result<Config, toml::de::Error> {
        toml::from_str(s)
    }

    /// Load from the resolved config path. A missing file (or no resolvable
    /// path) returns defaults silently; any read/parse error warns and
    /// returns full defaults (never a partial config).
    pub fn load() -> Self {
        let Some(path) = Self::resolve_path(|key| std::env::var(key).ok()) else {
            return Self::default();
        };
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                log::warn!("Failed to read config {}: {e}", path.display());
                return Self::default();
            }
        };
        match Self::from_toml_str(&contents) {
            Ok(config) => config,
            Err(e) => {
                log::warn!("Failed to parse config {}: {e}", path.display());
                Self::default()
            }
        }
    }

    /// `$WISP_CONFIG`, else `$XDG_CONFIG_HOME/wisp/config.toml`, else
    /// `$HOME/.config/wisp/config.toml`. Empty values count as unset. Takes
    /// an env-lookup closure so tests never mutate process env.
    pub fn resolve_path(env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
        let get = |key: &str| env(key).filter(|v| !v.is_empty());
        if let Some(path) = get("WISP_CONFIG") {
            return Some(PathBuf::from(path));
        }
        if let Some(xdg) = get("XDG_CONFIG_HOME") {
            return Some(PathBuf::from(xdg).join("wisp/config.toml"));
        }
        get("HOME").map(|home| PathBuf::from(home).join(".config/wisp/config.toml"))
    }
}

/// Parse `#rrggbb` into linear-ish `[r, g, b, 1.0]` floats; `None` for
/// anything that is not exactly `#` plus six hex digits.
pub fn parse_hex_color(s: &str) -> Option<[f32; 4]> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([
        byte(0)? as f32 / 255.0,
        byte(2)? as f32 / 255.0,
        byte(4)? as f32 / 255.0,
        1.0,
    ])
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
    /// Default palette with any `[colors]` overrides from `config` applied.
    /// Malformed colour strings keep the default for that slot.
    pub fn from_config(config: &Config) -> Self {
        let mut palette = Self::default();
        let apply = |slot: &mut [f32; 4], value: &Option<String>| {
            if let Some(color) = value.as_deref().and_then(parse_hex_color) {
                *slot = color;
            }
        };
        apply(&mut palette.background, &config.colors.background);
        apply(&mut palette.foreground, &config.colors.foreground);
        apply(&mut palette.cursor, &config.colors.cursor);
        palette
    }

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
    fn from_toml_str_full_file_applies_every_field() {
        let c = Config::from_toml_str(
            "font_size = 18.5\ncols = 120\nrows = 40\npadding = 8.0\n\
             scrollback_lines = 5000\nshell = \"/bin/zsh\"\ncursor_blink = false\n\
             [colors]\nbackground = \"#000000\"\nforeground = \"#ffffff\"\ncursor = \"#ff8000\"\n",
        )
        .unwrap();
        assert_eq!(c.font_size, 18.5);
        assert_eq!(c.cols, 120);
        assert_eq!(c.rows, 40);
        assert_eq!(c.padding, 8.0);
        assert_eq!(c.scrollback_lines, 5000);
        assert_eq!(c.shell.as_deref(), Some("/bin/zsh"));
        assert!(!c.cursor_blink);
        assert_eq!(c.colors.background.as_deref(), Some("#000000"));
        assert_eq!(c.colors.foreground.as_deref(), Some("#ffffff"));
        assert_eq!(c.colors.cursor.as_deref(), Some("#ff8000"));
    }

    #[test]
    fn from_toml_str_empty_gives_todays_defaults() {
        let c = Config::from_toml_str("").unwrap();
        let d = Config::default();
        assert_eq!(
            (c.font_size, c.cols, c.rows, c.padding),
            (14.0, 80, 24, 4.0)
        );
        assert_eq!(c.scrollback_lines, 1000);
        assert_eq!(c.shell, None);
        assert!(c.cursor_blink);
        assert!(c.colors.background.is_none());
        assert_eq!(c.font_size, d.font_size);
    }

    #[test]
    fn from_toml_str_partial_keeps_other_defaults() {
        let c = Config::from_toml_str("cols = 100\n[colors]\ncursor = \"#ff8000\"\n").unwrap();
        assert_eq!(c.cols, 100);
        assert_eq!((c.font_size, c.rows, c.padding), (14.0, 24, 4.0));
        assert_eq!(c.colors.cursor.as_deref(), Some("#ff8000"));
        assert!(c.colors.background.is_none());
    }

    #[test]
    fn from_toml_str_invalid_returns_err() {
        assert!(Config::from_toml_str("cols = = 80").is_err());
        assert!(Config::from_toml_str("[colors").is_err());
    }

    #[test]
    fn parse_hex_color_values_and_rejections() {
        assert_eq!(parse_hex_color("#000000"), Some([0.0, 0.0, 0.0, 1.0]));
        assert_eq!(parse_hex_color("#ffffff"), Some([1.0, 1.0, 1.0, 1.0]));
        assert_eq!(
            parse_hex_color("#ff8000"),
            Some([1.0, 128.0 / 255.0, 0.0, 1.0])
        );
        for bad in ["", "ff8000", "#fff", "#ff80000", "#gg0000", "# ff8000"] {
            assert_eq!(parse_hex_color(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn resolve_path_precedence_wisp_over_xdg_over_home() {
        let env = |pairs: &[(&str, &str)]| {
            let pairs: Vec<(String, String)> = pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            move |key: &str| pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
        };
        let all = env(&[
            ("WISP_CONFIG", "/custom/c.toml"),
            ("XDG_CONFIG_HOME", "/xdg"),
            ("HOME", "/home/u"),
        ]);
        assert_eq!(
            Config::resolve_path(all),
            Some(PathBuf::from("/custom/c.toml"))
        );
        let xdg = env(&[("XDG_CONFIG_HOME", "/xdg"), ("HOME", "/home/u")]);
        assert_eq!(
            Config::resolve_path(xdg),
            Some(PathBuf::from("/xdg/wisp/config.toml"))
        );
        let home = env(&[("HOME", "/home/u")]);
        assert_eq!(
            Config::resolve_path(home),
            Some(PathBuf::from("/home/u/.config/wisp/config.toml"))
        );
        assert_eq!(Config::resolve_path(env(&[])), None);
    }

    #[test]
    fn from_config_applies_color_overrides() {
        let mut c = Config::default();
        c.colors.background = Some("#000000".into());
        c.colors.cursor = Some("#ff8000".into());
        let p = ColorPalette::from_config(&c);
        assert_eq!(p.background, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(p.cursor, [1.0, 128.0 / 255.0, 0.0, 1.0]);
        assert_eq!(p.foreground, ColorPalette::default().foreground);
        assert_eq!(p.ansi, ColorPalette::default().ansi);
        // Malformed override keeps the default.
        c.colors.foreground = Some("blue".into());
        assert_eq!(
            ColorPalette::from_config(&c).foreground,
            ColorPalette::default().foreground
        );
    }

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
