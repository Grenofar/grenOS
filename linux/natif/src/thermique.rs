//! La chaleur, lue dans /sys/class/thermal (todo F4 — 18 novembre).
//!
//! grenHub MONTRE les sondes ; personne ne prévenait pendant qu'on travaille.
//! La barre relit les zones thermiques toutes les trente secondes (aucun
//! programme lancé) et prévient UNE fois quand l'une approche de son seuil
//! critique — celui au-delà duquel le noyau éteint la machine d'office. La
//! notification dit quoi faire, pas seulement le nombre.

use std::fs;
use std::path::Path;

/// Une zone : sa température et son seuil critique, en millidegrés.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zone {
    pub temperature: i64,
    pub critique: Option<i64>,
}

/// Faute de seuil déclaré (ou d'un seuil vraisemblable), l'arrêt est compté à
/// 105 °C — la limite que Grenofar a donnée le 20 novembre pour le
/// processeur et la carte graphique —, donc l'alerte à 100 °C.
pub const SANS_SEUIL: i64 = 100_000;
/// Un seuil critique hors de cette plage est faux. La sonde ACPI `acpitz`
/// d'une carte mère de PC fixe annonce souvent 16,8 °C figés et un seuil
/// critique de 20,8 °C : la barre prévenait « 16 °C, à 4 °C de l'arrêt de
/// sécurité » (PC de Grenofar, Gigabyte B760M, 20 novembre).
const SEUIL_VRAISEMBLABLE: std::ops::RangeInclusive<i64> = 60_000..=130_000;
/// On prévient à 5 °C du seuil critique.
pub const MARGE: i64 = 5_000;
/// On ne prévient de nouveau qu'une fois redescendu de 10 °C de plus.
pub const RETOMBEE: i64 = 10_000;

fn lire(chemin: &Path) -> String {
    fs::read_to_string(chemin).map(|t| t.trim().to_string()).unwrap_or_default()
}

pub fn lire_zones(racine: &Path) -> Vec<Zone> {
    let Ok(entrees) = fs::read_dir(racine) else { return Vec::new() };
    let mut zones = Vec::new();
    for e in entrees.flatten() {
        let d = e.path();
        if !d.file_name().map(|n| n.to_string_lossy().starts_with("thermal_zone")).unwrap_or(false) {
            continue;
        }
        let Ok(temperature) = lire(&d.join("temp")).parse::<i64>() else { continue };
        // Une sonde débranchée rend 0, ou une valeur absurde : on l'ignore.
        if !(1_000..=150_000).contains(&temperature) {
            continue;
        }
        let critique = (0..16).find_map(|i| {
            (lire(&d.join(format!("trip_point_{i}_type"))) == "critical")
                .then(|| lire(&d.join(format!("trip_point_{i}_temp"))).parse::<i64>().ok())
                .flatten()
                .filter(|c| SEUIL_VRAISEMBLABLE.contains(c))
        });
        zones.push(Zone { temperature, critique });
    }
    zones
}

/// Les sondes du PROCESSEUR et de la CARTE GRAPHIQUE, lues dans
/// /sys/class/hwmon. Les zones thermiques ne montrent souvent que l'ACPI de
/// la carte mère : sur un PC fixe, ni le cœur du processeur ni la carte
/// graphique n'y sont. `coretemp` (Intel), `k10temp` (AMD), et les pilotes
/// graphiques `nouveau`, `amdgpu`, `radeon`, `i915`, `xe`.
pub fn lire_sondes(racine: &Path) -> Vec<Zone> {
    const UTILES: [&str; 7] = ["coretemp", "k10temp", "nouveau", "amdgpu", "radeon", "i915", "xe"];
    let Ok(entrees) = fs::read_dir(racine) else { return Vec::new() };
    let mut zones = Vec::new();
    for e in entrees.flatten() {
        let d = e.path();
        if !UTILES.contains(&lire(&d.join("name")).as_str()) {
            continue;
        }
        let Ok(fichiers) = fs::read_dir(&d) else { continue };
        for f in fichiers.flatten() {
            let nom = f.file_name().to_string_lossy().into_owned();
            let Some(prefixe) = nom.strip_suffix("_input").filter(|p| p.starts_with("temp")) else { continue };
            let Ok(temperature) = lire(&f.path()).parse::<i64>() else { continue };
            if !(1_000..=150_000).contains(&temperature) {
                continue;
            }
            let critique = lire(&d.join(format!("{prefixe}_crit"))).parse::<i64>().ok().filter(|c| SEUIL_VRAISEMBLABLE.contains(c));
            zones.push(Zone { temperature, critique });
        }
    }
    zones
}

fn seuil(z: Zone) -> i64 {
    z.critique.map(|c| c - MARGE).unwrap_or(SANS_SEUIL)
}

