use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

type WakerCell = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;

pub struct PtyManager {
    writer: Box<dyn Write + Send>,
    receiver: Receiver<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
    /// Shared waker invoked by the reader thread after each successful
    /// send, so the event loop can sleep until output exists. Installed
    /// via [`PtyManager::set_waker`] right after construction.
    waker: WakerCell,
}

impl PtyManager {
    pub fn new(cols: u16, rows: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let pty_system = NativePtySystem::default();

        let pty_size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system.openpty(pty_size)?;

        // Get shell from environment or use default
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");

        let child = pair.slave.spawn_command(cmd)?;
        log::debug!("Spawned shell: {} (PID: {:?})", shell, child.process_id());

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
            waker,
        })
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
