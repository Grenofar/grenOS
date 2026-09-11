#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod desktop;
mod events;
mod fb;
mod font;
mod gdt;
mod idt;
mod keyboard;
mod mouse;
mod pic;
mod pit;
mod port;
mod ps2;
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
    unsafe { core::arch::asm!("int3", options(nomem, nostack, preserves_flags)) };

    pic::init();
    pit::init();
    let mouse_ok = ps2::init();
    // SAFETY: every vector the PIC can now raise has its handler in the IDT.
    unsafe { core::arch::asm!("sti", options(nomem, nostack)) };
    serial::write_str(if mouse_ok { "input: keyboard and mouse\n" } else { "input: keyboard, no mouse\n" });

    let Some(frame) = FRAMEBUFFER_REQUEST.get_response().and_then(|response| response.framebuffers().next()) else {
        serial::write_str("desktop: Limine gave no framebuffer\n");
        halt();
    };
    let mode = fb::Mode {
        width: frame.width() as usize,
        height: frame.height() as usize,
        pitch: frame.pitch() as usize,
        bits_per_pixel: usize::from(frame.bpp()),
        red_shift: frame.red_mask_shift(),
        green_shift: frame.green_mask_shift(),
        blue_shift: frame.blue_mask_shift(),
    };
    // SAFETY: Limine maps the framebuffer it describes, height rows of pitch
    // bytes, writable for as long as the kernel runs.
    let mut screen = unsafe { fb::Screen::new(frame.addr(), mode) };
    let mut desktop = desktop::Desktop::new(&screen, rtc::time());
    desktop.draw_all(&mut screen);
    serial::write_str("desktop: drawn\n");

    let mut keyboard = keyboard::Keyboard::default();
    let mut mouse = mouse::Decoder::default();
    loop {
        while let Some(event) = events::pop() {
            match event {
                events::Event::Second => desktop.set_clock(&mut screen, rtc::time()),
                events::Event::Key(code) => {
                    if let Some(key) = keyboard.feed(code) {
                        desktop.key(&mut screen, key);
                    }
                }
                events::Event::Mouse(byte) => {
                    if let Some(packet) = mouse.feed(byte) {
                        desktop.mouse(&mut screen, packet);
                    }
                }
            }
        }
        idle();
    }
}

/// Waits for the next interrupt, unless one has already left an event.
fn idle() {
    // SAFETY: cli, check, then sti immediately followed by hlt: sti takes
    // effect after the next instruction, so an interrupt arriving after the
    // check still wakes the hlt, and no event waits for the one after it.
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
        if events::is_empty() {
            core::arch::asm!("sti", "hlt", options(nomem, nostack));
        } else {
            core::arch::asm!("sti", options(nomem, nostack));
        }
    }
}

fn halt() -> ! {
    loop {
        // SAFETY: hlt rests until the next interrupt.
        unsafe { core::arch::asm!("hlt", options(nomem, nostack, preserves_flags)) };
    }
}

#[panic_handler]
fn rust_panic(_info: &PanicInfo) -> ! {
    halt()
}
