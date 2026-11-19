//! L'Explorateur de fichiers : « Accès rapide » puis « Ce PC », l'ordre et les
//! mots de Windows, et ses raccourcis.
//!
//! Repris de `grenos-fichiers` (Python) le 18 novembre. Ce fichier fait ce qui
//! touche au disque ; la page (fichiers.js) tient les onglets, l'histoire, la
//! sélection et le presse-papier. Les règles qui protègent les fichiers de la
//! personne n'ont pas bougé :
//! - on n'ÉCRASE jamais en silence : « (copie) », « archive-2.zip » ;
//! - un nom inchangé n'est pas un conflit (F2 sans rien changer ne dit rien) ;
//! - on ne colle pas un dossier dans lui-même ;
//! - « Supprimer » met à la corbeille (GIO), jamais d'effacement direct ;
//! - une archive s'extrait dans un dossier à son nom, pas en vrac.
//!
//! Les lignes série sont celles du Python, au mot près : la CI tape Ctrl+H,
//! Ctrl+F, Ctrl+Maj+N (deux fois), F2, Ctrl+T et Ctrl+W, et les lit.

use crate::systeme::{detacher, dire};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

fn maison() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
}

#[derive(Serialize)]
pub struct Element {
    nom: String,
    chemin: String,
    dossier: bool,
    octets: u64,
    modifie: i64,
    genre: String,
    famille: &'static str,
}

/// Une famille d'icône pour la page, d'après le type MIME.
pub(crate) fn famille(mime: &str, nom: &str) -> &'static str {
    let bas = nom.to_lowercase();
    if est_une_archive(&bas) {
        "archive"
    } else if mime.starts_with("image/") {
        "image"
    } else if mime.starts_with("video/") {
        "video"
    } else if mime.starts_with("audio/") {
        "audio"
    } else if mime == "application/pdf" {
        "pdf"
    } else if mime.contains("script") || mime.contains("x-c") || mime.contains("json") || mime.contains("xml") {
        "code"
    } else if mime.starts_with("text/") || mime.contains("document") || mime.contains("opendocument") {
        "texte"
    } else if mime.contains("executable") || bas.ends_with(".desktop") || bas.ends_with(".appimage") {
        "programme"
    } else {
        "fichier"
    }
}

/// Les formats que l'image sait vraiment ouvrir (7zip, unzip, bzip2, xz,
/// unrar-free), et rien de plus : « Extraire ici » sur un format qu'aucun
/// outil ne lit serait une promesse vide.
pub fn est_une_archive(chemin: &str) -> bool {
    let bas = chemin.to_lowercase();
    [".zip", ".7z", ".rar", ".tar", ".gz", ".bz2", ".xz", ".tgz"].iter().any(|f| bas.ends_with(f))
}

/// Le mot de la colonne « Type », en français, par GIO et shared-mime-info
/// (le trou « folder » du 2 octobre est comblé par notre XML).
#[cfg(not(target_os = "linux"))]
fn genre_et_mime(_chemin: &Path) -> (String, String) {
    (String::new(), String::new())
}

#[cfg(target_os = "linux")]
fn genre_et_mime(chemin: &Path) -> (String, String) {
    let (mime, _sur) = gio::content_type_guess(Some(chemin), &[]);
    let description = gio::content_type_get_description(&mime).to_string();
    (description, gio::content_type_get_mime_type(&mime).map(|m| m.to_string()).unwrap_or_default())
}

#[derive(Serialize)]
pub struct Liste {
    dossier: String,
    elements: Vec<Element>,
    caches: usize,
}

#[tauri::command]
pub async fn fichiers_lister(dossier: String, caches: bool) -> Result<Liste, String> {
    let lu = fs::read_dir(&dossier).map_err(|e| format!("Ce dossier ne peut pas être lu : {e}"))?;
    let mut elements = Vec::new();
    let mut masques = 0;
    for e in lu.flatten() {
        let nom = e.file_name().to_string_lossy().into_owned();
        if nom.starts_with('.') {
            masques += 1;
            if !caches {
                continue;
            }
        }
        let chemin = e.path();
        // `metadata` suit les liens : un lien vers un dossier EST un dossier.
        let meta = fs::metadata(&chemin).or_else(|_| fs::symlink_metadata(&chemin));
        let Ok(meta) = meta else { continue };
        let est_dossier = meta.is_dir();
        let modifie = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
        let (genre, mime) = if est_dossier { ("Dossier de fichiers".to_string(), String::new()) } else { genre_et_mime(&chemin) };
        elements.push(Element {
            famille: if est_dossier { "dossier" } else { famille(&mime, &nom) },
            nom,
            chemin: chemin.to_string_lossy().into_owned(),
            dossier: est_dossier,
            octets: if est_dossier { 0 } else { meta.len() },
            modifie,
            genre,
        });
    }
    Ok(Liste { dossier, elements, caches: masques })
}

