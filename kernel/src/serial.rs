pub const COM1: u16 = 0x3F8;

unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    value
}

pub fn init() {
    unsafe {
        // Disable interrupts
        outb(COM1 + 1, 0x00);

        // Enable DLAB (set baud rate divisor)
        outb(COM1 + 3, 0x80);

        // Set divisor to 3 (lo byte) 0 (hi byte) for 38400 baud
        outb(COM1, 0x03);
        outb(COM1 + 1, 0x00);

        // 8 bits, no parity, one stop bit
        outb(COM1 + 3, 0x03);

        // Enable FIFO, clear them, with 14-byte threshold
        outb(COM1 + 2, 0xC7);

        // IRQs enabled, RTS/DSR set
        outb(COM1 + 4, 0x0B);
    }
}

fn transmit_empty() -> bool {
    unsafe { inb(COM1 + 5) & 0x20 != 0 }
}

pub fn write_byte(byte: u8) {
    while !transmit_empty() {}
    unsafe {
        outb(COM1, byte);
    }
}

pub fn write_str(s: &str) {
    for byte in s.bytes() {
        write_byte(byte);
    }
}

pub fn write_hex(mut val: u64) {
    if val == 0 {
        write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 16];
    let mut len = 0;
    while val > 0 {
        let nibble = (val & 0xf) as u8;
        buf[len] = if nibble < 10 { b'0' + nibble } else { b'a' + nibble - 10 };
        len += 1;
        val >>= 4;
    }
    for i in (0..len).rev() {
        write_byte(buf[i]);
    }
}
