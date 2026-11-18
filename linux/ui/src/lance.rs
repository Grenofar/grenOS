//! GrenLaunch — tous les jeux au même endroit.
//!
//! Trois sources, une seule bibliothèque :
//! * Steam — lu dans ses `appmanifest_*.acf`, sur TOUTES ses bibliothèques
//!   (`libraryfolders.vdf`). Un jeu installé depuis Steam apparaît donc ici
//!   sans rien faire : on ne tient pas de liste à côté, on lit la sienne ;
//! * Heroic — les jeux Epic (legendary) et GOG qu'il a installés ;
//! * Flathub — les jeux et lanceurs du catalogue `jeux.json`.
//!
//! Les images : d'abord le cache local de Steam, sinon les serveurs
//! officiels — Steam pour ses jeux, Flathub pour les icônes des applications —
//! gardés ensuite par le cache de WebKit. Sans réseau, la carte retombe sur
//! ses deux couleurs et le nom du jeu.

use crate::systeme::{dire, sortie};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::Emitter;

static CATALOGUE: &str = include_str!("../../config/includes.chroot/usr/share/grenos/jeux.json");

const STEAM_FLATPAK: &str = "com.valvesoftware.Steam";
const HEROIC_FLATPAK: &str = "com.heroicgameslauncher.hgl";

#[derive(Deserialize, Serialize, Clone)]
pub struct Plateforme {
    cle: String, nom: String, note: String, identifiant: String, couleurs: Vec<String>,
    #[serde(default)] installe: bool,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Jeu {
    cle: String, nom: String, genre: String, etat: String, source: String, note: String,
    couleurs: Vec<String>,
    #[serde(default)] identifiant: Option<String>,
    #[serde(default)] adresse: Option<String>,
    #[serde(default)] steam: Option<u32>,
    #[serde(default)] logo: Option<String>,
    #[serde(default)] installe: bool,
}

#[derive(Deserialize)]
struct Catalogue { plateformes: Vec<Plateforme>, jeux: Vec<Jeu> }

#[derive(Serialize, Clone)]
pub struct JeuLocal {
    id: String,
    nom: String,
    /// steam | heroic-legendary | heroic-gog | flatpak
    origine: String,
    taille_go: f64,
    image: Option<String>,
}

#[derive(Serialize)]
pub struct Vue {
    plateformes: Vec<Plateforme>,
    jeux: Vec<Jeu>,
    bibliotheque: Vec<JeuLocal>,
    /// Les jeux du compte Steam qui ne sont pas installés ici.
    compte: Vec<JeuLocal>,
    steam: bool,
    heroic: bool,
}

fn maison() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into()))
}

fn flatpaks_installes() -> HashSet<String> {
    sortie("flatpak", &["list", "--app", "--columns=application"])
        .lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

fn dans_le_path(programme: &str) -> bool {
    std::env::var("PATH").unwrap_or_default().split(':').any(|d| Path::new(d).join(programme).is_file())
}

// ------------------------------------------------------------------ Steam

fn racines_steam() -> Vec<PathBuf> {
    let m = maison();
    let mut vues = vec![];
    for r in [".var/app/com.valvesoftware.Steam/.local/share/Steam", ".var/app/com.valvesoftware.Steam/.steam/steam",
              ".local/share/Steam", ".steam/steam", ".steam/debian-installation"] {
        let p = m.join(r);
        if let Ok(vraie) = fs::canonicalize(&p) {
            if vraie.join("steamapps").is_dir() && !vues.contains(&vraie) {
                vues.push(vraie);
            }
        }
    }
    vues
}

/// Les valeurs `"cle"  "valeur"` d'un fichier VDF/ACF, à plat. Suffisant pour
/// `appid`, `name`, `StateFlags` et les `path` des bibliothèques.
fn vdf_valeurs<'a>(texte: &'a str, cle: &str) -> Vec<&'a str> {
    let motif = format!("\"{cle}\"");
    texte.lines().filter_map(|l| {
        let l = l.trim();
        let reste = l.strip_prefix(&motif)?.trim();
        reste.strip_prefix('"')?.strip_suffix('"')
    }).collect()
}

