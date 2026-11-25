// grenos-adaptateur-xbox — le pilote de l'adaptateur sans fil Xbox.
// Copyright (C) 2019 Medusalix (xow, controller/input.cpp)
// Copyright (C) 2026 grenOS — GPL-2.0-or-later, voir LICENSE.

//! La manette telle que la voient les jeux : un périphérique d'entrée
//! virtuel (uinput), avec ses boutons, ses axes, et la vibration (FF_RUMBLE).
//! Steam, Proton, SDL la reconnaissent comme une manette Xbox ordinaire.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::raw::c_ulong;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

use crate::sys::{self, ioc, AUCUN, ECRIT, LU};

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;
pub const EV_FF: u16 = 0x15;
pub const EV_UINPUT: u16 = 0x0101;
pub const UI_FF_UPLOAD: u16 = 1;
pub const UI_FF_ERASE: u16 = 2;
pub const FF_RUMBLE: u16 = 0x50;
pub const FF_GAIN: u16 = 0x60;
const BUS_USB: u16 = 0x03;

pub const BTN_A: u16 = 0x130;
pub const BTN_B: u16 = 0x131;
pub const BTN_X: u16 = 0x133;
pub const BTN_Y: u16 = 0x134;
pub const BTN_TL: u16 = 0x136;
pub const BTN_TR: u16 = 0x137;
pub const BTN_SELECT: u16 = 0x13a;
pub const BTN_START: u16 = 0x13b;
pub const BTN_MODE: u16 = 0x13c;
pub const BTN_THUMBL: u16 = 0x13d;
pub const BTN_THUMBR: u16 = 0x13e;
pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_Z: u16 = 0x02;
pub const ABS_RX: u16 = 0x03;
pub const ABS_RY: u16 = 0x04;
pub const ABS_RZ: u16 = 0x05;
pub const ABS_HAT0X: u16 = 0x10;
pub const ABS_HAT0Y: u16 = 0x11;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Evenement {
    secondes: i64,
    microsecondes: i64,
    genre: u16,
    code: u16,
    valeur: i32,
}

#[repr(C)]
struct Identite {
    bus: u16,
    fabricant: u16,
    produit: u16,
    version: u16,
}

#[repr(C)]
struct Installation {
    id: Identite,
    nom: [u8; 80],
    effets_max: u32,
}

#[repr(C)]
struct InfoAxe {
    valeur: i32,
    minimum: i32,
    maximum: i32,
    flou: i32,
    plat: i32,
    resolution: i32,
}

#[repr(C)]
struct InstallationAxe {
    code: u16,
    _trou: u16,
    info: InfoAxe,
}

/// struct ff_effect (48 octets) : l'union commence à l'octet 16, alignée sur
/// 8 (elle contient un pointeur).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct EffetNoyau {
    pub genre: u16,
    pub id: i16,
    pub direction: u16,
    pub declencheur_bouton: u16,
    pub declencheur_intervalle: u16,
    pub duree: u16,
    pub delai: u16,
    _trou: u16,
    pub union_: [u64; 4],
}

impl EffetNoyau {
    /// Pour FF_RUMBLE : magnitude forte (16 premiers bits de l'union),
    /// faible (16 suivants).
    pub fn fort(&self) -> u16 {
        self.union_[0] as u16
    }
    pub fn faible(&self) -> u16 {
        (self.union_[0] >> 16) as u16
    }
}

#[repr(C)]
struct Televersement {
    demande: u32,
    retour: i32,
    effet: EffetNoyau,
    ancien: EffetNoyau,
}

#[repr(C)]
struct Effacement {
    demande: u32,
    retour: i32,
    effet: u32,
}

