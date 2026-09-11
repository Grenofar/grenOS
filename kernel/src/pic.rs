//! The two 8259 PICs. Device interrupts 0 to 15 move to vectors 32 to 47,
//! clear of the CPU exceptions, and only the timer (IRQ0), the keyboard
//! (IRQ1) and the mouse (IRQ12, through IRQ2) are let through.

use crate::port::{io_wait, outb};

const MASTER_COMMAND: u16 = 0x20;
const MASTER_DATA: u16 = 0x21;
const SLAVE_COMMAND: u16 = 0xA0;
const SLAVE_DATA: u16 = 0xA1;
const END_OF_INTERRUPT: u8 = 0x20;

/// The vector of IRQ0; IRQ n arrives on OFFSET + n.
pub const OFFSET: u8 = 32;

pub fn init() {
    let steps: [(u16, u8); 8] = [
        (MASTER_COMMAND, 0x11), // ICW1: initialise, ICW4 follows
        (SLAVE_COMMAND, 0x11),
        (MASTER_DATA, OFFSET), // ICW2: vector offsets
        (SLAVE_DATA, OFFSET + 8),
        (MASTER_DATA, 0x04), // ICW3: the slave hangs on IRQ2
        (SLAVE_DATA, 0x02),  //       and knows it is number 2
        (MASTER_DATA, 0x01), // ICW4: 8086 mode
        (SLAVE_DATA, 0x01),
    ];
    // SAFETY: the standard initialisation sequence of the PC's two 8259s,
    // then their masks; interrupts are still off.
    unsafe {
        for (port, value) in steps {
            outb(port, value);
            io_wait();
        }
        outb(MASTER_DATA, !0b0000_0111);
        outb(SLAVE_DATA, !0b0001_0000);
    }
}

/// Tells the PICs that interrupt `irq` has been handled.
pub fn end_of_interrupt(irq: u8) {
    // SAFETY: an end-of-interrupt command only acknowledges.
    unsafe {
        if irq >= 8 {
            outb(SLAVE_COMMAND, END_OF_INTERRUPT);
        }
        outb(MASTER_COMMAND, END_OF_INTERRUPT);
    }
}
