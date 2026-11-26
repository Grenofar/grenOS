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

// ---------------------------------------------------------------------------
// LE GESTIONNAIRE EN QUATRE ONGLETS (23 novembre). Grenofar : « améliore le
// gestionnaire des tâches, catégories processus, performance et jeu, vider la
// RAM — inspire-toi de Perf Saver », puis « ajoute la possibilité d'avoir des
// apps au démarrage, et que ça affiche tout, à part les process de grenOS :
// on ne peut ni les supprimer, ni [les mettre] dans démarrer avec, ni [les
// arrêter] dans le gestionnaire des tâches ». Processus et Jeu reprennent les
// mesures de grenPerf (perf.rs, déjà faites à la manière de Perf Saver) ;
// ce qui suit ajoute ce qui lui manquait : arrêter un groupe qui n'est pas à
// soi, et un démarrage où l'on AJOUTE une application.

/// Arrêter tous les processus d'un groupe : les siens directement, ceux du
/// système en UNE seule demande d'autorisation (pas une par processus).
#[tauri::command]
pub async fn taches_arreter_groupe(pids: Vec<u32>) -> Result<usize, String> {
    let mut a_autoriser = Vec::new();
    for pid in pids.iter().copied() {
        let nom = fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default().trim().to_string();
        if pid <= 1 || crate::perf::protege(&nom) {
            return Err(format!("« {nom} » fait tourner grenOS : il ne s'arrête pas d'ici."));
        }
        if !reussit("kill", &["-TERM", &pid.to_string()]) && std::path::Path::new(&format!("/proc/{pid}")).exists() {
            a_autoriser.push(pid.to_string());
        }
    }
    if !a_autoriser.is_empty() {
        let mut args = vec!["kill", "-TERM"];
        args.extend(a_autoriser.iter().map(String::as_str));
        if !reussit("pkexec", &args) {
            return Err("Le système a refusé.".into());
        }
    }
    dire(&format!("taches : groupe arrete, {} processus", pids.len()));
    Ok(pids.len())
}

#[derive(Serialize)]
pub struct Application {
    id: String,
    nom: String,
    commentaire: String,
    icone: String,
}

fn maison() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into()))
}

/// Les dossiers d'applications, du plus général au plus personnel : le
/// dernier qui définit un nom de fichier a raison (spécification XDG).
fn dossiers_applications() -> Vec<std::path::PathBuf> {
    vec![
        "/usr/share/applications".into(),
        "/usr/local/share/applications".into(),
        "/var/lib/flatpak/exports/share/applications".into(),
        maison().join(".local/share/flatpak/exports/share/applications"),
        maison().join(".local/share/applications"),
    ]
}

pub(crate) fn champ(texte: &str, cle: &str) -> String {
    // La section [Desktop Entry] seulement : les « Actions » ont aussi un Name.
    let mut dans = false;
    for l in texte.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            dans = l == "[Desktop Entry]";
            continue;
        }
        if dans {
            if let Some(v) = l.strip_prefix(&format!("{cle}=")) {
                return v.trim().to_string();
            }
        }
    }
    String::new()
}

pub(crate) fn nom_de(texte: &str) -> String {
    let fr = champ(texte, "Name[fr]");
    if fr.is_empty() { champ(texte, "Name") } else { fr }
}

fn id_sain(id: &str) -> bool {
    id.ends_with(".desktop") && !id.contains('/') && !id.starts_with('.') && id.len() < 200
}

/// Ce que l'on peut faire démarrer avec la session : les applications du
/// menu, sans celles de grenOS (elles démarrent déjà quand il le faut).
#[tauri::command]
pub fn taches_applications() -> Vec<Application> {
    let mut vues: std::collections::BTreeMap<String, Application> = Default::default();
    for dossier in dossiers_applications() {
        let Ok(entrees) = fs::read_dir(&dossier) else { continue };
        for e in entrees.flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            if !id_sain(&id) {
                continue;
            }
            let texte = fs::read_to_string(e.path()).unwrap_or_default();
            let cache = ["NoDisplay", "Hidden"].iter().any(|c| champ(&texte, c) == "true");
            if cache || champ(&texte, "Type") != "Application" || de_grenos(&id, &texte) {
                vues.remove(&id);
                continue;
            }
            let nom = nom_de(&texte);
            if nom.is_empty() {
                continue;
            }
            let commentaire = { let c = champ(&texte, "Comment[fr]"); if c.is_empty() { champ(&texte, "Comment") } else { c } };
            vues.insert(id.clone(), Application { id, nom, commentaire, icone: champ(&texte, "Icon") });
        }
    }
    let mut liste: Vec<Application> = vues.into_values().collect();
    liste.sort_by(|a, b| a.nom.to_lowercase().cmp(&b.nom.to_lowercase()));
    liste
}

/// Ce qui appartient à grenOS lui-même : ses programmes, et ce que son
/// bureau a besoin de lancer (trousseau, autorisations, réseau, agents des
/// machines virtuelles). Rien de tout cela ne se retire, donc rien ne
/// s'affiche — la demande du 23 novembre.
fn de_grenos(id: &str, texte: &str) -> bool {
    const SYSTEME: &[&str] = &[
        "gnome-keyring", "polkit", "xdg-user-dirs", "picom", "nm-applet", "spice-vdagent",
        "vmware-user", "org.gnome.SettingsDaemon", "at-spi", "pulseaudio", "pipewire", "geoclue",
        "print-applet", "user-dirs-update", "xscreensaver", "light-locker", "blueman",
    ];
    let exec = champ(texte, "Exec");
    let programme = exec.split_whitespace().next().unwrap_or("").rsplit('/').next().unwrap_or("");
    id.starts_with("gren") || programme.starts_with("gren") || SYSTEME.iter().any(|s| id.starts_with(s))
}

