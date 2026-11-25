//! grenPerf — la mémoire, le processeur, et ce qui les mange.
//!
//! Les idées de Perf Saver, portées sur Linux : les processus REGROUPÉS par
//! application (vingt processus « firefox-esr » sont une seule chose pour la
//! personne), les fermer ou changer leur priorité d'un clic, libérer la
//! mémoire, choisir ce qui démarre avec la session, et un mode jeu RÉVERSIBLE.
//!
//! Trois règles, qui sont celles de grenOS :
//! * on lit /proc directement, sans lancer `ps` toutes les deux secondes :
//!   un moniteur qui coûte cher ment sur ce qu'il mesure ;
//! * on ne touche qu'aux processus de la personne. Ceux du système sont
//!   montés en lecture seule, avec ce qu'ils coûtent ;
//! * une liste PROTÉGÉE : la barre, le bureau, le son, le serveur graphique.
//!   Les fermer d'un clic laisserait un écran noir — ce que Perf Saver fait
//!   avec sa liste SYSTEM, pour les mêmes raisons.

use crate::systeme::{dire, lire, privilegie};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

/// Jamais fermés ni ralentis depuis grenPerf.
pub(crate) const PROTEGES: &[&str] = &[
    "systemd", "Xorg", "X", "xinit", "openbox", "lightdm", "dbus-daemon", "dbus-broker",
    "pipewire", "pipewire-pulse", "wireplumber", "pulseaudio", "grenos-shell", "grenos-bureau",
    "grenos-veilleur", "grenos-session", "grenos-ui", "WebKitWebProcess", "WebKitNetworkProcess",
    "at-spi-bus-launcher", "at-spi2-registryd", "gvfsd", "xdg-desktop-portal",
    "xdg-desktop-portal-gtk", "xdg-document-portal", "xdg-permission-store", "polkit-gnome-authentication-agent-1",
    "sh", "bash", "earlyoom", "gamemoded", "lxpolkit", "ssh-agent", "gpg-agent",
    // Ce que la session de grenOS lance pour fonctionner (23 novembre : le
    // gestionnaire des tâches les montrait — « à part les process de grenOS,
    // on ne peut ni les supprimer ni les arrêter »).
    "gnome-keyring-daemon", "polkit-mate-authentication-agent-1", "xcape", "dunst",
    "notification-daemon", "xfce4-notifyd", "nm-applet", "dconf-service", "gvfsd-fuse",
    "gvfsd-metadata", "gvfs-udisks2-volume-monitor", "at-spi2-registr", "xdg-desktop-portal-gnome",
    "picom", "Xwayland", "Xorg.wrap", "agetty", "login", "plymouthd",
];

/// Ce qui exécute un script : c'est le script qu'il faut nommer. Sans cela,
/// les programmes Python de grenOS (grenos-mcp, grenos-capture…) passaient
/// pour un « Programme Python » anonyme, et n'étaient pas reconnus.
const INTERPRETES_PYTHON: &[&str] = &["python", "python3", "python3.13"];

/// Inutile pendant une partie : proposé (et coché) dans le mode jeu.
/// Chaque entrée dit ce que c'est — un nom de processus ne parle à personne.
const EN_TROP_POUR_JOUER: &[(&str, &str)] = &[
    ("tracker-miner-fs-3", "Indexation des fichiers"),
    ("tracker-extract-3", "Indexation des fichiers"),
    ("localsearch-3", "Indexation des fichiers"),
    ("baloo_file", "Indexation des fichiers"),
    ("evolution-source-registry", "Agenda et contacts en arrière-plan"),
    ("evolution-calendar-factory", "Agenda et contacts en arrière-plan"),
    ("evolution-addressbook-factory", "Agenda et contacts en arrière-plan"),
    ("zeitgeist-daemon", "Journal d'activité"),
    ("gnome-software", "Logithèque en arrière-plan"),
    ("update-notifier", "Notificateur de mises à jour"),
    ("clamscan", "Analyse antivirus en cours"),
    ("thunderbird", "Courrier"),
    ("libreoffice", "Bureautique ouverte"),
    ("soffice.bin", "Bureautique ouverte"),
];

