#![no_std]
#![no_main]

mod serial;
mod gdt;
mod idt;

use core::panic::PanicInfo;

use limine::BaseRevision;
use limine::request::{RequestsEndMarker, RequestsStartMarker, StackSizeRequest};

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(0x100000);

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
extern "C" fn kmain() -> ! {
    assert!(BASE_REVISION.is_supported());

    serial::init();
    gdt::init();
    idt::init();

    serial::write_str("grenOS\n");

    // Trigger a breakpoint exception
    unsafe { core::arch::asm!("int3"); }

    // Trigger a page fault by accessing an unmapped address
    // We choose an address in the higher half that is likely not mapped: 2MiB above the kernel base
    let ptr = 0xffffffff80000000 + 0x200000 as *const u8;
    let _ = unsafe { *ptr }; // This should cause a page fault

    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

#[panic_handler]
fn rust_panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}
