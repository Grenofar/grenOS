//! L'assistant de première configuration, côté système.
//!
//! CE PROGRAMME EST LE SEUL DONT UNE PANNE EMPÊCHE D'ENTRER DANS LA MACHINE.
//! Trois règles en découlent, reprises de la version Python qu'il remplace :
//!
//! * le renommage du compte est porté ligne pour ligne — c'est du code qui a
//!   déjà démarré des centaines de machines, ce n'est pas le moment d'innover ;
//! * si le renommage échoue, on garde l'ancien compte plutôt que d'ouvrir une
//!   session vers un compte qui n'existe pas ;
//! * le lanceur (`grenos-premier-lanceur`) rebascule sur l'assistant Python
//!   si cette fenêtre ne s'affiche pas ou se termine sans avoir nommé la
//!   machine. Une interface plus belle ne vaut pas une machine fermée.

use crate::materiel;
use crate::systeme::{dire, reussit, sortie};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tauri::Emitter;

const IDENTITE: &str = "/etc/grenos/identite";
const REGLAGES: &str = "/etc/grenos/premier-choix";
const MACHINE: &str = "grenos";
const FILE_DES_TRAVAUX: &str = "/usr/lib/grenos/grenos-au-prochain-demarrage";
const PRET: &str = "/run/grenos/premier-ui-pret";
const RESERVES: &[&str] = &[
    "root", "daemon", "bin", "sys", "sync", "nobody", "systemd", "lightdm",
    "messagebus", "grenos", "admin", "user", "live",
];

static DONNEES_JSON: &str = include_str!("../../config/includes.chroot/usr/share/grenos/premier.json");

#[derive(Deserialize, Serialize, Clone)]
pub struct Ecran { pub cle: String, pub titre: String, pub parcours: Vec<String> }
#[derive(Deserialize, Serialize, Clone)]
pub struct Usage { pub cle: String, pub titre: String, pub note: String, pub paquets: Vec<String> }
#[derive(Deserialize, Serialize, Clone)]
pub struct Clavier { pub nom: String, pub disposition: String, pub variante: String, pub fuseau: String }
#[derive(Deserialize, Serialize, Clone)]
pub struct Donnees { pub ecrans: Vec<Ecran>, pub usages: Vec<Usage>, pub claviers: Vec<Clavier> }

pub fn donnees() -> Donnees {
    serde_json::from_str(DONNEES_JSON).expect("premier.json est vérifié à la compilation par les essais")
}

// ------------------------------------------------------------------ le nom

fn sans_accent(c: char) -> &'static str {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => "a",
        'ç' | 'ć' | 'č' => "c",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => "e",
        'ì' | 'í' | 'î' | 'ï' | 'ī' => "i",
        'ñ' | 'ń' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => "o",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' => "u",
        'ý' | 'ÿ' => "y",
        'ś' | 'š' => "s",
        'ź' | 'ż' | 'ž' => "z",
        'œ' => "oe",
        'æ' => "ae",
        'ß' => "ss",
        _ => "",
    }
}

/// « Léa Martin » → « lea-martin ». Un compte Unix n'a ni accent ni espace.
pub fn en_login(texte: &str) -> String {
    let mut propre = String::new();
    for c in texte.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
            propre.push(c);
        } else if c == ' ' {
            propre.push('-');
        } else {
            propre.push_str(sans_accent(c));
        }
    }
    let propre = propre.trim_matches('-');
    propre.chars().take(31).collect::<String>().trim_end_matches('-').to_string()
}

fn existe(login: &str) -> bool {
    reussit("getent", &["passwd", login])
}

pub fn probleme(login: &str) -> Option<&'static str> {
    if login.is_empty() {
        return Some("Un nom, avec au moins une lettre.");
    }
    if !login.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
        return Some("Un nom commence par une lettre.");
    }
    if RESERVES.contains(&login) {
        return Some("Ce nom est déjà pris par le système, choisis-en un autre.");
    }
    if existe(login) {
        return Some("Ce compte existe déjà sur cette machine.");
    }
    None
}

fn compte_actuel() -> String {
    sortie("getent", &["passwd", "1000"]).split(':').next().unwrap_or("").trim().to_string()
}

// ---------------------------------------------------------- l'application