/// La zone la plus proche de son seuil, si l'une le dépasse : (°C, seuil °C).
pub fn alerte(zones: &[Zone]) -> Option<(i64, i64)> {
    zones
        .iter()
        .filter(|z| z.temperature >= seuil(**z))
        .max_by_key(|z| z.temperature - seuil(**z))
        .map(|z| (z.temperature / 1000, z.critique.unwrap_or(SANS_SEUIL + MARGE) / 1000))
}

/// Tout est-il redescendu assez pour réarmer l'alerte ?
pub fn retombe(zones: &[Zone]) -> bool {
    zones.iter().all(|z| z.temperature < seuil(*z) - RETOMBEE)
}

#[cfg(test)]
mod essais {
    use super::*;

    fn zone(racine: &Path, n: u32, temp: &str, critique: Option<&str>) {
        let d = racine.join(format!("thermal_zone{n}"));
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("temp"), format!("{temp}\n")).unwrap();
        fs::write(d.join("trip_point_0_type"), "passive\n").unwrap();
        fs::write(d.join("trip_point_0_temp"), "80000\n").unwrap();
        if let Some(c) = critique {
            fs::write(d.join("trip_point_1_type"), "critical\n").unwrap();
            fs::write(d.join("trip_point_1_temp"), format!("{c}\n")).unwrap();
        }
    }

    #[test]
    fn lit_les_zones_et_leur_seuil() {
        let r = std::env::temp_dir().join(format!("grenos-therm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&r);
        zone(&r, 0, "52000", Some("105000"));
        zone(&r, 1, "0", None); // sonde débranchée
        fs::create_dir_all(r.join("cooling_device0")).unwrap();
        let z = lire_zones(&r);
        assert_eq!(z, vec![Zone { temperature: 52_000, critique: Some(105_000) }]);
        let _ = fs::remove_dir_all(&r);
        assert!(lire_zones(&r).is_empty());
    }

    #[test]
    fn previent_pres_du_seuil() {
        let calme = [Zone { temperature: 60_000, critique: Some(105_000) }];
        assert_eq!(alerte(&calme), None);
        assert!(retombe(&calme));
        let chaud = [Zone { temperature: 101_000, critique: Some(105_000) }, Zone { temperature: 50_000, critique: None }];
        assert_eq!(alerte(&chaud), Some((101, 105)));
        assert!(!retombe(&chaud));
        // Sans seuil : arrêt compté à 105 °C, alerte à 100.
        let sans_seuil = [Zone { temperature: 96_000, critique: None }];
        assert_eq!(alerte(&sans_seuil), None);
        let tres_chaud = [Zone { temperature: 101_000, critique: None }];
        assert_eq!(alerte(&tres_chaud), Some((101, 105)));
    }

    #[test]
    fn ignore_le_faux_seuil_de_l_acpi() {
        // acpitz d'un PC fixe : 16,8 °C figés, « critique » à 20,8 °C.
        let r = std::env::temp_dir().join(format!("grenos-acpi-{}", std::process::id()));
        let _ = fs::remove_dir_all(&r);
        zone(&r, 0, "16800", Some("20800"));
        let z = lire_zones(&r);
        assert_eq!(z, vec![Zone { temperature: 16_800, critique: None }]);
        assert_eq!(alerte(&z), None);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn lit_le_processeur_et_la_carte_graphique() {
        let r = std::env::temp_dir().join(format!("grenos-hwmon-{}", std::process::id()));
        let _ = fs::remove_dir_all(&r);
        for (n, nom, temp, crit) in [(0, "coretemp", "45000", Some("100000")), (1, "nouveau", "52000", None), (2, "nvme", "38000", Some("84850")), (3, "acpitz", "16800", Some("20800"))] {
            let d = r.join(format!("hwmon{n}"));
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("name"), format!("{nom}\n")).unwrap();
            fs::write(d.join("temp1_input"), format!("{temp}\n")).unwrap();
            if let Some(c) = crit {
                fs::write(d.join("temp1_crit"), format!("{c}\n")).unwrap();
            }
        }
        let mut z = lire_sondes(&r);
        z.sort_by_key(|z| z.temperature);
        // Le disque NVMe et l'ACPI ne sont pas lus ici.
        assert_eq!(z, vec![Zone { temperature: 45_000, critique: Some(100_000) }, Zone { temperature: 52_000, critique: None }]);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn rearme_seulement_apres_la_retombee() {
        // 92 °C sous un seuil de 105 : sous l'alerte (100), mais pas assez
        // redescendu (90) pour réarmer — pas de rafale de notifications.
        let tiede = [Zone { temperature: 92_000, critique: Some(105_000) }];
        assert_eq!(alerte(&tiede), None);
        assert!(!retombe(&tiede));
    }
}
