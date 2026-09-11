//! The interrupt descriptor table: the CPU exceptions grenOS reports, and the
//! device interrupts its desktop listens to (timer, keyboard, mouse), which
//! the PIC moves to vectors 32 and up.

use core::mem::size_of;

use crate::events::{self, Event};
use crate::pic;
use crate::port::inb;

#[repr(C, packed)]
pub struct DescriptorTablePointer {
    pub limit: u16,
    pub base: u64,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}

impl IdtEntry {
    const MISSING: IdtEntry = IdtEntry {
        offset_low: 0,
        selector: 0,
        ist: 0,
        type_attr: 0,
        offset_mid: 0,
        offset_high: 0,
        zero: 0,
    };

    /// A present ring-0 interrupt gate to `handler`, on IST `ist` (0: none).
    fn gate(handler: usize, ist: u8) -> Self {
        let addr = handler as u64;
        IdtEntry {
            offset_low: (addr & 0xFFFF) as u16,
            selector: 0x08, // the kernel code segment of gdt.rs
            ist,
            type_attr: 0x8E,
            offset_mid: ((addr >> 16) & 0xFFFF) as u16,
            offset_high: (addr >> 32) as u32,
            zero: 0,
        }
    }
}

/// What the CPU pushes before calling a handler. The handlers only need its
/// layout, not its fields.
#[repr(C)]
#[allow(dead_code)]
pub struct ExceptionStackFrame {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::MISSING; 256];

fn halt() -> ! {
    loop {
        // SAFETY: hlt rests until the next interrupt.
        unsafe { core::arch::asm!("hlt", options(nomem, nostack, preserves_flags)) };
    }
}

extern "x86-interrupt" fn breakpoint_handler(_frame: ExceptionStackFrame) {
    crate::serial::write_str("Breakpoint\n");
}

extern "x86-interrupt" fn double_fault_handler(_frame: ExceptionStackFrame, _error_code: u64) -> ! {
    crate::serial::write_str("Double fault\n");
    halt()
}

extern "x86-interrupt" fn general_protection_handler(_frame: ExceptionStackFrame, error_code: u64) -> ! {
    crate::serial::write_str("General protection fault ");
    crate::serial::write_hex(error_code);
    crate::serial::write_str("\n");
    halt()
}

extern "x86-interrupt" fn page_fault_handler(_frame: ExceptionStackFrame, error_code: u64) {
    let cr2: u64;
    // SAFETY: reading CR2, the faulting address, has no effect.
    unsafe { core::arch::asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags)) };
    crate::serial::write_str("Page fault ");
    crate::serial::write_hex(cr2);
    crate::serial::write_str(" ");
    crate::serial::write_hex(error_code);
    crate::serial::write_str("\n");
    halt()
}

extern "x86-interrupt" fn timer_handler(_frame: ExceptionStackFrame) {
    events::tick();
    pic::end_of_interrupt(0);
}

extern "x86-interrupt" fn keyboard_handler(_frame: ExceptionStackFrame) {
    // SAFETY: IRQ1 means the controller holds a keyboard byte on port 0x60.
    let code = unsafe { inb(0x60) };
    events::push(Event::Key(code));
    pic::end_of_interrupt(1);
}

extern "x86-interrupt" fn mouse_handler(_frame: ExceptionStackFrame) {
    // SAFETY: IRQ12 means the controller holds a mouse byte on port 0x60.
    let byte = unsafe { inb(0x60) };
    events::push(Event::Mouse(byte));
    pic::end_of_interrupt(12);
}

/// IRQ7 can fire with nothing behind it; it takes no end of interrupt.
extern "x86-interrupt" fn spurious_master(_frame: ExceptionStackFrame) {}

/// A spurious IRQ15: the master still saw IRQ2, and wants its end.
extern "x86-interrupt" fn spurious_slave(_frame: ExceptionStackFrame) {
    pic::end_of_interrupt(2);
}

pub fn init() {
    let irq = |n: u8| usize::from(pic::OFFSET + n);
    let entries: [(usize, usize, u8); 9] = [
        (3, breakpoint_handler as usize, 0),
        (8, double_fault_handler as usize, 1), // on IST1, a stack of its own
        (13, general_protection_handler as usize, 0),
        (14, page_fault_handler as usize, 0),
        (irq(0), timer_handler as usize, 0),
        (irq(1), keyboard_handler as usize, 0),
        (irq(7), spurious_master as usize, 0),
        (irq(12), mouse_handler as usize, 0),
        (irq(15), spurious_slave as usize, 0),
    ];
    // SAFETY: the IDT is written here only, once, with interrupts off; lidt
    // then points the CPU at it for good.
    unsafe {
        let idt = core::ptr::addr_of_mut!(IDT);
        for (vector, handler, ist) in entries {
            (*idt)[vector] = IdtEntry::gate(handler, ist);
        }
        let pointer = DescriptorTablePointer {
            limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: idt as u64,
        };
        core::arch::asm!("lidt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
    }
}
