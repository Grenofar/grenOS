//! x86 I/O ports: the PIC, the PIT, the PS/2 controller, PCI configuration
//! space and the ACPI power registers all speak through them.

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

/// Reads a 16-bit word from `port`.
///
/// # Safety
///
/// As [`inb`], for a port that answers a word (the ACPI PM1 registers).
pub unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    // SAFETY: the caller vouches for the port; `in` only reads it.
    unsafe { asm!("in ax, dx", out("ax") value, in("dx") port, options(nomem, nostack, preserves_flags)) };
    value
}

/// Writes a 16-bit word to `port`.
///
/// # Safety
///
/// As [`outb`]. A word to the ACPI PM1a control register turns the machine off.
pub unsafe fn outw(port: u16, value: u16) {
    // SAFETY: the caller vouches for the port and the value.
    unsafe { asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags)) };
}

/// Reads a 32-bit word from `port`.
///
/// # Safety
///
/// As [`inb`], for a port that answers a double word (PCI configuration data).
pub unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    // SAFETY: the caller vouches for the port; `in` only reads it.
    unsafe { asm!("in eax, dx", out("eax") value, in("dx") port, options(nomem, nostack, preserves_flags)) };
    value
}

/// Writes a 32-bit word to `port`.
///
/// # Safety
///
/// As [`outb`], for a port that takes a double word (PCI configuration address).
pub unsafe fn outl(port: u16, value: u32) {
    // SAFETY: the caller vouches for the port and the value.
    unsafe { asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack, preserves_flags)) };
}

/// A short pause for slow devices: a write to port 0x80, unused once booted.
pub fn io_wait() {
    // SAFETY: port 0x80 is the POST code port; writing it only takes time.
    unsafe { outb(0x80, 0) };
}
