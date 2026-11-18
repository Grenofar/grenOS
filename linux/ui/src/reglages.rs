//! Les Réglages : tout ce qui se règle, au même endroit — dix pages.
//!
//! Repris de `grenos-parametres` (Python) le 18 novembre. Ce que la fenêtre
//! change, elle le change pour de vrai : la définition par RandR (ecran.py),
//! le son par PipeWire (son.py), l'heure par timedatectl (heure.py), le mot de
//! passe par `grenos-compte`. Ces modules restent : ils sont éprouvés, et le
//! pont (`pont.py`) les appelle.
//!
//! Les lignes série sont celles d'avant, au mot près — quatre sont EXIGÉES par
//! la construction : « reglages ouvert », « reglages securite : », « reglages
//! composants : », et « reglages son : melangeur ». Comme avant, toutes les
//! pages se remplissent dès l'ouverture (une par une), pas seulement celle
//! qu'on regarde : sinon ces preuves disparaîtraient.

use crate::pont;
use crate::systeme::{dire, lire, privilegie, sortie};
use serde_json::{json, Value};
use std::process::{Command, Stdio};

/// Les fonctions du pont que la page Réglages a le droit d'appeler.
const PERMISES: &[&str] = &[
    "reglages_ecran", "reglages_ecran_appliquer", "reglages_composants", "reglages_composants_chercher",
    "reglages_securite", "reglages_son", "reglages_son_regler", "reglages_heure", "reglages_heure_regler",
    "reglages_materiel", "reglages_maj",
];

pub const PAGES: usize = 10;

#[tauri::command]
pub fn reglages_ouvert() {
    dire(&format!("reglages ouvert, {PAGES} pages"));
}

/// Les lignes série que chaque lecture doit dire, comme l'ancienne fenêtre.
fn dire_pour(fonction: &str, v: &Value) {
    match fonction {
        "reglages_ecran" => {
            if let (Some(e), Some(modes)) = (v["ecran"].as_object(), v["modes"].as_array()) {
                let actuelle = e["actuelle"].as_str().unwrap_or("");
                let mode = modes.iter().find(|m| m["taille"] == actuelle).or(modes.first());
                let taux = mode.and_then(|m| m["taux"].as_array()).cloned().unwrap_or_default();
                let annoncees = taux.iter().filter(|t| t["annoncee"] == true).count();
                dire(&format!(
                    "reglages ecran : {}, {} definitions, {} frequences offertes dont {annoncees} annoncees",
                    if actuelle.is_empty() { "inconnue" } else { actuelle },
                    modes.len(),
                    taux.len()
                ));
            }
        }
        "reglages_composants" => {
            if v["present"] == true {
                dire(&format!("reglages composants : {} composant(s) lus par fwupd", v["composants"].as_array().map(|c| c.len()).unwrap_or(0)));
            }
        }
        "reglages_securite" => dire(&format!(
            "reglages securite : clamav {}, signatures {}",
            if v["present"] == true { "present" } else { "absent" },
            if v["signatures"] == true { "presentes" } else { "absentes" }
        )),
        "reglages_son" => {
            for l in v["lignes_serie"].as_array().into_iter().flatten() {
                if let Some(l) = l.as_str() {
                    dire(l);
                }
            }
        }
        _ => {}
    }
}

#[tauri::command]
pub async fn reglages(fonction: String, arguments: Option<Value>) -> Result<Value, String> {
    if !PERMISES.contains(&fonction.as_str()) {
        return Err("réglage inconnu".into());
    }
    let v = pont::appeler(&fonction, arguments.unwrap_or_else(|| json!({})))?;
    dire_pour(&fonction, &v);
    Ok(v)
}

/// La base de signatures de ClamAV : plusieurs centaines de Mo, pas dans
/// l'image, téléchargée ici et seulement à la demande.
#[tauri::command]
pub async fn reglages_signatures() -> Result<Value, String> {
    let p = privilegie();
    let mut arguments: Vec<&str> = vec!["900"];
    arguments.extend(p.iter().map(String::as_str));
    arguments.push("freshclam");
    let reussi = Command::new("timeout").args(&arguments).stdin(Stdio::null()).output().map(|o| o.status.success()).unwrap_or(false);
    let mut v = pont::appeler("reglages_securite", json!({}))?;
    if !reussi {
        v["souci"] = json!("Le téléchargement n'a pas abouti.");
    }
    Ok(v)
}

// ---- Réseau ------------------------------------------------------------------

#[tauri::command]
pub async fn reglages_reseau() -> String {
    let etat = sortie("nmcli", &["-t", "-f", "STATE", "general"]);
    match etat.trim() {
        "connected" => "connecté".into(),
        "disconnected" => "déconnecté".into(),
        "connecting" => "connexion en cours".into(),
        "" => "inconnu".into(),
        autre => autre.into(),
    }
}

// ---- Compte ------------------------------------------------------------------

const COMPTE: &str = "/usr/lib/grenos/grenos-compte";

fn login() -> String {
    crate::systeme::identite()["login"].clone()
}

