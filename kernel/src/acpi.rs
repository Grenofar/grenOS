//! ACPI: the firmware's tables, read once at boot for the one thing the
//! desktop needs from them — the registers that turn the machine off.
//!
//! Limine hands over the RSDP as a physical address (base revision 3), and its
//! higher-half map does not cover firmware memory, so the kernel maps every
//! table it reads into a window of its own and copies the bytes out. What
//! follows then works on plain slices.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::memory::{FRAME, Frames};
use crate::paging;

/// Where a table at physical address P is mapped: WINDOW + P.
const WINDOW: u64 = 0xFFFF_A000_0000_0000;

/// The registers that put the machine in its soft-off state, S5.
#[derive(Clone, Copy)]
pub struct Power {
    pub pm1a: u16,
    pub pm1b: u16,
    pub slp_typa: u16,
    pub slp_typb: u16,
    /// The port that asks the firmware for control of ACPI, 0 when there is none.
    pub smi: u16,
    pub enable: u8,
}

pub struct Acpi {
    pub revision: u8,
    pub oem: String,
    /// The signature of every table the root one points to.
    pub tables: Vec<String>,
    pub power: Option<Power>,
}

impl Acpi {
    /// One line for the boot log and for Paramètres.
    pub fn summary(&self) -> String {
        match self.power {
            Some(power) => format!(
                "ACPI {} ({}), {} tables, extinction par PM1a 0x{:04x} type {}",
                self.revision,
                self.oem,
                self.tables.len(),
                power.pm1a,
                power.slp_typa
            ),
            None => format!("ACPI {} ({}), {} tables, sans extinction", self.revision, self.oem, self.tables.len()),
        }
    }
}

fn u32le(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn u64le(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn text(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { ' ' }).collect::<String>().trim().into()
}

/// Copies `len` bytes of physical memory, mapping the pages they span.
///
/// # Safety
///
/// `phys` must be firmware memory: the mapping aliases it, and the copy reads
/// it, so nothing may be writing there.
unsafe fn snapshot(frames: &mut Frames, phys: u64, len: usize) -> Result<Vec<u8>, &'static str> {
    if phys == 0 || len == 0 || len > 4 * 1024 * 1024 {
        return Err("an ACPI table of an impossible size");
    }
    let first = phys / FRAME * FRAME;
    let last = (phys + len as u64 - 1) / FRAME * FRAME;
    for page in (first..=last).step_by(FRAME as usize) {
        // SAFETY: the window is the kernel's own, one virtual page per
        // physical one, and the caller vouches for the memory.
        unsafe { paging::map_shared(frames, WINDOW + page, page)? };
    }
    let mut out = vec![0u8; len];
    let source = (WINDOW + phys) as *const u8;
    for (i, byte) in out.iter_mut().enumerate() {
        // SAFETY: every page of the range has just been mapped.
        *byte = unsafe { source.add(i).read_volatile() };
    }
    Ok(out)
}

/// A whole table, header first (its length is in the header).
///
/// # Safety
///
/// As [`snapshot`].
unsafe fn table(frames: &mut Frames, phys: u64) -> Result<Vec<u8>, &'static str> {
    // SAFETY: the caller vouches for the address.
    let header = unsafe { snapshot(frames, phys, 36)? };
    let len = u32le(&header, 4).ok_or("a table header cut short")? as usize;
    if len < 36 {
        return Err("a table shorter than its own header");
    }
    // SAFETY: as above, now that the length is known.
    unsafe { snapshot(frames, phys, len) }
}

fn checksum(bytes: &[u8]) -> bool {
    bytes.iter().fold(0u8, |sum, &byte| sum.wrapping_add(byte)) == 0
}

