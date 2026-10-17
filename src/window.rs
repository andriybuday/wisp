use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::MouseButton,
    event_loop::EventLoop,
    keyboard::ModifiersState,
    window::{Window, WindowAttributes},
};

use crate::config::Config;
use crate::input;
use crate::parser::AnsiParser;
use crate::pty::PtyManager;
use crate::renderer::Renderer;
use crate::terminal::{MouseTracking, Terminal};

/// User events delivered to the event loop via `EventLoopProxy`.
#[derive(Debug)]
pub enum WispUserEvent {
    /// The PTY reader thread queued output; parse and redraw.
    PtyOutput,
}

/// Cursor blink period.
const BLINK_PERIOD: Duration = Duration::from_millis(530);
/// Synchronized-output failsafe: present anyway after this hold.
const SYNC_FAILSAFE: Duration = Duration::from_millis(100);

pub struct WispWindow {
    window: Arc<Window>,
    renderer: Renderer,
    parser: AnsiParser,
    pty: PtyManager,
    config: Config,
    clipboard: Option<arboard::Clipboard>,
    last_title: Option<String>,
    mouse_pressed: bool,
    last_mouse_position: Option<PhysicalPosition<f64>>,
    /// Last cell sent in a mouse report, for per-cell motion dedupe.
    last_reported_cell: Option<(usize, usize)>,
    /// Cached physical cell metrics (computed in `new`, refreshed in
    /// `resize`) so mouse positioning needs no per-move `FontManager`.
    cell_width: f32,
    cell_height: f32,
    padding: f32,
    /// Current cursor blink phase and its next flip deadline.
    blink_on: bool,
    next_blink: Instant,
    /// When the current synchronized-output hold was first observed.
    sync_since: Option<Instant>,
}

/// Physical (cell width, cell height, padding) for `config` at `scale`.
fn physical_metrics(config: &Config, scale: f32) -> (f32, f32, f32) {
    let mut font_manager = crate::font::FontManager::new(config.font_size * scale);
    (
        font_manager.cell_width(),
        font_manager.cell_height(),
        config.padding * scale,
    )
}

impl WispWindow {
    pub fn new(event_loop: &EventLoop<WispUserEvent>) -> Self {
        let config = Config::load();

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

        let scale = window.scale_factor() as f32;
        let (cell_width, cell_height, padding) = physical_metrics(&config, scale);

        let renderer = Renderer::new(window.clone(), &config);
        let mut terminal = Terminal::new(config.cols, config.rows);
        terminal.set_scrollback_limit(config.scrollback_lines.clamp(100, 100_000));
        let parser = AnsiParser::new(terminal);
        let mut pty =
            PtyManager::with_shell(config.cols as u16, config.rows as u16, config.shell.clone())
                .expect("Failed to create PTY");

        // Let the PTY reader thread wake the (sleeping) event loop.
        let proxy = event_loop.create_proxy();
        pty.set_waker(Arc::new(move || {
            let _ = proxy.send_event(WispUserEvent::PtyOutput);
        }));

        let clipboard = arboard::Clipboard::new()
            .map_err(|e| log::warn!("Clipboard unavailable: {e}"))
            .ok();

        Self {
            window,
            renderer,
            parser,
            pty,
            config,
            clipboard,
            last_title: None,
            mouse_pressed: false,
            last_mouse_position: None,
            last_reported_cell: None,
            cell_width,
            cell_height,
            padding,
            blink_on: true,
            next_blink: Instant::now() + BLINK_PERIOD,
            sync_since: None,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.renderer.resize(new_size);

        // Refresh cached physical metrics, then derive the new grid size.
        // new_size is in physical pixels, as are the cached metrics.
        let scale = self.window.scale_factor() as f32;
        let (cell_width, cell_height, padding) = physical_metrics(&self.config, scale);
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        self.padding = padding;

        let available_width = new_size.width as f32 - self.padding * 2.0;
        let available_height = new_size.height as f32 - self.padding * 2.0;

        let new_cols = (available_width / self.cell_width).floor() as usize;
        let new_rows = (available_height / self.cell_height).floor() as usize;

        // Ensure minimum size
        let new_cols = new_cols.max(10);
        let new_rows = new_rows.max(5);

        // Resize terminal and PTY
        self.parser.terminal_mut().resize(new_cols, new_rows);
        self.pty.resize(new_cols as u16, new_rows as u16);

        self.window.request_redraw();
    }

    /// Drain queued PTY output into the terminal (parsing, query replies,
    /// title). Returns true if any bytes were processed. Presentation is
    /// separate (`render`) so synchronized output can hold frames.
    pub fn process_pty(&mut self) -> bool {
        let mut processed = false;
        while let Some(data) = self.pty.try_read() {
            processed = true;
            log::debug!(
                "PTY output: {:?} '{}'",
                data,
                String::from_utf8_lossy(&data)
            );
            self.parser.advance(&data);
            // Answer terminal queries (DA, DSR, size report).
            for reply in self.parser.take_replies() {
                let _ = self.pty.write(&reply);
            }
        }

        if processed {
            // Arm/clear the sync failsafe from the post-parse flag.
            if self.parser.terminal().sync_output() {
                if self.sync_since.is_none() {
                    self.sync_since = Some(Instant::now());
                }
            } else {
                self.sync_since = None;
            }

            // Apply OSC title changes.
            if let Some(title) = self.parser.terminal().title() {
                if self.last_title.as_deref() != Some(title) {
                    self.window.set_title(title);
                    self.last_title = Some(title.to_owned());
                }
            }
        }

        processed
    }

    /// True while synchronized output (`?2026`) holds presentation and
    /// the 100 ms failsafe has not yet expired.
    pub fn sync_holding(&self) -> bool {
        if !self.parser.terminal().sync_output() {
            return false;
        }
        match self.sync_since {
            Some(since) => since.elapsed() < SYNC_FAILSAFE,
            // Flag set but no armed hold (failsafe already fired): present.
            None => false,
        }
    }

    /// Earliest deadline the event loop must wake for: the next blink
    /// flip, or the sync failsafe if it comes first.
    pub fn next_deadline(&self) -> Instant {
        let mut deadline = self.next_blink;
        if self.parser.terminal().sync_output() {
            if let Some(since) = self.sync_since {
                deadline = deadline.min(since + SYNC_FAILSAFE);
            }
        }
        deadline
    }

    /// Advance time-based state (blink phase, sync failsafe). Returns
    /// true when a redraw should be requested.
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let mut needs_redraw = false;

        if self.parser.terminal().sync_output() {
            if let Some(since) = self.sync_since {
                if now >= since + SYNC_FAILSAFE {
                    // Present anyway; a later held batch re-arms the hold.
                    self.sync_since = None;
                    needs_redraw = true;
                }
            }
        } else {
            self.sync_since = None;
        }

        if now >= self.next_blink {
            // The blink deadline doubles as the periodic wake the shell-exit
            // check rides on, so it keeps rescheduling even when blinking is
            // disabled in config (in which case the cursor stays steady-on).
            if self.config.cursor_blink {
                self.blink_on = !self.blink_on;
                if self.parser.terminal().cursor_visible() {
                    needs_redraw = true;
                }
            }
            self.next_blink = now + BLINK_PERIOD;
        }

        needs_redraw
    }

