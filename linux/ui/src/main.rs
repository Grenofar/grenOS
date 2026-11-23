//! grenos-ui : l'interface de grenOS, en Rust et Tauri.
//!
//!     grenos-ui premier   l'assistant de première configuration (root, avant le bureau)
//!     grenos-ui maj       Mises à jour
//!     grenos-ui perf      grenPerf : mémoire, processeur, processus, mode jeu
//!     grenos-ui lance     GrenLaunch : tous les jeux au même endroit
//!     grenos-ui arret     Fin de session : veille, redémarrer, éteindre
//!     grenos-ui installation  Installer grenOS depuis la clé (partitions, copie)
//!
//! Chaque page est une fenêtre du MÊME processus (voir `systeme::instance`).
//! Ajouter une application, c'est ajouter une page dans `web/` et ses
//! commandes ici — pas un programme de plus à charger en mémoire.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aleatoire;
mod arret;
mod recuperation;
mod grenai;
mod agenda;
mod horloge;
mod fichiers;
mod reglages;
mod jeux;
mod grenplace;
mod hub;
mod taches;
mod connexions;
mod secours;
mod bienvenue;
mod lance;
mod maj;
mod materiel;
mod installation;
mod installer;
mod perf;
mod pont;
mod steam_compte;
mod premier;
mod preparer;
mod systeme;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// La hauteur de notre barre et d'une barre de titre d'Openbox : les mêmes
/// nombres que `grenosui.py` (BARRE, TITRE), pour juger le 720p pareil.
// La place du dock en bas, air compris (64 + 8) : le dock est la disposition
// par défaut depuis le 19 novembre. La barre n'en prend que 60 ; on garde la
// plus grande, pour qu'aucune fenêtre ne passe sous l'une ou l'autre.
const BARRE: f64 = 72.0;
const TITRE: f64 = 30.0;

/// Quand chaque fenêtre a été demandée : pour dire « ouverture : X en N ms ».
/// La première du processus compte depuis la naissance du processus, comme
/// le faisait Python ; les suivantes depuis la demande reçue par la prise.
static DEMANDES: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
static NAISSANCE: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Vrai une fois la première fenêtre du processus demandée.
static PREMIERE_FAITE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn noter_demande(page: &str) {
    // La toute première fenêtre compte depuis la naissance du processus (le
    // chargement de grenos-ui fait partie de son attente) ; les suivantes,
    // depuis la demande. L'ancien test (« aucune fenêtre en attente ») donnait
    // aux suivantes l'âge du processus : la CI du 18 novembre lisait
    // « Réglages en 625732 ms » pour une fenêtre ouverte en une fraction de
    // seconde.
    let premiere = !PREMIERE_FAITE.swap(true, std::sync::atomic::Ordering::SeqCst);
    let debut = if premiere { *NAISSANCE.get_or_init(Instant::now) } else { Instant::now() };
    DEMANDES.lock().unwrap().get_or_insert_with(HashMap::new).insert(page.to_string(), debut);
}

/// La taille voulue, ramenée à ce que l'écran peut montrer sous la barre
/// (Grenofar en 1280×720 : « on voit pas tout »). On ne rétrécit que ce qui
/// déborde. Renvoie aussi la zone utile, pour la juger après l'affichage.
fn taille_tenable(largeur: f64, hauteur: f64, ecran: Option<(f64, f64)>) -> (f64, f64) {
    let Some((l, h)) = ecran else { return (largeur, hauteur) };
    let utile_h = (h - BARRE).max(240.0);
    (largeur.min(l - 16.0).max(320.0), hauteur.min(utile_h - TITRE).max(240.0))
}

fn ecran_logique(app: &AppHandle) -> Option<(f64, f64)> {
    let ecran = app.primary_monitor().ok()??;
    let t = ecran.size().to_logical::<f64>(ecran.scale_factor());
    Some((t.width, t.height))
}

