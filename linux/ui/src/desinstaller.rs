//! Désinstaller une application : Réglages › Applications (25 novembre).
//!
//! Grenofar : « ajoute la possibilité de désinstaller des apps, dans
//! paramètres — applications ». La page liste ce que la personne a ajouté —
//! les Flatpak, et les paquets Debian qui ont une entrée dans le menu — et le
//! retire en un clic, comme « Applications installées » de Windows.
//!
//! LA RÈGLE QUI TIENT TOUT : ce qui fait grenOS ne se retire pas d'ici.
//! `grenos-systeme` nomme chaque paquet de l'image et `grenos-desktop` en
//! dépend : retirer Firefox emporterait grenos-systeme, puis grenos-desktop —
//! tout le bureau, la panne de calamares du 21 novembre (make-deb.sh). Donc :
//!   - ce que ces deux paquets nomment est rangé à part (« Livrées avec
//!     grenOS »), sans bouton ;
//!   - avant de retirer un paquet, apt SIMULE le retrait (`apt-get -s`) : si
//!     un seul paquet de grenOS partirait avec, c'est non ;
//!   - la désinstallation ne croit pas la page : elle refait la liste et
//!     refuse ce qui n'y est pas proposé.
//! Comme dans installer.rs, l'identifiant passe `identifiant_sain` : rien de
//! libre n'atteint la ligne de commande.

use crate::installer::identifiant_sain;
use crate::systeme::{dire, privilegie, sortie};
use crate::taches::{champ, nom_de};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Installee {
    /// « flatpak » ou « apt ».
    source: String,
    identifiant: String,
    nom: String,
    description: String,
    version: String,
    /// Déjà lisible : « 412,3 Mo ».
    taille: String,
    /// Pour un Flatpak : « system » ou « user ». Vide pour un paquet.
    installation: String,
    /// Une adresse data:, ou rien.
    icone: String,
    /// Ses entrées du menu (« gimp.desktop ») : le clic droit du menu
    /// Démarrer ouvre cette page sur l'une d'elles (26 novembre).
    entrees: Vec<String>,
}

fn maison() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into()))
}

/// « 412,3 Mo », à la française et en puissances de mille, comme la page
/// Partitions (octets() dans reglages.js).
pub(crate) fn taille_lisible(octets: u64) -> String {
    const UNITES: [&str; 5] = ["o", "Ko", "Mo", "Go", "To"];
    let mut v = octets as f64;
    let mut i = 0;
    while v >= 1000.0 && i < UNITES.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    let chiffre = if i > 0 && v < 10.0 { format!("{v:.1}") } else { format!("{v:.0}") };
    format!("{} {}", chiffre.replace('.', ","), UNITES[i])
}

// ---- Ce que grenOS protège ----------------------------------------------------

/// Les noms d'un champ Depends/Recommends : « a (>= 1), b | c, d:any » → a, b, c, d.
pub(crate) fn noms_dependances(texte: &str) -> HashSet<String> {
    texte
        .split([',', '|', '\n'])
        .filter_map(|d| d.split_whitespace().next())
        .map(|d| d.split(':').next().unwrap_or(d).to_string())
        .filter(|d| identifiant_sain(d))
        .collect()
}

/// Ce que grenOS est : tout ce que nomment grenos-systeme (l'image) et
/// grenos-desktop (le bureau, et ce qu'il recommande — le terminal, le
/// gestionnaire de fichiers, flatpak).
fn proteges() -> HashSet<String> {
    noms_dependances(&sortie("dpkg-query", &["-W", "-f=${Depends}, ${Recommends}\n", "grenos-systeme", "grenos-desktop"]))
}

fn de_grenos(paquet: &str, proteges: &HashSet<String>) -> bool {
    paquet.starts_with("grenos") || proteges.contains(paquet)
}

/// Les paquets qu'un `apt-get -s remove` retirerait (« Remv nom [version] »).
pub(crate) fn retires_de(texte: &str) -> Vec<String> {
    texte
        .lines()
        .filter_map(|l| l.strip_prefix("Remv "))
        .filter_map(|r| r.split_whitespace().next())
        .map(|p| p.split(':').next().unwrap_or(p).to_string())
        .filter(|p| identifiant_sain(p))
        .collect()
}

