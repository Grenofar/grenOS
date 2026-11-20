//! Le créateur de clé USB grenOS (Windows), 21 novembre.
//!
//! Grenofar : « fais un créateur de clé USB bootable en Rust Tauri, avec l'UI
//! de grenOS : on choisit la clé, il formate, met la dernière version dedans,
//! téléchargeable depuis le site ». C'est, en une fenêtre, exactement ce que
//! nous faisions à la main pour sa clé — et qui a marché :
//!
//!   1. la dernière version publiée (GitHub), son ISO TÉLÉCHARGÉE puis
//!      VÉRIFIÉE par l'empreinte SHA-256 que GitHub publie ;
//!   2. la clé effacée, une partition FAT32 (MBR, active) — ce que le
//!      firmware UEFI lit, Secure Boot compris ;
//!   3. l'ISO ouverte par Windows (Mount-DiskImage), ses fichiers COPIÉS ;
//!   4. chaque fichier de la clé revérifié contre sha256sum.txt.
//!
//! Garde-fous : seuls les disques USB sont proposés, jamais un disque
//! interne ni celui de Windows ; Windows vérifie encore avant d'effacer.
//!
//! `grenos-createur --essai <disque> <iso> <journal> [--virtuel]` fait le
//! même travail sans fenêtre : c'est ainsi que la CI le prouve, sur un disque
//! virtuel (createur.yml).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const DEPOT: &str = "Grenofar/grenOS";
const TAMPON: usize = 4 * 1024 * 1024;

