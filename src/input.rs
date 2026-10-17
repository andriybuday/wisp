use winit::keyboard::{Key, KeyCode, ModifiersState, PhysicalKey};

fn alt_prefixed(base: &[u8], alt: bool) -> Vec<u8> {
    if alt {
        let mut out = vec![0x1b];
        out.extend_from_slice(base);
        out
    } else {
        base.to_vec()
    }
}

/// `CSI 1;{m}{final}` when modified, plain `CSI {final}` otherwise.
fn csi_1(final_byte: u8, m: u8) -> Vec<u8> {
    if m > 1 {
        format!("\x1b[1;{}{}", m, final_byte as char).into_bytes()
    } else {
        vec![0x1b, b'[', final_byte]
    }
}

/// `CSI {n};{m}~` when modified, plain `CSI {n}~` otherwise.
fn csi_tilde(n: u8, m: u8) -> Vec<u8> {
    if m > 1 {
        format!("\x1b[{};{}~", n, m).into_bytes()
    } else {
        format!("\x1b[{}~", n).into_bytes()
    }
}

/// Control byte for a Ctrl+character combination, per the traditional table.
fn ctrl_byte(c: char) -> Option<u8> {
    match c {
        'a'..='z' => Some(c as u8 - b'a' + 1),
        'A'..='Z' => Some(c as u8 - b'A' + 1),
        ' ' | '2' => Some(0),
        '[' | '3' => Some(27),
        '\\' | '4' => Some(28),
        ']' | '5' => Some(29),
        '^' | '6' => Some(30),
        '_' | '7' => Some(31),
        '?' | '8' => Some(127),
        _ => None,
    }
}

/// Unshifted character for physical keys that can appear in Ctrl combos, used
/// only as a fallback when the platform gives no usable text/logical char.
fn physical_char(code: KeyCode) -> Option<char> {
    Some(match code {
        KeyCode::KeyA => 'a',
        KeyCode::KeyB => 'b',
        KeyCode::KeyC => 'c',
        KeyCode::KeyD => 'd',
        KeyCode::KeyE => 'e',
        KeyCode::KeyF => 'f',
        KeyCode::KeyG => 'g',
        KeyCode::KeyH => 'h',
        KeyCode::KeyI => 'i',
        KeyCode::KeyJ => 'j',
        KeyCode::KeyK => 'k',
        KeyCode::KeyL => 'l',
        KeyCode::KeyM => 'm',
        KeyCode::KeyN => 'n',
        KeyCode::KeyO => 'o',
        KeyCode::KeyP => 'p',
        KeyCode::KeyQ => 'q',
        KeyCode::KeyR => 'r',
        KeyCode::KeyS => 's',
        KeyCode::KeyT => 't',
        KeyCode::KeyU => 'u',
        KeyCode::KeyV => 'v',
        KeyCode::KeyW => 'w',
        KeyCode::KeyX => 'x',
        KeyCode::KeyY => 'y',
        KeyCode::KeyZ => 'z',
        KeyCode::Digit0 => '0',
        KeyCode::Digit1 => '1',
        KeyCode::Digit2 => '2',
        KeyCode::Digit3 => '3',
        KeyCode::Digit4 => '4',
        KeyCode::Digit5 => '5',
        KeyCode::Digit6 => '6',
        KeyCode::Digit7 => '7',
        KeyCode::Digit8 => '8',
        KeyCode::Digit9 => '9',
        KeyCode::Space => ' ',
        KeyCode::BracketLeft => '[',
        KeyCode::BracketRight => ']',
        KeyCode::Backslash => '\\',
        _ => return None,
    })
}

