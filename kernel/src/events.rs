//! What the interrupt handlers hand to the main loop: a ring of events the
//! handlers write and the loop reads, with atomics only. Handlers run with
//! interrupts off, so there is one writer at a time, and one reader.

use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

#[derive(Clone, Copy)]
pub enum Event {
    /// A second has passed on the timer.
    Second,
    /// A byte from the keyboard: a scancode of set 1.
    Key(u8),
    /// A byte from the mouse: a third of a packet.
    Mouse(u8),
}

const SIZE: usize = 512;
static RING: [AtomicU32; SIZE] = [const { AtomicU32::new(0) }; SIZE];
static HEAD: AtomicUsize = AtomicUsize::new(0);
static TAIL: AtomicUsize = AtomicUsize::new(0);
static TICKS: AtomicU64 = AtomicU64::new(0);
static LOST: AtomicU32 = AtomicU32::new(0);

fn encode(event: Event) -> u32 {
    match event {
        Event::Second => 1 << 8,
        Event::Key(byte) => 2 << 8 | u32::from(byte),
        Event::Mouse(byte) => 3 << 8 | u32::from(byte),
    }
}

fn decode(word: u32) -> Option<Event> {
    let byte = (word & 0xFF) as u8;
    match word >> 8 {
        1 => Some(Event::Second),
        2 => Some(Event::Key(byte)),
        3 => Some(Event::Mouse(byte)),
        _ => None,
    }
}

/// Adds an event, from an interrupt handler. A full ring drops it and counts
/// it: a handler never waits.
pub fn push(event: Event) {
    let head = HEAD.load(Ordering::Relaxed);
    let next = (head + 1) % SIZE;
    if next == TAIL.load(Ordering::Acquire) {
        LOST.fetch_add(1, Ordering::Relaxed);
        return;
    }
    RING[head].store(encode(event), Ordering::Relaxed);
    HEAD.store(next, Ordering::Release);
}

/// The oldest event, for the main loop.
pub fn pop() -> Option<Event> {
    let tail = TAIL.load(Ordering::Relaxed);
    if tail == HEAD.load(Ordering::Acquire) {
        return None;
    }
    let word = RING[tail].load(Ordering::Relaxed);
    TAIL.store((tail + 1) % SIZE, Ordering::Release);
    decode(word)
}

pub fn is_empty() -> bool {
    TAIL.load(Ordering::Acquire) == HEAD.load(Ordering::Acquire)
}

/// Events dropped because the loop was too slow. Anything but zero means the
/// mouse decoder lost its place at some point.
pub fn lost() -> u32 {
    LOST.load(Ordering::Relaxed)
}

/// Timer ticks since boot, at [`crate::pit::HZ`] a second.
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// From the timer handler, HZ times a second.
pub fn tick() {
    let ticks = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
    if ticks % u64::from(crate::pit::HZ) == 0 {
        push(Event::Second);
    }
}