#[derive(Serialize, Debug)]
pub struct Rapide {
    nom: String,
    chemin: String,
    icone: &'static str,
    /// « » pour les dossiers de grenOS, « windows » pour ceux de Windows :
    /// le volet les range sous deux titres.
    groupe: &'static str,
}

/// « Accès rapide » : les dossiers de la personne qui existent vraiment.
#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub fn fichiers_rapides() -> Vec<Rapide> {
    Vec::new()
}

#[cfg(target_os = "linux")]
#[tauri::command]
pub fn fichiers_rapides() -> Vec<Rapide> {
    use glib::UserDirectory as U;
    let mut liste: Vec<Rapide> = [
        ("Bureau", U::Desktop, "bureau"),
        ("Téléchargements", U::Downloads, "telechargement"),
        ("Documents", U::Documents, "texte"),
        ("Images", U::Pictures, "image"),
        ("Musique", U::Music, "audio"),
        ("Vidéos", U::Videos, "video"),
    ]
    .into_iter()
    .filter_map(|(nom, cle, icone)| {
        let chemin = glib::user_special_dir(cle)?;
        chemin.is_dir().then(|| Rapide { nom: nom.to_string(), chemin: chemin.to_string_lossy().into_owned(), icone, groupe: "" })
    })
    .collect();
    // Windows, ouvert en lecture seule au démarrage par `grenos-windows`, qui
    // écrit où dans /run/grenos/windows. Pas de fichier : pas de Windows.
    if let Ok(racine) = fs::read_to_string("/run/grenos/windows") {
        liste.extend(dossiers_windows(Path::new(racine.trim())));
    }
    liste
}

/// Les dossiers de chaque compte de Windows, sous `racine/Users`.
///
/// Grenofar, le 19 novembre : « garde la base comme Windows — download,
/// images, vidéos ». Les comptes techniques de Windows (Public, Default…) ne
/// sont pas des personnes. OneDrive déplace souvent Bureau, Documents et
/// Images dans `OneDrive\…`, sous leur nom anglais OU français : on prend le
/// premier qui existe. Un seul compte : « Téléchargements » ; plusieurs : le
/// nom du compte suit, pour qu'on sache à qui est quoi.
pub fn dossiers_windows(racine: &Path) -> Vec<Rapide> {
    const TECHNIQUES: [&str; 5] = ["public", "default", "default user", "all users", "wdagutilityaccount"];
    let Ok(entrees) = fs::read_dir(racine.join("Users")) else { return Vec::new() };
    let mut comptes: Vec<PathBuf> = entrees
        .flatten()
        .filter(|e| {
            let nom = e.file_name().to_string_lossy().to_lowercase();
            // `file_type` ne suit pas les liens : « All Users » en est un.
            !TECHNIQUES.contains(&nom.as_str()) && e.file_type().map(|t| t.is_dir()).unwrap_or(false)
        })
        .map(|e| e.path())
        .filter(|p| p.join("Downloads").is_dir() || p.join("Desktop").is_dir())
        .collect();
    comptes.sort();
    let plusieurs = comptes.len() > 1;
    let mut liste = Vec::new();
    for compte in &comptes {
        let qui = compte.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        for (nom, icone, candidats) in [
            ("Bureau", "bureau", &["Desktop", "OneDrive/Desktop", "OneDrive/Bureau"][..]),
            ("Téléchargements", "telechargement", &["Downloads"][..]),
            ("Documents", "texte", &["Documents", "OneDrive/Documents"][..]),
            ("Images", "image", &["Pictures", "OneDrive/Pictures", "OneDrive/Images"][..]),
            ("Musique", "audio", &["Music"][..]),
            ("Vidéos", "video", &["Videos"][..]),
        ] {
            let Some(chemin) = candidats.iter().map(|c| compte.join(c)).find(|p| p.is_dir()) else { continue };
            let nom = if plusieurs { format!("{nom} · {qui}") } else { nom.to_string() };
            liste.push(Rapide { nom, chemin: chemin.to_string_lossy().into_owned(), icone, groupe: "windows" });
        }
    }
    liste
}