#[derive(Serialize, Clone, Debug)]
struct Cle {
    numero: u32,
    nom: String,
    taille: u64,
    lettres: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Version {
    nom: String,
    fichier: String,
    adresse: String,
    taille: u64,
    empreinte: String,
}

#[derive(Serialize, Clone, Debug)]
struct Avancee {
    etape: String,
    fait: u64,
    total: u64,
    texte: String,
}

/// Où dire l'avancée : la fenêtre (événement « avancee »), ou le journal de
/// l'essai sans fenêtre.
trait Dire: Send + Sync {
    fn dire(&self, a: Avancee);
}

struct Fenetre(tauri::AppHandle);
impl Dire for Fenetre {
    fn dire(&self, a: Avancee) {
        use tauri::Emitter;
        let _ = self.0.emit("avancee", a);
    }
}

struct Journal(std::sync::Mutex<fs::File>);
impl Dire for Journal {
    fn dire(&self, a: Avancee) {
        if let Ok(mut f) = self.0.lock() {
            let _ = writeln!(f, "{} {}/{} {}", a.etape, a.fait, a.total, a.texte);
        }
    }
}

fn avancee(etape: &str, fait: u64, total: u64, texte: impl Into<String>) -> Avancee {
    Avancee { etape: etape.into(), fait, total, texte: texte.into() }
}

static EN_COURS: AtomicBool = AtomicBool::new(false);

// ------------------------------------------------------------------ Windows

/// Un script PowerShell, sortie en UTF-8, sans fenêtre noire.
fn powershell(script: &str) -> Result<String, String> {
    let entete = "$ErrorActionPreference = 'Stop'; [Console]::OutputEncoding = [Text.Encoding]::UTF8; ";
    let mut c = std::process::Command::new("powershell.exe");
    c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command"])
        .arg(format!("{entete}{script}"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let sortie = c.output().map_err(|e| format!("PowerShell introuvable : {e}"))?;
    if sortie.status.success() {
        Ok(String::from_utf8_lossy(&sortie.stdout).trim().to_string())
    } else {
        let erreur = String::from_utf8_lossy(&sortie.stderr);
        // Le message utile est la première ligne ; le reste est la pile.
        Err(erreur.lines().find(|l| !l.trim().is_empty()).unwrap_or("PowerShell a échoué").trim().to_string())
    }
}

fn guillemets(chemin: &Path) -> String {
    format!("'{}'", chemin.display().to_string().replace('\'', "''"))
}

/// Les clés USB branchées. JAMAIS un disque interne, ni celui de Windows
/// (IsSystem / IsBoot), même branché en USB.
#[tauri::command]
fn cles() -> Result<Vec<Cle>, String> {
    lister_cles(false)
}

fn lister_cles(virtuel: bool) -> Result<Vec<Cle>, String> {
    let bus = if virtuel { "@('USB', 'File Backed Virtual')" } else { "@('USB')" };
    let json = powershell(&format!(
        "$r = @(Get-Disk | Where-Object {{ {bus} -contains [string]$_.BusType -and -not $_.IsSystem -and -not $_.IsBoot }} | ForEach-Object {{ \
           $l = @(Get-Partition -DiskNumber $_.Number -ErrorAction SilentlyContinue | Where-Object DriveLetter | ForEach-Object {{ [string]$_.DriveLetter }}); \
           [pscustomobject]@{{ numero = [int]$_.Number; nom = [string]$_.FriendlyName; taille = [uint64]$_.Size; lettres = $l }} }}); \
         ConvertTo-Json -InputObject $r -Depth 3 -Compress"
    ))?;
    let valeur: serde_json::Value = serde_json::from_str(if json.is_empty() { "[]" } else { &json })
        .map_err(|e| format!("liste des clés illisible : {e}"))?;
    let liste = match valeur {
        serde_json::Value::Array(a) => a,
        autre => vec![autre],
    };
    Ok(liste
        .iter()
        .map(|c| Cle {
            numero: c["numero"].as_u64().unwrap_or(0) as u32,
            nom: c["nom"].as_str().unwrap_or("Clé USB").trim().to_string(),
            taille: c["taille"].as_u64().unwrap_or(0),
            lettres: c["lettres"]
                .as_array()
                .map(|l| l.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default(),
        })
        .collect())
}

// ------------------------------------------------------------------ GitHub

#[tauri::command]
fn derniere_version() -> Result<Version, String> {
    let reponse: serde_json::Value = ureq::get(&format!("https://api.github.com/repos/{DEPOT}/releases/latest"))
        .set("User-Agent", "grenos-createur")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("GitHub ne répond pas : {e}"))?
        .into_json()
        .map_err(|e| format!("réponse de GitHub illisible : {e}"))?;
    let tag = reponse["tag_name"].as_str().unwrap_or("").to_string();
    let iso = reponse["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"].as_str().is_some_and(|n| n.ends_with(".iso"))))
        .ok_or("aucune ISO dans la dernière version")?;
    Ok(Version {
        nom: tag.trim_start_matches("linux-").to_string(),
        fichier: iso["name"].as_str().unwrap_or("grenos.iso").to_string(),
        adresse: iso["browser_download_url"].as_str().unwrap_or("").to_string(),
        taille: iso["size"].as_u64().unwrap_or(0),
        empreinte: iso["digest"].as_str().unwrap_or("").trim_start_matches("sha256:").to_lowercase(),
    })
}

fn dossier_cache() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("grenOS").join("createur")
}

fn empreinte(chemin: &Path, dire: &dyn Dire, etape: &str) -> Result<String, String> {
    let mut f = fs::File::open(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
    let total = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut h = Sha256::new();
    let mut tampon = vec![0u8; TAMPON];
    let (mut fait, mut dernier) = (0u64, Instant::now());
    loop {
        let n = f.read(&mut tampon).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&tampon[..n]);
        fait += n as u64;
        if dernier.elapsed().as_millis() > 250 {
            dire.dire(avancee(etape, fait, total, ""));
            dernier = Instant::now();
        }
    }
    Ok(format!("{:x}", h.finalize()))
}

