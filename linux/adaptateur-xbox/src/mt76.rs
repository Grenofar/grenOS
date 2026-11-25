// grenos-adaptateur-xbox — le pilote de l'adaptateur sans fil Xbox.
// Copyright (C) 2019 Medusalix (xow, dongle/mt76.cpp)
// Copyright (C) 2026 grenOS — GPL-2.0-or-later, voir LICENSE.
//
// Les numéros de registres viennent du pilote mt76 d'OpenWrt, sous licence
// ISC : Copyright (C) Felix Fietkau, Lorenzo Bianconi, Stanislaw Gruszka.

//! La puce MT76 de l'adaptateur : on lui envoie le firmware de Microsoft,
//! on règle sa radio, puis elle se comporte en point d'accès que seules les
//! manettes Xbox savent joindre. Ordre et valeurs : ceux de xow, eux-mêmes
//! relevés sur le pilote Windows.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::trames::*;
use crate::usb::Usb;
use crate::{details, erreur, info};

// ---- Points USB --------------------------------------------------------------
pub const POINT_LECTURE: u8 = 5;
pub const POINT_LECTURE_PAQUETS: u8 = 4;
const POINT_ECRITURE: u8 = 4;

pub const NOMBRE_WCID: usize = 16;

// ---- Requêtes « fabricant » ---------------------------------------------------
const VEND_DEV_MODE: u8 = 0x01;
const VEND_MULTI_WRITE: u8 = 0x06;
const VEND_MULTI_READ: u8 = 0x07;
const VEND_WRITE_CFG: u8 = 0x46;
const VEND_READ_CFG: u8 = 0x47;

// ---- Registres -------------------------------------------------------------------
const ASIC_VERSION: u16 = 0x0000;
const CMB_CTRL: u16 = 0x0020;
const EFUSE_CTRL: u16 = 0x0024;
const EFUSE_DATA_BASE: u16 = 0x0028;
const LDO_CTRL_1: u16 = 0x0070;
const XO_CTRL5: u16 = 0x0114;
const XO_CTRL6: u16 = 0x0118;
const RF_PATCH: u16 = 0x0130;
const WPDMA_GLO_CFG: u16 = 0x0208;
const WMM_AIFSN: u16 = 0x0214;
const WMM_CWMIN: u16 = 0x0218;
const WMM_CWMAX: u16 = 0x021c;
const FCE_DMA_ADDR: u16 = 0x0230;
const FCE_DMA_LEN: u16 = 0x0234;
const USB_DMA_CFG: u16 = 0x0238;
const TSO_CTRL: u16 = 0x0250;
const PBF_SYS_CTRL: u16 = 0x0400;
const PBF_CFG: u16 = 0x0404;
const PBF_TX_MAX_PCNT: u16 = 0x0408;
const RF_BYPASS_0: u16 = 0x0504;
const RF_SETTING_0: u16 = 0x050c;
const FCE_PSE_CTRL: u16 = 0x0800;
const FCE_L2_STUFF: u16 = 0x080c;
const TX_CPU_FROM_FCE_BASE_PTR: u16 = 0x09a0;
const TX_CPU_FROM_FCE_MAX_COUNT: u16 = 0x09a4;
const TX_CPU_FROM_FCE_CPU_DESC_IDX: u16 = 0x09a8;
const FCE_PDMA_GLOBAL_CONF: u16 = 0x09c4;
const PAUSE_ENABLE_CONTROL1: u16 = 0x0a38;
const FCE_SKIP_FS: u16 = 0x0a6c;
const MAC_CSR0: u16 = 0x1000;
const MAC_SYS_CTRL: u16 = 0x1004;
const MAC_ADDR_DW0: u32 = 0x1008;
const MAC_BSSID_DW0: u32 = 0x1010;
const MAX_LEN_CFG: u16 = 0x1018;
const AMPDU_MAX_LEN_20M1S: u16 = 0x1030;
const AMPDU_MAX_LEN_20M2S: u16 = 0x1034;
const XIFS_TIME_CFG: u16 = 0x1100;
const BKOFF_SLOT_CFG: u16 = 0x1104;
const CH_TIME_CFG: u16 = 0x110c;
const BEACON_TIME_CFG: u16 = 0x1114;
const PWR_PIN_CFG: u16 = 0x1204;
const RF_PA_MODE_ADJ0: u16 = 0x1228;
const RF_PA_MODE_ADJ1: u16 = 0x122c;
const DACCLK_EN_DLY_CFG: u16 = 0x1264;
const EDCA_CFG_BASE: u16 = 0x1300;
const TX_PIN_CFG: u16 = 0x1328;
const TX_SW_CFG0: u16 = 0x1330;
const TX_SW_CFG1: u16 = 0x1334;
const TXOP_CTRL_CFG: u16 = 0x1340;
const TX_RTS_CFG: u16 = 0x1344;
const TX_TIMEOUT_CFG: u16 = 0x1348;
const TX_RETRY_CFG: u16 = 0x134c;
const CCK_PROT_CFG: u16 = 0x1364;
const OFDM_PROT_CFG: u16 = 0x1368;
const MM20_PROT_CFG: u16 = 0x136c;
const GF20_PROT_CFG: u16 = 0x1374;
const GF40_PROT_CFG: u16 = 0x1378;
const EXP_ACK_TIME: u16 = 0x1380;
const TX0_RF_GAIN_CORR: u16 = 0x13a0;
const TX1_RF_GAIN_CORR: u16 = 0x13a4;
const TX_ALC_CFG_2: u16 = 0x13a8;
const TX_ALC_CFG_3: u16 = 0x13ac;
const TX_ALC_CFG_0: u16 = 0x13b0;
const TX_ALC_CFG_4: u16 = 0x13c0;
const TX_PROT_CFG6: u16 = 0x13e0;
const TX_PROT_CFG7: u16 = 0x13e4;
const TX_PROT_CFG8: u16 = 0x13e8;
const PIFS_TX_CFG: u16 = 0x13ec;
const RX_FILTR_CFG: u16 = 0x1400;
const AUTO_RSP_CFG: u16 = 0x1404;
const LEGACY_BASIC_RATE: u16 = 0x1408;
const HT_BASIC_RATE: u16 = 0x140c;
const EXT_CCA_CFG: u16 = 0x141c;
const PN_PAD_MODE: u16 = 0x150c;
const TXOP_HLDR_ET: u16 = 0x1608;
const WCID_ADDR_BASE: u32 = 0x1800;
const BBP_AGC_BASE: u16 = 0x2300;
const USB_U3DMA_CFG: u16 = 0x9018;
const BEACON_BASE: u32 = 0xc000;

