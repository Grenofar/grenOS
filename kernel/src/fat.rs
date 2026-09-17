//! FAT32: find our partition, find a file by its path, read it, overwrite it in
//! place (docs/specs/disk-and-updates.md §4) — and, for what the person keeps
//! (settings, documents), create, replace and remove files and directories.
//!
//! Two kinds of write, on purpose:
//! - `overwrite` never allocates a cluster, never changes a directory entry and
//!   never touches the FATs. The kernel slots and `limine.conf` are only ever
//!   written that way: rule 2 of the spec.
//! - `write_file`, `make_dir` and `remove` do allocate, in the order that keeps
//!   the file system whole if the machine stops half-way: data first, then the
//!   FAT entries of the new clusters (in every FAT), then the directory entry,
//!   and only then the old clusters are freed. Stopped before the directory
//!   entry, the old file is still there and the new clusters are merely lost;
//!   stopped after, the new file is there and the old clusters are lost. Lost
//!   clusters are what `fsck` reclaims; nothing points at garbage.
//!
//! No hardware here: everything goes through `Blocks`, so the module is tested
//! on the host against the disk image the CI builds with mtools, and the CI
//! runs `fsck.fat` on the image after the kernel has written to it.

use alloc::string::String;
use alloc::vec::Vec;

pub const SECTOR: usize = 512;

/// Sectors in one read or write: 64 KiB, the AHCI driver's bounce buffer.
const RUN_SECTORS: u64 = 128;
/// The largest file `read` returns whole; bigger ones go through `each_chunk`.
const READ_LIMIT: u32 = 1 << 20;
/// The largest file `write_file` takes: settings and documents, not images.
const WRITE_LIMIT: usize = 4 << 20;
/// FAT entries at or above this end a chain; what a new chain ends with.
const END_OF_CHAIN: u32 = 0x0FFF_FFF8;
const END_MARK: u32 = 0x0FFF_FFFF;
/// Directory entry attributes.
const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_ARCHIVE: u8 = 0x20;
const ATTR_LABEL: u8 = 0x08;
const ATTR_LONG: u8 = 0x0F;
/// Characters a long-name slot holds, and where they sit in its 32 bytes.
const LONG_CHARS: usize = 13;
const LONG_POSITIONS: [usize; LONG_CHARS] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];

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

/// When a file was written, as FAT stores it: the date packs year − 1980,
/// month and day; the time packs hours, minutes and seconds halved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub date: u16,
    pub time: u16,
}

impl Stamp {
    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Stamp {
        let year = year.clamp(1980, 2107) - 1980;
        Stamp {
            date: year << 9 | u16::from(month.clamp(1, 12)) << 5 | u16::from(day.clamp(1, 31)),
            time: u16::from(hour.min(23)) << 11 | u16::from(minute.min(59)) << 5 | u16::from(second.min(59) / 2),
        }
    }
}

pub struct Volume {
    sectors_per_cluster: u64,
    fat_start: u64,
    fat_size: u64,
    fats: u64,
    data_start: u64,
    root: u32,
    clusters: u32,
    /// The FSInfo sector, when the boot sector names a valid one.
    fsinfo: Option<u64>,
    label: String,
}

/// One entry of a directory as it sits on the disk: where its slots are, so
/// that it can be rewritten or removed.
struct Item {
    name: String,
    short: [u8; 11],
    entry: Entry,
    /// Index of its first slot (the first long-name slot, if any) and of its
    /// short entry, counted in 32-byte slots from the directory's start.
    first_slot: usize,
    slot: usize,
}

/// A directory read whole: its clusters, its bytes, its entries.
struct Directory {
    chain: Vec<u32>,
    bytes: Vec<u8>,
    items: Vec<Item>,
}

/// FAT sectors read and changed during one operation, written to every FAT
/// copy together at the end.
struct FatTable {
    sectors: Vec<(u64, [u8; SECTOR], bool)>,
}

impl FatTable {
    fn new() -> FatTable {
        FatTable { sectors: Vec::new() }
    }