/// L'ISO dans le cache, téléchargée seulement si elle manque ou ne
/// correspond pas à l'empreinte publiée.
fn obtenir_iso(version: &Version, dire: &dyn Dire) -> Result<PathBuf, String> {
    let dossier = dossier_cache();
    fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    let chemin = dossier.join(&version.fichier);
    if chemin.exists() && !version.empreinte.is_empty() {
        dire.dire(avancee("verifier-iso", 0, version.taille, "ISO déjà téléchargée : vérification"));
        if empreinte(&chemin, dire, "verifier-iso")? == version.empreinte {
            dire.dire(avancee("telecharger", version.taille, version.taille, "déjà là, vérifiée"));
            return Ok(chemin);
        }
    }
    let partiel = chemin.with_extension("iso.partiel");
    let reponse = ureq::get(&version.adresse)
        .set("User-Agent", "grenos-createur")
        .call()
        .map_err(|e| format!("téléchargement impossible : {e}"))?;
    let total = reponse.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(version.taille);
    let mut lecteur = reponse.into_reader();
    let mut sortie = fs::File::create(&partiel).map_err(|e| e.to_string())?;
    let mut tampon = vec![0u8; TAMPON];
    let (mut fait, mut dernier, debut) = (0u64, Instant::now(), Instant::now());
    loop {
        let n = lecteur.read(&mut tampon).map_err(|e| format!("téléchargement coupé : {e}"))?;
        if n == 0 {
            break;
        }
        sortie.write_all(&tampon[..n]).map_err(|e| e.to_string())?;
        fait += n as u64;
        if dernier.elapsed().as_millis() > 250 {
            let debit = fait as f64 / debut.elapsed().as_secs_f64().max(0.1) / 1_048_576.0;
            dire.dire(avancee("telecharger", fait, total, format!("{debit:.1} Mo/s")));
            dernier = Instant::now();
        }
    }
    sortie.flush().map_err(|e| e.to_string())?;
    drop(sortie);
    if total > 0 && fait != total {
        return Err(format!("téléchargement incomplet ({fait} octets sur {total})"));
    }
    if !version.empreinte.is_empty() {
        dire.dire(avancee("verifier-iso", 0, total, "vérification de l'empreinte"));
        let vue = empreinte(&partiel, dire, "verifier-iso")?;
        if vue != version.empreinte {
            let _ = fs::remove_file(&partiel);
            return Err("l'ISO téléchargée ne correspond pas à l'empreinte publiée : elle a été jetée, réessaie".into());
        }
    }
    let _ = fs::remove_file(&chemin);
    fs::rename(&partiel, &chemin).map_err(|e| e.to_string())?;
    Ok(chemin)
}

// ------------------------------------------------------------------ la clé

/// Efface la clé et y crée une partition FAT32 (MBR, active), au plus
/// 32 Go (la limite du FAT32 de Windows). Rend sa lettre.
fn preparer_cle(numero: u32, virtuel: bool) -> Result<String, String> {
    let bus = if virtuel { "@('USB', 'File Backed Virtual')" } else { "@('USB')" };
    powershell(&format!(
        "$n = {numero}; $d = Get-Disk -Number $n; \
         if (-not ({bus} -contains [string]$d.BusType) -or $d.IsSystem -or $d.IsBoot) {{ throw 'Ce disque n''est pas une clé USB : rien n''a été touché.' }}; \
         if ($d.IsOffline) {{ Set-Disk -Number $n -IsOffline $false }}; \
         if ($d.IsReadOnly) {{ Set-Disk -Number $n -IsReadOnly $false }}; \
         if ([string]$d.PartitionStyle -ne 'RAW') {{ Clear-Disk -Number $n -RemoveData -RemoveOEM -Confirm:$false }}; \
         Initialize-Disk -Number $n -PartitionStyle MBR; \
         $taille = [Math]::Min([uint64]$d.Size - 2MB, [uint64](32GB - 4MB)); \
         $p = New-Partition -DiskNumber $n -Size $taille -IsActive -AssignDriveLetter; \
         $null = Format-Volume -Partition $p -FileSystem FAT32 -NewFileSystemLabel 'GRENOS' -Confirm:$false -Force; \
         Start-Sleep -Milliseconds 800; \
         $l = (Get-Partition -DiskNumber $n -PartitionNumber $p.PartitionNumber).DriveLetter; \
         if (-not $l) {{ throw 'La clé est formatée, mais Windows ne lui a pas donné de lettre.' }}; \
         Write-Output $l"
    ))
    .map(|l| l.trim().to_string())
}