pub fn key_to_bytes(
    key: PhysicalKey,
    text: &str,
    logical_key: &Key,
    modifiers: ModifiersState,
) -> Option<Vec<u8>> {
    // Cmd/Super combinations belong to the OS/app, never the shell.
    if modifiers.super_key() {
        return None;
    }
    let shift = modifiers.shift_key();
    let alt = modifiers.alt_key();
    let ctrl = modifiers.control_key();
    let m: u8 = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl);

    if let PhysicalKey::Code(code) = key {
        match code {
            KeyCode::Enter => return Some(alt_prefixed(b"\r", alt && !ctrl)),
            KeyCode::Escape => return Some(alt_prefixed(b"\x1b", alt && !ctrl)),
            KeyCode::Backspace => {
                if ctrl {
                    return Some(vec![127]);
                }
                return Some(alt_prefixed(b"\x7f", alt));
            }
            KeyCode::Tab => {
                if shift {
                    return Some(b"\x1b[Z".to_vec());
                }
                return Some(alt_prefixed(b"\t", alt && !ctrl));
            }
            KeyCode::ArrowUp => return Some(csi_1(b'A', m)),
            KeyCode::ArrowDown => return Some(csi_1(b'B', m)),
            KeyCode::ArrowRight => return Some(csi_1(b'C', m)),
            KeyCode::ArrowLeft => return Some(csi_1(b'D', m)),
            KeyCode::Home => return Some(csi_1(b'H', m)),
            KeyCode::End => return Some(csi_1(b'F', m)),
            KeyCode::Insert => return Some(csi_tilde(2, m)),
            KeyCode::Delete => return Some(csi_tilde(3, m)),
            KeyCode::PageUp => return Some(csi_tilde(5, m)),
            KeyCode::PageDown => return Some(csi_tilde(6, m)),
            KeyCode::F1 => return Some(f1_f4(b'P', m)),
            KeyCode::F2 => return Some(f1_f4(b'Q', m)),
            KeyCode::F3 => return Some(f1_f4(b'R', m)),
            KeyCode::F4 => return Some(f1_f4(b'S', m)),
            KeyCode::F5 => return Some(csi_tilde(15, m)),
            KeyCode::F6 => return Some(csi_tilde(17, m)),
            KeyCode::F7 => return Some(csi_tilde(18, m)),
            KeyCode::F8 => return Some(csi_tilde(19, m)),
            KeyCode::F9 => return Some(csi_tilde(20, m)),
            KeyCode::F10 => return Some(csi_tilde(21, m)),
            KeyCode::F11 => return Some(csi_tilde(23, m)),
            KeyCode::F12 => return Some(csi_tilde(24, m)),
            _ => {}
        }
    }

    // Ctrl combos win over the plain text path.
    if ctrl {
        let logical_char = match logical_key {
            Key::Character(ch) => ch.chars().next(),
            _ => None,
        };
        let phys_char = match key {
            PhysicalKey::Code(code) => physical_char(code),
            _ => None,
        };
        for candidate in [text.chars().next(), logical_char, phys_char]
            .into_iter()
            .flatten()
        {
            if let Some(byte) = ctrl_byte(candidate) {
                return Some(vec![byte]);
            }
        }
    }

    // Try text first, then fall back to logical_key character.
    let plain = if !text.is_empty() {
        Some(text.as_bytes().to_vec())
    } else if let Key::Character(ch) = logical_key {
        Some(ch.as_bytes().to_vec())
    } else {
        None
    };
    match plain {
        Some(bytes) if alt && !ctrl => Some(alt_prefixed(&bytes, true)),
        other => other,
    }
}

/// xterm mouse button codes (before modifier/motion bits are added).
pub const MOUSE_LEFT: u32 = 0;
pub const MOUSE_MIDDLE: u32 = 1;
pub const MOUSE_RIGHT: u32 = 2;
pub const MOUSE_WHEEL_UP: u32 = 64;
pub const MOUSE_WHEEL_DOWN: u32 = 65;
/// Added to a button code to mark a motion event.
pub const MOUSE_MOTION: u32 = 32;
/// Motion with no button held (any-event tracking, ?1003): motion + button 3.
pub const MOUSE_NO_BUTTON_MOTION: u32 = 35;

/// Base button code for a winit mouse button; `None` for buttons xterm
/// mouse reporting does not encode.
pub fn mouse_button_base(button: winit::event::MouseButton) -> Option<u32> {
    use winit::event::MouseButton;
    match button {
        MouseButton::Left => Some(MOUSE_LEFT),
        MouseButton::Middle => Some(MOUSE_MIDDLE),
        MouseButton::Right => Some(MOUSE_RIGHT),
        _ => None,
    }
}

