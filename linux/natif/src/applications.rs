//! Les applications du système, lues dans les fichiers .desktop, et les
//! épinglées de la barre (`~/.config/grenos/barre.json`).
//!
//! Port fidèle de `lire_desktop`, `applications`, `meme_application` de
//! grenos-shell : la CI (`verif-recherches.py`) vérifie la règle du menu
//! contre ces mêmes champs, il ne fallait pas en inventer une autre.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Ce que personne ne cherche dans une liste d'applications. On compare au
/// PROGRAMME lancé autant qu'au nom du fichier : Debian livre
/// `org.pulseaudio.pavucontrol.desktop`, pas `pavucontrol.desktop`.
pub const HORS_LISTE: &[&str] = &[
    "pavucontrol", "nm-connection-editor", "galculator", "calamares",
    "grenos-installer", "grenos-theme", "grenos-maj", "xfce4-terminal-settings",
    "xfce4-settings-manager", "vim", "gvim", "xarchiver", "debian-installer",
    "qvidcap", "qdbusviewer", "lstopo", "assistant", "designer", "linguist",
    "xfce4-about", "openbox", "uxterm", "xterm", "display-im6.q16", "feh",
    "lightdm-gtk-greeter", "qv4l2", "pcmanfm-desktop-pref",
];

#[derive(Clone, Debug)]
pub struct Application {
    pub nom: String,
    pub note: String,
    pub commande: String,
    pub icone: String,
    pub classe: String,
    pub fichier: PathBuf,
    pub cache: bool,
    pub terminal: bool,
}

/// Les champs de la section [Desktop Entry].
pub fn champs_desktop(texte: &str) -> HashMap<String, String> {
    let mut champs = HashMap::new();
    let mut dans = false;
    for ligne in texte.lines() {
        let ligne = ligne.trim();
        if ligne.starts_with('[') {
            dans = ligne == "[Desktop Entry]";
        } else if dans {
            if let Some((c, v)) = ligne.split_once('=') {
                champs.insert(c.to_string(), v.to_string());
            }
        }
    }
    champs
}

/// La commande d'un Exec=, sans les codes %f, %U…
pub fn nettoyer_exec(exec: &str) -> String {
    let mut c = exec.to_string();
    for code in ["%f", "%F", "%u", "%U", "%i", "%c", "%k"] {
        c = c.replace(code, "");
    }
    c.trim().to_string()
}

fn base(chemin: &str) -> String {
    Path::new(chemin).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn programme_de(commande: &str) -> String {
    commande.split_whitespace().next().map(base).unwrap_or_default()
}

/// Ce qu'un fichier .desktop dit d'une application (None : à ne pas montrer).
pub fn lire_desktop(chemin: &Path) -> Option<Application> {
    let texte = std::fs::read(chemin).ok()?;
    let champs = champs_desktop(&String::from_utf8_lossy(&texte));
    let vrai = |c: &str| champs.get(c).map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(false);
    if champs.get("Type").map(String::as_str) != Some("Application") || vrai("NoDisplay") || vrai("Hidden") {
        return None;
    }
    let nom = champs.get("Name[fr]").or_else(|| champs.get("Name"))?.clone();
    let commande = nettoyer_exec(champs.get("Exec")?);
    if nom.is_empty() || commande.is_empty() {
        return None;
    }
    let programme = programme_de(&commande);
    let souche = chemin.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let cache = HORS_LISTE.contains(&souche.as_str()) || HORS_LISTE.contains(&programme.as_str());
    let classe = champs.get("StartupWMClass").filter(|c| !c.is_empty()).cloned().unwrap_or_else(|| programme.clone());
    Some(Application {
        nom,
        note: champs.get("Comment[fr]").or_else(|| champs.get("Comment")).cloned().unwrap_or_default(),
        commande,
        icone: champs.get("Icon").cloned().unwrap_or_default(),
        classe,
        fichier: chemin.to_path_buf(),
        cache,
        terminal: vrai("Terminal"),
    })
}

/// Toutes les applications du système, triées par nom. Un même nom de
/// fichier plus loin dans la liste remplace le précédent (~/.local gagne).
pub fn applications() -> Vec<Application> {
    let maison = crate::maison();
    let dossiers = [
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        maison.join(".local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        maison.join(".local/share/flatpak/exports/share/applications"),
    ];
    let mut trouvees: HashMap<String, Application> = HashMap::new();
    for dossier in dossiers {
        let Ok(lecture) = std::fs::read_dir(&dossier) else { continue };
        let mut noms: Vec<String> = lecture.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        noms.sort();
        for nom in noms {
            if nom.ends_with(".desktop") {
                if let Some(a) = lire_desktop(&dossier.join(&nom)) {
                    trouvees.insert(nom, a);
                }
            }
        }
    }
    let mut liste: Vec<Application> = trouvees.into_values().collect();
    liste.sort_by_key(|a| a.nom.to_lowercase());
    liste
}

/// Ce que le menu montre pour une recherche : la règle exacte de grenos-shell
/// (nom ou note qui contient le texte ; sans texte, on cache les réglages).
pub fn visibles<'a>(apps: &'a [Application], filtre: &str) -> Vec<&'a Application> {
    let f = filtre.trim().to_lowercase();
    if f.is_empty() {
        return apps.iter().filter(|a| !a.cache).collect();
    }
    apps.iter().filter(|a| a.nom.to_lowercase().contains(&f) || a.note.to_lowercase().contains(&f)).collect()
}

