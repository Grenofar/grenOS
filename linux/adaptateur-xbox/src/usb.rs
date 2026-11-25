// grenos-adaptateur-xbox — le pilote de l'adaptateur sans fil Xbox.
// Copyright (C) 2019 Medusalix (xow, dongle/usb.cpp)
// Copyright (C) 2026 grenOS — GPL-2.0-or-later, voir LICENSE.

//! L'adaptateur vu par usbfs (/dev/bus/usb/BBB/DDD) : transferts de contrôle
//! et transferts en bloc, ce que xow demandait à libusb.
//!
//! LE PILOTE WI-FI DU NOYAU PREND LA CLÉ. Dedans, c'est une puce Wi-Fi
//! MediaTek (MT76) : `mt76x2u` la réclame dès qu'elle est branchée, et elle
//! n'est alors plus à nous. On l'en détache (USBDEVFS_DISCONNECT) avant de la
//! remettre à zéro, puis encore après (la remise à zéro la lui rend). La
//! règle modprobe de grenOS l'empêche d'y revenir au branchement suivant.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::raw::{c_int, c_uint, c_void};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::sys::{self, ioc, AUCUN, ECRIT, LU};
use crate::{details, erreur};

/// Microsoft.
pub const FABRICANT: u16 = 0x045e;
/// L'adaptateur d'origine (2015), le petit (2017, « pour Windows »), celui
/// qu'a la Surface Book 2 dans le clavier.
pub const PRODUITS: [u16; 3] = [0x02e6, 0x02fe, 0x091e];

const DELAI_MS: u32 = 1000;
pub const TAILLE_LECTURE: usize = 512;

#[repr(C)]
struct Controle {
    type_requete: u8,
    requete: u8,
    valeur: u16,
    index: u16,
    longueur: u16,
    delai: u32,
    donnees: *mut c_void,
}

#[repr(C)]
struct EnBloc {
    point: c_uint,
    longueur: c_uint,
    delai: c_uint,
    donnees: *mut c_void,
}

#[repr(C)]
struct IoctlInterface {
    interface: c_int,
    code: c_int,
    donnees: *mut c_void,
}

const USBDEVFS_CONTROL: u64 = ioc(LU | ECRIT, b'U', 0, std::mem::size_of::<Controle>());
const USBDEVFS_BULK: u64 = ioc(LU | ECRIT, b'U', 2, std::mem::size_of::<EnBloc>());
const USBDEVFS_SETCONFIGURATION: u64 = ioc(LU, b'U', 5, std::mem::size_of::<c_uint>());
const USBDEVFS_CLAIMINTERFACE: u64 = ioc(LU, b'U', 15, std::mem::size_of::<c_uint>());
const USBDEVFS_RELEASEINTERFACE: u64 = ioc(LU, b'U', 16, std::mem::size_of::<c_uint>());
const USBDEVFS_IOCTL: u64 = ioc(LU | ECRIT, b'U', 18, std::mem::size_of::<IoctlInterface>());
const USBDEVFS_RESET: u64 = ioc(AUCUN, b'U', 20, 0);
const USBDEVFS_DISCONNECT: u64 = ioc(AUCUN, b'U', 22, 0);

// Les tailles que le noyau attend (linux/usbdevice_fs.h, x86_64) : une
// erreur ici changerait le numéro de l'ioctl, et le noyau dirait ENOTTY.
const _: () = assert!(std::mem::size_of::<Controle>() == 24);
const _: () = assert!(std::mem::size_of::<EnBloc>() == 24);
const _: () = assert!(std::mem::size_of::<IoctlInterface>() == 16);
const _: () = assert!(USBDEVFS_CONTROL == 0xc018_5500);
const _: () = assert!(USBDEVFS_BULK == 0xc018_5502);
const _: () = assert!(USBDEVFS_SETCONFIGURATION == 0x8004_5505);
const _: () = assert!(USBDEVFS_CLAIMINTERFACE == 0x8004_550f);
const _: () = assert!(USBDEVFS_RELEASEINTERFACE == 0x8004_5510);
const _: () = assert!(USBDEVFS_IOCTL == 0xc010_5512);
const _: () = assert!(USBDEVFS_RESET == 0x5514);
const _: () = assert!(USBDEVFS_DISCONNECT == 0x5516);