    fn sector(&mut self, volume: &Volume, disk: &mut impl Blocks, index: u64) -> Result<usize, &'static str> {
        if let Some(at) = self.sectors.iter().position(|(i, _, _)| *i == index) {
            return Ok(at);
        }
        let mut bytes = [0u8; SECTOR];
        disk.read(volume.fat_start + index, &mut bytes)?;
        self.sectors.push((index, bytes, false));
        Ok(self.sectors.len() - 1)
    }

    fn get(&mut self, volume: &Volume, disk: &mut impl Blocks, cluster: u32) -> Result<u32, &'static str> {
        let offset = u64::from(cluster) * 4;
        let at = self.sector(volume, disk, offset / SECTOR as u64)?;
        let byte = (offset % SECTOR as u64) as usize;
        let raw = &self.sectors[at].1[byte..byte + 4];
        Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) & 0x0FFF_FFFF)
    }

    /// Sets a FAT entry, keeping its four reserved high bits as they were.
    fn set(&mut self, volume: &Volume, disk: &mut impl Blocks, cluster: u32, value: u32) -> Result<(), &'static str> {
        let offset = u64::from(cluster) * 4;
        let at = self.sector(volume, disk, offset / SECTOR as u64)?;
        let byte = (offset % SECTOR as u64) as usize;
        let slot = &mut self.sectors[at];
        let old = u32::from_le_bytes([slot.1[byte], slot.1[byte + 1], slot.1[byte + 2], slot.1[byte + 3]]);
        let new = (old & 0xF000_0000) | (value & 0x0FFF_FFFF);
        slot.1[byte..byte + 4].copy_from_slice(&new.to_le_bytes());
        slot.2 = true;
        Ok(())
    }

    /// Writes every changed sector into every FAT.
    fn flush(&mut self, volume: &Volume, disk: &mut impl Blocks) -> Result<(), &'static str> {
        for (index, bytes, dirty) in self.sectors.iter_mut() {
            if !*dirty {
                continue;
            }
            for copy in 0..volume.fats {
                disk.write(volume.fat_start + copy * volume.fat_size + *index, bytes)?;
            }
            *dirty = false;
        }
        Ok(())
    }
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
        let flags = u16_at(0x28);
        let root = u32_at(0x2C);
        let fsinfo = u16_at(0x30);
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
        let clusters = u32::try_from(clusters).map_err(|_| "too many clusters")?;
        // The FAT itself must have room for every cluster's entry.
        if u64::from(clusters) + 2 > u64::from(fat_size) * (SECTOR as u64 / 4) {
            return Err("the FAT is too small for the partition");
        }
        let mut volume = Volume {
            sectors_per_cluster: u64::from(sectors_per_cluster),
            fat_start: start_lba + u64::from(reserved),
            fat_size: u64::from(fat_size),
            // Bit 7 of the flags: only the FAT the low bits name is in use,
            // and the others are not kept in step. Writing is then refused.
            fats: if flags & 0x80 != 0 { 0 } else { u64::from(fats) },
            data_start: start_lba + overhead,
            root,
            clusters,
            fsinfo: (fsinfo != 0 && fsinfo != 0xFFFF && fsinfo < reserved).then(|| start_lba + u64::from(fsinfo)),
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
        if first == 0 {
            return Ok(chain);
        }
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

    /// A directory read whole, with where each entry's slots are.
    fn directory(&self, disk: &mut impl Blocks, cluster: u32) -> Result<Directory, &'static str> {
        let chain = self.chain(disk, cluster)?;
        let mut bytes = Vec::new();
        for (lba, sectors) in self.runs(&chain) {
            let at = bytes.len();
            bytes.resize(at + sectors as usize * SECTOR, 0);
            disk.read(lba, &mut bytes[at..])?;
        }
        let mut items = Vec::new();
        let mut long: Vec<(u8, [u16; LONG_CHARS])> = Vec::new();
        let mut long_start = 0;
        for (slot, raw) in bytes.chunks_exact(32).enumerate() {
            let attr = raw[11];
            match raw[0] {
                0x00 => break,
                0xE5 => {
                    long.clear();
                    continue;
                }
                _ => {}
            }
            if attr == ATTR_LONG {
                if long.is_empty() {
                    long_start = slot;
                }
                let mut part = [0u16; LONG_CHARS];
                for (unit, at) in part.iter_mut().zip(LONG_POSITIONS) {
                    *unit = u16::from_le_bytes([raw[at], raw[at + 1]]);
                }
                long.push((raw[0] & 0x1F, part));
                continue;
            }
            if attr & ATTR_LABEL != 0 {
                long.clear();
                continue;
            }
            let (name, first_slot) =
                if long.is_empty() { (short_name(raw), slot) } else { (long_name(&mut long), long_start) };
            long.clear();
            if name == "." || name == ".." {
                continue;
            }
            let entry = Entry {
                cluster: u32::from(u16::from_le_bytes([raw[20], raw[21]])) << 16
                    | u32::from(u16::from_le_bytes([raw[26], raw[27]])),
                size: u32::from_le_bytes([raw[28], raw[29], raw[30], raw[31]]),
                directory: attr & ATTR_DIRECTORY != 0,
            };
            let mut short = [0u8; 11];
            short.copy_from_slice(&raw[..11]);
            items.push(Item { name, short, entry, first_slot, slot });
        }
        Ok(Directory { chain, bytes, items })
    }

    /// The contents of a directory, as (name, entry) pairs.
    fn list(&self, disk: &mut impl Blocks, cluster: u32) -> Result<Vec<(String, Entry)>, &'static str> {
        Ok(self.directory(disk, cluster)?.items.into_iter().map(|item| (item.name, item.entry)).collect())
    }

    /// The label entry of the root directory, if it has one.
    fn root_label(&self, disk: &mut impl Blocks) -> Result<Option<String>, &'static str> {
        let mut sector = [0u8; SECTOR];
        disk.read(self.lba_of(self.root), &mut sector)?;
        for raw in sector.chunks_exact(32) {
            if raw[0] == 0 {
                break;
            }
            if raw[0] != 0xE5 && raw[11] & ATTR_LABEL != 0 && raw[11] != ATTR_LONG {
                return Ok(Some(String::from_utf8_lossy(&raw[..11]).trim_end().into()));
            }
        }
        Ok(None)
    }

    /// The file or directory at `path` ("/boot/kernel-a"), names compared
    /// without regard to case (`same_name`).
    pub fn find(&self, disk: &mut impl Blocks, path: &str) -> Result<Entry, &'static str> {
        let mut current = Entry { cluster: self.root, size: 0, directory: true };
        for part in path.split('/').filter(|part| !part.is_empty()) {
            if !current.directory {
                return Err("a path goes through a file");
            }
            current = self
                .list(disk, current.cluster)?
                .into_iter()
                .find(|(name, _)| same_name(name, part))
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
        self.write_clusters(disk, &chain[..needed], data)
    }

    /// `data` into the clusters of `chain`, zeros after it.
    fn write_clusters(&self, disk: &mut impl Blocks, chain: &[u32], data: &[u8]) -> Result<(), &'static str> {
        let mut buffer = alloc::vec![0u8; RUN_SECTORS as usize * SECTOR];
        let mut written = 0usize;
        for (lba, sectors) in self.runs(chain) {
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

    // ---- Allocating writes ----------------------------------------------------

    /// Creates the file at `path`, or replaces it, with `data`. Its directory
    /// must exist. A name that is not a plain 8.3 name gets long-name slots.
    pub fn write_file(&self, disk: &mut impl Blocks, path: &str, data: &[u8], stamp: Stamp) -> Result<(), &'static str> {
        if data.len() > WRITE_LIMIT {
            return Err("the file is larger than grenOS writes");
        }
        let (parent, name) = self.parent_and_name(disk, path)?;
        let directory = self.directory(disk, parent.cluster)?;
        let existing = directory.items.iter().position(|item| same_name(&item.name, name));
        if let Some(at) = existing {
            if directory.items[at].entry.directory {
                return Err("a directory has that name");
            }
        }
        let mut table = FatTable::new();
        // 1. The data, in clusters nothing points at yet.
        let needed = data.len().div_ceil(self.cluster_bytes());
        let chain = self.allocate(disk, &mut table, needed)?;
        self.write_clusters(disk, &chain, data)?;
        // 2. Their FAT entries, in every FAT.
        table.flush(self, disk)?;
        let first = chain.first().copied().unwrap_or(0);
        let size = u32::try_from(data.len()).map_err(|_| "the file is too large")?;
        match existing {
            // 3. The directory entry, now pointing at the new clusters; 4. the
            // old clusters freed.
            Some(at) => {
                let old = directory.items[at].entry.cluster;
                let slot = directory.items[at].slot;
                self.patch_slot(disk, &directory, slot, |raw| {
                    raw[20..22].copy_from_slice(&((first >> 16) as u16).to_le_bytes());
                    raw[26..28].copy_from_slice(&(first as u16).to_le_bytes());
                    raw[28..32].copy_from_slice(&size.to_le_bytes());
                    raw[18..20].copy_from_slice(&stamp.date.to_le_bytes());
                    raw[22..24].copy_from_slice(&stamp.time.to_le_bytes());
                    raw[24..26].copy_from_slice(&stamp.date.to_le_bytes());
                })?;
                self.free_chain(disk, old)?;
            }
            None => self.add_entry(disk, directory, name, ATTR_ARCHIVE, first, size, stamp)?,
        }
        self.adjust_free(disk, -(needed as i64))
    }

    /// Creates the directory at `path`; its parent must exist. An existing
    /// directory of that name is left as it is.
    pub fn make_dir(&self, disk: &mut impl Blocks, path: &str, stamp: Stamp) -> Result<(), &'static str> {
        let (parent, name) = self.parent_and_name(disk, path)?;
        let directory = self.directory(disk, parent.cluster)?;
        if let Some(item) = directory.items.iter().find(|item| same_name(&item.name, name)) {
            return if item.entry.directory { Ok(()) } else { Err("a file has that name") };
        }
        let mut table = FatTable::new();
        let chain = self.allocate(disk, &mut table, 1)?;
        let cluster = chain[0];
        // "." and "..", the parent written as 0 when it is the root.
        let mut block = alloc::vec![0u8; self.cluster_bytes()];
        let parent_cluster = if parent.cluster == self.root { 0 } else { parent.cluster };
        for (index, (dot, target)) in [(&b".          "[..], cluster), (&b"..         "[..], parent_cluster)].into_iter().enumerate() {
            let raw = &mut block[index * 32..index * 32 + 32];
            raw[..11].copy_from_slice(dot);
            raw[11] = ATTR_DIRECTORY;
            fill_entry_fields(raw, target, 0, stamp);
        }
        self.write_clusters(disk, &chain, &block)?;
        table.flush(self, disk)?;
        self.add_entry(disk, directory, name, ATTR_DIRECTORY, cluster, 0, stamp)?;
        self.adjust_free(disk, -1)
    }

    /// Removes the file, or the empty directory, at `path`.
    pub fn remove(&self, disk: &mut impl Blocks, path: &str) -> Result<(), &'static str> {
        let (parent, name) = self.parent_and_name(disk, path)?;
        let directory = self.directory(disk, parent.cluster)?;
        let item = directory.items.iter().find(|item| same_name(&item.name, name)).ok_or("no such file")?;
        if item.entry.directory && !self.list(disk, item.entry.cluster)?.is_empty() {
            return Err("the directory is not empty");
        }
        let (first_slot, slot, cluster) = (item.first_slot, item.slot, item.entry.cluster);
        // The entry goes first: a machine that stops here has lost clusters,
        // never an entry pointing at freed ones.
        for index in first_slot..=slot {
            self.patch_slot(disk, &directory, index, |raw| raw[0] = 0xE5)?;
        }
        self.free_chain(disk, cluster)
    }

    /// The directory a path's last part goes into, and that part.
    fn parent_and_name<'a>(&self, disk: &mut impl Blocks, path: &'a str) -> Result<(Entry, &'a str), &'static str> {
        if self.fats == 0 {
            return Err("the FATs are not mirrored: grenOS does not write to this file system");
        }
        let trimmed = path.trim_end_matches('/');
        let (parent, name) = trimmed.rsplit_once('/').unwrap_or(("", trimmed));
        if name.is_empty() || name == "." || name == ".." || name.chars().count() > 255 {
            return Err("not a valid file name");
        }
        if name.chars().any(|c| matches!(c, '"' | '*' | '/' | ':' | '<' | '>' | '?' | '\\' | '|') || (c as u32) < 0x20) {
            return Err("the name has a character FAT does not allow");
        }
        let parent = self.find(disk, parent)?;
        if !parent.directory {
            return Err("a path goes through a file");
        }
        Ok((parent, name))
    }

    /// Finds `count` free clusters and chains them in `table` (not yet
    /// written), the last one ending the chain.
    fn allocate(&self, disk: &mut impl Blocks, table: &mut FatTable, count: usize) -> Result<Vec<u32>, &'static str> {
        let mut chain = Vec::with_capacity(count);
        let mut cluster = 2;
        while chain.len() < count {
            if cluster >= self.clusters + 2 {
                return Err("the disk is full");
            }
            if table.get(self, disk, cluster)? == 0 {
                chain.push(cluster);
            }
            cluster += 1;
        }
        for (index, &cluster) in chain.iter().enumerate() {
            let next = chain.get(index + 1).copied().unwrap_or(END_MARK);
            table.set(self, disk, cluster, next)?;
        }
        Ok(chain)
    }

    /// Marks every cluster of a chain free again, in every FAT.
    fn free_chain(&self, disk: &mut impl Blocks, first: u32) -> Result<(), &'static str> {
        let chain = self.chain(disk, first)?;
        if chain.is_empty() {
            return Ok(());
        }
        let mut table = FatTable::new();
        for &cluster in &chain {
            table.set(self, disk, cluster, 0)?;
        }
        table.flush(self, disk)?;
        self.adjust_free(disk, chain.len() as i64)
    }

    /// Keeps FSInfo's count of free clusters true, when it keeps one.
    fn adjust_free(&self, disk: &mut impl Blocks, delta: i64) -> Result<(), &'static str> {
        let Some(lba) = self.fsinfo else {
            return Ok(());
        };
        let mut sector = [0u8; SECTOR];
        disk.read(lba, &mut sector)?;
        let u32_at = |at: usize| u32::from_le_bytes([sector[at], sector[at + 1], sector[at + 2], sector[at + 3]]);
        if u32_at(0) != 0x4161_5252 || u32_at(484) != 0x6141_7272 {
            return Ok(());
        }
        let free = u32_at(488);
        if free == u32::MAX || delta == 0 {
            return Ok(());
        }
        let updated = (i64::from(free) + delta).clamp(0, i64::from(self.clusters));
        sector[488..492].copy_from_slice(&(updated as u32).to_le_bytes());
        disk.write(lba, &sector)
    }

    /// Rewrites one 32-byte slot of a directory through `change`.
    fn patch_slot(
        &self,
        disk: &mut impl Blocks,
        directory: &Directory,
        slot: usize,
        change: impl FnOnce(&mut [u8]),
    ) -> Result<(), &'static str> {
        let offset = slot * 32;
        let cluster = directory.chain.get(offset / self.cluster_bytes()).ok_or("the slot is past the directory")?;
        let within = offset % self.cluster_bytes();
        let lba = self.lba_of(*cluster) + (within / SECTOR) as u64;
        let mut sector = [0u8; SECTOR];
        disk.read(lba, &mut sector)?;
        let at = within % SECTOR;
        change(&mut sector[at..at + 32]);
        disk.write(lba, &sector)
    }

    /// Adds an entry (with its long-name slots when needed) to a directory,
    /// growing the directory by a cluster when it has no room.
    #[allow(clippy::too_many_arguments)]
    fn add_entry(
        &self,
        disk: &mut impl Blocks,
        mut directory: Directory,
        name: &str,
        attr: u8,
        cluster: u32,
        size: u32,
        stamp: Stamp,
    ) -> Result<(), &'static str> {
        let taken: Vec<[u8; 11]> = directory.items.iter().map(|item| item.short).collect();
        let (short, case, long) = short_for(name, &taken)?;
        let checksum = long_checksum(&short);
        let mut slots: Vec<[u8; 32]> = Vec::new();
        let units: Vec<u16> = name.encode_utf16().collect();
        let pieces = if long { units.len().div_ceil(LONG_CHARS) } else { 0 };
        for piece in (0..pieces).rev() {
            let mut raw = [0u8; 32];
            raw[0] = (piece as u8 + 1) | if piece + 1 == pieces { 0x40 } else { 0 };
            raw[11] = ATTR_LONG;
            raw[13] = checksum;
            for (index, at) in LONG_POSITIONS.into_iter().enumerate() {
                let position = piece * LONG_CHARS + index;
                let unit = match position.cmp(&units.len()) {
                    core::cmp::Ordering::Less => units[position],
                    core::cmp::Ordering::Equal => 0x0000,
                    core::cmp::Ordering::Greater => 0xFFFF,
                };
                raw[at..at + 2].copy_from_slice(&unit.to_le_bytes());
            }
            slots.push(raw);
        }
        let mut raw = [0u8; 32];
        raw[..11].copy_from_slice(&short);
        raw[11] = attr;
        raw[12] = case;
        fill_entry_fields(&mut raw, cluster, size, stamp);
        slots.push(raw);

        // A run of free slots long enough: deleted ones, or the end marker and
        // everything after it.
        let total = directory.bytes.len() / 32;
        let mut start = None;
        let mut run = 0;
        for index in 0..total {
            let first = directory.bytes[index * 32];
            if first == 0x00 {
                if run == 0 {
                    start = Some(index);
                }
                run = total - start.unwrap_or(index);
                break;
            }
            if first == 0xE5 {
                if run == 0 {
                    start = Some(index);
                }
                run += 1;
                if run >= slots.len() {
                    break;
                }
            } else {
                run = 0;
                start = None;
            }
        }
        let start = match start {
            Some(start) if run >= slots.len() => start,
            _ => {
                // No room: one more cluster, zeroed, linked after the last.
                let mut table = FatTable::new();
                let added = self.allocate(disk, &mut table, 1)?[0];
                self.write_clusters(disk, &[added], &[])?;
                let last = *directory.chain.last().ok_or("the directory has no cluster")?;
                table.set(self, disk, last, added)?;
                table.flush(self, disk)?;
                self.adjust_free(disk, -1)?;
                let begin = start.filter(|_| run > 0).unwrap_or(total);
                directory.chain.push(added);
                directory.bytes.resize(directory.bytes.len() + self.cluster_bytes(), 0);
                begin
            }
        };
        for (offset, slot) in slots.iter().enumerate() {
            self.patch_slot(disk, &directory, start + offset, |raw| raw.copy_from_slice(slot))?;
        }
        Ok(())
    }
}

