#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod desktop;
mod fb;
mod font;
mod gdt;
mod idt;
mod rtc;
mod serial;

use core::panic::PanicInfo;

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker, StackSizeRequest};

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(0x100000);

/// The screen: Limine sets a graphics mode and hands over its framebuffer.
#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

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

    // The IDT answers: the breakpoint handler prints its line and returns.
    // SAFETY: int3 only raises the breakpoint exception, which idt::init has
    // just given a handler.
    unsafe {
        core::arch::asm!("int3", options(nomem, nostack, preserves_flags));
    }

    match FRAMEBUFFER_REQUEST.get_response().and_then(|response| response.framebuffers().next()) {
        Some(frame) => {
            let mode = fb::Mode {
                width: frame.width() as usize,
                height: frame.height() as usize,
                pitch: frame.pitch() as usize,
                bits_per_pixel: usize::from(frame.bpp()),
                red_shift: frame.red_mask_shift(),
                green_shift: frame.green_mask_shift(),
                blue_shift: frame.blue_mask_shift(),
            };
            // SAFETY: Limine maps the framebuffer it describes, height rows of
            // pitch bytes, writable for as long as the kernel runs.
            let mut screen = unsafe { fb::Screen::new(frame.addr(), mode) };
            desktop::draw(&mut screen, rtc::time());
            serial::write_str("desktop: drawn\n");
        }
        None => serial::write_str("desktop: Limine gave no framebuffer\n"),
    }

    loop {
        // SAFETY: hlt waits for an interrupt; with none enabled yet, the CPU
        // rests with the desktop on screen.
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
