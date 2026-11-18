//! grenos-ui : l'interface de grenOS, en Rust et Tauri.
//!
//!     grenos-ui premier   l'assistant de première configuration (root, avant le bureau)
//!     grenos-ui maj       Mises à jour
//!     grenos-ui perf      grenPerf : mémoire, processeur, processus, mode jeu
//!     grenos-ui lance     GrenLaunch : tous les jeux au même endroit
//!     grenos-ui arret     Fin de session : veille, redémarrer, éteindre
//!
//! Chaque page est une fenêtre du MÊME processus (voir `systeme::instance`).
//! Ajouter une application, c'est ajouter une page dans `web/` et ses
//! commandes ici — pas un programme de plus à charger en mémoire.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod arret;
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
    ("arret", "Fin de session", 440.0, 560.0),
];

pub fn ouvrir(app: &AppHandle, page: &str) -> tauri::Result<()> {
    // « jeu » n'est pas une page : c'est grenPerf ouvert sur son onglet Mode
    // jeu (Ctrl+Alt+Échap, todo I8). Fenêtre déjà ouverte : on change d'onglet.
    let onglet = if page == "jeu" { Some("jeu") } else { None };
    let page = if page == "jeu" { "perf" } else { page };
    if let (Some(o), Some(fenetre)) = (onglet, app.get_webview_window(page)) {
        let _ = fenetre.eval(&format!("window.grenosOnglet && window.grenosOnglet({o:?})"));
    }
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
        .initialization_script(&format!(
            "window.GRENOS_PAGE = {page:?}; window.GRENOS_ONGLET = {:?};",
            onglet.unwrap_or("")
        ));

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
    } else if page == "arret" {
        // Un geste qu'on ne veut jamais déclencher par erreur : la fenêtre
        // reste au-dessus, au centre, et ne se redimensionne pas.
        constructeur = constructeur
            .inner_size(largeur, hauteur)
            .resizable(false)
            .always_on_top(true)
            .center();
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

/// Combien de temps le contenu s'efface avant que la fenêtre disparaisse.
/// Le même chiffre que `@keyframes fermeture` dans `style.css`.
const DUREE_FERMETURE_MS: u64 = 140;

/// Le délai de fermeture selon le réglage « effets » : aucun en performance.
fn delai_fermeture(effets: &str) -> Option<u64> {
    (effets != "performance").then_some(DUREE_FERMETURE_MS)
}

/// Fermer une fenêtre, en douceur (17 novembre).
///
/// Openbox détruirait la fenêtre d'un coup. On retient la fermeture, on dit
/// à la page de s'effacer, et on détruit la fenêtre une fois le fondu joué.
/// `destroy()` ne repasse pas par `CloseRequested` : pas de boucle. L'assistant
/// du premier démarrage n'a pas de bouton fermer, il n'est pas concerné.
fn fermeture_animee(fenetre: &tauri::Window, evenement: &tauri::WindowEvent) {
    let tauri::WindowEvent::CloseRequested { api, .. } = evenement else { return };
    if fenetre.label() == "premier" {
        return;
    }
    let Some(delai) = delai_fermeture(&systeme::apparence().effets) else { return };
    let Some(vue) = fenetre.app_handle().get_webview_window(fenetre.label()) else { return };
    api.prevent_close();
    let _ = vue.eval("document.documentElement.classList.add('fermeture')");
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(delai));
        let _ = vue.destroy();
    });
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
            arret::arret_vue,
            arret::arret_faire,
        ])
        .on_window_event(fermeture_animee)
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

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn la_fermeture_suit_le_reglage_effets() {
        assert_eq!(delai_fermeture("complets"), Some(DUREE_FERMETURE_MS));
        assert_eq!(delai_fermeture("equilibres"), Some(DUREE_FERMETURE_MS));
        assert_eq!(delai_fermeture("performance"), None);
    }

    #[test]
    fn la_duree_est_celle_de_la_feuille_de_style() {
        let css = include_str!("../web/style.css");
        let attendu = format!("fermeture {DUREE_FERMETURE_MS}ms");
        assert!(css.contains(&attendu), "style.css ne joue plus « {attendu} »");
    }
}
