//! Le fuzzing léger de grenos-ui (todo E11 — 18 novembre).
//!
//! Nos analyseurs lisent ce que d'autres programmes écrivent : nmcli,
//! bluetoothctl, pactl, dpkg-query, le journal du noyau, les fichiers de la
//! personne. Une seule entrée inattendue qui fait paniquer Rust (un index hors
//! limite, une découpe au milieu d'un caractère UTF-8) ferait tomber la
//! fenêtre entière. Ici, chacun reçoit des dizaines de milliers d'entrées
//! fabriquées au hasard — des morceaux qui RESSEMBLENT à leurs vraies entrées,
//! mélangés à des octets quelconques et à de l'UTF-8 sur plusieurs octets —,
//! et la seule chose exigée est qu'aucun ne panique.
//!
//! « Continu » parce que `cargo test` tourne à chaque construction (natif.yml,
//! linux.yml) : le tirage change avec la graine du jour, donc chaque
//! construction essaie d'autres entrées. La graine est imprimée en cas
//! d'échec pour qu'on rejoue exactement le même tirage (GRENOS_GRAINE=…).
//!
//! Sans dépendance : un générateur xorshift suffit, on ne fait pas de
//! cryptographie.

#![cfg(test)]

use std::path::Path;

struct Tirage(u64);

impl Tirage {
    fn suivant(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn jusqua(&mut self, n: usize) -> usize {
        (self.suivant() % n.max(1) as u64) as usize
    }

    /// Un texte qui ressemble aux vraies sorties : des morceaux connus, des
    /// séparateurs, de l'UTF-8 à plusieurs octets, et du bruit.
    fn texte(&mut self, morceaux: &[&str]) -> String {
        const BRUIT: &[&str] = &[":", "\\:", "\\", "\n", "\r\n", " ", "\t", "%", "=", "\"", "'", "é", "€", "😀", "\u{0}", "/", "..", "-", "#", "(", ")", "[", "]", "0", "100", "-5", "4294967295", "999999999999999999999"];
        let n = self.jusqua(24);
        let mut s = String::new();
        for _ in 0..n {
            let m = if self.jusqua(3) == 0 { BRUIT[self.jusqua(BRUIT.len())] } else { morceaux[self.jusqua(morceaux.len())] };
            s.push_str(m);
            if self.jusqua(5) == 0 {
                // Un caractère quelconque, valide en UTF-8.
                if let Some(c) = char::from_u32((self.suivant() % 0x2FFF) as u32) {
                    s.push(c);
                }
            }
        }
        s
    }
}

fn graine() -> u64 {
    std::env::var("GRENOS_GRAINE").ok().and_then(|g| g.parse().ok()).unwrap_or_else(|| {
        let jour = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(1);
        jour.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1
    })
}

/// Fait tourner `essai` sur `tours` entrées, et dit la graine si l'une panique.
fn eprouver(nom: &str, tours: usize, morceaux: &[&str], essai: impl Fn(&str) + std::panic::RefUnwindSafe) {
    let g = graine();
    let mut t = Tirage(g);
    for i in 0..tours {
        let entree = t.texte(morceaux);
        let r = std::panic::catch_unwind(|| essai(&entree));
        assert!(r.is_ok(), "{nom} a paniqué au tour {i} (GRENOS_GRAINE={g}) sur {entree:?}");
    }
}

const TOURS: usize = 20_000;

#[test]
fn nmcli_et_bluetoothctl() {
    let m = ["*", "Livebox", "WPA2", "--", "wifi", "ethernet", "connected", "disconnected", "Device", "AA:BB:CC:DD:EE:FF", "Connected", "Secrets were required", "802-11-wireless-security"];
    eprouver("decouper", TOURS, &m, |t| { let _ = crate::premier::decouper(t); });
    eprouver("reseaux_de", TOURS, &m, |t| { let _ = crate::connexions::reseaux_de(t); });
    eprouver("lien_de", TOURS, &m, |t| { let _ = crate::connexions::lien_de(t); });
    eprouver("appareils_de", TOURS, &m, |t| { let _ = crate::connexions::appareils_de(t, t); });
    eprouver("refus", TOURS, &m, |t| { let _ = crate::connexions::refus(t); });
    eprouver("adresse_valide", TOURS, &m, |t| { let _ = crate::connexions::adresse_valide(t); });
}

#[test]
fn journal_du_noyau() {
    let m = ["Direct firmware load for ", "probe with driver ", " failed", "iwlwifi", "nouveau", "0000:01:00.0:", " 0000:", "-cc-a0", "ahci", "with error -110"];
    eprouver("module_fautif", TOURS, &m, |t| { let _ = crate::secours::module_fautif(t); });
    eprouver("echecs_de", TOURS, &m, |t| { let _ = crate::secours::echecs_de(t); });
    eprouver("veille_de", TOURS, &["freeze", "mem", "disk", "standby"], |t| { let _ = crate::arret::veille_de(t); });
}

#[test]
fn paquets_et_installation() {
    let m = ["gimp", "install ok installed", "deinstall ok config-files", "libc6:amd64", "com.valvesoftware.Steam", "Unpacking", "Setting up", "42%", "100%"];
    eprouver("paquets_de", TOURS, &m, |t| { let _ = crate::grenplace::paquets_de(t); });
    eprouver("identifiant_sain", TOURS, &m, |t| { let _ = crate::installer::identifiant_sain(t); });
    eprouver("pour_cent", TOURS, &m, |t| { let _ = crate::installer::pour_cent(t); });
    eprouver("etape", TOURS, &m, |t| { let _ = crate::installer::etape(t); });
    let l = ["AVANCE ", "ERREUR ", "FINI", "/dev/", "nvme0n1p3", " 42 ", "— 51 %"];
    eprouver("dit", TOURS, &l, |t| { let _ = crate::installation::dit(t); });
    eprouver("peripherique_valide", TOURS, &l, |t| { let _ = crate::installation::peripherique_valide(t); });
}

#[test]
fn fichiers_et_reglages() {
    let m = ["/home/lea", "..", ".", "~", "Documents", "photo.JPG", ".tar.gz", "Xft.dpi:", " 144", "Xcursor.size: 24", "café"];
    eprouver("normaliser", TOURS, &m, |t| { let _ = crate::fichiers::normaliser(Path::new(t)); });
    eprouver("nom_valable", TOURS, &m, |t| { let _ = crate::fichiers::nom_valable(t); });
    eprouver("famille", TOURS, &m, |t| { let _ = crate::fichiers::famille(t, t); });
    eprouver("mime_image", TOURS, &m, |t| { let _ = crate::fichiers::mime_image(t); });
    eprouver("est_une_archive", TOURS, &m, |t| { let _ = crate::fichiers::est_une_archive(t); });
    eprouver("texte_lisible", TOURS, &m, |t| {
        // Toutes les coupes possibles d'un même texte : c'est là que l'UTF-8
        // se casse en deux.
        let o = t.as_bytes();
        for fin in 0..o.len().min(64) {
            let _ = crate::fichiers::texte_lisible(&o[..fin]);
        }
    });
    eprouver("base64", TOURS / 4, &m, |t| { let _ = crate::fichiers::base64(t.as_bytes()); });
    eprouver("dpi", TOURS, &m, |t| {
        let _ = crate::reglages::dpi_de(t);
        let _ = crate::reglages::avec_dpi(t, 120);
    });
    eprouver("nom_lisible", TOURS, &m, |t| {
        let mots: Vec<&str> = t.split(' ').collect();
        let _ = crate::taches::nom_lisible(&mots);
    });
}