fn ouverture(login: &str) {
    let lignes = format!(
        "# Écrit par grenOS. LightDM lit lightdm.conf en dernier.\n\
         [Seat:*]\nuser-session=grenos\ngreeter-session=lightdm-gtk-greeter\n\
         autologin-user={login}\nautologin-user-timeout=0\nautologin-session=grenos\n"
    );
    let _ = fs::create_dir_all("/etc/lightdm/lightdm.conf.d");
    for chemin in ["/etc/lightdm/lightdm.conf", "/etc/lightdm/lightdm.conf.d/60-grenos.conf"] {
        let _ = fs::write(chemin, &lignes);
    }
}

fn ecrire_clavier(disposition: &str, variante: &str) {
    let contenu = format!(
        "# Écrit par grenOS, au premier démarrage.\nXKBMODEL=\"pc105\"\n\
         XKBLAYOUT=\"{disposition}\"\nXKBVARIANT=\"{variante}\"\nXKBOPTIONS=\"\"\nBACKSPACE=\"guess\"\n"
    );
    let _ = fs::write("/etc/default/keyboard", contenu);
}

/// Renomme le compte, écrit qui habite ici, et rend le nom retenu.
/// Porté de `grenos-premier` (Python) sans changer l'ordre des gestes.
fn appliquer(login: &str, affiche: &str, clavier: &Clavier) -> String {
    ecrire_clavier(&clavier.disposition, &clavier.variante);
    let _ = Command::new("timedatectl").args(["set-timezone", &clavier.fuseau]).output();

    let ancien = compte_actuel();
    let mut login = login.to_string();
    if !ancien.is_empty() && ancien != login {
        let maison = format!("/home/{login}");
        let _ = Command::new("usermod").args(["-l", &login, "-d", &maison, "-m", &ancien]).status();
        let _ = Command::new("groupmod").args(["-n", &login, &ancien]).status();
    }
    if !existe(&login) {
        dire(&format!("le compte n'a pas pu etre renomme, on garde {ancien}"));
        login = if ancien.is_empty() { "grenos".into() } else { ancien };
    }
    let _ = Command::new("usermod").args(["-c", affiche, &login]).status();

    let _ = fs::create_dir_all("/etc/grenos");
    let _ = fs::write(
        IDENTITE,
        format!(
            "# Qui habite cette machine. Écrit une seule fois, au premier démarrage.\n\
             login={login}\nnom={affiche}\nmachine={MACHINE}\n\
             clavier={}\nvariante={}\nfuseau={}\n",
            clavier.disposition, clavier.variante, clavier.fuseau
        ),
    );

    ouverture(&login);
    let sudoers = "/etc/sudoers.d/grenos-identite";
    if fs::write(sudoers, format!("{login} ALL=(ALL) NOPASSWD: ALL\n")).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(sudoers, fs::Permissions::from_mode(0o440));
        }
    }
    let _ = Command::new("passwd").args(["-d", &login]).output();
    login
}

/// I5 — « un mot de passe au login s'il le souhaite ». Passé par l'entrée
/// standard à `grenos-compte`, qui coupe aussi l'ouverture automatique et le
/// sudo sans mot de passe : une protection à moitié n'en est pas une.
fn poser_mot_de_passe(login: &str, mot: &str) -> bool {
    let Ok(mut enfant) = Command::new("/usr/lib/grenos/grenos-compte")
        .args(["definir", login])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut entree) = enfant.stdin.take() {
        let _ = writeln!(entree, "{mot}");
    }
    enfant.wait().map(|s| s.success()).unwrap_or(false)
}

/// Le thème, l'accent et le niveau d'effets, dans le compte de la personne.
/// « auto » ne s'écrit pas : l'absence de fichier veut dire « laisse grenOS
/// décider », et c'est ce que `grenosui.palette()` comprend.
fn poser_apparence(login: &str, reponses: &BTreeMap<String, String>) {
    let dossier = format!("/home/{login}/.config/grenos");
    if fs::create_dir_all(&dossier).is_err() {
        return;
    }
    if let Some(theme) = reponses.get("apparence").filter(|t| *t == "jour" || *t == "nuit") {
        let _ = fs::write(format!("{dossier}/theme"), format!("{theme}\n"));
    }
    for cle in ["accent", "effets"] {
        if let Some(v) = reponses.get(cle).filter(|v| !v.is_empty()) {
            let _ = fs::write(format!("{dossier}/{cle}"), format!("{v}\n"));
        }
    }
    let _ = Command::new("chown")
        .args(["-R", &format!("{login}:{login}"), &format!("/home/{login}/.config")])
        .status();
}

