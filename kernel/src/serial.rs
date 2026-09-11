pub const COM1: u16 = 0x3F8;

/// Initialize the serial port (COM1) for 38400 baud, 8N1, FIFO enabled, interrupts disabled.
pub fn init() {
    unsafe {
        // Disable interrupts
        outb(COM1 + 1, 0x00);
        // Set DLAB to access baud divisor
        outb(COM1 + 3, 0x80);
        // Set baud rate to 38400: divisor = 3 (115200 / 38400)
        outb(COM1 + 0, 0x03); // low byte
        outb(COM1 + 1, 0x00); // high byte
        // Set DLAB back to 0, and set data bits: 8, stop bits: 1, parity: none
        outb(COM1 + 3, 0x03);
        // Enable FIFO, clear them, set 14-byte threshold
        outb(COM1 + 2, 0xC7);
        // Disable interrupts again (just in case)
        outb(COM1 + 1, 0x00);
    }
}

/// Write a byte to the serial port.
pub fn write_byte(byte: u8) {
    // Wait for transmit buffer to be empty
    unsafe {
        while (inb(COM1 + 5) & 0x20) == 0 {}
        outb(COM1, byte);
    }
}

/// Write a string to the serial port.
pub fn write_str(s: &str) {
    for byte in s.bytes() {
        write_byte(byte);
    }
}

/// Read a byte from the specified port (unsafe)
unsafe fn inb(port: u16) -> u8 {
    let ret: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") ret, options(nomem, nostack, preserves_flags));
    ret
}

/// Write a byte to the specified port (unsafe)
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}