/// Ce que Steam installe pour lui-même et qui n'est pas un jeu.
fn outil_steam(appid: u32, nom: &str) -> bool {
    matches!(appid, 228980 | 1070560 | 1391110 | 1628350 | 1493710 | 2180100 | 1826330 | 961940 | 1054830 | 1113280 | 1245040 | 1420170 | 1580130 | 1887720 | 2348590 | 2805730)
        || nom.starts_with("Proton") || nom.starts_with("Steam Linux Runtime") || nom.starts_with("Steamworks")
}

fn image_steam(racine: &Path, appid: u32) -> Option<String> {
    let cache = racine.join("appcache/librarycache");
    let mut candidats = vec![cache.join(format!("{appid}_library_600x900.jpg")), cache.join(format!("{appid}/library_600x900.jpg"))];
    // Depuis 2024, Steam range chaque jeu dans un dossier, parfois sous un
    // sous-dossier au nom de hachage : on regarde un étage plus bas.
    if let Ok(entrees) = fs::read_dir(cache.join(appid.to_string())) {
        for e in entrees.flatten() {
            if e.path().is_dir() {
                candidats.push(e.path().join("library_600x900.jpg"));
            }
        }
    }
    candidats.push(cache.join(format!("{appid}_header.jpg")));
    candidats.push(cache.join(format!("{appid}/header.jpg")));
    let chemin = candidats.into_iter().find(|c| c.is_file())?;
    let octets = fs::read(chemin).ok()?;
    if octets.len() > 1_500_000 {
        return None;
    }
    Some(format!("data:image/jpeg;base64,{}", base64(&octets)))
}

