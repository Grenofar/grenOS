//! Physical memory: the map Limine hands over, and a frame allocator on its
//! usable parts. Frames are 4 KiB. The free ones form a stack: each holds the
//! address of the next, written through the HHDM, Limine's window on all of
//! physical memory.

use core::ops::Range;

use limine::memory_map::{Entry, EntryType};

pub const FRAME: u64 = 4096;

pub struct Frames {
    hhdm: u64,
    head: u64,
    free: u64,
    total: u64,
}

impl Frames {
    /// Every usable frame of `map`, except those in `keep`.
    ///
    /// # Safety
    ///
    /// `hhdm` must be Limine's HHDM offset, and the usable regions of `map`
    /// free RAM that the HHDM maps.
    pub unsafe fn new(map: &[&Entry], hhdm: u64, keep: Range<u64>) -> Self {
        let mut frames = Frames { hhdm, head: 0, free: 0, total: 0 };
        for entry in map.iter().filter(|entry| entry.entry_type == EntryType::USABLE) {
            let start = entry.base.div_ceil(FRAME) * FRAME;
            let end = (entry.base + entry.length) / FRAME * FRAME;
            for frame in (start..end).step_by(FRAME as usize) {
                if frame != 0 && !keep.contains(&frame) {
                    // SAFETY: a usable frame, mapped in the HHDM, handed to no one.
                    unsafe { frames.push(frame) };
                }
            }
        }
        frames.total = frames.free;
        frames
    }

    /// # Safety
    ///
    /// `frame` must be free, and stay untouched until `allocate` returns it.
    unsafe fn push(&mut self, frame: u64) {
        // SAFETY: the caller hands the frame over; its first 8 bytes keep the list.
        unsafe { ((frame + self.hhdm) as *mut u64).write_volatile(self.head) };
        self.head = frame;
        self.free += 1;
    }

    /// A zeroed frame, or None when none is left.
    pub fn allocate(&mut self) -> Option<u64> {
        if self.head == 0 {
            return None;
        }
        let frame = self.head;
        let bytes = (frame + self.hhdm) as *mut u8;
        // SAFETY: the frame is on the free list, so it is ours, and the HHDM
        // maps it; its first 8 bytes hold the next free frame.
        unsafe {
            self.head = bytes.cast::<u64>().read_volatile();
            core::ptr::write_bytes(bytes, 0, FRAME as usize);
        }
        self.free -= 1;
        Some(frame)
    }

    /// Gives back a frame `allocate` returned.
    ///
    /// # Safety
    ///
    /// Nothing may use `frame` any more.
    pub unsafe fn deallocate(&mut self, frame: u64) {
        // SAFETY: the caller no longer uses the frame.
        unsafe { self.push(frame) }
    }

    pub fn free(&self) -> u64 {
        self.free
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn hhdm(&self) -> u64 {
        self.hhdm
    }
}

/// The largest usable region of `map`, as (base, length).
pub fn largest_usable(map: &[&Entry]) -> Option<(u64, u64)> {
    map.iter()
        .filter(|entry| entry.entry_type == EntryType::USABLE)
        .max_by_key(|entry| entry.length)
        .map(|entry| (entry.base, entry.length))
}