/// « Ce PC » : le dossier personnel, la racine, puis les volumes que GIO
/// montre (ceux que l'explorateur doit proposer — un disque branché,
/// une partition interne). Commande synchrone : elle tourne sur le fil
/// principal, celui de la boucle GLib que le moniteur de volumes attend.
#[tauri::command]
pub fn fichiers_ce_pc() -> Vec<Value> {
    let id = crate::systeme::identite();
    let nom = if id["nom"].is_empty() { id["login"].clone() } else { id["nom"].clone() };
    #[allow(unused_mut)]
    let mut liste = vec![
        json!({ "nom": if nom.is_empty() { "Dossier personnel".to_string() } else { nom }, "chemin": maison().to_string_lossy(), "icone": "compte" }),
        json!({ "nom": "Système de fichiers", "chemin": "/", "icone": "disque" }),
    ];
    #[cfg(target_os = "linux")]
    for v in {
        use gio::prelude::*;
        gio::VolumeMonitor::get().volumes()
    } {
        use gio::prelude::*;
        let chemin = v.get_mount().and_then(|m| m.root().path()).map(|p| p.to_string_lossy().into_owned());
        let peripherique = v.identifier("unix-device").map(|s| s.to_string());
        let fs = peripherique.as_deref().map(type_de_fs).unwrap_or_default();
        liste.push(json!({ "nom": v.name().to_string(), "chemin": chemin, "peripherique": peripherique, "icone": "disque",
                           "fs": fs, "conseil": conseil_fs(&fs) }));
    }
    liste
}

/// Le système de fichiers d'un disque, lu par lsblk dans la base d'udev :
/// sans root, et sans monter le disque.
#[cfg(target_os = "linux")]
fn type_de_fs(peripherique: &str) -> String {
    std::process::Command::new("lsblk").args(["-no", "FSTYPE", peripherique]).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").trim().to_string())
        .unwrap_or_default()
}

/// A9 (18 novembre) : « FAT32 est privilégié et ne convient pas ». Les clés
/// arrivent du magasin en FAT32, et rien ne le disait — jusqu'au fichier de
/// 5 Go qui refuse d'y entrer sans raison donnée. On ne reformate RIEN
/// (une clé porte des fichiers) : on le dit, et on dit quoi choisir.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn conseil_fs(fs: &str) -> Option<&'static str> {
    match fs {
        "vfat" | "msdos" | "fat" => Some("FAT32 : un fichier de plus de 4 Go n'y entre pas, et une coupure peut l'abîmer.             Pour une clé partagée avec Windows et Mac, exFAT ; pour grenOS seul, ext4 — Disques › Formater (cela efface la clé)."),
        "ntfs" | "ntfs3" => Some("NTFS : le format de Windows. grenOS le lit et l'écrit, mais une copie interrompue s'y répare mieux sous Windows."),
        _ => None,
    }
}

/// Monter un disque, puis dire où il est. `udisksctl` passe par polkit, et
/// notre règle le permet sans mot de passe (la session le prouve à chaque
/// démarrage : « montage : sans mot de passe — amovible oui, interne oui »).
#[tauri::command]
pub async fn fichiers_monter(peripherique: String) -> Result<String, String> {
    if !peripherique.starts_with("/dev/") || peripherique.contains("..") || peripherique.contains(' ') {
        return Err("Disque inconnu.".into());
    }
    let o = std::process::Command::new("udisksctl").args(["mount", "-b", &peripherique, "--no-user-interaction"]).output().map_err(|e| e.to_string())?;
    let texte = String::from_utf8_lossy(&o.stdout);
    // « Mounted /dev/sdb1 at /media/léa/CLÉ »
    if let Some(i) = texte.find(" at ") {
        return Ok(texte[i + 4..].trim().trim_end_matches('.').to_string());
    }
    let erreur = String::from_utf8_lossy(&o.stderr);
    // Déjà monté : udisks le dit, et donne le chemin entre guillemets.
    if let Some(debut) = erreur.find("mounted at `") {
        let reste = &erreur[debut + 12..];
        if let Some(fin) = reste.find('\'') {
            return Ok(reste[..fin].to_string());
        }
    }
    Err(format!("Ce disque n'a pas pu être ouvert : {}", erreur.lines().next().unwrap_or("raison inconnue")))
}

#[tauri::command]
pub fn fichiers_ouvrir(chemin: String) -> Result<(), String> {
    if !Path::new(&chemin).exists() {
        return Err("Ce fichier n'existe plus.".into());
    }
    detacher("xdg-open", &[&chemin]).map_err(|e| format!("Impossible d'ouvrir ce fichier : {e}"))
}

/// Ce qu'on a tapé dans la barre d'adresse : un dossier, un fichier, ou rien.
#[tauri::command]
pub fn fichiers_viser(texte: String, depuis: String) -> Value {
    let t = texte.trim();
    let brut = if let Some(r) = t.strip_prefix('~') { format!("{}{}", maison().to_string_lossy(), r) } else { t.to_string() };
    let p = if Path::new(&brut).is_absolute() { PathBuf::from(&brut) } else { Path::new(&depuis).join(&brut) };
    let p = normaliser(&p);
    let genre = if p.is_dir() { "dossier" } else if p.is_file() { "fichier" } else { "rien" };
    json!({ "genre": genre, "chemin": p.to_string_lossy() })
}