#[tauri::command]
pub async fn reglages_compte() -> Value {
    // `passwd -S` lit la base des mots de passe : en root, sans rien demander
    // tant qu'aucun mot de passe n'existe. S'il refuse, c'est qu'il y en a un.
    let qui = login();
    let s = Command::new("sudo").args(["-n", "passwd", "-S", &qui]).output();
    let protege = match s {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).split_whitespace().nth(1) == Some("P"),
        _ => true,
    };
    let id = crate::systeme::identite();
    json!({ "protege": protege, "adresse": crate::systeme::adresse(), "nom": id["nom"] })
}

fn en_root(arguments: &[&str], entree: &str) -> Result<(), String> {
    use std::io::Write;
    let p = privilegie();
    let mut c = Command::new(&p[0]);
    c.args(&p[1..]).arg(COMPTE).args(arguments).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped());
    let mut enfant = c.spawn().map_err(|e| e.to_string())?;
    if let Some(mut e) = enfant.stdin.take() {
        let _ = e.write_all(entree.as_bytes());
    }
    let fin = enfant.wait_with_output().map_err(|e| e.to_string())?;
    if fin.status.success() {
        Ok(())
    } else {
        let t = String::from_utf8_lossy(&fin.stderr).trim().to_string();
        Err(if t.is_empty() { "Le système a refusé.".into() } else { t })
    }
}

#[tauri::command]
pub async fn reglages_mot_de_passe(premier: String, second: String) -> Result<String, String> {
    if premier.chars().count() < 4 {
        return Err("Quatre caractères au minimum.".into());
    }
    if premier != second {
        return Err("Les deux ne sont pas identiques.".into());
    }
    if premier.contains('\n') {
        return Err("Un mot de passe tient sur une ligne.".into());
    }
    en_root(&["definir", &login()], &format!("{premier}\n"))?;
    Ok("C'est fait. Il sera demandé à la prochaine ouverture.".into())
}

#[tauri::command]
pub async fn reglages_retirer_mot_de_passe() -> Result<String, String> {
    en_root(&["effacer", &login()], "")?;
    Ok("Retiré : la session s'ouvrira toute seule.".into())
}

// ---- Apparence -----------------------------------------------------------------

#[tauri::command]
pub fn reglages_theme(moment: String) -> Result<(), String> {
    if moment != "nuit" && moment != "jour" {
        return Err("thème inconnu".into());
    }
    crate::systeme::detacher("grenos-theme", &[&moment])
}

const IMAGES: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp"];

/// Les images de la personne, pour le fond d'écran : ses dossiers Images,
/// Bureau et Téléchargements (sans descendre), les plus récentes d'abord.
#[tauri::command]
pub async fn reglages_fonds() -> Vec<Value> {
    let maison = std::env::var("HOME").unwrap_or_default();
    let mut trouvees: Vec<(std::time::SystemTime, String, String)> = Vec::new();
    for dossier in ["Images", "Bureau", "Téléchargements"] {
        let Ok(entrees) = std::fs::read_dir(format!("{maison}/{dossier}")) else { continue };
        for e in entrees.flatten() {
            let chemin = e.path();
            let bonne = chemin.extension().and_then(|x| x.to_str()).map(|x| IMAGES.contains(&x.to_lowercase().as_str())).unwrap_or(false);
            if !bonne {
                continue;
            }
            let quand = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
            trouvees.push((quand, format!("{dossier}/{}", e.file_name().to_string_lossy()), chemin.to_string_lossy().into_owned()));
        }
    }
    trouvees.sort_by(|a, b| b.0.cmp(&a.0));
    let actuel = lire(&format!("{maison}/.config/grenos/fond")).trim().to_string();
    trouvees.into_iter().take(60).map(|(_, nom, chemin)| json!({ "nom": nom, "actuel": chemin == actuel, "chemin": chemin })).collect()
}

