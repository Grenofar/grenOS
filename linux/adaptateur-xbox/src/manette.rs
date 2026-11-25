// grenos-adaptateur-xbox — le pilote de l'adaptateur sans fil Xbox.
// Copyright (C) 2019 Medusalix (xow, controller/controller.cpp)
// Copyright (C) 2026 grenOS — GPL-2.0-or-later, voir LICENSE.

//! Une manette connectée : ses paquets GIP deviennent les événements de sa
//! manette virtuelle, et les vibrations demandées par les jeux repartent
//! vers elle.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::entree::{self, Axe, Demande, Description, EffetNoyau, Entree, *};
use crate::gip::{self, Message, Sequence};
use crate::{details, erreur, info};

/// Envoie un paquet GIP à cette manette (par l'adaptateur).
pub type Envoi = Arc<dyn Fn(&[u8]) -> bool + Send + Sync>;

/// Le nom sous lequel la manette apparaît. Celui de xow, que SDL et Steam
/// connaissent déjà.
const NOM: &str = "Xbox One Wireless Controller";
/// Mode compatibilité (GRENOS_XBOX_360=1) : certains jeux comparent le nom
/// à une valeur écrite en dur ; se dire manette Xbox 360 les contente.
const NOM_360: &str = "Microsoft X-Box 360 pad";
const PRODUIT_360: u16 = 0x028e;
const VERSION_360: u16 = 0x0104;

/// La vibration suivante est retardée de 10 ms : le firmware des manettes
/// perd des ordres trop rapprochés.
const PAUSE_VIBRATION: Duration = Duration::from_millis(10);

pub struct Manette {
    envoi: Envoi,
    sequence: Arc<Mutex<Sequence>>,
    entree: Option<Arc<Entree>>,
    arret: Arc<AtomicBool>,
    /// La dernière vibration demandée, pas encore envoyée.
    vibration: Arc<(Mutex<Option<[u8; 9]>>, Condvar)>,
    fils: Vec<JoinHandle<()>>,
    batterie: u8,
}

impl Manette {
    pub fn nouvelle(envoi: Envoi) -> Manette {
        Manette {
            envoi,
            sequence: Arc::new(Mutex::new(Sequence::default())),
            entree: None,
            arret: Arc::new(AtomicBool::new(false)),
            vibration: Arc::new((Mutex::new(None), Condvar::new())),
            fils: Vec::new(),
            batterie: 0xff,
        }
    }

    fn seq(&self) -> u8 {
        self.sequence.lock().unwrap_or_else(|e| e.into_inner()).suivante()
    }

    /// Un paquet GIP de la manette. Faux si l'accusé n'est pas parti.
    pub fn paquet(&mut self, donnees: &[u8]) -> bool {
        let Some((trame, charge)) = gip::lire(donnees) else { return true };
        if trame.genre & gip::TYPE_ACCUSE != 0 && !(self.envoi)(&gip::accuse(&trame)) {
            erreur!("l'accusé de réception n'est pas parti");
            return false;
        }
        // Les accessoires (casque branché sur la manette) : pas pour nous.
        if trame.appareil > 0 {
            return true;
        }
        match gip::interpreter(&trame, charge) {
            Some(Message::Annonce(a)) => self.annoncee(&a),
            Some(Message::Etat { niveau, genre }) => {
                // En charge, ou rien de changé : on ne dit rien.
                if genre != 0 && niveau != self.batterie {
                    const NIVEAUX: [&str; 4] = ["vide", "faible", "moyenne", "pleine"];
                    info!("batterie de la manette : {}", NIVEAUX[niveau as usize & 3]);
                    self.batterie = niveau;
                }
            }
            Some(Message::Guide(appuye)) => {
                if let Some(e) = &self.entree {
                    e.touche(BTN_MODE, appuye);
                    e.rapport();
                }
            }
            Some(Message::Serie(s)) => info!("numéro de série de la manette : {s}"),
            Some(Message::Entree(e)) => self.entrees(&e),
            None => {}
        }
        true
    }

    fn entrees(&self, i: &gip::Entree) {
        let Some(e) = &self.entree else { return };
        use gip::*;
        e.touche(BTN_START, i.bouton(B_MENU));
        e.touche(BTN_SELECT, i.bouton(B_VUE));
        e.touche(BTN_A, i.bouton(B_A));
        e.touche(BTN_B, i.bouton(B_B));
        e.touche(BTN_X, i.bouton(B_X));
        e.touche(BTN_Y, i.bouton(B_Y));
        e.touche(BTN_TL, i.bouton(B_LB));
        e.touche(BTN_TR, i.bouton(B_RB));
        e.touche(BTN_THUMBL, i.bouton(B_STICK_G));
        e.touche(BTN_THUMBR, i.bouton(B_STICK_D));
        e.axe(ABS_X, i.gauche_x as i32);
        e.axe(ABS_RX, i.droite_x as i32);
        // L'axe Y est inversé (vers le haut = négatif pour Linux), comme xow :
        // le complément à un, pour que -32768 donne 32767.
        e.axe(ABS_Y, !(i.gauche_y as i32));
        e.axe(ABS_RY, !(i.droite_y as i32));
        e.axe(ABS_Z, i.gachette_gauche as i32);
        e.axe(ABS_RZ, i.gachette_droite as i32);
        e.axe(ABS_HAT0X, i.bouton(B_DROITE) as i32 - i.bouton(B_GAUCHE) as i32);
        e.axe(ABS_HAT0Y, i.bouton(B_BAS) as i32 - i.bouton(B_HAUT) as i32);
        e.rapport();
    }

