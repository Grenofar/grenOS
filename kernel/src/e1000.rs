//! The Intel 8254x network card — `8086:100e`, which is what QEMU gives by
//! default and what VirtualBox calls the 82540EM. One driver covers both.
//!
//! No interrupts: the card is polled from the main loop. A desktop that asks
//! for a page a few times a minute has nothing to gain from an interrupt line,
//! and polling keeps the receive path out of the handlers, where allocation is
//! not allowed.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::memory::{FRAME, Frames};
use crate::paging;
use crate::pci::{self, Device};

/// Where the card's registers are mapped: its physical address plus this.
const WINDOW: u64 = 0xFFFF_B000_0000_0000;

// The registers this driver touches, as byte offsets from the card's base.
const CTRL: usize = 0x0000;
const STATUS: usize = 0x0008;
const EERD: usize = 0x0014;
const ICR: usize = 0x00C0;
const IMC: usize = 0x00D8;
const RCTL: usize = 0x0100;
const TCTL: usize = 0x0400;
const TIPG: usize = 0x0410;
const RDBAL: usize = 0x2800;
const RDBAH: usize = 0x2804;
const RDLEN: usize = 0x2808;
const RDH: usize = 0x2810;
const RDT: usize = 0x2818;
const TDBAL: usize = 0x3800;
const TDBAH: usize = 0x3804;
const TDLEN: usize = 0x3808;
const TDH: usize = 0x3810;
const TDT: usize = 0x3818;
const MTA: usize = 0x5200;
const RAL: usize = 0x5400;
const RAH: usize = 0x5404;

const CTRL_RST: u32 = 1 << 26;
const CTRL_SLU: u32 = 1 << 6;
const CTRL_ASDE: u32 = 1 << 5;
const STATUS_LU: u32 = 1 << 1;

const RCTL_EN: u32 = 1 << 1;
const RCTL_BAM: u32 = 1 << 15;
const RCTL_SECRC: u32 = 1 << 26;
const TCTL_EN: u32 = 1 << 1;
const TCTL_PSP: u32 = 1 << 3;

/// Descriptors in each ring, and the size of one packet buffer.
const RING: usize = 32;
const BUFFER: usize = 2048;
/// A descriptor is sixteen bytes in both rings.
const DESCRIPTOR: usize = 16;

/// The status bit both rings use to say "the card is done with this one".
const DESCRIPTOR_DONE: u8 = 1;
/// Transmit command: end of packet, insert the checksum, report status.
const TX_CMD: u8 = (1 << 0) | (1 << 1) | (1 << 3);

pub struct Nic {
    /// The card's registers, in the kernel's window.
    base: *mut u8,
    /// The rings, reachable through the higher-half map.
    rx_ring: *mut u8,
    tx_ring: *mut u8,
    /// Where each buffer is, physically and virtually.
    rx_buffers: [(u64, *mut u8); RING],
    tx_buffers: [(u64, *mut u8); RING],
    rx_at: usize,
    tx_at: usize,
    pub mac: [u8; 6],
    pub sent: u32,
    pub received: u32,
    pub dropped: u32,
}

impl Nic {
    fn read(&self, register: usize) -> u32 {
        // SAFETY: `base` maps the card's register window, and every offset
        // used here is inside it.
        unsafe { self.base.add(register).cast::<u32>().read_volatile() }
    }

    fn write(&self, register: usize, value: u32) {
        // SAFETY: as above; these registers are the card's own.
        unsafe { self.base.add(register).cast::<u32>().write_volatile(value) };
    }

    /// A field of a descriptor, by ring and index.
    fn descriptor(&self, ring: *mut u8, index: usize) -> *mut u8 {
        // SAFETY: the ring holds RING descriptors of DESCRIPTOR bytes, and
        // the caller keeps `index` under RING.
        unsafe { ring.add(index * DESCRIPTOR) }
    }

    /// True while the link is up, which takes a moment after the reset.
    pub fn link_up(&self) -> bool {
        self.read(STATUS) & STATUS_LU != 0
    }

