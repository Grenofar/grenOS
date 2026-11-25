// grenos-adaptateur-xbox — le pilote de l'adaptateur sans fil Xbox.
// Copyright (C) 2019 Medusalix (xow, dongle/mt76.h et mt76.cpp)
// Copyright (C) 2026 grenOS — GPL-2.0-or-later, voir LICENSE.

//! Les octets qu'échangent la puce MT76 et l'ordinateur, construits et lus
//! ici, sans aucun accès au matériel : tout ce fichier se vérifie par des
//! essais (en bas).
//!
//! xow décrivait ces trames par des structures C à champs de bits. GCC, sur
//! x86, range les champs de bits d'un mot de 32 bits du bit de poids faible
//! vers le fort, et le mot s'écrit en petit-boutiste : c'est exactement ce
//! que font les décalages ci-dessous.

/// Ce qu'il manque à `n` pour tomber sur un multiple de 4.
pub fn bourrage(n: usize) -> usize {
    (4 - n % 4) % 4
}

fn mot(valeur: u32) -> [u8; 4] {
    valeur.to_le_bytes()
}

pub fn lire_mot(octets: &[u8], position: usize) -> u32 {
    u32::from_le_bytes([octets[position], octets[position + 1], octets[position + 2], octets[position + 3]])
}

// ---- Les ports DMA et les types d'information ----------------------------
pub const PORT_WLAN: u32 = 0;
pub const PORT_CPU_RX: u32 = 1;
pub const PORT_CPU_TX: u32 = 2;
const INFO_NORMAL: u32 = 0;
const INFO_COMMANDE: u32 = 1;
/// File EDCA (accès au canal « amélioré »).
const QSEL_EDCA: u32 = 2;

// ---- Les commandes du microcontrôleur ---------------------------------------
pub const CMD_PACKET_TX: u8 = 0;
pub const CMD_FUN_SET_OP: u8 = 1;
pub const CMD_LOAD_CR: u8 = 2;
pub const CMD_INTERNAL_FW_OP: u8 = 3;
pub const CMD_BURST_WRITE: u8 = 8;
pub const CMD_LED_MODE_OP: u8 = 16;
pub const CMD_POWER_SAVING_OP: u8 = 20;
pub const CMD_SWITCH_CHANNEL_OP: u8 = 30;
pub const CMD_CALIBRATION_OP: u8 = 31;

/// Une commande pour le microcontrôleur de la puce : un mot d'information,
/// les données alignées sur 32 bits, puis 32 bits à zéro qui marquent la fin.
pub fn commande_mcu(commande: u8, donnees: &[u8]) -> Vec<u8> {
    let b = bourrage(donnees.len());
    // longueur (16 bits) | séquence 0 (4) | commande (7) | port (3) | type (2)
    let info = ((donnees.len() + b) as u32 & 0xffff)
        | ((commande as u32 & 0x7f) << 20)
        | (PORT_CPU_TX << 27)
        | (INFO_COMMANDE << 30);
    let mut sortie = Vec::with_capacity(4 + donnees.len() + b + 4);
    sortie.extend_from_slice(&mot(info));
    sortie.extend_from_slice(donnees);
    sortie.resize(sortie.len() + b + 4, 0);
    sortie
}

/// Un paquet 802.11 à émettre tel quel (TxWi en tête).
pub fn paquet_wlan(donnees: &[u8]) -> Vec<u8> {
    let b = bourrage(donnees.len());
    // longueur (16) | est 802.11 (bit 19) | TxWi valide (bit 24) | file (25-26)
    // | port WLAN (27-29) | type normal (30-31)
    let info = ((donnees.len() + b) as u32 & 0xffff)
        | (1 << 19)
        | (1 << 24)
        | (QSEL_EDCA << 25)
        | (PORT_WLAN << 27)
        | (INFO_NORMAL << 30);
    let mut sortie = Vec::with_capacity(4 + donnees.len() + b + 4);
    sortie.extend_from_slice(&mot(info));
    sortie.extend_from_slice(donnees);
    sortie.resize(sortie.len() + b + 4, 0);
    sortie
}

/// Un morceau du firmware envoyé au microcontrôleur. Pas d'alignement (xow
/// n'en mettait pas : les morceaux font 0x3800 octets, le dernier tel quel).
pub fn morceau_firmware(morceau: &[u8]) -> Vec<u8> {
    let info = (morceau.len() as u32 & 0xffff) | (PORT_CPU_TX << 27) | (INFO_NORMAL << 30);
    let mut sortie = Vec::with_capacity(4 + morceau.len() + 4);
    sortie.extend_from_slice(&mot(info));
    sortie.extend_from_slice(morceau);
    sortie.resize(sortie.len() + 4, 0);
    sortie
}