/// Cluster, size and the three dates of a short entry.
fn fill_entry_fields(raw: &mut [u8], cluster: u32, size: u32, stamp: Stamp) {
    raw[14..16].copy_from_slice(&stamp.time.to_le_bytes());
    raw[16..18].copy_from_slice(&stamp.date.to_le_bytes());
    raw[18..20].copy_from_slice(&stamp.date.to_le_bytes());
    raw[20..22].copy_from_slice(&((cluster >> 16) as u16).to_le_bytes());
    raw[22..24].copy_from_slice(&stamp.time.to_le_bytes());
    raw[24..26].copy_from_slice(&stamp.date.to_le_bytes());
    raw[26..28].copy_from_slice(&(cluster as u16).to_le_bytes());
    raw[28..32].copy_from_slice(&size.to_le_bytes());
}

/// The 8.3 name a new entry is stored under, its case flags, and whether it
/// also needs long-name slots. A name that fits 8.3 in one case is stored as
/// is (lower case through the flags Windows NT reads); any other name gets
/// long-name slots and a `BASE~N.EXT` alias no other entry has.
fn short_for(name: &str, taken: &[[u8; 11]]) -> Result<([u8; 11], u8, bool), &'static str> {
    const SPECIAL: &[u8] = b"$%'-_@~`!(){}^#&";
    let valid = |c: u8| c.is_ascii_uppercase() || c.is_ascii_digit() || SPECIAL.contains(&c);
    let (base, extension) = match name.rsplit_once('.') {
        Some((base, extension)) if !base.is_empty() => (base, extension),
        _ => (name, ""),
    };
    let fits = |part: &str, limit: usize| {
        !part.is_empty() && part.len() <= limit && part.bytes().all(|b| valid(b.to_ascii_uppercase()))
    };
    let one_case = |part: &str| {
        !part.bytes().any(|b| b.is_ascii_lowercase()) || !part.bytes().any(|b| b.is_ascii_uppercase())
    };
    let plain = fits(base, 8)
        && (extension.is_empty() || fits(extension, 3))
        && one_case(base)
        && one_case(extension)
        && name.bytes().filter(|&b| b == b'.').count() <= 1;
    let mut short = [b' '; 11];
    if plain {
        for (slot, byte) in short.iter_mut().zip(base.bytes()) {
            *slot = byte.to_ascii_uppercase();
        }
        for (slot, byte) in short[8..].iter_mut().zip(extension.bytes()) {
            *slot = byte.to_ascii_uppercase();
        }
        if short[0] == 0xE5 {
            short[0] = 0x05;
        }
        if taken.contains(&short) {
            return Err("an entry already has that 8.3 name");
        }
        let lower = |part: &str| part.bytes().any(|b| b.is_ascii_lowercase());
        let case = if lower(base) { 0x08 } else { 0 } | if lower(extension) { 0x10 } else { 0 };
        return Ok((short, case, false));
    }
    // An alias: the name's valid characters, upper case, then ~N.
    let clean = |part: &str, limit: usize| -> Vec<u8> {
        part.bytes().map(|b| b.to_ascii_uppercase()).filter(|&b| valid(b)).take(limit).collect()
    };
    let stem = clean(base, 6);
    let stem = if stem.is_empty() { b"FICHIER"[..6].to_vec() } else { stem };
    let extension = clean(extension, 3);
    for number in 1..=999_999u32 {
        let suffix = alloc::format!("~{number}");
        let keep = (8 - suffix.len()).min(stem.len());
        let mut candidate = [b' '; 11];
        for (slot, &byte) in candidate.iter_mut().zip(stem[..keep].iter().chain(suffix.as_bytes())) {
            *slot = byte;
        }
        for (slot, &byte) in candidate[8..].iter_mut().zip(&extension) {
            *slot = byte;
        }
        if !taken.contains(&candidate) {
            return Ok((candidate, 0, true));
        }
    }
    Err("no free 8.3 alias for that name")
}

