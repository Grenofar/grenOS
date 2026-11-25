// grenos-adaptateur-xbox — le pilote de l'adaptateur sans fil Xbox.
// Copyright (C) 2019 Medusalix (xow, controller/gip.cpp et controller.cpp)
// Copyright (C) 2026 grenOS — GPL-2.0-or-later, voir LICENSE.

//! GIP (Game Input Protocol) : ce que la manette et l'adaptateur se disent,
//! une fois la liaison radio établie. Une trame : commande, appareil (4 bits)
//! et type (4 bits), séquence, longueur, puis les données.
//!
//! Poignée de main : la manette s'annonce ; on la met en marche, on règle sa
//! LED, on demande son numéro de série. Ensuite elle envoie ses entrées.

pub const CMD_ACCUSE: u8 = 0x01;
pub const CMD_ANNONCE: u8 = 0x02;
pub const CMD_ETAT: u8 = 0x03;
pub const CMD_ALIMENTATION: u8 = 0x05;
pub const CMD_GUIDE: u8 = 0x07;
pub const CMD_VIBRATION: u8 = 0x09;
pub const CMD_LED: u8 = 0x0a;
pub const CMD_SERIE: u8 = 0x1e;
pub const CMD_ENTREE: u8 = 0x20;

/// Types : commande (pas de réponse), accusé demandé, requête.
pub const TYPE_COMMANDE: u8 = 0x00;
pub const TYPE_ACCUSE: u8 = 0x01;
pub const TYPE_REQUETE: u8 = 0x02;

pub const ALIM_MARCHE: u8 = 0x00;
pub const ALIM_ARRET: u8 = 0x04;
pub const LED_ALLUMEE: u8 = 0x01;

pub const TAILLE_ANNONCE: usize = 28;
pub const TAILLE_ETAT: usize = 4;
pub const TAILLE_GUIDE: usize = 2;
pub const TAILLE_SERIE: usize = 16;
pub const TAILLE_ENTREE: usize = 14;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trame {
    pub commande: u8,
    pub appareil: u8,
    pub genre: u8,
    pub sequence: u8,
    pub longueur: u8,
}

impl Trame {
    pub fn octets(&self) -> [u8; 4] {
        [self.commande, (self.appareil & 0xf) | (self.genre << 4), self.sequence, self.longueur]
    }
}

/// L'en-tête et les données d'un paquet GIP.
pub fn lire(paquet: &[u8]) -> Option<(Trame, &[u8])> {
    if paquet.len() < 4 {
        return None;
    }
    let t = Trame {
        commande: paquet[0],
        appareil: paquet[1] & 0xf,
        genre: paquet[1] >> 4,
        sequence: paquet[2],
        longueur: paquet[3],
    };
    Some((t, &paquet[4..]))
}

/// L'accusé de réception d'une trame qui en demande un.
pub fn accuse(t: &Trame) -> Vec<u8> {
    let entete = Trame { commande: CMD_ACCUSE, appareil: t.appareil, genre: TYPE_REQUETE, sequence: t.sequence, longueur: 4 + 5 };
    let rappel = Trame { commande: t.commande, appareil: t.appareil, genre: TYPE_REQUETE, sequence: t.longueur, longueur: 0 };
    let mut o = entete.octets().to_vec();
    o.push(0);
    o.extend_from_slice(&rappel.octets());
    o.extend_from_slice(&[0; 4]);
    o
}

fn trame(commande: u8, appareil: u8, genre: u8, sequence: u8, donnees: &[u8]) -> Vec<u8> {
    let mut o = Trame { commande, appareil, genre, sequence, longueur: donnees.len() as u8 }.octets().to_vec();
    o.extend_from_slice(donnees);
    o
}

pub fn alimentation(sequence: u8, appareil: u8, mode: u8) -> Vec<u8> {
    trame(CMD_ALIMENTATION, appareil, TYPE_REQUETE, sequence, &[mode])
}

/// La LED du bouton Xbox : mode, luminosité (0 à 0x20).
pub fn led(sequence: u8, mode: u8, luminosite: u8) -> Vec<u8> {
    trame(CMD_LED, 0, TYPE_REQUETE, sequence, &[0, mode, luminosite])
}

pub fn demande_serie(sequence: u8) -> Vec<u8> {
    trame(CMD_SERIE, 0, TYPE_REQUETE | TYPE_ACCUSE, sequence, &[0x04])
}