/// Ce qui partirait AVEC le paquet, ou le refus si grenOS en ferait partie.
pub(crate) fn juger_retrait(paquet: &str, retires: &[String], proteges: &HashSet<String>) -> Result<Vec<String>, String> {
    if !retires.iter().any(|r| r == paquet) {
        return Err("apt ne sait pas retirer ce paquet : il n'est peut-être plus installé.".into());
    }
    let touches: Vec<&str> = retires.iter().map(String::as_str).filter(|r| de_grenos(r, proteges)).collect();
    if !touches.is_empty() {
        return Err(format!(
            "La retirer emporterait aussi {} : ce morceau de grenOS reste, donc elle aussi.",
            touches.join(", ")
        ));
    }
    Ok(retires.iter().filter(|r| *r != paquet).cloned().collect())
}

fn simuler_retrait(paquet: &str) -> Result<Vec<String>, String> {
    let texte = sortie("apt-get", &["-s", "-q", "remove", paquet]);
    juger_retrait(paquet, &retires_de(&texte), &proteges())
}

// ---- Les Flatpak --------------------------------------------------------------

/// `flatpak list --app --columns=application,name,version,size,installation`,
/// hors terminal : une application par ligne, colonnes séparées par des
/// tabulations.
pub(crate) fn flatpaks_de(texte: &str) -> Vec<Installee> {
    texte
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').map(str::trim).collect();
            let (id, installation) = (*c.first()?, *c.get(4)?);
            (identifiant_sain(id) && matches!(installation, "system" | "user")).then(|| Installee {
                source: "flatpak".into(),
                identifiant: id.into(),
                nom: c.get(1).filter(|n| !n.is_empty()).unwrap_or(&id).to_string(),
                version: c.get(2).unwrap_or(&"").to_string(),
                taille: c.get(3).unwrap_or(&"").to_string(),
                installation: installation.into(),
                entrees: vec![format!("{id}.desktop")],
                ..Default::default()
            })
        })
        .collect()
}

fn exports(installation: &str) -> PathBuf {
    if installation == "user" { maison().join(".local/share/flatpak/exports/share") } else { PathBuf::from("/var/lib/flatpak/exports/share") }
}

/// Les entrées du menu qu'une Flatpak exporte. Flatpak exige qu'elles
/// commencent par son identifiant : LibreOffice en a sept (.calc, .writer…),
/// et le clic droit sur « LibreOffice Calc » doit trouver LibreOffice. Une
/// entrée qui commence par l'identifiant, plus long, d'une autre est à l'autre.
pub(crate) fn entrees_flatpak(id: &str, noms: &[String], autres: &[&str]) -> Vec<String> {
    let a_lui = |n: &str, i: &str| n == format!("{i}.desktop") || (n.starts_with(&format!("{i}.")) && n.ends_with(".desktop"));
    let principale = format!("{id}.desktop");
    let mut v: Vec<String> = noms
        .iter()
        .filter(|n| a_lui(n, id) && !autres.iter().any(|o| o.len() > id.len() && a_lui(n, o)))
        .cloned()
        .collect();
    v.sort_by_key(|n| (*n != principale, n.clone()));
    if v.is_empty() {
        v.push(principale);
    }
    v
}