const MAC_SYS_CTRL_RESET_CSR: u32 = 1 << 0;
const MAC_SYS_CTRL_RESET_BBP: u32 = 1 << 1;
const MAC_SYS_CTRL_ENABLE_TX: u32 = 1 << 2;
const MAC_SYS_CTRL_ENABLE_RX: u32 = 1 << 3;
const EFUSE_CTRL_KICK: u32 = 1 << 30;
/// GENMASK(14, 8)
const XO_CTRL_C2: u32 = 0x7f00;

// ---- L'EEPROM (efuse) ---------------------------------------------------------------
const EE_CHIP_ID: u8 = 0x000;
const EE_MAC_ADDR: u8 = 0x004;
const EE_XTAL_TRIM_1: u8 = 0x03a;
const EE_XTAL_TRIM_2: u8 = 0x09e;
const EE_TX_POWER_0_START_2G: u8 = 0x056;
const EE_TX_POWER_0_START_5G: u8 = 0x062;
const EE_TX_POWER_GROUP_SIZE_5G: u8 = 5;

// ---- Ce qui appartient à xow ---------------------------------------------------------
const DELAI_ATTENTE: Duration = Duration::from_secs(1);
const FW_RESET_IVB: u16 = 0x01;
const FW_LOAD_IVB: u16 = 0x12;
const MCU_ILM_OFFSET: u32 = 0x80000;
const MCU_DLM_OFFSET: u32 = 0x100000 + 0x10800;
const FW_CHUNK_SIZE: usize = 0x3800;
const DMA_COMPLETE: u32 = 0xc000_0000;
const REGISTER_OFFSET: u32 = 0x410000;
const CH_POWER_MIN: i32 = 0x00;
const CH_POWER_MAX: i32 = 0x2f;

// ---- Les commandes ---------------------------------------------------------------------
const FW_MAC_ADDRESS_SET: u32 = 0;
const FW_CLIENT_ADD: u32 = 1;
const FW_CLIENT_REMOVE: u32 = 2;
const FW_CHANNEL_CANDIDATES_SET: u32 = 7;

const Q_SELECT: u32 = 1;
const RADIO_OFF: u32 = 0x30;
const RADIO_ON: u32 = 0x31;
const RF_BBP_CR: u32 = 2;
const CAL_TEMP_SENSOR: u32 = 2;
const CAL_RXDCOC: u32 = 3;
const CAL_RC: u32 = 4;

pub const LED_CLIGNOTE: u32 = 0;
pub const LED_ALLUMEE: u32 = 1;
pub const LED_ETEINTE: u32 = 2;

const BW_20: u8 = 0;
const BW_40: u8 = 1;
const BW_80: u8 = 2;

pub struct Mt76 {
    pub usb: Usb,
    pub mac: [u8; 6],
    /// Un bit par manette connectée (WCID 1 à 16).
    clients: Mutex<u16>,
}