// ---- Les réglages, trouvables depuis la recherche du menu --------------------
//
// Grenofar (23 novembre) : « quand on est dans la recherche depuis le dock,
// on tape partition et tout ». Comme Windows : taper un mot fait aussi
// apparaître la page des Réglages qui le règle, ouverte directement dessus
// (`grenos-parametres <page>`).

pub struct Reglage {
    pub nom: &'static str,
    pub page: &'static str,
    pub icone: &'static str,
    pub mots: &'static [&'static str],
}

impl Reglage {
    pub fn commande(&self) -> String {
        format!("grenos-parametres {}", self.page)
    }
}

pub const REGLAGES: &[Reglage] = &[
    Reglage { nom: "Partitions et disques", page: "partitions", icone: "drive-harddisk",
        mots: &["partition", "disque", "ssd", "hdd", "nvme", "formater", "formatage", "stockage", "clé usb", "usb", "monter", "démonter", "volume", "ext4", "ntfs", "exfat", "fat32", "espace", "redimensionner"] },
    Reglage { nom: "Écran — définition et fréquence", page: "ecran", icone: "video-display",
        mots: &["écran", "ecran", "résolution", "définition", "fréquence", "hz", "hertz", "fps", "images par seconde", "moniteur", "affichage", "rafraîchissement", "plusieurs écrans", "disposer"] },
    Reglage { nom: "Apparence — thème, fond, effets", page: "apparence", icone: "preferences-desktop-theme",
        mots: &["apparence", "thème", "theme", "sombre", "clair", "nuit", "jour", "fond d'écran", "fond", "couleur", "accent", "effets", "animation", "taille", "dpi", "barre des tâches", "dock", "police"] },
    Reglage { nom: "Son", page: "son", icone: "audio-volume-high",
        mots: &["son", "audio", "volume", "haut-parleur", "casque", "micro", "microphone", "sortie", "entrée"] },
    Reglage { nom: "Réseau — Wi-Fi et câble", page: "reseau", icone: "network-wireless",
        mots: &["réseau", "reseau", "wifi", "wi-fi", "internet", "ethernet", "câble", "ip", "dns", "vpn"] },
    Reglage { nom: "Compte — mot de passe, navigateur principal", page: "compte", icone: "avatar-default",
        mots: &["compte", "mot de passe", "utilisateur", "session", "navigateur", "navigateur principal", "navigateur par défaut", "google", "connexion", "clé de sécurité", "yubikey"] },
    Reglage { nom: "Heure et date", page: "heure", icone: "preferences-system-time",
        mots: &["heure", "date", "fuseau", "horloge", "temps"] },
    Reglage { nom: "Mises à jour", page: "maj", icone: "system-software-update",
        mots: &["mise à jour", "mises à jour", "maj", "update", "version"] },
    Reglage { nom: "Mise à jour des composants", page: "composants", icone: "application-x-firmware",
        mots: &["composant", "firmware", "micrologiciel", "bios", "uefi", "fwupd"] },
    Reglage { nom: "Sécurité — antivirus", page: "securite", icone: "security-high",
        mots: &["sécurité", "securite", "antivirus", "virus", "clamav", "pare-feu", "firewall"] },
    Reglage { nom: "Matériel — processeur, mémoire, carte graphique", page: "materiel", icone: "computer",
        mots: &["matériel", "materiel", "processeur", "cpu", "mémoire", "ram", "carte graphique", "gpu", "nvidia", "pilote", "température"] },
];

/// Sans accents ni majuscules : « Écran » se trouve en tapant « ecran ».
pub fn plier(texte: &str) -> String {
    texte
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            autre => autre,
        })
        .collect()
}

/// Les pages des Réglages qui répondent à ce qu'on tape (deux lettres au
/// moins : une seule ferait tout sortir).
pub fn reglages_trouves(filtre: &str) -> Vec<&'static Reglage> {
    let f = plier(filtre.trim());
    if f.chars().count() < 2 {
        return Vec::new();
    }
    REGLAGES
        .iter()
        .filter(|r| {
            plier(r.nom).contains(&f)
                || r.mots.iter().any(|m| {
                    let m = plier(m);
                    // « partitions » trouve « partition » ; pas « sonic » → « son ».
                    m.starts_with(&f) || (m.chars().count() >= 4 && f.starts_with(&m))
                })
        })
        .collect()
}

// ---- Les épinglées ------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Epingle {
    pub nom: String,
    pub commande: String,
    #[serde(default)]
    pub icone: String,
}

