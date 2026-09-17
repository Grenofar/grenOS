//! SATA disks through an AHCI controller: find the controller on the PCI bus,
//! bring up every port with a disk behind it, and read and write sectors.
//!
//! Every register, bit and structure layout here is in
//! docs/specs/disk-and-updates.md §3, checked against Linux's
//! `drivers/ata/ahci.h`, `include/linux/ata.h` and `ata_tf_to_fis`.
//!
//! No interrupts: like the network card, the controller is polled. One command
//! slot (0), one command at a time, and a bounce buffer of frames that every
//! transfer goes through, so no caller ever hands the device a heap address.

use alloc::string::String;
use alloc::vec::Vec;

use crate::events;
use crate::memory::{FRAME, Frames};
use crate::paging;
use crate::pci::{self, Device};

/// Where the controller's registers are mapped: its physical address plus
/// this. The network card uses 0xFFFF_B000_0000_0000.
const WINDOW: u64 = 0xFFFF_B100_0000_0000;

/// Every sector this driver reads or writes is 512 bytes.
pub const SECTOR: usize = 512;

// Host bus adapter registers, from the controller's base.
const GHC: usize = 0x04;
const PI: usize = 0x0C;
const GHC_AE: u32 = 1 << 31;

// Port registers, from the port's base (0x100 + n * 0x80).
const PORTS: usize = 0x100;
const PORT_SIZE: usize = 0x80;
const CLB: usize = 0x00;
const CLBU: usize = 0x04;
const FB: usize = 0x08;
const FBU: usize = 0x0C;
const IS: usize = 0x10;
const IE: usize = 0x14;
const CMD: usize = 0x18;
const TFD: usize = 0x20;
const SIG: usize = 0x24;
const SSTS: usize = 0x28;
const SERR: usize = 0x30;
const CI: usize = 0x38;

const CMD_ST: u32 = 1 << 0;
const CMD_SUD: u32 = 1 << 1;
const CMD_POD: u32 = 1 << 2;
const CMD_FRE: u32 = 1 << 4;
const CMD_FR: u32 = 1 << 14;
const CMD_CR: u32 = 1 << 15;
const TFD_ERR: u32 = 0x01;
const TFD_DRQ: u32 = 0x08;
const TFD_BSY: u32 = 0x80;
const IS_TFES: u32 = 1 << 30;
/// What PxSIG reads for an ATA disk (an ATAPI drive reads 0xEB14_0101).
const SIG_ATA: u32 = 0x0000_0101;
/// PxSSTS bits 3:0: a device is present and the link is up.
const DET_PRESENT: u32 = 3;

// ATA commands.
const IDENTIFY: u8 = 0xEC;
const READ_EXT: u8 = 0x25;
const WRITE_EXT: u8 = 0x35;
const FLUSH_EXT: u8 = 0xEA;

/// Frames in the bounce buffer, one PRDT entry each: 64 KiB, 128 sectors per
/// command. Longer transfers loop.
const BOUNCE_FRAMES: usize = 16;
const PER_COMMAND: usize = BOUNCE_FRAMES * FRAME as usize / SECTOR;
/// The command table starts its PRDT at this offset, 16 bytes an entry.
const PRDT: usize = 0x80;
/// How long a port may take to stop, to become ready, or to finish a command.
const PATIENCE_MS: u64 = 5_000;

pub struct Disk {
    /// The port's registers, in the kernel's window.
    base: *mut u8,
    hhdm: u64,
    /// Physical addresses of the command list and the command table.
    list: u64,
    table: u64,
    /// Physical addresses of the bounce buffer's frames.
    bounce: [u64; BOUNCE_FRAMES],
    /// Which port of the controller the disk is on.
    pub port: u32,
    /// The model string the disk gives in IDENTIFY, spaces trimmed.
    pub model: String,
    /// Its size, in 512-byte sectors.
    pub sectors: u64,
}