    /// Sends one Ethernet frame. False when the ring is full or the frame is
    /// too big for a buffer.
    pub fn send(&mut self, frame: &[u8]) -> bool {
        if frame.is_empty() || frame.len() > BUFFER {
            return false;
        }
        let index = self.tx_at;
        let descriptor = self.descriptor(self.tx_ring, index);
        // SAFETY: the descriptor is inside the ring; its status byte says
        // whether the card has finished with it.
        let status = unsafe { descriptor.add(12).read_volatile() };
        if self.sent as usize >= RING && status & DESCRIPTOR_DONE == 0 {
            self.dropped += 1;
            return false;
        }
        let (physical, buffer) = self.tx_buffers[index];
        // SAFETY: the buffer is a page of our own, at least BUFFER bytes, and
        // the frame fits in it.
        unsafe {
            core::ptr::copy_nonoverlapping(frame.as_ptr(), buffer, frame.len());
            descriptor.cast::<u64>().write_volatile(physical);
            descriptor.add(8).cast::<u16>().write_volatile(frame.len() as u16);
            descriptor.add(10).write_volatile(0); // no checksum offset
            descriptor.add(11).write_volatile(TX_CMD);
            descriptor.add(12).write_volatile(0); // status, which the card sets
        }
        self.tx_at = (index + 1) % RING;
        self.write(TDT, self.tx_at as u32);
        self.sent += 1;
        true
    }

    /// The next frame the card has received, if any.
    pub fn receive(&mut self) -> Option<Vec<u8>> {
        let index = self.rx_at;
        let descriptor = self.descriptor(self.rx_ring, index);
        // SAFETY: the descriptor is inside the ring.
        let (status, length) = unsafe {
            (descriptor.add(12).read_volatile(), descriptor.add(8).cast::<u16>().read_volatile() as usize)
        };
        if status & DESCRIPTOR_DONE == 0 {
            return None;
        }
        let (_, buffer) = self.rx_buffers[index];
        let mut frame = alloc::vec![0u8; length.min(BUFFER)];
        // SAFETY: the card wrote `length` bytes into this buffer, which is
        // ours and at least BUFFER long.
        unsafe {
            core::ptr::copy_nonoverlapping(buffer, frame.as_mut_ptr(), frame.len());
            descriptor.add(12).write_volatile(0);
        }
        self.rx_at = (index + 1) % RING;
        // The tail points at the last descriptor the card may use.
        self.write(RDT, ((index + RING - 1) % RING) as u32);
        self.received += 1;
        Some(frame)
    }

    pub fn mac_text(&self) -> String {
        let m = self.mac;
        format!("{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", m[0], m[1], m[2], m[3], m[4], m[5])
    }
}