pub fn vibration(sequence: u8, v: &[u8; 9]) -> Vec<u8> {
    trame(CMD_VIBRATION, 0, TYPE_COMMANDE, sequence, v)
}

/// Les numéros de séquence : de 1 à 255, jamais 0.
#[derive(Default)]
pub struct Sequence(u8);

impl Sequence {
    pub fn suivante(&mut self) -> u8 {
        if self.0 == 0 {
            self.0 = 1;
        }
        let s = self.0;
        self.0 = self.0.wrapping_add(1);
        s
    }
}

// ---- Ce que dit la manette ---------------------------------------------------------
#[derive(Debug, PartialEq)]
pub struct Annonce {
    pub fabricant: u16,
    pub produit: u16,
    pub firmware: [u16; 4],
}

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct Entree {
    pub boutons: u16,
    pub gachette_gauche: u16,
    pub gachette_droite: u16,
    pub gauche_x: i16,
    pub gauche_y: i16,
    pub droite_x: i16,
    pub droite_y: i16,
}

impl Entree {
    pub fn bouton(&self, bit: u8) -> bool {
        self.boutons & (1 << bit) != 0
    }
}

// Les bits des boutons (mot de 16 bits).
pub const B_MENU: u8 = 2;
pub const B_VUE: u8 = 3;
pub const B_A: u8 = 4;
pub const B_B: u8 = 5;
pub const B_X: u8 = 6;
pub const B_Y: u8 = 7;
pub const B_HAUT: u8 = 8;
pub const B_BAS: u8 = 9;
pub const B_GAUCHE: u8 = 10;
pub const B_DROITE: u8 = 11;
pub const B_LB: u8 = 12;
pub const B_RB: u8 = 13;
pub const B_STICK_G: u8 = 14;
pub const B_STICK_D: u8 = 15;

#[derive(Debug, PartialEq)]
pub enum Message {
    Annonce(Annonce),
    Etat { niveau: u8, genre: u8 },
    Guide(bool),
    Serie(String),
    Entree(Entree),
}

fn u16_le(o: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([o[i], o[i + 1]])
}

/// Ce que veut dire une trame de la manette (appareil 0). Les autres
/// commandes, et les tailles inattendues, ne veulent rien dire pour nous.
pub fn interpreter(t: &Trame, d: &[u8]) -> Option<Message> {
    let l = t.longueur as usize;
    match t.commande {
        CMD_ANNONCE if l == TAILLE_ANNONCE && d.len() >= TAILLE_ANNONCE => Some(Message::Annonce(Annonce {
            fabricant: u16_le(d, 8),
            produit: u16_le(d, 10),
            firmware: [u16_le(d, 12), u16_le(d, 14), u16_le(d, 16), u16_le(d, 18)],
        })),
        CMD_ETAT if l == TAILLE_ETAT && d.len() >= TAILLE_ETAT => Some(Message::Etat { niveau: d[0] & 3, genre: (d[0] >> 2) & 3 }),
        CMD_GUIDE if l == TAILLE_GUIDE && d.len() >= TAILLE_GUIDE => Some(Message::Guide(d[0] != 0)),
        CMD_SERIE if l == TAILLE_SERIE && d.len() >= TAILLE_SERIE => {
            Some(Message::Serie(String::from_utf8_lossy(&d[2..16]).trim_end_matches('\0').to_string()))
        }
        // L'Elite envoie un paquet plus long (ses boutons d'origine ajoutés
        // à la fin) : on lit le début.
        CMD_ENTREE if l >= TAILLE_ENTREE && d.len() >= TAILLE_ENTREE => Some(Message::Entree(Entree {
            boutons: u16_le(d, 0),
            gachette_gauche: u16_le(d, 2),
            gachette_droite: u16_le(d, 4),
            gauche_x: u16_le(d, 6) as i16,
            gauche_y: u16_le(d, 8) as i16,
            droite_x: u16_le(d, 10) as i16,
            droite_y: u16_le(d, 12) as i16,
        })),
        _ => None,
    }
}

// ---- Les vibrations -------------------------------------------------------------------
/// Un effet de vibration demandé par un jeu (struct ff_effect du noyau).
#[derive(Clone, Copy, Debug, Default)]
pub struct Effet {
    pub fort: u16,
    pub faible: u16,
    pub direction: u16,
    pub duree_ms: u16,
    pub delai_ms: u16,
}