#[derive(Default)]
pub struct Etat {
    precedent: Mutex<Option<Releve>>,
}

struct Releve {
    total_cpu: u64,
    par_pid: HashMap<u32, u64>,
}

#[derive(Serialize, Clone)]
pub struct Groupe {
    nom: String,
    description: String,
    pids: Vec<u32>,
    nombre: usize,
    memoire_mo: f64,
    cpu: f64,
    a_moi: bool,
    protege: bool,
    en_trop: bool,
    priorite: i32,
}

#[derive(Serialize)]
pub struct Instantane {
    coeurs: usize,
    cpu: f64,
    memoire_totale_go: f64,
    memoire_dispo_go: f64,
    memoire_cache_go: f64,
    echange_total_go: f64,
    echange_utilise_go: f64,
    charge: String,
    groupes: Vec<Groupe>,
    mode_jeu: bool,
    /// La carte graphique (25 novembre) : None si on ne sait pas la lire.
    carte: Option<Carte>,
}

/// LA CARTE GRAPHIQUE DANS LE GESTIONNAIRE (25 novembre). Grenofar :
/// « ajoute la quantité de VRAM et de 3D utilisée dans Ctrl+Maj+Échap »,
/// comme l'onglet Performance de Windows. « 3D » : l'occupation du moteur
/// graphique, en pourcentage ; « VRAM » : la mémoire de la carte.
///
/// NVIDIA (pilote officiel) : `nvidia-smi`, 40 ms sur une RTX 5060, sous
/// `timeout` — un pilote bloqué ne doit pas figer la fenêtre. AMD : le
/// noyau le dit dans /sys (gpu_busy_percent, mem_info_vram_*). Ailleurs
/// (Intel, `nouveau`), aucun chiffre honnête n'est disponible : rien ne
/// s'affiche plutôt qu'un zéro faux.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Carte {
    nom: String,
    trois_d: f64,
    vram_utilisee_go: f64,
    vram_totale_go: f64,
}

fn carte_graphique() -> Option<Carte> {
    carte_nvidia().or_else(carte_amd)
}

