//! La batterie, lue dans /sys/class/power_supply (todo F3 — 18 novembre).
//!
//! Aucun programme lancé : le noyau publie la charge et l'état dans des
//! fichiers, et la barre les relit toutes les trente secondes. Une machine
//! sans batterie (un fixe, une machine virtuelle) rend `None`, et la barre
//! ne montre rien — pas une icône « batterie inconnue » qui n'apprend rien.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etat {
    EnCharge,
    SurBatterie,
    Pleine,
    /// Branchée mais elle ne charge pas (seuil de charge, batterie chaude).
    Arretee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Batterie {
    pub pour_cent: u32,
    pub etat: Etat,
}

fn lire(chemin: &Path) -> String {
    fs::read_to_string(chemin).map(|t| t.trim().to_string()).unwrap_or_default()
}

/// La première batterie présente sous `racine` (/sys/class/power_supply).
/// Les batteries de souris et de manettes (`scope` = Device) ne comptent pas :
/// ce n'est pas l'autonomie de la machine.
pub fn lire_batterie(racine: &Path) -> Option<Batterie> {
    let mut noms: Vec<_> = fs::read_dir(racine).ok()?.flatten().map(|e| e.path()).collect();
    noms.sort();
    for dossier in noms {
        if lire(&dossier.join("type")) != "Battery" || lire(&dossier.join("scope")) == "Device" {
            continue;
        }
        if lire(&dossier.join("present")) == "0" {
            continue;
        }
        let pour_cent = lire(&dossier.join("capacity")).parse::<u32>().ok().map(|p| p.min(100));
        let Some(pour_cent) = pour_cent else { continue };
        let etat = match lire(&dossier.join("status")).as_str() {
            "Charging" => Etat::EnCharge,
            "Full" => Etat::Pleine,
            "Not charging" => Etat::Arretee,
            _ => Etat::SurBatterie,
        };
        return Some(Batterie { pour_cent, etat });
    }
    None
}

/// Le nom d'icône du thème (adwaita-icon-theme-legacy) et la phrase de
/// l'infobulle.
pub fn decrire(b: Batterie) -> (&'static str, String) {
    let niveau = match b.pour_cent {
        0..=9 => "empty",
        10..=24 => "caution",
        25..=49 => "low",
        50..=89 => "good",
        _ => "full",
    };
    let icone = match (niveau, b.etat == Etat::EnCharge || b.etat == Etat::Pleine) {
        ("empty", true) => "battery-empty-charging",
        ("caution", true) => "battery-caution-charging",
        ("low", true) => "battery-low-charging",
        ("good", true) => "battery-good-charging",
        ("full", true) => "battery-full-charging",
        ("empty", _) => "battery-empty",
        ("caution", _) => "battery-caution",
        ("low", _) => "battery-low",
        ("good", _) => "battery-good",
        _ => "battery-full",
    };
    let phrase = match b.etat {
        Etat::EnCharge => format!("Batterie : {} %, en charge", b.pour_cent),
        Etat::Pleine => "Batterie pleine, sur secteur".to_string(),
        Etat::Arretee => format!("Batterie : {} %, branchée mais ne charge pas", b.pour_cent),
        Etat::SurBatterie => format!("Batterie : {} %, sur batterie", b.pour_cent),
    };
    (icone, phrase)
}

/// Faut-il prévenir ? Une fois en passant sous 10 % sur batterie, et pas à
/// chaque relecture : `deja` est remis à faux dès qu'on rebranche.
pub fn prevenir(b: Batterie, deja: bool) -> bool {
    b.etat == Etat::SurBatterie && b.pour_cent < 10 && !deja
}

#[cfg(test)]
mod essais {
    use super::*;

    fn poser(racine: &Path, nom: &str, champs: &[(&str, &str)]) {
        let d = racine.join(nom);
        fs::create_dir_all(&d).unwrap();
        for (f, v) in champs {
            fs::write(d.join(f), format!("{v}\n")).unwrap();
        }
    }

    #[test]
    fn sans_batterie_rien() {
        let r = std::env::temp_dir().join(format!("grenos-bat-a-{}", std::process::id()));
        let _ = fs::remove_dir_all(&r);
        poser(&r, "AC", &[("type", "Mains"), ("online", "1")]);
        assert_eq!(lire_batterie(&r), None);
        assert_eq!(lire_batterie(&r.join("absent")), None);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn la_batterie_de_la_machine_pas_celle_de_la_souris() {
        let r = std::env::temp_dir().join(format!("grenos-bat-b-{}", std::process::id()));
        let _ = fs::remove_dir_all(&r);
        poser(&r, "hid-souris", &[("type", "Battery"), ("scope", "Device"), ("capacity", "40"), ("status", "Discharging")]);
        poser(&r, "BAT0", &[("type", "Battery"), ("present", "1"), ("capacity", "83"), ("status", "Charging")]);
        assert_eq!(lire_batterie(&r), Some(Batterie { pour_cent: 83, etat: Etat::EnCharge }));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn icones_et_phrases() {
        assert_eq!(decrire(Batterie { pour_cent: 83, etat: Etat::EnCharge }).0, "battery-good-charging");
        assert_eq!(decrire(Batterie { pour_cent: 7, etat: Etat::SurBatterie }).0, "battery-empty");
        assert_eq!(decrire(Batterie { pour_cent: 100, etat: Etat::Pleine }).1, "Batterie pleine, sur secteur");
        assert!(decrire(Batterie { pour_cent: 55, etat: Etat::SurBatterie }).1.contains("55 %"));
    }

    #[test]
    fn prevenir_une_seule_fois() {
        let basse = Batterie { pour_cent: 8, etat: Etat::SurBatterie };
        assert!(prevenir(basse, false));
        assert!(!prevenir(basse, true));
        assert!(!prevenir(Batterie { pour_cent: 8, etat: Etat::EnCharge }, false));
        assert!(!prevenir(Batterie { pour_cent: 30, etat: Etat::SurBatterie }, false));
    }
}