fn monter_iso(iso: &Path) -> Result<String, String> {
    let i = guillemets(iso);
    powershell(&format!(
        "Dismount-DiskImage -ImagePath {i} -ErrorAction SilentlyContinue | Out-Null; \
         $m = Mount-DiskImage -ImagePath {i} -PassThru; Start-Sleep -Milliseconds 800; \
         $l = ($m | Get-Volume).DriveLetter; if (-not $l) {{ throw 'Windows n''a pas ouvert l''ISO.' }}; Write-Output $l"
    ))
    .map(|l| l.trim().to_string())
}

fn demonter_iso(iso: &Path) {
    let _ = powershell(&format!("Dismount-DiskImage -ImagePath {} | Out-Null", guillemets(iso)));
}

fn lister(racine: &Path, relatif: &Path, liste: &mut Vec<(PathBuf, u64)>) -> Result<(), String> {
    for e in fs::read_dir(racine.join(relatif)).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let r = relatif.join(e.file_name());
        let t = e.file_type().map_err(|e| e.to_string())?;
        if t.is_dir() {
            lister(racine, &r, liste)?;
        } else {
            liste.push((r, e.metadata().map(|m| m.len()).unwrap_or(0)));
        }
    }
    Ok(())
}

fn copier(source: &Path, cible: &Path, dire: &dyn Dire) -> Result<u64, String> {
    let mut liste = Vec::new();
    lister(source, Path::new(""), &mut liste)?;
    let total: u64 = liste.iter().map(|(_, t)| t).sum();
    let mut tampon = vec![0u8; TAMPON];
    let (mut fait, mut dernier) = (0u64, Instant::now());
    for (relatif, _) in &liste {
        let destination = cible.join(relatif);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{} : {e}", parent.display()))?;
        }
        // Une copie octet par octet, pas fs::copy : les fichiers de l'ISO sont
        // en lecture seule, et cet attribut suivrait sur la clé.
        let mut entree = fs::File::open(source.join(relatif)).map_err(|e| format!("{} : {e}", relatif.display()))?;
        let mut sortie = fs::File::create(&destination).map_err(|e| format!("{} : {e}", destination.display()))?;
        loop {
            let n = entree.read(&mut tampon).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            sortie.write_all(&tampon[..n]).map_err(|e| format!("écriture sur la clé : {e}"))?;
            fait += n as u64;
            if dernier.elapsed().as_millis() > 250 {
                dire.dire(avancee("copier", fait, total, relatif.display().to_string()));
                dernier = Instant::now();
            }
        }
        sortie.sync_all().map_err(|e| format!("écriture sur la clé : {e}"))?;
    }
    dire.dire(avancee("copier", total, total, format!("{} fichiers", liste.len())));
    Ok(liste.len() as u64)
}

