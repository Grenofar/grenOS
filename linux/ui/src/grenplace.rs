//! GrenPlace : les applications de grenOS, demandées à grenOS.
//!
//! Repris de `grenplace` (Python) le 18 novembre. Le magasin ne connaît
//! aucune application : il lit le catalogue (en ligne, sinon la copie gardée,
//! sinon celle de l'image — `pont.py`, magasin_catalogue). Aucune application
//! ni aucun logo n'est stocké chez nous : la fiche dit d'où l'application
//! s'installe (Flathub ou Debian) et où est son logo.
//!
//! Ce qui change : « déjà installé ? » se demande UNE fois par source
//! (`flatpak list`, `dpkg-query`), et non une fois par fiche — 233 `flatpak
//! info` à l'ouverture, c'était 233 programmes lancés pour dessiner une liste.

use crate::installer::identifiant_sain;
use crate::pont;
use crate::systeme::{dire, sortie};
use serde_json::{json, Value};
use std::collections::HashSet;

fn flatpaks_installes() -> HashSet<String> {
    sortie("flatpak", &["list", "--app", "--columns=application"])
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Les paquets Debian installés parmi ceux demandés, en un seul dpkg-query.
fn paquets_installes(noms: &[&str]) -> HashSet<String> {
    if noms.is_empty() {
        return HashSet::new();
    }
    let mut arguments = vec!["-W", "-f=${Package} ${Status}\n"];
    arguments.extend(noms.iter().copied());
    paquets_de(&sortie("dpkg-query", &arguments))
}

pub(crate) fn paquets_de(texte: &str) -> HashSet<String> {
    texte
        .lines()
        .filter_map(|l| {
            let (nom, etat) = l.split_once(' ')?;
            etat.starts_with("install ok").then(|| nom.split(':').next().unwrap_or(nom).to_string())
        })
        .collect()
}

#[tauri::command]
pub async fn grenplace_catalogue() -> Result<Value, String> {
    let mut v = pont::appeler("magasin_catalogue", json!({}))?;
    let origine = v["origine"].as_str().unwrap_or("?").to_string();
    let applications = v["applications"].as_array_mut().ok_or("catalogue illisible")?;
    let flatpaks = flatpaks_installes();
    let apt: Vec<String> = applications
        .iter()
        .filter(|a| a["source"] == "apt")
        .filter_map(|a| a["identifiant"].as_str().map(String::from))
        .filter(|i| identifiant_sain(i))
        .collect();
    let paquets = paquets_installes(&apt.iter().map(String::as_str).collect::<Vec<_>>());
    for a in applications.iter_mut() {
        let id = a["identifiant"].as_str().unwrap_or("").to_string();
        let la = if a["source"] == "flatpak" { flatpaks.contains(&id) } else { paquets.contains(&id) };
        a["installe"] = json!(la);
    }
    dire(&format!("grenplace ouvert, {} applications ({origine})", applications.len()));
    Ok(v)
}

/// Lesquelles des applications INSTALLÉES ont une mise à jour (magasin.py :
/// une question par source, puis on croise).
#[tauri::command]
pub async fn grenplace_retards(applications: Vec<Value>) -> Result<Vec<String>, String> {
    let v = pont::appeler("magasin_retards", json!({ "applications": applications }))?;
    let retards: Vec<String> = v["retards"].as_array().map(|l| l.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default();
    dire(&format!("grenplace : {} application(s) a mettre a jour", retards.len()));
    Ok(retards)
}

#[tauri::command]
pub async fn grenplace_installe(source: String, identifiant: String) -> bool {
    crate::installer::installe(&source, &identifiant)
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn dpkg_lu() {
        let p = paquets_de("gimp install ok installed\nvlc deinstall ok config-files\nlibc6:amd64 install ok installed\n");
        assert!(p.contains("gimp") && p.contains("libc6") && !p.contains("vlc"));
    }
}