fn garder_choix(reponses: &BTreeMap<String, String>) {
    let mut texte = String::from("# Les réponses de l'assistant de première configuration.\n");
    for (cle, valeur) in reponses {
        texte.push_str(&format!("{cle}={valeur}\n"));
    }
    let _ = fs::create_dir_all("/etc/grenos");
    let _ = fs::write(REGLAGES, texte);
}

/// I11 — ENFIN APPLIQUÉ. L'assistant Python rangeait ce choix dans un fichier
/// que personne ne relisait : « Automatiques (recommandé) » n'allumait rien.
fn regler_majs(mode: &str) {
    let mode = match mode {
        "automatiques" | "demander" | "manuelles" => mode,
        _ => "automatiques",
    };
    let ok = reussit("/usr/lib/grenos/grenos-maj-reglage", &["mode", mode]);
    dire(&format!("premier : mises a jour reglees sur {mode}{}", if ok { "" } else { " — REFUSE" }));
}

/// I13 — les sauvegardes. « externe » allume le minuteur de session de
/// grenos-sauvegarde pour TOUS les comptes (`--global`) : une copie par jour
/// dès qu'un disque externe est branché, relue pour vérifier qu'elle se
/// restaure. Rien n'est allumé sans le choix explicite.
fn regler_sauvegardes(mode: &str) {
    if mode != "externe" {
        return;
    }
    let ok = reussit("systemctl", &["--global", "enable", "grenos-sauvegarde.timer"]);
    let _ = fs::write("/etc/grenos/sauvegardes", "externe
");
    dire(&format!("premier : sauvegardes quotidiennes vers disque externe{}", if ok { "" } else { " — minuteur REFUSE" }));
}

/// I17 — ce que le parcours Dev choisit en plus, appliqué pour de vrai. Les
/// écrans ne sont montrés qu'en Dev ; on n'applique rien hors de ce parcours,
/// même si une valeur par défaut traîne dans les réponses.
fn regler_dev(reponses: &BTreeMap<String, String>) {
    if reponses.get("parcours").map(|s| s.as_str()) != Some("dev") {
        return;
    }
    let v = |c: &str, d: &str| reponses.get(c).cloned().unwrap_or_else(|| d.to_string());

    // Le pare-feu : ufw, entrant refusé, sortant permis.
    if v("parefeu", "actif") == "actif" {
        let ok = reussit("ufw", &["default", "deny", "incoming"])
            && reussit("ufw", &["default", "allow", "outgoing"])
            && reussit("ufw", &["--force", "enable"]);
        dire(&format!("premier : pare-feu {}", if ok { "actif, entrant refuse" } else { "PAS active (ufw a refuse)" }));
    }

    // Les journaux : une taille et une durée, dans un fichier qui passe après
    // le nôtre (10-grenos-persistant.conf) et l'emporte donc.
    let (taille, duree) = match v("journaux", "mois").as_str() {
        "semaine" => ("50M", "1week"),
        "trimestre" => ("500M", "3month"),
        _ => ("100M", "1month"),
    };
    let _ = fs::create_dir_all("/etc/systemd/journald.conf.d");
    let _ = fs::write(
        "/etc/systemd/journald.conf.d/60-grenos-choix.conf",
        format!("# Choisi au premier démarrage (parcours Dev).
[Journal]
SystemMaxUse={taille}
MaxRetentionSec={duree}
"),
    );
    let _ = reussit("systemctl", &["restart", "systemd-journald"]);

    // L'échange : zram, et quand y puiser.
    if v("echange", "zram") == "aucun" {
        let _ = reussit("systemctl", &["disable", "--now", "zramswap.service"]);
    }
    let swappiness = if v("swappiness", "equilibre") == "reactif" { 10 } else { 60 };
    let _ = fs::create_dir_all("/etc/sysctl.d");
    let _ = fs::write("/etc/sysctl.d/60-grenos-choix.conf", format!("# Choisi au premier démarrage (parcours Dev).
vm.swappiness = {swappiness}
"));
    let _ = reussit("sysctl", &["-w", &format!("vm.swappiness={swappiness}")]);
    dire(&format!("premier : dev — journaux {taille}/{duree}, echange {}, swappiness {swappiness}", v("echange", "zram")));
}

fn deposer_les_travaux(reponses: &BTreeMap<String, String>, donnees: &Donnees) {
    let mut travaux: Vec<(String, Vec<String>)> = Vec::new();
    let mut choisies: Vec<String> = reponses
        .get("applications").map(|s| s.as_str()).unwrap_or("")
        .split(',').filter(|c| !c.is_empty()).map(String::from).collect();
    if let Some(usage) = donnees.usages.iter().find(|u| Some(&u.cle) == reponses.get("usage")) {
        for p in &usage.paquets {
            if !choisies.contains(p) {
                choisies.push(p.clone());
            }
        }
    }
    choisies.sort();
    if !choisies.is_empty() {
        travaux.push(("Installer les applications choisies".into(),
                      vec!["/usr/lib/grenos/grenos-installer-choix".into(), choisies.join(",")]));
    }
    if reponses.get("optimiser").map(|s| s.as_str()) == Some("oui") {
        travaux.push(("Régler la machine pour la performance".into(),
                      vec!["/usr/lib/grenos/grenos-performances".into()]));
    }
    if travaux.is_empty() {
        dire("premier : aucun travail a deposer — rien n a ete demande");
        return;
    }
    for (intitule, commande) in travaux {
        let mut arguments = vec!["20".to_string(), FILE_DES_TRAVAUX.into(), intitule.clone()];
        arguments.extend(commande);
        match Command::new("timeout").args(&arguments).output() {
            Ok(fin) if fin.status.success() => dire(&format!("premier : travail depose — {intitule}")),
            Ok(fin) => dire(&format!("premier : travail « {intitule} » refuse par la file — {}",
                                     String::from_utf8_lossy(&fin.stderr).trim().chars().take(80).collect::<String>())),
            Err(e) => dire(&format!("premier : travail « {intitule} » PAS depose — {e}")),
        }
    }
}

// ----------------------------------------------------------- Secure Boot

pub fn secure_boot(racine: &str) -> &'static str {
    let Ok(entrees) = fs::read_dir(racine) else { return "pas en UEFI" };
    let mut noms: Vec<_> = entrees.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    noms.sort();
    for nom in noms {
        if !nom.starts_with("SecureBoot-") {
            continue;
        }
        return match fs::read(format!("{racine}/{nom}")) {
            Ok(o) if o.len() >= 5 => if o[4] != 0 { "actif" } else { "inactif" },
            _ => "inconnu",
        };
    }
    "inconnu"
}

pub fn mot_sur_secure_boot(etat: &str) -> &'static str {
    match etat {
        "actif" => "Secure Boot est actif. grenOS n'est pas encore signé : il faut le désactiver dans le BIOS pour démarrer cette machine sur grenOS.",
        "inactif" => "Secure Boot est désactivé, ce qu'il faut aujourd'hui pour démarrer grenOS. Le noyau signé est en chantier.",
        "pas en UEFI" => "Cette machine a démarré sans UEFI, donc Secure Boot ne s'applique pas.",
        _ => "L'état de Secure Boot n'a pas pu être lu sur cette machine.",
    }
}

// --------------------------------------------------------------- commandes

#[derive(Serialize)]
pub struct Infos {
    donnees: Donnees,
    materiel: Vec<materiel::Ligne>,
    virtuelle: String,
    secure_boot: String,
    secure_boot_phrase: String,
    machine: String,
}

#[tauri::command]
pub async fn premier_infos() -> Infos {
    let materiel = materiel::resume();
    let etat = secure_boot("/sys/firmware/efi/efivars");
    dire(&format!("premier : securite — secure boot {etat}"));
    if let (Some(cpu), Some(ram), Some(gpu)) = (materiel.first(), materiel.get(1), materiel.get(2)) {
        dire(&format!("premier : optimisation — {}, {}, carte {}", cpu.valeur, ram.valeur, gpu.valeur));
    }
    Infos {
        donnees: donnees(),
        materiel,
        virtuelle: materiel::virtuelle(),
        secure_boot: etat.into(),
        secure_boot_phrase: mot_sur_secure_boot(etat).into(),
        machine: MACHINE.into(),
    }
}

#[tauri::command]
pub fn premier_dire(texte: String) {
    dire(&texte);
}

/// La page est dessinée : on le dit au lanceur (qui sinon rebascule sur le
/// secours Python), et on prend le clavier. Sans gestionnaire de fenêtres,
/// personne d'autre ne le donnera — la machine d'intégration a déjà vu une
/// personne taper dans le vide.
#[tauri::command]
pub fn premier_pret(fenetre: tauri::WebviewWindow) {
    let _ = fs::create_dir_all("/run/grenos");
    let _ = fs::write(PRET, "oui\n");
    let _ = fenetre.set_focus();
    dire("premier : interface grenos-ui affichee");
}

#[derive(Serialize)]
pub struct Nom { login: String, adresse: String, probleme: Option<String> }

#[tauri::command]
pub fn premier_nom(texte: String) -> Nom {
    let login = en_login(&texte);
    let souci = probleme(&login).map(String::from);
    Nom { adresse: if login.is_empty() { String::new() } else { format!("{login}@{MACHINE}") }, login, probleme: souci }
}

#[tauri::command]
pub fn premier_clavier(index: usize) {
    let d = donnees();
    if let Some(c) = d.claviers.get(index) {
        let mut arguments = vec![c.disposition.as_str()];
        if !c.variante.is_empty() {
            arguments.push(c.variante.as_str());
        }
        let _ = Command::new("setxkbmap").args(&arguments).output();
    }
}

#[tauri::command]
pub async fn premier_terminer(
    app: tauri::AppHandle,
    reponses: BTreeMap<String, String>,
    mot_de_passe: Option<String>,
) -> Result<String, String> {
    let donnees = donnees();
    let login = reponses.get("compte").cloned().unwrap_or_default();
    if let Some(souci) = probleme(&login) {
        return Err(souci.into());
    }
    let index: usize = reponses.get("clavier_index").and_then(|i| i.parse().ok()).unwrap_or(0);
    let clavier = donnees.claviers.get(index).cloned().unwrap_or_else(|| donnees.claviers[0].clone());
    let affiche = reponses.get("nom_affiche").filter(|s| !s.trim().is_empty()).cloned().unwrap_or_else(|| login.clone());

    let retenu = appliquer(&login, affiche.trim(), &clavier);
    if let Some(mot) = mot_de_passe.filter(|m| !m.is_empty()) {
        let ok = poser_mot_de_passe(&retenu, &mot);
        dire(&format!("premier : mot de passe {}", if ok { "pose" } else { "PAS pose" }));
    }
    poser_apparence(&retenu, &reponses);
    regler_majs(reponses.get("majs").map(|s| s.as_str()).unwrap_or("automatiques"));
    regler_sauvegardes(reponses.get("sauvegardes").map(|s| s.as_str()).unwrap_or("non"));
    regler_dev(&reponses);
    garder_choix(&reponses);
    deposer_les_travaux(&reponses, &donnees);
    dire(&format!("nom choisi, la machine est a {retenu}@{MACHINE}"));
    let v = |c: &str, d: &str| reponses.get(c).cloned().unwrap_or_else(|| d.to_string());
    dire(&format!("premier : choix gardes — apparence {}, telemetrie {}, majs {}",
                  v("apparence", "auto"), v("telemetrie", "aucune"), v("majs", "automatiques")));
    // DEUX CHEMINS VERS LA PAGE (18 novembre). La réponse de cette commande
    // passe par le protocole IPC de Tauri (une requête de la page) ; trois
    // machines d'essai sur huit ne l'ont jamais reçue, la page restait sur
    // « Mise en place… ». L'événement passe par un autre chemin — un script
    // injecté dans la page par WebKit — et la page prend le premier arrivé.
    let adresse = format!("{retenu}@{MACHINE}");
    let _ = app.emit("premier-fini", adresse.clone());
    // LE FILET DE FIN (18 novembre). Tout est écrit à cette ligne : la page
    // n'a plus qu'à afficher « grenOS est prêt » et appeler premier_quitter.
    // Deux machines d'essai sur cinq sont pourtant restées figées ici, la
    // réponse jamais arrivée à la page — et le bureau ne s'ouvrait jamais.
    // La cause n'est pas encore trouvée ; en attendant, le processus se ferme
    // seul douze secondes plus tard (la page le fait d'habitude à 2,6 s), et
    // il le DIT : la ligne du port série nommera chaque fois où il a servi.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(12));
        dire("premier : la page n a pas quitte seule apres 12 s — fermeture par le filet");
        std::process::exit(0);
    });
    Ok(adresse)
}

