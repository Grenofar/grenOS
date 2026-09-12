//! The PCI bus: what is plugged into the machine, read from configuration
//! space through the 0xCF8 / 0xCFC window. Enumeration only for now — the
//! disk and network drivers will start from this list.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::port::{inl, outl};

const ADDRESS: u16 = 0xCF8;
const DATA: u16 = 0xCFC;

/// One function of one device on one bus.
#[derive(Clone, Copy)]
pub struct Device {
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
    pub vendor: u16,
    pub id: u16,
    pub class: u8,
    pub subclass: u8,
    pub interface: u8,
}

/// The 32 bits at `offset` of a function's configuration space.
fn read(bus: u8, slot: u8, function: u8, offset: u8) -> u32 {
    let address = 1 << 31
        | u32::from(bus) << 16
        | u32::from(slot & 0x1F) << 11
        | u32::from(function & 0x07) << 8
        | u32::from(offset & 0xFC);
    // SAFETY: the configuration address and data ports; picking a function and
    // reading its registers changes nothing in the machine.
    unsafe {
        outl(ADDRESS, address);
        inl(DATA)
    }
}

fn probe(bus: u8, slot: u8, function: u8) -> Option<Device> {
    let identity = read(bus, slot, function, 0);
    let vendor = (identity & 0xFFFF) as u16;
    // 0xFFFF is what the bus answers when nothing is there.
    if vendor == 0xFFFF {
        return None;
    }
    let classes = read(bus, slot, function, 0x08);
    Some(Device {
        bus,
        slot,
        function,
        vendor,
        id: (identity >> 16) as u16,
        class: (classes >> 24) as u8,
        subclass: (classes >> 16) as u8,
        interface: (classes >> 8) as u8,
    })
}

/// Every function that answers, bus by bus. The brute-force walk: 8192 probes,
/// a few milliseconds, and no bridge to follow by hand.
pub fn scan() -> Vec<Device> {
    let mut found = Vec::new();
    for bus in 0..=255u8 {
        for slot in 0..32u8 {
            let Some(device) = probe(bus, slot, 0) else {
                continue;
            };
            found.push(device);
            // Bit 7 of the header type: the device has more than one function.
            let header = (read(bus, slot, 0, 0x0C) >> 16) as u8;
            if header & 0x80 != 0 {
                found.extend((1..8u8).filter_map(|function| probe(bus, slot, function)));
            }
        }
    }
    found
}

/// The makers we are likely to meet, in QEMU, in VirtualBox and on a PC.
const VENDORS: [(u16, &str); 16] = [
    (0x8086, "Intel"),
    (0x1022, "AMD"),
    (0x1002, "AMD/ATI"),
    (0x10DE, "NVIDIA"),
    (0x10EC, "Realtek"),
    (0x14E4, "Broadcom"),
    (0x168C, "Qualcomm Atheros"),
    (0x15AD, "VMware"),
    (0x80EE, "Oracle VirtualBox"),
    (0x1234, "QEMU"),
    (0x1AF4, "Red Hat (virtio)"),
    (0x1B36, "Red Hat (QEMU)"),
    (0x1013, "Cirrus Logic"),
    (0x1106, "VIA"),
    (0x106B, "Apple"),
    (0x1B21, "ASMedia"),
];

impl Device {
    pub fn vendor_name(self) -> &'static str {
        VENDORS
            .iter()
            .find(|&&(id, _)| id == self.vendor)
            .map_or("constructeur inconnu", |&(_, name)| name)
    }

    /// What the device is, in French, from its class and subclass.
    pub fn kind(self) -> &'static str {
        match (self.class, self.subclass) {
            (0x00, _) => "Périphérique ancien",
            (0x01, 0x01) => "Contrôleur de disque IDE",
            (0x01, 0x05) => "Contrôleur de disque ATA",
            (0x01, 0x06) => "Contrôleur de disque SATA (AHCI)",
            (0x01, 0x07) => "Contrôleur SAS",
            (0x01, 0x08) => "Disque NVMe",
            (0x01, _) => "Contrôleur de stockage",
            (0x02, 0x00) => "Carte réseau Ethernet",
            (0x02, 0x80) => "Carte réseau",
            (0x02, _) => "Contrôleur réseau",
            (0x03, _) => "Carte graphique",
            (0x04, 0x03) => "Audio haute définition",
            (0x04, _) => "Carte son ou vidéo",
            (0x05, _) => "Contrôleur mémoire",
            (0x06, 0x00) => "Pont hôte",
            (0x06, 0x01) => "Pont ISA",
            (0x06, 0x04) => "Pont PCI vers PCI",
            (0x06, _) => "Pont",
            (0x07, _) => "Port de communication",
            (0x08, 0x80) => "Périphérique système",
            (0x08, _) => "Périphérique de base du système",
            (0x09, 0x00) => "Clavier",
            (0x09, 0x02) => "Souris",
            (0x09, _) => "Périphérique d'entrée",
            (0x0C, 0x03) => match self.interface {
                0x00 => "Contrôleur USB UHCI",
                0x10 => "Contrôleur USB OHCI",
                0x20 => "Contrôleur USB 2.0 EHCI",
                0x30 => "Contrôleur USB 3.0 xHCI",
                _ => "Contrôleur USB",
            },
            (0x0C, 0x05) => "Bus SMBus",
            (0x0C, _) => "Contrôleur de bus série",
            (0x0D, _) => "Contrôleur sans fil",
            (0x10, _) => "Contrôleur de chiffrement",
            _ => "Périphérique PCI",
        }
    }

    /// `00:02.0  Carte graphique  QEMU 1234:1111`, as Paramètres and lspci show it.
    pub fn line(self) -> String {
        format!(
            "{:02x}:{:02x}.{}  {}  {} {:04x}:{:04x}",
            self.bus,
            self.slot,
            self.function,
            self.kind(),
            self.vendor_name(),
            self.vendor,
            self.id
        )
    }
}
