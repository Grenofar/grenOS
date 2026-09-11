#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

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
    serial::write_str("grenOS\n");
    gdt::init();
    idt::init();
    
    // Trigger breakpoint (int 3)
    unsafe {
        core::arch::asm!("int3", options(nomem, nostack, preserves_flags));
    }

    // Initialize framebuffer and draw desktop
    if let Some(response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = response.framebuffers().next() {
            let mut display = crate::fb::Display::from_limine(framebuffer);
            crate::desktop::render_desktop(&mut display);
            serial::write_str("desktop\n");
        }
    }

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