/// The first AHCI controller on the bus, every ATA disk behind it, and the
/// ports that have a disk but did not come up, with the reason. An error means
/// there is no controller the driver can use.
///
/// # Safety
///
/// Call once, after interrupts are enabled: the timeouts count milliseconds
/// on the timer. It maps the controller's registers and hands the controller
/// frames of our own.
pub unsafe fn find(frames: &mut Frames, devices: &[Device]) -> Result<Found, &'static str> {
    let controller = devices
        .iter()
        .copied()
        .find(|device| device.class == 0x01 && device.subclass == 0x06 && device.interface == 0x01)
        .ok_or("no AHCI controller")?;
    pci::enable_bus_master(controller);
    let bar = pci::bar(controller, 5);
    let physical = bar & !0xF;
    if bar & 1 != 0 || physical == 0 {
        return Err("the AHCI controller's registers are not memory mapped");
    }
    // The registers of 32 ports end at 0x1100: two pages.
    for page in (0..0x2000u64).step_by(FRAME as usize) {
        // SAFETY: the window is the kernel's own, and these are the
        // controller's registers, which must not be cached.
        unsafe { paging::map_device(frames, WINDOW + physical + page, physical + page)? };
    }
    let hba = (WINDOW + physical) as *mut u8;
    // SAFETY: GHC and PI are inside the two pages just mapped.
    let implemented = unsafe {
        let ghc = hba.add(GHC).cast::<u32>();
        ghc.write_volatile(ghc.read_volatile() | GHC_AE);
        hba.add(PI).cast::<u32>().read_volatile()
    };

    let mut found = Found { disks: Vec::new(), failed: Vec::new() };
    for port in 0..32u32 {
        if implemented & (1 << port) == 0 {
            continue;
        }
        // SAFETY: port registers of an implemented port, inside the mapping.
        let base = unsafe { hba.add(PORTS + port as usize * PORT_SIZE) };
        // SAFETY: as above.
        let (status, signature) =
            unsafe { (base.add(SSTS).cast::<u32>().read_volatile(), base.add(SIG).cast::<u32>().read_volatile()) };
        if status & 0xF != DET_PRESENT || signature != SIG_ATA {
            continue;
        }
        // SAFETY: an implemented port with an ATA disk, brought up once.
        match unsafe { Disk::start(frames, base, port) } {
            Ok(disk) => found.disks.push(disk),
            Err(why) => found.failed.push((port, why)),
        }
    }
    Ok(found)
}

/// What `find` found.
pub struct Found {
    pub disks: Vec<Disk>,
    /// Ports with a disk that did not come up, and why.
    pub failed: Vec<(u32, &'static str)>,
}

impl Disk {
    fn get(&self, register: usize) -> u32 {
        // SAFETY: `base` maps the port's registers, and every offset used is
        // one of them.
        unsafe { self.base.add(register).cast::<u32>().read_volatile() }
    }

    fn set(&self, register: usize, value: u32) {
        // SAFETY: as above.
        unsafe { self.base.add(register).cast::<u32>().write_volatile(value) };
    }

    /// A frame of ours, through the higher-half map.
    fn memory(&self, physical: u64) -> *mut u8 {
        (physical + self.hhdm) as *mut u8
    }

    /// Waits until `done` holds, for PATIENCE_MS at most. The spin count is a
    /// second limit, should the timer not be running.
    fn wait(&self, done: impl Fn(&Self) -> bool) -> bool {
        let start = events::millis();
        let mut spins: u64 = 0;
        loop {
            if done(self) {
                return true;
            }
            if events::millis().saturating_sub(start) > PATIENCE_MS || spins > 400_000_000 {
                return false;
            }
            spins += 1;
            core::hint::spin_loop();
        }
    }

    /// # Safety
    ///
    /// `base` must be the registers of an implemented port with an ATA disk,
    /// mapped without caching, and nothing else may drive that port.
    unsafe fn start(frames: &mut Frames, base: *mut u8, port: u32) -> Result<Disk, &'static str> {
        let list = frames.allocate().ok_or("no frame for the command list")?;
        let received = frames.allocate().ok_or("no frame for the received FIS")?;
        let table = frames.allocate().ok_or("no frame for the command table")?;
        let mut bounce = [0u64; BOUNCE_FRAMES];
        for slot in bounce.iter_mut() {
            *slot = frames.allocate().ok_or("no frame for the transfer buffer")?;
        }
        let mut disk = Disk { base, hhdm: frames.hhdm(), list, table, bounce, port, model: String::new(), sectors: 0 };

