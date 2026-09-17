//! The mouse: PS/2 packets, turned into moves, a button and the wheel. A plain
//! mouse sends three bytes a packet; one that answered the IntelliMouse knock
//! (`ps2::init`) sends a fourth, the wheel.

/// One packet: how far the mouse moved, in screen axes (y grows downwards),
/// whether its left button is down, and how many notches the wheel turned
/// (positive towards the person: the page goes down).
#[derive(Clone, Copy)]
pub struct Packet {
    pub dx: i32,
    pub dy: i32,
    pub left: bool,
    pub wheel: i32,
}

/// Collects the bytes IRQ12 delivers until a packet is whole.
pub struct Decoder {
    bytes: [u8; 4],
    count: usize,
    size: usize,
}

impl Decoder {
    /// A decoder for packets of four bytes when the mouse has a wheel, three
    /// otherwise.
    pub fn new(wheel: bool) -> Self {
        Decoder { bytes: [0; 4], count: 0, size: if wheel { 4 } else { 3 } }
    }

    pub fn feed(&mut self, byte: u8) -> Option<Packet> {
        // A first byte always has bit 3 set: anything else is out of step.
        if self.count == 0 && byte & 0x08 == 0 {
            return None;
        }
        self.bytes[self.count] = byte;
        self.count += 1;
        if self.count < self.size {
            return None;
        }
        self.count = 0;
        let [flags, x, y, z] = self.bytes;
        // The fourth byte is a signed count of notches (zero on a plain mouse,
        // whose buffer slot is never written).
        let wheel = i32::from(z as i8);
        // An overflowed packet says nothing reliable.
        if flags & 0xC0 != 0 {
            return None;
        }
        let dx = i32::from(x) - if flags & 0x10 != 0 { 256 } else { 0 };
        let dy = i32::from(y) - if flags & 0x20 != 0 { 256 } else { 0 };
        Some(Packet { dx, dy: -dy, left: flags & 0x01 != 0, wheel })
    }
}