/// Appelée par chaque page une fois dessinée (`grenos.pret()`, commun.js) :
/// remonter la fenêtre si son bas passe sous la barre, puis DIRE sa taille
/// réelle et son temps d'ouverture — les mêmes lignes que les fenêtres
/// Python, que la CI lit (« fenetre … : tient », « ouverture : … »).
//
// ASYNCHRONE, donc hors du fil principal (18 novembre). Synchrone, elle
// tournait SUR le fil principal, et y déplacer la fenêtre (set_position)
// attend ce même fil : seules les trois fenêtres qui n'avaient pas besoin
// d'être remontées ont dit leur taille à la CI, les six autres se sont tues.
// Et la ligne est dite AVANT le déplacement : même si celui-ci échouait, la
// mesure arrive.
#[tauri::command]
async fn fenetre_prete(fenetre: tauri::WebviewWindow) {
    let page = fenetre.label().to_string();
    // La fenêtre est née cachée (voir `ouvrir`) : sa page est dessinée, on la
    // montre — après l'avoir remontée au besoin, pour qu'elle n'apparaisse
    // pas à un endroit puis saute à un autre.
    let montrer = |f: &tauri::WebviewWindow| {
        if !f.is_visible().unwrap_or(true) {
            let _ = f.show();
            let _ = f.set_focus();
        }
    };
    let titre = fenetre.title().unwrap_or_else(|_| page.clone());
    if let Some(debut) = DEMANDES.lock().unwrap().as_mut().and_then(|c| c.remove(&page)) {
        systeme::dire(&format!("ouverture : {titre} en {} ms (grenos-ui)", debut.elapsed().as_millis()));
    } else {
        // Déjà dite (une page qui se redessine ne se rechronomètre pas), ou
        // une fenêtre sans chrono (une 2e fenêtre de l'explorateur) : on la
        // montre quand même, sans attendre le filet.
        montrer(&fenetre);
        return;
    }
    if page == "premier" {
        return;
    }
    let echelle = fenetre.scale_factor().unwrap_or(1.0);
    let (Ok(taille), Ok(pos)) = (fenetre.inner_size(), fenetre.inner_position()) else {
        montrer(&fenetre);
        return;
    };
    let taille = taille.to_logical::<f64>(echelle);
    let pos = pos.to_logical::<f64>(echelle);
    let (el, eh) = ecran_logique(fenetre.app_handle()).unwrap_or((taille.width, taille.height + BARRE));
    let utile_h = (eh - BARRE).max(240.0);
    let bouger = pos.y + taille.height > utile_h;
    let y = if bouger { TITRE.max(utile_h - taille.height) } else { pos.y };
    let tient = taille.width <= el && y + taille.height <= utile_h;
    systeme::dire(&format!(
        "fenetre {titre} {}x{} en {},{} dans {}x{} : {}",
        taille.width as i64, taille.height as i64, pos.x as i64, y as i64,
        el as i64, utile_h as i64, if tient { "tient" } else { "DEBORDE" }
    ));
    if bouger {
        let _ = fenetre.set_position(tauri::LogicalPosition::new(pos.x, y));
    }
    montrer(&fenetre);
}

const PAGES: &[(&str, &str, f64, f64)] = &[
    // page, titre, largeur, hauteur
    ("premier", "Bienvenue dans grenOS", 1280.0, 800.0),
    ("maj", "Mises à jour", 760.0, 620.0),
    ("perf", "grenPerf", 1080.0, 720.0),
    ("lance", "GrenLaunch", 1220.0, 800.0),
    ("arret", "Fin de session", 440.0, 560.0),
    ("recuperation", "grenRecovery", 880.0, 660.0),
    ("grenai", "GrenAI", 860.0, 680.0),
    ("agenda", "Agenda", 1000.0, 680.0),
    ("horloge", "Horloge", 760.0, 640.0),
    ("fichiers", "Explorateur de fichiers", 1000.0, 640.0),
    ("reglages", "Réglages de grenOS", 960.0, 660.0),
    ("jeux", "Jeux", 900.0, 720.0),
    ("grenplace", "GrenPlace", 980.0, 720.0),
    ("hub", "grenHub", 900.0, 660.0),
    ("taches", "Gestionnaire de tâches", 1000.0, 700.0),
    ("preparer", "Préparer le matériel", 820.0, 680.0),
    ("connexions", "Connexions", 560.0, 640.0),
    ("secours", "Mode sans échec de grenOS", 720.0, 560.0),
    ("bienvenue", "Bienvenue dans grenOS", 940.0, 700.0),
    ("installation", "Installer grenOS", 1040.0, 700.0),
];

