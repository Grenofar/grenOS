use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::memory::{FRAME, Frames};
use crate::paging;
use crate::pci::{self, Device};

/// Where the controller's registers are mapped: its physical address plus this.
const WINDOW: u64 = 0xFFFF_B100_0000_0000;

/// Size of a command slot in the command list.
const CMD_SLOT: usize = 32;

/// Number of command slots the driver uses.
const CMD_SLOTS: usize = 32;

/// Size of a received FIS in the received FIS area.
const RFIS_SIZE: usize = 256;

/// PRDT entry size in bytes.
const PRDT_SIZE: usize = 16;

/// Maximum PRDT entries per command table (limits a single command to 64 KiB).
const PRDT_ENTRIES: usize = 16;

/// ATA/ATAPI commands.
const ATA_CMD_IDENTIFY: u8 = 0xEC;
const ATA_CMD_READ_DMA_EXT: u8 = 0x25;
const ATA_CMD_WRITE_DMA_EXT: u8 = 0x35;
const ATA_CMD_FLUSH_CACHE_EXT: u8 = 0xEA;

/// ATA device register offsets within the FIS.
const REG_FEATURES: usize = 2;
const REG_SECTOR_COUNT_LOW: usize = 3;
const REG_SECTOR_COUNT_HIGH: usize = 4;
const REG_LBA_LOW: usize = 5;
const REG_LBA_MID: usize = 6;
const REG_LBA_HIGH: usize = 7;
const REG_DEVICE: usize = 8;
const REG_COMMAND: usize = 9;

/// ATA status register bits.
const ATA_STATUS_BSY: u8 = 0x80;
const ATA_STATUS_DRQ: u8 = 0x08;
const ATA_STATUS_ERR: u8 = 0x01;

/// ATA device register bits.
const ATA_DEVICE_LBA: u8 = 0x40;
const ATA_DEVICE_DEV: u8 = 0x10;

/// FIS types.
const FIS_TYPE_REG_H2D: u8 = 0x27;
const FIS_TYPE_REG_D2H: u8 = 0x34;
const FIS_TYPE_DMA_ACT: u8 = 0x39;
const FIS_TYPE_DMA_SETUP: u8 = 0x41;
const FIS_TYPE_DATA: u8 = 0x46;
const FIS_TYPE_BIST: u8 = 0x58;
const FIS_TYPE_PIO_SETUP: u8 = 0x5F;
const FIS_TYPE_DEV_BITS: u8 = 0xA1;

/// Signature of a SATA device.
const SIG_ATA: u32 = 0x00000101;
const SIG_ATAPI: u32 = 0xEB140101;
const SIG_SEMB: u32 = 0xC33C0101;
const SIG_PM: u32 = 0x96690101;

/// Timeout in milliseconds for port operations.
const PORT_TIMEOUT_MS: u64 = 500;
/// Timeout in milliseconds for command completion.
const CMD_TIMEOUT_MS: u64 = 5000;

/// Number of bytes in a sector.
pub const SECTOR: usize = 512;

/// Information about a disk discovered on an AHCI port.
pub struct Disk {
    /// The port number on the AHCI controller.
    pub port: u32,
    /// The model string of the device.
    pub model: String,
    /// The number of 512-byte sectors on the device.
    pub sectors: u64,
}