impl Mt76 {
    /// Met la puce en route : firmware, radio, registres, adresse, canaux,
    /// balise. Rend la raison si une étape échoue.
    pub fn demarrer(usb: Usb, firmware: &[u8]) -> Result<Mt76, String> {
        let mut m = Mt76 { usb, mac: [0; 6], clients: Mutex::new(0) };
        m.charger_firmware(firmware)?;
        // File de réception 1, radio allumée, registres de commande du BBP.
        if !m.choisir_fonction(Q_SELECT, 1) || !m.alimentation(RADIO_ON) || !m.charger_cr(RF_BBP_CR) {
            return Err("la radio ne s'allume pas".into());
        }
        m.initialiser_registres()?;
        let mac = m.mac;
        if !m.commande_firmware(FW_MAC_ADDRESS_SET, &mac) {
            return Err("l'adresse sans fil n'a pas été prise".into());
        }
        // Nécessaire pour que les associations tiennent.
        m.ecrire(MAC_SYS_CTRL, 0);
        m.ecrire(RF_BYPASS_0, 0);
        m.ecrire(RF_SETTING_0, 0);
        if !m.calibrer(CAL_TEMP_SENSOR, 0) || !m.calibrer(CAL_RXDCOC, 1) || !m.calibrer(CAL_RC, 0) {
            return Err("l'étalonnage de la puce a échoué".into());
        }
        m.ecrire(MAC_SYS_CTRL, MAC_SYS_CTRL_ENABLE_TX | MAC_SYS_CTRL_ENABLE_RX);
        if !m.initialiser_canaux() {
            return Err("les canaux n'ont pas été réglés".into());
        }
        if !m.ecrire_balise(false) {
            return Err("la balise n'a pas été écrite".into());
        }
        if m.usb.perdu() {
            return Err("l'adaptateur ne répond plus".into());
        }
        Ok(m)
    }

    /// Éteint la LED et la radio (à la fin).
    pub fn eteindre(&self) {
        if !self.led(LED_ETEINTE) {
            erreur!("la LED ne s'éteint pas");
        }
        if !self.alimentation(RADIO_OFF) {
            erreur!("la radio ne s'éteint pas");
        }
    }

    // ---- Les manettes (clients sans fil) -----------------------------------------
    /// Accepte la manette `adresse` : un WCID libre, son adresse dans la
    /// table de la puce, et la réponse d'association. Rend le WCID, ou 0.
    pub fn associer(&self, adresse: &[u8; 6]) -> u8 {
        let mut clients = self.clients.lock().unwrap_or_else(|e| e.into_inner());
        let libres = !*clients;
        if libres == 0 {
            erreur!("les seize places de manette sont prises");
            return 0;
        }
        let wcid = libres.trailing_zeros() as u8 + 1;
        // WCID 0 est réservé aux balises : la table commence à 1.
        let donnees_wcid = [wcid - 1, 0, 0, 0, 0x40, 0x1f, 0, 0];
        let ok = self.ecrire_en_rafale(WCID_ADDR_BASE + wcid as u32 * 8, adresse)
            && self.commande_firmware(FW_CLIENT_ADD, &donnees_wcid)
            && self.usb.ecrire(POINT_ECRITURE, &paquet_wlan(&trame_association(&self.mac, adresse)))
            && self.led(LED_ALLUMEE);
        if !ok {
            erreur!("la manette {} n'a pas pu être associée", crate::journal::adresse(adresse));
            return 0;
        }
        *clients |= 1 << (wcid - 1);
        wcid
    }

    /// Libère le WCID d'une manette partie.
    pub fn retirer(&self, wcid: u8) -> bool {
        let mut clients = self.clients.lock().unwrap_or_else(|e| e.into_inner());
        *clients &= !(1u16 << (wcid - 1));
        if !self.commande_firmware(FW_CLIENT_REMOVE, &[wcid - 1, 0, 0, 0]) {
            erreur!("la manette {wcid} n'a pas été retirée");
            return false;
        }
        if !self.ecrire_en_rafale(WCID_ADDR_BASE + wcid as u32 * 8, &[0; 6]) {
            erreur!("la place {wcid} n'a pas été effacée");
            return false;
        }
        // Plus aucune manette : la LED s'éteint.
        *clients != 0 || self.led(LED_ETEINTE)
    }

    /// Répond à une demande d'appairage.
    pub fn appairer(&self, adresse: &[u8; 6]) -> bool {
        self.usb.ecrire(POINT_ECRITURE, &paquet_wlan(&trame_appairage(&self.mac, adresse)))
    }

    /// Envoie un paquet GIP à la manette `wcid` (si elle est encore là).
    pub fn envoyer(&self, wcid: u8, adresse: &[u8; 6], paquet: &[u8]) -> bool {
        if wcid == 0 || wcid as usize > NOMBRE_WCID {
            return false;
        }
        let connecte = *self.clients.lock().unwrap_or_else(|e| e.into_inner()) & (1 << (wcid - 1)) != 0;
        if !connecte {
            return true;
        }
        self.commande(CMD_PACKET_TX, &trame_client(wcid, &self.mac, adresse, paquet))
    }

