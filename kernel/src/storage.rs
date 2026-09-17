//! grenOS's own disk: the one it booted from, found and proven writable
//! (docs/specs/disk-and-updates.md §4.6).
//!
//! Rule 1 of the spec lives here. A PC can have Windows on another disk, and
//! a write to the wrong one destroys it, so a disk is ours only when both hold:
//! its MBR signature is the one Limine reports for the kernel file it loaded,
//! and its first partition is FAT32 labelled GRENOS. Anything else is refused,
//! and nothing is ever written to it.

use alloc::vec::Vec;

use crate::ahci::Disk;
use crate::fat::{self, Volume};

/// The disk grenOS booted from, and its file system.
pub struct Store {
    /// Index of the disk in the AHCI driver's list.
    pub disk: usize,
    pub volume: Volume,
    /// The kernel slot that booted: 'a' or 'b'.
    pub slot: char,
}

/// Which slot a kernel path names: "/boot/kernel-a" is 'a'. The ISO's
/// "/boot/kernel" names none.
pub fn slot_of(path: &str) -> Option<char> {
    if path.ends_with("/kernel-a") {
        Some('a')
    } else if path.ends_with("/kernel-b") {
        Some('b')
    } else {
        None
    }
}

/// Finds our disk among `disks`, from the path and the MBR signature Limine
/// gives for the kernel file.
pub fn locate(disks: &mut [Disk], path: &str, signature: Option<u32>) -> Result<Store, &'static str> {
    let slot = slot_of(path).ok_or("booted from the ISO, nothing to install to")?;
    let signature = signature.ok_or("the boot disk has no MBR signature")?;
    for (index, disk) in disks.iter_mut().enumerate() {
        let mut first = [0u8; fat::SECTOR];
        if disk.read(0, &mut first).is_err() {
            continue;
        }
        let Some(mbr) = fat::mbr(&first) else {
            continue;
        };
        if mbr.signature != signature {
            continue;
        }
        let volume = Volume::open(disk, mbr.start)?;
        if volume.label() != "GRENOS" {
            return Err("the boot disk's partition is not labelled GRENOS: nothing will be written to it");
        }
        return Ok(Store { disk: index, volume, slot });
    }
    Err("no disk carries the boot disk's MBR signature")
}

/// Overwrites /grenos/essai.bin with fresh random bytes, flushes, reads the
/// file back and compares. The file exists for this; nothing else is touched.
pub fn write_test(store: &Store, disks: &mut [Disk]) -> Result<(), &'static str> {
    let disk = disks.get_mut(store.disk).ok_or("the disk is gone")?;
    let entry = store.volume.find(disk, "/grenos/essai.bin")?;
    let seed = crate::rand::bytes();
    let pattern: Vec<u8> = (0..entry.size as usize).map(|i| seed[i % seed.len()] ^ (i as u8)).collect();
    store.volume.overwrite(disk, &entry, &pattern)?;
    disk.flush()?;
    let back = store.volume.read(disk, &entry)?;
    if back != pattern {
        return Err("what was read back differs from what was written");
    }
    Ok(())
}
