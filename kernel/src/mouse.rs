//! The mouse: three-byte PS/2 packets, turned into moves and a button.

/// One packet: how far the mouse moved, in screen axes (y grows downwards),
/// and whether its left button is down.
#[derive(Clone, Copy)]
pub struct Packet {
    pub dx: i32,
    pub dy: i32,
    pub left: bool,
}

/// Collects the bytes IRQ12 delivers until a packet is whole.
#[derive(Default)]
pub struct Decoder {
    bytes: [u8; 3],
    count: usize,
}

impl Decoder {
    pub fn feed(&mut self, byte: u8) -> Option<Packet> {
        // A first byte always has bit 3 set: anything else is out of step.
        if self.count == 0 && byte & 0x08 == 0 {
            return None;
        }
        self.bytes[self.count] = byte;
        self.count += 1;
        if self.count < 3 {
            return None;
        }
        self.count = 0;
        let [flags, x, y] = self.bytes;
        // An overflowed packet says nothing reliable.
        if flags & 0xC0 != 0 {
            return None;
        }
        let dx = i32::from(x) - if flags & 0x10 != 0 { 256 } else { 0 };
        let dy = i32::from(y) - if flags & 0x20 != 0 { 256 } else { 0 };
        Some(Packet { dx, dy: -dy, left: flags & 0x01 != 0 })
    }
}
