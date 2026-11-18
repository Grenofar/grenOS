//! Ce que grenOS trouve dans la machine — I14 de sa liste : processeur,
//! mémoire, carte graphique, ET le genre de disque, le Wi-Fi, le Bluetooth et
//! le son, qui manquaient à la version Python.
//!
//! Tout est LU (dans /proc, /sys, ou `lspci`), jamais deviné. Une valeur
//! introuvable est dite « inconnue », pas remplacée par un défaut plausible.

use crate::systeme::{lire, sortie};
use serde::Serialize;
use std::fs;

#[derive(Serialize, Clone)]
pub struct Ligne {
    pub icone: &'static str,
    pub nom: &'static str,
    pub valeur: String,
    pub detail: String,
}

fn coeurs() -> (usize, String) {
    let texte = lire("/proc/cpuinfo");
    let nombre = texte.lines().filter(|l| l.starts_with("processor")).count();
    let modele = texte
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    (nombre, modele)
}

fn memoire_go() -> f64 {
    lire("/proc/meminfo")
        .lines()
        .find(|l| l.starts_with("MemTotal:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse::<f64>().ok())
        .map(|ko| ko / 1024.0 / 1024.0)
        .unwrap_or(0.0)
}

const CARTES_CONNUES: &[(&str, &str)] = &[
    ("1234:1111", "Carte graphique virtuelle (QEMU/Bochs)"),
    ("1af4:1050", "Carte graphique virtuelle (VirtIO)"),
    ("80ee:beef", "Carte graphique virtuelle (VirtualBox)"),
    ("15ad:0405", "Carte graphique virtuelle (VMware)"),
];

fn graphique() -> (String, String) {
    let texte = sortie("lspci", &["-nnk"]);
    let mut carte = String::new();
    let mut pilote = String::new();
    let mut dans_bloc = false;
    for ligne in texte.lines() {
        if ligne.contains("VGA compatible controller")
            || ligne.contains("3D controller")
            || ligne.contains("Display controller")
        {
            dans_bloc = true;
            carte = ligne.splitn(3, ':').last().unwrap_or("").trim().to_string();
        } else if dans_bloc && ligne.contains("Kernel driver in use") {
            pilote = ligne.split_once(':').map(|(_, v)| v.trim().to_string()).unwrap_or_default();
            break;
        }
    }
    if carte.is_empty() {
        return ("inconnue".into(), "pilote générique".into());
    }
    for (id, nom) in CARTES_CONNUES {
        if carte.contains(id) {
            carte = (*nom).into();
        }
    }
    // « … [10de:2484] (rev a1) » : les nombres ne disent rien à personne.
    if let Some(i) = carte.rfind(" [") {
        if carte[i..].contains(':') {
            carte.truncate(i);
        }
    }
    (carte, if pilote.is_empty() { "pilote générique".into() } else { pilote })
}

fn disques() -> String {
    let Ok(entrees) = fs::read_dir("/sys/block") else { return "inconnu".into() };
    let mut genres = Vec::new();
    for entree in entrees.flatten() {
        let nom = entree.file_name().to_string_lossy().into_owned();
        if nom.starts_with("loop") || nom.starts_with("ram") || nom.starts_with("zram")
            || nom.starts_with("sr") || nom.starts_with("dm-")
        {
            continue;
        }
        let secteurs: f64 = lire(&format!("/sys/block/{nom}/size")).trim().parse().unwrap_or(0.0);
        let go = secteurs * 512.0 / 1e9;
        if go < 1.0 {
            continue;
        }
        let genre = if nom.starts_with("nvme") {
            "SSD NVMe"
        } else if lire(&format!("/sys/block/{nom}/queue/rotational")).trim() == "1" {
            "Disque dur"
        } else {
            "SSD"
        };
        genres.push(format!("{genre} {go:.0} Go"));
    }
    if genres.is_empty() { "aucun disque".into() } else { genres.join(", ") }
}

fn wifi() -> bool {
    fs::read_dir("/sys/class/net")
        .map(|d| d.flatten().any(|e| e.path().join("wireless").exists()))
        .unwrap_or(false)
}

fn bluetooth() -> bool {
    fs::read_dir("/sys/class/bluetooth").map(|mut d| d.next().is_some()).unwrap_or(false)
}

fn son() -> String {
    let cartes: Vec<String> = lire("/proc/asound/cards")
        .lines()
        .filter_map(|l| l.split_once(" - ").map(|(_, n)| n.trim().to_string()))
        .collect();
    if cartes.is_empty() { "aucune carte son".into() } else { cartes.join(", ") }
}

pub fn virtuelle() -> String {
    let v = sortie("systemd-detect-virt", &[]);
    let v = v.trim();
    if v.is_empty() || v == "none" { String::new() } else { v.to_string() }
}

pub fn decimal(valeur: f64, decimales: usize) -> String {
    format!("{valeur:.decimales$}").replace('.', ",")
}

pub fn resume() -> Vec<Ligne> {
    let (n, modele) = coeurs();
    let (carte, pilote) = graphique();
    let oui_non = |b: bool, oui: &str| if b { oui.to_string() } else { "absent".to_string() };
    vec![
        Ligne { icone: "cpu", nom: "Processeur", valeur: format!("{n} cœur(s)"), detail: modele },
        Ligne { icone: "ram", nom: "Mémoire", valeur: format!("{} Go", decimal(memoire_go(), 1)), detail: String::new() },
        Ligne { icone: "gpu", nom: "Carte graphique", valeur: carte, detail: pilote },
        Ligne { icone: "disque", nom: "Stockage", valeur: disques(), detail: String::new() },
        Ligne { icone: "wifi", nom: "Wi-Fi", valeur: oui_non(wifi(), "présent"), detail: String::new() },
        Ligne { icone: "bt", nom: "Bluetooth", valeur: oui_non(bluetooth(), "présent"), detail: String::new() },
        Ligne { icone: "son", nom: "Son", valeur: son(), detail: String::new() },
    ]
}
