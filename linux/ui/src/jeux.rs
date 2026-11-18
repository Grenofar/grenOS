//! Jeux : ce que la machine sait faire, ce qu'il lui manque, et la vérité sur
//! chaque jeu demandé — celui qui marche, et celui qui ne marche pas, avec le
//! chemin qui, lui, marche.
//!
//! Repris de `grenos-jeux` (Python) le 18 novembre. La lecture (Vulkan, pilote,
//! GameMode, Steam) reste `jeux.py`, par le pont : elle est exercée par nos
//! essais. Les lignes série sont celles du Python, au mot près.

use crate::pont;
use crate::systeme::{detacher, dire};
use serde_json::{json, Value};

#[tauri::command]
pub async fn jeux_vue() -> Result<Value, String> {
    let v = pont::appeler("jeux_vue", json!({}))?;
    let titres = v["jeux"].as_array().map(|j| j.len()).unwrap_or(0);
    dire(&format!("jeux ouvert, {titres} titres, 3D {}", if v["vulkan"] == true { "oui" } else { "non" }));
    dire(&format!(
        "rocket league : epic {}, steam {}",
        if v["heroic_pose"] == true { "pose" } else { "a installer" },
        if v["rocket_prete"] == true { "regle" } else { "a regler" }
    ));
    Ok(v)
}

#[tauri::command]
pub async fn jeux_paquet_nvidia() -> Result<String, String> {
    Ok(pont::appeler("jeux_paquet_nvidia", json!({}))?["paquet"].as_str().unwrap_or("nvidia-driver").to_string())
}

/// « steam » (à installer), « jamais » (jamais lancé), « ouvert » (à fermer),
/// « illisible », ou "" : Proton Experimental est réglé.
#[tauri::command]
pub async fn jeux_preparer_rocket() -> Result<String, String> {
    let manque = pont::appeler("jeux_preparer_rocket", json!({}))?["manque"].as_str().unwrap_or("").to_string();
    if manque.is_empty() {
        let _ = detacher("xdg-open", &["steam://store/252950"]);
    } else if manque == "jamais" {
        let _ = detacher("xdg-open", &["steam://open/main"]);
    }
    Ok(manque)
}

/// « chromium » (à installer d'abord), ou "" : posé sur le bureau et au menu.
#[tauri::command]
pub async fn jeux_poser_fortnite() -> Result<String, String> {
    Ok(pont::appeler("jeux_poser_fortnite", json!({}))?["manque"].as_str().unwrap_or("").to_string())
}

/// Les gestes « ouvrir » de la page, par clé : jamais une commande libre.
fn geste(cle: &str) -> Option<(&'static str, &'static [&'static str])> {
    Some(match cle {
        "heroic" => ("flatpak", &["run", "com.heroicgameslauncher.hgl"]),
        "epic" => ("xdg-open", &["https://store.epicgames.com/p/rocket-league"]),
        "rocket" => ("xdg-open", &["steam://store/252950"]),
        "roblox" => ("flatpak", &["run", "org.vinegarhq.Sober"]),
        "roblox-site" => ("xdg-open", &["https://sober.vinegarhq.org"]),
        "fortnite" => ("gtk-launch", &["grenos-fortnite"]),
        _ => return None,
    })
}

#[tauri::command]
pub fn jeux_ouvrir(cle: String) -> Result<(), String> {
    let (programme, arguments) = geste(&cle).ok_or("geste inconnu")?;
    detacher(programme, arguments)
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn gestes_connus_seulement() {
        for c in ["heroic", "epic", "rocket", "roblox", "roblox-site", "fortnite"] {
            assert!(geste(c).is_some(), "{c}");
        }
        assert!(geste("sh").is_none());
    }
}