    pub fn render(&mut self) {
        self.renderer.render(self.parser.terminal(), self.blink_on);
    }

    /// Has the shell (PTY child) exited? The event loop closes the window
    /// when this becomes true.
    pub fn shell_exited(&mut self) -> bool {
        self.pty.has_exited()
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

    /// True when mouse events go to the application instead of local
    /// selection/scrolling: the app enabled tracking and Shift (the usual
    /// user override) is not held.
    fn mouse_reporting_active(&self, modifiers: ModifiersState) -> bool {
        self.parser.terminal().mouse_tracking() != MouseTracking::Off && !modifiers.shift_key()
    }

    /// Encode one mouse report in the negotiated encoding and send it to
    /// the application via the PTY.
    fn report_mouse(&mut self, button: u32, col: usize, row: usize, pressed: bool) {
        let sgr = self.parser.terminal().sgr_mouse();
        let bytes = input::encode_mouse(button, col, row, pressed, sgr);
        let _ = self.pty.write(&bytes);
    }

    /// The cell under the cursor for reporting, clamped into the grid.
    fn report_cell(&self) -> Option<(usize, usize)> {
        self.last_mouse_position
            .map(|pos| self.position_to_grid_clamped(pos))
    }

    /// Handle a focus change: report `ESC [ I` / `ESC [ O` when the
    /// application enabled focus reporting (?1004).
    pub fn focus_changed(&mut self, focused: bool) {
        if self.parser.terminal().focus_reporting() {
            let seq: &[u8] = if focused { b"\x1b[I" } else { b"\x1b[O" };
            let _ = self.pty.write(seq);
        }
    }

    /// Handle mouse press: reported to the app while tracking is active,
    /// otherwise starts a local (left-button) selection.
    pub fn mouse_press(&mut self, button: MouseButton, modifiers: ModifiersState) {
        if self.mouse_reporting_active(modifiers) {
            if let (Some(base), Some((col, row))) =
                (input::mouse_button_base(button), self.report_cell())
            {
                self.last_reported_cell = Some((col, row));
                let code = base + input::mouse_modifiers(modifiers);
                self.report_mouse(code, col, row, true);
            }
            return;
        }

        if button != MouseButton::Left {
            return;
        }
        self.mouse_pressed = true;

        // Convert physical position to grid coordinates
        if let Some(position) = self.last_mouse_position {
            if let Some((col, row)) = self.position_to_grid(position) {
                self.parser.terminal_mut().set_selection_start(col, row);
                self.window.request_redraw();
            }
        }
    }

    /// Handle cursor motion: reported per the tracking level (button-event
    /// needs a held button, any-event always reports), once per cell
    /// change; otherwise updates the local selection drag.
    pub fn mouse_move(
        &mut self,
        position: PhysicalPosition<f64>,
        modifiers: ModifiersState,
        pressed_buttons: &[MouseButton],
    ) {
        self.last_mouse_position = Some(position);

        if self.mouse_reporting_active(modifiers) {
            let mods = input::mouse_modifiers(modifiers);
            let held = pressed_buttons
                .iter()
                .find_map(|b| input::mouse_button_base(*b));
            let code = match self.parser.terminal().mouse_tracking() {
                MouseTracking::ButtonEvent => held.map(|base| base + input::MOUSE_MOTION + mods),
                MouseTracking::AnyEvent => Some(match held {
                    Some(base) => base + input::MOUSE_MOTION + mods,
                    None => input::MOUSE_NO_BUTTON_MOTION + mods,
                }),
                MouseTracking::Off | MouseTracking::Normal => None,
            };
            if let Some(code) = code {
                let (col, row) = self.position_to_grid_clamped(position);
                if self.last_reported_cell != Some((col, row)) {
                    self.last_reported_cell = Some((col, row));
                    self.report_mouse(code, col, row, true);
                }
            }
            return;
        }

        if !self.mouse_pressed {
            return;
        }

        // Convert physical position to grid coordinates
        if let Some((col, row)) = self.position_to_grid(position) {
            self.parser.terminal_mut().set_selection_end(col, row);
            self.window.request_redraw();
        }
    }

    /// Handle mouse release: reported to the app while tracking is active,
    /// otherwise ends the local selection drag.
    pub fn mouse_release(&mut self, button: MouseButton, modifiers: ModifiersState) {
        if self.mouse_reporting_active(modifiers) {
            if let (Some(base), Some((col, row))) =
                (input::mouse_button_base(button), self.report_cell())
            {
                let code = base + input::mouse_modifiers(modifiers);
                self.report_mouse(code, col, row, false);
            }
            return;
        }

        if button == MouseButton::Left {
            self.mouse_pressed = false;
        }
    }

    /// Handle mouse wheel: reported as buttons 64/65 at the cursor cell
    /// while tracking is active, otherwise scrolls the viewport.
    pub fn mouse_wheel(&mut self, delta: f32, modifiers: ModifiersState) {
        if self.mouse_reporting_active(modifiers) {
            if delta != 0.0 {
                if let Some((col, row)) = self.report_cell() {
                    let base = if delta > 0.0 {
                        input::MOUSE_WHEEL_UP
                    } else {
                        input::MOUSE_WHEEL_DOWN
                    };
                    let code = base + input::mouse_modifiers(modifiers);
                    self.report_mouse(code, col, row, true);
                }
            }
            return;
        }

        // Positive delta = scroll up (show older content)
        // Negative delta = scroll down (show newer content)
        let lines = (delta.abs() / 10.0).ceil().max(1.0) as usize;

        if delta > 0.0 {
            self.parser.terminal_mut().scroll_viewport_up(lines);
            self.window.request_redraw();
        } else if delta < 0.0 {
            self.parser.terminal_mut().scroll_viewport_down(lines);
            self.window.request_redraw();
        }
    }

    /// Convert physical pixel position to grid coordinates (col, row).
    /// Uses the cached physical cell metrics only.
    fn position_to_grid(&self, position: PhysicalPosition<f64>) -> Option<(usize, usize)> {
        if self.cell_width <= 0.0 || self.cell_height <= 0.0 {
            return None;
        }

        // Adjust for padding
        let x = position.x as f32 - self.padding;
        let y = position.y as f32 - self.padding;

        if x < 0.0 || y < 0.0 {
            return None;
        }

        let col = (x / self.cell_width) as usize;
        let row = (y / self.cell_height) as usize;

        let terminal = self.parser.terminal();
        if col >= terminal.cols() || row >= terminal.rows() {
            return None;
        }

        Some((col, row))
    }

    /// Like `position_to_grid`, but clamps into the grid instead of
    /// returning `None`: positions in the padding or past the last cell
    /// map to the nearest edge cell. Used for mouse reporting, where an
    /// edge event must still be reported.
    fn position_to_grid_clamped(&self, position: PhysicalPosition<f64>) -> (usize, usize) {
        let scale = self.window.scale_factor() as f32;
        let padding = self.config.padding * scale;

        let mut font_manager = crate::font::FontManager::new(self.config.font_size * scale);
        let cell_width = font_manager.cell_width();
        let cell_height = font_manager.cell_height();

        let x = position.x as f32 - padding;
        let y = position.y as f32 - padding;

        let terminal = self.parser.terminal();
        let max_col = terminal.cols().saturating_sub(1);
        let max_row = terminal.rows().saturating_sub(1);

        let col = if x <= 0.0 {
            0
        } else {
            (x / cell_width) as usize
        }
        .min(max_col);
        let row = if y <= 0.0 {
            0
        } else {
            (y / cell_height) as usize
        }
        .min(max_row);

        (col, row)
    }
}