/// Finds the card on the PCI bus and brings it up.
///
/// # Safety
///
/// Must be called once, with interrupts off or before the desktop starts:
/// it maps the card's registers and hands it pages of our own memory.
pub unsafe fn start(frames: &mut Frames) -> Result<Nic, &'static str> {
    let device = pci::scan().into_iter().find(is_e1000).ok_or("no 8254x card on the bus")?;
    // The card must be allowed to read and write memory by itself.
    pci::enable_bus_master(device);
    let bar = pci::bar(device, 0);
    let physical = bar & !0xF;
    if bar & 1 != 0 || physical == 0 {
        return Err("the card's registers are not memory mapped");
    }

    // The register window: 128 KiB, mapped without caching.
    for page in (0..0x2_0000u64).step_by(FRAME as usize) {
        // SAFETY: the window is the kernel's own, and these are the card's
        // registers, which must not be cached.
        unsafe { paging::map_device(frames, WINDOW + physical + page, physical + page)? };
    }
    let base = (WINDOW + physical) as *mut u8;

    let mut nic = Nic {
        base,
        rx_ring: core::ptr::null_mut(),
        tx_ring: core::ptr::null_mut(),
        rx_buffers: [(0, core::ptr::null_mut()); RING],
        tx_buffers: [(0, core::ptr::null_mut()); RING],
        rx_at: 0,
        tx_at: 0,
        mac: [0; 6],
        sent: 0,
        received: 0,
        dropped: 0,
    };

    // A reset, then the link, and no interrupts: this driver polls.
    nic.write(CTRL, nic.read(CTRL) | CTRL_RST);
    for _ in 0..1_000_000 {
        if nic.read(CTRL) & CTRL_RST == 0 {
            break;
        }
        core::hint::spin_loop();
    }
    nic.write(CTRL, nic.read(CTRL) | CTRL_SLU | CTRL_ASDE);
    nic.write(IMC, 0xFFFF_FFFF);
    let _ = nic.read(ICR);
    for entry in 0..128 {
        nic.write(MTA + entry * 4, 0);
    }

    nic.mac = mac_of(&nic);
    if nic.mac == [0; 6] {
        return Err("the card gave no address");
    }

    // The rings, and a buffer for every descriptor.
    let hhdm = frames.hhdm();
    let rx_ring = frames.allocate().ok_or("no frame for the receive ring")?;
    let tx_ring = frames.allocate().ok_or("no frame for the transmit ring")?;
    nic.rx_ring = (rx_ring + hhdm) as *mut u8;
    nic.tx_ring = (tx_ring + hhdm) as *mut u8;
    for index in 0..RING {
        // Two buffers of 2 KiB fit in one frame of 4 KiB.
        let (receive, transmit) = if index % 2 == 0 {
            (frames.allocate().ok_or("no frame for a buffer")?, frames.allocate().ok_or("no frame for a buffer")?)
        } else {
            (nic.rx_buffers[index - 1].0 + BUFFER as u64, nic.tx_buffers[index - 1].0 + BUFFER as u64)
        };
        nic.rx_buffers[index] = (receive, (receive + hhdm) as *mut u8);
        nic.tx_buffers[index] = (transmit, (transmit + hhdm) as *mut u8);
        let descriptor = nic.descriptor(nic.rx_ring, index);
        // SAFETY: the ring is a zeroed frame of our own; the card reads these
        // sixteen bytes to know where to put a packet.
        unsafe {
            descriptor.cast::<u64>().write_volatile(receive);
            descriptor.add(12).write_volatile(0);
        }
        let descriptor = nic.descriptor(nic.tx_ring, index);
        // SAFETY: as above, for the transmit ring.
        unsafe {
            descriptor.cast::<u64>().write_volatile(transmit);
            descriptor.add(12).write_volatile(DESCRIPTOR_DONE);
        }
    }

    nic.write(RDBAL, rx_ring as u32);
    nic.write(RDBAH, (rx_ring >> 32) as u32);
    nic.write(RDLEN, (RING * DESCRIPTOR) as u32);
    nic.write(RDH, 0);
    nic.write(RDT, (RING - 1) as u32);
    // Take every packet addressed to us, and broadcasts; 2 KiB buffers; the
    // card strips the Ethernet checksum.
    nic.write(RCTL, RCTL_EN | RCTL_BAM | RCTL_SECRC);

    nic.write(TDBAL, tx_ring as u32);
    nic.write(TDBAH, (tx_ring >> 32) as u32);
    nic.write(TDLEN, (RING * DESCRIPTOR) as u32);
    nic.write(TDH, 0);
    nic.write(TDT, 0);
    nic.write(TCTL, TCTL_EN | TCTL_PSP | (0x0F << 4) | (0x40 << 12));
    nic.write(TIPG, 0x0060_200A);
    Ok(nic)
}

fn is_e1000(device: &Device) -> bool {
    // 8086:100e is the 82540EM of QEMU and VirtualBox; the others are the
    // same family, and the same registers.
    device.vendor == 0x8086 && matches!(device.id, 0x100E | 0x100F | 0x1010 | 0x10D3 | 0x153A | 0x1533)
}

/// The card's address: the receive registers hold it after a reset, and the
/// EEPROM has it when they do not.
fn mac_of(nic: &Nic) -> [u8; 6] {
    let low = nic.read(RAL);
    let high = nic.read(RAH);
    if low != 0 || high & 0xFFFF != 0 {
        let mut mac = [0u8; 6];
        mac[..4].copy_from_slice(&low.to_le_bytes());
        mac[4..].copy_from_slice(&(high as u16).to_le_bytes());
        return mac;
    }
    let mut mac = [0u8; 6];
    for word in 0..3 {
        nic.write(EERD, ((word as u32) << 8) | 1);
        let mut value = 0;
        for _ in 0..100_000 {
            let read = nic.read(EERD);
            if read & (1 << 4) != 0 {
                value = (read >> 16) as u16;
                break;
            }
            core::hint::spin_loop();
        }
        mac[word * 2..word * 2 + 2].copy_from_slice(&value.to_le_bytes());
    }
    mac
}