/// Le vrai sélecteur de fichiers de GTK (celui de l'ancienne fenêtre), ouvert
/// sur le dossier Images, filtré sur les images. Rend le chemin choisi, déjà
/// posé en fond, ou None si la personne a annulé. Commande synchrone : elle
/// tourne sur le fil principal, celui de GTK.
#[cfg(target_os = "linux")]
#[tauri::command]
pub fn reglages_parcourir_fond(fenetre: tauri::WebviewWindow) -> Result<Option<String>, String> {
    use gtk::prelude::*;
    let parent = fenetre.gtk_window().ok();
    let boite = gtk::FileChooserNative::new(Some("Choisir un fond d'écran"), parent.as_ref(), gtk::FileChooserAction::Open, Some("Choisir"), Some("Annuler"));
    let filtre = gtk::FileFilter::new();
    filtre.set_name(Some("Images"));
    for e in IMAGES {
        filtre.add_pattern(&format!("*.{e}"));
        filtre.add_pattern(&format!("*.{}", e.to_uppercase()));
    }
    boite.add_filter(filtre);
    let maison = std::env::var("HOME").unwrap_or_default();
    let images = std::path::Path::new(&maison).join("Images");
    let _ = boite.set_current_folder(if images.is_dir() { images } else { std::path::PathBuf::from(&maison) });
    let reponse = boite.run();
    let choisi = boite.filename();
    boite.destroy();
    if reponse != gtk::ResponseType::Accept {
        return Ok(None);
    }
    let Some(chemin) = choisi.map(|p| p.to_string_lossy().into_owned()) else { return Ok(None) };
    reglages_fond(chemin.clone())?;
    Ok(Some(chemin))
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub fn reglages_parcourir_fond(_fenetre: tauri::WebviewWindow) -> Result<Option<String>, String> {
    Err("Le sélecteur de fichiers n'existe que sous Linux.".into())
}

#[tauri::command]
pub fn reglages_fond(chemin: String) -> Result<(), String> {
    // Une image qui existe, n'importe où (le sélecteur peut montrer
    // /usr/share/backgrounds) — ou "" pour revenir au fond de grenOS.
    let p = std::path::Path::new(&chemin);
    let permis = chemin.is_empty()
        || (p.is_absolute()
            && !chemin.contains("/../")
            && p.extension().and_then(|x| x.to_str()).map(|x| IMAGES.contains(&x.to_lowercase().as_str())).unwrap_or(false)
            && p.is_file());
    if !permis {
        return Err("Cette image n'est pas utilisable.".into());
    }
    let maison = std::env::var("HOME").unwrap_or_default();
    let reglages = format!("{maison}/.config/grenos");
    std::fs::create_dir_all(&reglages).map_err(|e| e.to_string())?;
    let fichier = format!("{reglages}/fond");
    if chemin.is_empty() {
        let _ = std::fs::remove_file(&fichier);
    } else {
        std::fs::write(&fichier, format!("{chemin}\n")).map_err(|e| e.to_string())?;
    }
    crate::systeme::detacher("grenos-fond", &[])
}

// ---- F8 : la taille de l'affichage (DPI) ------------------------------------------

/// Les échelles proposées, en points par pouce : 100 % à 200 %. Rien entre
/// deux : une valeur bizarre (103 %) floute le texte de certains programmes.
const DPI: &[u32] = &[96, 108, 120, 144, 168, 192];

fn xresources() -> std::path::PathBuf {
    std::path::Path::new(&std::env::var("HOME").unwrap_or_default()).join(".Xresources")
}

pub(crate) fn dpi_de(texte: &str) -> u32 {
    texte
        .lines()
        .find_map(|l| l.trim().strip_prefix("Xft.dpi:").and_then(|v| v.trim().parse().ok()))
        .unwrap_or(96)
}

/// Remplace (ou ajoute) la ligne Xft.dpi, sans toucher au reste du fichier.
pub(crate) fn avec_dpi(texte: &str, dpi: u32) -> String {
    let mut lignes: Vec<String> = texte.lines().filter(|l| !l.trim().starts_with("Xft.dpi:")).map(String::from).collect();
    lignes.push(format!("Xft.dpi: {dpi}"));
    lignes.join("\n") + "\n"
}

#[tauri::command]
pub fn reglages_dpi_lire() -> u32 {
    dpi_de(&lire(&xresources().to_string_lossy()))
}

/// La taille de tout l'affichage. Écrite dans ~/.Xresources — que la session
/// relit à chaque ouverture (Xsession, 30x11-common_xresources) — et passée
/// tout de suite à `xrdb` : les fenêtres OUVERTES ENSUITE la prennent ; la
/// barre et le bureau, au prochain démarrage de session. La page le dit.
#[tauri::command]
pub fn reglages_dpi(dpi: u32) -> Result<String, String> {
    if !DPI.contains(&dpi) {
        return Err("Taille non proposée.".into());
    }
    let chemin = xresources();
    let neuf = avec_dpi(&lire(&chemin.to_string_lossy()), dpi);
    std::fs::write(&chemin, &neuf).map_err(|e| e.to_string())?;
    let _ = Command::new("xrdb").args(["-merge", &chemin.to_string_lossy()]).status();
    dire(&format!("reglages ecran : taille {} %", dpi * 100 / 96));
    Ok(format!("Taille {} %. Les fenêtres ouvertes à partir de maintenant la prennent ; la barre et le bureau, à la prochaine ouverture de session.", dpi * 100 / 96))
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn dpi_garde_le_reste() {
        assert_eq!(dpi_de(""), 96);
        assert_eq!(dpi_de("Xcursor.size: 24\nXft.dpi: 144\n"), 144);
        assert_eq!(avec_dpi("Xcursor.size: 24\nXft.dpi: 96\n", 120), "Xcursor.size: 24\nXft.dpi: 120\n");
        assert_eq!(avec_dpi("", 192), "Xft.dpi: 192\n");
    }

    #[test]
    fn seules_les_fonctions_des_reglages() {
        assert!(PERMISES.iter().all(|f| f.starts_with("reglages_")));
        assert!(!PERMISES.contains(&"hub_diagnostic"));
    }
}