    /// La manette s'est annoncée : on la met en marche, puis on crée sa
    /// manette virtuelle.
    fn annoncee(&mut self, a: &gip::Annonce) {
        info!("manette annoncée (produit {:04x}, firmware {}.{}.{}.{})", a.produit, a.firmware[0], a.firmware[1], a.firmware[2], a.firmware[3]);
        if self.entree.is_some() {
            // Une seconde annonce (elle se reconnecte) : déjà prête.
            return;
        }
        // LED un peu atténuée, comme le pilote d'origine (0x14 sur 0x20).
        let s1 = self.seq();
        let s2 = self.seq();
        let s3 = self.seq();
        if !(self.envoi)(&gip::alimentation(s1, 0, gip::ALIM_MARCHE))
            || !(self.envoi)(&gip::led(s2, gip::LED_ALLUMEE, 0x14))
            || !(self.envoi)(&gip::demande_serie(s3))
        {
            erreur!("la manette ne répond pas à la mise en marche");
            return;
        }
        let compat = std::env::var_os("GRENOS_XBOX_360").is_some();
        let stick = |code| Axe { code, minimum: -32768, maximum: 32767, flou: 255, plat: 4095 };
        let gachette = |code| Axe { code, minimum: 0, maximum: 1023, flou: 3, plat: 63 };
        let croix = |code| Axe { code, minimum: -1, maximum: 1, flou: 0, plat: 0 };
        let axes = [
            stick(ABS_X),
            stick(ABS_RX),
            stick(ABS_Y),
            stick(ABS_RY),
            gachette(ABS_Z),
            gachette(ABS_RZ),
            croix(ABS_HAT0X),
            croix(ABS_HAT0Y),
        ];
        let touches = [BTN_MODE, BTN_START, BTN_SELECT, BTN_A, BTN_B, BTN_X, BTN_Y, BTN_TL, BTN_TR, BTN_THUMBL, BTN_THUMBR];
        let d = Description {
            nom: if compat { NOM_360 } else { NOM },
            fabricant: a.fabricant,
            produit: if compat { PRODUIT_360 } else { a.produit },
            // Comme xow : majeur << 8 | mineur, sur 16 bits.
            version: if compat { VERSION_360 } else { (a.firmware[0] << 8) | a.firmware[1] },
            touches: &touches,
            axes: &axes,
        };
        let entree = match Entree::creer(&d) {
            Ok(e) => Arc::new(e),
            Err(e) => {
                erreur!("la manette virtuelle n'a pas pu être créée (/dev/uinput) : {e}");
                return;
            }
        };
        info!("manette prête : « {} »", d.nom);
        self.entree = Some(entree.clone());
        self.lancer_vibrations(entree);
    }

    /// Deux fils : l'un écoute ce que le jeu demande à la manette virtuelle,
    /// l'autre envoie les vibrations à la vraie.
    fn lancer_vibrations(&mut self, entree: Arc<Entree>) {
        let (arret, vibration) = (self.arret.clone(), self.vibration.clone());
        self.fils.push(std::thread::spawn(move || {
            let mut effet: Option<EffetNoyau> = None;
            let mut gain: u16 = 0xffff;
            while !arret.load(Ordering::SeqCst) {
                match entree.demande(200) {
                    Some(Demande::Televerse(e)) => effet = Some(e),
                    Some(Demande::Efface) => effet = None,
                    Some(Demande::Gain(g)) => gain = g,
                    Some(Demande::Joue { id, repetitions }) => {
                        let Some(e) = effet.filter(|e| e.id == id) else { continue };
                        if e.genre != entree::FF_RUMBLE {
                            continue;
                        }
                        let v = gip::vibration_pour(
                            gain,
                            &gip::Effet { fort: e.fort(), faible: e.faible(), direction: e.direction, duree_ms: e.duree, delai_ms: e.delai },
                            repetitions as u8,
                        );
                        details!("vibration {v:?}");
                        let (attente, signal) = &*vibration;
                        *attente.lock().unwrap_or_else(|e| e.into_inner()) = Some(v);
                        signal.notify_one();
                    }
                    None => {}
                }
            }
        }));
        let (arret, vibration, envoi, sequence) = (self.arret.clone(), self.vibration.clone(), self.envoi.clone(), self.sequence.clone());
        self.fils.push(std::thread::spawn(move || {
            let (attente, signal) = &*vibration;
            loop {
                let v = {
                    let mut a = attente.lock().unwrap_or_else(|e| e.into_inner());
                    while a.is_none() && !arret.load(Ordering::SeqCst) {
                        a = signal.wait_timeout(a, Duration::from_millis(200)).unwrap_or_else(|e| e.into_inner()).0;
                    }
                    if arret.load(Ordering::SeqCst) {
                        return;
                    }
                    a.take()
                };
                if let Some(v) = v {
                    let s = sequence.lock().unwrap_or_else(|e| e.into_inner()).suivante();
                    envoi(&gip::vibration(s, &v));
                    std::thread::sleep(PAUSE_VIBRATION);
                }
            }
        }));
    }

    /// La manette part : ses fils s'arrêtent, sa manette virtuelle
    /// disparaît, et on lui dit de s'éteindre.
    pub fn fermer(&mut self) {
        self.arret.store(true, Ordering::SeqCst);
        self.vibration.1.notify_all();
        for f in self.fils.drain(..) {
            let _ = f.join();
        }
        self.entree = None;
        let s = self.seq();
        if !(self.envoi)(&gip::alimentation(s, 0, gip::ALIM_ARRET)) {
            erreur!("la manette ne s'éteint pas");
        }
    }
}