fn carte_nvidia() -> Option<Carte> {
    let sortie = Command::new("timeout")
        .args(["2", "nvidia-smi", "--query-gpu=name,utilization.gpu,memory.used,memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    if !sortie.status.success() {
        return None;
    }
    lire_nvidia(&String::from_utf8_lossy(&sortie.stdout))
}

/// « NVIDIA GeForce RTX 5060, 12, 2053, 8151 » : nom, 3D en %, VRAM utilisée
/// et totale en Mio. La première carte seulement.
fn lire_nvidia(texte: &str) -> Option<Carte> {
    let ligne = texte.lines().next()?;
    let champs: Vec<&str> = ligne.split(',').map(str::trim).collect();
    if champs.len() < 4 {
        return None;
    }
    let nombre = |t: &str| t.parse::<f64>().ok();
    Some(Carte {
        nom: champs[0].to_string(),
        trois_d: nombre(champs[1])?,
        vram_utilisee_go: nombre(champs[2])? / 1024.0,
        vram_totale_go: nombre(champs[3])? / 1024.0,
    })
}

fn carte_amd() -> Option<Carte> {
    let mut cartes: Vec<PathBuf> = fs::read_dir("/sys/class/drm").ok()?.flatten().map(|e| e.path()).collect();
    cartes.sort();
    for carte in cartes {
        let nom = carte.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // « card0 », pas « card0-DP-1 » (un connecteur).
        if !nom.starts_with("card") || nom.contains('-') {
            continue;
        }
        let d = carte.join("device");
        let lire_nombre = |f: &str| fs::read_to_string(d.join(f)).ok()?.trim().parse::<f64>().ok();
        let (Some(occupe), Some(utilisee), Some(totale)) =
            (lire_nombre("gpu_busy_percent"), lire_nombre("mem_info_vram_used"), lire_nombre("mem_info_vram_total"))
        else {
            continue;
        };
        if totale <= 0.0 {
            continue;
        }
        let go = 1024.0 * 1024.0 * 1024.0;
        return Some(Carte { nom: "Carte graphique AMD".into(), trois_d: occupe, vram_utilisee_go: utilisee / go, vram_totale_go: totale / go });
    }
    None
}

/// Un processus de grenOS lui-même : ni fermé, ni ralenti, ni montré dans le
/// gestionnaire de tâches (il n'y aurait rien à en faire).
pub(crate) fn protege(nom: &str) -> bool {
    PROTEGES.contains(&nom) || nom.starts_with("grenos-") || nom.starts_with("systemd")
}

pub(crate) fn meminfo() -> HashMap<String, f64> {
    lire("/proc/meminfo")
        .lines()
        .filter_map(|l| {
            let (cle, reste) = l.split_once(':')?;
            let ko: f64 = reste.split_whitespace().next()?.parse().ok()?;
            Some((cle.to_string(), ko))
        })
        .collect()
}

pub(crate) fn cpu_total() -> (u64, usize) {
    let texte = lire("/proc/stat");
    let total = texte
        .lines()
        .next()
        .map(|l| l.split_whitespace().skip(1).filter_map(|v| v.parse::<u64>().ok()).sum())
        .unwrap_or(0);
    let coeurs = texte.lines().filter(|l| l.starts_with("cpu") && !l.starts_with("cpu ")).count();
    (total, coeurs.max(1))
}

fn mon_uid() -> u32 {
    lire("/proc/self/status")
        .lines()
        .find(|l| l.starts_with("Uid:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(u32::MAX)
}

struct Proc {
    pid: u32,
    nom: String,
    ticks: u64,
    rss_ko: f64,
    uid: u32,
    nice: i32,
}

fn lire_proc(pid: u32) -> Option<Proc> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let ouvre = stat.find('(')?;
    let ferme = stat.rfind(')')?;
    let nom = stat[ouvre + 1..ferme].to_string();
    let champs: Vec<&str> = stat[ferme + 2..].split_whitespace().collect();
    // champs[0] est le 3e champ de stat : utime=14 → 11, stime=15 → 12, nice=19 → 16.
    let ticks = champs.get(11)?.parse::<u64>().ok()? + champs.get(12)?.parse::<u64>().ok()?;
    let nice = champs.get(16)?.parse().unwrap_or(0);
    let status = fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
    let mut rss_ko = 0.0;
    let mut uid = 0;
    for l in status.lines() {
        if let Some(v) = l.strip_prefix("VmRSS:") {
            rss_ko = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        } else if let Some(v) = l.strip_prefix("Uid:") {
            uid = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0);
        }
    }
    // Le nom de /proc/pid/stat est coupé à 15 caractères ; la ligne de
    // commande dit le vrai nom du programme quand c'est le cas.
    let nom = if INTERPRETES_PYTHON.contains(&nom.as_str()) {
        let brut = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
        let mots: Vec<String> = brut.split(|b| *b == 0).filter(|m| !m.is_empty()).map(|m| String::from_utf8_lossy(m).into_owned()).collect();
        let mots: Vec<&str> = mots.iter().map(String::as_str).collect();
        let script = crate::taches::nom_lisible(&mots);
        if script.is_empty() { nom } else { script }
    } else if nom.len() >= 15 {
        fs::read(format!("/proc/{pid}/cmdline"))
            .ok()
            .and_then(|c| c.split(|b| *b == 0).next().map(|p| String::from_utf8_lossy(p).into_owned()))
            .map(|p| p.rsplit('/').next().unwrap_or(&p).to_string())
            .filter(|p| !p.is_empty())
            .unwrap_or(nom)
    } else {
        nom
    };
    Some(Proc { pid, nom, ticks, rss_ko, uid, nice })
}

fn processus() -> Vec<Proc> {
    let Ok(entrees) = fs::read_dir("/proc") else { return vec![] };
    entrees
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(lire_proc)
        // Les fils du noyau n'ont pas de mémoire à eux : rien à montrer.
        .filter(|p| p.rss_ko > 0.0)
        .collect()
}

fn description(nom: &str) -> String {
    if let Some((_, d)) = EN_TROP_POUR_JOUER.iter().find(|(n, _)| *n == nom) {
        return (*d).into();
    }
    match nom {
        "firefox-esr" | "firefox" => "Navigateur Firefox",
        "chromium" | "chrome" => "Navigateur Chromium",
        "steam" | "steamwebhelper" => "Steam",
        "pcmanfm" | "grenos-fichiers" => "Explorateur de fichiers",
        "Xorg" => "Serveur graphique",
        "pipewire" | "wireplumber" | "pipewire-pulse" => "Le son",
        "grenos-shell" | "grenos-shell-natif" => "La barre de grenOS",
        "grenos-bureau" | "grenos-bureau-natif" => "Le bureau de grenOS",
        "grenos-coins" => "Les coins arrondis",
        "grenos-ui" | "WebKitWebProcess" | "WebKitNetworkProcess" => "Fenêtres grenOS",
        "NetworkManager" => "Le réseau",
        "systemd" => "Le système",
        "python3" => "Programme Python",
        "Discord" | "discord" => "Discord",
        "code" => "Visual Studio Code",
        _ => "",
    }
    .into()
}

#[tauri::command]
pub async fn perf_instantane(etat: tauri::State<'_, Etat>) -> Result<Instantane, String> {
    let (total, coeurs) = cpu_total();
    let liste = processus();
    let moi = mon_uid();

    let mut precedent = etat.precedent.lock().map_err(|e| e.to_string())?;
    let (ecart_total, anciens) = match precedent.as_ref() {
        Some(r) if total > r.total_cpu => (total - r.total_cpu, Some(&r.par_pid)),
        _ => (0, None),
    };
    let cpu_global = match precedent.as_ref() {
        Some(r) if total > r.total_cpu => {
            let repos_avant = r.par_pid.get(&0).copied().unwrap_or(0);
            let repos = idle();
            let d_repos = repos.saturating_sub(repos_avant) as f64;
            (1.0 - d_repos / (total - r.total_cpu) as f64).clamp(0.0, 1.0) * 100.0
        }
        _ => 0.0,
    };

    let mut groupes: BTreeMap<String, Groupe> = BTreeMap::new();
    for p in &liste {
        let part = match anciens.and_then(|a| a.get(&p.pid)) {
            Some(avant) if ecart_total > 0 => {
                // Les tics de /proc/stat comptent TOUS les cœurs : rapporté
                // au total, 100 % veut dire « la machine entière ».
                p.ticks.saturating_sub(*avant) as f64 / ecart_total as f64 * 100.0
            }
            _ => 0.0,
        };
        let g = groupes.entry(p.nom.clone()).or_insert_with(|| Groupe {
            nom: p.nom.clone(),
            description: description(&p.nom),
            pids: vec![],
            nombre: 0,
            memoire_mo: 0.0,
            cpu: 0.0,
            a_moi: true,
            protege: PROTEGES.contains(&p.nom.as_str()) || p.nom.starts_with("grenos-") || p.nom.starts_with("systemd"),
            en_trop: EN_TROP_POUR_JOUER.iter().any(|(n, _)| *n == p.nom),
            priorite: p.nice,
        });
        g.pids.push(p.pid);
        g.nombre += 1;
        g.memoire_mo += p.rss_ko / 1024.0;
        g.cpu += part;
        g.a_moi &= p.uid == moi;
        g.priorite = g.priorite.min(p.nice);
    }

    let mut par_pid: HashMap<u32, u64> = liste.iter().map(|p| (p.pid, p.ticks)).collect();
    par_pid.insert(0, idle());
    *precedent = Some(Releve { total_cpu: total, par_pid });
    drop(precedent);

    let m = meminfo();
    let go = |cle: &str| m.get(cle).copied().unwrap_or(0.0) / 1024.0 / 1024.0;
    let mut groupes: Vec<Groupe> = groupes.into_values().collect();
    groupes.sort_by(|a, b| (b.cpu * 40.0 + b.memoire_mo).total_cmp(&(a.cpu * 40.0 + a.memoire_mo)));

    Ok(Instantane {
        coeurs,
        cpu: cpu_global,
        memoire_totale_go: go("MemTotal"),
        memoire_dispo_go: go("MemAvailable"),
        memoire_cache_go: go("Cached") + go("Buffers"),
        echange_total_go: go("SwapTotal"),
        echange_utilise_go: go("SwapTotal") - go("SwapFree"),
        charge: lire("/proc/loadavg").split_whitespace().take(3).collect::<Vec<_>>().join(" "),
        groupes,
        mode_jeu: chemin_mode_jeu().exists(),
        carte: carte_graphique(),
    })
}

fn idle() -> u64 {
    lire("/proc/stat")
        .lines()
        .next()
        .map(|l| {
            let v: Vec<u64> = l.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
            v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0)
        })
        .unwrap_or(0)
}