/// `..` et `.` résolus sans toucher au disque (comme os.path.abspath).
pub(crate) fn normaliser(p: &Path) -> PathBuf {
    let mut r = PathBuf::from("/");
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                r.pop();
            }
            std::path::Component::Normal(x) => r.push(x),
            _ => {}
        }
    }
    r
}

pub(crate) fn nom_valable(nom: &str) -> Result<(), &'static str> {
    if nom.contains('/') {
        Err("porte un /")
    } else if nom.is_empty() || nom == "." || nom == ".." {
        Err("vide")
    } else {
        Ok(())
    }
}

#[tauri::command]
pub fn fichiers_nouveau_dossier(dossier: String, nom: String) -> Result<String, String> {
    let nom = nom.trim().to_string();
    if nom_valable(&nom).is_err() {
        dire("fichiers : Ctrl+Maj+N refuse — un nom porte un /");
        return Err("Un nom de dossier ne peut pas contenir « / ».".into());
    }
    let cible = Path::new(&dossier).join(&nom);
    if cible.exists() {
        dire(&format!("fichiers : Ctrl+Maj+N refuse — « {nom} » existe deja"));
        return Err(format!("« {nom} » existe déjà ici."));
    }
    fs::create_dir(&cible).map_err(|e| {
        dire(&format!("fichiers : Ctrl+Maj+N a echoue — {e}"));
        format!("Création impossible : {e}")
    })?;
    Ok(cible.to_string_lossy().into_owned())
}

/// Rend le nouveau chemin, ou None quand le nom n'a pas changé.
#[tauri::command]
pub fn fichiers_renommer(ancien: String, nom: String) -> Result<Option<String>, String> {
    let nom = nom.trim().to_string();
    if nom_valable(&nom).is_err() {
        dire("fichiers : F2 refuse — un nom porte un /");
        return Err("Un nom de fichier ne peut pas contenir « / ».".into());
    }
    let ancien_p = Path::new(&ancien);
    let neuf = ancien_p.parent().unwrap_or(Path::new("/")).join(&nom);
    if neuf == ancien_p {
        dire("fichiers : F2 -> nom inchange, rien a faire");
        return Ok(None);
    }
    if neuf.exists() {
        dire(&format!("fichiers : F2 refuse — « {nom} » existe deja"));
        return Err(format!("« {nom} » existe déjà ici."));
    }
    fs::rename(&ancien, &neuf).map_err(|e| {
        dire(&format!("fichiers : F2 a echoue — {e}"));
        format!("Renommage impossible : {e}")
    })?;
    let avant = ancien_p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    dire(&format!("fichiers : F2 -> « {avant} » est devenu « {nom} »"));
    Ok(Some(neuf.to_string_lossy().into_owned()))
}

/// Un nom libre : « nom », puis « nom (copie) », « nom (copie 2) »…
fn nom_libre(dossier: &Path, nom: &str) -> Option<PathBuf> {
    let cible = dossier.join(nom);
    if !cible.exists() {
        return Some(cible);
    }
    let (base, extension) = match nom.rfind('.') {
        Some(i) if i > 0 => (&nom[..i], &nom[i..]),
        _ => (nom, ""),
    };
    (1..100).map(|n| {
        let suffixe = if n == 1 { " (copie)".to_string() } else { format!(" (copie {n})") };
        dossier.join(format!("{base}{suffixe}{extension}"))
    }).find(|c| !c.exists())
}

fn copier(source: &Path, cible: &Path) -> std::io::Result<()> {
    if fs::symlink_metadata(source)?.is_dir() {
        fs::create_dir(cible)?;
        for e in fs::read_dir(source)? {
            let e = e?;
            copier(&e.path(), &cible.join(e.file_name()))?;
        }
        Ok(())
    } else if fs::symlink_metadata(source)?.file_type().is_symlink() {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(fs::read_link(source)?, cible)
        }
        #[cfg(not(unix))]
        {
            fs::copy(source, cible).map(|_| ())
        }
    } else {
        fs::copy(source, cible).map(|_| ())
    }
}

#[tauri::command]
pub async fn fichiers_coller(sources: Vec<String>, dossier: String, couper: bool) -> Value {
    let ici = normaliser(Path::new(&dossier));
    let (mut faits, mut soucis) = (0, Vec::<String>::new());
    for s in &sources {
        let source = Path::new(s);
        let base = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !source.exists() {
            soucis.push(format!("{base} a disparu"));
            continue;
        }
        let racine = normaliser(source);
        if ici == racine || (source.is_dir() && ici.starts_with(&racine)) {
            soucis.push("on ne colle pas un dossier dans lui-même".into());
            continue;
        }
        let Some(cible) = nom_libre(&ici, &base) else {
            soucis.push(format!("trop de copies de {base}"));
            continue;
        };
        let r = if couper {
            fs::rename(source, &cible).or_else(|_| copier(source, &cible).and_then(|_| {
                if source.is_dir() { fs::remove_dir_all(source) } else { fs::remove_file(source) }
            }))
        } else {
            copier(source, &cible)
        };
        match r {
            Ok(()) => faits += 1,
            Err(e) => soucis.push(format!("{base} : {e}")),
        }
    }
    json!({ "faits": faits, "soucis": soucis })
}