/// Whether two names are the same file name: FAT ignores case, accented
/// letters included (NUMÉRO is numéro).
fn same_name(a: &str, b: &str) -> bool {
    a.chars().flat_map(char::to_lowercase).eq(b.chars().flat_map(char::to_lowercase))
}

/// The checksum of an 8.3 name that ties long-name slots to their entry.
fn long_checksum(short: &[u8; 11]) -> u8 {
    short.iter().fold(0u8, |sum, &byte| ((sum & 1) << 7).wrapping_add(sum >> 1).wrapping_add(byte))
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
    let mut base = piece(&raw[0..8], lower_base);
    // 0x05 stands for a real 0xE5 in the first byte.
    if raw[0] == 0x05 {
        base.replace_range(..1, "\u{E5}");
    }
    let extension = piece(&raw[8..11], lower_extension);
    if extension.is_empty() { base } else { alloc::format!("{base}.{extension}") }
}

/// A long name from its pieces, which the directory stores last first.
fn long_name(parts: &mut [(u8, [u16; LONG_CHARS])]) -> String {
    parts.sort_by_key(|(order, _)| *order);
    let units: Vec<u16> =
        parts.iter().flat_map(|(_, part)| part.iter().copied()).take_while(|&unit| unit != 0x0000).collect();
    char::decode_utf16(units).map(|c| c.unwrap_or('?')).collect()
}