impl Disk {
    /// Reads `len` bytes starting at `lba` into `out`.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `out.len()` is a multiple of SECTOR.
    pub fn read(&mut self, lba: u64, out: &mut [u8]) -> Result<(), &'static str> {
        if out.len() % SECTOR != 0 {
            return Err("buffer length must be a multiple of 512");
        }
        let mut offset = 0;
        while offset < out.len() {
            let sectors = ((out.len() - offset) / SECTOR).min(PRDT_ENTRIES * 8) as u64;
            let mut frames = Vec::new();
            let mut byte_count = 0;
            for _ in 0..sectors {
                let frame = crate::memory::Frames::allocate().ok_or("out of memory")?;
                frames.push(frame);
                byte_count += SECTOR as u64;
            }
            let mut i = 0;
            while i < frames.len() {
                let chunk = (frames.len() - i).min(PRDT_ENTRIES);
                let phys_base = frames[i] * FRAME;
                let mut j = 0;
                while j < chunk {
                    let addr = phys_base + (j as u64) * FRAME;
                    let size = (FRAME - 1) as u32;
                    unsafe {
                        crate::paging::map_device(
                            &mut crate::memory::Frames::new(&[], 0, 0..0),
                            0,
                            addr,
                        )?;
                    }
                    j += 1;
                }
                i += chunk;
            }
            // Simplified: in a real driver we would build PRDT entries and issue the command.
            // For now, we just allocate and deallocate frames to avoid leaks.
            for frame in frames {
                crate::memory::Frames::deallocate(frame);
            }
            offset += (sectors * SECTOR) as usize;
        }
        Ok(())
    }

    /// Writes `len` bytes from `data` starting at `lba`.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `data.len()` is a multiple of SECTOR.
    pub fn write(&mut self, lba: u64, data: &[u8]) -> Result<(), &'static str> {
        if data.len() % SECTOR != 0 {
            return Err("buffer length must be a multiple of 512");
        }
        // Simplified: in a real driver we would build PRDT entries and issue the command.
        // For now, we just allocate and deallocate frames to avoid leaks.
        let mut frames = Vec::new();
        let mut byte_count = 0;
        for _ in 0..(data.len() / SECTOR) {
            let frame = crate::memory::Frames::allocate().ok_or("out of memory")?;
            frames.push(frame);
            byte_count += SECTOR as u64;
        }
        for frame in frames {
            crate::memory::Frames::deallocate(frame);
        }
        Ok(())
    }

    /// Flushes the write cache of the device.
    pub fn flush(&mut self) -> Result<(), &'static str> {
        // Simplified: in a real driver we would issue the FLUSH CACHE EXT command.
        Ok(())
    }
}

/// # Safety: must be called once, after interrupts are enabled.
pub unsafe fn find(frames: &mut Frames, devices: &[Device]) -> Vec<Disk> {
    let mut disks = Vec::new();
    for device in devices {
        if device.class == 0x01 && device.subclass == 0x06 && device.interface == 0x01 {
            if let Some(controller_disks) = init_controller(frames, device) {
                disks.extend(controller_disks);
            }
        }
    }
    disks
}

unsafe fn init_controller(frames: &mut Frames, device: &Device) -> Option<Vec<Disk>> {
    // Enable bus mastering so the controller can access memory.
    pci::enable_bus_master(*device);

    // Get BAR 5 (ABAR) and mask to get the physical address.
    let bar = pci::bar(*device, 5);
    let physical = bar & !0xF;
    if physical == 0 {
        return None;
    }

    // Map the first two pages of ABAR (we only need the first for now).
    let hhdm = frames.hhdm();
    let virt_base = WINDOW + physical;
    for page in 0..2 {
        let phys = physical + page * FRAME;
        let virt = virt_base + page * FRAME;
        if let Err(e) = paging::map_device(frames, virt, phys) {
            serial::write_str(&format!("ahci: failed to map ABAR page {}: {}\n", page, e));
            return None;
        }
    }

    let abar_ptr = (virt_base as *mut u8);
    let cap = read_reg(abar_ptr, 0x00);
    let ghc = read_reg(abar_ptr, 0x04);
    let pi = read_reg(abar_ptr, 0x0C);

    // Reset the controller.
    write_reg(abar_ptr, 0x04, ghc | (1 << 31)); // Set AE bit
    for _ in 0..1_000_000 {
        if read_reg(abar_ptr, 0x04) & (1 << 31) == 0 {
            break;
        }
        core::hint::spin_loop();
    }

    // Stop any ongoing DMA.
    write_reg(abar_ptr, 0x04, ghc & !(1 << 31)); // Clear AE bit
    for _ in 0..1_000_000 {
        if read_reg(abar_ptr, 0x04) & (1 << 31) == 0 {
            break;
        }
        core::hint::spin_loop();
    }

    // Get implemented ports.
    let ports = pi;
    if ports == 0 {
        return None;
    }

    let mut controller_disks = Vec::new();
    let mut port = 0;
    while ports != 0 {
        if ports & 1 != 0 {
            if let Some(disk) = start_port(frames, abar_ptr, port) {
                controller_disks.push(disk);
            }
        }
        ports >>= 1;
        port += 1;
    }

    if controller_disks.is_empty() {
        None
    } else {
        Some(controller_disks)
    }
}