#[tauri::command]
pub async fn fichiers_corbeille(chemins: Vec<String>) -> Value {
    let (mut faits, mut refus) = (0, Vec::<String>::new());
    for c in &chemins {
        #[cfg(target_os = "linux")]
        let r = {
            use gio::prelude::*;
            gio::File::for_path(c).trash(gio::Cancellable::NONE).map_err(|e| e.message().to_string())
        };
        #[cfg(not(target_os = "linux"))]
        let r: Result<(), String> = Err("corbeille absente ici".into());
        match r {
            Ok(()) => faits += 1,
            Err(e) => refus.push(format!("{} : {e}", Path::new(c).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())),
        }
    }
    json!({ "faits": faits, "refus": refus })
}

/// Compresser (en .zip : il s'ouvre partout) ou extraire (dans un dossier à
/// son nom). Cinq minutes au plus ; un échec se DIT, la fenêtre reste.
#[tauri::command]
pub async fn fichiers_archive(geste: String, choisis: Vec<String>, dossier: String) -> Result<String, String> {
    let ici = PathBuf::from(&dossier);
    let (commande, quoi, nom): (Vec<String>, &str, String) = if geste == "compresser" {
        let base = if choisis.len() == 1 {
            let n = Path::new(&choisis[0]).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            match n.rfind('.') { Some(i) if i > 0 => n[..i].to_string(), _ => n }
        } else {
            "archive".into()
        };
        let mut cible = ici.join(format!("{base}.zip"));
        let mut rang = 2;
        while cible.exists() {
            cible = ici.join(format!("{base}-{rang}.zip"));
            rang += 1;
        }
        let mut c = vec!["zip".to_string(), "-r".into(), "-q".into(), cible.to_string_lossy().into_owned()];
        c.extend(choisis.iter().filter_map(|x| Path::new(x).file_name().map(|n| n.to_string_lossy().into_owned())));
        (c, "compression", cible.file_name().unwrap().to_string_lossy().into_owned())
    } else {
        let archive = choisis.iter().find(|c| est_une_archive(c)).ok_or("Aucune archive choisie.")?;
        let mut nom = Path::new(archive).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let bas = nom.to_lowercase();
        if let Some(fin) = [".tar.gz", ".tar.bz2", ".tar.xz"].iter().find(|f| bas.ends_with(*f)) {
            nom.truncate(nom.len() - fin.len());
        } else if let Some(i) = nom.rfind('.') {
            nom.truncate(i);
        }
        let mut cible = ici.join(&nom);
        let mut rang = 2;
        while cible.exists() {
            cible = ici.join(format!("{nom}-{rang}"));
            rang += 1;
        }
        let c = vec!["7z".to_string(), "x".into(), "-y".into(), format!("-o{}", cible.to_string_lossy()), archive.clone()];
        (c, "extraction", cible.file_name().unwrap().to_string_lossy().into_owned())
    };
    let fin = std::process::Command::new("timeout").arg("300").args(&commande).current_dir(&ici).output();
    match fin {
        Err(_) => {
            dire(&format!("fichiers : {quoi} impossible — outil absent"));
            Err("L'outil d'archive n'est pas installé sur cette machine.".into())
        }
        Ok(o) if o.status.code() == Some(124) => {
            dire(&format!("fichiers : {quoi} coupee apres 300 s"));
            Err("L'archive est trop longue : arrêtée après cinq minutes.".into())
        }
        Ok(o) if o.status.code() == Some(127) => {
            dire(&format!("fichiers : {quoi} impossible — outil absent"));
            Err("L'outil d'archive n'est pas installé sur cette machine.".into())
        }
        Ok(o) if !o.status.success() => {
            let code = o.status.code().unwrap_or(-1);
            dire(&format!("fichiers : {quoi} echouee — code {code}"));
            let raison = String::from_utf8_lossy(if o.stderr.is_empty() { &o.stdout } else { &o.stderr }).lines().next().unwrap_or("").chars().take(90).collect::<String>();
            Err(format!("Échec : {}", if raison.is_empty() { format!("code {code}") } else { raison }))
        }
        Ok(_) => Ok(format!("{quoi}|{nom}")),
    }
}