#[derive(Serialize)]
pub struct Demarrage {
    id: String,
    nom: String,
    commentaire: String,
    icone: String,
    actif: bool,
    /// Ajoutée par la personne (son dossier seulement) : « Retirer » l'efface.
    retirable: bool,
}

const SYSTEME_AUTOSTART: &str = "/etc/xdg/autostart";

fn dossier_autostart() -> std::path::PathBuf {
    maison().join(".config/autostart")
}

/// Ce qui démarre avec la session, sans ce qui fait tourner grenOS. Le
/// dossier personnel a le dernier mot (spécification XDG Autostart).
#[tauri::command]
pub fn taches_demarrage() -> Vec<Demarrage> {
    let mut vues: std::collections::BTreeMap<String, Demarrage> = Default::default();
    for (dossier, perso) in [(std::path::PathBuf::from(SYSTEME_AUTOSTART), false), (dossier_autostart(), true)] {
        let Ok(entrees) = fs::read_dir(&dossier) else { continue };
        for e in entrees.flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            if !id_sain(&id) {
                continue;
            }
            let texte = fs::read_to_string(e.path()).unwrap_or_default();
            if de_grenos(&id, &texte) {
                continue;
            }
            let seulement = champ(&texte, "OnlyShowIn");
            let ailleurs = !seulement.is_empty() && !seulement.contains("grenOS");
            let pas_ici = champ(&texte, "NotShowIn").contains("grenOS");
            let actif = champ(&texte, "Hidden") != "true" && !ailleurs && !pas_ici
                && champ(&texte, "X-GNOME-Autostart-enabled") != "false";
            let nom = nom_de(&texte);
            let systeme_aussi = std::path::Path::new(SYSTEME_AUTOSTART).join(&id).exists();
            vues.insert(id.clone(), Demarrage {
                nom: if nom.is_empty() { id.trim_end_matches(".desktop").to_string() } else { nom },
                commentaire: { let c = champ(&texte, "Comment[fr]"); if c.is_empty() { champ(&texte, "Comment") } else { c } },
                icone: champ(&texte, "Icon"),
                actif,
                retirable: perso && !systeme_aussi,
                id,
            });
        }
    }
    let mut liste: Vec<Demarrage> = vues.into_values().collect();
    liste.sort_by(|a, b| a.nom.to_lowercase().cmp(&b.nom.to_lowercase()));
    liste
}

/// Faire démarrer une application avec la session : sa fiche du menu,
/// copiée dans ~/.config/autostart.
#[tauri::command]
pub fn taches_demarrage_ajouter(id: String) -> Result<(), String> {
    if !id_sain(&id) {
        return Err("nom refusé".into());
    }
    let source = dossiers_applications().into_iter().rev().map(|d| d.join(&id)).find(|p| p.is_file())
        .ok_or("Cette application n'est plus installée.")?;
    let texte = fs::read_to_string(&source).map_err(|e| e.to_string())?;
    if de_grenos(&id, &texte) {
        return Err("grenOS démarre déjà ce qu'il lui faut.".into());
    }
    // Le texte tel quel, sans ce qui empêcherait le démarrage.
    let lignes: Vec<&str> = texte.lines()
        .filter(|l| !l.starts_with("Hidden=") && !l.starts_with("X-GNOME-Autostart-enabled=") && !l.starts_with("NoDisplay="))
        .collect();
    let dossier = dossier_autostart();
    fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    fs::write(dossier.join(&id), lignes.join("\n") + "\n").map_err(|e| e.to_string())?;
    dire(&format!("taches : {id} demarre avec la session"));
    Ok(())
}

#[tauri::command]
pub fn taches_demarrage_retirer(id: String) -> Result<(), String> {
    if !id_sain(&id) {
        return Err("nom refusé".into());
    }
    if std::path::Path::new(SYSTEME_AUTOSTART).join(&id).exists() {
        return Err("Celle-ci vient du système : coupe-la plutôt.".into());
    }
    let _ = fs::remove_file(dossier_autostart().join(&id));
    dire(&format!("taches : {id} retire du demarrage"));
    Ok(())
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn les_fiches_de_grenos_ne_se_montrent_pas() {
        assert!(de_grenos("grenos-presse.desktop", "[Desktop Entry]\nExec=grenos-presse\n"));
        assert!(de_grenos("polkit-mate-authentication-agent-1.desktop", "[Desktop Entry]\nExec=/usr/libexec/polkit-mate\n"));
        assert!(de_grenos("x.desktop", "[Desktop Entry]\nExec=/usr/bin/grenos-ui taches\n"));
        assert!(!de_grenos("discord.desktop", "[Desktop Entry]\nExec=/usr/bin/discord\n"));
        assert!(!de_grenos("firefox-esr.desktop", "[Desktop Entry]\nExec=/usr/lib/firefox-esr/firefox-esr %u\n"));
    }

    #[test]
    fn un_nom_de_fiche_ne_sort_pas_du_dossier() {
        assert!(id_sain("discord.desktop"));
        assert!(!id_sain("../x.desktop"));
        assert!(!id_sain(".cache.desktop"));
        assert!(!id_sain("discord"));
    }

    #[test]
    fn le_nom_vient_de_la_bonne_section() {
        let t = "[Desktop Entry]\nName=Firefox\nName[fr]=Navigateur\n[Desktop Action new]\nName=Nouvelle fenêtre\n";
        assert_eq!(nom_de(t), "Navigateur");
        assert_eq!(champ("[Desktop Action x]\nName=Non\n", "Name"), "");
    }

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
