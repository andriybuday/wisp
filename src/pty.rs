use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

pub struct PtyManager {
    writer: Box<dyn Write + Send>,
    receiver: Receiver<Vec<u8>>,
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
        println!("Spawned shell: {} (PID: {:?})", shell, child.process_id());
        
        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        
        let (sender, receiver) = channel();
        
        // Spawn thread to read from PTY
        thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(n) if n > 0 => {
                        if sender.send(buffer[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {
                        // EOF
                        break;
                    }
                    Err(e) => {
                        eprintln!("PTY read error: {}", e);
                        break;
                    }
                }
            }
        });
        
        Ok(Self { writer, receiver })
    }
    
    pub fn write(&mut self, data: &[u8]) -> Result<(), std::io::Error> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }
    
    pub fn try_read(&mut self) -> Option<Vec<u8>> {
        self.receiver.try_recv().ok()
    }
    
    pub fn resize(&mut self, _cols: u16, _rows: u16) {
        // TODO: Implement PTY resize
    }
}
