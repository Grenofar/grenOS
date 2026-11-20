//! Installer (ou mettre à jour) une application : Flathub ou les dépôts Debian.
//!
//! La fenêtre « Installation de … » de GrenPlace et de Jeux (Python), en une
//! seule pour les deux le 18 novembre : une barre, une phrase, le détail
//! complet pour qui veut le lire. Les commandes sont celles du Python, au mot
//! près — `flatpak install -y --noninteractive flathub <id>`, ou `apt-get
//! install -y <paquet>` par sudo/pkexec.
//!
//! L'identifiant passe la même règle que dans le magasin (IDENTIFIANT_SAIN) :
//! un catalogue empoisonné ne peut pas glisser une option ou un `;` jusqu'à
//! la ligne de commande.

use crate::systeme::{dire, privilegie};
use serde::Serialize;
use std::io::Read;
use std::process::{Command, Stdio};
use tauri::Emitter;

/// `^[A-Za-z0-9][A-Za-z0-9._+-]*$`, comme `IDENTIFIANT_SAIN` de grenplace.
pub fn identifiant_sain(id: &str) -> bool {
    let mut c = id.chars();
    matches!(c.next(), Some(p) if p.is_ascii_alphanumeric())
        && c.all(|x| x.is_ascii_alphanumeric() || "._+-".contains(x))
}

#[derive(Serialize, Clone)]
struct Avance {
    cle: String,
    /// De 0 à 100, ou absent quand la ligne ne dit pas où l'on en est.
    part: Option<u32>,
    ligne: Option<String>,
    texte: Option<String>,
    fin: bool,
    bien: bool,
}

/// Ce paquet vient-il du dépôt de son éditeur (usr/share/grenos/depots) ?
pub fn depot_editeur(identifiant: &str) -> bool {
    identifiant_sain(identifiant) && std::path::Path::new(&format!("/usr/share/grenos/depots/{identifiant}.list")).is_file()
}

/// Les paquets à installer avec celui-ci (usr/share/grenos/depots/<id>.avec),
/// chacun passé à la même règle de nom que l'identifiant.
pub fn paquets_avec(identifiant: &str) -> Vec<String> {
    if !identifiant_sain(identifiant) {
        return Vec::new();
    }
    std::fs::read_to_string(format!("/usr/share/grenos/depots/{identifiant}.avec"))
        .map(|t| t.split_whitespace().filter(|p| identifiant_sain(p)).map(String::from).collect())
        .unwrap_or_default()
}

/// Le premier « NN% » d'une ligne.
pub(crate) fn pour_cent(texte: &str) -> Option<u32> {
    let i = texte.find('%')?;
    // La fin du dernier caractère qui n'est pas un chiffre — PAS « j + 1 » :
    // ce caractère peut tenir sur plusieurs octets (la barre « ██42% » de
    // flatpak), et couper au milieu fait paniquer Rust. Trouvé par le fuzzing
    // (aleatoire.rs) le 18 novembre.
    let debut = texte[..i].char_indices().rev().find(|(_, c)| !c.is_ascii_digit()).map(|(j, c)| j + c.len_utf8()).unwrap_or(0);
    texte[debut..i].parse().ok().filter(|p| *p <= 100)
}

pub(crate) fn etape(ligne: &str) -> bool {
    ["Unpacking", "Dépaquetage", "Setting up", "Paramétrage", "Installing"].iter().any(|d| ligne.starts_with(d))
}

