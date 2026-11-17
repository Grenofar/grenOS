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
pub fn dire(texte: &str) {
    if let Ok(mut port) = fs::OpenOptions::new().write(true).open("/dev/ttyS0") {
        let _ = writeln!(port, "grenos: {texte}");
    }
    eprintln!("grenos-ui: {texte}");
}

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