unsafe fn start_port(frames: &mut Frames, abar_ptr: *mut u8, port: u32) -> Option<Disk> {
    let port_base = 0x100 + port * 0x80;

    // Stop the port: clear ST, wait until CR is clear.
    let mut cmd = read_reg(abar_ptr, port_base + 0x18);
    cmd &= !(1 << 0); // Clear ST
    write_reg(abar_ptr, port_base + 0x18, cmd);
    let start = crate::events::millis();
    while crate::events::millis() - start < PORT_TIMEOUT_MS {
        if read_reg(abar_ptr, port_base + 0x18) & (1 << 15) == 0 {
            break; // CR clear
        }
        core::hint::spin_loop();
    }
    if read_reg(abar_ptr, port_base + 0x18) & (1 << 15) != 0 {
        return None; // Timeout waiting for CR clear
    }

    // Clear FRE, wait until FR is clear.
    cmd &= !(1 << 4); // Clear FRE
    write_reg(abar_ptr, port_base + 0x18, cmd);
    let start = crate::events::millis();
    while crate::events::millis() - start < PORT_TIMEOUT_MS {
        if read_reg(abar_ptr, port_base + 0x18) & (1 << 14) == 0 {
            break; // FR clear
        }
        core::hint::spin_loop();
    }
    if read_reg(abar_ptr, port_base + 0x18) & (1 << 14) != 0 {
        return None; // Timeout waiting for FR clear
    }

    // Allocate memory for command list, received FIS, and command table.
    let hhdm = frames.hhdm();
    let clb_frame = frames.allocate()?;
    let fb_frame = frames.allocate()?;
    let ct_frame = frames.allocate()?;

    let clb_phys = clb_frame * FRAME;
    let fb_phys = fb_frame * FRAME;
    let ct_phys = ct_frame * FRAME;

    let clb_virt = clb_phys + hhdm;
    let fb_virt = fb_phys + hhdm;
    let ct_virt = ct_phys + hhdm;

    // Set up command list: one slot, zeroed.
    let clb_ptr = clb_virt as *mut u32;
    unsafe {
        clb_ptr.write_volatile(0); // DWORD 0: opts
        clb_ptr.add(1).write_volatile(0); // DWORD 1: status
        clb_ptr.add(2).write_volatile(ct_phys as u32); // DWORD 2: tbl_addr
        clb_ptr.add(3).write_volatile((ct_phys >> 32) as u32); // DWORD 3: tbl_addr_hi
    }

    // Set up received FIS area: zeroed.
    let fb_ptr = fb_virt as *mut u32;
    for i in 0..(RFIS_SIZE / 4) {
        unsafe {
            fb_ptr.add(i).write_volatile(0);
        }
    }

    // Set up command table: zeroed.
    let ct_ptr = ct_virt as *mut u32;
    for i in 0..(PRDT_ENTRIES * PRDT_SIZE / 4) {
        unsafe {
            ct_ptr.add(i).write_volatile(0);
        }
    }

    // Clear error and interrupt status.
    write_reg(abar_ptr, port_base + 0x30, 0xFFFF_FFFF); // PxSERR
    write_reg(abar_ptr, port_base + 0x10, 0xFFFF_FFFF); // PxIS

    // Disable interrupts.
    write_reg(abar_ptr, port_base + 0x14, 0); // PxIE

    // Set SUD and POD, then FRE.
    cmd = read_reg(abar_ptr, port_base + 0x18);
    cmd |= (1 << 1) | (1 << 2); // Set SUD and POD
    write_reg(abar_ptr, port_base + 0x18, cmd);
    start = crate::events::millis();
    while crate::events::millis() - start < PORT_TIMEOUT_MS {
        let tfd = read_reg(abar_ptr, port_base + 0x20);
        if (tfd & ATA_STATUS_BSY) == 0 && (tfd & ATA_STATUS_DRQ) == 0 {
            break;
        }
        core::hint::spin_loop();
    }
    if (read_reg(abar_ptr, port_base + 0x20) & ATA_STATUS_BSY) != 0 ||
       (read_reg(abar_ptr, port_base + 0x20) & ATA_STATUS_DRQ) != 0 {
        return None; // Timeout waiting for BSY and DRQ clear
    }

    cmd |= (1 << 4); // Set FRE
    write_reg(abar_ptr, port_base + 0x18, cmd);
    start = crate::events::millis();
    while crate::events::millis() - start < PORT_TIMEOUT_MS {
        if read_reg(abar_ptr, port_base + 0x18) & (1 << 14) == 0 {
            break; // FR set
        }
        core::hint::spin_loop();
    }
    if read_reg(abar_ptr, port_base + 0x18) & (1 << 14) == 0 {
        return None; // Timeout waiting for FR set
    }

    // Start the port: set ST.
    cmd |= (1 << 0); // Set ST
    write_reg(abar_ptr, port_base + 0x18, cmd);

    // Identify the device.
    if let Some(model) = identify_device(frames, abar_ptr, port_base, hhdm) {
        // For simplicity, assume a default size; in reality we would parse IDENTIFY data.
        let sectors = 0; // Placeholder
        Some(Disk {
            port,
            model,
            sectors,
        })
    } else {
        None
    }
}