pub fn ouvrir(app: &AppHandle, page: &str) -> tauri::Result<()> {
    // Le résident de la session est arrivé après nous : on le devient.
    if page == "--resident" {
        if !RESIDENT.swap(true, std::sync::atomic::Ordering::SeqCst) {
            systeme::dire("grenos-ui : devenu resident, les applications s ouvriront chaudes");
        }
        return Ok(());
    }
    // « jeu » n'est pas une page : c'est grenPerf ouvert sur son onglet Mode
    // jeu (Ctrl+Alt+Échap, todo I8). Et « reglages:son » ouvre les Réglages
    // sur leur page Son (`grenos-parametres son`). Fenêtre déjà ouverte : on
    // change d'onglet.
    let (page, onglet) = match page.split_once(':') {
        _ if page == "jeu" => ("perf", Some("jeu")),
        Some((p, o)) if !o.is_empty() => (p, Some(o)),
        _ => (page, None),
    };
    // L'explorateur ouvre AUTANT de fenêtres qu'on lui demande (Ctrl+N, un
    // dossier depuis le bureau) : une étiquette neuve à chaque fois. Les
    // autres pages n'en ont qu'une, qu'on ramène devant.
    let plusieurs = page == "fichiers";
    if let (false, Some(o), Some(fenetre)) = (plusieurs, onglet, app.get_webview_window(page)) {
        let o = serde_json::to_string(o).unwrap_or_default();
        let _ = fenetre.eval(&format!("window.grenosOnglet && window.grenosOnglet({o})"));
    }
    let Some(&(page, titre, largeur, hauteur)) = PAGES.iter().find(|p| p.0 == page) else {
        systeme::dire(&format!("grenos-ui : page inconnue « {page} »"));
        return Ok(());
    };
    if let (false, Some(fenetre)) = (plusieurs, app.get_webview_window(page)) {
        let _ = fenetre.unminimize();
        let _ = fenetre.set_focus();
        return Ok(());
    }
    let etiquette = if plusieurs {
        (1..).map(|n| format!("{page}-{n}")).find(|e| app.get_webview_window(e).is_none()).unwrap_or_default()
    } else {
        page.to_string()
    };
    noter_demande(&etiquette);
    let (largeur, hauteur) = if page == "premier" { (largeur, hauteur) } else { taille_tenable(largeur, hauteur, ecran_logique(app)) };
    let mut constructeur = WebviewWindowBuilder::new(app, &etiquette, WebviewUrl::App("index.html".into()))
        .title(titre)
        .initialization_script(&format!(
            "window.GRENOS_PAGE = {}; window.GRENOS_ONGLET = {};",
            serde_json::to_string(page).unwrap_or_default(),
            serde_json::to_string(onglet.unwrap_or("")).unwrap_or_default()
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
        // PAS cachée, elle (19 novembre) : née cachée puis montrée, cette
        // fenêtre fixe et toujours au-dessus ressortait étirée à toute la
        // hauteur utile (440x660 au lieu de 440x560, « DEBORDE » en 720p,
        // run 44396230022). Elle est petite et s'ouvre vite : on la garde
        // visible dès sa création, comme avant.
    } else {
        constructeur = constructeur
            .inner_size(largeur, hauteur)
            .min_inner_size(480.0, 400.0)
            .center()
            .visible(false);
    }
    let fenetre = constructeur.build()?;
    // OUVRIR SANS CADRE VIDE (19 novembre) : « rend plus smooth les ouvertures
    // d'app ». La fenêtre naît cachée et se montre quand sa page est dessinée
    // (`fenetre_prete`) ; on ne voit plus un cadre vide 200 ms avant le
    // contenu. LE FILET : une page qui ne dirait jamais « prête » (erreur JS)
    // laisserait une application invisible — elle se montre seule à 1,5 s.
    if page != "premier" && page != "arret" {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            if !fenetre.is_visible().unwrap_or(true) {
                systeme::dire(&format!("ouverture : {} montree par le filet (la page n a rien dit en 1,5 s)", fenetre.label()));
                let _ = fenetre.show();
                let _ = fenetre.set_focus();
            }
        });
    }
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
    // AVANT le fondu : `destroy()` passerait outre (22 novembre).
    if installation::retient(fenetre.label(), installation::en_cours().is_some()) {
        api.prevent_close();
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
    NAISSANCE.get_or_init(Instant::now);
    let page = std::env::args().nth(1).unwrap_or_else(|| "maj".into());

    // `grenos-ui --grenai` : une question sur l'entrée standard, la réponse
    // sur la sortie — pour `gren ia` et `gren corrige` (todo H4). La MÊME
    // fonction que la fenêtre GrenAI : mêmes clés, même modèle, et aucune
    // deuxième façon d'appeler un fournisseur qui finirait par diverger.
    if page == "--grenai" {
        use std::io::Read;
        let mut question = String::new();
        let _ = std::io::stdin().read_to_string(&mut question);
        let messages = vec![grenai::Message { role: "user".into(), contenu: question }];
        match tauri::async_runtime::block_on(grenai::grenai_demander(messages)) {
            Ok(reponse) => {
                println!("{reponse}");
                return;
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    systeme::preparer_webkit();

    // LES APPLICATIONS S'OUVRENT CHAUDES (20 novembre). Grenofar : « toutes
    // les apps qui s'ouvrent beaucoup plus vite que ça ». Le processus unique
    // s'arrêtait avec sa dernière fenêtre : chaque ouverture suivante
    // rechargeait le moteur web (bibliothèques, processus réseau) — une à
    // trois secondes, davantage depuis la clé USB. `grenos-ui --resident`,
    // lancé par la session une fois le bureau ouvert, garde ce processus
    // vivant sans fenêtre : les applications naissent dedans.
    let resident = page == "--resident";

    // L'assistant tourne seul, en root, avant toute session : pas de prise à
    // partager, et surtout aucune chance qu'une autre fenêtre lui vole l'écran.
    let seul = page == "premier";
    if resident {
        // Une fenêtre déjà ouverte (l'accueil part 2 s après la session) :
        // c'est SON processus qui devient résident, on ne double rien.
        if systeme::instance::transmettre("--resident") {
            return;
        }
        RESIDENT.store(true, std::sync::atomic::Ordering::SeqCst);
        // Ses fenêtres se chronomètrent depuis leur demande, pas depuis la
        // naissance du résident (qui peut dater d'une heure).
        PREMIERE_FAITE.store(true, std::sync::atomic::Ordering::SeqCst);
    } else if !seul && systeme::instance::transmettre(&page) {
        return;
    }

    let page_de_depart = page.clone();
    tauri::Builder::default()
        .manage(maj::Etat::default())
        .manage(perf::Etat::default())
        .manage(taches::Etat::default())
        .invoke_handler(tauri::generate_handler![
            apparence,
            regler_apparence,
            fermer,
            fenetre_prete,
            systeme::ouvrir_programme,
            installer::installer,
            installation::installation_infos,
            installation::installation_nom,
            installation::installation_lire,
            installation::installation_lancer,
            installation::installation_redemarrer,
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
            recuperation::recuperation_lister,
            recuperation::recuperation_faire,
            recuperation::recuperation_instantanes,
            grenai::grenai_reglages,
            grenai::grenai_regler,
            grenai::grenai_demander,
            grenai::grenai_lire_document,
            grenai::grenai_claude_code,
            grenai::grenai_mcp_etat,
            grenai::grenai_mcp,
            agenda::agenda_lire,
            agenda::agenda_poser,
            agenda::agenda_retirer,
            horloge::horloge_sonner,
            fichiers::fichiers_lister,
            fichiers::fichiers_rapides,
            fichiers::fichiers_ce_pc,
            fichiers::fichiers_monter,
            fichiers::fichiers_ouvrir,
            fichiers::fichiers_viser,
            fichiers::fichiers_nouveau_dossier,
            fichiers::fichiers_renommer,
            fichiers::fichiers_coller,
            fichiers::fichiers_corbeille,
            fichiers::fichiers_archive,
            fichiers::fichiers_proprietes,
            fichiers::fichiers_dire,
            fichiers::fichiers_titre,
            fichiers::fichiers_nouvelle_fenetre,
            fichiers::fichiers_maison,
            fichiers::fichiers_apercu,
            fichiers::fichiers_chiffrer,
            fichiers::fichiers_dechiffrer,
            fichiers::fichiers_ocr,
            reglages::reglages_ouvert,
            reglages::reglages,
            reglages::reglages_signatures,
            reglages::reglages_partitions_agir,
            reglages::reglages_reseau,
            reglages::reglages_compte,
            reglages::reglages_mot_de_passe,
            reglages::reglages_retirer_mot_de_passe,
            reglages::reglages_theme,
            reglages::reglages_fonds,
            reglages::reglages_fond,
            reglages::reglages_parcourir_fond,
            reglages::reglages_dpi_lire,
            reglages::reglages_barre_lire,
            reglages::reglages_dispositions_lire,
            reglages::reglages_dispositions,
            reglages::reglages_cle_etat,
            reglages::reglages_cle_ajouter,
            reglages::reglages_cle_retirer,
            reglages::reglages_barre,
            reglages::reglages_dpi,
            jeux::jeux_vue,
            jeux::jeux_paquet_nvidia,
            jeux::jeux_preparer_rocket,
            jeux::jeux_poser_fortnite,
            jeux::jeux_ouvrir,
            grenplace::grenplace_catalogue,
            grenplace::grenplace_retards,
            grenplace::grenplace_installe,
            hub::hub_etat,
            hub::hub_diagnostic,
            taches::taches_mesure,
            taches::taches_arreter,
            taches::taches_arreter_groupe,
            taches::taches_applications,
            taches::taches_demarrage,
            taches::taches_demarrage_ajouter,
            taches::taches_demarrage_retirer,
            preparer::preparer_etat,
            preparer::preparer_fini,
            preparer::preparer_redemarrer,
            connexions::connexions_etat,
            connexions::connexions_wifi_liste,
            connexions::connexions_wifi_radio,
            connexions::connexions_wifi_connu,
            connexions::connexions_wifi_connecter,
            connexions::connexions_wifi_deconnecter,
            connexions::connexions_bt_liste,
            connexions::connexions_bt_radio,
            connexions::connexions_bt_chercher,
            connexions::connexions_bt_lier,
            connexions::connexions_bt_delier,
            secours::secours_vue,
            secours::secours_faire,
            bienvenue::bienvenue_vue,
        ])
        .on_window_event(fermeture_animee)
        .setup(move |app| {
            if !seul {
                systeme::instance::ecouter(app.handle().clone());
            }
            if resident {
                prechauffer(app.handle());
                systeme::dire("grenos-ui : resident pret, les applications s ouvriront chaudes");
            } else {
                ouvrir(app.handle(), &page_de_depart)?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("grenos-ui n'a pas pu démarrer")
        .run(move |_app, evenement| {
            // La dernière fenêtre fermée par la personne (code absent) ne
            // termine pas le résident ; un `exit()` demandé, si.
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = evenement {
                if RESIDENT.load(std::sync::atomic::Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
}

/// Vrai quand ce processus doit survivre à sa dernière fenêtre : lancé par
/// `--resident`, ou devenu résident à sa demande (voir `ouvrir`).
static RESIDENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Le moteur web chargé UNE fois, au repos : une vue invisible sur une page
/// vide lève le processus réseau et met les bibliothèques en mémoire, puis
/// disparaît. La première application ouverte ensuite n'attend plus rien de
/// tout cela. Jamais montrée, jamais dans la barre.
fn prechauffer(app: &AppHandle) {
    let Ok(adresse) = "about:blank".parse() else { return };
    let vue = WebviewWindowBuilder::new(app, "prechauffe", WebviewUrl::External(adresse))
        .visible(false)
        .skip_taskbar(true)
        .inner_size(64.0, 64.0)
        .build();
    if let Ok(vue) = vue {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(4));
            let _ = vue.destroy();
        });
    }
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn la_taille_tient_en_720p() {
        // 1280×720 : 648 au-dessus du dock (64 + 8 d'air), 618 sans la barre
        // de titre.
        assert_eq!(taille_tenable(1220.0, 800.0, Some((1280.0, 720.0))), (1220.0, 618.0));
        assert_eq!(taille_tenable(1400.0, 600.0, Some((1280.0, 720.0))), (1264.0, 600.0));
        assert_eq!(taille_tenable(760.0, 620.0, Some((1920.0, 1080.0))), (760.0, 620.0));
        assert_eq!(taille_tenable(760.0, 620.0, None), (760.0, 620.0));
    }

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