fn flatpaks() -> Vec<Installee> {
    let mut liste = flatpaks_de(&sortie("flatpak", &["list", "--app", "--columns=application,name,version,size,installation"]));
    let ids: Vec<String> = liste.iter().map(|a| a.identifiant.clone()).collect();
    let autres: Vec<&str> = ids.iter().map(String::as_str).collect();
    for a in liste.iter_mut() {
        let racine = exports(&a.installation);
        let noms: Vec<String> = fs::read_dir(racine.join("applications"))
            .map(|l| l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        a.entrees = entrees_flatpak(&a.identifiant, &noms, &autres);
        let texte = fs::read_to_string(racine.join(format!("applications/{}.desktop", a.identifiant))).unwrap_or_default();
        a.description = commentaire(&texte);
        let icone = champ(&texte, "Icon");
        a.icone = icone_de(if icone.is_empty() { &a.identifiant } else { &icone }, &[racine]);
    }
    liste
}

/// La taille de ce que l'application a gardé dans le dossier personnel
/// (~/.var/app/<id>) — pour Steam, les jeux eux-mêmes. Bornée à huit
/// secondes : un dossier de 500 Go se compte lentement.
fn donnees_flatpak(identifiant: &str) -> Option<(PathBuf, Option<u64>)> {
    let dossier = maison().join(".var/app").join(identifiant);
    if !identifiant_sain(identifiant) || !dossier.is_dir() {
        return None;
    }
    let chemin = dossier.to_string_lossy().into_owned();
    let taille = sortie("timeout", &["8", "du", "-sb", "--", &chemin]).split_whitespace().next().and_then(|t| t.parse().ok());
    Some((dossier, taille))
}

// ---- Les paquets Debian -------------------------------------------------------

/// `dpkg-query -S chemin…` : « paquet[:arch][, autre]: /chemin ».
pub(crate) fn proprietaires_de(texte: &str) -> HashMap<String, Vec<String>> {
    let mut m = HashMap::new();
    for l in texte.lines() {
        if l.starts_with("diversion") {
            continue;
        }
        let Some((paquets, chemin)) = l.split_once(": /") else { continue };
        let p: Vec<String> = paquets
            .split(',')
            .map(|p| p.trim().split(':').next().unwrap_or("").to_string())
            .filter(|p| identifiant_sain(p))
            .collect();
        if !p.is_empty() {
            m.insert(format!("/{}", chemin.trim()), p);
        }
    }
    m
}

/// `dpkg-query -W -f='${Package}\t${Version}\t${Installed-Size}\n'` : la
/// version et la taille (en Kio) de chaque paquet.
pub(crate) fn versions_de(texte: &str) -> HashMap<String, (String, Option<u64>)> {
    texte
        .lines()
        .filter_map(|l| {
            let mut c = l.split('\t');
            let (p, v) = (c.next()?.trim(), c.next()?.trim());
            let k = c.next().and_then(|k| k.trim().parse::<u64>().ok());
            identifiant_sain(p).then(|| (p.to_string(), (v.to_string(), k.map(|k| k * 1024))))
        })
        .collect()
}

fn commentaire(texte: &str) -> String {
    let c = champ(texte, "Comment[fr]");
    if c.is_empty() { champ(texte, "Comment") } else { c }
}

/// Les entrées du menu posées par dpkg : (chemin, texte du .desktop).
fn entrees_du_menu() -> Vec<(String, String)> {
    let Ok(entrees) = fs::read_dir("/usr/share/applications") else { return Vec::new() };
    let mut liste: Vec<(String, String)> = entrees
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".desktop"))
        .filter_map(|e| {
            let texte = fs::read_to_string(e.path()).ok()?;
            let cache = ["NoDisplay", "Hidden"].iter().any(|c| champ(&texte, c) == "true");
            (!cache && champ(&texte, "Type") == "Application" && !nom_de(&texte).is_empty())
                .then(|| (e.path().to_string_lossy().into_owned(), texte))
        })
        .collect();
    liste.sort();
    liste
}

/// Les paquets qu'apt a posés comme dépendances (« Auto-Installed: 1 »).
/// Lu dans le fichier : `apt-mark showmanual` reconstruit tout le cache
/// d'apt, dix secondes sur grenOS, qui n'en garde pas (00grenos-sans-cache-binaire).
pub(crate) fn auto_installes_de(texte: &str) -> HashSet<String> {
    let mut auto = HashSet::new();
    for bloc in texte.split("\n\n") {
        let nom = bloc.lines().find_map(|l| l.strip_prefix("Package:")).map(|n| n.trim().to_string());
        if let (Some(n), true) = (nom, bloc.lines().any(|l| l.trim() == "Auto-Installed: 1")) {
            auto.insert(n);
        }
    }
    auto
}

