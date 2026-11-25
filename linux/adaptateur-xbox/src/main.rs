// grenos-adaptateur-xbox — le pilote de l'adaptateur sans fil Xbox.
// Copyright (C) 2019 Medusalix (xow, dont ce programme est la réécriture)
// Copyright (C) 2026 grenOS
//
// Ce programme est un logiciel libre ; vous pouvez le redistribuer ou le
// modifier selon les termes de la GNU General Public License, version 2 ou
// (à votre choix) toute version ultérieure, publiée par la Free Software
// Foundation. Il est distribué SANS AUCUNE GARANTIE. Voir le fichier LICENSE.

//! L'ADAPTATEUR SANS FIL XBOX SOUS grenOS (25 novembre).
//!
//! Grenofar : « mon USB pour connecter sans fil ne veut pas ». La clé
//! (045e:02fe) est une puce Wi-Fi MediaTek qui parle le protocole de
//! Microsoft ; le noyau Linux n'a pas de pilote pour elle (xone en est un,
//! hors du noyau : il faudrait le signer pour le Secure Boot). Ce programme
//! en est un qui tourne comme n'importe quel programme : il parle à la clé
//! par usbfs et crée les manettes par uinput. Rien à signer.
//!
//! C'est la réécriture de xow (Medusalix, 2019-2022, abandonné), dont il
//! garde chaque valeur envoyée à la puce. Le firmware de la clé appartient
//! à Microsoft : grenos-adaptateur-xbox-firmware le télécharge chez eux.
//!
//!   grenos-adaptateur-xbox                 attend la clé, puis la fait marcher
//!   grenos-adaptateur-xbox --firmware F    un autre fichier de firmware
//!   kill -USR1 <pid>                        ouvre l'appairage (comme le bouton)

mod adaptateur;
mod entree;
mod gip;
mod journal;
mod manette;
mod mt76;
mod sys;
mod trames;
mod usb;

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const FIRMWARE: &str = "/lib/firmware/xow_dongle.bin";

static ARRET: AtomicBool = AtomicBool::new(false);
static APPAIRAGE: AtomicBool = AtomicBool::new(false);

extern "C" fn sur_arret(_: i32) {
    ARRET.store(true, Ordering::SeqCst);
}

extern "C" fn sur_appairage(_: i32) {
    APPAIRAGE.store(true, Ordering::SeqCst);
}

fn main() {
    let mut chemin_firmware = FIRMWARE.to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--firmware" => match args.next() {
                Some(f) => chemin_firmware = f,
                None => quitter(2, "--firmware demande un fichier"),
            },
            "--version" => {
                let _ = writeln!(std::io::stdout(), "grenos-adaptateur-xbox {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--aide" | "--help" | "-h" => {
                let _ = writeln!(std::io::stdout(), "grenos-adaptateur-xbox [--firmware FICHIER] — le pilote de l'adaptateur sans fil Xbox");
                return;
            }
            autre => quitter(2, &format!("option inconnue : {autre}")),
        }
    }
    sys::sur_signal(sys::SIGINT, sur_arret);
    sys::sur_signal(sys::SIGTERM, sur_arret);
    sys::sur_signal(sys::SIGUSR1, sur_appairage);

    info!("grenos-adaptateur-xbox {} (d'après xow, de Medusalix)", env!("CARGO_PKG_VERSION"));
    let firmware = match std::fs::read(&chemin_firmware) {
        Ok(f) => f,
        Err(e) => quitter(
            1,
            &format!("le firmware {chemin_firmware} manque ({e}) : lance « sudo /usr/lib/grenos/grenos-adaptateur-xbox-firmware »"),
        ),
    };

    // L'adaptateur : udev lance ce service quand on le branche ; lancé à la
    // main, on l'attend.
    let mut annonce = true;
    let trouve = loop {
        if ARRET.load(Ordering::SeqCst) {
            return;
        }
        if let Some(t) = usb::chercher() {
            break t;
        }
        if annonce {
            info!("en attente de l'adaptateur sans fil Xbox…");
            annonce = false;
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    info!("adaptateur trouvé : 045e:{:04x} ({})", trouve.produit, trouve.noeud.display());

    let usb = match usb::Usb::ouvrir(&trouve) {
        Ok(u) => u,
        Err(e) => quitter(1, &format!("l'adaptateur ne s'ouvre pas : {e}")),
    };
    let mt = match mt76::Mt76::demarrer(usb, &firmware) {
        Ok(m) => m,
        Err(e) => quitter(1, &format!("l'adaptateur ne démarre pas : {e}")),
    };
    let (adaptateur, fils) = adaptateur::Adaptateur::lancer(mt);

    while !ARRET.load(Ordering::SeqCst) && !adaptateur.perdu() {
        if APPAIRAGE.swap(false, Ordering::SeqCst) {
            adaptateur.mode_appairage(true);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let perdu = adaptateur.perdu();
    info!("arrêt…");
    adaptateur.arreter(fils);
    if perdu {
        // Débranché : udev relancera le service au prochain branchement.
        // Encore là mais muet : une erreur, que systemd relance.
        if usb::chercher().is_some() {
            quitter(1, "l'adaptateur ne répond plus");
        }
        info!("adaptateur débranché");
    }
}

fn quitter(code: i32, raison: &str) -> ! {
    erreur!("{raison}");
    std::process::exit(code);
}
