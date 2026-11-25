// grenos-adaptateur-xbox — le pilote de l'adaptateur sans fil Xbox.
// Copyright (C) 2026 grenOS — GPL-2.0-or-later, voir LICENSE.

//! Les trois fonctions de la bibliothèque C dont on a besoin (ioctl, poll,
//! signal), déclarées ici plutôt que de tirer la caisse `libc` pour elles.
//! glibc et musl les exportent toutes les trois.

use std::os::raw::{c_int, c_short, c_ulong};

#[repr(C)]
pub struct PollFd {
    pub fd: c_int,
    pub events: c_short,
    pub revents: c_short,
}

pub const POLLIN: c_short = 0x001;

extern "C" {
    // glibc : `unsigned long request` ; musl : `int request`. Sur x86_64 les
    // deux passent par le même registre, et le noyau n'en lit que 32 bits :
    // les numéros d'ioctl d'ici (tous < 2³²) arrivent pareil des deux côtés.
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn poll(fds: *mut PollFd, nfds: c_ulong, timeout: c_int) -> c_int;
    fn signal(signum: c_int, handler: usize) -> usize;
}

pub const SIGINT: c_int = 2;
pub const SIGTERM: c_int = 15;
pub const SIGUSR1: c_int = 10;

/// _IOC du noyau : sens (2 bits), taille (14), type (8), numéro (8).
pub const fn ioc(sens: u32, genre: u8, numero: u8, taille: usize) -> c_ulong {
    ((sens as c_ulong) << 30) | ((taille as c_ulong) << 16) | ((genre as c_ulong) << 8) | numero as c_ulong
}
pub const AUCUN: u32 = 0;
pub const ECRIT: u32 = 1;
pub const LU: u32 = 2;

/// Un ioctl qui passe un pointeur. Rend l'erreur du système si -1.
///
/// # Safety
/// `arg` doit pointer sur la structure qu'attend `requete`, vivante le temps
/// de l'appel.
pub unsafe fn ioctl_ptr<T>(fd: c_int, requete: c_ulong, arg: *mut T) -> std::io::Result<c_int> {
    let r = ioctl(fd, requete, arg);
    if r < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(r)
    }
}

/// Un ioctl qui passe un entier (UI_SET_EVBIT…) ou rien (UI_DEV_CREATE).
pub fn ioctl_val(fd: c_int, requete: c_ulong, valeur: c_ulong) -> std::io::Result<c_int> {
    // SAFETY : aucun pointeur ne passe, le noyau lit un entier.
    let r = unsafe { ioctl(fd, requete, valeur) };
    if r < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(r)
    }
}

/// Attend que `fd` ait quelque chose à lire, `ms` millisecondes au plus.
pub fn attendre_lecture(fd: c_int, ms: c_int) -> bool {
    let mut p = PollFd { fd, events: POLLIN, revents: 0 };
    // SAFETY : un seul PollFd, vivant le temps de l'appel.
    let r = unsafe { poll(&mut p, 1, ms) };
    r > 0 && (p.revents & POLLIN) != 0
}

/// Pose un gestionnaire de signal (une fonction qui ne fait que lever un
/// drapeau atomique : c'est tout ce qu'un gestionnaire a le droit de faire).
pub fn sur_signal(numero: c_int, gestionnaire: extern "C" fn(c_int)) {
    // SAFETY : le gestionnaire est une fonction `extern "C"` statique.
    unsafe {
        signal(numero, gestionnaire as usize);
    }
}