/// Chaque fichier de sha256sum.txt, relu SUR LA CLÉ et comparé.
fn verifier(cle: &Path, dire: &dyn Dire) -> Result<u64, String> {
    let texte = fs::read_to_string(cle.join("sha256sum.txt")).map_err(|e| format!("sha256sum.txt absent de la clé : {e}"))?;
    let lignes: Vec<(String, String)> = texte
        .lines()
        .filter_map(|l| {
            let (h, chemin) = l.split_once(char::is_whitespace)?;
            let chemin = chemin.trim().trim_start_matches('*').trim_start_matches("./");
            (h.len() == 64).then(|| (h.to_lowercase(), chemin.replace('/', "\\")))
        })
        .collect();
    let total = lignes.len() as u64;
    let mut fautes = Vec::new();
    for (i, (attendue, chemin)) in lignes.iter().enumerate() {
        dire.dire(avancee("verifier", i as u64, total, chemin.clone()));
        let vue = empreinte(&cle.join(chemin), &Silence, "verifier").unwrap_or_default();
        if &vue != attendue {
            fautes.push(chemin.clone());
        }
    }
    if !fautes.is_empty() {
        return Err(format!("{} fichier(s) abîmé(s) sur la clé : {}", fautes.len(), fautes.join(", ")));
    }
    dire.dire(avancee("verifier", total, total, format!("{total} fichiers identiques")));
    Ok(total)
}

struct Silence;
impl Dire for Silence {
    fn dire(&self, _a: Avancee) {}
}

/// Tout le travail, dans l'ordre. L'ISO est toujours refermée, même en cas
/// d'échec.
fn creer_cle(numero: u32, iso: Option<PathBuf>, version: Option<Version>, virtuel: bool, dire: &dyn Dire) -> Result<(), String> {
    let iso = match iso {
        Some(i) => i,
        None => obtenir_iso(version.as_ref().ok_or("aucune version choisie")?, dire)?,
    };
    dire.dire(avancee("preparer", 0, 1, "effacement et FAT32"));
    let lettre = preparer_cle(numero, virtuel)?;
    dire.dire(avancee("preparer", 1, 1, format!("clé prête en {lettre}:")));
    let source = monter_iso(&iso)?;
    let resultat = copier(Path::new(&format!("{source}:\\")), Path::new(&format!("{lettre}:\\")), dire);
    demonter_iso(&iso);
    resultat?;
    verifier(Path::new(&format!("{lettre}:\\")), dire)?;
    let _ = powershell(&format!("Write-VolumeCache -DriveLetter {lettre}"));
    dire.dire(avancee("fini", 1, 1, format!("{lettre}:")));
    Ok(())
}

#[tauri::command]
fn creer(app: tauri::AppHandle, numero: u32, version: Version) -> Result<(), String> {
    if EN_COURS.swap(true, Ordering::SeqCst) {
        return Err("une clé est déjà en cours de création".into());
    }
    std::thread::spawn(move || {
        let dire = Fenetre(app);
        if let Err(e) = creer_cle(numero, None, Some(version), false, &dire) {
            dire.dire(avancee("erreur", 0, 0, e));
        }
        EN_COURS.store(false, Ordering::SeqCst);
    });
    Ok(())
}

fn essai(arguments: &[String]) -> i32 {
    // --essai <disque> <iso> <journal> [--virtuel]
    let (Some(disque), Some(iso), Some(journal)) = (arguments.get(2), arguments.get(3), arguments.get(4)) else {
        return 2;
    };
    let Ok(numero) = disque.parse::<u32>() else { return 2 };
    let virtuel = arguments.iter().any(|a| a == "--virtuel");
    let Ok(fichier) = fs::File::create(journal) else { return 2 };
    let dire = Journal(std::sync::Mutex::new(fichier));
    if let Ok(liste) = lister_cles(virtuel) {
        dire.dire(avancee("cles", liste.len() as u64, 0, format!("{liste:?}")));
    }
    match creer_cle(numero, Some(PathBuf::from(iso)), None, virtuel, &dire) {
        Ok(()) => 0,
        Err(e) => {
            dire.dire(avancee("erreur", 0, 0, e));
            1
        }
    }
}

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.get(1).map(String::as_str) == Some("--essai") {
        std::process::exit(essai(&arguments));
    }
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![cles, derniere_version, creer])
        .run(tauri::generate_context!())
        .expect("le créateur n'a pas pu démarrer");
}
