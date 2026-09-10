use core::arch::x86_64;

pub const COM1: u16 = 0x3F8;

#[inline(always)]
unsafe fn outb(port: u16, val: u8) {
    x86_64::outb(port, val);
}

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    x86_64::inb(port)
}

pub fn init() {
    unsafe {
        // Disable interrupts
        outb(COM1 + 1, 0x00);
        // Set baud rate to 38400 (divisor = 3)
        outb(COM1 + 3, 0x80); // Set DLAB
        outb(COM1 + 0, 0x03); // Low byte
        outb(COM1 + 1, 0x00); // High byte
        // Set line format: 8N1
        outb(COM1 + 3, 0x03);
        // Enable FIFO, clear, 14-byte threshold
        outb(COM1 + 2, 0xC7);
        // Disable interrupts
        outb(COM1 + 1, 0x00);
    }
}

pub fn write_byte(byte: u8) {
    unsafe {
        // Wait for transmit buffer empty
        while (inb(COM1 + 5) & 0x20) == 0 {}
        outb(COM1 + 0, byte);
    }
}

pub fn write_str(s: &str) {
    for b in s.bytes() {
        write_byte(b);
    }
}
