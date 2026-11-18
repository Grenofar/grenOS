//! grenHub : l'état complet de la machine, et un diagnostic qui l'éprouve.
//!
//! Repris de `grenos-hub` (Python) le 18 novembre. Le haut LIT (sans rien
//! toucher), le bas ÉPROUVE (calcule, écrit, relit, va chercher le réseau).
//! Le diagnostic part APRÈS l'affichage, hors du fil qui dessine : c'est la
//! règle payée par la page Jeux (`vulkaninfo`, vingt-cinq secondes d'écran
//! vide). La mesure elle-même reste `sante.py`, par le pont.
//!
//! Les lignes série sont celles du Python, au mot près : « hub ouvert, N
//! bloc(s) d etat », « hub : thermique — … », « hub : diagnostic X bon » (et
//! la raison quand ça va mal), « hub : diagnostic termine — … ».

use crate::pont;
use crate::systeme::dire;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

static EN_COURS: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub async fn hub_etat() -> Result<Value, String> {
    let v = pont::appeler("hub_etat", json!({}))?;
    let blocs = v["paves"].as_array().map(|p| p.len()).unwrap_or(0);
    dire(&format!("hub ouvert, {blocs} bloc(s) d etat"));
    dire(&format!(
        "hub : thermique — {} sonde(s), {} ventilateur(s)",
        v["sondes"].as_u64().unwrap_or(0),
        v["ventilateurs"].as_u64().unwrap_or(0)
    ));
    Ok(v)
}

/// Une ligne série par essai : le détail seulement quand ça va mal — sept
/// détails à chaque démarrage noieraient les lignes qui comptent.
fn ligne_serie(v: &Value) -> String {
    let nom = v["nom"].as_str().unwrap_or("?");
    let etat = v["etat"].as_str().unwrap_or("?");
    if etat == "bon" {
        format!("hub : diagnostic {nom} {etat}")
    } else {
        format!("hub : diagnostic {nom} {etat} — {}", v["detail"].as_str().unwrap_or(""))
    }
}

#[tauri::command]
pub fn hub_diagnostic(app: tauri::AppHandle) -> Result<(), String> {
    if EN_COURS.swap(true, Ordering::SeqCst) {
        return Err("Un diagnostic est déjà en cours.".into());
    }
    std::thread::spawn(move || {
        let resultat = pont::suivre("hub_diagnostic", json!({}), |v| {
            if v.get("fin").and_then(Value::as_bool) == Some(true) {
                dire(&format!(
                    "hub : diagnostic termine — {}, {} point(s)",
                    v["etat"].as_str().unwrap_or("?"),
                    v["points"].as_u64().unwrap_or(0)
                ));
                let _ = app.emit("hub-fin", v);
            } else {
                dire(&ligne_serie(&v));
                let _ = app.emit("hub-ligne", v);
            }
        });
        if let Err(e) = resultat {
            dire(&format!("hub : diagnostic impossible — {e}"));
            let _ = app.emit("hub-fin", json!({"fin": true, "etat": "en echec", "phrase": e, "points": 0}));
        }
        EN_COURS.store(false, Ordering::SeqCst);
    });
    Ok(())
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn lignes_serie() {
        assert_eq!(ligne_serie(&json!({"nom": "Processeur", "etat": "bon", "detail": "x"})), "hub : diagnostic Processeur bon");
        assert_eq!(
            ligne_serie(&json!({"nom": "Services", "etat": "en echec", "detail": "cups.service"})),
            "hub : diagnostic Services en echec — cups.service"
        );
    }
}
