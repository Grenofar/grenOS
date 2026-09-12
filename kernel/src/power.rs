//! Restarting the machine, and turning it off.

use crate::acpi::Power;
use crate::port::{inb, inw, outb, outw};

/// The bit that starts the sleep the type field names.
const SLP_EN: u16 = 1 << 13;
/// The bit that says the kernel, not the firmware, drives ACPI.
const SCI_EN: u16 = 1;

/// Restarts the machine: the keyboard controller's reset line first, which
/// every PC, QEMU and VirtualBox answer, and a triple fault if it does not.
pub fn reboot() -> ! {
    crate::serial::write_str("power: reboot\n");
    for _ in 0..100_000 {
        // SAFETY: reading the PS/2 status register changes nothing.
        if unsafe { inb(0x64) } & 0x02 == 0 {
            break;
        }
    }
    // SAFETY: 0xFE pulses the reset line of the 8042; that is the whole point.
    unsafe { outb(0x64, 0xFE) };
    for _ in 0..1_000_000 {
        core::hint::spin_loop();
    }
    // Still here: leave the CPU without a table to handle an interrupt with,
    // then raise one. The triple fault resets the machine.
    #[repr(C, packed)]
    struct Pointer {
        limit: u16,
        base: u64,
    }
    let nothing = Pointer { limit: 0, base: 0 };
    // SAFETY: an empty IDT and an int3 are the standard way out when the
    // firmware ignores the reset line; the machine restarts.
    unsafe {
        core::arch::asm!("lidt [{}]", in(reg) &nothing, options(readonly, nostack, preserves_flags));
        core::arch::asm!("int3", options(nomem, nostack));
    }
    halt()
}

/// Turns the machine off through ACPI. Comes back only if the firmware
/// ignores it, and the caller then leaves the goodbye screen up.
pub fn off(power: Power) -> ! {
    crate::serial::write_str("power: off\n");
    // SAFETY: the ACPI registers the firmware's own tables named. Asking for
    // control of ACPI and then writing the S5 sleep type is what turns a PC
    // off; nothing else answers on these ports.
    unsafe {
        if power.smi != 0 && power.enable != 0 && inw(power.pm1a) & SCI_EN == 0 {
            outb(power.smi, power.enable);
            for _ in 0..100_000 {
                if inw(power.pm1a) & SCI_EN != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
        }
        outw(power.pm1a, power.slp_typa << 10 | SLP_EN);
        if power.pm1b != 0 {
            outw(power.pm1b, power.slp_typb << 10 | SLP_EN);
        }
    }
    halt()
}

/// Nothing left to do: rest, and let the human close the window.
pub fn halt() -> ! {
    loop {
        // SAFETY: hlt rests until the next interrupt.
        unsafe { core::arch::asm!("hlt", options(nomem, nostack, preserves_flags)) };
    }
}
