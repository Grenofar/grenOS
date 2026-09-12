//! The kernel heap: `alloc`'s Box, Vec and String, on SIZE bytes of
//! contiguous RAM reached through the HHDM, managed by linked_list_allocator.

use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub const SIZE: usize = 4 * 1024 * 1024;

/// Hands `start`, the virtual address of SIZE bytes, to the allocator.
///
/// # Safety
///
/// Those bytes must be RAM that nothing else uses, now or later, and this
/// must be called once.
pub unsafe fn init(start: u64) {
    // SAFETY: the caller hands the memory over, once.
    unsafe { ALLOCATOR.lock().init(start as *mut u8, SIZE) };
}

/// Bytes the heap currently hands out.
pub fn used() -> usize {
    ALLOCATOR.lock().used()
}