fn avec<'a>(debut: &[&'a str], reste: &[&'a str]) -> Vec<&'a str> {
    [debut, reste].concat()
}

/// Lance une commande et rapporte chaque ligne (flatpak réécrit la sienne
/// avec \r : on découpe sur les deux). Rend le code de sortie.
fn courir(programme: &str, arguments: &[&str], mut ligne: impl FnMut(&str)) -> bool {
    let Ok(mut enfant) = Command::new(programme)
        .args(arguments)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    else {
        ligne(&format!("{programme} : introuvable"));
        return false;
    };
    // stderr dans un fil : un tube d'erreurs plein bloquerait le programme.
    let erreurs = enfant.stderr.take().map(|mut e| {
        std::thread::spawn(move || {
            let mut t = String::new();
            let _ = e.read_to_string(&mut t);
            t
        })
    });
    if let Some(mut sortie) = enfant.stdout.take() {
        let mut tampon = [0u8; 1024];
        let mut courante = String::new();
        while let Ok(n) = sortie.read(&mut tampon) {
            if n == 0 {
                break;
            }
            for c in String::from_utf8_lossy(&tampon[..n]).chars() {
                if c == '\r' || c == '\n' {
                    if !courante.trim().is_empty() {
                        ligne(courante.trim_end());
                    }
                    courante.clear();
                } else {
                    courante.push(c);
                }
            }
        }
        if !courante.trim().is_empty() {
            ligne(courante.trim_end());
        }
    }
    if let Some(Ok(t)) = erreurs.map(|f| f.join()) {
        for l in t.lines().filter(|l| !l.trim().is_empty()) {
            ligne(l);
        }
    }
    enfant.wait().map(|s| s.success()).unwrap_or(false)
}

#[tauri::command]
pub fn installer(app: tauri::AppHandle, cle: String, nom: String, source: String, identifiant: String, mise_a_jour: bool) -> Result<(), String> {
    if !identifiant_sain(&identifiant) || !(source == "flatpak" || source == "apt") {
        return Err("Cette fiche n'est pas installable : identifiant ou source refusés.".into());
    }
    std::thread::spawn(move || {
        let emettre = |a: Avance| {
            let _ = app.emit("installation", a);
        };
        let avance = |part: Option<u32>, ligne: Option<String>, texte: Option<String>| Avance {
            cle: cle.clone(), part, ligne, texte, fin: false, bien: false,
        };
        emettre(avance(Some(2), None, Some(if mise_a_jour { "Téléchargement et mise à jour…" } else { "Téléchargement et installation…" }.into())));
        let mut faits = 0u32;
        let mut suivre = |l: &str| {
            let part = if let Some(p) = pour_cent(l) {
                Some(p)
            } else if etape(l) {
                faits += 1;
                Some((15 + faits * 100 / 30).min(95))
            } else {
                None
            };
            emettre(avance(part, Some(l.to_string()), None));
        };
        let bien = if source == "flatpak" {
            courir("flatpak", &["remote-add", "--if-not-exists", "flathub", "https://dl.flathub.org/repo/flathub.flatpakrepo"], &mut suivre);
            if mise_a_jour {
                courir("flatpak", &["update", "-y", "--noninteractive", &identifiant], &mut suivre)
            } else {
                courir("flatpak", &["install", "-y", "--noninteractive", "flathub", &identifiant], &mut suivre)
            }
        } else {
            let p = privilegie();
            let (programme, debut) = (p[0].as_str(), p[1..].iter().map(String::as_str).collect::<Vec<_>>());
            // Une application publiée dans le dépôt de son ÉDITEUR (Claude
            // Desktop, 20 novembre) : le dépôt n'est branché qu'ici, au clic
            // (voir grenos-depot-editeur), et ses paquets recommandés viennent
            // avec — pour Claude, le trousseau sans lequel on ne se connecte pas.
            let editeur = depot_editeur(&identifiant);
            // Ce qui s'installe AVEC (pilote NVIDIA, 20 novembre : les en-têtes
            // du noyau, sans lesquels son module ne se compile pas).
            let avec_lui = paquets_avec(&identifiant);
            if editeur && !courir(programme, &avec(&debut, &["/usr/lib/grenos/grenos-depot-editeur", &identifiant]), &mut suivre) {
                false
            } else {
                courir(programme, &avec(&debut, &["apt-get", "update"]), &mut suivre);
                let mut commande = vec!["apt-get", "install", "-y"];
                if mise_a_jour {
                    commande.push("--only-upgrade");
                }
                if editeur {
                    commande.push("--install-recommends");
                }
                commande.extend(avec_lui.iter().map(String::as_str));
                commande.push(&identifiant);
                let pose = courir(programme, &avec(&debut, &commande), &mut suivre);
                // Un pilote : son module est-il compilé pour CE noyau ? Sinon
                // le script revient au pilote libre, et c'est un échec dit.
                if pose && editeur {
                    courir(programme, &avec(&debut, &["/usr/lib/grenos/grenos-depot-editeur", "--apres", &identifiant]), &mut suivre)
                } else {
                    pose
                }
            }
        };
        let texte = match (bien, mise_a_jour) {
            (true, true) => format!("{nom} est à jour."),
            (true, false) => format!("{nom} est installé. Il est dans le menu."),
            (false, true) => "La mise à jour s'est arrêtée. Le détail est ci-dessous.".into(),
            (false, false) => "L'installation s'est arrêtée. Le détail est ci-dessous.".into(),
        };
        dire(&format!("installation : {identifiant} {}", if bien { "ok" } else { "echec" }));
        emettre(Avance { cle: cle.clone(), part: Some(100), ligne: None, texte: Some(texte), fin: true, bien });
    });
    Ok(())
}

/// Cette application est-elle déjà là ? (`flatpak info`, ou dpkg.)
pub fn installe(source: &str, identifiant: &str) -> bool {
    if !identifiant_sain(identifiant) {
        return false;
    }
    if source == "flatpak" {
        crate::systeme::reussit("flatpak", &["info", identifiant])
    } else {
        crate::systeme::sortie("dpkg-query", &["-W", "-f=${Status}", identifiant]).starts_with("install ok")
    }
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn identifiants() {
        assert!(identifiant_sain("com.valvesoftware.Steam"));
        assert!(identifiant_sain("g++"));
        assert!(identifiant_sain("nvidia-driver"));
        assert!(!identifiant_sain("-y"));
        assert!(!identifiant_sain("a;rm"));
        assert!(!identifiant_sain(""));
        assert!(!identifiant_sain("a b"));
    }

    #[test]
    fn avance_lue() {
        assert_eq!(pour_cent("Installing 3/7… ████  42%  1.2 MB/s"), Some(42));
        assert_eq!(pour_cent("rien"), None);
        assert!(etape("Setting up firefox-esr (128.0)"));
        assert!(!etape("Get:1 http://deb.debian.org"));
    }
}