const UI_SET_EVBIT: c_ulong = ioc(ECRIT, b'U', 100, 4);
const UI_SET_KEYBIT: c_ulong = ioc(ECRIT, b'U', 101, 4);
const UI_SET_ABSBIT: c_ulong = ioc(ECRIT, b'U', 103, 4);
const UI_SET_FFBIT: c_ulong = ioc(ECRIT, b'U', 107, 4);
const UI_DEV_CREATE: c_ulong = ioc(AUCUN, b'U', 1, 0);
const UI_DEV_DESTROY: c_ulong = ioc(AUCUN, b'U', 2, 0);
const UI_DEV_SETUP: c_ulong = ioc(ECRIT, b'U', 3, std::mem::size_of::<Installation>());
const UI_ABS_SETUP: c_ulong = ioc(ECRIT, b'U', 4, std::mem::size_of::<InstallationAxe>());
const UI_BEGIN_FF_UPLOAD: c_ulong = ioc(LU | ECRIT, b'U', 200, std::mem::size_of::<Televersement>());
const UI_END_FF_UPLOAD: c_ulong = ioc(ECRIT, b'U', 201, std::mem::size_of::<Televersement>());
const UI_BEGIN_FF_ERASE: c_ulong = ioc(LU | ECRIT, b'U', 202, std::mem::size_of::<Effacement>());
const UI_END_FF_ERASE: c_ulong = ioc(ECRIT, b'U', 203, std::mem::size_of::<Effacement>());

// Les tailles et numéros de linux/uinput.h et linux/input.h (x86_64).
const _: () = assert!(std::mem::size_of::<Evenement>() == 24);
const _: () = assert!(std::mem::size_of::<Installation>() == 92);
const _: () = assert!(std::mem::size_of::<InstallationAxe>() == 28);
const _: () = assert!(std::mem::size_of::<EffetNoyau>() == 48);
const _: () = assert!(std::mem::size_of::<Televersement>() == 104);
const _: () = assert!(std::mem::size_of::<Effacement>() == 12);
const _: () = assert!(UI_SET_EVBIT == 0x4004_5564);
const _: () = assert!(UI_DEV_CREATE == 0x5501);
const _: () = assert!(UI_DEV_SETUP == 0x405c_5503);
const _: () = assert!(UI_ABS_SETUP == 0x401c_5504);
const _: () = assert!(UI_BEGIN_FF_UPLOAD == 0xc068_55c8);
const _: () = assert!(UI_END_FF_UPLOAD == 0x4068_55c9);
const _: () = assert!(UI_BEGIN_FF_ERASE == 0xc00c_55ca);
const _: () = assert!(UI_END_FF_ERASE == 0x400c_55cb);

pub struct Axe {
    pub code: u16,
    pub minimum: i32,
    pub maximum: i32,
    pub flou: i32,
    pub plat: i32,
}

pub struct Description<'a> {
    pub nom: &'a str,
    pub fabricant: u16,
    pub produit: u16,
    pub version: u16,
    pub touches: &'a [u16],
    pub axes: &'a [Axe],
}

/// Ce que le jeu demande à la manette virtuelle.
pub enum Demande {
    /// Un effet vient d'être (re)défini.
    Televerse(EffetNoyau),
    Efface,
    Gain(u16),
    /// L'effet `id` joue `repetitions` fois (0 : stop).
    Joue { id: i16, repetitions: i32 },
}

pub struct Entree {
    fichier: File,
}

impl Entree {
    pub fn creer(d: &Description) -> io::Result<Entree> {
        let fichier = OpenOptions::new().read(true).write(true).custom_flags(0o4000 /* O_NONBLOCK */).open("/dev/uinput")?;
        let fd = fichier.as_raw_fd();
        for &t in d.touches {
            sys::ioctl_val(fd, UI_SET_EVBIT, EV_KEY as c_ulong)?;
            sys::ioctl_val(fd, UI_SET_KEYBIT, t as c_ulong)?;
        }
        for a in d.axes {
            sys::ioctl_val(fd, UI_SET_EVBIT, EV_ABS as c_ulong)?;
            sys::ioctl_val(fd, UI_SET_ABSBIT, a.code as c_ulong)?;
            let mut r = InstallationAxe {
                code: a.code,
                _trou: 0,
                info: InfoAxe { valeur: 0, minimum: a.minimum, maximum: a.maximum, flou: a.flou, plat: a.plat, resolution: 0 },
            };
            // SAFETY : la structure vit le temps de l'appel.
            unsafe { sys::ioctl_ptr(fd, UI_ABS_SETUP, &mut r)? };
        }
        sys::ioctl_val(fd, UI_SET_EVBIT, EV_FF as c_ulong)?;
        sys::ioctl_val(fd, UI_SET_FFBIT, FF_RUMBLE as c_ulong)?;
        let mut nom = [0u8; 80];
        let n = d.nom.len().min(79);
        nom[..n].copy_from_slice(&d.nom.as_bytes()[..n]);
        let mut installation = Installation {
            id: Identite { bus: BUS_USB, fabricant: d.fabricant, produit: d.produit, version: d.version },
            nom,
            effets_max: 1,
        };
        // SAFETY : idem.
        unsafe { sys::ioctl_ptr(fd, UI_DEV_SETUP, &mut installation)? };
        sys::ioctl_val(fd, UI_DEV_CREATE, 0)?;
        Ok(Entree { fichier })
    }