/// Les pids actuels d'un groupe qui m'appartiennent et ne sont pas protégés.
fn pids_permis(nom: &str) -> Result<Vec<u32>, String> {
    if PROTEGES.contains(&nom) || nom.starts_with("grenos-") || nom.starts_with("systemd") {
        return Err(format!("« {nom} » fait tourner le bureau : grenPerf n'y touche pas."));
    }
    let moi = mon_uid();
    let pids: Vec<u32> = processus().into_iter().filter(|p| p.nom == nom && p.uid == moi).map(|p| p.pid).collect();
    if pids.is_empty() {
        return Err("Plus rien de ce nom ne tourne pour toi.".into());
    }
    Ok(pids)
}

fn signaler(signal: &str, pids: &[u32]) -> bool {
    let mut args = vec![format!("-{signal}")];
    args.extend(pids.iter().map(|p| p.to_string()));
    Command::new("kill").args(&args).output().map(|o| o.status.success()).unwrap_or(false)
}

/// Fermer une application : poliment d'abord (TERM, elle peut enregistrer),
/// puis KILL trois secondes plus tard pour ce qui ne répond plus.
#[tauri::command]
pub async fn perf_fermer(nom: String) -> Result<usize, String> {
    let pids = pids_permis(&nom)?;
    signaler("TERM", &pids);
    std::thread::sleep(std::time::Duration::from_secs(3));
    let restants: Vec<u32> = pids.iter().copied().filter(|p| PathBuf::from(format!("/proc/{p}")).exists()).collect();
    if !restants.is_empty() {
        signaler("KILL", &restants);
    }
    dire(&format!("perf : {nom} ferme ({} processus)", pids.len()));
    Ok(pids.len())
}