        // The engines stop before their addresses change.
        disk.set(CMD, disk.get(CMD) & !CMD_ST);
        if !disk.wait(|d| d.get(CMD) & CMD_CR == 0) {
            return Err("the port's command engine does not stop");
        }
        disk.set(CMD, disk.get(CMD) & !CMD_FRE);
        if !disk.wait(|d| d.get(CMD) & CMD_FR == 0) {
            return Err("the port's FIS engine does not stop");
        }

        disk.set(CLB, list as u32);
        disk.set(CLBU, (list >> 32) as u32);
        disk.set(FB, received as u32);
        disk.set(FBU, (received >> 32) as u32);
        disk.set(SERR, u32::MAX);
        disk.set(IS, u32::MAX);
        disk.set(IE, 0);

        disk.set(CMD, disk.get(CMD) | CMD_SUD | CMD_POD | CMD_FRE);
        if !disk.wait(|d| d.get(TFD) & (TFD_BSY | TFD_DRQ) == 0) {
            return Err("the disk stays busy");
        }
        disk.set(CMD, disk.get(CMD) | CMD_ST);

        disk.identify()?;
        Ok(disk)
    }

    /// The model and the size, from IDENTIFY DEVICE.
    fn identify(&mut self) -> Result<(), &'static str> {
        self.command(IDENTIFY, 0, 1, false)?;
        let mut data = [0u8; SECTOR];
        self.unload_bounce(&mut data);
        let word = |index: usize| u16::from_le_bytes([data[index * 2], data[index * 2 + 1]]);

        // Words 27 to 46: forty characters, the first of each pair in the
        // high byte.
        let mut model = String::new();
        for index in 27..47 {
            let pair = word(index);
            for byte in [(pair >> 8) as u8, pair as u8] {
                if byte.is_ascii_graphic() || byte == b' ' {
                    model.push(char::from(byte));
                }
            }
        }
        self.model = String::from(model.trim());