#[tauri::command]
pub async fn fichiers_proprietes(chemin: String) -> Result<Vec<(String, String)>, String> {
    let p = Path::new(&chemin);
    let meta = fs::metadata(p).map_err(|e| format!("Lecture impossible : {e}"))?;
    let (genre, taille) = if meta.is_dir() {
        ("Dossier de fichiers".to_string(), String::new())
    } else {
        let g = genre_et_mime(p).0;
        (if g.is_empty() { "Fichier".into() } else { g }, meta.len().to_string())
    };
    let modifie = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    Ok(vec![
        ("Nom".into(), p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "/".into())),
        ("Type".into(), genre),
        ("Emplacement".into(), p.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()),
        ("Taille".into(), taille),
        ("Modifié le".into(), modifie.to_string()),
    ])
}

/// Les phrases que seule la page sait compléter (le nombre d'éléments
/// affichés après un filtre, un onglet…). Seulement les lignes « fichiers » :
/// la page ne peut pas écrire n'importe quoi sur le port série.
#[tauri::command]
pub fn fichiers_dire(texte: String) {
    if texte.starts_with("fichiers ouvert, ") || texte.starts_with("fichiers : ") {
        dire(&texte.chars().take(300).collect::<String>());
    }
}

#[tauri::command]
pub fn fichiers_titre(fenetre: tauri::WebviewWindow, titre: String) {
    let _ = fenetre.set_title(&titre);
}

#[tauri::command]
pub fn fichiers_nouvelle_fenetre(app: tauri::AppHandle, dossier: String) -> Result<(), String> {
    crate::ouvrir(&app, &format!("fichiers:{dossier}")).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fichiers_maison() -> String {
    maison().to_string_lossy().into_owned()
}

// ---- B6 : chiffrer et déchiffrer un fichier ---------------------------------------

/// gpg, la phrase sur l'entrée standard (jamais en argument : elle serait
/// lisible par tous dans la liste des processus).
fn gpg(arguments: &[&str], phrase: &str) -> Result<(), String> {
    use std::io::Write;
    let mut enfant = std::process::Command::new("gpg")
        .args(["--batch", "--yes", "--quiet", "--pinentry-mode", "loopback", "--passphrase-fd", "0"])
        .args(arguments)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| "GnuPG n'est pas installé.".to_string())?;
    if let Some(mut e) = enfant.stdin.take() {
        let _ = e.write_all(phrase.as_bytes());
    }
    let fin = enfant.wait_with_output().map_err(|e| e.to_string())?;
    if fin.status.success() {
        Ok(())
    } else {
        let t = String::from_utf8_lossy(&fin.stderr);
        Err(if t.contains("Bad passphrase") || t.contains("bad key") || t.contains("decryption failed") {
            "Phrase de passe incorrecte.".into()
        } else {
            format!("GnuPG a refusé : {}", t.lines().last().unwrap_or("raison inconnue"))
        })
    }
}

