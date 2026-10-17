use portable_pty::{Child, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

type WakerCell = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;

pub struct PtyManager {
    writer: Box<dyn Write + Send>,
    receiver: Receiver<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send>,
    /// Shared waker invoked by the reader thread after each successful
    /// send, so the event loop can sleep until output exists. Installed
    /// via [`PtyManager::set_waker`] right after construction.
    waker: WakerCell,
}

impl PtyManager {
    // Retained as the no-override constructor; the window uses `with_shell`.
    #[allow(dead_code)]
    pub fn new(cols: u16, rows: u16) -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_shell(cols, rows, None)
    }

    /// Spawn `shell` if given, else `$SHELL` (then `/bin/bash`).
    pub fn with_shell(
        cols: u16,
        rows: u16,
        shell: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Configured shell, else environment, else default
        let shell = shell
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var("SHELL").ok().filter(|s| !s.is_empty()))
            .unwrap_or_else(|| "/bin/bash".to_string());
        log::debug!("Spawning shell: {}", shell);

        let cmd = CommandBuilder::new(&shell);
        Self::spawn_with_command(cols, rows, cmd)
    }

    /// Spawn an arbitrary command in a new PTY. Shared by `with_shell` (user
    /// shell) and tests (custom commands). Sets the terminal environment
    /// (`TERM`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`) on every command
    /// spawned through here, and retains the child so `has_exited` can poll it.
    pub(crate) fn spawn_with_command(
        cols: u16,
        rows: u16,
        mut cmd: CommandBuilder,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let pty_system = NativePtySystem::default();

        let pty_size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system.openpty(pty_size)?;

        cmd.env("TERM", "xterm-256color");
        cmd.env("TERM_PROGRAM", "Wisp");
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));

        let child = pair.slave.spawn_command(cmd)?;
        log::debug!("Spawned PTY child (PID: {:?})", child.process_id());

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let master = pair.master;

        let (sender, receiver) = channel();

        let waker: WakerCell = Arc::new(Mutex::new(None));
        let reader_waker = waker.clone();

        // Spawn thread to read from PTY
        thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(n) if n > 0 => {
                        if sender.send(buffer[..n].to_vec()).is_err() {
                            break;
                        }
                        // Wake the event loop; clone out from under the
                        // lock so the callback never runs while holding it.
                        let waker_fn = reader_waker.lock().ok().and_then(|guard| guard.clone());
                        if let Some(waker_fn) = waker_fn {
                            waker_fn();
                        }
                    }
                    Ok(_) => {
                        // EOF
                        break;
                    }
                    Err(e) => {
                        log::warn!("PTY read error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            writer,
            receiver,
            master,
            child,
            waker,
        })
    }

    /// Non-blocking check: has the spawned child process terminated?
    ///
    /// Based solely on `try_wait()`; reader-thread EOF does not count as
    /// exit. Wait errors are logged and treated as "not exited" so a
    /// transient error cannot close a live window.
    pub fn has_exited(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                log::warn!("PTY child try_wait error: {}", e);
                false
            }
        }
    }

    /// Install the waker the reader thread calls after each successful
    /// send. Output produced before this is called stays queued in the
    /// channel and is picked up on the next wake/redraw.
    pub fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut guard) = self.waker.lock() {
            *guard = Some(waker);
        }
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), std::io::Error> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn try_read(&mut self) -> Option<Vec<u8>> {
        self.receiver.try_recv().ok()
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let pty_size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let _ = self.master.resize(pty_size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn spawn_sh(script: &str) -> PtyManager {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.arg("-c");
        cmd.arg(script);
        PtyManager::spawn_with_command(80, 24, cmd).expect("spawn /bin/sh in PTY")
    }

    #[test]
    fn exited_child_is_reported_within_deadline() {
        let mut pty = spawn_sh("exit 0");

        let deadline = Instant::now() + Duration::from_secs(2);
        while !pty.has_exited() {
            assert!(
                Instant::now() < deadline,
                "child did not report exit within 2s"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        // Repeated polls after exit keep reporting true.
        assert!(pty.has_exited());
    }

    #[test]
    fn spawned_child_sees_term_program_env() {
        let mut pty = spawn_sh(
            "printf '%s|%s|%s' \"$TERM_PROGRAM\" \"$TERM_PROGRAM_VERSION\" \"$TERM\"; exit 0",
        );

        let mut output = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            while let Some(chunk) = pty.try_read() {
                output.extend_from_slice(&chunk);
            }
            if pty.has_exited() && String::from_utf8_lossy(&output).contains("xterm-256color") {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        while let Some(chunk) = pty.try_read() {
            output.extend_from_slice(&chunk);
        }

        let text = String::from_utf8_lossy(&output);
        let expected = format!("Wisp|{}|xterm-256color", env!("CARGO_PKG_VERSION"));
        assert!(text.contains(&expected), "got PTY output: {text:?}");
        assert!(pty.has_exited());
    }

    #[test]
    fn running_child_is_not_reported_exited() {
        let mut pty = spawn_sh("sleep 5");

        assert!(!pty.has_exited(), "sleeping child must not count as exited");

        // Clean up so no process outlives the test.
        let _ = pty.child.kill();
    }
}