    /// Le mode appairage : la balise l'annonce, la LED clignote.
    pub fn mode_appairage(&self, actif: bool) -> bool {
        if !self.ecrire_balise(actif) {
            erreur!("la balise n'a pas été écrite");
            return false;
        }
        if !self.led(if actif { LED_CLIGNOTE } else { LED_ALLUMEE }) {
            return false;
        }
        info!("{}", if actif { "appairage ouvert : appuie sur le bouton d'appairage de la manette" } else { "appairage fermé" });
        true
    }

    // ---- La mise en route ------------------------------------------------------------
    fn charger_firmware(&self, firmware: &[u8]) -> Result<(), String> {
        let (ilm, dlm) = parties_firmware(firmware).ok_or("le firmware est abîmé (en-tête incohérent)")?;
        if self.lire_avec(FCE_DMA_ADDR, VEND_READ_CFG) != 0 {
            details!("firmware déjà chargé, remise à zéro");
            let patch = self.lire_avec(RF_PATCH, VEND_READ_CFG) & !(1 << 19);
            // Obligatoire pour une radio déjà initialisée.
            self.ecrire_avec(RF_PATCH, patch, VEND_WRITE_CFG);
            self.ecrire_avec(FW_RESET_IVB, 0, VEND_DEV_MODE);
            if !self.attendre(|| self.lire_avec(FCE_DMA_ADDR, VEND_READ_CFG) != 0x8000_0000) {
                // Vu le 25 novembre sur la clé de Grenofar : le pilote Wi-Fi
                // du noyau (mt76x2u) l'avait prise au branchement et y avait
                // mis SON firmware. La puce ne se remet pas à zéro depuis
                // cet état ; seule une coupure de courant la libère.
                return Err("la puce tourne avec le firmware d'un autre pilote (le Wi-Fi du noyau l'a prise au branchement) : \
                            débranche l'adaptateur et rebranche-le"
                    .into());
            }
        }
        // DMA : réception et émission en bloc (bits 22 et 23).
        self.ecrire_avec(USB_U3DMA_CFG, (1 << 22) | (1 << 23), VEND_WRITE_CFG);
        self.ecrire(FCE_PSE_CTRL, 0x01);
        self.ecrire(TX_CPU_FROM_FCE_BASE_PTR, 0x400230);
        self.ecrire(TX_CPU_FROM_FCE_MAX_COUNT, 0x01);
        self.ecrire(TX_CPU_FROM_FCE_CPU_DESC_IDX, 0x01);
        self.ecrire(FCE_PDMA_GLOBAL_CONF, 0x44);
        self.ecrire(FCE_SKIP_FS, 0x03);
        // Mémoire d'instructions (ILM), puis de données (DLM).
        self.charger_partie(MCU_ILM_OFFSET, ilm).map_err(|e| format!("ILM : {e}"))?;
        self.charger_partie(MCU_DLM_OFFSET, dlm).map_err(|e| format!("DLM : {e}"))?;
        // Le bloc de vecteurs initial : le firmware démarre.
        self.ecrire_avec(FCE_DMA_ADDR, 0, VEND_WRITE_CFG);
        self.ecrire_avec(FW_LOAD_IVB, 0, VEND_DEV_MODE);
        if !self.attendre(|| self.lire_avec(FCE_DMA_ADDR, VEND_READ_CFG) != 0x01) {
            return Err("le firmware ne démarre pas".into());
        }
        details!("firmware chargé ({} + {} octets)", ilm.len(), dlm.len());
        Ok(())
    }

    fn charger_partie(&self, decalage: u32, partie: &[u8]) -> Result<(), String> {
        for (i, morceau) in partie.chunks(FW_CHUNK_SIZE).enumerate() {
            let adresse = decalage + (i * FW_CHUNK_SIZE) as u32;
            let longueur = morceau.len() as u32;
            self.ecrire_avec(FCE_DMA_ADDR, adresse, VEND_WRITE_CFG);
            self.ecrire_avec(FCE_DMA_LEN, longueur << 16, VEND_WRITE_CFG);
            if !self.usb.ecrire(POINT_ECRITURE, &morceau_firmware(morceau)) {
                return Err("un morceau n'est pas passé".into());
            }
            let fini = (longueur << 16) | DMA_COMPLETE;
            if !self.attendre(|| self.lire_avec(FCE_DMA_LEN, VEND_READ_CFG) != fini) {
                return Err("un morceau n'a pas été accepté".into());
            }
        }
        Ok(())
    }