/// Chiffre une COPIE (`nom.gpg`) ; l'original reste, à la personne de le
/// supprimer une fois la copie vérifiée. Rend le nom du fichier chiffré.
#[tauri::command]
pub async fn fichiers_chiffrer(chemin: String, phrase: String) -> Result<String, String> {
    let p = Path::new(&chemin);
    if !p.is_file() {
        return Err("On ne chiffre qu'un fichier (pour un dossier : le compresser d'abord).".into());
    }
    if phrase.chars().count() < 8 {
        return Err("Huit caractères au moins : une phrase courte se devine.".into());
    }
    if phrase.contains('\n') {
        return Err("La phrase tient sur une ligne.".into());
    }
    let nom = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let cible = nom_libre(p.parent().unwrap_or(Path::new("/")), &format!("{nom}.gpg")).ok_or("Trop de copies chiffrées ici.")?;
    let c = cible.to_string_lossy().into_owned();
    gpg(&["--symmetric", "--cipher-algo", "AES256", "--output", &c, &chemin], &phrase)?;
    dire("fichiers : chiffrement -> un fichier .gpg ecrit");
    Ok(cible.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

#[tauri::command]
pub async fn fichiers_dechiffrer(chemin: String, phrase: String) -> Result<String, String> {
    let p = Path::new(&chemin);
    let nom = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let clair = nom.strip_suffix(".gpg").or_else(|| nom.strip_suffix(".pgp")).filter(|n| !n.is_empty()).unwrap_or("dechiffre");
    let cible = nom_libre(p.parent().unwrap_or(Path::new("/")), clair).ok_or("Trop de copies ici.")?;
    let c = cible.to_string_lossy().into_owned();
    if let Err(e) = gpg(&["--output", &c, "--decrypt", &chemin], &phrase) {
        let _ = fs::remove_file(&cible); // pas de fichier à moitié écrit
        return Err(e);
    }
    Ok(cible.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

// ---- H6 : l'OCR, en local --------------------------------------------------------

/// Le texte d'une image (Tesseract, français et anglais) dans un .txt à côté ;
/// un PDF scanné devient une copie cherchable (ocrmypdf). Rien ne quitte la
/// machine. Cinq minutes au plus : un PDF de cent pages scannées est long.
#[tauri::command]
pub async fn fichiers_ocr(chemin: String) -> Result<String, String> {
    let p = Path::new(&chemin);
    if !p.is_file() {
        return Err("Choisis une image ou un PDF.".into());
    }
    let dossier = p.parent().unwrap_or(Path::new("/"));
    let nom = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let base = match nom.rfind('.') { Some(i) if i > 0 => nom[..i].to_string(), _ => nom.clone() };
    let pdf = nom.to_lowercase().ends_with(".pdf");
    let cible = if pdf {
        nom_libre(dossier, &format!("{base} (texte).pdf"))
    } else {
        nom_libre(dossier, &format!("{base}.txt"))
    }
    .ok_or("Trop de copies ici.")?;
    let c = cible.to_string_lossy().into_owned();
    let sortie = if pdf {
        std::process::Command::new("timeout")
            .args(["300", "ocrmypdf", "--skip-text", "-l", "fra+eng", "--output-type", "pdf", &chemin, &c])
            .output()
    } else {
        // tesseract ajoute lui-même « .txt » au nom donné.
        let sans = c.strip_suffix(".txt").unwrap_or(&c).to_string();
        std::process::Command::new("timeout").args(["300", "tesseract", &chemin, &sans, "-l", "fra+eng"]).output()
    }
    .map_err(|_| "L'OCR n'est pas installé.".to_string())?;
    if !sortie.status.success() {
        let _ = fs::remove_file(&cible);
        return Err(match sortie.status.code() {
            Some(124) => "Trop long : arrêté après cinq minutes.".into(),
            Some(127) => "L'OCR n'est pas installé.".into(),
            _ => format!("L'OCR a échoué : {}", String::from_utf8_lossy(&sortie.stderr).lines().last().unwrap_or("raison inconnue")),
        });
    }
    if !pdf {
        let lu = fs::read_to_string(&cible).unwrap_or_default();
        if lu.trim().is_empty() {
            let _ = fs::remove_file(&cible);
            return Err("Aucun texte trouvé dans cette image.".into());
        }
    }
    dire(&format!("fichiers : ocr -> {}", if pdf { "pdf cherchable" } else { "texte extrait" }));
    Ok(cible.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

// ---- G3 : l'aperçu (Alt+P, comme Windows) --------------------------------------

const APERCU_IMAGE_MAX: u64 = 8 * 1024 * 1024;
const APERCU_TEXTE_MAX: usize = 16 * 1024;

/// Base64 standard, sans dépendance de plus pour une seule fonction.
pub(crate) fn base64(octets: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(octets.len().div_ceil(3) * 4);
    for bloc in octets.chunks(3) {
        let n = (bloc[0] as u32) << 16 | (*bloc.get(1).unwrap_or(&0) as u32) << 8 | *bloc.get(2).unwrap_or(&0) as u32;
        for (i, decalage) in [18, 12, 6, 0].iter().enumerate() {
            if i <= bloc.len() {
                s.push(T[(n >> decalage & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

pub(crate) fn mime_image(nom: &str) -> Option<&'static str> {
    let bas = nom.to_lowercase();
    let ext = bas.rsplit('.').next()?;
    Some(match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

/// Un texte lisible : de l'UTF-8 sans octet nul dans le début du fichier.
pub(crate) fn texte_lisible(debut: &[u8]) -> Option<String> {
    if debut.contains(&0) {
        return None;
    }
    // Coupé au milieu d'un caractère : on recule jusqu'à une limite valide.
    let mut fin = debut.len();
    while fin > 0 && std::str::from_utf8(&debut[..fin]).is_err() {
        fin -= 1;
        if debut.len() - fin > 4 {
            return None;
        }
    }
    std::str::from_utf8(&debut[..fin]).ok().map(String::from)
}

/// L'aperçu d'un fichier : une image (en data: URI, la page n'a pas accès au
/// disque), le début d'un texte, ou rien. Bornés : 8 Mo pour une image,
/// 16 Ko de texte — un aperçu ne doit jamais faire attendre la fenêtre.
#[tauri::command]
pub async fn fichiers_apercu(chemin: String) -> Value {
    let p = Path::new(&chemin);
    let Ok(meta) = fs::metadata(p) else { return json!({ "genre": "rien" }) };
    if meta.is_dir() {
        let n = fs::read_dir(p).map(|d| d.count()).unwrap_or(0);
        return json!({ "genre": "dossier", "elements": n });
    }
    let nom = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(mime) = mime_image(&nom) {
        if meta.len() <= APERCU_IMAGE_MAX {
            if let Ok(octets) = fs::read(p) {
                return json!({ "genre": "image", "donnees": format!("data:{mime};base64,{}", base64(&octets)) });
            }
        }
        return json!({ "genre": "trop", "octets": meta.len() });
    }
    let mut debut = vec![0u8; APERCU_TEXTE_MAX];
    let lu = fs::File::open(p).and_then(|mut f| std::io::Read::read(&mut f, &mut debut)).unwrap_or(0);
    debut.truncate(lu);
    match texte_lisible(&debut) {
        Some(t) => json!({ "genre": "texte", "texte": t, "coupe": meta.len() as usize > lu }),
        None => json!({ "genre": "rien" }),
    }
}

#[cfg(test)]
mod essais {
    #[test]
    fn fat32_est_signale_et_ext4_non() {
        assert!(super::conseil_fs("vfat").unwrap().contains("4 Go"));
        assert!(super::conseil_fs("ntfs").is_some());
        assert!(super::conseil_fs("ext4").is_none());
        assert!(super::conseil_fs("exfat").is_none());
        assert!(super::conseil_fs("").is_none());
    }

    use super::*;

    #[test]
    fn les_dossiers_de_windows() {
        let racine = std::env::temp_dir().join(format!("grenos-windows-{}", std::process::id()));
        let _ = fs::remove_dir_all(&racine);
        for d in ["Users/rayan/Downloads", "Users/rayan/Videos", "Users/rayan/OneDrive/Images", "Users/Public/Downloads", "Users/Default/Desktop"] {
            fs::create_dir_all(racine.join(d)).unwrap();
        }
        fs::create_dir_all(racine.join("Windows/System32")).unwrap();
        let un = dossiers_windows(&racine);
        let noms: Vec<&str> = un.iter().map(|r| r.nom.as_str()).collect();
        // Public et Default ne sont pas des personnes ; Images vient d'OneDrive.
        assert_eq!(noms, ["Téléchargements", "Images", "Vidéos"]);
        assert!(un[1].chemin.ends_with("Images"));
        assert!(un.iter().all(|r| r.groupe == "windows"));
        // Deux comptes : le nom du compte suit.
        fs::create_dir_all(racine.join("Users/lea/Downloads")).unwrap();
        let deux = dossiers_windows(&racine);
        assert!(deux.iter().any(|r| r.nom == "Téléchargements · lea"));
        assert!(deux.iter().any(|r| r.nom == "Téléchargements · rayan"));
        // Pas de Windows : rien.
        assert!(dossiers_windows(&racine.join("rien")).is_empty());
        let _ = fs::remove_dir_all(&racine);
    }

    #[test]
    fn base64_comme_la_norme() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn texte_ou_pas() {
        assert_eq!(texte_lisible("Bonjour à tous".as_bytes()).as_deref(), Some("Bonjour à tous"));
        assert_eq!(texte_lisible(b"\x89PNG\x00\x00"), None);
        // « é » coupé en deux par la limite de lecture : on garde ce qui précède.
        let coupe = &"café".as_bytes()[..4];
        assert_eq!(texte_lisible(coupe).as_deref(), Some("caf"));
        assert_eq!(mime_image("Photo.JPG"), Some("image/jpeg"));
        assert_eq!(mime_image("notes.txt"), None);
    }

    #[test]
    fn archives() {
        assert!(est_une_archive("Photos.TAR.GZ"));
        assert!(est_une_archive("a.zip"));
        assert!(!est_une_archive("a.pdf"));
    }

    #[test]
    fn noms() {
        assert!(nom_valable("Nouveau dossier").is_ok());
        assert!(nom_valable("a/b").is_err());
        assert!(nom_valable("..").is_err());
    }

    #[test]
    fn chemins_normalises() {
        assert_eq!(normaliser(Path::new("/home/lea/../lea/./Documents")), PathBuf::from("/home/lea/Documents"));
        assert_eq!(normaliser(Path::new("/..")), PathBuf::from("/"));
    }

    #[test]
    fn jamais_ecraser() {
        let d = std::env::temp_dir().join(format!("grenos-essai-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        assert_eq!(nom_libre(&d, "a.txt"), Some(d.join("a.txt")));
        fs::write(d.join("a.txt"), "x").unwrap();
        assert_eq!(nom_libre(&d, "a.txt"), Some(d.join("a (copie).txt")));
        fs::write(d.join("a (copie).txt"), "x").unwrap();
        assert_eq!(nom_libre(&d, "a.txt"), Some(d.join("a (copie 2).txt")));
        let _ = fs::remove_dir_all(&d);
    }
}
