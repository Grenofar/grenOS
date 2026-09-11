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

pub fn init() {
    unsafe {
        // Zero out the IDT
        core::ptr::write_bytes(&mut IDT as *mut IdtEntry as u8, 0, size_of::<IdtEntry>() * 256);

        // Set up the exception handlers
        set_idt_entry(3, handler_breakpoint_stub as u64, 0x08, 0, 0x8E);   // #BP
        set_idt_entry(8, handler_double_fault_stub as u64, 0x08, 1, 0x8E); // #DF with IST1
        set_idt_entry(13, handler_general_protection_stub as u64, 0x08, 0, 0x8E); // #GP
        set_idt_entry(14, handler_page_fault_stub as u64, 0x08, 0, 0x8E); // #PF

        // Load the IDT
        let idt_ptr = DescriptorTablePointer {
            limit: (size_of::<IdtEntry>() * 256 - 1) as u16,
            base: &IDT as *const _ as u64,
        };
        load_idt(&idt_ptr);
    }
}

fn set_idt_entry(index: usize, offset: u64, selector: u16, ist: u8, type_attr: u8) {
    let entry = &mut IDT[index];
    entry.offset_low = (offset & 0xFFFF) as u16;
    entry.selector = selector;
    entry.ist = ist & 0x7; // only lower 3 bits
    entry.type_attr = type_attr;
    entry.offset_mid = ((offset >> 16) & 0xFFFF) as u16;
    entry.offset_high = ((offset >> 32) & 0xFFFFFFFF) as u32;
    entry.zero = 0;
}

#[naked]
pub unsafe extern "C" fn handler_breakpoint_stub() {
    core::arch::asm!(
        "push rax",
        "push rbx",
        "call handler_breakpoint_rust",
        "pop rbx",
        "pop rax",
        "iretq",
        options(noreturn)
    );
}

#[naked]
pub unsafe extern "C" fn handler_double_fault_stub() {
    core::arch::asm!(
        "push rax",
        "push rbx",
        "mov rbx, [rsp + 40]", // error code after pushing rax, rbx
        "call handler_double_fault_rust",
        "pop rbx",
        "pop rax",
        "hlt",
        "jmp .",
        options(noreturn)
    );
}

#[naked]
pub unsafe extern "C" fn handler_general_protection_stub() {
    core::arch::asm!(
        "push rax",
        "push rbx",
        "mov rbx, [rsp + 40]", // error code
        "call handler_general_protection_rust",
        "pop rbx",
        "pop rax",
        "hlt",
        "jmp .",
        options(noreturn)
    );
}

#[naked]
pub unsafe extern "C" fn handler_page_fault_stub() {
    core::arch::asm!(
        "push rax",
        "push rbx",
        "mov rbx, [rsp + 40]", // error code
        "call handler_page_fault_rust",
        "pop rbx",
        "pop rax",
        "hlt",
        "jmp .",
        options(noreturn)
    );
}

extern "C" fn handler_breakpoint_rust() {
    serial::write_str("breakpoint\n");
}

extern "C" fn handler_double_fault_rust(_error_code: u64) {
    serial::write_str("double fault\n");
}

extern "C" fn handler_general_protection_rust(_error_code: u64) {
    serial::write_str("general protection fault\n");
}

extern "C" fn handler_page_fault_rust(error_code: u64) {
    serial::write_str("page fault: ");
    unsafe {
        let cr2: u64;
        core::arch::asm!("mov {}, cr2", out(reg) cr2);
        serial::write_hex(cr2);
    }
    serial::write_str(" error: ");
    serial::write_hex(error_code);
    serial::write_str("\n");
}
