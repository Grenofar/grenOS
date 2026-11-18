//! Gestionnaire de tâches (Ctrl+Maj+Échap) : ce qui tourne, et ce que ça coûte.
//!
//! Repris de `grenos-taches` (Python) le 18 novembre. grenPerf regroupe par
//! application ; ici, c'est la liste de TOUS les processus, un par ligne, avec
//! leur PID — la vue de celui qui cherche ce qui ne répond plus.
//!
//! Tout est lu dans /proc. Le pourcentage de processeur est une vraie mesure :
//! la différence de temps consommé entre deux lectures, rapportée au temps
//! écoulé. La première lecture sert de base, et dit 0 %.

use crate::perf::{cpu_total, meminfo};
use crate::systeme::{dire, reussit};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

#[derive(Default)]
pub struct Etat {
    precedent: Mutex<Option<(u64, HashMap<u32, u64>)>>,
}

/// Ce qui exécute le programme de quelqu'un d'autre : voir « python3 » quatre
/// fois n'apprend rien, c'est le programme d'après qu'il faut lire.
const INTERPRETES: &[&str] = &["python", "python3", "sh", "bash", "dash", "perl", "ruby", "node", "nodejs", "wish", "tclsh", "env"];

fn base(chemin: &str) -> &str {
    chemin.rsplit('/').next().unwrap_or(chemin)
}

/// Le nom à montrer pour une ligne de commande, ou rien.
pub fn nom_lisible(mots: &[&str]) -> String {
    let Some(premier) = mots.first() else { return String::new() };
    let nom = base(premier);
    if !INTERPRETES.contains(&nom) {
        return if nom.starts_with('-') { String::new() } else { nom.to_string() };
    }
    for mot in &mots[1..] {
        if mot.starts_with('-') {
            continue;
        }
        let suivant = base(mot);
        if INTERPRETES.contains(&suivant) {
            continue;
        }
        return suivant.to_string();
    }
    nom.to_string()
}

struct Proc {
    pid: u32,
    nom: String,
    ticks: u64,
    memoire_ko: u64,
}

fn lire(pid: u32) -> Option<Proc> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let ouvre = stat.find('(')?;
    let ferme = stat.rfind(')')?;
    let mut nom = stat[ouvre + 1..ferme].to_string();
    let champs: Vec<&str> = stat.get(ferme + 2..)?.split_whitespace().collect();
    let ticks = champs.get(11)?.parse::<u64>().ok()? + champs.get(12)?.parse::<u64>().ok()?;
    let memoire_ko = fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:").and_then(|v| v.split_whitespace().next()?.parse().ok()))
        .unwrap_or(0);
    if let Ok(brut) = fs::read(format!("/proc/{pid}/cmdline")) {
        let mots: Vec<String> = brut.split(|b| *b == 0).filter(|m| !m.is_empty()).map(|m| String::from_utf8_lossy(m).into_owned()).collect();
        let mots: Vec<&str> = mots.iter().map(String::as_str).collect();
        let candidat = nom_lisible(&mots);
        if !candidat.is_empty() {
            nom = candidat;
        }
    }
    Some(Proc { pid, nom, ticks, memoire_ko })
}

fn tous() -> Vec<Proc> {
    let Ok(entrees) = fs::read_dir("/proc") else { return vec![] };
    entrees.flatten().filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok()).filter_map(lire).collect()
}

#[derive(Serialize)]
pub struct Ligne {
    pid: u32,
    nom: String,
    cpu: f64,
    memoire_mo: f64,
}

#[derive(Serialize)]
pub struct Mesure {
    lignes: Vec<Ligne>,
    total: usize,
    coeurs: usize,
    cpu: f64,
    memoire_utilisee_go: f64,
    memoire_totale_go: f64,
}

#[tauri::command]
pub async fn taches_mesure(etat: tauri::State<'_, Etat>) -> Result<Mesure, String> {
    let (horloge, coeurs) = cpu_total();
    let liste = tous();
    let mut garde = etat.precedent.lock().unwrap();
    let premiere = garde.is_none();
    let (avant_horloge, avant) = garde.take().unwrap_or((horloge, HashMap::new()));
    let ecoule = horloge.saturating_sub(avant_horloge).max(1) as f64;
    let mut total_cpu = 0.0;
    let mut lignes = Vec::new();
    for p in &liste {
        let part = avant.get(&p.pid).map(|t| p.ticks.saturating_sub(*t) as f64 / ecoule * 100.0 * coeurs as f64).unwrap_or(0.0);
        total_cpu += part;
        // Les fils du noyau : ni mémoire à eux ni activité. Les montrer
        // noierait ce que la personne cherche.
        if p.memoire_ko == 0 && part < 0.05 {
            continue;
        }
        lignes.push(Ligne { pid: p.pid, nom: p.nom.clone(), cpu: part, memoire_mo: p.memoire_ko as f64 / 1024.0 });
    }
    *garde = Some((horloge, liste.iter().map(|p| (p.pid, p.ticks)).collect()));
    if premiere {
        dire(&format!("taches ouvert, {} processus", liste.len()));
    }
    let m = meminfo();
    let totale = m.get("MemTotal").copied().unwrap_or(0.0);
    let dispo = m.get("MemAvailable").copied().unwrap_or(0.0);
    Ok(Mesure {
        lignes,
        total: liste.len(),
        coeurs,
        cpu: (total_cpu / coeurs as f64).min(100.0),
        memoire_utilisee_go: (totale - dispo) / 1024.0 / 1024.0,
        memoire_totale_go: totale / 1024.0 / 1024.0,
    })
}

/// Arrêter un processus (SIGTERM). Le sien : directement. Celui d'un autre :
/// pkexec demande l'autorisation.
#[tauri::command]
pub async fn taches_arreter(pid: u32) -> Result<(), String> {
    if pid <= 1 {
        return Err("Ce processus fait tourner la machine.".into());
    }
    if reussit("kill", &["-TERM", &pid.to_string()]) {
        return Ok(());
    }
    if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
        return Ok(()); // déjà parti
    }
    if reussit("pkexec", &["kill", "-TERM", &pid.to_string()]) {
        Ok(())
    } else {
        Err("Le système a refusé.".into())
    }
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn noms_lisibles() {
        assert_eq!(nom_lisible(&["/usr/bin/python3", "-I", "/usr/bin/grenplace"]), "grenplace");
        assert_eq!(nom_lisible(&["/usr/bin/env", "python3", "grenos-hub"]), "grenos-hub");
        assert_eq!(nom_lisible(&["/usr/lib/firefox-esr/firefox-esr", "-new-tab"]), "firefox-esr");
        assert_eq!(nom_lisible(&["-bash"]), "");
        assert_eq!(nom_lisible(&["bash"]), "bash");
        assert_eq!(nom_lisible(&[]), "");
    }
}