/// Le nom du fichier d'une entrée : « /usr/share/applications/gimp.desktop » → « gimp.desktop ».
fn nom_fichier(chemin: &str) -> String {
    Path::new(chemin).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Ce que grenOS livre : leurs noms (la page les range à part), et les
/// fichiers de leurs entrées (le clic droit sur l'une d'elles le dit).
#[derive(Default)]
struct Livrees {
    noms: Vec<String>,
    entrees: Vec<String>,
}

/// (retirables, livrées avec grenOS). Un paquet est retirable s'il a été
/// installé à la demande (pas comme dépendance), s'il a une entrée dans le
/// menu, et si grenOS ne le nomme pas.
fn paquets() -> (Vec<Installee>, Livrees) {
    let entrees = entrees_du_menu();
    if entrees.is_empty() {
        return (Vec::new(), Livrees::default());
    }
    let mut arguments = vec!["-S"];
    arguments.extend(entrees.iter().map(|(c, _)| c.as_str()));
    let proprietaires = proprietaires_de(&sortie("dpkg-query", &arguments));
    let auto = auto_installes_de(&fs::read_to_string("/var/lib/apt/extended_states").unwrap_or_default());
    let proteges = proteges();

    // Un paquet, ses entrées. La principale est celle qui porte son nom si
    // elle existe, sinon la première (libreoffice-calc a la sienne, un jeu
    // de paquets aussi).
    let mut par_paquet: BTreeMap<String, Vec<&(String, String)>> = BTreeMap::new();
    for e in &entrees {
        for p in proprietaires.get(&e.0).into_iter().flatten() {
            let liste = par_paquet.entry(p.clone()).or_default();
            let id = Path::new(&e.0).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if id == *p { liste.insert(0, e) } else { liste.push(e) }
        }
    }

    let mut livrees = Livrees::default();
    let mut choisis: Vec<(&String, &Vec<&(String, String)>)> = Vec::new();
    for (p, es) in &par_paquet {
        if de_grenos(p, &proteges) {
            for e in es {
                let fichier = nom_fichier(&e.0);
                // Les fenêtres de grenOS lui-même n'ont rien à faire dans cette liste.
                if !fichier.starts_with("gren") {
                    livrees.noms.push(nom_de(&e.1));
                }
                livrees.entrees.push(fichier);
            }
        } else if !auto.contains(p.as_str()) {
            choisis.push((p, es));
        }
    }
    let mut arguments = vec!["-W", "-f=${Package}\t${Version}\t${Installed-Size}\n"];
    arguments.extend(choisis.iter().map(|(p, _)| p.as_str()));
    let versions = if choisis.is_empty() { HashMap::new() } else { versions_de(&sortie("dpkg-query", &arguments)) };
    let racines = [PathBuf::from("/usr/share")];
    let retirables = choisis
        .into_iter()
        .map(|(p, es)| {
            let texte = &es[0].1;
            let (version, taille) = versions.get(p.as_str()).cloned().unwrap_or_default();
            Installee {
                source: "apt".into(),
                identifiant: p.clone(),
                nom: nom_de(texte),
                description: commentaire(texte),
                version,
                taille: taille.map(taille_lisible).unwrap_or_default(),
                installation: String::new(),
                icone: icone_de(&champ(texte, "Icon"), &racines),
                entrees: es.iter().map(|e| nom_fichier(&e.0)).collect(),
            }
        })
        .collect();
    livrees.noms.sort_by_key(|n| n.to_lowercase());
    livrees.noms.dedup();
    (retirables, livrees)
}

// ---- Les icônes ---------------------------------------------------------------

const TAILLES: &[&str] = &["128x128", "256x256", "96x96", "64x64", "512x512", "48x48", "scalable"];

/// L'icône d'une entrée du menu, en adresse data: (la page n'a pas le droit
/// de lire le disque). Un chemin absolu, ou un nom cherché dans hicolor,
/// Papirus puis pixmaps.
fn icone_de(nom: &str, racines: &[PathBuf]) -> String {
    if nom.is_empty() || nom.contains("..") {
        return String::new();
    }
    let mut candidats: Vec<PathBuf> = Vec::new();
    if nom.starts_with('/') {
        candidats.push(PathBuf::from(nom));
    } else if !nom.contains('/') {
        for r in racines {
            for t in TAILLES {
                for ext in ["png", "svg"] {
                    candidats.push(r.join(format!("icons/hicolor/{t}/apps/{nom}.{ext}")));
                }
            }
        }
        candidats.push(PathBuf::from(format!("/usr/share/icons/Papirus/64x64/apps/{nom}.svg")));
        for ext in ["png", "svg"] {
            candidats.push(PathBuf::from(format!("/usr/share/pixmaps/{nom}.{ext}")));
        }
        candidats.push(PathBuf::from(format!("/usr/share/pixmaps/{nom}")));
    }
    for c in candidats {
        let type_mime = match c.extension().and_then(|e| e.to_str()) {
            Some("png") => "image/png",
            Some("svg") => "image/svg+xml",
            _ => continue,
        };
        if let Ok(octets) = fs::read(&c) {
            if !octets.is_empty() && octets.len() < 400_000 {
                return format!("data:{type_mime};base64,{}", crate::fichiers::base64(&octets));
            }
        }
    }
    String::new()
}

// ---- Les commandes ------------------------------------------------------------

#[tauri::command]
pub async fn reglages_applications() -> Value {
    let mut applications = flatpaks();
    let (debian, livrees) = paquets();
    applications.extend(debian);
    applications.sort_by_key(|a| a.nom.to_lowercase());
    dire(&format!("reglages applications : {} retirable(s), {} livree(s) avec grenOS", applications.len(), livrees.noms.len()));
    json!({ "applications": applications, "livrees": livrees.noms, "livrees_entrees": livrees.entrees })
}

/// L'application est-elle dans ce que la page propose de retirer ? La page
/// ne décide rien : on refait la liste.
fn proposee(source: &str, identifiant: &str, installation: &str) -> Option<Installee> {
    if source == "flatpak" {
        flatpaks().into_iter().find(|a| a.identifiant == identifiant && a.installation == installation)
    } else {
        paquets().0.into_iter().find(|a| a.identifiant == identifiant)
    }
}

fn verifier(source: &str, identifiant: &str, installation: &str) -> Result<Installee, String> {
    if !identifiant_sain(identifiant) || !matches!(source, "flatpak" | "apt") {
        return Err("Cette application n'est pas désinstallable d'ici.".into());
    }
    proposee(source, identifiant, installation).ok_or_else(|| "Cette application n'est plus installée, ou fait partie de grenOS.".into())
}

/// Avant de demander « Sûr ? » : ce qui partirait avec, ce qu'elle a gardé.
#[tauri::command]
pub async fn reglages_desinstaller_apercu(source: String, identifiant: String, installation: Option<String>) -> Value {
    let installation = installation.unwrap_or_default();
    if let Err(souci) = verifier(&source, &identifiant, &installation) {
        return json!({ "refus": souci });
    }
    if source == "flatpak" {
        let donnees = donnees_flatpak(&identifiant);
        return json!({
            "avec": [],
            "donnees": donnees.is_some(),
            "taille_donnees": donnees.and_then(|(_, t)| t).map(taille_lisible).unwrap_or_default(),
        });
    }
    match simuler_retrait(&identifiant) {
        Ok(avec) => json!({ "avec": avec, "donnees": false }),
        Err(refus) => json!({ "refus": refus }),
    }
}

/// Une commande, sa sortie entière (les deux flux), bornée dans le temps.
fn courir(secondes: &str, programme: &[String]) -> (bool, String) {
    let mut arguments = vec![secondes.to_string()];
    arguments.extend(programme.iter().cloned());
    match Command::new("timeout").args(&arguments).env("DEBIAN_FRONTEND", "noninteractive").stdin(Stdio::null()).output() {
        Ok(o) => (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))),
        Err(e) => (false, e.to_string()),
    }
}