pub fn epingles_par_defaut() -> Vec<Epingle> {
    [
        ("Fichiers", "grenos-fichiers", "grenos-fichiers"),
        ("Web", "firefox-esr", "firefox-esr"),
        ("Terminal", "x-terminal-emulator", "utilities-terminal"),
        ("GrenPlace", "grenplace", "grenplace"),
    ]
    .iter()
    .map(|(n, c, i)| Epingle { nom: n.to_string(), commande: c.to_string(), icone: i.to_string() })
    .collect()
}

fn fichier_barre() -> PathBuf {
    crate::reglages().join("barre.json")
}

pub fn lire_epingles() -> Vec<Epingle> {
    std::fs::read_to_string(fichier_barre())
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<Epingle>>(&t).ok())
        .unwrap_or_else(epingles_par_defaut)
}

pub fn ecrire_epingles(e: &[Epingle]) {
    let _ = std::fs::create_dir_all(crate::reglages());
    if let Ok(t) = serde_json::to_string_pretty(e) {
        let _ = std::fs::write(fichier_barre(), t);
    }
}

/// Derrière « x-terminal-emulator », le programme qui s'ouvre vraiment.
pub fn vrai_programme(nom: &str) -> String {
    if !nom.starts_with("x-") {
        return nom.to_string();
    }
    std::fs::canonicalize(format!("/usr/bin/{nom}"))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| nom.to_string())
}

/// Cette fenêtre (classe, titre) appartient-elle à cette commande ?
/// Comparaison imparfaite mais identique à celle de grenos-shell.
pub fn meme_application(groupe: &str, titre: &str, commande: &str, apps: &[Application]) -> bool {
    let programme = programme_de(commande);
    if programme.is_empty() {
        return false;
    }
    let groupe = groupe.to_lowercase();
    let titre = titre.to_lowercase();
    let mut candidats = vec![programme.to_lowercase(), vrai_programme(&programme).to_lowercase()];
    for a in apps {
        if a.commande == commande && !a.classe.is_empty() {
            candidats.push(a.classe.to_lowercase());
        }
    }
    candidats.iter().filter(|c| !c.is_empty()).any(|c| c.contains(&groupe) || groupe.contains(c.as_str()) || titre.contains(c.as_str()))
}

/// L'application du système qui correspond à une classe de fenêtre.
pub fn application_de<'a>(groupe: &str, apps: &'a [Application]) -> Option<&'a Application> {
    let g = groupe.to_lowercase();
    apps.iter().find(|a| {
        let c = a.classe.to_lowercase();
        !c.is_empty() && (c.contains(&g) || g.contains(&c))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_recherche_trouve_les_reglages() {
        let pages = |f: &str| reglages_trouves(f).iter().map(|r| r.page).collect::<Vec<_>>();
        assert_eq!(pages("partition"), vec!["partitions"]);
        assert_eq!(pages("Partitions"), vec!["partitions"]);
        assert!(pages("ecran").contains(&"ecran"));
        assert!(pages("hz").contains(&"ecran"));
        assert!(pages("navigateur").contains(&"compte"));
        assert!(pages("formater").contains(&"partitions"));
        assert!(pages("sonic").is_empty());
        assert!(pages("p").is_empty());
    }

    #[test]
    fn un_desktop_se_lit() {
        let c = champs_desktop("[Desktop Entry]\nName=Firefox\nName[fr]=Web\nExec=firefox-esr %u\n[Desktop Action x]\nName=Autre\n");
        assert_eq!(c.get("Name[fr]").map(String::as_str), Some("Web"));
        assert_eq!(c.get("Name").map(String::as_str), Some("Firefox"));
        assert_eq!(nettoyer_exec(c.get("Exec").unwrap()), "firefox-esr");
    }

    #[test]
    fn la_comparaison_de_fenetres() {
        let apps = vec![];
        assert!(meme_application("Firefox-esr", "Accueil", "firefox-esr", &apps));
        assert!(meme_application("grenplace", "GrenPlace", "grenplace", &apps));
        assert!(!meme_application("Xfce4-terminal", "x", "grenplace", &apps));
        assert!(!meme_application("abc", "x", "", &apps));
    }

    #[test]
    fn la_recherche_suit_la_regle_du_menu() {
        let a = |nom: &str, note: &str, cache: bool| Application {
            nom: nom.into(), note: note.into(), commande: "x".into(), icone: String::new(),
            classe: String::new(), fichier: PathBuf::new(), cache, terminal: false,
        };
        let apps = vec![a("Firefox", "Le web", false), a("Volume", "son", true)];
        assert_eq!(visibles(&apps, "").len(), 1);
        assert_eq!(visibles(&apps, "SON").len(), 1);
        assert_eq!(visibles(&apps, "web").len(), 1);
    }

    #[test]
    fn les_epingles_par_defaut_tiennent() {
        let e = epingles_par_defaut();
        let texte = serde_json::to_string(&e).unwrap();
        let relu: Vec<Epingle> = serde_json::from_str(&texte).unwrap();
        assert_eq!(e, relu);
        // Le format Python (sans « icone ») se relit aussi.
        let ancien: Vec<Epingle> = serde_json::from_str(r#"[{"nom":"A","commande":"a"}]"#).unwrap();
        assert_eq!(ancien[0].icone, "");
    }
}
