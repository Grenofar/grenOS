#![allow(dead_code)]

use core::mem::size_of;

#[repr(C, packed)]
pub struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[repr(C, packed)]
struct TaskStateSegment {
    reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    reserved2: u64,
    reserved3: u16,
    iopb_offset: u16,
}

static mut GDT: [u64; 5] = [0; 5];
static mut TSS: TaskStateSegment = TaskStateSegment {
    reserved0: 0,
    rsp0: 0,
    rsp1: 0,
    rsp2: 0,
    reserved1: 0,
    ist1: 0,
    ist2: 0,
    ist3: 0,
    ist4: 0,
    ist5: 0,
    ist6: 0,
    ist7: 0,
    reserved2: 0,
    reserved3: 0,
    iopb_offset: 0,
};
static mut IST1_STACK: [u8; 0x4000] = [0; 0x4000]; // 16 KiB

pub unsafe fn load_gdt(ptr: &DescriptorTablePointer) {
    // SAFETY: ptr points to a valid GDT descriptor table pointer.
    unsafe {
        core::arch::asm!("lgdt [{}]", in(reg) ptr, options(readonly, nostack, preserves_flags));
    }
}

pub unsafe fn load_tss(tss_sel: u16) {
    // SAFETY: tss_sel references a valid TSS descriptor in the active GDT.
    unsafe {
        core::arch::asm!("ltr {0:x}", in(reg) tss_sel, options(nostack, preserves_flags));
    }
}

pub unsafe fn reload_segments(code_sel: u16, data_sel: u16) {
    // SAFETY: code_sel and data_sel are valid selectors in the loaded GDT.
    unsafe {
        core::arch::asm!(
            "mov ds, {0:x}",
            "mov es, {0:x}",
            "mov fs, {0:x}",
            "mov gs, {0:x}",
            "mov ss, {0:x}",
            "push {1}",
            "lea {2}, [rip + 2f]",
            "push {2}",
            "retfq",
            "2:",
            in(reg) data_sel,
            in(reg) u64::from(code_sel),
            lateout(reg) _,
            options(preserves_flags)
        );
    }
}

pub fn init() {
    // Safety: We are initializing the GDT and TSS for the first time.
    unsafe {
        // Set up the GDT entries
        // Null descriptor (index 0) is already zero.

        // Kernel 64-bit code segment: base=0, limit=0, flags: 0x0020980000000000
        GDT[1] = 0x0020980000000000;

        // Kernel 64-bit data segment: base=0, limit=0, flags: 0x0000920000000000
        GDT[2] = 0x0000920000000000;

        // Set up the TSS
        let tss_base = core::ptr::addr_of!(TSS) as u64;
        let tss_limit = (size_of::<TaskStateSegment>() - 1) as u16;

        // Low part of the TSS descriptor (8 bytes)
        let tss_low = 
            (tss_limit as u64) |
            (tss_base & 0x00FF_FFFF) << 16 |
            0x89_u64 << 40 |
            (tss_base >> 24 & 0xFF) << 56;

        // High part of the TSS descriptor (8 bytes)
        let tss_high = 
            tss_base >> 32 & 0xFFFF_FFFF;

        GDT[3] = tss_low;
        GDT[4] = tss_high;

        // Set up the IST1 stack in the TSS
        TSS.ist1 = (core::ptr::addr_of!(IST1_STACK) as u64) + size_of::<[u8; 0x4000]>();

        // Load the GDT
        let gdt_ptr = DescriptorTablePointer {
            limit: (size_of::<[u64; 5]>() - 1) as u16,
            base: core::ptr::addr_of!(GDT) as u64,
        };
        load_gdt(&gdt_ptr);

        // Reload data segments and then code segment via far jump
        reload_segments(0x08, 0x10); // code selector=0x08, data selector=0x10

        // Load the TSS
        load_tss(0x18); // TSS selector=0x18
    }
}
