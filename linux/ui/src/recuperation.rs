//! grenRecovery (todo C3 à C9, C18, J11 — 18 novembre) : réparer une
//! installation grenOS qui ne démarre plus, depuis la clé grenOS.
//!
//! Tout ce qui demande root passe par UN programme, `grenos-recuperation`,
//! qui vérifie chaque nom de partition avant de s'en servir. La page ne donne
//! qu'un geste connu et une partition ; Rust refuse le reste.

use crate::systeme::{dire, privilegie};
use serde_json::Value;
use std::process::{Command, Stdio};

const OUTIL: &str = "/usr/lib/grenos/grenos-recuperation";
const GESTES: &[&str] = &["verifier", "ouvrir", "fermer", "mdp", "noyau", "demarrage"];

fn root(arguments: &[&str]) -> Result<String, String> {
    let p = privilegie();
    let o = Command::new(&p[0])
        .args(&p[1..])
        .arg(OUTIL)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("grenRecovery n'a pas pu démarrer : {e}"))?;
    let sortie = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if o.status.success() {
        Ok(sortie)
    } else {
        let erreur = String::from_utf8_lossy(&o.stderr).trim().to_string();
        Err(if erreur.is_empty() { format!("Le geste a échoué.\n{sortie}") } else { erreur })
    }
}

/// Une partition : /dev/sda2, /dev/nvme0n1p2, /dev/vda1, /dev/mmcblk0p1.
pub fn partition_valide(d: &str) -> bool {
    let Some(nom) = d.strip_prefix("/dev/") else { return false };
    let chiffres_finals = nom.chars().rev().take_while(|c| c.is_ascii_digit()).count();
    if chiffres_finals == 0 || nom.len() > 20 || !nom.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    nom.starts_with("sd") || nom.starts_with("vd") || (nom.starts_with("nvme") && nom.contains('p')) || (nom.starts_with("mmcblk") && nom.contains('p'))
}

pub fn compte_valide(c: &str) -> bool {
    let mut it = c.chars();
    matches!(it.next(), Some(p) if p.is_ascii_lowercase() || p == '_')
        && c.len() <= 32
        && it.all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || x == '_' || x == '-')
}

#[tauri::command]
pub async fn recuperation_lister() -> Result<Vec<Value>, String> {
    let sortie = root(&["lister"])?;
    let liste: Vec<Value> = sortie.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let grenos = liste.iter().filter(|p| p["grenos"] == true).count();
    dire(&format!("recuperation ouvert, {} partition(s), {grenos} installation(s) grenOS", liste.len()));
    Ok(liste)
}

#[tauri::command]
pub async fn recuperation_faire(geste: String, partition: String, compte: Option<String>) -> Result<String, String> {
    if !GESTES.contains(&geste.as_str()) {
        return Err("geste inconnu".into());
    }
    if !partition_valide(&partition) {
        return Err("partition refusée".into());
    }
    let mut arguments = vec![geste.as_str(), partition.as_str()];
    let c;
    if geste == "mdp" {
        c = compte.unwrap_or_default();
        if !compte_valide(&c) {
            return Err("compte refusé".into());
        }
        arguments.push(&c);
    }
    root(&arguments)
}

/// Timeshift (restaurer un instantané) : sa propre fenêtre, qui sait
/// restaurer vers un autre disque depuis la clé.
#[tauri::command]
pub fn recuperation_instantanes() -> Result<(), String> {
    crate::systeme::detacher("pkexec", &["timeshift-gtk"])
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn partitions() {
        for ok in ["/dev/sda2", "/dev/vda1", "/dev/nvme0n1p3", "/dev/mmcblk0p1"] {
            assert!(partition_valide(ok), "{ok}");
        }
        for non in ["/dev/sda", "/dev/nvme0n1", "/etc/passwd", "/dev/sda1;rm", "/dev/../sda1", "sda1", "/dev/loop0"] {
            assert!(!partition_valide(non), "{non}");
        }
    }

    #[test]
    fn comptes() {
        assert!(compte_valide("lea"));
        assert!(compte_valide("jean-marc_2"));
        assert!(!compte_valide("Root"));
        assert!(!compte_valide("-x"));
        assert!(!compte_valide("a;b"));
        assert!(!compte_valide(""));
    }
}