    fn initialiser_registres(&mut self) -> Result<(), String> {
        const VALEURS: &[(u16, u32)] = &[
            (MAC_SYS_CTRL, MAC_SYS_CTRL_RESET_CSR | MAC_SYS_CTRL_RESET_BBP),
            (USB_DMA_CFG, 0),
            (MAC_SYS_CTRL, 0),
            (PWR_PIN_CFG, 0),
            (LDO_CTRL_1, 0x6b006464),
            (WPDMA_GLO_CFG, 0x70),
            (WMM_AIFSN, 0x2273),
            (WMM_CWMIN, 0x2344),
            (WMM_CWMAX, 0x34aa),
            (FCE_DMA_ADDR, 0x041200),
            (TSO_CTRL, 0),
            (PBF_SYS_CTRL, 0x080c00),
            (PBF_TX_MAX_PCNT, 0x1fbf1f1f),
            (FCE_PSE_CTRL, 0x01),
            (MAC_SYS_CTRL, MAC_SYS_CTRL_ENABLE_TX | MAC_SYS_CTRL_ENABLE_RX),
            (AUTO_RSP_CFG, 0x13),
            (MAX_LEN_CFG, 0x3e3fff),
            (AMPDU_MAX_LEN_20M1S, 0xfffc9855),
            (AMPDU_MAX_LEN_20M2S, 0xff),
            (BKOFF_SLOT_CFG, 0x0109),
            (PWR_PIN_CFG, 0),
            (EDCA_CFG_BASE, 0x064320),
            (EDCA_CFG_BASE + 4, 0x0a4700),
            (EDCA_CFG_BASE + 8, 0x043238),
            (EDCA_CFG_BASE + 12, 0x03212f),
            (TX_PIN_CFG, 0x150f0f),
            (TX_SW_CFG0, 0x101001),
            (TX_SW_CFG1, 0x010000),
            (TXOP_CTRL_CFG, 0x10583f),
            (TX_TIMEOUT_CFG, 0x0a0f90),
            (TX_RETRY_CFG, 0x47d01f0f),
            (CCK_PROT_CFG, 0x03f40003),
            (OFDM_PROT_CFG, 0x03f40003),
            (MM20_PROT_CFG, 0x01742004),
            (GF20_PROT_CFG, 0x01742004),
            (GF40_PROT_CFG, 0x03f42084),
            (EXP_ACK_TIME, 0x2c00dc),
            (TX_ALC_CFG_2, 0x22160a00),
            (TX_ALC_CFG_3, 0x22160a76),
            (TX_ALC_CFG_0, 0x3f3f1818),
            (TX_ALC_CFG_4, 0x0606),
            (PIFS_TX_CFG, 0x060fff),
            (RX_FILTR_CFG, 0x017f17),
            (LEGACY_BASIC_RATE, 0x017f),
            (HT_BASIC_RATE, 0x8003),
            (PN_PAD_MODE, 0x02),
            (TXOP_HLDR_ET, 0x02),
            (TX_PROT_CFG6, 0xe3f42004),
            (TX_PROT_CFG7, 0xe3f42084),
            (TX_PROT_CFG8, 0xe3f42104),
            (DACCLK_EN_DLY_CFG, 0),
            (RF_PA_MODE_ADJ0, 0xee000000),
            (RF_PA_MODE_ADJ1, 0xee000000),
            (TX0_RF_GAIN_CORR, 0x0f3c3c3c),
            (TX1_RF_GAIN_CORR, 0x0f3c3c3c),
            (PBF_CFG, 0x1efebcf5),
            (PAUSE_ENABLE_CONTROL1, 0x0a),
            (RF_BYPASS_0, 0x7f000000),
            (RF_SETTING_0, 0x1a800000),
            (XIFS_TIME_CFG, 0x33a40e0a),
            (FCE_L2_STUFF, 0x03ff0223),
            (TX_RTS_CFG, 0),
            (BEACON_TIME_CFG, 0x0640),
            (EXT_CCA_CFG, 0xf0e4),
            (CH_TIME_CFG, 0x015f),
        ];
        for &(registre, valeur) in VALEURS {
            self.ecrire(registre, valeur);
        }
        // L'oscillateur à quartz.
        self.etalonner_quartz()?;
        // Le contrôle automatique de gain (AGC), registres 8 et 9.
        self.ecrire(BBP_AGC_BASE + 8 * 4, 0x18365efa);
        self.ecrire(BBP_AGC_BASE + 9 * 4, 0x18365efa);

        let lue = self.lire_efuse(EE_MAC_ADDR, 6);
        if lue.len() < 6 {
            return Err("l'adresse sans fil ne se lit pas".into());
        }
        self.mac.copy_from_slice(&lue);
        // Certains adaptateurs ont une adresse en 6c:5d:3a ; les manettes ne
        // se connectent qu'à 62:45:bx:xx:xx:xx.
        if self.mac[0] != 0x62 {
            details!("adresse {} corrigée", crate::journal::adresse(&self.mac));
            self.mac[0] = 0x62;
            self.mac[1] = 0x45;
            self.mac[2] = 0xbd;
        }
        let mac = self.mac;
        if !self.ecrire_en_rafale(MAC_ADDR_DW0, &mac) || !self.ecrire_en_rafale(MAC_BSSID_DW0, &mac) {
            return Err("l'adresse sans fil ne s'écrit pas".into());
        }
        let asic = self.lire(ASIC_VERSION) >> 16;
        let version_mac = self.lire(MAC_CSR0) >> 16;
        let puce = self.lire_efuse(EE_CHIP_ID, 4);
        if puce.len() < 4 {
            return Err("l'identifiant de la puce ne se lit pas".into());
        }
        details!("ASIC {asic:x}, MAC {version_mac:x}, puce {:x}", ((puce[1] as u16) << 8) | puce[2] as u16);
        info!("adresse sans fil : {}", crate::journal::adresse(&self.mac));
        Ok(())
    }

