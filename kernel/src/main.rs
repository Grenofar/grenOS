#![no_std]
#![no_main]

use core::panic::PanicInfo;

use limine::BaseRevision;

#[used]
#[link_section = ".requests"]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // SAFETY: This is the entry point, called once by the bootloader.
    // The serial port is a fixed hardware address on x86_64.
    unsafe {
        init_serial();
        write_serial_str("grenOS\n");
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

const COM1: u16 = 0x3F8;

unsafe fn init_serial() {
    // SAFETY: COM1 is a valid I/O port address on x86_64.
    unsafe {
        // Disable interrupts
        outb(COM1 + 1, 0x00);
        // Enable DLAB
        outb(COM1 + 3, 0x80);
        // Set divisor to 3 (38400 baud)
        outb(COM1 + 0, 0x03);
        outb(COM1 + 1, 0x00);
        // 8 bits, no parity, one stop bit
        outb(COM1 + 3, 0x03);
        // Enable FIFO, clear them, with 14-byte threshold
        outb(COM1 + 2, 0xC7);
        // IRQs enabled, RTS/DSR set
        outb(COM1 + 4, 0x0B);
    }
}

unsafe fn write_serial_str(s: &str) {
    for byte in s.bytes() {
        // SAFETY: COM1 is a valid I/O port address on x86_64.
        unsafe {
            write_serial_byte(byte);
        }
    }
}

unsafe fn write_serial_byte(byte: u8) {
    // SAFETY: COM1 is a valid I/O port address on x86_64.
    unsafe {
        while (inb(COM1 + 5) & 0x20) == 0 {}
        outb(COM1, byte);
    }
}

unsafe fn outb(port: u16, value: u8) {
    // SAFETY: The caller must ensure the port is valid for the platform.
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

unsafe fn inb(port: u16) -> u8 {
    // SAFETY: The caller must ensure the port is valid for the platform.
    let value: u8;
    unsafe {
        core::arch::asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    value
}
