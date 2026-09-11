//! The PS/2 controller: the keyboard on its first port, the mouse on its
//! second, each raising an interrupt (IRQ1, IRQ12) for every byte.

use crate::port::{inb, outb};

const DATA: u16 = 0x60;
const STATUS_COMMAND: u16 = 0x64;

fn status() -> u8 {
    // SAFETY: reading the status register changes nothing.
    unsafe { inb(STATUS_COMMAND) }
}

fn wait_input_empty() {
    for _ in 0..100_000 {
        if status() & 0x02 == 0 {
            return;
        }
    }
}

fn wait_output_full() -> bool {
    (0..100_000).any(|_| status() & 0x01 != 0)
}

fn command(byte: u8) {
    wait_input_empty();
    // SAFETY: a controller command; the ones sent here only configure it.
    unsafe { outb(STATUS_COMMAND, byte) };
}

fn write(byte: u8) {
    wait_input_empty();
    // SAFETY: a data byte, for the controller or, after 0xD4, the mouse.
    unsafe { outb(DATA, byte) };
}

fn read() -> Option<u8> {
    // SAFETY: the output buffer is full, so the data port holds a byte.
    wait_output_full().then(|| unsafe { inb(DATA) })
}

/// Sends `byte` to the mouse; true when it acknowledges (0xFA).
fn to_mouse(byte: u8) -> bool {
    command(0xD4);
    write(byte);
    read() == Some(0xFA)
}

/// Turns on both ports and their interrupts, and asks the mouse to report
/// its movements. True when the mouse answered. Call with interrupts off.
pub fn init() -> bool {
    // Whatever the firmware left in the output buffer.
    while status() & 0x01 != 0 {
        // SAFETY: the buffer is full; reading empties it.
        unsafe { inb(DATA) };
    }
    command(0xA8); // the second port, the mouse's, on
    command(0x20); // read the configuration byte
    let config = read().unwrap_or(0x47);
    command(0x60); // and write it back with IRQ1 and IRQ12 on, both clocks running
    write((config | 0x03) & !0x30);
    to_mouse(0xF6) && to_mouse(0xF4) // defaults, then report movements
}