    fn etalonner_quartz(&self) -> Result<(), String> {
        let reglage = self.lire_efuse(EE_XTAL_TRIM_2, 4);
        if reglage.len() < 4 {
            return Err("le deuxième réglage du quartz ne se lit pas".into());
        }
        let mut valeur: u16 = ((reglage[3] as u16) << 8) | reglage[2] as u16;
        let mut decalage: i8 = (valeur & 0x7f) as i8;
        if valeur & 0xff == 0xff {
            decalage = 0;
        } else if valeur & 0x80 != 0 {
            decalage = -decalage;
        }
        valeur >>= 8;
        if valeur == 0x00 || valeur == 0xff {
            let reglage = self.lire_efuse(EE_XTAL_TRIM_1, 4);
            if reglage.len() < 4 {
                return Err("le premier réglage du quartz ne se lit pas".into());
            }
            valeur = (((reglage[3] as u16) << 8) | reglage[2] as u16) & 0xff;
            if valeur == 0x00 || valeur == 0xff {
                valeur = 0x14;
            }
        }
        // Même arithmétique que xow (un u16 qui peut repasser sous zéro).
        let valeur = ((valeur & 0x7f) as i32 + decalage as i32) as u16;
        let ctrl = self.lire(XO_CTRL5) & !XO_CTRL_C2;
        self.ecrire_avec(XO_CTRL5, ctrl | ((valeur as u32) << 8), VEND_WRITE_CFG);
        self.ecrire_avec(XO_CTRL6, XO_CTRL_C2, VEND_WRITE_CFG);
        self.ecrire(CMB_CTRL, 0x0091a7ff);
        Ok(())
    }

    fn initialiser_canaux(&self) -> bool {
        // La puissance des canaux 0x24 à 0x30 : le pilote d'origine la monte,
        // parfois au-delà du maximum ; on garde celle de l'EEPROM.
        const CANAUX: &[(u8, u8, bool)] = &[
            (0x01, BW_20, true),
            (0x06, BW_20, true),
            (0x0b, BW_20, true),
            (0x24, BW_40, true),
            (0x28, BW_40, false),
            (0x2c, BW_40, true),
            (0x30, BW_40, false),
            (0x95, BW_80, true),
            (0x99, BW_80, false),
            (0x9d, BW_80, true),
            (0xa1, BW_80, false),
            (0xa5, BW_80, false),
        ];
        for &(canal, largeur, balayage) in CANAUX {
            // Comme xow : un canal refusé n'arrête pas tout.
            self.regler_canal(canal, largeur, balayage);
        }
        // Les canaux candidats, un par mot de 32 bits.
        const CANDIDATS: [u8; 14] = [0x01, 0xa5, 0x0b, 0x01, 0x06, 0x0b, 0x24, 0x28, 0x2c, 0x30, 0x95, 0x99, 0x9d, 0xa1];
        let valeurs: Vec<u8> = CANDIDATS.iter().flat_map(|&c| (c as u32).to_le_bytes()).collect();
        if !self.commande_firmware(FW_CHANNEL_CANDIDATES_SET, &valeurs) {
            erreur!("les canaux candidats n'ont pas été pris");
            return false;
        }
        true
    }

    fn regler_canal(&self, canal: u8, largeur: u8, balayage: bool) -> bool {
        let puissance = self.puissance_canal(canal);
        details!("canal {canal}, puissance {puissance}");
        self.commande(CMD_SWITCH_CHANNEL_OP, &reglage_canal(canal, largeur, puissance, balayage))
    }

    fn puissance_canal(&self, canal: u8) -> u8 {
        let deux_ghz = canal <= 14;
        let mut index = if deux_ghz { EE_TX_POWER_0_START_2G } else { EE_TX_POWER_0_START_5G };
        if !deux_ghz {
            index = index.wrapping_add(groupe_canal(canal).wrapping_mul(EE_TX_POWER_GROUP_SIZE_5G));
        }
        let entree = self.lire_efuse(index, 8);
        if entree.len() < 8 {
            erreur!("la table de puissance ne se lit pas");
            return CH_POWER_MIN as u8;
        }
        puissance_depuis_table(&entree, deux_ghz, sous_groupe_canal(canal))
    }

