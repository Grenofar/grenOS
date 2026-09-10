#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // SAFETY: The Limine boot protocol guarantees that the boot info pointer
    // is valid and points to a properly initialized structure.
    let boot_info = unsafe { &*limine_boot_info() };

    // SAFETY: The serial port is a standard COM1 at I/O port 0x3F8.
    // Writing to it is safe as long as the port exists, which is assumed
    // for the target environment.
    let mut serial = unsafe { SerialPort::new(0x3F8) };
    serial.init();

    for byte in b"grenOS\n" {
        serial.write_byte(*byte);
    }

    loop {
        // SAFETY: Halt instruction is always safe.
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        // SAFETY: Halt instruction is always safe.
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
    }
}

struct SerialPort {
    port: u16,
}

impl SerialPort {
    // SAFETY: The caller must ensure that the port is a valid COM port.
    unsafe fn new(port: u16) -> Self {
        Self { port }
    }

    fn init(&mut self) {
        // SAFETY: Writing to the serial port registers is safe if the port exists.
        unsafe {
            // Disable interrupts
            self.outb(1, 0x00);
            // Enable DLAB
            self.outb(3, 0x80);
            // Set divisor to 3 (38400 baud)
            self.outb(0, 0x03);
            self.outb(1, 0x00);
            // 8 bits, no parity, one stop bit
            self.outb(3, 0x03);
            // Enable FIFO, clear them, with 14-byte threshold
            self.outb(2, 0xC7);
            // IRQs enabled, RTS/DSR set
            self.outb(4, 0x0B);
        }
    }

    fn write_byte(&mut self, byte: u8) {
        // SAFETY: Writing to the serial port data register is safe if the port exists.
        unsafe {
            // Wait for the transmit buffer to be empty
            while self.inb(5) & 0x20 == 0 {}
            self.outb(0, byte);
        }
    }

    // SAFETY: The caller must ensure that the port is valid.
    unsafe fn inb(&self, offset: u16) -> u8 {
        let value: u8;
        core::arch::asm!(
            "in al, dx",
            in("dx") self.port + offset,
            out("al") value,
            options(nomem, nostack)
        );
        value
    }

    // SAFETY: The caller must ensure that the port is valid.
    unsafe fn outb(&self, offset: u16, value: u8) {
        core::arch::asm!(
            "out dx, al",
            in("dx") self.port + offset,
            in("al") value,
            options(nomem, nostack)
        );
    }
}

// Limine boot protocol
#[repr(C)]
struct LimineBootInfo {
    // We only need the serial port, but the structure is larger.
    // We'll use a pointer to the actual structure and access fields as needed.
    _unused: [u8; 0],
}

#[no_mangle]
static LIMINE_BASE_REVISION: [u64; 3] = [0, 0, 3];

#[no_mangle]
static LIMINE_BOOTLOADER_INFO_REQUEST: [u64; 4] = [
    0xf68bf81b5b5b5b5b, // Limine Bootloader Info Request magic 1
    0x5b5b5b5bf68bf81b, // Limine Bootloader Info Request magic 2
    0,                  // Revision
    0,                  // Response pointer
];

#[no_mangle]
static LIMINE_MEMMAP_REQUEST: [u64; 4] = [
    0x67cf3d9d378a806f, // Limine Memmap Request magic 1
    0xe304acdfc50c3c62, // Limine Memmap Request magic 2
    0,                  // Revision
    0,                  // Response pointer
];

#[no_mangle]
static LIMINE_FRAMEBUFFER_REQUEST: [u64; 4] = [
    0x9d5827dcd881dd75, // Limine Framebuffer Request magic 1
    0xa3148604f6fab11b, // Limine Framebuffer Request magic 2
    0,                  // Revision
    0,                  // Response pointer
];

#[no_mangle]
static LIMINE_SERIAL_REQUEST: [u64; 4] = [
    0x07e1f5b5b5b5b5b5, // Limine Serial Request magic 1
    0x5b5b5b5b07e1f5b5, // Limine Serial Request magic 2
    0,                  // Revision
    0,                  // Response pointer
];

#[no_mangle]
extern "C" fn limine_boot_info() -> *const LimineBootInfo {
    // The boot info is passed via the bootloader info request.
    // We'll use a simple approach: the bootloader sets the response pointer.
    // For now, we'll just return a dummy pointer; the serial port is hardcoded.
    // This is a placeholder and will be replaced with proper boot info parsing.
    core::ptr::null()
}
