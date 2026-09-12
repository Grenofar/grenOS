//! Four-level paging: a 4 KiB page mapped wherever the kernel wants it, with
//! the page tables reached through the HHDM and new ones made from frames.

use crate::memory::Frames;

const PRESENT: u64 = 1;
const WRITABLE: u64 = 1 << 1;
const HUGE: u64 = 1 << 7;
const ADDRESS: u64 = 0x000F_FFFF_FFFF_F000;

/// The page table entry for `virt` at `level` (3: PML4, 2: PDPT, 1: PD,
/// 0: PT) in `table`, a physical address.
fn entry(table: u64, virt: u64, level: u32, hhdm: u64) -> *mut u64 {
    let index = (virt >> (12 + 9 * level)) & 0x1FF;
    (table + hhdm + index * 8) as *mut u64
}

/// Maps the page at `virt` to `frame`, present and writable, and makes the
/// tables it needs on the way.
///
/// # Safety
///
/// Nothing may rely on what is mapped at `virt` now, and `frame` must be a
/// frame nothing else uses.
pub unsafe fn map(frames: &mut Frames, virt: u64, frame: u64) -> Result<(), &'static str> {
    // SAFETY: the caller vouches for the address and the frame.
    unsafe { put(frames, virt, frame, false) }
}

/// Maps the page at `virt` to `frame` unless that mapping is already there,
/// which is not an error: firmware tables span pages, and two of them can
/// share one.
///
/// # Safety
///
/// `virt` must be in a window the kernel keeps for this purpose, and `frame`
/// a physical frame that is read, not owned — an ACPI table, say. Mapping it
/// makes an alias, which is why the mapping is read through volatile reads
/// and never handed out as a reference.
pub unsafe fn map_shared(frames: &mut Frames, virt: u64, frame: u64) -> Result<(), &'static str> {
    // SAFETY: as above; an identical mapping already in place is accepted.
    unsafe { put(frames, virt, frame, true) }
}

/// # Safety
///
/// As [`map`]; `again` accepts a page already mapped to the same frame.
unsafe fn put(frames: &mut Frames, virt: u64, frame: u64, again: bool) -> Result<(), &'static str> {
    let hhdm = frames.hhdm();
    let cr3: u64;
    // SAFETY: reading CR3 has no effect.
    unsafe { core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags)) };
    let mut table = cr3 & ADDRESS;
    for level in [3, 2, 1] {
        let slot = entry(table, virt, level, hhdm);
        // SAFETY: `table` is a page table, which the HHDM maps.
        let value = unsafe { slot.read_volatile() };
        if value & PRESENT == 0 {
            let fresh = frames.allocate().ok_or("no frame left for a page table")?;
            // SAFETY: an empty slot, pointed at a zeroed frame of our own.
            unsafe { slot.write_volatile(fresh | PRESENT | WRITABLE) };
            table = fresh;
        } else if value & HUGE != 0 {
            return Err("a huge page already covers this address");
        } else {
            table = value & ADDRESS;
        }
    }
    let slot = entry(table, virt, 0, hhdm);
    // SAFETY: the last table, which the HHDM maps; the caller vouches for the
    // address and the frame, and invlpg drops any stale translation.
    unsafe {
        let current = slot.read_volatile();
        if current & PRESENT != 0 {
            if again && current & ADDRESS == frame {
                return Ok(());
            }
            return Err("the page is already mapped");
        }
        slot.write_volatile(frame | PRESENT | WRITABLE);
        core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
    }
    Ok(())
}