#[tauri::command]
pub fn premier_quitter(app: tauri::AppHandle) {
    app.exit(0);
}

// ----------------------------------------------------- I4 : le réseau

#[derive(Serialize)]
pub struct EtatReseau { connecte: bool, wifi: bool, gestionnaire: bool, nom: String }

#[tauri::command]
pub async fn reseau_etat() -> EtatReseau {
    let general = sortie("nmcli", &["-t", "-f", "STATE", "general"]);
    let appareils = sortie("nmcli", &["-t", "-f", "TYPE,STATE,CONNECTION", "device"]);
    let nom = appareils
        .lines()
        .filter_map(|l| {
            let c = decouper(l);
            (c.len() >= 3 && c[1] == "connected").then(|| c[2].clone())
        })
        .next()
        .unwrap_or_default();
    EtatReseau {
        connecte: general.trim().starts_with("connected"),
        wifi: appareils.lines().any(|l| l.starts_with("wifi:")),
        gestionnaire: !general.trim().is_empty(),
        nom,
    }
}

/// `nmcli -t` sépare par « : » et échappe les « : » des valeurs en « \: ».
fn decouper(ligne: &str) -> Vec<String> {
    let mut champs = vec![String::new()];
    let mut echappe = false;
    for c in ligne.chars() {
        if echappe {
            champs.last_mut().unwrap().push(c);
            echappe = false;
        } else if c == '\\' {
            echappe = true;
        } else if c == ':' {
            champs.push(String::new());
        } else {
            champs.last_mut().unwrap().push(c);
        }
    }
    champs
}

