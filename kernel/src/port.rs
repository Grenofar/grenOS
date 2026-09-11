//! x86 I/O ports: the PIC, the PIT and the PS/2 controller all speak
//! through them.

use core::arch::asm;

/// Reads a byte from `port`.
///
/// # Safety
///
/// Reading some ports has effects (the PS/2 data port hands over a byte):
/// the caller must know what `port` is.
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: the caller vouches for the port; `in` only reads it.
    unsafe { asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags)) };
    value
}

/// Writes a byte to `port`.
///
/// # Safety
///
/// The caller must know what writing `value` to `port` does.
pub unsafe fn outb(port: u16, value: u8) {
    // SAFETY: the caller vouches for the port and the value.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)) };
}

/// A short pause for slow devices: a write to port 0x80, unused once booted.
pub fn io_wait() {
    // SAFETY: port 0x80 is the POST code port; writing it only takes time.
    unsafe { outb(0x80, 0) };
}
