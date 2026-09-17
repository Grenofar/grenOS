//! grenOS's own disk: the one it booted from, found and proven writable
//! (docs/specs/disk-and-updates.md §4.6).
//!
//! Rule 1 of the spec lives here. A PC can have Windows on another disk, and
//! a write to the wrong one destroys it, so a disk is ours only when both hold:
//! its MBR signature is the one Limine reports for the kernel file it loaded,
//! and its first partition is FAT32 labelled GRENOS. Anything else is refused,
//! and nothing is ever written to it.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ahci::Disk;
use crate::fat::{self, Stamp, Volume};
use crate::settings;

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

/// The settings file as text, empty when the disk has none yet.
pub fn load_settings(store: &Store, disks: &mut [Disk]) -> Result<String, &'static str> {
    let disk = disks.get_mut(store.disk).ok_or("the disk is gone")?;
    match store.volume.find(disk, settings::PATH) {
        Ok(entry) if entry.directory || entry.size as usize > settings::LIMIT => {
            Err("the settings file is not a file, or is larger than settings can be")
        }
        Ok(entry) => Ok(String::from_utf8_lossy(&store.volume.read(disk, &entry)?).into_owned()),
        // No file yet: this disk has never kept anything. Not a failure.
        Err(_) => Ok(String::new()),
    }
}

/// Writes the settings file, makes the disk keep it, and reads it back.
pub fn save_settings(store: &Store, disks: &mut [Disk], text: &str, stamp: Stamp) -> Result<(), &'static str> {
    if text.len() > settings::LIMIT {
        return Err("the settings are larger than their file may be");
    }
    let disk = disks.get_mut(store.disk).ok_or("the disk is gone")?;
    store.volume.write_file(disk, settings::PATH, text.as_bytes(), stamp)?;
    disk.flush()?;
    let entry = store.volume.find(disk, settings::PATH)?;
    if store.volume.read(disk, &entry)? != text.as_bytes() {
        return Err("what was read back differs from what was written");
    }
    Ok(())
}

/// Counts this start in the settings file and gives the new count: the proof,
/// readable on the serial line, that what grenOS writes survives a restart.
pub fn count_boot(store: &Store, disks: &mut [Disk], stamp: Stamp) -> Result<u32, &'static str> {
    let text = load_settings(store, disks)?;
    let count = settings::number(&text, "demarrages").unwrap_or(0).saturating_add(1);
    save_settings(store, disks, &settings::set(&text, "demarrages", &format!("{count}")), stamp)?;
    Ok(count)
}

/// Makes a directory, writes a file with a long name in it, reads it back and
/// removes both: the allocating writes proven on the real disk, at every
/// start, and the partition left exactly as it was found.
pub fn create_and_remove(store: &Store, disks: &mut [Disk], stamp: Stamp) -> Result<(), &'static str> {
    const DIRECTORY: &str = "/grenos/essai-dossier";
    let file = format!("{DIRECTORY}/un fichier créé par grenOS.txt");
    let disk = disks.get_mut(store.disk).ok_or("the disk is gone")?;
    let written = format!("écrit au démarrage numéro {}", stamp.time);
    store.volume.make_dir(disk, DIRECTORY, stamp)?;
    store.volume.write_file(disk, &file, written.as_bytes(), stamp)?;
    disk.flush()?;
    let entry = store.volume.find(disk, &file)?;
    if store.volume.read(disk, &entry)? != written.as_bytes() {
        return Err("the file read back differs from what was written");
    }
    store.volume.remove(disk, &file)?;
    store.volume.remove(disk, DIRECTORY)?;
    disk.flush()?;
    if store.volume.find(disk, DIRECTORY).is_ok() {
        return Err("the directory is still there after being removed");
    }
    Ok(())
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