#[derive(Serialize)]
pub struct Reseau { nom: String, signal: u32, protege: bool, actif: bool }

#[tauri::command]
pub async fn reseau_liste() -> Vec<Reseau> {
    let texte = sortie("nmcli", &["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", "auto"]);
    let mut vus: BTreeMap<String, Reseau> = BTreeMap::new();
    for ligne in texte.lines() {
        let c = decouper(ligne);
        if c.len() < 4 || c[1].is_empty() {
            continue;
        }
        let r = Reseau {
            nom: c[1].clone(),
            signal: c[2].parse().unwrap_or(0),
            protege: !c[3].is_empty() && c[3] != "--",
            actif: c[0] == "*",
        };
        let garder = vus.get(&r.nom).map(|v| r.signal > v.signal || r.actif).unwrap_or(true);
        if garder {
            vus.insert(r.nom.clone(), r);
        }
    }
    let mut liste: Vec<Reseau> = vus.into_values().collect();
    liste.sort_by(|a, b| b.actif.cmp(&a.actif).then(b.signal.cmp(&a.signal)));
    liste
}

#[tauri::command]
pub async fn reseau_connecter(nom: String, mot_de_passe: Option<String>) -> Result<(), String> {
    let mut arguments = vec!["45".to_string(), "nmcli".into(), "device".into(), "wifi".into(), "connect".into(), nom.clone()];
    if let Some(m) = mot_de_passe.filter(|m| !m.is_empty()) {
        arguments.push("password".into());
        arguments.push(m);
    }
    let fin = Command::new("timeout").args(&arguments).output().map_err(|e| e.to_string())?;
    if fin.status.success() {
        dire(&format!("premier : reseau — connecte a {nom}"));
        Ok(())
    } else {
        let texte = String::from_utf8_lossy(&fin.stderr).to_lowercase();
        Err(if texte.contains("secrets") || texte.contains("password") {
            "Mot de passe refusé par le réseau.".into()
        } else if fin.status.code() == Some(124) {
            "Le réseau n'a pas répondu à temps.".into()
        } else {
            "La connexion a échoué.".into()
        })
    }
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn noms() {
        assert_eq!(en_login("Léa Martin"), "lea-martin");
        assert_eq!(en_login("  Zoë  "), "zoe");
        assert_eq!(en_login("Œdipe"), "oedipe");
        assert_eq!(en_login("--a--"), "a");
        assert_eq!(en_login("😀"), "");
        assert_eq!(en_login(&"a".repeat(50)).len(), 31);
    }

    #[test]
    fn problemes() {
        assert!(probleme("").is_some());
        assert!(probleme("1abc").is_some());
        assert!(probleme("root").is_some());
    }

    #[test]
    fn donnees_partagees() {
        let d = donnees();
        assert_eq!(d.ecrans.first().unwrap().cle, "bienvenue");
        assert_eq!(d.ecrans.last().unwrap().cle, "final");
        assert_eq!(d.claviers.len(), 16);
        assert!(d.usages.iter().any(|u| u.cle == "developpement"));
    }

    #[test]
    fn nmcli_echappe() {
        assert_eq!(decouper(r"*:Box\:5G:80:WPA2"), vec!["*", "Box:5G", "80", "WPA2"]);
    }

    #[test]
    fn secure_boot_absent() {
        assert_eq!(secure_boot("/nulle/part"), "pas en UEFI");
    }
}
