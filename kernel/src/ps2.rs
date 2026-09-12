//! The PS/2 controller: the keyboard on its first port, the mouse on its
//! second. Every byte either raises an interrupt (IRQ1, IRQ12) or is picked up
//! by the timer, and the status register — not the line it came in on — says
//! which device sent it.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::events::{self, Event};
use crate::port::{inb, outb};

const DATA: u16 = 0x60;
const STATUS_COMMAND: u16 = 0x64;

/// Status bits: a byte waits in the output buffer; the input buffer is still
/// busy; the waiting byte came from the mouse and not the keyboard.
const OUTPUT_FULL: u8 = 0x01;
const INPUT_FULL: u8 = 0x02;
const FROM_MOUSE: u8 = 0x20;

/// Which handler is asking for the waiting byte.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum From {
    Keyboard,
    Mouse,
    /// The timer, sweeping up a byte whose interrupt never came.
    Timer,
}

static KEY_IRQ: AtomicU32 = AtomicU32::new(0);
static MOUSE_IRQ: AtomicU32 = AtomicU32::new(0);
static SWEPT: AtomicU32 = AtomicU32::new(0);
static KEY_BYTES: AtomicU32 = AtomicU32::new(0);
static MOUSE_BYTES: AtomicU32 = AtomicU32::new(0);

/// What the controller has done since boot. Shown in the panel and in
/// Paramètres: on a machine where the mouse stays still, these numbers say
/// whether the bytes arrive at all, and by which road.
#[derive(Clone, Copy, Default)]
pub struct Counts {
    pub key_irq: u32,
    pub mouse_irq: u32,
    pub swept: u32,
    pub key_bytes: u32,
    pub mouse_bytes: u32,
}

pub fn counts() -> Counts {
    Counts {
        key_irq: KEY_IRQ.load(Ordering::Relaxed),
        mouse_irq: MOUSE_IRQ.load(Ordering::Relaxed),
        swept: SWEPT.load(Ordering::Relaxed),
        key_bytes: KEY_BYTES.load(Ordering::Relaxed),
        mouse_bytes: MOUSE_BYTES.load(Ordering::Relaxed),
    }
}

fn status() -> u8 {
    // SAFETY: reading the status register changes nothing.
    unsafe { inb(STATUS_COMMAND) }
}

fn wait_input_empty() {
    for _ in 0..100_000 {
        if status() & INPUT_FULL == 0 {
            return;
        }
    }
}

fn wait_output_full() -> bool {
    (0..100_000).any(|_| status() & OUTPUT_FULL != 0)
}

fn command(byte: u8) {
    wait_input_empty();
    // SAFETY: a controller command; the ones sent here only configure it.
    unsafe { outb(STATUS_COMMAND, byte) };
}

fn write(byte: u8) {
    wait_input_empty();
    // SAFETY: a data byte, for the controller or, after 0xD4, the mouse.
    unsafe { outb(DATA, byte) };
}

fn read() -> Option<u8> {
    // SAFETY: the output buffer is full, so the data port holds a byte.
    wait_output_full().then(|| unsafe { inb(DATA) })
}

/// Sends `byte` to the mouse; true when it acknowledges with 0xFA.
fn to_mouse(byte: u8) -> bool {
    command(0xD4);
    write(byte);
    // The acknowledgement can be preceded by a keyboard byte, which the status
    // register lets us tell apart and drop.
    for _ in 0..4 {
        let state = status();
        if state & OUTPUT_FULL == 0 {
            if !wait_output_full() {
                return false;
            }
            continue;
        }
        // SAFETY: a byte waits in the output buffer.
        let answer = unsafe { inb(DATA) };
        if state & FROM_MOUSE != 0 {
            return answer == 0xFA;
        }
    }
    false
}

/// Turns both ports on with their interrupts, and asks the mouse to report
/// what it does. True when the mouse answered. Call with interrupts off.
pub fn init() -> bool {
    // Whatever the firmware left in the output buffer.
    while status() & OUTPUT_FULL != 0 {
        // SAFETY: the buffer is full; reading it empties it.
        unsafe { inb(DATA) };
    }
    command(0xA8); // the second port, the mouse's, on
    command(0x20); // read the configuration byte
    let config = read().unwrap_or(0x47);
    command(0x60); // write it back: IRQ1 and IRQ12 on, both clocks running
    write((config | 0x03) & !0x30);
    let ok = to_mouse(0xF6) && to_mouse(0xF4); // defaults, then report movement
    // Whatever the two commands left behind, so the first real packet starts
    // on its first byte.
    while status() & OUTPUT_FULL != 0 {
        // SAFETY: the buffer is full; reading it empties it.
        unsafe { inb(DATA) };
    }
    ok
}

/// Takes the byte the controller has ready, if any, and files it under the
/// device the status register names.
///
/// Called from IRQ1, from IRQ12 and from the timer. Reading blindly on the
/// strength of the line alone is what broke the mouse: an interrupt can arrive
/// for a byte already read, and the empty read then handed the decoder a stale
/// byte, which put every packet after it out of step. Bit 5 of the status
/// register never lies, and bit 0 says whether there is anything at all.
pub fn take(source: From) {
    match source {
        From::Keyboard => KEY_IRQ.fetch_add(1, Ordering::Relaxed),
        From::Mouse => MOUSE_IRQ.fetch_add(1, Ordering::Relaxed),
        From::Timer => 0,
    };
    let mut swept = false;
    // A sweep can find several bytes waiting; an interrupt, one.
    for _ in 0..8 {
        let state = status();
        if state & OUTPUT_FULL == 0 {
            break;
        }
        // SAFETY: the output buffer is full, so the data port holds a byte.
        let byte = unsafe { inb(DATA) };
        if state & FROM_MOUSE != 0 {
            MOUSE_BYTES.fetch_add(1, Ordering::Relaxed);
            events::push(Event::Mouse(byte));
        } else {
            KEY_BYTES.fetch_add(1, Ordering::Relaxed);
            events::push(Event::Key(byte));
        }
        if source == From::Timer && !swept {
            swept = true;
            SWEPT.fetch_add(1, Ordering::Relaxed);
        }
        if source != From::Timer {
            break;
        }
    }
}