fn renice(valeur: i32, pids: &[u32]) -> bool {
    if pids.is_empty() {
        return true;
    }
    let mut args = vec!["-n".to_string(), valeur.to_string(), "-p".into()];
    args.extend(pids.iter().map(|p| p.to_string()));
    Command::new("renice").args(&args).output().map(|o| o.status.success()).unwrap_or(false)
}

/// Haute, normale ou basse. Monter une priorité est permis sans mot de passe
/// parce que grenOS met chaque compte dans les groupes `audio` et `gamemode`
/// (limite `nice -19` de 95-grenos-audio.conf) — c'est ce qui rend le mode
/// jeu réversible sans root.
#[tauri::command]
pub async fn perf_priorite(nom: String, niveau: String) -> Result<usize, String> {
    let valeur = match niveau.as_str() {
        "haute" => -5,
        "normale" => 0,
        "basse" => 10,
        _ => return Err("niveau inconnu".into()),
    };
    let pids = pids_permis(&nom)?;
    if renice(valeur, &pids) { Ok(pids.len()) } else { Err("Le système a refusé ce changement de priorité.".into()) }
}

#[derive(Serialize)]
pub struct Liberation { avant_go: f64, apres_go: f64, message: String }

/// « Libérer la RAM », honnêtement.
///
/// Linux garde en cache ce qu'il a lu, et rend ce cache à la première
/// demande : vider le cache ne donne PAS de mémoire en plus à un jeu, il la
/// lui aurait donnée de toute façon. Ce qui aide vraiment : compacter la
/// mémoire (de grands blocs libres, utiles aux jeux et aux pilotes) et
/// rapatrier ce qui dort dans l'échange. C'est ce que fait l'aide root ; la
/// fenêtre dit ce qui a changé, mesuré, pas promis.
#[tauri::command]
pub async fn perf_liberer() -> Result<Liberation, String> {
    let dispo = || meminfo().get("MemAvailable").copied().unwrap_or(0.0) / 1024.0 / 1024.0;
    let avant = dispo();
    let mut l = privilegie();
    let programme = l.remove(0);
    let ok = Command::new(&programme)
        .args(&l)
        .arg("/usr/lib/grenos/grenos-perf-liberer")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let apres = dispo();
    if !ok {
        return Err("L'autorisation a été refusée.".into());
    }
    let gagne = apres - avant;
    Ok(Liberation {
        avant_go: avant,
        apres_go: apres,
        message: if gagne > 0.05 {
            format!("{} Go de plus immédiatement disponibles, mémoire compactée.", crate::materiel::decimal(gagne, 1))
        } else {
            "Mémoire compactée. Le cache était déjà prêt à être rendu : pour gagner plus, ferme une application de la liste.".into()
        },
    })
}