    fn ecrire_balise(&self, appairage: bool) -> bool {
        // Minuteries TSF et TBTT, mode point d'accès, émission des balises.
        let config = self.lire(BEACON_TIME_CFG) | (1 << 16) | (3 << 17) | (1 << 19) | (1 << 20);
        if !self.ecrire_en_rafale(BEACON_BASE, &trame_balise(&self.mac, appairage)) {
            return false;
        }
        self.ecrire(BEACON_TIME_CFG, config);
        self.calibrer(CAL_RXDCOC, 0)
    }

    // ---- Les commandes du microcontrôleur -----------------------------------------
    fn commande(&self, commande: u8, donnees: &[u8]) -> bool {
        let ok = self.usb.ecrire(POINT_ECRITURE, &commande_mcu(commande, donnees));
        if !ok {
            erreur!("la commande {commande} n'est pas passée");
        }
        ok
    }

    fn deux_mots(a: u32, b: u32) -> [u8; 8] {
        let mut o = [0u8; 8];
        o[..4].copy_from_slice(&a.to_le_bytes());
        o[4..].copy_from_slice(&b.to_le_bytes());
        o
    }

    fn choisir_fonction(&self, fonction: u32, valeur: u32) -> bool {
        self.commande(CMD_FUN_SET_OP, &Self::deux_mots(fonction, valeur))
    }

    fn alimentation(&self, mode: u32) -> bool {
        self.commande(CMD_POWER_SAVING_OP, &mode.to_le_bytes())
    }

    fn charger_cr(&self, mode: u32) -> bool {
        self.commande(CMD_LOAD_CR, &mode.to_le_bytes())
    }

    fn calibrer(&self, etalonnage: u32, valeur: u32) -> bool {
        self.commande(CMD_CALIBRATION_OP, &Self::deux_mots(etalonnage, valeur))
    }

    fn led(&self, mode: u32) -> bool {
        self.commande(CMD_LED_MODE_OP, &mode.to_le_bytes())
    }

    fn commande_firmware(&self, commande: u32, donnees: &[u8]) -> bool {
        let mut sortie = commande.to_le_bytes().to_vec();
        sortie.extend_from_slice(donnees);
        self.commande(CMD_INTERNAL_FW_OP, &sortie)
    }

    fn ecrire_en_rafale(&self, registre: u32, valeurs: &[u8]) -> bool {
        let mut sortie = (registre + REGISTER_OFFSET).to_le_bytes().to_vec();
        sortie.extend_from_slice(valeurs);
        self.commande(CMD_BURST_WRITE, &sortie)
    }

    // ---- Registres et EEPROM -------------------------------------------------------------
    fn lire(&self, registre: u16) -> u32 {
        self.lire_avec(registre, VEND_MULTI_READ)
    }

    fn lire_avec(&self, registre: u16, requete: u8) -> u32 {
        let mut valeur = [0u8; 4];
        self.usb.controle(requete, 0, registre, &mut valeur, false);
        u32::from_le_bytes(valeur)
    }

    fn ecrire(&self, registre: u16, valeur: u32) {
        self.ecrire_avec(registre, valeur, VEND_MULTI_WRITE);
    }

    fn ecrire_avec(&self, registre: u16, valeur: u32, requete: u8) {
        if requete == VEND_DEV_MODE {
            // Le « mode » passe dans wValue, sans données.
            self.usb.controle(requete, registre, 0, &mut [], true);
        } else {
            let mut octets = valeur.to_le_bytes();
            self.usb.controle(requete, 0, registre, &mut octets, true);
        }
    }

    /// Lit `longueur` octets de l'EEPROM à partir de `adresse`, par blocs de
    /// 16 octets (4 registres de données). Vide si la lecture n'aboutit pas.
    fn lire_efuse(&self, adresse: u8, longueur: u8) -> Vec<u8> {
        // Mode lecture (bits 6-7 à 0), adresse du bloc (bits 16-25), départ
        // (bit 30).
        let mut controle = self.lire(EFUSE_CTRL);
        controle &= !(0x3 << 6);
        controle = (controle & !(0x3ff << 16)) | (((adresse & 0xf0) as u32) << 16);
        controle |= EFUSE_CTRL_KICK;
        self.ecrire(EFUSE_CTRL, controle);
        if !self.attendre(|| self.lire(EFUSE_CTRL) & EFUSE_CTRL_KICK != 0) {
            erreur!("l'EEPROM ne répond pas");
            return Vec::new();
        }
        let mut donnees = Vec::new();
        let mut i = 0u8;
        while i < longueur {
            let decalage = (adresse & 0x0c).wrapping_add(i);
            let valeur = self.lire(EFUSE_DATA_BASE + decalage as u16);
            let taille = (longueur - i).min(4) as usize;
            donnees.extend_from_slice(&valeur.to_le_bytes()[..taille]);
            i += 4;
        }
        donnees
    }