/// Modifier bits added to a mouse button code: Shift 4, Alt 8, Ctrl 16.
pub fn mouse_modifiers(modifiers: ModifiersState) -> u32 {
    let mut bits = 0;
    if modifiers.shift_key() {
        bits += 4;
    }
    if modifiers.alt_key() {
        bits += 8;
    }
    if modifiers.control_key() {
        bits += 16;
    }
    bits
}

/// Encode one xterm mouse report for cell (`col`, `row`) (0-based).
///
/// SGR form (?1006): `ESC [ < {button} ; {col+1} ; {row+1} M` for a press,
/// final `m` for a release. Legacy form: `ESC [ M` followed by the button,
/// column, and row as single bytes, each `value + 32` clamped to 255; a
/// release cannot identify the button, so its low button bits become 3
/// (modifier and motion bits are kept).
pub fn encode_mouse(button: u32, col: usize, row: usize, pressed: bool, sgr: bool) -> Vec<u8> {
    if sgr {
        let final_byte = if pressed { 'M' } else { 'm' };
        format!(
            "\x1b[<{};{};{}{}",
            button,
            col.saturating_add(1),
            row.saturating_add(1),
            final_byte
        )
        .into_bytes()
    } else {
        let effective = if pressed {
            button
        } else {
            (button & !0b11) | 3
        };
        let enc = |v: u32| v.saturating_add(32).min(255) as u8;
        let cx = u32::try_from(col.saturating_add(1)).unwrap_or(u32::MAX);
        let cy = u32::try_from(row.saturating_add(1)).unwrap_or(u32::MAX);
        vec![0x1b, b'[', b'M', enc(effective), enc(cx), enc(cy)]
    }
}

