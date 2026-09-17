//! FAT32, just enough to update grenOS in place: find our partition, find a
//! file by its path, read it, and overwrite it inside the clusters it already
//! has (docs/specs/disk-and-updates.md §4).
//!
//! It never allocates a cluster, never changes a directory entry and never
//! touches the FATs: an overwrite that would grow a file is refused. That is
//! rule 2 of the spec, and it is what keeps a half-finished write from
//! corrupting the file system itself.
//!
//! No hardware here: everything goes through `Blocks`, so the module is tested
//! on the host against the disk image the CI builds with mtools.

use alloc::string::String;
use alloc::vec::Vec;

pub const SECTOR: usize = 512;

/// Sectors in one read or write: 64 KiB, the AHCI driver's bounce buffer.
const RUN_SECTORS: u64 = 128;
/// The largest file `read` returns whole; bigger ones go through `each_chunk`.
const READ_LIMIT: u32 = 1 << 20;
/// FAT entries at or above this end a chain.
const END_OF_CHAIN: u32 = 0x0FFF_FFF8;

pub trait Blocks {
    fn read(&mut self, lba: u64, out: &mut [u8]) -> Result<(), &'static str>;
    fn write(&mut self, lba: u64, data: &[u8]) -> Result<(), &'static str>;
}

/// The MBR of a disk: its signature, and partition 1's first sector.
pub struct Mbr {
    pub signature: u32,
    pub start: u64,
}

/// Reads the MBR out of sector 0, or None when it is not one.
pub fn mbr(sector: &[u8]) -> Option<Mbr> {
    if sector.len() < SECTOR || sector[510] != 0x55 || sector[511] != 0xAA {
        return None;
    }
    let u32_at = |at: usize| u32::from_le_bytes([sector[at], sector[at + 1], sector[at + 2], sector[at + 3]]);
    let entry = 446;
    let start = u64::from(u32_at(entry + 8));
    let sectors = u64::from(u32_at(entry + 12));
    if start == 0 || sectors == 0 {
        return None;
    }
    Some(Mbr { signature: u32_at(440), start })
}

/// A file or directory found by `find`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub cluster: u32,
    pub size: u32,
    pub directory: bool,
}

pub struct Volume {
    sectors_per_cluster: u64,
    fat_start: u64,
    data_start: u64,
    root: u32,
    clusters: u32,
    label: String,
}

impl Volume {
    /// Reads the FAT32 boot sector at `start_lba` and checks it is one.
    pub fn open(disk: &mut impl Blocks, start_lba: u64) -> Result<Volume, &'static str> {
        let mut boot = [0u8; SECTOR];
        disk.read(start_lba, &mut boot)?;
        if boot[510] != 0x55 || boot[511] != 0xAA {
            return Err("the partition has no boot sector");
        }
        let u16_at = |at: usize| u16::from_le_bytes([boot[at], boot[at + 1]]);
        let u32_at = |at: usize| u32::from_le_bytes([boot[at], boot[at + 1], boot[at + 2], boot[at + 3]]);
        let bytes_per_sector = u16_at(0x0B);
        let sectors_per_cluster = boot[0x0D];
        let reserved = u16_at(0x0E);
        let fats = boot[0x10];
        let fat16_size = u16_at(0x16);
        let total = u32_at(0x20);
        let fat_size = u32_at(0x24);
        let root = u32_at(0x2C);
        if usize::from(bytes_per_sector) != SECTOR {
            return Err("the file system does not use 512-byte sectors");
        }
        if sectors_per_cluster == 0 || !sectors_per_cluster.is_power_of_two() {
            return Err("the cluster size is not a power of two");
        }
        if reserved == 0 || fats == 0 || fat16_size != 0 || fat_size == 0 || root < 2 {
            return Err("this is not a FAT32 file system");
        }
        let overhead = u64::from(reserved) + u64::from(fats) * u64::from(fat_size);
        let clusters = u64::from(total).checked_sub(overhead).ok_or("the FATs are larger than the partition")?
            / u64::from(sectors_per_cluster);
        let mut volume = Volume {
            sectors_per_cluster: u64::from(sectors_per_cluster),
            fat_start: start_lba + u64::from(reserved),
            data_start: start_lba + overhead,
            root,
            clusters: u32::try_from(clusters).map_err(|_| "too many clusters")?,
            label: String::from_utf8_lossy(&boot[0x47..0x52]).trim_end().into(),
        };
        // mformat -v does not say where it puts the label: the boot sector, or
        // an entry of the root directory. Either counts.
        if volume.label.is_empty() || volume.label == "NO NAME" {
            if let Some(label) = volume.root_label(disk)? {
                volume.label = label;
            }
        }
        if root >= volume.clusters + 2 {
            return Err("the root directory is outside the partition");
        }
        Ok(volume)
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    fn cluster_bytes(&self) -> usize {
        self.sectors_per_cluster as usize * SECTOR
    }