// ---- TxWi : ce qui précède chaque trame émise (20 octets) -------------------
pub const PHY_OFDM: u32 = 1;

#[derive(Default, Clone, Copy)]
pub struct TxWi {
    pub horodatage: bool,
    pub accuse: bool,
    pub sequence_materielle: bool,
    pub wcid: u8,
    pub octets_mpdu: u16,
}

impl TxWi {
    pub fn octets(&self) -> [u8; 20] {
        // Mot 0 : horodatage (bit 3), type PHY (29-31).
        let m0 = ((self.horodatage as u32) << 3) | (PHY_OFDM << 29);
        // Mot 1 : accusé (0), séquence matérielle (1), WCID (8-15),
        // longueur MPDU (16-29).
        let m1 = (self.accuse as u32)
            | ((self.sequence_materielle as u32) << 1)
            | ((self.wcid as u32) << 8)
            | ((self.octets_mpdu as u32 & 0x3fff) << 16);
        let mut o = [0u8; 20];
        o[0..4].copy_from_slice(&mot(m0));
        o[4..8].copy_from_slice(&mot(m1));
        o
    }
}

// ---- L'en-tête 802.11 (24 octets) ---------------------------------------------
pub const WLAN_GESTION: u8 = 0x00;
pub const WLAN_DONNEES: u8 = 0x02;
pub const SOUS_ASSOCIATION_DEMANDE: u8 = 0x00;
pub const SOUS_ASSOCIATION_REPONSE: u8 = 0x01;
pub const SOUS_RESERVE: u8 = 0x07;
pub const SOUS_BALISE: u8 = 0x08;
pub const SOUS_DESASSOCIATION: u8 = 0x0a;
pub const SOUS_DONNEES_QOS: u8 = 0x08;

pub const TAILLE_WLAN: usize = 24;
pub const TAILLE_RXWI: usize = 32;

/// Le champ de contrôle : version 0, type (bits 2-3), sous-type (4-7),
/// « depuis le système de distribution » (bit 9).
pub fn controle_trame(genre: u8, sous_genre: u8, depuis_ds: bool) -> u16 {
    ((genre as u16 & 3) << 2) | ((sous_genre as u16 & 0xf) << 4) | ((depuis_ds as u16) << 9)
}

pub fn entete_wlan(controle: u16, duree: u16, destination: &[u8; 6], source: &[u8; 6], bssid: &[u8; 6]) -> [u8; TAILLE_WLAN] {
    let mut o = [0u8; TAILLE_WLAN];
    o[0..2].copy_from_slice(&controle.to_le_bytes());
    o[2..4].copy_from_slice(&duree.to_le_bytes());
    o[4..10].copy_from_slice(destination);
    o[10..16].copy_from_slice(source);
    o[16..22].copy_from_slice(bssid);
    // Contrôle de séquence : 0 (le matériel s'en charge).
    o
}

/// La réponse d'association : l'adaptateur accepte la manette.
pub fn trame_association(mac: &[u8; 6], adresse: &[u8; 6]) -> Vec<u8> {
    // Capacités 0, statut 0x0110, identifiant 0x0f00 (les valeurs du
    // pilote d'origine), 8 octets inconnus.
    let mut reponse = [0u8; 14];
    reponse[2..4].copy_from_slice(&0x0110u16.to_le_bytes());
    reponse[4..6].copy_from_slice(&0x0f00u16.to_le_bytes());
    let txwi = TxWi { accuse: true, wcid: 0xff, octets_mpdu: (TAILLE_WLAN + reponse.len()) as u16, ..Default::default() };
    let mut sortie = Vec::new();
    sortie.extend_from_slice(&txwi.octets());
    sortie.extend_from_slice(&entete_wlan(controle_trame(WLAN_GESTION, SOUS_ASSOCIATION_REPONSE, false), 0, adresse, mac, mac));
    sortie.extend_from_slice(&reponse);
    sortie
}

/// La réponse à une demande d'appairage (trame « réservée »).
pub fn trame_appairage(mac: &[u8; 6], adresse: &[u8; 6]) -> Vec<u8> {
    const DONNEES: [u8; 11] = [0x70, 0x02, 0x00, 0x45, 0x55, 0x01, 0x0f, 0x8f, 0xff, 0x87, 0x1f];
    let txwi = TxWi { accuse: true, wcid: 0xff, octets_mpdu: (TAILLE_WLAN + DONNEES.len()) as u16, ..Default::default() };
    let mut sortie = Vec::new();
    sortie.extend_from_slice(&txwi.octets());
    sortie.extend_from_slice(&entete_wlan(controle_trame(WLAN_GESTION, SOUS_RESERVE, false), 0, adresse, mac, mac));
    sortie.extend_from_slice(&DONNEES);
    sortie
}