// ------------------------------------------------------------ le démarrage

#[derive(Serialize)]
pub struct Demarrage { id: String, nom: String, commentaire: String, actif: bool, systeme: bool }

fn dossier_autostart() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into())).join(".config/autostart")
}

fn champ(texte: &str, cle: &str) -> String {
    texte.lines().find_map(|l| l.strip_prefix(&format!("{cle}="))).unwrap_or("").trim().to_string()
}

/// Ce qui démarre avec la session : /etc/xdg/autostart, et le dossier
/// personnel qui a le dernier mot (spécification XDG Autostart).
#[tauri::command]
pub fn perf_demarrage() -> Vec<Demarrage> {
    let mut vus: BTreeMap<String, Demarrage> = BTreeMap::new();
    for (dossier, systeme) in [(PathBuf::from("/etc/xdg/autostart"), true), (dossier_autostart(), false)] {
        let Ok(entrees) = fs::read_dir(&dossier) else { continue };
        for e in entrees.flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            if !id.ends_with(".desktop") {
                continue;
            }
            let texte = fs::read_to_string(e.path()).unwrap_or_default();
            let cache = champ(&texte, "Hidden") == "true";
            let ailleurs = !champ(&texte, "OnlyShowIn").is_empty() && !champ(&texte, "OnlyShowIn").contains("grenOS");
            let nom = champ(&texte, "Name");
            let entree = vus.entry(id.clone()).or_insert(Demarrage {
                id: id.clone(), nom: if nom.is_empty() { id.clone() } else { nom.clone() },
                commentaire: champ(&texte, "Comment"), actif: true, systeme,
            });
            if !nom.is_empty() {
                entree.nom = nom;
            }
            entree.actif = !cache && !ailleurs;
        }
    }
    vus.into_values().collect()
}

#[tauri::command]
pub fn perf_regler_demarrage(id: String, actif: bool) -> Result<(), String> {
    if id.contains('/') || !id.ends_with(".desktop") {
        return Err("nom refusé".into());
    }
    let dossier = dossier_autostart();
    fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    let perso = dossier.join(&id);
    let source = if perso.exists() { perso.clone() } else { PathBuf::from("/etc/xdg/autostart").join(&id) };
    let texte = fs::read_to_string(&source).map_err(|e| e.to_string())?;
    let mut lignes: Vec<String> = texte.lines().filter(|l| !l.starts_with("Hidden=")).map(String::from).collect();
    if !actif {
        // Juste après l'en-tête : un « Hidden » placé dans une autre section
        // ne compterait pas.
        let place = lignes.iter().position(|l| l.trim() == "[Desktop Entry]").map(|i| i + 1).unwrap_or(0);
        lignes.insert(place, "Hidden=true".into());
    }
    fs::write(&perso, lignes.join("\n") + "\n").map_err(|e| e.to_string())
}

// ------------------------------------------------------------- le mode jeu

fn chemin_mode_jeu() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into())).join(".cache/grenos/mode-jeu")
}

#[derive(Serialize)]
pub struct Bilan { fermes: Vec<String>, ralentis: usize, message: String }

