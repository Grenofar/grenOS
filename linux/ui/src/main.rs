//! grenos-ui : l'interface de grenOS, en Rust et Tauri.
//!
//!     grenos-ui premier   l'assistant de première configuration (root, avant le bureau)
//!     grenos-ui maj       Mises à jour
//!     grenos-ui perf      grenPerf : mémoire, processeur, processus, mode jeu
//!     grenos-ui lance     GrenLaunch : tous les jeux au même endroit
//!
//! Chaque page est une fenêtre du MÊME processus (voir `systeme::instance`).
//! Ajouter une application, c'est ajouter une page dans `web/` et ses
//! commandes ici — pas un programme de plus à charger en mémoire.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod lance;
mod maj;
mod materiel;
mod perf;
mod steam_compte;
mod premier;
mod systeme;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const PAGES: &[(&str, &str, f64, f64)] = &[
    // page, titre, largeur, hauteur
    ("premier", "Bienvenue dans grenOS", 1280.0, 800.0),
    ("maj", "Mises à jour", 760.0, 620.0),
    ("perf", "grenPerf", 1080.0, 720.0),
    ("lance", "GrenLaunch", 1220.0, 800.0),
];

pub fn ouvrir(app: &AppHandle, page: &str) -> tauri::Result<()> {
    let Some(&(page, titre, largeur, hauteur)) = PAGES.iter().find(|p| p.0 == page) else {
        systeme::dire(&format!("grenos-ui : page inconnue « {page} »"));
        return Ok(());
    };
    if let Some(fenetre) = app.get_webview_window(page) {
        let _ = fenetre.unminimize();
        let _ = fenetre.set_focus();
        return Ok(());
    }
    let mut constructeur = WebviewWindowBuilder::new(app, page, WebviewUrl::App("index.html".into()))
        .title(titre)
        .initialization_script(&format!("window.GRENOS_PAGE = {page:?};"));

    if page == "premier" {
        // Aucun gestionnaire de fenêtres ne tourne avant le bureau : une
        // demande de plein écran n'a personne pour l'exaucer. On prend donc
        // nous-mêmes toute la surface de l'écran.
        constructeur = constructeur.decorations(false).resizable(false).fullscreen(true);
        if let Ok(Some(ecran)) = app.primary_monitor() {
            let taille = ecran.size().to_logical::<f64>(ecran.scale_factor());
            constructeur = constructeur
                .inner_size(taille.width, taille.height)
                .position(0.0, 0.0);
        } else {
            constructeur = constructeur.inner_size(largeur, hauteur).position(0.0, 0.0);
        }
    } else {
        constructeur = constructeur
            .inner_size(largeur, hauteur)
            .min_inner_size(480.0, 400.0)
            .center();
    }
    constructeur.build()?;
    Ok(())
}

#[tauri::command]
fn apparence() -> systeme::Apparence {
    systeme::apparence()
}

#[tauri::command]
fn regler_apparence(cle: String, valeur: String) -> Result<(), String> {
    let permis: &[&str] = match cle.as_str() {
        "theme" => &["auto", "nuit", "jour"],
        "accent" => &["bleu", "violet", "vert", "orange", "rose", "cyan"],
        "effets" => &["complets", "equilibres", "performance"],
        _ => return Err("réglage inconnu".into()),
    };
    if !permis.contains(&valeur.as_str()) {
        return Err("valeur refusée".into());
    }
    systeme::ecrire_reglage(&cle, &valeur)
}

#[tauri::command]
fn fermer(fenetre: tauri::WebviewWindow) {
    let _ = fenetre.close();
}

fn main() {
    let page = std::env::args().nth(1).unwrap_or_else(|| "maj".into());
    systeme::preparer_webkit();

    // L'assistant tourne seul, en root, avant toute session : pas de prise à
    // partager, et surtout aucune chance qu'une autre fenêtre lui vole l'écran.
    let seul = page == "premier";
    if !seul && systeme::instance::transmettre(&page) {
        return;
    }

    let page_de_depart = page.clone();
    tauri::Builder::default()
        .manage(maj::Etat::default())
        .manage(perf::Etat::default())
        .invoke_handler(tauri::generate_handler![
            apparence,
            regler_apparence,
            fermer,
            premier::premier_infos,
            premier::premier_dire,
            premier::premier_pret,
            premier::premier_nom,
            premier::premier_clavier,
            premier::premier_terminer,
            premier::premier_quitter,
            premier::reseau_etat,
            premier::reseau_liste,
            premier::reseau_connecter,
            maj::maj_etat,
            maj::maj_mode,
            maj::maj_verifier,
            maj::maj_preparer,
            maj::maj_redemarrer,
            perf::perf_instantane,
            perf::perf_fermer,
            perf::perf_priorite,
            perf::perf_liberer,
            perf::perf_demarrage,
            perf::perf_regler_demarrage,
            perf::perf_mode_jeu,
            lance::lance_vue,
            lance::lance_jouer,
            lance::lance_steam_installer,
            lance::lance_ouvrir,
            lance::lance_installer,
        ])
        .setup(move |app| {
            if !seul {
                systeme::instance::ecouter(app.handle().clone());
            }
            ouvrir(app.handle(), &page_de_depart)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("grenos-ui n'a pas pu démarrer");
}