/// F1-F4: SS3 form unmodified, `CSI 1;{m}` form modified.
fn f1_f4(final_byte: u8, m: u8) -> Vec<u8> {
    if m > 1 {
        format!("\x1b[1;{}{}", m, final_byte as char).into_bytes()
    } else {
        vec![0x1b, b'O', final_byte]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{NamedKey, NativeKeyCode};

    fn key(code: KeyCode) -> Option<Vec<u8>> {
        key_to_bytes(
            PhysicalKey::Code(code),
            "",
            &Key::Named(NamedKey::F1),
            ModifiersState::empty(),
        )
    }

    fn key_mod(code: KeyCode, mods: ModifiersState) -> Option<Vec<u8>> {
        key_to_bytes(PhysicalKey::Code(code), "", &Key::Named(NamedKey::F1), mods)
    }

    fn text_key(code: KeyCode, text: &str, mods: ModifiersState) -> Option<Vec<u8>> {
        key_to_bytes(
            PhysicalKey::Code(code),
            text,
            &Key::Character(text.into()),
            mods,
        )
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
        let out = key_to_bytes(
            PhysicalKey::Code(KeyCode::KeyA),
            "a",
            &logical,
            ModifiersState::empty(),
        );
        assert_eq!(out.unwrap(), b"a");
    }

    #[test]
    fn logical_character_fallback() {
        let logical = Key::Character("q".into());
        let out = key_to_bytes(
            PhysicalKey::Code(KeyCode::KeyQ),
            "",
            &logical,
            ModifiersState::empty(),
        );
        assert_eq!(out.unwrap(), b"q");
    }

    #[test]
    fn unmapped_named_key_is_none() {
        assert_eq!(key(KeyCode::CapsLock), None);
    }

    #[test]
    fn unidentified_physical_uses_text_then_character() {
        let phys = PhysicalKey::Unidentified(NativeKeyCode::Unidentified);
        let logical = Key::Character("m".into());
        assert_eq!(
            key_to_bytes(phys, "hi", &logical, ModifiersState::empty()).unwrap(),
            b"hi"
        );
        assert_eq!(
            key_to_bytes(phys, "", &logical, ModifiersState::empty()).unwrap(),
            b"m"
        );
        assert_eq!(
            key_to_bytes(
                phys,
                "",
                &Key::Named(NamedKey::Shift),
                ModifiersState::empty()
            ),
            None
        );
    }

    #[test]
    fn ctrl_letters_upper_and_lower() {
        let ctrl = ModifiersState::CONTROL;
        assert_eq!(text_key(KeyCode::KeyC, "c", ctrl).unwrap(), [3]);
        assert_eq!(text_key(KeyCode::KeyC, "C", ctrl).unwrap(), [3]);
        assert_eq!(text_key(KeyCode::KeyA, "a", ctrl).unwrap(), [1]);
        assert_eq!(text_key(KeyCode::KeyZ, "Z", ctrl).unwrap(), [26]);
        // Logical-key-only path (no text under Ctrl).
        let out = key_to_bytes(
            PhysicalKey::Code(KeyCode::KeyD),
            "",
            &Key::Character("d".into()),
            ctrl,
        );
        assert_eq!(out.unwrap(), [4]);
        // Ctrl wins over plain text: the letter itself never leaks.
        assert_ne!(text_key(KeyCode::KeyL, "l", ctrl).unwrap(), b"l".to_vec());
    }

    #[test]
    fn ctrl_space_and_ctrl_bracket_right() {
        let ctrl = ModifiersState::CONTROL;
        assert_eq!(text_key(KeyCode::Space, " ", ctrl).unwrap(), [0]);
        assert_eq!(text_key(KeyCode::BracketRight, "]", ctrl).unwrap(), [29]);
    }

    #[test]
    fn ctrl_punctuation_and_digits() {
        let ctrl = ModifiersState::CONTROL;
        assert_eq!(text_key(KeyCode::Digit2, "2", ctrl).unwrap(), [0]);
        assert_eq!(text_key(KeyCode::BracketLeft, "[", ctrl).unwrap(), [27]);
        assert_eq!(text_key(KeyCode::Digit3, "3", ctrl).unwrap(), [27]);
        assert_eq!(text_key(KeyCode::Backslash, "\\", ctrl).unwrap(), [28]);
        assert_eq!(text_key(KeyCode::Digit4, "4", ctrl).unwrap(), [28]);
        assert_eq!(text_key(KeyCode::Digit5, "5", ctrl).unwrap(), [29]);
        assert_eq!(text_key(KeyCode::Digit6, "^", ctrl).unwrap(), [30]);
        assert_eq!(text_key(KeyCode::Digit6, "6", ctrl).unwrap(), [30]);
        assert_eq!(text_key(KeyCode::Minus, "_", ctrl).unwrap(), [31]);
        assert_eq!(text_key(KeyCode::Digit7, "7", ctrl).unwrap(), [31]);
        assert_eq!(text_key(KeyCode::Slash, "?", ctrl).unwrap(), [127]);
        assert_eq!(text_key(KeyCode::Digit8, "8", ctrl).unwrap(), [127]);
    }

    #[test]
    fn ctrl_backspace_and_plain_backspace() {
        assert_eq!(
            key_mod(KeyCode::Backspace, ModifiersState::CONTROL).unwrap(),
            [127]
        );
        assert_eq!(key(KeyCode::Backspace).unwrap(), b"\x7f");
    }

    #[test]
    fn alt_char_and_alt_enter_escape() {
        let alt = ModifiersState::ALT;
        assert_eq!(text_key(KeyCode::KeyX, "x", alt).unwrap(), b"\x1bx");
        assert_eq!(key_mod(KeyCode::Enter, alt).unwrap(), b"\x1b\r");
        assert_eq!(key_mod(KeyCode::Escape, alt).unwrap(), b"\x1b\x1b");
    }

    #[test]
    fn function_keys_unmodified() {
        assert_eq!(key(KeyCode::F1).unwrap(), b"\x1bOP");
        assert_eq!(key(KeyCode::F2).unwrap(), b"\x1bOQ");
        assert_eq!(key(KeyCode::F3).unwrap(), b"\x1bOR");
        assert_eq!(key(KeyCode::F4).unwrap(), b"\x1bOS");
        assert_eq!(key(KeyCode::F5).unwrap(), b"\x1b[15~");
        assert_eq!(key(KeyCode::F6).unwrap(), b"\x1b[17~");
        assert_eq!(key(KeyCode::F7).unwrap(), b"\x1b[18~");
        assert_eq!(key(KeyCode::F8).unwrap(), b"\x1b[19~");
        assert_eq!(key(KeyCode::F9).unwrap(), b"\x1b[20~");
        assert_eq!(key(KeyCode::F10).unwrap(), b"\x1b[21~");
        assert_eq!(key(KeyCode::F11).unwrap(), b"\x1b[23~");
        assert_eq!(key(KeyCode::F12).unwrap(), b"\x1b[24~");
    }

    #[test]
    fn function_keys_modified() {
        assert_eq!(
            key_mod(KeyCode::F1, ModifiersState::CONTROL).unwrap(),
            b"\x1b[1;5P"
        );
        assert_eq!(
            key_mod(KeyCode::F4, ModifiersState::SHIFT).unwrap(),
            b"\x1b[1;2S"
        );
        assert_eq!(
            key_mod(KeyCode::F5, ModifiersState::ALT).unwrap(),
            b"\x1b[15;3~"
        );
        assert_eq!(
            key_mod(KeyCode::F12, ModifiersState::CONTROL).unwrap(),
            b"\x1b[24;5~"
        );
    }

    #[test]
    fn modified_arrows_and_navigation() {
        // Ctrl+Shift+Right: m = 1 + 1 (shift) + 4 (ctrl) = 6.
        let cs = ModifiersState::CONTROL | ModifiersState::SHIFT;
        assert_eq!(key_mod(KeyCode::ArrowRight, cs).unwrap(), b"\x1b[1;6C");
        assert_eq!(
            key_mod(KeyCode::ArrowUp, ModifiersState::ALT).unwrap(),
            b"\x1b[1;3A"
        );
        assert_eq!(
            key_mod(KeyCode::Home, ModifiersState::SHIFT).unwrap(),
            b"\x1b[1;2H"
        );
        assert_eq!(
            key_mod(KeyCode::End, ModifiersState::CONTROL).unwrap(),
            b"\x1b[1;5F"
        );
        assert_eq!(
            key_mod(KeyCode::Delete, ModifiersState::CONTROL).unwrap(),
            b"\x1b[3;5~"
        );
        assert_eq!(
            key_mod(KeyCode::PageUp, ModifiersState::SHIFT).unwrap(),
            b"\x1b[5;2~"
        );
        assert_eq!(
            key_mod(KeyCode::PageDown, ModifiersState::ALT).unwrap(),
            b"\x1b[6;3~"
        );
    }

    #[test]
    fn insert_plain_and_modified() {
        assert_eq!(key(KeyCode::Insert).unwrap(), b"\x1b[2~");
        assert_eq!(
            key_mod(KeyCode::Insert, ModifiersState::CONTROL).unwrap(),
            b"\x1b[2;5~"
        );
    }

    #[test]
    fn shift_tab_is_backtab() {
        assert_eq!(
            key_mod(KeyCode::Tab, ModifiersState::SHIFT).unwrap(),
            b"\x1b[Z"
        );
        assert_eq!(key(KeyCode::Tab).unwrap(), b"\t");
    }

    #[test]
    fn super_returns_none() {
        let sup = ModifiersState::SUPER;
        assert_eq!(text_key(KeyCode::KeyC, "c", sup), None);
        assert_eq!(key_mod(KeyCode::ArrowUp, sup), None);
        assert_eq!(key_mod(KeyCode::Enter, sup), None);
        assert_eq!(key_mod(KeyCode::F1, sup | ModifiersState::SHIFT), None);
    }

    #[test]
    fn mouse_sgr_press_and_release() {
        // Left press at (0,0) reports 1-based cell 1;1 with final 'M'.
        assert_eq!(encode_mouse(MOUSE_LEFT, 0, 0, true, true), b"\x1b[<0;1;1M");
        // Release keeps the button code and uses final 'm'.
        assert_eq!(encode_mouse(MOUSE_LEFT, 0, 0, false, true), b"\x1b[<0;1;1m");
        // Right button at a non-origin cell.
        assert_eq!(
            encode_mouse(MOUSE_RIGHT, 9, 4, true, true),
            b"\x1b[<2;10;5M"
        );
        assert_eq!(
            encode_mouse(MOUSE_MIDDLE, 9, 4, false, true),
            b"\x1b[<1;10;5m"
        );
    }

    #[test]
    fn mouse_legacy_press_and_release() {
        // Bytes are value+32: button 0 -> 32, col/row 1-based 1 -> 33.
        assert_eq!(
            encode_mouse(MOUSE_LEFT, 0, 0, true, false),
            vec![0x1b, b'[', b'M', 32, 33, 33]
        );
        // Legacy release uses button 3 regardless of which button.
        assert_eq!(
            encode_mouse(MOUSE_LEFT, 0, 0, false, false),
            vec![0x1b, b'[', b'M', 35, 33, 33]
        );
        assert_eq!(
            encode_mouse(MOUSE_RIGHT, 4, 2, false, false),
            vec![0x1b, b'[', b'M', 35, 37, 35]
        );
        assert_eq!(
            encode_mouse(MOUSE_RIGHT, 4, 2, true, false),
            vec![0x1b, b'[', b'M', 34, 37, 35]
        );
    }

    #[test]
    fn mouse_motion_adds_32_and_no_button_is_35() {
        let drag = MOUSE_LEFT + MOUSE_MOTION;
        assert_eq!(encode_mouse(drag, 2, 3, true, true), b"\x1b[<32;3;4M");
        assert_eq!(
            encode_mouse(drag, 2, 3, true, false),
            vec![0x1b, b'[', b'M', 64, 35, 36]
        );
        assert_eq!(MOUSE_NO_BUTTON_MOTION, 35);
        assert_eq!(
            encode_mouse(MOUSE_NO_BUTTON_MOTION, 0, 0, true, true),
            b"\x1b[<35;1;1M"
        );
    }

    #[test]
    fn mouse_wheel_buttons_64_and_65() {
        assert_eq!(
            encode_mouse(MOUSE_WHEEL_UP, 5, 5, true, true),
            b"\x1b[<64;6;6M"
        );
        assert_eq!(
            encode_mouse(MOUSE_WHEEL_DOWN, 5, 5, true, true),
            b"\x1b[<65;6;6M"
        );
        assert_eq!(
            encode_mouse(MOUSE_WHEEL_UP, 0, 0, true, false),
            vec![0x1b, b'[', b'M', 96, 33, 33]
        );
    }

    #[test]
    fn mouse_modifier_bits() {
        assert_eq!(mouse_modifiers(ModifiersState::SHIFT), 4);
        assert_eq!(mouse_modifiers(ModifiersState::ALT), 8);
        assert_eq!(mouse_modifiers(ModifiersState::CONTROL), 16);
        assert_eq!(
            mouse_modifiers(ModifiersState::SHIFT | ModifiersState::ALT | ModifiersState::CONTROL),
            28
        );
        assert_eq!(mouse_modifiers(ModifiersState::empty()), 0);
        // Ctrl+left press: 16 + 0 = 16 in both encodings.
        let code = MOUSE_LEFT + mouse_modifiers(ModifiersState::CONTROL);
        assert_eq!(encode_mouse(code, 0, 0, true, true), b"\x1b[<16;1;1M");
        assert_eq!(
            encode_mouse(code, 0, 0, true, false),
            vec![0x1b, b'[', b'M', 48, 33, 33]
        );
        // Legacy release keeps modifier bits but reports button 3: 16|3=19 -> 51.
        assert_eq!(
            encode_mouse(code, 0, 0, false, false),
            vec![0x1b, b'[', b'M', 51, 33, 33]
        );
    }

    #[test]
    fn mouse_legacy_coordinates_clamp_at_255() {
        // col 300 -> 301+32 clamps to 255; SGR is not clamped.
        assert_eq!(
            encode_mouse(MOUSE_LEFT, 300, 1000, true, false),
            vec![0x1b, b'[', b'M', 32, 255, 255]
        );
        assert_eq!(
            encode_mouse(MOUSE_LEFT, 300, 1000, true, true),
            b"\x1b[<0;301;1001M"
        );
        // An oversized button value also clamps instead of overflowing.
        assert_eq!(
            encode_mouse(1000, 0, 0, true, false),
            vec![0x1b, b'[', b'M', 255, 33, 33]
        );
    }

    #[test]
    fn plain_keys_unchanged_with_shift_text() {
        // Shifted printable text passes through exactly as the platform reports it.
        assert_eq!(
            text_key(KeyCode::KeyA, "A", ModifiersState::SHIFT).unwrap(),
            b"A"
        );
        assert_eq!(
            text_key(KeyCode::Digit1, "!", ModifiersState::SHIFT).unwrap(),
            b"!"
        );
    }
}
