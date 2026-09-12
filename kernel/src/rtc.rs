//! The CMOS real-time clock: the date and time the panel shows, read again
//! every second.

use core::arch::asm;

use crate::time::DateTime;

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

/// The date and time as the clock keeps them: UTC in QEMU, local time in
/// VirtualBox and on most PCs.
pub fn now() -> DateTime {
    // During an update the registers disagree with one another.
    while read(0x0A) & 0x80 != 0 {}
    let (second, minute, hour, day, month, year, format) =
        (read(0x00), read(0x02), read(0x04), read(0x07), read(0x08), read(0x09), read(0x0B));
    let binary = format & 0x04 != 0;
    let decode = |value: u8| if binary { value } else { from_bcd(value) };
    let mut hours = decode(hour & 0x7F);
    if format & 0x02 == 0 {
        // A 12-hour clock: bit 7 of the hour marks the afternoon.
        hours = hours % 12 + if hour & 0x80 != 0 { 12 } else { 0 };
    }
    DateTime {
        year: 2000 + u16::from(decode(year)),
        month: decode(month),
        day: decode(day),
        hour: hours % 24,
        minute: decode(minute) % 60,
        second: decode(second) % 60,
    }
}