    fn emettre(&self, genre: u16, code: u16, valeur: i32) {
        let e = Evenement { genre, code, valeur, ..Default::default() };
        // SAFETY : Evenement est repr(C), 24 octets sans pointeur.
        let octets = unsafe { std::slice::from_raw_parts((&e as *const Evenement).cast::<u8>(), 24) };
        if let Err(err) = (&self.fichier).write_all(octets) {
            crate::erreur!("manette virtuelle : {err}");
        }
    }

    pub fn touche(&self, code: u16, appuyee: bool) {
        self.emettre(EV_KEY, code, appuyee as i32);
    }

    pub fn axe(&self, code: u16, valeur: i32) {
        self.emettre(EV_ABS, code, valeur);
    }

    pub fn rapport(&self) {
        self.emettre(EV_SYN, 0, 0);
    }

    /// La prochaine demande du jeu, en attendant `ms` au plus.
    pub fn demande(&self, ms: i32) -> Option<Demande> {
        let fd = self.fichier.as_raw_fd();
        if !sys::attendre_lecture(fd, ms) {
            return None;
        }
        let mut octets = [0u8; 24];
        match (&self.fichier).read(&mut octets) {
            Ok(24) => {}
            _ => return None,
        }
        // SAFETY : 24 octets lus, Evenement est repr(C) de 24 octets, toute
        // combinaison de bits y est valide.
        let e: Evenement = unsafe { std::ptr::read_unaligned(octets.as_ptr().cast()) };
        match (e.genre, e.code) {
            (EV_UINPUT, UI_FF_UPLOAD) => {
                // SAFETY : toute la structure est à zéro puis remplie par le noyau.
                let mut t: Televersement = unsafe { std::mem::zeroed() };
                t.demande = e.valeur as u32;
                // SAFETY : la structure vit le temps des deux appels.
                if unsafe { sys::ioctl_ptr(fd, UI_BEGIN_FF_UPLOAD, &mut t) }.is_err() {
                    return None;
                }
                let effet = t.effet;
                t.retour = 0;
                // SAFETY : idem.
                let _ = unsafe { sys::ioctl_ptr(fd, UI_END_FF_UPLOAD, &mut t) };
                Some(Demande::Televerse(effet))
            }
            (EV_UINPUT, UI_FF_ERASE) => {
                let mut f = Effacement { demande: e.valeur as u32, retour: 0, effet: 0 };
                // SAFETY : idem.
                if unsafe { sys::ioctl_ptr(fd, UI_BEGIN_FF_ERASE, &mut f) }.is_err() {
                    return None;
                }
                f.retour = 0;
                // SAFETY : idem.
                let _ = unsafe { sys::ioctl_ptr(fd, UI_END_FF_ERASE, &mut f) };
                Some(Demande::Efface)
            }
            (EV_FF, FF_GAIN) => Some(Demande::Gain(e.valeur as u16)),
            (EV_FF, code) => Some(Demande::Joue { id: code as i16, repetitions: e.valeur }),
            _ => None,
        }
    }
}

impl Drop for Entree {
    fn drop(&mut self) {
        let _ = sys::ioctl_val(self.fichier.as_raw_fd(), UI_DEV_DESTROY, 0);
    }
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn la_vibration_se_lit_dans_l_union() {
        let mut e = EffetNoyau { genre: FF_RUMBLE, ..Default::default() };
        e.union_[0] = 0x1234_abcd;
        assert_eq!((e.fort(), e.faible()), (0xabcd, 0x1234));
    }
}
