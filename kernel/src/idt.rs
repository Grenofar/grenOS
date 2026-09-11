#![allow(dead_code)]

use core::mem::size_of;

#[repr(C, packed)]
pub struct DescriptorTablePointer {
    pub limit: u16,
    pub base: u64,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct IdtEntry {
    pub offset_low: u16,
    pub selector: u16,
    pub ist: u8,          // bits 0..2: IST index (0 = none, 1..7 = IST1..7)
    pub type_attr: u8,    // 0x8E = present, ring 0, 64-bit interrupt gate
    pub offset_mid: u16,
    pub offset_high: u32,
    pub zero: u32,
}

#[repr(C)]
pub struct ExceptionStackFrame {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry {
    offset_low: 0,
    selector: 0,
    ist: 0,
    type_attr: 0,
    offset_mid: 0,
    offset_high: 0,
    zero: 0,
}; 256];

pub unsafe fn load_idt(ptr: &DescriptorTablePointer) {
    // SAFETY: ptr points to a valid IDT pointer with correct limit and base.
    unsafe {
        core::arch::asm!("lidt [{}]", in(reg) ptr, options(readonly, nostack, preserves_flags));
    }
}

extern "x86-interrupt" fn breakpoint_handler(_stack_frame: ExceptionStackFrame) {
    crate::serial::write_str("Breakpoint\n");
}

extern "x86-interrupt" fn double_fault_handler(_stack_frame: ExceptionStackFrame, _error_code: u64) -> ! {
    crate::serial::write_str("Double fault\n");
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

extern "x86-interrupt" fn general_protection_handler(_stack_frame: ExceptionStackFrame, _error_code: u64) -> ! {
    crate::serial::write_str("General protection fault\n");
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

extern "x86-interrupt" fn page_fault_handler(stack_frame: ExceptionStackFrame, _error_code: u64) {
    let cr2: u64;
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) cr2);
    }
    crate::serial::write_str("Page fault\n");
    crate::serial::write_hex(cr2);
    crate::serial::write_str("\n");
}

pub fn init() {
    unsafe {
        // Set up IDT descriptor
        let idt_ptr = DescriptorTablePointer {
            limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: core::ptr::addr_of_mut!(IDT) as u64,
        };

        // Zero IDT
        for entry in IDT.iter_mut() {
            *entry = IdtEntry {
                offset_low: 0,
                selector: 0,
                ist: 0,
                type_attr: 0,
                offset_mid: 0,
                offset_high: 0,
                zero: 0,
            };
        }

        // Set up handlers
        // Breakpoint (#BP) - vector 3, no error code
        IDT[3] = IdtEntry {
            offset_low: breakpoint_handler as u64 & 0xFFFF,
            selector: 0x08, // kernel code segment
            ist: 0,
            type_attr: 0x8E, // present, ring 0, 64-bit interrupt gate
            offset_mid: (breakpoint_handler as u64 >> 16) & 0xFFFF,
            offset_high: (breakpoint_handler as u64 >> 32) as u32,
            zero: 0,
        };

        // Double fault (#DF) - vector 8, with error code, IST1
        IDT[8] = IdtEntry {
            offset_low: double_fault_handler as u64 & 0xFFFF,
            selector: 0x08,
            ist: 1, // IST1
            type_attr: 0x8E,
            offset_mid: (double_fault_handler as u64 >> 16) & 0xFFFF,
            offset_high: (double_fault_handler as u64 >> 32) as u32,
            zero: 0,
        };

        // General protection fault (#GP) - vector 13, with error code
        IDT[13] = IdtEntry {
            offset_low: general_protection_handler as u64 & 0xFFFF,
            selector: 0x08,
            ist: 0,
            type_attr: 0x8E,
            offset_mid: (general_protection_handler as u64 >> 16) & 0xFFFF,
            offset_high: (general_protection_handler as u64 >> 32) as u32,
            zero: 0,
        };

        // Page fault (#PF) - vector 14, with error code
        IDT[14] = IdtEntry {
            offset_low: page_fault_handler as u64 & 0xFFFF,
            selector: 0x08,
            ist: 0,
            type_attr: 0x8E,
            offset_mid: (page_fault_handler as u64 >> 16) & 0xFFFF,
            offset_high: (page_fault_handler as u64 >> 32) as u32,
            zero: 0,
        };

        // Load IDT
        load_idt(&idt_ptr);
    }
}
