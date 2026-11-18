//! Ce que toutes les fenêtres partagent : parler au port série, lancer une
//! commande, lire un fichier, et les préférences de la personne.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

/// Une ligne sur le port série : c'est par là que la CI nous regarde.
///
/// Les phrases sont reprises MOT POUR MOT de l'assistant Python : la machine
/// d'intégration attend « premier : ecran … », « nom choisi », etc. En changer
/// une seule lettre ferait échouer l'essai pour une raison qui n'a rien à voir
/// avec grenOS.
///
/// En root (l'assistant, avant le bureau), on écrit sur le port. Dans la
/// session, le compte n'a PAS le droit d'y écrire (pas dans `dialout`) : on
/// dépose un billet dans /tmp/grenos-billets, que `grenos-preuve` (root)
/// relaie — exactement ce que fait `grenos-dire`. Jusqu'au 18 novembre, les
/// fenêtres grenos-ui ouvertes dans la session ne disaient donc RIEN à la CI.
pub fn dire(texte: &str) {
    eprintln!("grenos-ui: {texte}");
    if let Ok(mut port) = fs::OpenOptions::new().write(true).open("/dev/ttyS0") {
        let _ = writeln!(port, "grenos: {texte}");
        return;
    }
    deposer_billet(texte);
}

#[cfg(unix)]
fn deposer_billet(texte: &str) {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    // Sans port série, personne ne relèvera le billet : on n'en dépose pas.
    match fs::metadata("/dev/ttyS0") {
        Ok(m) if m.file_type().is_char_device() => {}
        _ => return,
    }
    let boite = std::path::Path::new("/tmp/grenos-billets");
    if fs::create_dir_all(boite).is_err() {
        return;
    }
    let _ = fs::set_permissions(boite, fs::Permissions::from_mode(0o1777));
    use std::sync::atomic::{AtomicU32, Ordering};
    static NUMERO: AtomicU32 = AtomicU32::new(0);
    let secondes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Numéro sur six chiffres : le relais lit les billets dans l'ordre
    // alphabétique, et « 10 » passerait avant « 2 ». Écrit sous un nom caché
    // puis renommé : le relais (qui ignore les noms en « . ») ne lit jamais
    // un billet à moitié écrit.
    let nom = format!("{secondes}-{}-{:06}", std::process::id(), NUMERO.fetch_add(1, Ordering::SeqCst));
    let cache = boite.join(format!(".{nom}"));
    if fs::write(&cache, format!("{texte}
")).is_ok() {
        let _ = fs::rename(&cache, boite.join(nom));
    }
}

#[cfg(not(unix))]
fn deposer_billet(_texte: &str) {}

pub fn lancer(programme: &str, arguments: &[&str]) -> Option<Output> {
    Command::new(programme)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .ok()
}

pub fn reussit(programme: &str, arguments: &[&str]) -> bool {
    lancer(programme, arguments).map(|s| s.status.success()).unwrap_or(false)
}

pub fn sortie(programme: &str, arguments: &[&str]) -> String {
    lancer(programme, arguments)
        .map(|s| String::from_utf8_lossy(&s.stdout).into_owned())
        .unwrap_or_default()
}

pub fn lire(chemin: &str) -> String {
    fs::read_to_string(chemin).unwrap_or_default()
}

/// Avec sudo quand il ne demande rien (le cas tant qu'aucun mot de passe
/// n'est défini), pkexec sinon — exactement la règle de `maj.py`.
pub fn privilegie() -> Vec<String> {
    if reussit("sudo", &["-n", "true"]) {
        vec!["sudo".into(), "-n".into()]
    } else {
        vec!["pkexec".into()]
    }
}

fn dossier_reglages() -> PathBuf {
    let maison = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(maison).join(".config").join("grenos")
}

/// Les préférences visuelles de la personne, lues par CHAQUE fenêtre.
///
/// Les mêmes fichiers que `grenosui.py` : une personnalisation faite ici se
/// voit aussi dans les applications Python qui n'ont pas encore migré.
#[derive(serde::Serialize)]
pub struct Apparence {
    pub theme: String,
    pub accent: String,
    pub effets: String,
}

pub fn apparence() -> Apparence {
    let base = dossier_reglages();
    let lu = |nom: &str, defaut: &str| {
        let v = fs::read_to_string(base.join(nom)).unwrap_or_default();
        let v = v.trim();
        if v.is_empty() { defaut.to_string() } else { v.to_string() }
    };
    Apparence {
        theme: lu("theme", "auto"),
        accent: lu("accent", "bleu"),
        effets: lu("effets", "equilibres"),
    }
}

pub fn ecrire_reglage(nom: &str, valeur: &str) -> Result<(), String> {
    let base = dossier_reglages();
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    fs::write(base.join(nom), format!("{valeur}\n")).map_err(|e| e.to_string())
}

/// WebKitGTK sur une machine sans carte 3D.
///
/// Le mode composité demande OpenGL ; sans accélération, il passe par
/// llvmpipe et coûte plus cher que le rendu logiciel direct. Le rendu par
/// DMA-BUF, lui, laisse une fenêtre blanche sur plusieurs cartes virtuelles.
/// On ne force rien si la personne (ou un essai) a déjà choisi.
pub fn preparer_webkit() {
    for (cle, valeur) in [
        ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
        ("WEBKIT_DISABLE_COMPOSITING_MODE", "1"),
    ] {
        if std::env::var_os(cle).is_none() {
            std::env::set_var(cle, valeur);
        }
    }
}

/// UN SEUL PROCESSUS POUR TOUTES LES FENÊTRES.
///
/// C'est ce qui rend Tauri moins gourmand que nos programmes Python et non
/// plus : chaque `grenos-ui <page>` lancé alors qu'un autre tourne déjà
/// envoie le nom de la page par une prise Unix et s'arrête aussitôt. La
/// fenêtre s'ouvre dans le processus existant, qui partage son moteur web.
#[cfg(unix)]
pub mod instance {
    use std::io::{Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;
    use tauri::AppHandle;

    fn chemin() -> PathBuf {
        match std::env::var("XDG_RUNTIME_DIR") {
            Ok(d) if !d.is_empty() => PathBuf::from(d).join("grenos-ui.sock"),
            _ => {
                let qui = std::env::var("USER").unwrap_or_else(|_| "inconnu".into());
                PathBuf::from(format!("/tmp/grenos-ui-{qui}.sock"))
            }
        }
    }

    /// Vrai si une instance tournait et a pris la demande.
    pub fn transmettre(page: &str) -> bool {
        match UnixStream::connect(chemin()) {
            Ok(mut prise) => prise.write_all(page.as_bytes()).is_ok(),
            Err(_) => false,
        }
    }

    pub fn ecouter(app: AppHandle) {
        let chemin = chemin();
        // Une prise restée d'un processus mort : personne n'y répond plus.
        let _ = std::fs::remove_file(&chemin);
        let Ok(ecoute) = UnixListener::bind(&chemin) else { return };
        std::thread::spawn(move || {
            for prise in ecoute.incoming().flatten() {
                let mut prise = prise;
                let mut page = String::new();
                if prise.read_to_string(&mut page).is_ok() {
                    let page = page.trim().to_string();
                    let poignee = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        let _ = crate::ouvrir(&poignee, &page);
                    });
                }
            }
        });
    }
}

