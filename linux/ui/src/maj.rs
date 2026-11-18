//! Mises à jour : l'état, le mode, et la préparation.
//!
//! Le principe ne change pas, et il a coûté cher à apprendre : RIEN ne
//! s'installe pendant que le bureau tourne. La fenêtre télécharge, met à jour
//! les applications Flatpak (qui se remplacent sans rien casser), et arme
//! l'installation au prochain démarrage, avant le bureau.
//!
//! Tout le travail privilégié passe par UN seul programme,
//! `grenos-maj-preparer`, lancé une seule fois : une demande d'autorisation,
//! pas quatre. Il parle par lignes `@etape <pour cent> <texte>` et
//! `@fin ok|echec <texte>`, que l'on transforme en événements pour la page.

use crate::systeme::{dire, lire, privilegie, reussit, sortie};
use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, State};

const REGLAGE: &str = "/usr/lib/grenos/grenos-maj-reglage";
const PREPARER: &str = "/usr/lib/grenos/grenos-maj-preparer";

#[derive(Default)]
pub struct Etat {
    en_cours: AtomicBool,
}

#[derive(Serialize)]
pub struct Vue {
    /// None quand on ne sait pas : sans réseau, « rien » n'est pas « zéro ».
    disponibles: Option<u32>,
    flatpak: Option<u32>,
    date: String,
    paquets: Vec<String>,
    arme: bool,
    telechargee: bool,
    mode: String,
    dernier_resultat: String,
    en_cours: bool,
}

fn nombre(chemin: &str) -> Option<u32> {
    lire(chemin).trim().parse().ok()
}

pub fn mode() -> String {
    let lu = lire("/etc/grenos/maj-mode");
    match lu.trim() {
        m @ ("automatiques" | "demander" | "manuelles") => m.to_string(),
        _ if std::path::Path::new("/etc/grenos/maj-automatique").exists() => "automatiques".into(),
        _ => "demander".into(),
    }
}

fn arme() -> bool {
    sortie("systemctl", &["is-enabled", "grenos-maj-demarrage.service"]).trim() == "enabled"
}

#[tauri::command]
pub async fn maj_etat(etat: State<'_, Etat>) -> Result<Vue, String> {
    Ok(Vue {
        disponibles: nombre("/run/grenos/maj"),
        flatpak: nombre("/run/grenos/maj-flatpak"),
        date: lire("/run/grenos/maj-date").trim().to_string(),
        paquets: lire("/run/grenos/maj-liste").lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect(),
        arme: arme(),
        telechargee: lire("/run/grenos/maj-telechargee").trim() == "oui",
        mode: mode(),
        dernier_resultat: lire("/run/grenos/maj-resultat").trim().to_string(),
        en_cours: etat.en_cours.load(Ordering::SeqCst),
    })
}

fn privilegie_lancer(arguments: &[&str]) -> Result<(), String> {
    let mut l = privilegie();
    let programme = l.remove(0);
    let mut tout: Vec<&str> = l.iter().map(|s| s.as_str()).collect();
    tout.extend_from_slice(arguments);
    if reussit(&programme, &tout) { Ok(()) } else { Err("Le système a refusé.".into()) }
}

#[tauri::command]
pub async fn maj_mode(mode: String) -> Result<(), String> {
    if !["automatiques", "demander", "manuelles"].contains(&mode.as_str()) {
        return Err("mode inconnu".into());
    }
    privilegie_lancer(&[REGLAGE, "mode", &mode])
}

#[tauri::command]
pub async fn maj_verifier() -> Result<(), String> {
    privilegie_lancer(&[REGLAGE, "maintenant"])
}

#[derive(Serialize, Clone)]
struct Avance { part: u32, texte: String }
#[derive(Serialize, Clone)]
struct Fin { bien: bool, texte: String, arme: bool }

#[tauri::command]
pub fn maj_preparer(app: tauri::AppHandle, etat: State<'_, Etat>) -> Result<(), String> {
    if etat.en_cours.swap(true, Ordering::SeqCst) {
        return Err("Une préparation est déjà en cours.".into());
    }
    std::thread::spawn(move || {
        let fin = preparer(&app);
        let _ = app.emit("maj-fin", fin);
        if let Some(e) = tauri::Manager::try_state::<Etat>(&app) {
            e.en_cours.store(false, Ordering::SeqCst);
        }
    });
    Ok(())
}

fn preparer(app: &tauri::AppHandle) -> Fin {
    let mut l = privilegie();
    let programme = l.remove(0);
    let enfant = Command::new(&programme)
        .args(&l)
        .arg(PREPARER)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("DEBIAN_FRONTEND", "noninteractive")
        .spawn();
    let mut enfant = match enfant {
        Ok(e) => e,
        Err(e) => return Fin { bien: false, texte: format!("Impossible de lancer la préparation : {e}"), arme: false },
    };
    let mut fin: Option<Fin> = None;
    if let Some(sortie) = enfant.stdout.take() {
        for ligne in BufReader::new(sortie).lines().map_while(Result::ok) {
            if let Some(reste) = ligne.strip_prefix("@etape ") {
                let (part, texte) = reste.split_once(' ').unwrap_or((reste, ""));
                let _ = app.emit("maj-avance", Avance { part: part.parse().unwrap_or(0), texte: texte.into() });
            } else if let Some(reste) = ligne.strip_prefix("@fin ") {
                let (verdict, texte) = reste.split_once(' ').unwrap_or((reste, ""));
                fin = Some(Fin { bien: verdict == "ok", texte: texte.into(), arme: false });
            } else {
                let _ = app.emit("maj-ligne", ligne);
            }
        }
    }
    let code = enfant.wait().map(|s| s.success()).unwrap_or(false);
    let mut fin = fin.unwrap_or(Fin {
        bien: false,
        texte: if code { "La préparation s'est arrêtée sans rien dire.".into() }
               else { "L'autorisation a été refusée, ou la préparation a échoué.".into() },
        arme: false,
    });
    fin.arme = arme();
    dire(&format!("maj : preparation {} — {}", if fin.bien { "ok" } else { "echec" }, fin.texte));
    fin
}

#[tauri::command]
pub fn maj_redemarrer() -> Result<(), String> {
    if reussit("systemctl", &["reboot"]) { Ok(()) } else { Err("Le redémarrage a été refusé.".into()) }
}