/// Où est l'adaptateur : son dossier dans /sys et son nœud dans /dev.
pub struct Trouve {
    pub sys: PathBuf,
    pub noeud: PathBuf,
    pub produit: u16,
}

fn lire_hex(chemin: &Path) -> Option<u16> {
    u16::from_str_radix(std::fs::read_to_string(chemin).ok()?.trim(), 16).ok()
}

fn lire_dec(chemin: &Path) -> Option<u32> {
    std::fs::read_to_string(chemin).ok()?.trim().parse().ok()
}

/// Le premier adaptateur branché, s'il y en a un.
pub fn chercher() -> Option<Trouve> {
    let mut entrees: Vec<_> = std::fs::read_dir("/sys/bus/usb/devices").ok()?.flatten().collect();
    entrees.sort_by_key(|e| e.file_name());
    for e in entrees {
        let nom = e.file_name();
        // « 1-2:1.0 » est une interface ; l'appareil est « 1-2 ».
        if nom.to_string_lossy().contains(':') {
            continue;
        }
        let d = e.path();
        if lire_hex(&d.join("idVendor")) != Some(FABRICANT) {
            continue;
        }
        let Some(produit) = lire_hex(&d.join("idProduct")).filter(|p| PRODUITS.contains(p)) else { continue };
        let (Some(bus), Some(num)) = (lire_dec(&d.join("busnum")), lire_dec(&d.join("devnum"))) else { continue };
        return Some(Trouve { sys: d, noeud: PathBuf::from(format!("/dev/bus/usb/{bus:03}/{num:03}")), produit });
    }
    None
}

pub struct Usb {
    fichier: File,
    sys: PathBuf,
    /// Une erreur de transfert : l'adaptateur est parti, ou ne répond plus.
    /// Tout s'arrête (xow appelait cela « terminate »).
    perdu: AtomicBool,
}

impl Usb {
    pub fn ouvrir(t: &Trouve) -> io::Result<Usb> {
        let fichier = OpenOptions::new().read(true).write(true).open(&t.noeud)?;
        let usb = Usb { fichier, sys: t.sys.clone(), perdu: AtomicBool::new(false) };
        usb.detacher();
        // SAFETY : USBDEVFS_RESET ne prend pas d'argument.
        sys::ioctl_val(usb.fd(), USBDEVFS_RESET, 0).map_err(|e| contexte(e, "remise à zéro de l'adaptateur"))?;
        // Le noyau relance ses pilotes après la remise à zéro : mt76x2u
        // reprendrait la clé si la règle modprobe n'est pas encore là.
        std::thread::sleep(Duration::from_millis(200));
        usb.detacher();
        let mut config: c_uint = 1;
        // SAFETY : un entier, vivant le temps de l'appel.
        if let Err(e) = unsafe { sys::ioctl_ptr(usb.fd(), USBDEVFS_SETCONFIGURATION, &mut config) } {
            // Déjà sur la configuration 1 (le cas courant) : rien à faire.
            if lire_dec(&usb.sys.join("bConfigurationValue")) != Some(1) {
                return Err(contexte(e, "choix de la configuration USB"));
            }
            details!("configuration déjà choisie ({e})");
        }
        let mut interface: c_uint = 0;
        // SAFETY : idem.
        if unsafe { sys::ioctl_ptr(usb.fd(), USBDEVFS_CLAIMINTERFACE, &mut interface) }.is_err() {
            usb.detacher();
            // SAFETY : idem.
            unsafe { sys::ioctl_ptr(usb.fd(), USBDEVFS_CLAIMINTERFACE, &mut interface) }
                .map_err(|e| contexte(e, "prise de l'interface 0"))?;
        }
        Ok(usb)
    }

    fn fd(&self) -> c_int {
        self.fichier.as_raw_fd()
    }

