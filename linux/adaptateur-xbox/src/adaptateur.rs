// grenos-adaptateur-xbox — le pilote de l'adaptateur sans fil Xbox.
// Copyright (C) 2019 Medusalix (xow, dongle/dongle.cpp)
// Copyright (C) 2026 grenOS — GPL-2.0-or-later, voir LICENSE.

//! L'adaptateur en marche : deux fils lisent ce qui arrive (événements de
//! la puce, trames 802.11) et le distribuent aux manettes. Le bouton de
//! l'adaptateur ouvre l'appairage.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::manette::{Envoi, Manette};
use crate::mt76::{Mt76, NOMBRE_WCID, POINT_LECTURE, POINT_LECTURE_PAQUETS};
use crate::trames::*;
use crate::usb::TAILLE_LECTURE;
use crate::{details, erreur, info};

pub struct Adaptateur {
    mt: Arc<Mt76>,
    manettes: Mutex<Vec<Option<Manette>>>,
    arret: AtomicBool,
}

impl Adaptateur {
    /// Lance la lecture. Rend l'adaptateur et ses deux fils de lecture.
    pub fn lancer(mt: Mt76) -> (Arc<Adaptateur>, Vec<JoinHandle<()>>) {
        let a = Arc::new(Adaptateur {
            mt: Arc::new(mt),
            manettes: Mutex::new((0..NOMBRE_WCID).map(|_| None).collect()),
            arret: AtomicBool::new(false),
        });
        let fils = [POINT_LECTURE, POINT_LECTURE_PAQUETS]
            .into_iter()
            .map(|point| {
                let a = a.clone();
                std::thread::spawn(move || a.lire(point))
            })
            .collect();
        info!("adaptateur prêt : appuie sur son bouton, puis sur celui de la manette, pour l'appairer");
        (a, fils)
    }

    pub fn perdu(&self) -> bool {
        self.mt.usb.perdu()
    }

    pub fn mode_appairage(&self, actif: bool) {
        self.mt.mode_appairage(actif);
    }

    fn lire(&self, point: u8) {
        let mut tampon = vec![0u8; TAILLE_LECTURE];
        while !self.arret.load(Ordering::SeqCst) {
            match self.mt.usb.lire(point, &mut tampon) {
                None => break,
                Some(0) => {}
                Some(n) => self.recu(&tampon[..n]),
            }
        }
    }

    /// Une lecture : un mot d'information, le paquet, puis 4 octets de fin
    /// (identiques au mot de tête).
    fn recu(&self, donnees: &[u8]) {
        if donnees.len() <= 4 + 4 {
            return;
        }
        let info = InfoRecue(lire_mot(donnees, 0));
        let paquet = &donnees[4..donnees.len() - 4];
        details!("reçu {} octets : port {}, événement {:#x}, 802.11 {}", donnees.len(), info.port(), info.evenement(), info.est_80211());
        match info.port() {
            PORT_CPU_RX => match info.evenement() {
                EVT_BOUTON => {
                    details!("bouton de l'adaptateur");
                    self.mt.mode_appairage(true);
                }
                EVT_PAQUET => self.trame(paquet),
                // Le paquet n'est jamais vide ici (plus de 8 octets lus).
                EVT_CLIENT_PERDU => self.deconnectee(paquet[0]),
                _ => {}
            },
            PORT_WLAN if info.est_80211() => self.trame(paquet),
            _ => {}
        }
    }

    fn trame(&self, paquet: &[u8]) {
        let Some(t) = lire_trame(paquet) else {
            details!("trame trop courte ({} octets)", paquet.len());
            return;
        };
        details!(
            "trame {}/{:#x} de {} pour {} (WCID {})",
            t.genre,
            t.sous_genre,
            crate::journal::adresse(&t.source),
            crate::journal::adresse(&t.destination),
            t.wcid
        );
        // Pas pour nous.
        if t.destination != self.mt.mac {
            return;
        }
        match (t.genre, t.sous_genre) {
            (WLAN_GESTION, SOUS_ASSOCIATION_DEMANDE) => self.connectee(&t.source),
            // Les manettes « 1537 » s'associent, se désassocient et se
            // réassocient pendant l'appairage, sans « client perdu ».
            (WLAN_GESTION, SOUS_DESASSOCIATION) => self.deconnectee(t.wcid),
            (WLAN_GESTION, SOUS_RESERVE) => self.demande_appairage(&t.source, t.charge),
            (WLAN_DONNEES, SOUS_DONNEES_QOS) => self.paquet_manette(t.wcid, t.charge),
            _ => {}
        }
    }

    fn connectee(&self, adresse: &[u8; 6]) {
        let mut manettes = self.manettes.lock().unwrap_or_else(|e| e.into_inner());
        let wcid = self.mt.associer(adresse);
        if wcid == 0 {
            return;
        }
        let (mt, adresse_copie) = (self.mt.clone(), *adresse);
        let envoi: Envoi = Arc::new(move |paquet: &[u8]| mt.envoyer(wcid, &adresse_copie, paquet));
        manettes[wcid as usize - 1] = Some(Manette::nouvelle(envoi));
        info!("manette {wcid} connectée ({})", crate::journal::adresse(adresse));
    }

    fn deconnectee(&self, wcid: u8) {
        if wcid == 0 || wcid as usize > NOMBRE_WCID {
            return;
        }
        let mut manettes = self.manettes.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut m) = manettes[wcid as usize - 1].take() else { return };
        m.fermer();
        if self.mt.retirer(wcid) {
            info!("manette {wcid} déconnectée");
        }
    }

    fn demande_appairage(&self, adresse: &[u8; 6], charge: &[u8]) {
        // Type 0x01 : une demande d'appairage.
        if charge.len() < 2 || charge[1] != 0x01 {
            return;
        }
        if !self.mt.appairer(adresse) {
            erreur!("la manette {} n'a pas été appairée", crate::journal::adresse(adresse));
            return;
        }
        self.mt.mode_appairage(false);
        info!("manette appairée : {}", crate::journal::adresse(adresse));
    }

    fn paquet_manette(&self, wcid: u8, charge: &[u8]) {
        if wcid == 0 || wcid as usize > NOMBRE_WCID {
            return;
        }
        // Contrôle QoS (2 octets) et 2 octets de bourrage.
        if charge.len() <= 4 {
            return;
        }
        let mut manettes = self.manettes.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(m) = manettes[wcid as usize - 1].as_mut() {
            if !m.paquet(&charge[4..]) {
                erreur!("paquet de la manette {wcid} non traité");
            }
        }
    }

    /// L'arrêt : les lectures cessent, chaque manette est éteinte, puis la
    /// LED et la radio de l'adaptateur.
    pub fn arreter(&self, fils: Vec<JoinHandle<()>>) {
        self.arret.store(true, Ordering::SeqCst);
        for f in fils {
            let _ = f.join();
        }
        let mut manettes = self.manettes.lock().unwrap_or_else(|e| e.into_inner());
        for m in manettes.iter_mut().filter_map(Option::take) {
            let mut m = m;
            m.fermer();
        }
        drop(manettes);
        if !self.perdu() {
            self.mt.eteindre();
        }
    }
}
