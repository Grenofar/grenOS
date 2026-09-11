//! The CMOS real-time clock: the time the taskbar shows at boot. Read once;
//! it ticks when the timer interrupt arrives.

use core::arch::asm;

const INDEX: u16 = 0x70;
const DATA: u16 = 0x71;

fn read(register: u8) -> u8 {
    let value: u8;
    // SAFETY: 0x70 and 0x71 are the CMOS index and data ports on every PC, in
    // QEMU and in VirtualBox; selecting a register and reading it changes
    // nothing else.
    unsafe {
        asm!("out dx, al", in("dx") INDEX, in("al") register, options(nomem, nostack, preserves_flags));
        asm!("in al, dx", out("al") value, in("dx") DATA, options(nomem, nostack, preserves_flags));
    }
    value
}

fn from_bcd(value: u8) -> u8 {
    (value & 0x0F) + (value >> 4) * 10
}

/// Hours (0-23) and minutes, as the clock keeps them: UTC in QEMU, local
/// time in VirtualBox and on most PCs.
pub fn time() -> (u8, u8) {
    // During an update the registers disagree with one another.
    while read(0x0A) & 0x80 != 0 {}
    let (raw_hours, raw_minutes, format) = (read(0x04), read(0x02), read(0x0B));
    let binary = format & 0x04 != 0;
    let hours_24 = format & 0x02 != 0;
    let afternoon = raw_hours & 0x80 != 0;
    let mut hours = raw_hours & 0x7F;
    let mut minutes = raw_minutes;
    if !binary {
        hours = from_bcd(hours);
        minutes = from_bcd(minutes);
    }
    if !hours_24 {
        hours = hours % 12 + if afternoon { 12 } else { 0 };
    }
    (hours % 24, minutes % 60)
}