/// Reads the tables from the RSDP Limine points at.
///
/// # Safety
///
/// `rsdp` must be the physical address Limine gave.
pub unsafe fn read(frames: &mut Frames, rsdp: u64) -> Result<Acpi, &'static str> {
    // SAFETY: the caller passes on Limine's own address.
    let head = unsafe { snapshot(frames, rsdp, 36)? };
    if !head.starts_with(b"RSD PTR ") {
        return Err("no RSDP where Limine said");
    }
    if !checksum(&head[..20]) {
        return Err("the RSDP checksum does not add up");
    }
    let revision = head[15];
    let rsdt = u64::from(u32le(&head, 16).unwrap_or(0));
    let xsdt = if revision >= 2 { u64le(&head, 24).unwrap_or(0) } else { 0 };
    let (root, width) = if xsdt != 0 { (xsdt, 8) } else { (rsdt, 4) };
    // SAFETY: an address out of the RSDP, which the firmware wrote.
    let root = unsafe { table(frames, root)? };
    let oem = text(&root[10..16]);

    let mut tables = Vec::new();
    let mut fadt = None;
    let mut extra = Vec::new();
    for entry in root[36..].chunks_exact(width) {
        let phys = match width {
            8 => u64::from_le_bytes(entry.try_into().unwrap_or([0; 8])),
            _ => u64::from(u32::from_le_bytes(entry.try_into().unwrap_or([0; 4]))),
        };
        // SAFETY: an address out of the root table, which the firmware wrote.
        let Ok(bytes) = (unsafe { table(frames, phys) }) else {
            continue;
        };
        let signature = text(&bytes[..4]);
        if signature == "FACP" {
            fadt = Some(bytes);
        } else if signature == "SSDT" {
            extra.push(bytes);
        }
        tables.push(signature);
    }

    // SAFETY: the DSDT address comes out of the FADT, which the firmware wrote.
    let power = match fadt {
        Some(fadt) => unsafe { power(frames, &fadt, &extra) },
        None => None,
    };
    Ok(Acpi { revision, oem, tables, power })
}

/// The PM1 registers and the S5 sleep type, from the FADT and the DSDT.
///
/// # Safety
///
/// `fadt` must be the firmware's own table.
unsafe fn power(frames: &mut Frames, fadt: &[u8], extra: &[Vec<u8>]) -> Option<Power> {
    let pm1a = u32le(fadt, 64)? as u16;
    if pm1a == 0 {
        return None;
    }
    let dsdt = match u64le(fadt, 140) {
        Some(long) if long != 0 && fadt.len() > 148 => long,
        _ => u64::from(u32le(fadt, 40)?),
    };
    // SAFETY: an address out of the FADT.
    let dsdt = unsafe { table(frames, dsdt) }.ok()?;
    let (slp_typa, slp_typb) = s5(&dsdt).or_else(|| extra.iter().find_map(|table| s5(table)))?;
    Some(Power {
        pm1a,
        pm1b: u32le(fadt, 68).unwrap_or(0) as u16,
        slp_typa,
        slp_typb,
        smi: u32le(fadt, 48).unwrap_or(0) as u16,
        enable: fadt.get(52).copied().unwrap_or(0),
    })
}

/// The two sleep types of `\_S5_`, found in the byte code of a table:
/// `NameOp _S5_ PackageOp <length> <count> <first> <second> ...`.
fn s5(aml: &[u8]) -> Option<(u16, u16)> {
    let at = aml.windows(4).position(|window| window == b"_S5_")?;
    let named = aml.get(at.checked_sub(1)?) == Some(&0x08)
        || (aml.get(at.checked_sub(2)?) == Some(&0x08) && aml.get(at - 1) == Some(&0x5C));
    if !named {
        return None;
    }
    let mut i = at + 4;
    if *aml.get(i)? != 0x12 {
        return None;
    }
    i += 1;
    // The package length: the top two bits of its first byte count the bytes
    // that follow it, then one byte for the number of elements.
    i += usize::from(*aml.get(i)? >> 6) + 2;
    let first = element(aml, &mut i)?;
    let second = element(aml, &mut i).unwrap_or(0);
    Some((first, second))
}

/// One element of an AML package: a byte behind its prefix, or a constant.
fn element(aml: &[u8], at: &mut usize) -> Option<u16> {
    let byte = *aml.get(*at)?;
    *at += 1;
    match byte {
        0x0A => {
            let value = *aml.get(*at)?;
            *at += 1;
            Some(u16::from(value))
        }
        0x00 => Some(0),
        0x01 => Some(1),
        other => Some(u16::from(other)),
    }
}
