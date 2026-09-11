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

extern "x86-interrupt" fn page_fault_handler(_stack_frame: ExceptionStackFrame, _error_code: u64) {
    let cr2: u64;
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) cr2);
    }
    crate::serial::write_str("Page fault ");
    crate::serial::write_hex(cr2);
    crate::serial::write_str(" ");
    crate::serial::write_hex(_error_code);
    crate::serial::write_str("\n");
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

pub fn init() {
    unsafe {
        // Set up IDT descriptor
        let idt_ptr = core::ptr::addr_of_mut!(IDT);
        // Zero IDT
        for i in 0..IDT.len() {
            (*idt_ptr)[i] = IdtEntry {
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
        let bp_addr = breakpoint_handler as usize as u64;
        (*idt_ptr)[3] = IdtEntry {
            offset_low: (bp_addr & 0xFFFF) as u16,
            selector: 0x08, // kernel code segment
            ist: 0,
            type_attr: 0x8E, // present, ring 0, 64-bit interrupt gate
            offset_mid: ((bp_addr >> 16) & 0xFFFF) as u16,
            offset_high: (bp_addr >> 32) as u32,
            zero: 0,
        };

        // Double fault (#DF) - vector 8, with error code, IST1
        let df_addr = double_fault_handler as usize as u64;
        (*idt_ptr)[8] = IdtEntry {
            offset_low: (df_addr & 0xFFFF) as u16,
            selector: 0x08,
            ist: 1, // IST1
            type_attr: 0x8E,
            offset_mid: ((df_addr >> 16) & 0xFFFF) as u16,
            offset_high: (df_addr >> 32) as u32,
            zero: 0,
        };

        // General protection fault (#GP) - vector 13, with error code
        let gp_addr = general_protection_handler as usize as u64;
        (*idt_ptr)[13] = IdtEntry {
            offset_low: (gp_addr & 0xFFFF) as u16,
            selector: 0x08,
            ist: 0,
            type_attr: 0x8E,
            offset_mid: ((gp_addr >> 16) & 0xFFFF) as u16,
            offset_high: (gp_addr >> 32) as u32,
            zero: 0,
        };

        // Page fault (#PF) - vector 14, with error code
        let pf_addr = page_fault_handler as usize as u64;
        (*idt_ptr)[14] = IdtEntry {
            offset_low: (pf_addr & 0xFFFF) as u16,
            selector: 0x08,
            ist: 0,
            type_attr: 0x8E,
            offset_mid: ((pf_addr >> 16) & 0xFFFF) as u16,
            offset_high: (pf_addr >> 32) as u32,
            zero: 0,
        };

        // Load IDT
        let idt_ptr = DescriptorTablePointer {
            limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: core::ptr::addr_of!(IDT) as u64,
        };
        load_idt(&idt_ptr);
    }
}