/// Le mode jeu : fermer ce qui a été coché, et passer tout le reste de la
/// personne en priorité basse — sauf ce qui est protégé et ce qu'elle a
/// désigné comme le jeu. On note ce qu'on a ralenti pour pouvoir le rendre.
#[tauri::command]
pub async fn perf_mode_jeu(app: tauri::AppHandle, actif: bool, fermer: Vec<String>, garder: Vec<String>) -> Result<Bilan, String> {
    let chemin = chemin_mode_jeu();
    if !actif {
        let notes = fs::read_to_string(&chemin).unwrap_or_default();
        let pids: Vec<u32> = notes.split_whitespace().filter_map(|p| p.parse().ok())
            .filter(|p| PathBuf::from(format!("/proc/{p}")).exists()).collect();
        renice(0, &pids);
        let _ = fs::remove_file(&chemin);
        dire(&format!("perf : mode jeu coupe, {} processus rendus", pids.len()));
        return Ok(Bilan { fermes: vec![], ralentis: pids.len(), message: "Mode jeu coupé : tout a retrouvé sa priorité.".into() });
    }
    let mut fermes = vec![];
    for nom in &fermer {
        if let Ok(pids) = pids_permis(nom) {
            signaler("TERM", &pids);
            fermes.push(nom.clone());
        }
    }
    // GRENAI SE COUPE (I8, 18 novembre) : ce que la demande disait, et que la
    // page excusait par « GrenAI n'existe pas encore ». Sa fenêtre vit dans
    // CE processus — on ne la tue pas, on la ferme. Ce qui pèse vraiment, c'est
    // Ollama : il garde le modèle en mémoire (plusieurs Go, parfois la carte
    // graphique) des minutes après la dernière question. Seulement le NÔTRE :
    // un service Ollama installé pour tout le système n'est pas à nous.
    if let Some(f) = tauri::Manager::get_webview_window(&app, "grenai") {
        let _ = f.close();
        fermes.push("GrenAI".into());
    }
    if let Ok(pids) = pids_permis("ollama") {
        signaler("TERM", &pids);
        fermes.push("ollama (le modèle de GrenAI)".into());
    }
    let moi = mon_uid();
    let a_ralentir: Vec<u32> = processus().into_iter()
        .filter(|p| p.uid == moi && p.nice == 0)
        .filter(|p| !PROTEGES.contains(&p.nom.as_str()) && !p.nom.starts_with("grenos-") && !p.nom.starts_with("systemd"))
        .filter(|p| !garder.contains(&p.nom) && !fermer.contains(&p.nom))
        .filter(|p| !["steam", "steamwebhelper", "gamemoded", "wine", "wineserver", "wine64", "proton", "heroic"].iter().any(|j| p.nom.contains(j)))
        .map(|p| p.pid).collect();
    renice(5, &a_ralentir);
    if let Some(d) = chemin.parent() {
        let _ = fs::create_dir_all(d);
    }
    let _ = fs::write(&chemin, a_ralentir.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(" "));
    dire(&format!("perf : mode jeu, {} ferme(s), {} ralenti(s)", fermes.len(), a_ralentir.len()));
    Ok(Bilan {
        message: format!("Mode jeu actif : {} application(s) fermée(s), {} processus passés en arrière-plan.", fermes.len(), a_ralentir.len()),
        ralentis: a_ralentir.len(),
        fermes,
    })
}

#[cfg(test)]
mod essais_carte {
    use super::*;

    #[test]
    fn nvidia_smi_se_lit() {
        let c = lire_nvidia("NVIDIA GeForce RTX 5060, 12, 2053, 8151\n").unwrap();
        assert_eq!(c.nom, "NVIDIA GeForce RTX 5060");
        assert_eq!(c.trois_d, 12.0);
        assert!((c.vram_utilisee_go - 2.0048).abs() < 0.001);
        assert!((c.vram_totale_go - 7.9600).abs() < 0.001);
        // Une carte qui ne sait pas dire son occupation (« [N/A] ») : rien.
        assert_eq!(lire_nvidia("NVIDIA Quadro, [N/A], 10, 100"), None);
        assert_eq!(lire_nvidia(""), None);
        assert_eq!(lire_nvidia("NVIDIA, 1, 2"), None);
    }
}
