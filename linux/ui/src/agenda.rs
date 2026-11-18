//! L'Agenda (todo G32 — 18 novembre) : les événements de la personne, dans
//! un fichier à elle (~/.local/share/grenos/agenda.json). Pas de compte, pas
//! de nuage : c'est sa machine.
//!
//! Rust ne fait que lire et écrire le fichier — par un fichier temporaire
//! renommé, pour qu'une coupure au milieu ne laisse jamais un agenda à moitié
//! écrit. Le calendrier lui-même (mois, jours fériés) est dans la page.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Evenement {
    pub id: String,
    /// AAAA-MM-JJ
    pub date: String,
    /// HH:MM, ou vide pour « toute la journée ».
    #[serde(default)]
    pub heure: String,
    pub titre: String,
    #[serde(default)]
    pub note: String,
}

fn fichier() -> PathBuf {
    let maison = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(maison).join(".local/share/grenos/agenda.json")
}

fn date_valable(d: &str) -> bool {
    let b = d.as_bytes();
    b.len() == 10 && b[4] == b'-' && b[7] == b'-' && d.chars().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

fn heure_valable(h: &str) -> bool {
    h.is_empty() || (h.len() == 5 && h.as_bytes()[2] == b':' && h.chars().enumerate().all(|(i, c)| i == 2 || c.is_ascii_digit()))
}

/// Ce que la page envoie est vérifié avant d'être écrit : un agenda illisible
/// au prochain lancement serait un agenda perdu.
pub fn valable(e: &Evenement) -> Result<(), &'static str> {
    if !date_valable(&e.date) {
        return Err("Date illisible.");
    }
    if !heure_valable(&e.heure) {
        return Err("Heure illisible (HH:MM).");
    }
    if e.titre.trim().is_empty() {
        return Err("Un titre, au moins un mot.");
    }
    if e.titre.len() > 200 || e.note.len() > 4000 || e.id.len() > 64 {
        return Err("Trop long.");
    }
    Ok(())
}

pub fn trier(v: &mut [Evenement]) {
    v.sort_by(|a, b| (a.date.as_str(), a.heure.is_empty() == false, a.heure.as_str()).cmp(&(b.date.as_str(), b.heure.is_empty() == false, b.heure.as_str())));
}

fn lire() -> Vec<Evenement> {
    std::fs::read_to_string(fichier()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn ecrire(v: &[Evenement]) -> Result<(), String> {
    let f = fichier();
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = f.with_extension("json.part");
    std::fs::write(&tmp, serde_json::to_string_pretty(v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &f).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn agenda_lire() -> Vec<Evenement> {
    let mut v = lire();
    trier(&mut v);
    v
}

/// Ajoute, ou remplace l'événement de même id.
#[tauri::command]
pub fn agenda_poser(evenement: Evenement) -> Result<Vec<Evenement>, String> {
    valable(&evenement)?;
    let mut v = lire();
    v.retain(|e| e.id != evenement.id);
    v.push(evenement);
    trier(&mut v);
    ecrire(&v)?;
    Ok(v)
}

#[tauri::command]
pub fn agenda_retirer(id: String) -> Result<Vec<Evenement>, String> {
    let mut v = lire();
    v.retain(|e| e.id != id);
    ecrire(&v)?;
    Ok(v)
}

#[cfg(test)]
mod essais {
    use super::*;

    fn e(date: &str, heure: &str, titre: &str) -> Evenement {
        Evenement { id: titre.into(), date: date.into(), heure: heure.into(), titre: titre.into(), note: String::new() }
    }

    #[test]
    fn verifie_avant_d_ecrire() {
        assert!(valable(&e("2026-11-18", "14:30", "Dentiste")).is_ok());
        assert!(valable(&e("2026-11-18", "", "Anniversaire")).is_ok());
        assert!(valable(&e("18/11/2026", "", "x")).is_err());
        assert!(valable(&e("2026-11-18", "2h", "x")).is_err());
        assert!(valable(&e("2026-11-18", "", "   ")).is_err());
    }

    #[test]
    fn la_journee_avant_les_heures() {
        let mut v = vec![e("2026-11-19", "09:00", "b"), e("2026-11-18", "18:00", "c"), e("2026-11-18", "", "a"), e("2026-11-18", "08:00", "d")];
        trier(&mut v);
        let t: Vec<_> = v.iter().map(|x| x.titre.as_str()).collect();
        assert_eq!(t, ["a", "d", "c", "b"]);
    }
}