const PUISSANCE_MAX: u32 = 100;

/// Les 9 octets de vibration : inconnu, moteurs à régler (les quatre),
/// gâchette gauche, gâchette droite, gauche, droite, durée, délai, répétition
/// (durées en dizaines de ms). `repetitions` 0 ou `gain` 0 : tout s'arrête.
pub fn vibration_pour(gain: u16, effet: &Effet, repetitions: u8) -> [u8; 9] {
    let mut v = [0u8; 9];
    v[1] = 0x0f;
    if repetitions == 0 || gain == 0 {
        return v;
    }
    // Le gain appliqué. Comme xow : « fort » nourrit le moteur gauche.
    let faible = (effet.fort as u32 * gain as u32 / 0xffff) as u16;
    let fort = (effet.faible as u32 * gain as u32 / 0xffff) as u16;
    let gauche = (faible as u32 * PUISSANCE_MAX / 0xffff) as u8;
    let droite = (fort as u32 * PUISSANCE_MAX / 0xffff) as u8;
    v[4] = gauche;
    v[5] = droite;
    // Une direction vers le haut de la manette fait aussi vibrer les
    // gâchettes, à gauche ou à droite selon l'angle (décalé d'un huitième).
    if (0x4000..=0xc000).contains(&effet.direction) {
        let angle = effet.direction as f32 / 65535.0 - 0.125;
        let a = 2.0 * std::f64::consts::PI * angle as f64;
        let g = a.sin() as f32;
        let d = a.cos() as f32;
        let max = gauche.max(droite) as f32;
        let g = if g > 0.0 { g } else { 0.0 };
        let d = if d < 0.0 { -d } else { 0.0 };
        // Les moteurs des gâchettes sont très forts : moitié.
        v[2] = (g * max / 2.0) as u8;
        v[3] = (d * max / 2.0) as u8;
    }
    let mut duree = effet.duree_ms / 10;
    let mut delai = effet.delai_ms / 10;
    if duree == 0 || duree > 0xff {
        duree = 0xff;
    }
    if delai > 0xff {
        delai = 0xff;
    }
    v[6] = duree as u8;
    v[7] = delai as u8;
    v[8] = repetitions.wrapping_sub(1);
    v
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn une_trame_se_lit() {
        let (t, d) = lire(&[0x20, 0x00, 0x05, 0x0e, 1, 2]).unwrap();
        assert_eq!(t, Trame { commande: 0x20, appareil: 0, genre: 0, sequence: 5, longueur: 14 });
        assert_eq!(d, &[1, 2]);
        let (t, _) = lire(&[0x1e, 0x30, 0x07, 0x10]).unwrap();
        assert_eq!((t.genre, t.appareil), (TYPE_REQUETE | TYPE_ACCUSE, 0));
        assert!(lire(&[1, 2, 3]).is_none());
    }

    #[test]
    fn l_accuse_de_reception() {
        // Le numéro de série arrive avec un accusé demandé : séquence 7,
        // longueur 16.
        let t = Trame { commande: CMD_SERIE, appareil: 0, genre: TYPE_REQUETE | TYPE_ACCUSE, sequence: 7, longueur: 16 };
        assert_eq!(accuse(&t), vec![0x01, 0x20, 0x07, 0x09, 0x00, 0x1e, 0x20, 0x10, 0x00, 0, 0, 0, 0]);
    }

    #[test]
    fn ce_que_l_on_envoie() {
        assert_eq!(alimentation(1, 0, ALIM_MARCHE), vec![0x05, 0x20, 0x01, 0x01, 0x00]);
        assert_eq!(led(2, LED_ALLUMEE, 0x14), vec![0x0a, 0x20, 0x02, 0x03, 0x00, 0x01, 0x14]);
        assert_eq!(demande_serie(3), vec![0x1e, 0x30, 0x03, 0x01, 0x04]);
        let v = [0, 0x0f, 0, 0, 50, 25, 0xff, 0, 0];
        assert_eq!(vibration(4, &v), vec![0x09, 0x00, 0x04, 0x09, 0, 0x0f, 0, 0, 50, 25, 0xff, 0, 0]);
    }

    #[test]
    fn la_sequence_saute_zero() {
        let mut s = Sequence::default();
        assert_eq!((s.suivante(), s.suivante()), (1, 2));
        let mut s = Sequence(255);
        assert_eq!((s.suivante(), s.suivante()), (255, 1));
    }

    #[test]
    fn les_entrees() {
        let mut d = vec![0u8; 14];
        // A (bit 4), croix droite (11), gâchette gauche 1023, stick gauche
        // X = -32768, Y = 1000.
        d[0..2].copy_from_slice(&((1u16 << 4) | (1 << 11)).to_le_bytes());
        d[2..4].copy_from_slice(&1023u16.to_le_bytes());
        d[6..8].copy_from_slice(&(-32768i16).to_le_bytes());
        d[8..10].copy_from_slice(&1000i16.to_le_bytes());
        let t = Trame { commande: CMD_ENTREE, appareil: 0, genre: 0, sequence: 1, longueur: 14 };
        let Some(Message::Entree(e)) = interpreter(&t, &d) else { panic!() };
        assert!(e.bouton(B_A) && e.bouton(B_DROITE) && !e.bouton(B_B));
        assert_eq!((e.gachette_gauche, e.gauche_x, e.gauche_y), (1023, -32768, 1000));
        // L'Elite : plus long, même début.
        let mut elite = d.clone();
        elite.extend_from_slice(&[0; 14]);
        let t2 = Trame { longueur: 28, ..t };
        assert_eq!(interpreter(&t2, &elite), Some(Message::Entree(e)));
        // Trop court : rien.
        assert_eq!(interpreter(&t, &d[..10]), None);
    }

    #[test]
    fn l_annonce_et_le_reste() {
        let mut d = vec![0u8; 28];
        d[8..10].copy_from_slice(&0x045eu16.to_le_bytes());
        d[10..12].copy_from_slice(&0x0b12u16.to_le_bytes());
        d[12..14].copy_from_slice(&5u16.to_le_bytes());
        d[14..16].copy_from_slice(&17u16.to_le_bytes());
        let t = Trame { commande: CMD_ANNONCE, appareil: 0, genre: 0, sequence: 1, longueur: 28 };
        assert_eq!(interpreter(&t, &d), Some(Message::Annonce(Annonce { fabricant: 0x045e, produit: 0x0b12, firmware: [5, 17, 0, 0] })));
        let t = Trame { commande: CMD_ETAT, longueur: 4, ..t };
        assert_eq!(interpreter(&t, &[0b1110, 0, 0, 0]), Some(Message::Etat { niveau: 2, genre: 3 }));
        let t = Trame { commande: CMD_GUIDE, longueur: 2, ..t };
        assert_eq!(interpreter(&t, &[1, 0]), Some(Message::Guide(true)));
        let mut s = vec![0u8, 0];
        s.extend_from_slice(b"0123456789AB\0\0");
        let t = Trame { commande: CMD_SERIE, longueur: 16, ..t };
        assert_eq!(interpreter(&t, &s), Some(Message::Serie("0123456789AB".into())));
    }

    #[test]
    fn les_vibrations() {
        let e = Effet { fort: 0xffff, faible: 0x8000, direction: 0, duree_ms: 500, delai_ms: 0 };
        // Plein gain : gauche 100, droite 50 (0x8000 * 100 / 0xffff).
        assert_eq!(vibration_pour(0xffff, &e, 1), [0, 0x0f, 0, 0, 100, 50, 50, 0, 0]);
        // Arrêt.
        assert_eq!(vibration_pour(0xffff, &e, 0), [0, 0x0f, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(vibration_pour(0, &e, 1), [0, 0x0f, 0, 0, 0, 0, 0, 0, 0]);
        // Durée nulle : la plus longue (0xff) ; délai borné.
        let e2 = Effet { duree_ms: 0, delai_ms: 60000, ..e };
        let v = vibration_pour(0xffff, &e2, 3);
        assert_eq!((v[6], v[7], v[8]), (0xff, 0xff, 2));
        // Direction 0x8000 (le haut) : angle 0,375 → sin > 0, cos < 0 : les
        // deux gâchettes, ~0,707 × 100 / 2.
        let e3 = Effet { direction: 0x8000, ..e };
        let v = vibration_pour(0xffff, &e3, 1);
        assert_eq!((v[2], v[3]), (35, 35));
    }
}