/// La balise que l'adaptateur émet toutes les 100 ms. `appairage` : les
/// manettes qu'on appaire la cherchent (l'octet 8 de l'élément 0xdd).
pub fn trame_balise(mac: &[u8; 6], appairage: bool) -> Vec<u8> {
    let element: [u8; 18] = [
        0xdd, 0x10, 0x00, 0x50, 0xf2, 0x11, 0x01, 0x10, appairage as u8, 0xa5, 0x30, 0x99, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    // Horodatage 0 (le matériel le met), intervalle 100 ms, capacités 0xc631,
    // SSID vide (élément 0, longueur 0).
    let mut balise = [0u8; 14];
    balise[8..10].copy_from_slice(&0x64u16.to_le_bytes());
    balise[10..12].copy_from_slice(&0xc631u16.to_le_bytes());
    let txwi = TxWi {
        horodatage: true,
        sequence_materielle: true,
        octets_mpdu: (TAILLE_WLAN + balise.len() + element.len()) as u16,
        ..Default::default()
    };
    let mut sortie = Vec::new();
    sortie.extend_from_slice(&txwi.octets());
    sortie.extend_from_slice(&entete_wlan(controle_trame(WLAN_GESTION, SOUS_BALISE, false), 0, &[0xff; 6], mac, mac));
    sortie.extend_from_slice(&balise);
    sortie.extend_from_slice(&element);
    sortie
}

/// Un paquet GIP pour la manette `wcid` (données QoS, depuis l'adaptateur).
/// C'est une commande CMD_PACKET_TX : le WCID en tête, en gros-boutiste.
pub fn trame_client(wcid: u8, mac: &[u8; 6], adresse: &[u8; 6], paquet: &[u8]) -> Vec<u8> {
    let txwi = TxWi { accuse: true, octets_mpdu: (TAILLE_WLAN + 2 + paquet.len()) as u16, ..Default::default() };
    // Durée : 144 µs, ce que met le pilote d'origine.
    let wlan = entete_wlan(controle_trame(WLAN_DONNEES, SOUS_DONNEES_QOS, true), 144, adresse, mac, mac);
    let mut sortie = Vec::new();
    sortie.extend_from_slice(&(wcid.wrapping_sub(1) as u32).to_be_bytes());
    sortie.extend_from_slice(&[0; 4]);
    sortie.extend_from_slice(&txwi.octets());
    sortie.extend_from_slice(&wlan);
    // Contrôle QoS : 0.
    sortie.extend_from_slice(&[0; 2]);
    sortie.resize(sortie.len() + bourrage(20 + TAILLE_WLAN + 2), 0);
    sortie.extend_from_slice(paquet);
    sortie.resize(sortie.len() + bourrage(paquet.len()), 0);
    sortie
}

/// Le réglage d'un canal (20 octets) : canal, flux 1 en émission et en
/// réception, largeur, puissance, balayage.
pub fn reglage_canal(canal: u8, largeur: u8, puissance: u8, balayage: bool) -> [u8; 20] {
    let mut o = [0u8; 20];
    o[0] = canal;
    o[4..6].copy_from_slice(&0x0101u16.to_le_bytes());
    o[16] = largeur;
    o[17] = puissance;
    o[18] = balayage as u8;
    o
}

// ---- Ce qui arrive de l'adaptateur -------------------------------------------
pub const EVT_BOUTON: u32 = 0x04;
pub const EVT_PAQUET: u32 = 0x0c;
pub const EVT_CLIENT_PERDU: u32 = 0x0e;

/// Le mot d'information en tête de chaque lecture.
#[derive(Clone, Copy, Debug)]
pub struct InfoRecue(pub u32);

impl InfoRecue {
    pub fn port(self) -> u32 {
        (self.0 >> 27) & 7
    }
    pub fn evenement(self) -> u32 {
        (self.0 >> 20) & 0xf
    }
    pub fn est_80211(self) -> bool {
        (self.0 >> 19) & 1 == 1
    }
}

/// Une trame 802.11 reçue, après son RxWi de 32 octets.
pub struct TrameRecue<'a> {
    pub wcid: u8,
    pub genre: u8,
    pub sous_genre: u8,
    pub destination: [u8; 6],
    pub source: [u8; 6],
    /// Ce qui suit l'en-tête 802.11.
    pub charge: &'a [u8],
}