#[cfg(not(unix))]
pub mod instance {
    pub fn transmettre(_page: &str) -> bool { false }
    pub fn ecouter(_app: tauri::AppHandle) {}
}

/// Lance un programme détaché : s'il meurt, la fenêtre survit, et s'il dure,
/// il ne garde pas grenos-ui comme parent attentif (`start_new_session` de
/// `grenosui.lancer`).
pub fn detacher(programme: &str, arguments: &[&str]) -> Result<(), String> {
    let mut c = Command::new(programme);
    c.args(arguments).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    c.spawn().map(|_| ()).map_err(|e| format!("{programme} : {e}"))
}

/// Ce qu'une page a le droit d'ouvrir par `ouvrir_programme` : des
/// applications, jamais une commande libre. Une page compromise ne peut rien
/// lancer de plus qu'un clic dans le menu.
pub const PROGRAMMES: &[&str] = &[
    "grenplace", "grenlaunch", "grenos-jeux", "grenos-connexions", "grenos-parametres",
    "grenos-maj", "grenos-fichiers", "grenos-taches", "grenos-son", "grenos-hub",
    "grenos-perf", "grenos-secours", "grenos-bienvenue", "grenos-arret",
    "firefox-esr", "x-terminal-emulator", "installer", "pavucontrol", "nm-connection-editor", "arandr", "gnome-disks", "gufw",
];

#[tauri::command]
pub fn ouvrir_programme(nom: String) -> Result<(), String> {
    if !PROGRAMMES.contains(&nom.as_str()) {
        return Err(format!("« {nom} » n'est pas une application de grenOS"));
    }
    if nom == "installer" {
        return detacher("pkexec", &["calamares"]);
    }
    detacher(&nom, &[])
}

/// Le nom choisi au premier démarrage, comme `grenosui.identite()`.
pub fn identite() -> std::collections::HashMap<String, String> {
    let mut v = std::collections::HashMap::new();
    v.insert("login".to_string(), std::env::var("USER").unwrap_or_else(|_| "grenos".into()));
    v.insert("nom".to_string(), String::new());
    v.insert("machine".to_string(), "grenos".to_string());
    for ligne in lire("/etc/grenos/identite").lines() {
        if let Some((cle, valeur)) = ligne.trim().split_once('=') {
            v.insert(cle.trim().to_string(), valeur.trim().to_string());
        }
    }
    v
}

/// « login@machine », comme `grenosui.adresse()`.
pub fn adresse() -> String {
    let i = identite();
    format!("{}@{}", i["login"], i["machine"])
}
