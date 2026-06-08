use winit::keyboard::{KeyCode, PhysicalKey};

pub fn key_to_bytes(key: PhysicalKey, text: &str) -> Option<Vec<u8>> {
    match key {
        PhysicalKey::Code(code) => match code {
            KeyCode::Enter => Some(b"\r".to_vec()),
            KeyCode::Backspace => Some(b"\x7f".to_vec()),
            KeyCode::Tab => Some(b"\t".to_vec()),
            KeyCode::Escape => Some(b"\x1b".to_vec()),
            KeyCode::ArrowUp => Some(b"\x1b[A".to_vec()),
            KeyCode::ArrowDown => Some(b"\x1b[B".to_vec()),
            KeyCode::ArrowRight => Some(b"\x1b[C".to_vec()),
            KeyCode::ArrowLeft => Some(b"\x1b[D".to_vec()),
            KeyCode::Home => Some(b"\x1b[H".to_vec()),
            KeyCode::End => Some(b"\x1b[F".to_vec()),
            KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
            KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
            KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
            _ => {
                if !text.is_empty() {
                    Some(text.as_bytes().to_vec())
                } else {
                    None
                }
            }
        },
        _ => {
            if !text.is_empty() {
                Some(text.as_bytes().to_vec())
            } else {
                None
            }
        }
    }
}