/// Ce qui restait d'elle sous le même nom que ses entrées : les raccourcis
/// que « Ajouter au bureau » du menu avait posés (barre.rs,
/// poser_sur_le_bureau), et la copie que certaines se font dans
/// ~/.local/share/applications (Claude) — celle-là masquerait le retrait et
/// la laisserait dans le menu. Sans l'application, rien de tout ça
/// n'ouvrirait plus rien. Rend le nombre de raccourcis du bureau retirés.
fn retirer_raccourcis(entrees: &[String]) -> usize {
    let (bureau, menu) = (maison().join("Bureau"), maison().join(".local/share/applications"));
    let mut retires = 0;
    for e in entrees.iter().filter(|e| e.ends_with(".desktop") && identifiant_sain(e)) {
        if fs::remove_file(bureau.join(e)).is_ok() {
            retires += 1;
        }
        let _ = fs::remove_file(menu.join(e));
    }
    retires
}

fn dernieres_lignes(texte: &str, n: usize) -> String {
    let lignes: Vec<&str> = texte.lines().filter(|l| !l.trim().is_empty()).collect();
    lignes[lignes.len().saturating_sub(n)..].join("\n")
}

#[tauri::command]
pub async fn reglages_desinstaller(source: String, identifiant: String, installation: Option<String>, donnees: Option<bool>) -> Result<Value, String> {
    let installation = installation.unwrap_or_default();
    let appli = verifier(&source, &identifiant, &installation)?;
    let (bien, journal) = if source == "flatpak" {
        let ou = if installation == "user" { "--user" } else { "--system" };
        let mut commande: Vec<String> = ["flatpak", "uninstall", "-y", "--noninteractive", ou].map(String::from).to_vec();
        if donnees.unwrap_or(false) {
            commande.push("--delete-data".into());
        }
        commande.push(identifiant.clone());
        let (bien, mut journal) = courir("600", &commande);
        // Les bibliothèques (runtimes) que plus personne n'utilise : souvent
        // plus lourdes que l'application elle-même.
        if bien {
            let (_, suite) = courir("600", &["flatpak", "uninstall", "--unused", "-y", "--noninteractive", ou].map(String::from));
            journal.push_str(&suite);
        }
        (bien, journal)
    } else {
        // Le juge, encore, au moment d'agir : rien n'a pu changer entre-temps ?
        simuler_retrait(&identifiant)?;
        let mut commande = privilegie();
        commande.extend(["apt-get", "remove", "-y", identifiant.as_str()].map(String::from));
        courir("900", &commande)
    };
    dire(&format!("desinstallation : {source} {identifiant} {}", if bien { "ok" } else { "echec" }));
    if bien {
        let raccourcis = retirer_raccourcis(&appli.entrees);
        return Ok(json!({ "fait": format!("{} est désinstallée.", appli.nom), "souci": "", "raccourcis": raccourcis }));
    }
    let souci = if journal.contains("Could not get lock") || journal.contains("Impossible d'obtenir le verrou") {
        "Une installation ou une mise à jour est en cours : réessaie quand elle aura fini.".to_string()
    } else if journal.contains("Not authorized") || journal.contains("Request dismissed") || journal.contains("Error executing command as another user") {
        "Le mot de passe administrateur n'a pas été donné : rien n'a été retiré.".to_string()
    } else {
        "La désinstallation s'est arrêtée. Le détail est ci-dessous.".to_string()
    };
    Ok(json!({ "fait": "", "souci": souci, "detail": dernieres_lignes(&journal, 40) }))
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn les_flatpak_se_lisent() {
        let l = flatpaks_de("com.discordapp.Discord\tDiscord\t0.0.76\t312,4 Mo\tsystem\norg.gimp.GIMP\t\t3.0\t1 Go\tuser\n-y\tPiège\t1\t1\tsystem\norg.x.Y\tY\t1\t1\tailleurs\nmal formée\n");
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].identifiant, "com.discordapp.Discord");
        assert_eq!(l[0].nom, "Discord");
        assert_eq!(l[0].taille, "312,4 Mo");
        assert_eq!(l[0].installation, "system");
        // Son entrée du menu : ce que le clic droit du menu Démarrer désigne.
        assert_eq!(l[0].entrees, vec!["com.discordapp.Discord.desktop"]);
        // Sans nom, l'identifiant le remplace.
        assert_eq!(l[1].nom, "org.gimp.GIMP");
        assert_eq!(l[1].installation, "user");
    }

    #[test]
    fn les_entrees_d_une_flatpak() {
        let noms: Vec<String> = ["org.libreoffice.LibreOffice.desktop", "org.libreoffice.LibreOffice.calc.desktop", "org.libreoffice.LibreOffice.writer.desktop",
            "com.opera.Opera.desktop", "com.opera.opera-gx.desktop", "org.x.A.desktop", "org.x.A.B.desktop", "org.x.A.B.outil.desktop", "org.libreoffice.LibreOffice.png"]
            .map(String::from).to_vec();
        let autres = ["org.libreoffice.LibreOffice", "com.opera.Opera", "com.opera.opera-gx", "org.x.A", "org.x.A.B"];
        let lo = entrees_flatpak("org.libreoffice.LibreOffice", &noms, &autres);
        assert_eq!(lo, ["org.libreoffice.LibreOffice.desktop", "org.libreoffice.LibreOffice.calc.desktop", "org.libreoffice.LibreOffice.writer.desktop"]);
        // Opera n'emporte pas Opera GX ; org.x.A n'emporte pas les entrées de org.x.A.B.
        assert_eq!(entrees_flatpak("com.opera.Opera", &noms, &autres), ["com.opera.Opera.desktop"]);
        assert_eq!(entrees_flatpak("org.x.A", &noms, &autres), ["org.x.A.desktop"]);
        assert_eq!(entrees_flatpak("org.x.A.B", &noms, &autres), ["org.x.A.B.desktop", "org.x.A.B.outil.desktop"]);
        // Rien d'exporté : l'entrée attendue quand même.
        assert_eq!(entrees_flatpak("org.y.Z", &[], &autres), ["org.y.Z.desktop"]);
    }

    #[test]
    fn les_dependances_se_lisent() {
        let d = noms_dependances("grenos-systeme (= 20261125), libwebkit2gtk-4.1-0, python3:any, a | b, ${Recommends}\nflatpak, lightdm");
        for n in ["grenos-systeme", "libwebkit2gtk-4.1-0", "python3", "a", "b", "flatpak", "lightdm"] {
            assert!(d.contains(n), "{n} manque");
        }
        assert!(!d.iter().any(|n| n.contains('$') || n.contains('(')));
    }

    #[test]
    fn les_proprietaires_se_lisent() {
        let m = proprietaires_de("firefox-esr: /usr/share/applications/firefox-esr.desktop\nlibfoo:amd64, libbar: /usr/share/applications/foo.desktop\ndiversion by x from: /usr/share/applications/z.desktop\ndpkg-query: no path found matching pattern /usr/share/applications/w.desktop\n");
        assert_eq!(m["/usr/share/applications/firefox-esr.desktop"], vec!["firefox-esr"]);
        assert_eq!(m["/usr/share/applications/foo.desktop"], vec!["libfoo", "libbar"]);
        assert!(!m.contains_key("/usr/share/applications/z.desktop"));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn versions_et_tailles() {
        let v = versions_de("gimp\t3.0.2-1\t81234\ndiscord\t0.0.76\t\n");
        assert_eq!(v["gimp"], ("3.0.2-1".to_string(), Some(81234 * 1024)));
        assert_eq!(v["discord"].1, None);
        assert_eq!(taille_lisible(83_183_616), "83 Mo");
        assert_eq!(taille_lisible(1_500_000_000), "1,5 Go");
        assert_eq!(taille_lisible(512), "512 o");
    }

    #[test]
    fn les_dependances_posees_par_apt() {
        let a = auto_installes_de("Package: openvpn\nArchitecture: amd64\nAuto-Installed: 1\n\nPackage: gimp\nArchitecture: amd64\nAuto-Installed: 0\n");
        assert!(a.contains("openvpn") && !a.contains("gimp"));
    }

    #[test]
    fn grenos_ne_part_jamais_avec_une_application() {
        let proteges: HashSet<String> = ["firefox-esr", "atril"].map(String::from).into();
        let r = |t: &str| retires_de(t);
        // Un retrait simple : rien d'autre ne part.
        assert_eq!(juger_retrait("gimp", &r("NOTE: simulation\nRemv gimp [3.0]\n"), &proteges), Ok(vec![]));
        // Une extension part avec : elle est annoncée.
        assert_eq!(juger_retrait("gimp", &r("Remv gimp-help-fr [3.0]\nRemv gimp [3.0]\n"), &proteges), Ok(vec!["gimp-help-fr".to_string()]));
        // Le métapaquet partirait : refus.
        assert!(juger_retrait("firefox-esr", &r("Remv grenos-desktop [1]\nRemv grenos-systeme [1]\nRemv firefox-esr [128]\n"), &proteges).is_err());
        // Un paquet de l'image partirait avec : refus.
        assert!(juger_retrait("x", &r("Remv atril [1]\nRemv x [1]\n"), &proteges).is_err());
        // apt ne retirerait même pas le paquet demandé : refus.
        assert!(juger_retrait("gimp", &r("rien\n"), &proteges).is_err());
    }
}
