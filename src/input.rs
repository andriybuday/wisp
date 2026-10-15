use winit::keyboard::{Key, KeyCode, PhysicalKey};

pub fn key_to_bytes(key: PhysicalKey, text: &str, logical_key: &Key) -> Option<Vec<u8>> {
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
                // Try text first, then fall back to logical_key character
                if !text.is_empty() {
                    Some(text.as_bytes().to_vec())
                } else if let Key::Character(ch) = logical_key {
                    Some(ch.as_bytes().to_vec())
                } else {
                    None
                }
            }
        },
        _ => {
            if !text.is_empty() {
                Some(text.as_bytes().to_vec())
            } else if let Key::Character(ch) = logical_key {
                Some(ch.as_bytes().to_vec())
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{NamedKey, NativeKeyCode};

    fn key(code: KeyCode) -> Option<Vec<u8>> {
        key_to_bytes(PhysicalKey::Code(code), "", &Key::Named(NamedKey::F1))
    }

    #[test]
    fn editing_keys() {
        assert_eq!(key(KeyCode::Enter).unwrap(), b"\r");
        assert_eq!(key(KeyCode::Backspace).unwrap(), b"\x7f");
        assert_eq!(key(KeyCode::Tab).unwrap(), b"\t");
        assert_eq!(key(KeyCode::Escape).unwrap(), b"\x1b");
        assert_eq!(key(KeyCode::Delete).unwrap(), b"\x1b[3~");
    }

    #[test]
    fn arrow_keys() {
        assert_eq!(key(KeyCode::ArrowUp).unwrap(), b"\x1b[A");
        assert_eq!(key(KeyCode::ArrowDown).unwrap(), b"\x1b[B");
        assert_eq!(key(KeyCode::ArrowRight).unwrap(), b"\x1b[C");
        assert_eq!(key(KeyCode::ArrowLeft).unwrap(), b"\x1b[D");
    }

    #[test]
    fn navigation_keys() {
        assert_eq!(key(KeyCode::Home).unwrap(), b"\x1b[H");
        assert_eq!(key(KeyCode::End).unwrap(), b"\x1b[F");
        assert_eq!(key(KeyCode::PageUp).unwrap(), b"\x1b[5~");
        assert_eq!(key(KeyCode::PageDown).unwrap(), b"\x1b[6~");
    }

    #[test]
    fn text_fallback_and_precedence() {
        let logical = Key::Character("z".into());
        let out = key_to_bytes(PhysicalKey::Code(KeyCode::KeyA), "a", &logical);
        assert_eq!(out.unwrap(), b"a");
    }

    #[test]
    fn logical_character_fallback() {
        let logical = Key::Character("q".into());
        let out = key_to_bytes(PhysicalKey::Code(KeyCode::KeyQ), "", &logical);
        assert_eq!(out.unwrap(), b"q");
    }

    #[test]
    fn unmapped_named_key_is_none() {
        assert_eq!(key(KeyCode::F1), None);
    }

    #[test]
    fn unidentified_physical_uses_text_then_character() {
        let phys = PhysicalKey::Unidentified(NativeKeyCode::Unidentified);
        let logical = Key::Character("m".into());
        assert_eq!(key_to_bytes(phys, "hi", &logical).unwrap(), b"hi");
        assert_eq!(key_to_bytes(phys, "", &logical).unwrap(), b"m");
        assert_eq!(key_to_bytes(phys, "", &Key::Named(NamedKey::Shift)), None);
    }
}