    fn lba_of(&self, cluster: u32) -> u64 {
        self.data_start + u64::from(cluster - 2) * self.sectors_per_cluster
    }

    /// Every cluster of the chain that starts at `first`, in order. Refuses a
    /// cluster outside the partition and a chain longer than the partition —
    /// the shape a loop takes in a damaged FAT.
    fn chain(&self, disk: &mut impl Blocks, first: u32) -> Result<Vec<u32>, &'static str> {
        let mut chain = Vec::new();
        let mut cluster = first;
        let mut cached: Option<(u64, [u8; SECTOR])> = None;
        while cluster < END_OF_CHAIN {
            if cluster < 2 || cluster >= self.clusters + 2 {
                return Err("a cluster chain leaves the partition");
            }
            if chain.len() > self.clusters as usize {
                return Err("a cluster chain loops");
            }
            chain.push(cluster);
            let offset = u64::from(cluster) * 4;
            let lba = self.fat_start + offset / SECTOR as u64;
            let sector = match cached {
                Some((at, sector)) if at == lba => sector,
                _ => {
                    let mut sector = [0u8; SECTOR];
                    disk.read(lba, &mut sector)?;
                    cached = Some((lba, sector));
                    sector
                }
            };
            let at = (offset % SECTOR as u64) as usize;
            cluster = u32::from_le_bytes([sector[at], sector[at + 1], sector[at + 2], sector[at + 3]]) & 0x0FFF_FFFF;
        }
        Ok(chain)
    }

    /// Consecutive clusters grouped, at most RUN_SECTORS sectors a group:
    /// (first sector, sectors).
    fn runs(&self, chain: &[u32]) -> Vec<(u64, u64)> {
        let per_run = (RUN_SECTORS / self.sectors_per_cluster).max(1);
        let mut runs: Vec<(u64, u64, u32, u64)> = Vec::new(); // lba, sectors, last cluster, clusters
        for &cluster in chain {
            match runs.last_mut() {
                Some((_, sectors, last, count)) if *last + 1 == cluster && *count < per_run => {
                    *sectors += self.sectors_per_cluster;
                    *last = cluster;
                    *count += 1;
                }
                _ => runs.push((self.lba_of(cluster), self.sectors_per_cluster, cluster, 1)),
            }
        }
        runs.into_iter().map(|(lba, sectors, _, _)| (lba, sectors)).collect()
    }

    /// The contents of a directory, as (name, entry) pairs.
    fn list(&self, disk: &mut impl Blocks, cluster: u32) -> Result<Vec<(String, Entry)>, &'static str> {
        let mut bytes = Vec::new();
        for (lba, sectors) in self.runs(&self.chain(disk, cluster)?) {
            let at = bytes.len();
            bytes.resize(at + sectors as usize * SECTOR, 0);
            disk.read(lba, &mut bytes[at..])?;
        }
        let mut found = Vec::new();
        let mut long: Vec<(u8, [u16; 13])> = Vec::new();
        for raw in bytes.chunks_exact(32) {
            let attr = raw[11];
            match raw[0] {
                0x00 => break,
                0xE5 => {
                    long.clear();
                    continue;
                }
                _ => {}
            }
            if attr == 0x0F {
                let mut part = [0u16; 13];
                let positions = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
                for (unit, at) in part.iter_mut().zip(positions) {
                    *unit = u16::from_le_bytes([raw[at], raw[at + 1]]);
                }
                long.push((raw[0] & 0x1F, part));
                continue;
            }
            if attr & 0x08 != 0 {
                long.clear();
                continue;
            }
            let name = if long.is_empty() { short_name(raw) } else { long_name(&mut long) };
            long.clear();
            if name == "." || name == ".." {
                continue;
            }
            let cluster = u32::from(u16::from_le_bytes([raw[20], raw[21]])) << 16
                | u32::from(u16::from_le_bytes([raw[26], raw[27]]));
            let size = u32::from_le_bytes([raw[28], raw[29], raw[30], raw[31]]);
            found.push((name, Entry { cluster, size, directory: attr & 0x10 != 0 }));
        }
        Ok(found)
    }

    /// The label entry of the root directory, if it has one.
    fn root_label(&self, disk: &mut impl Blocks) -> Result<Option<String>, &'static str> {
        let mut sector = [0u8; SECTOR];
        disk.read(self.lba_of(self.root), &mut sector)?;
        for raw in sector.chunks_exact(32) {
            if raw[0] == 0 {
                break;
            }
            if raw[0] != 0xE5 && raw[11] & 0x08 != 0 && raw[11] != 0x0F {
                return Ok(Some(String::from_utf8_lossy(&raw[..11]).trim_end().into()));
            }
        }
        Ok(None)
    }

    /// The file or directory at `path` ("/boot/kernel-a"), names compared
    /// without regard to case.
    pub fn find(&self, disk: &mut impl Blocks, path: &str) -> Result<Entry, &'static str> {
        let mut current = Entry { cluster: self.root, size: 0, directory: true };
        for part in path.split('/').filter(|part| !part.is_empty()) {
            if !current.directory {
                return Err("a path goes through a file");
            }
            current = self
                .list(disk, current.cluster)?
                .into_iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(part))
                .map(|(_, entry)| entry)
                .ok_or("no such file")?;
        }
        Ok(current)
    }

    /// Feeds the first `limit` bytes of a file (at most its size) to `take`,
    /// 64 KiB at a time: how a kernel slot of 8 MiB is hashed without holding
    /// it in the heap.
    pub fn each_chunk(
        &self,
        disk: &mut impl Blocks,
        entry: &Entry,
        limit: usize,
        mut take: impl FnMut(&[u8]),
    ) -> Result<(), &'static str> {
        if entry.directory {
            return Err("a directory is not a file");
        }
        let mut left = limit.min(entry.size as usize);
        if left == 0 {
            return Ok(());
        }
        let mut buffer = alloc::vec![0u8; RUN_SECTORS as usize * SECTOR];
        for (lba, sectors) in self.runs(&self.chain(disk, entry.cluster)?) {
            let bytes = sectors as usize * SECTOR;
            disk.read(lba, &mut buffer[..bytes])?;
            let used = bytes.min(left);
            take(&buffer[..used]);
            left -= used;
            if left == 0 {
                return Ok(());
            }
        }
        Err("the file's clusters end before its size")
    }

    /// A whole file, up to 1 MiB.
    pub fn read(&self, disk: &mut impl Blocks, entry: &Entry) -> Result<Vec<u8>, &'static str> {
        if entry.size > READ_LIMIT {
            return Err("the file is too large to read whole");
        }
        let mut out = Vec::with_capacity(entry.size as usize);
        self.each_chunk(disk, entry, entry.size as usize, |chunk| out.extend_from_slice(chunk))?;
        Ok(out)
    }

    /// Writes `data` over the file from its start and zeroes the rest of its
    /// clusters. Refused when `data` is larger than the file: the file never
    /// grows, no cluster is allocated, no directory entry changes.
    pub fn overwrite(&self, disk: &mut impl Blocks, entry: &Entry, data: &[u8]) -> Result<(), &'static str> {
        if entry.directory {
            return Err("a directory is not a file");
        }
        if data.len() > entry.size as usize {
            return Err("the new contents are larger than the file, which must not grow");
        }
        let chain = self.chain(disk, entry.cluster)?;
        let needed = (entry.size as usize).div_ceil(self.cluster_bytes());
        if chain.len() < needed {
            return Err("the file's clusters end before its size");
        }
        let mut buffer = alloc::vec![0u8; RUN_SECTORS as usize * SECTOR];
        let mut written = 0usize;
        for (lba, sectors) in self.runs(&chain[..needed]) {
            let bytes = sectors as usize * SECTOR;
            let piece = &mut buffer[..bytes];
            piece.fill(0);
            let from = written.min(data.len());
            let to = (written + bytes).min(data.len());
            piece[..to - from].copy_from_slice(&data[from..to]);
            disk.write(lba, piece)?;
            written += bytes;
        }
        Ok(())
    }
}

/// An 8.3 name as it is shown: "KERNEL-A", "LIMINE.CON", lower case when the
/// entry's case flags say so.
fn short_name(raw: &[u8]) -> String {
    let lower_base = raw[12] & 0x08 != 0;
    let lower_extension = raw[12] & 0x10 != 0;
    let piece = |bytes: &[u8], lower: bool| -> String {
        let text: String = bytes.iter().map(|&b| char::from(b)).collect::<String>().trim_end().into();
        if lower { text.to_ascii_lowercase() } else { text }
    };
    let base = piece(&raw[0..8], lower_base);
    let extension = piece(&raw[8..11], lower_extension);
    if extension.is_empty() { base } else { alloc::format!("{base}.{extension}") }
}

/// A long name from its pieces, which the directory stores last first.
fn long_name(parts: &mut [(u8, [u16; 13])]) -> String {
    parts.sort_by_key(|(order, _)| *order);
    let units: Vec<u16> =
        parts.iter().flat_map(|(_, part)| part.iter().copied()).take_while(|&unit| unit != 0x0000).collect();
    char::decode_utf16(units).map(|c| c.unwrap_or('?')).collect()
}