fn base64(octets: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(octets.len() * 4 / 3 + 4);
    for bloc in octets.chunks(3) {
        let n = (bloc[0] as u32) << 16 | (*bloc.get(1).unwrap_or(&0) as u32) << 8 | *bloc.get(2).unwrap_or(&0) as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if bloc.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if bloc.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

fn jeux_steam(images: bool) -> Vec<JeuLocal> {
    let mut jeux = vec![];
    let mut vus = HashSet::new();
    for racine in racines_steam() {
        let mut bibliotheques = vec![racine.clone()];
        let dossiers = fs::read_to_string(racine.join("steamapps/libraryfolders.vdf")).unwrap_or_default();
        for p in vdf_valeurs(&dossiers, "path") {
            let p = PathBuf::from(p.replace("\\\\", "/"));
            if p.is_dir() && !bibliotheques.contains(&p) {
                bibliotheques.push(p);
            }
        }
        for b in bibliotheques {
            let Ok(entrees) = fs::read_dir(b.join("steamapps")) else { continue };
            for e in entrees.flatten() {
                let nom_fichier = e.file_name().to_string_lossy().into_owned();
                if !nom_fichier.starts_with("appmanifest_") || !nom_fichier.ends_with(".acf") {
                    continue;
                }
                let texte = fs::read_to_string(e.path()).unwrap_or_default();
                let appid: u32 = vdf_valeurs(&texte, "appid").first().and_then(|v| v.parse().ok()).unwrap_or(0);
                let nom = vdf_valeurs(&texte, "name").first().map(|s| s.to_string()).unwrap_or_default();
                let etat: u32 = vdf_valeurs(&texte, "StateFlags").first().and_then(|v| v.parse().ok()).unwrap_or(0);
                // 4 = entièrement installé. Un jeu en cours de téléchargement
                // a d'autres bits : il apparaîtra quand il sera jouable.
                if appid == 0 || outil_steam(appid, &nom) || etat & 4 == 0 || !vus.insert(appid) {
                    continue;
                }
                let octets: f64 = vdf_valeurs(&texte, "SizeOnDisk").first().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                jeux.push(JeuLocal {
                    id: appid.to_string(), nom, origine: "steam".into(), taille_go: octets / 1e9,
                    image: if images { image_steam(&racine, appid) } else { None },
                });
            }
        }
    }
    jeux
}

// ----------------------------------------------------------------- Heroic

fn racines_heroic() -> Vec<PathBuf> {
    let m = maison();
    [".var/app/com.heroicgameslauncher.hgl/config/heroic", ".config/heroic"]
        .iter().map(|r| m.join(r)).filter(|p| p.is_dir()).collect()
}

fn jeux_heroic() -> Vec<JeuLocal> {
    let mut jeux = vec![];
    for racine in racines_heroic() {
        let legendary = racine.join("legendaryConfig/legendary/installed.json");
        if let Ok(v) = fs::read_to_string(legendary).map(|t| serde_json::from_str::<serde_json::Value>(&t)) {
            if let Ok(serde_json::Value::Object(carte)) = v {
                for (app, info) in carte {
                    let nom = info.get("title").and_then(|t| t.as_str()).unwrap_or(&app).to_string();
                    let taille = info.get("install_size").and_then(|t| t.as_f64()).unwrap_or(0.0) / 1e9;
                    jeux.push(JeuLocal { id: app, nom, origine: "heroic-legendary".into(), taille_go: taille, image: None });
                }
            }
        }
        let gog = racine.join("gog_store/installed.json");
        if let Ok(Ok(v)) = fs::read_to_string(gog).map(|t| serde_json::from_str::<serde_json::Value>(&t)) {
            for g in v.get("installed").and_then(|i| i.as_array()).cloned().unwrap_or_default() {
                let app = g.get("appName").and_then(|a| a.as_str()).unwrap_or("").to_string();
                if app.is_empty() { continue; }
                let nom = g.get("title").and_then(|t| t.as_str())
                    .map(String::from)
                    .or_else(|| g.get("install_path").and_then(|p| p.as_str()).and_then(|p| p.rsplit('/').next()).map(String::from))
                    .unwrap_or_else(|| app.clone());
                jeux.push(JeuLocal { id: app, nom, origine: "heroic-gog".into(), taille_go: 0.0, image: None });
            }
        }
    }
    jeux
}

// --------------------------------------------------------------- commandes

#[tauri::command]
pub async fn lance_vue(images: bool) -> Vue {
    let catalogue: Catalogue = serde_json::from_str(CATALOGUE).expect("jeux.json est vérifié par les essais");
    let flatpaks = flatpaks_installes();
    let steam_ok = flatpaks.contains(STEAM_FLATPAK) || dans_le_path("steam");
    let heroic_ok = flatpaks.contains(HEROIC_FLATPAK) || dans_le_path("heroic");
    let steam = jeux_steam(images);
    let heroic = jeux_heroic();
    let steam_ids: HashSet<String> = steam.iter().map(|j| j.id.clone()).collect();
    let heroic_ids: HashSet<String> = heroic.iter().map(|j| j.id.clone()).collect();

    let plateformes = catalogue.plateformes.into_iter().map(|mut p| {
        p.installe = flatpaks.contains(&p.identifiant) || (p.cle == "steam" && steam_ok) || (p.cle == "heroic" && heroic_ok);
        p
    }).collect();

    let mut bibliotheque = steam;
    bibliotheque.extend(heroic);
    let jeux: Vec<Jeu> = catalogue.jeux.into_iter().map(|mut j| {
        j.installe = match j.source.as_str() {
            "flatpak" => j.identifiant.as_ref().map(|i| flatpaks.contains(i)).unwrap_or(false),
            "steam" => j.steam.map(|s| steam_ids.contains(&s.to_string())).unwrap_or(false),
            "heroic" => j.identifiant.as_ref().map(|i| heroic_ids.contains(i)).unwrap_or(false)
                || j.steam.map(|s| steam_ids.contains(&s.to_string())).unwrap_or(false),
            _ => false,
        };
        j
    }).collect();
    // Les jeux Flathub du catalogue installés font partie de la bibliothèque.
    for j in &jeux {
        if j.source == "flatpak" && j.installe {
            bibliotheque.push(JeuLocal { id: j.identifiant.clone().unwrap_or_default(), nom: j.nom.clone(), origine: "flatpak".into(), taille_go: 0.0, image: None });
        }
    }
    bibliotheque.sort_by(|a, b| a.nom.to_lowercase().cmp(&b.nom.to_lowercase()));
    let installes: HashSet<u32> = steam_ids.iter().filter_map(|i| i.parse().ok()).collect();
    let compte = crate::steam_compte::jeux_du_compte(&racines_steam(), &installes, outil_steam)
        .into_iter()
        .map(|(id, nom)| JeuLocal { id: id.to_string(), nom, origine: "steam-compte".into(), taille_go: 0.0, image: None })
        .collect();
    Vue { plateformes, jeux, bibliotheque, compte, steam: steam_ok, heroic: heroic_ok }
}

fn detacher(programme: &str, arguments: &[&str]) -> Result<(), String> {
    Command::new(programme).args(arguments)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map(|_| ()).map_err(|e| format!("Impossible de lancer {programme} : {e}"))
}

fn steam(url: &str) -> Result<(), String> {
    if flatpaks_installes().contains(STEAM_FLATPAK) {
        detacher("flatpak", &["run", STEAM_FLATPAK, url])
    } else if dans_le_path("steam") {
        detacher("steam", &[url])
    } else {
        Err("Steam n'est pas installé : installe-le depuis l'onglet Plateformes.".into())
    }
}

fn heroic(url: &str) -> Result<(), String> {
    let url: &[&str] = if url.is_empty() { &[] } else { std::slice::from_ref(&url) };
    if flatpaks_installes().contains(HEROIC_FLATPAK) {
        detacher("flatpak", &[&["run", HEROIC_FLATPAK][..], url].concat())
    } else if dans_le_path("heroic") {
        detacher("heroic", url)
    } else {
        Err("Heroic n'est pas installé : installe-le depuis l'onglet Plateformes.".into())
    }
}

/// Jouer. `origine` vient de la bibliothèque ou du catalogue.
#[tauri::command]
pub fn lance_jouer(origine: String, id: String) -> Result<(), String> {
    if id.is_empty() || id.contains(char::is_whitespace) {
        return Err("identifiant refusé".into());
    }
    dire(&format!("lance : jouer {origine} {id}"));
    match origine.as_str() {
        "steam" => steam(&format!("steam://rungameid/{}", id.parse::<u32>().map_err(|_| "appid refusé")?)),
        "heroic-legendary" => heroic(&format!("heroic://launch?appName={id}&runner=legendary")),
        "heroic-gog" => heroic(&format!("heroic://launch?appName={id}&runner=gog")),
        "flatpak" => detacher("flatpak", &["run", &id]),
        "nuage" => {
            if !id.starts_with("https://") {
                return Err("adresse refusée".into());
            }
            // Xbox Cloud Gaming refuse Firefox : Chromium en mode application
            // quand il est là, le navigateur par défaut sinon.
            for c in ["chromium", "chromium-browser", "google-chrome"] {
                if dans_le_path(c) {
                    return detacher(c, &[&format!("--app={id}"), "--start-maximized"]);
                }
            }
            detacher("xdg-open", &[&id])
        }
        _ => Err("origine inconnue".into()),
    }
}

/// Installer un jeu Steam : c'est Steam qui l'installe (compte, achat,
/// Proton), on lui passe seulement la demande. Dès qu'il a fini, le jeu
/// apparaît dans la bibliothèque — on relit ses manifestes.
#[tauri::command]
pub fn lance_steam_installer(appid: u32) -> Result<(), String> {
    steam(&format!("steam://install/{appid}"))
}

#[tauri::command]
pub fn lance_ouvrir(plateforme: String) -> Result<(), String> {
    match plateforme.as_str() {
        "steam" => steam("steam://open/games"),
        "heroic" => heroic(""),
        _ => Err("plateforme inconnue".into()),
    }
}

#[derive(Serialize, Clone)]
struct Progres { identifiant: String, part: u32, texte: String }
#[derive(Serialize, Clone)]
struct Fin { identifiant: String, bien: bool, texte: String }

/// Installer depuis Flathub, à l'échelle du système comme GrenPlace — la
/// règle polkit de grenOS l'autorise sans mot de passe aux comptes `sudo`.
/// Si le système refuse, on retombe sur l'installation personnelle.
#[tauri::command]
pub fn lance_installer(app: tauri::AppHandle, identifiant: String) -> Result<(), String> {
    let permis = identifiant.split('.').count() >= 3
        && identifiant.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');
    if !permis {
        return Err("identifiant refusé".into());
    }
    std::thread::spawn(move || {
        let mut bien = false;
        for portee in ["--system", "--user"] {
            let _ = app.emit("lance-progres", Progres { identifiant: identifiant.clone(), part: 2, texte: "Préparation…".into() });
            let Ok(mut enfant) = Command::new("flatpak")
                .args(["install", portee, "-y", "--noninteractive", "flathub", &identifiant])
                .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
            else { break };
            if let Some(mut sortie) = enfant.stdout.take() {
                // flatpak réécrit sa ligne avec \r : on découpe sur les deux.
                let mut tampon = [0u8; 512];
                let mut ligne = String::new();
                while let Ok(n) = sortie.read(&mut tampon) {
                    if n == 0 { break; }
                    for c in String::from_utf8_lossy(&tampon[..n]).chars() {
                        if c == '\r' || c == '\n' {
                            if let Some(part) = pour_cent(&ligne) {
                                let _ = app.emit("lance-progres", Progres { identifiant: identifiant.clone(), part, texte: format!("Téléchargement {part} %") });
                            }
                            ligne.clear();
                        } else {
                            ligne.push(c);
                        }
                    }
                }
            }
            if enfant.wait().map(|s| s.success()).unwrap_or(false) {
                bien = true;
                break;
            }
        }
        dire(&format!("lance : installation {identifiant} {}", if bien { "ok" } else { "echec" }));
        let _ = app.emit("lance-fin", Fin {
            identifiant: identifiant.clone(), bien,
            texte: if bien { "Installé.".into() } else { "L'installation a échoué. Vérifie la connexion.".into() },
        });
    });
    Ok(())
}

fn pour_cent(ligne: &str) -> Option<u32> {
    let i = ligne.rfind('%')?;
    // La fin du dernier caractère qui n'est pas un chiffre — PAS « j + 1 » :
    // ce caractère peut tenir sur plusieurs octets (la barre « ██42% » de
    // flatpak), et couper au milieu fait paniquer Rust. Trouvé par le fuzzing
    // (aleatoire.rs) le 18 novembre.
    let debut = ligne[..i].char_indices().rev().find(|(_, c)| !c.is_ascii_digit()).map(|(j, c)| j + c.len_utf8()).unwrap_or(0);
    ligne[debut..i].parse().ok().filter(|p| *p <= 100)
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn catalogue_tient() {
        let c: Catalogue = serde_json::from_str(CATALOGUE).unwrap();
        assert!(c.jeux.len() >= 20);
        for j in &c.jeux {
            match j.source.as_str() {
                "flatpak" | "heroic" => assert!(j.identifiant.is_some(), "{} sans identifiant", j.cle),
                "steam" => assert!(j.steam.is_some(), "{} sans appid", j.cle),
                "nuage" => assert!(j.adresse.as_deref().unwrap_or("").starts_with("https://")),
                "aucune" => assert_eq!(j.etat, "indisponible"),
                s => panic!("source inconnue {s}"),
            }
            assert!(["natif", "proton", "nuage", "indisponible"].contains(&j.etat.as_str()));
            assert_eq!(j.couleurs.len(), 2);
        }
    }

    #[test]
    fn acf() {
        let t = "\"AppState\"\n{\n\t\"appid\"\t\t\"730\"\n\t\"name\"\t\t\"Counter-Strike 2\"\n\t\"StateFlags\"\t\t\"4\"\n}";
        assert_eq!(vdf_valeurs(t, "appid"), vec!["730"]);
        assert_eq!(vdf_valeurs(t, "name"), vec!["Counter-Strike 2"]);
    }

    #[test]
    fn progres() {
        assert_eq!(pour_cent("Installing 1/2… ████  45%  2.1 MB/s"), Some(45));
        assert_eq!(pour_cent("rien"), None);
    }

    #[test]
    fn b64() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}