pub fn lire_trame(paquet: &[u8]) -> Option<TrameRecue<'_>> {
    if paquet.len() <= TAILLE_RXWI + TAILLE_WLAN {
        return None;
    }
    let w = &paquet[TAILLE_RXWI..];
    let controle = u16::from_le_bytes([w[0], w[1]]);
    let mut destination = [0u8; 6];
    let mut source = [0u8; 6];
    destination.copy_from_slice(&w[4..10]);
    source.copy_from_slice(&w[10..16]);
    Some(TrameRecue {
        // Mot 1 du RxWi, bits 0-7.
        wcid: paquet[4],
        genre: ((controle >> 2) & 3) as u8,
        sous_genre: ((controle >> 4) & 0xf) as u8,
        destination,
        source,
        charge: &paquet[TAILLE_RXWI + TAILLE_WLAN..],
    })
}

// ---- Les registres du firmware de la puce --------------------------------------
/// L'en-tête du firmware : longueur de l'ILM (instructions), de la DLM
/// (données), puis 24 octets (versions, date) ; 32 octets en tout.
pub fn parties_firmware(firmware: &[u8]) -> Option<(&[u8], &[u8])> {
    if firmware.len() < 32 {
        return None;
    }
    let ilm = lire_mot(firmware, 0) as usize;
    let dlm = lire_mot(firmware, 4) as usize;
    let fin_ilm = 32usize.checked_add(ilm)?;
    let fin_dlm = fin_ilm.checked_add(dlm)?;
    if fin_dlm > firmware.len() {
        return None;
    }
    Some((&firmware[32..fin_ilm], &firmware[fin_ilm..fin_dlm]))
}

#[cfg(test)]
mod essais {
    use super::*;

    const MAC: [u8; 6] = [0x62, 0x45, 0xbd, 0x01, 0x02, 0x03];
    const MANETTE: [u8; 6] = [0x7e, 0xed, 0x11, 0x22, 0x33, 0x44];

    #[test]
    fn le_bourrage_aligne_sur_32_bits() {
        assert_eq!((bourrage(0), bourrage(1), bourrage(2), bourrage(3), bourrage(4), bourrage(46)), (0, 3, 2, 1, 0, 2));
    }

    #[test]
    fn une_commande_mcu() {
        // LED allumée : CMD_LED_MODE_OP (16), un mot.
        let c = commande_mcu(CMD_LED_MODE_OP, &1u32.to_le_bytes());
        // longueur 4 | commande 16 << 20 | port 2 << 27 | type 1 << 30
        let info: u32 = 4 | (16 << 20) | (2 << 27) | (1 << 30);
        assert_eq!(info, 0x5100_0004);
        assert_eq!(c, [0x04, 0x00, 0x00, 0x51, 1, 0, 0, 0, 0, 0, 0, 0]);
        // 10 octets (adresse MAC) : 2 de bourrage, longueur annoncée 12.
        let c = commande_mcu(CMD_INTERNAL_FW_OP, &[0u8; 10]);
        assert_eq!(c.len(), 4 + 12 + 4);
        assert_eq!(lire_mot(&c, 0) & 0xffff, 12);
        assert_eq!((lire_mot(&c, 0) >> 20) & 0x7f, 3);
    }

    #[test]
    fn un_paquet_wlan() {
        let p = paquet_wlan(&[1, 2, 3, 4, 5]);
        let info = lire_mot(&p, 0);
        assert_eq!(info & 0xffff, 8);
        assert_eq!(info, 8 | (1 << 19) | (1 << 24) | (2 << 25));
        assert_eq!(p.len(), 4 + 8 + 4);
        assert_eq!(&p[4..9], &[1, 2, 3, 4, 5]);
        assert!(p[9..].iter().all(|&o| o == 0));
    }

    #[test]
    fn le_txwi() {
        let t = TxWi { accuse: true, wcid: 0xff, octets_mpdu: 38, ..Default::default() }.octets();
        assert_eq!(&t[0..4], &[0, 0, 0, 0x20]);
        assert_eq!(&t[4..8], &[0x01, 0xff, 0x26, 0x00]);
        assert!(t[8..].iter().all(|&o| o == 0));
        let b = TxWi { horodatage: true, sequence_materielle: true, octets_mpdu: 56, ..Default::default() }.octets();
        assert_eq!(&b[0..8], &[0x08, 0, 0, 0x20, 0x02, 0x00, 0x38, 0x00]);
    }

