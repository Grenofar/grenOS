//! The PIT's channel 0: the timer interrupt, HZ times a second.

use crate::port::outb;

pub const HZ: u32 = 100;

/// The PIT's input clock, in hertz.
const BASE: u32 = 1_193_182;

pub fn init() {
    let divisor = BASE / HZ;
    // SAFETY: channel 0, low byte then high byte, mode 3 (square wave); then
    // the divisor. Only the timer's pace changes.
    unsafe {
        outb(0x43, 0x36);
        outb(0x40, (divisor & 0xFF) as u8);
        outb(0x40, (divisor >> 8) as u8);
    }
}