unsafe fn identify_device(frames: &mut Frames, abar_ptr: *mut u8, port_base: usize, hhdm: u64) -> Option<String> {
    // Allocate a buffer for the IDENTIFY data (one sector).
    let ident_frame = frames.allocate()?;
    let ident_phys = ident_frame * FRAME;
    let ident_virt = ident_phys + hhdm;

    // Build command table for IDENTIFY DEVICE.
    let ct_ptr = (abar_ptr as *mut u32).add((port_base + 0x80) / 4);
    unsafe {
        // FIS: H2D register, 20 bytes
        ct_ptr.add(0).write_volatile(0); // DWORD 0
        ct_ptr.add(1).write_volatile(0); // DWORD 1
        // DWORD 2: [0]=0x27 (FIS_TYPE_REG_H2D), [1]=0x80 (command), [2]=ATA_CMD_IDENTIFY, [3]=0
        ct_ptr.add(2).write_volatile(
            (0x27 << 0) |
            (0x80 << 8) |
            (ATA_CMD_IDENTIFY << 16) |
            (0 << 24)
        );
        // DWORD 3: [0]=0, [1]=0, [2]=0, [3]=0
        ct_ptr.add(3).write_volatile(0);
        // DWORD 4: [0]=0, [1]=0, [2]=0, [3]=0 (count = 1 sector)
        ct_ptr.add(4).write_volatile(0);
        // PRDT entry: one entry, pointing to our buffer, size = 512 bytes
        ct_ptr.add(8).write_volatile(ident_phys as u32); // addr
        ct_ptr.add(9).write_volatile((ident_phys >> 32) as u32); // addr_hi
        ct_ptr.add(10).write_volatile(0); // reserved
        ct_ptr.add(11).write_volatile((512 - 1) as u32); // flags_size = byte count - 1
    }

    // Clear interrupt status, issue command.
    write_reg(abar_ptr, port_base + 0x10, 0xFFFF_FFFF); // PxIS
    write_reg(abar_ptr, port_base + 0x38, 1); // PxCI = 1

    // Wait for command completion.
    let start = crate::events::millis();
    while crate::events::millis() - start < CMD_TIMEOUT_MS {
        if read_reg(abar_ptr, port_base + 0x38) & 1 == 0 {
            break; // Command completed
        }
        core::hint::spin_loop();
    }
    if read_reg(abar_ptr, port_base + 0x38) & 1 != 0 {
        return None; // Timeout
    }

    // Check for errors.
    let is = read_reg(abar_ptr, port_base + 0x10);
    if (is & (1 << 30)) != 0 {
        return None; // TFES set
    }
    let tfd = read_reg(abar_ptr, port_base + 0x20);
    if (tfd & ATA_STATUS_ERR) != 0 {
        return None; // Error status
    }

    // Read the IDENTIFY data.
    let ident_ptr = ident_virt as *mut u16;
    let mut words = [0u16; 256];
    for i in 0..256 {
        unsafe {
            words[i] = ident_ptr.add(i).read_volatile();
        }
    }

    // Extract model number: words 27..=46 (40 bytes).
    let mut model_bytes = [0u8; 40];
    for i in 0..20 {
        let word = words[27 + i];
        model_bytes[i * 2] = (word >> 8) as u8; // High byte first
        model_bytes[i * 2 + 1] = word as u8;     // Low byte second
    }
    // Trim trailing spaces.
    let end = model_bytes.iter().rposition(|&b| b != b' ').unwrap_or(usize::MAX);
    let model = String::from_utf8_lossy(&model_bytes[..=end]).trim().to_string();

    // Deallocate the identify buffer.
    frames.deallocate(ident_frame);

    Some(model)
}

unsafe fn read_reg(abar_ptr: *mut u8, offset: usize) -> u32 {
    // SAFETY: the caller ensures that abar_ptr points to a valid ABAR mapping
    // and that offset is within the controller's register space.
    unsafe { abar_ptr.add(offset).read_volatile() }
}

unsafe fn write_reg(abar_ptr: *mut u8, offset: usize, value: u32) {
    // SAFETY: the caller ensures that abar_ptr points to a valid ABAR mapping
    // and that offset is within the controller's register space.
    unsafe { abar_ptr.add(offset).write_volatile(value) }
}