    /// Attend, une seconde au plus, que `encore` devienne faux.
    fn attendre(&self, encore: impl Fn() -> bool) -> bool {
        let debut = Instant::now();
        while encore() {
            if self.usb.perdu() || debut.elapsed() > DELAI_ATTENTE {
                return false;
            }
        }
        true
    }
}

/// Le groupe de canaux 5 GHz (l'entrée de la table de puissance).
fn groupe_canal(canal: u8) -> u8 {
    match canal {
        184..=196 => 0, // Japon
        0..=48 => 1,    // UNII-1
        49..=64 => 2,   // UNII-2
        65..=114 => 3,  // UNII-2e, première moitié
        115..=144 => 4, // UNII-2e, seconde moitié
        _ => 5,         // UNII-3
    }
}

/// Le sous-groupe (l'octet de décalage de puissance dans l'entrée).
fn sous_groupe_canal(canal: u8) -> u8 {
    const BAS_5G: u8 = 1;
    const HAUT_5G: u8 = 2;
    match canal {
        192.. => HAUT_5G,
        184..=191 => BAS_5G,
        0..=5 => 1,   // 2,4 GHz bas
        6..=10 => 2,  // 2,4 GHz milieu
        11..=14 => 3, // 2,4 GHz haut
        15..=43 => BAS_5G,
        44..=51 => HAUT_5G,
        52..=57 => BAS_5G,
        58..=97 => HAUT_5G,
        98..=105 => BAS_5G,
        106..=115 => HAUT_5G,
        116..=129 => BAS_5G,
        130..=148 => HAUT_5G,
        149..=156 => BAS_5G,
        _ => HAUT_5G,
    }
}

/// La puissance d'un canal d'après son entrée de table (8 octets) : la cible,
/// corrigée d'un décalage en demi-décibels s'il est activé (bit 7), vers le
/// haut ou le bas (bit 6), bornée à 0..0x2f.
fn puissance_depuis_table(entree: &[u8], deux_ghz: bool, sous_groupe: u8) -> u8 {
    let index = if deux_ghz { 4 } else { 5 };
    let cible = entree[index];
    let decalage = entree[(index + sous_groupe as usize).min(entree.len() - 1)];
    if decalage & 0x80 == 0 {
        return cible;
    }
    let pas = (decalage & 0x3f) as i32;
    // Comme xow : le calcul passe par un i8.
    let puissance = (if decalage & 0x40 != 0 { cible as i32 + pas } else { cible as i32 - pas }) as i8 as i32;
    puissance.clamp(CH_POWER_MIN, CH_POWER_MAX) as u8
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn groupes_et_sous_groupes() {
        assert_eq!((groupe_canal(0x24), groupe_canal(0x30), groupe_canal(0x95), groupe_canal(0xa5)), (1, 1, 5, 5));
        assert_eq!((sous_groupe_canal(1), sous_groupe_canal(6), sous_groupe_canal(11)), (1, 2, 3));
        assert_eq!((sous_groupe_canal(0x24), sous_groupe_canal(0x2c), sous_groupe_canal(0x95), sous_groupe_canal(0x9d)), (1, 2, 1, 2));
    }

    #[test]
    fn la_puissance_d_un_canal() {
        // Sans décalage : la cible.
        assert_eq!(puissance_depuis_table(&[0, 0, 0, 0, 0x20, 0x00, 0, 0], true, 1), 0x20);
        // +3 demi-dB.
        assert_eq!(puissance_depuis_table(&[0, 0, 0, 0, 0x20, 0xc3, 0, 0], true, 1), 0x23);
        // -3.
        assert_eq!(puissance_depuis_table(&[0, 0, 0, 0, 0x20, 0x83, 0, 0], true, 1), 0x1d);
        // Bornée au maximum, et au minimum.
        assert_eq!(puissance_depuis_table(&[0, 0, 0, 0, 0x2e, 0xff, 0, 0], true, 1), 0x2f);
        assert_eq!(puissance_depuis_table(&[0, 0, 0, 0, 0x02, 0xbf, 0, 0], true, 1), 0x00);
        // 5 GHz : cible à l'index 5.
        assert_eq!(puissance_depuis_table(&[0, 0, 0, 0, 0, 0x18, 0, 0x00], false, 2), 0x18);
    }

    #[test]
    fn les_decalages_du_firmware() {
        assert_eq!(MCU_DLM_OFFSET, 0x110800);
        assert_eq!(REGISTER_OFFSET + BEACON_BASE, 0x41c000);
    }
}
