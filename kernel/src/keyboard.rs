//! The keyboard: scancodes of set 1, which the PS/2 controller translates to,
//! read as a French AZERTY keyboard. VirtualBox and a real PC send the
//! physical key; the layout is the human's own.

/// What a key press means to the desktop.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    Escape,
    Tab,
    /// F1 or the Windows key: the application menu.
    Menu,
}

/// (scancode, alone, with Shift, with AltGr); '\0' where the key gives nothing.
const AZERTY: [(u8, char, char, char); 49] = [
    (0x02, '&', '1', '\0'),
    (0x03, 'é', '2', '~'),
    (0x04, '"', '3', '#'),
    (0x05, '\'', '4', '{'),
    (0x06, '(', '5', '['),
    (0x07, '-', '6', '|'),
    (0x08, 'è', '7', '`'),
    (0x09, '_', '8', '\\'),
    (0x0A, 'ç', '9', '^'),
    (0x0B, 'à', '0', '@'),
    (0x0C, ')', '°', ']'),
    (0x0D, '=', '+', '}'),
    (0x10, 'a', 'A', '\0'),
    (0x11, 'z', 'Z', '\0'),
    (0x12, 'e', 'E', '\0'),
    (0x13, 'r', 'R', '\0'),
    (0x14, 't', 'T', '\0'),
    (0x15, 'y', 'Y', '\0'),
    (0x16, 'u', 'U', '\0'),
    (0x17, 'i', 'I', '\0'),
    (0x18, 'o', 'O', '\0'),
    (0x19, 'p', 'P', '\0'),
    (0x1A, '^', '¨', '\0'),
    (0x1B, '$', '£', '¤'),
    (0x1E, 'q', 'Q', '\0'),
    (0x1F, 's', 'S', '\0'),
    (0x20, 'd', 'D', '\0'),
    (0x21, 'f', 'F', '\0'),
    (0x22, 'g', 'G', '\0'),
    (0x23, 'h', 'H', '\0'),
    (0x24, 'j', 'J', '\0'),
    (0x25, 'k', 'K', '\0'),
    (0x26, 'l', 'L', '\0'),
    (0x27, 'm', 'M', '\0'),
    (0x28, 'ù', '%', '\0'),
    (0x29, '²', '\0', '\0'),
    (0x2B, '*', 'µ', '\0'),
    (0x2C, 'w', 'W', '\0'),
    (0x2D, 'x', 'X', '\0'),
    (0x2E, 'c', 'C', '\0'),
    (0x2F, 'v', 'V', '\0'),
    (0x30, 'b', 'B', '\0'),
    (0x31, 'n', 'N', '\0'),
    (0x32, ',', '?', '\0'),
    (0x33, ';', '.', '\0'),
    (0x34, ':', '/', '\0'),
    (0x35, '!', '§', '\0'),
    (0x39, ' ', ' ', '\0'),
    (0x56, '<', '>', '\0'),
];

/// The state a scancode needs to be read: the modifiers held, and whether the
/// previous byte announced an extended key.
#[derive(Default)]
pub struct Keyboard {
    shift: bool,
    altgr: bool,
    extended: bool,
}

impl Keyboard {
    pub fn feed(&mut self, code: u8) -> Option<Key> {
        if code == 0xE0 {
            self.extended = true;
            return None;
        }
        let extended = core::mem::replace(&mut self.extended, false);
        let released = code & 0x80 != 0;
        let make = code & 0x7F;
        if make == 0x2A || make == 0x36 {
            self.shift = !released;
            return None;
        }
        if make == 0x38 && extended {
            self.altgr = !released;
            return None;
        }
        if released {
            return None;
        }
        if extended {
            return match make {
                0x48 => Some(Key::Up),
                0x50 => Some(Key::Down),
                0x4B => Some(Key::Left),
                0x4D => Some(Key::Right),
                0x5B | 0x5C => Some(Key::Menu),
                0x1C => Some(Key::Enter),
                _ => None,
            };
        }
        match make {
            0x01 => Some(Key::Escape),
            0x0E => Some(Key::Backspace),
            0x0F => Some(Key::Tab),
            0x1C => Some(Key::Enter),
            0x3B => Some(Key::Menu),
            _ => {
                let &(_, alone, shifted, altgr) = AZERTY.iter().find(|entry| entry.0 == make)?;
                let c = if self.altgr {
                    altgr
                } else if self.shift {
                    shifted
                } else {
                    alone
                };
                (c != '\0').then_some(Key::Char(c))
            }
        }
    }
}