        let features = word(83);
        self.sectors = if features & 0xC000 == 0x4000 && features & (1 << 10) != 0 {
            u64::from(word(100))
                | u64::from(word(101)) << 16
                | u64::from(word(102)) << 32
                | u64::from(word(103)) << 48
        } else {
            u64::from(word(60)) | u64::from(word(61)) << 16
        };
        if self.sectors == 0 {
            return Err("the disk reports no sectors");
        }
        Ok(())
    }

    /// Runs one ATA command in slot 0, moving `sectors` sectors through the
    /// bounce buffer, towards the disk when `write`.
    fn command(&mut self, ata: u8, lba: u64, sectors: usize, write: bool) -> Result<(), &'static str> {
        let bytes = sectors * SECTOR;
        let frame = FRAME as usize;
        let entries = bytes.div_ceil(frame);
        if entries > BOUNCE_FRAMES {
            return Err("a command moves at most 128 sectors");
        }

        let header = self.memory(self.list);
        let table = self.memory(self.table);
        // Command FIS length 5 double words, the write bit, the PRDT length.
        let flags = 5 | if write { 1 << 6 } else { 0 } | (entries as u32) << 16;
        // The register FIS, host to device (Linux's ata_tf_to_fis): LBA mode
        // for reads and writes, device 0 for IDENTIFY.
        let device = if ata == IDENTIFY { 0 } else { 0x40 };
        let fis: [u8; 20] = [
            0x27,
            0x80,
            ata,
            0,
            lba as u8,
            (lba >> 8) as u8,
            (lba >> 16) as u8,
            device,
            (lba >> 24) as u8,
            (lba >> 32) as u8,
            (lba >> 40) as u8,
            0,
            sectors as u8,
            (sectors >> 8) as u8,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        // SAFETY: the command list and the command table are frames of ours,
        // reached through the HHDM; the header is the first 32 bytes of the
        // list, and the table holds the FIS and at most BOUNCE_FRAMES entries,
        // well inside one frame.
        unsafe {
            core::ptr::write_bytes(header, 0, 32);
            header.cast::<u32>().write_volatile(flags);
            header.add(8).cast::<u32>().write_volatile(self.table as u32);
            header.add(12).cast::<u32>().write_volatile((self.table >> 32) as u32);
            core::ptr::write_bytes(table, 0, PRDT + BOUNCE_FRAMES * 16);
            core::ptr::copy_nonoverlapping(fis.as_ptr(), table, fis.len());
            for (index, physical) in self.bounce.iter().take(entries).enumerate() {
                let size = (bytes - index * frame).min(frame);
                let entry = table.add(PRDT + index * 16);
                entry.cast::<u32>().write_volatile(*physical as u32);
                entry.add(4).cast::<u32>().write_volatile((*physical >> 32) as u32);
                entry.add(12).cast::<u32>().write_volatile(size as u32 - 1);
            }
        }

        if !self.wait(|d| d.get(TFD) & (TFD_BSY | TFD_DRQ) == 0) {
            return Err("the disk is busy");
        }
        self.set(IS, u32::MAX);
        self.set(CI, 1);
        let finished = self.wait(|d| d.get(CI) & 1 == 0 || d.get(IS) & IS_TFES != 0);
        if self.get(IS) & IS_TFES != 0 || self.get(TFD) & TFD_ERR != 0 {
            return Err("the disk reported an error");
        }
        if !finished {
            return Err("the disk did not answer in time");
        }
        Ok(())
    }

    fn load_bounce(&self, data: &[u8]) {
        for (piece, physical) in data.chunks(FRAME as usize).zip(self.bounce) {
            // SAFETY: a bounce frame of ours, and a piece of at most one frame.
            unsafe { core::ptr::copy_nonoverlapping(piece.as_ptr(), self.memory(physical), piece.len()) };
        }
    }

    fn unload_bounce(&self, out: &mut [u8]) {
        for (piece, physical) in out.chunks_mut(FRAME as usize).zip(self.bounce) {
            // SAFETY: as above, the other way.
            unsafe { core::ptr::copy_nonoverlapping(self.memory(physical), piece.as_mut_ptr(), piece.len()) };
        }
    }

    /// Refuses a transfer that is not whole sectors, or that runs past the end
    /// of the disk.
    fn check(&self, lba: u64, bytes: usize) -> Result<usize, &'static str> {
        if bytes % SECTOR != 0 {
            return Err("a transfer is made of whole sectors");
        }
        let count = (bytes / SECTOR) as u64;
        if lba.checked_add(count).is_none_or(|end| end > self.sectors) {
            return Err("the transfer runs past the end of the disk");
        }
        Ok(bytes / SECTOR)
    }

    /// Reads `out.len() / 512` sectors from `lba`.
    pub fn read(&mut self, lba: u64, out: &mut [u8]) -> Result<(), &'static str> {
        self.check(lba, out.len())?;
        for (index, piece) in out.chunks_mut(PER_COMMAND * SECTOR).enumerate() {
            self.command(READ_EXT, lba + (index * PER_COMMAND) as u64, piece.len() / SECTOR, false)?;
            self.unload_bounce(piece);
        }
        Ok(())
    }

    /// Writes `data.len() / 512` sectors at `lba`.
    #[allow(dead_code)] // the write test and the installer use it (docs/specs/disk-and-updates.md §4, §7)
    pub fn write(&mut self, lba: u64, data: &[u8]) -> Result<(), &'static str> {
        self.check(lba, data.len())?;
        for (index, piece) in data.chunks(PER_COMMAND * SECTOR).enumerate() {
            self.load_bounce(piece);
            self.command(WRITE_EXT, lba + (index * PER_COMMAND) as u64, piece.len() / SECTOR, true)?;
        }
        Ok(())
    }

    /// Asks the disk to put what it holds in its cache onto the platters.
    #[allow(dead_code)] // after writes, as above
    pub fn flush(&mut self) -> Result<(), &'static str> {
        self.command(FLUSH_EXT, 0, 0, false)
    }
}