    #[test]
    fn le_controle_de_trame() {
        assert_eq!(controle_trame(WLAN_GESTION, SOUS_BALISE, false), 0x0080);
        assert_eq!(controle_trame(WLAN_GESTION, SOUS_ASSOCIATION_REPONSE, false), 0x0010);
        assert_eq!(controle_trame(WLAN_GESTION, SOUS_RESERVE, false), 0x0070);
        assert_eq!(controle_trame(WLAN_DONNEES, SOUS_DONNEES_QOS, true), 0x0288);
    }

    #[test]
    fn l_association() {
        let a = trame_association(&MAC, &MANETTE);
        assert_eq!(a.len(), 20 + 24 + 14);
        assert_eq!(&a[20..22], &[0x10, 0x00]);
        assert_eq!(&a[24..30], &MANETTE);
        assert_eq!(&a[30..36], &MAC);
        assert_eq!(&a[36..42], &MAC);
        assert_eq!(&a[44..58], &[0, 0, 0x10, 0x01, 0x00, 0x0f, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn la_balise() {
        let b = trame_balise(&MAC, true);
        assert_eq!(b.len(), 20 + 24 + 14 + 18);
        assert_eq!(&b[24..30], &[0xff; 6]);
        assert_eq!(&b[44 + 8..44 + 12], &[0x64, 0x00, 0x31, 0xc6]);
        assert_eq!(b[44 + 14 + 8], 1);
        assert_eq!(trame_balise(&MAC, false)[44 + 14 + 8], 0);
    }

    #[test]
    fn un_paquet_pour_la_manette() {
        let gip = [0x05, 0x20, 0x01, 0x01, 0x00];
        let t = trame_client(3, &MAC, &MANETTE, &gip);
        // WCID 3 → 2, gros-boutiste, puis 4 zéros.
        assert_eq!(&t[0..8], &[0, 0, 0, 2, 0, 0, 0, 0]);
        // TxWi : accusé, WCID 0, MPDU 24 + 2 + 5 = 31.
        assert_eq!(&t[8..16], &[0, 0, 0, 0x20, 0x01, 0x00, 31, 0x00]);
        // 802.11 : données QoS depuis le DS, durée 144.
        assert_eq!(&t[28..32], &[0x88, 0x02, 0x90, 0x00]);
        // 8 + 20 + 24 + 2 = 54, plus 2 de bourrage = 56 ; puis 5 + 3.
        assert_eq!(&t[56..61], &gip);
        assert_eq!(t.len(), 64);
    }

    #[test]
    fn un_canal() {
        let c = reglage_canal(0x24, 1, 0x1a, true);
        assert_eq!(c, [0x24, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0x1a, 1, 0]);
    }

    #[test]
    fn ce_qui_arrive() {
        // Événement « bouton » sur le port CPU RX.
        let info = InfoRecue((PORT_CPU_RX << 27) | (EVT_BOUTON << 20) | 8);
        assert_eq!((info.port(), info.evenement()), (PORT_CPU_RX, EVT_BOUTON));
        let info = InfoRecue((PORT_WLAN << 27) | (1 << 19));
        assert!(info.est_80211());
        // Une demande d'association : RxWi (WCID 5), puis l'en-tête.
        let mut p = vec![0u8; 32];
        p[4] = 5;
        p.extend_from_slice(&entete_wlan(controle_trame(WLAN_GESTION, SOUS_ASSOCIATION_DEMANDE, false), 0, &MAC, &MANETTE, &MAC));
        p.extend_from_slice(&[9, 9]);
        let t = lire_trame(&p).unwrap();
        assert_eq!((t.wcid, t.genre, t.sous_genre), (5, WLAN_GESTION, SOUS_ASSOCIATION_DEMANDE));
        assert_eq!((t.destination, t.source, t.charge), (MAC, MANETTE, &[9u8, 9][..]));
        assert!(lire_trame(&p[..56]).is_none());
    }

    #[test]
    fn le_firmware_se_decoupe() {
        let mut f = vec![0u8; 32];
        f[0..4].copy_from_slice(&3u32.to_le_bytes());
        f[4..8].copy_from_slice(&2u32.to_le_bytes());
        f.extend_from_slice(&[1, 2, 3, 4, 5]);
        let (ilm, dlm) = parties_firmware(&f).unwrap();
        assert_eq!((ilm, dlm), (&[1u8, 2, 3][..], &[4u8, 5][..]));
        f.pop();
        assert!(parties_firmware(&f).is_none());
        let mut faux = vec![0xffu8; 32];
        faux[8] = 0;
        assert!(parties_firmware(&faux).is_none());
    }

    #[test]
    fn un_morceau_de_firmware() {
        let m = morceau_firmware(&[7; 6]);
        assert_eq!(lire_mot(&m, 0), 6 | (2 << 27));
        assert_eq!(m.len(), 4 + 6 + 4);
    }
}