    /// Retire le pilote du noyau de l'interface 0 (mt76x2u), s'il y en a un.
    fn detacher(&self) {
        let mut demande = IoctlInterface { interface: 0, code: USBDEVFS_DISCONNECT as c_int, donnees: std::ptr::null_mut() };
        // SAFETY : la structure vit le temps de l'appel.
        match unsafe { sys::ioctl_ptr(self.fd(), USBDEVFS_IOCTL, &mut demande) } {
            Ok(_) => crate::info!("pilote Wi-Fi du noyau détaché de l'adaptateur"),
            // ENODATA : aucun pilote dessus, c'est ce qu'on veut.
            Err(e) if e.raw_os_error() == Some(61) => {}
            Err(e) => details!("détacher : {e}"),
        }
    }

    pub fn perdu(&self) -> bool {
        self.perdu.load(Ordering::SeqCst)
    }

    fn perdre(&self, quoi: &str, e: &io::Error) {
        if !self.perdu.swap(true, Ordering::SeqCst) {
            erreur!("{quoi} : {e}");
        }
    }

    /// Un transfert de contrôle « fabricant ». `ecrire` : vers l'adaptateur.
    pub fn controle(&self, requete: u8, valeur: u16, index: u16, donnees: &mut [u8], ecrire: bool) -> bool {
        let mut c = Controle {
            // Fabricant | appareil | sens.
            type_requete: 0x40 | if ecrire { 0x00 } else { 0x80 },
            requete,
            valeur,
            index,
            longueur: donnees.len() as u16,
            delai: DELAI_MS,
            donnees: if donnees.is_empty() { std::ptr::null_mut() } else { donnees.as_mut_ptr().cast() },
        };
        // SAFETY : `donnees` vit le temps de l'appel et a la longueur annoncée.
        match unsafe { sys::ioctl_ptr(self.fd(), USBDEVFS_CONTROL, &mut c) } {
            Ok(n) if n as usize == donnees.len() => true,
            Ok(n) => {
                self.perdre("transfert de contrôle", &io::Error::other(format!("{n} octets sur {}", donnees.len())));
                false
            }
            Err(e) => {
                self.perdre("transfert de contrôle", &e);
                false
            }
        }
    }

    /// Lit un paquet sur le point `point` (sans le bit 0x80). Rend 0 si rien
    /// n'est venu dans la seconde, None si l'adaptateur est perdu.
    pub fn lire(&self, point: u8, tampon: &mut [u8]) -> Option<usize> {
        let mut b = EnBloc {
            point: (point | 0x80) as c_uint,
            longueur: tampon.len() as c_uint,
            delai: DELAI_MS,
            donnees: tampon.as_mut_ptr().cast(),
        };
        // SAFETY : `tampon` vit le temps de l'appel et a la longueur annoncée.
        match unsafe { sys::ioctl_ptr(self.fd(), USBDEVFS_BULK, &mut b) } {
            Ok(n) => Some(n as usize),
            // ETIMEDOUT : rien à lire, on recommence.
            Err(e) if e.raw_os_error() == Some(110) => Some(0),
            Err(e) => {
                self.perdre("lecture USB", &e);
                None
            }
        }
    }

    /// Écrit un paquet sur le point `point`.
    pub fn ecrire(&self, point: u8, donnees: &[u8]) -> bool {
        let mut copie = donnees.to_vec();
        let mut b = EnBloc {
            point: point as c_uint,
            longueur: copie.len() as c_uint,
            delai: DELAI_MS,
            donnees: copie.as_mut_ptr().cast(),
        };
        // SAFETY : `copie` vit le temps de l'appel et a la longueur annoncée.
        match unsafe { sys::ioctl_ptr(self.fd(), USBDEVFS_BULK, &mut b) } {
            Ok(n) if n as usize == donnees.len() => true,
            Ok(n) => {
                self.perdre("écriture USB", &io::Error::other(format!("{n} octets sur {}", donnees.len())));
                false
            }
            Err(e) => {
                self.perdre("écriture USB", &e);
                false
            }
        }
    }

}

impl Drop for Usb {
    fn drop(&mut self) {
        let mut interface: c_uint = 0;
        // SAFETY : un entier, vivant le temps de l'appel.
        let _ = unsafe { sys::ioctl_ptr(self.fd(), USBDEVFS_RELEASEINTERFACE, &mut interface) };
    }
}

fn contexte(e: io::Error, quoi: &str) -> io::Error {
    io::Error::new(e.kind(), format!("{quoi} : {e}"))
}
