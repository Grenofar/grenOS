//! « Installer grenOS » (22 novembre), toutes ses pages : bienvenue, clavier
//! et région, disque (supprimer, créer pour grenOS, taille en Gio), compte,
//! résumé, installation, terminé.
//!
//! Tout ce qui demande root passe par UN programme, `grenos-installation`,
//! qui revérifie le plan en entier avant d'écrire. La page ne donne qu'un
//! plan ; Rust en vérifie la forme et relaie l'avancée.

use crate::premier;
use crate::systeme::{dire, identite, lire, privilegie, reussit, sortie};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use tauri::Emitter;

const MOTEUR: &str = "/usr/lib/grenos/grenos-installation";

#[derive(Serialize, Clone)]
pub struct Avance {
    part: u32,
    texte: String,
}

#[derive(Serialize, Clone)]
struct Fin {
    bien: bool,
    texte: String,
}

/// L'installation en cours, s'il y en a une : une fenêtre rouverte la
/// retrouve au lieu de reproposer les disques.
static EN_COURS: Mutex<Option<Avance>> = Mutex::new(None);

pub fn en_cours() -> Option<Avance> {
    EN_COURS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Fermer la fenêtre au milieu de la copie laisserait croire que tout s'est
/// arrêté : elle attend la fin. Demandé par `main::fermeture_animee` — un
/// gestionnaire posé ici ne suffisait pas, le fondu la DÉTRUISAIT quand même.
pub fn retient(fenetre: &str, installe: bool) -> bool {
    fenetre == "installation" && installe
}

fn noter(etat: Option<Avance>) {
    *EN_COURS.lock().unwrap_or_else(|e| e.into_inner()) = etat;
}

// Pas de `Debug` ici : un plan porte un mot de passe, il ne s'affiche pas.
#[derive(Serialize, Deserialize, Clone)]
pub struct Compte {
    login: String,
    nom: String,
    mot_de_passe: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Disposition {
    disposition: String,
    variante: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Plan {
    disque: String,
    empreinte: String,
    effacer: bool,
    supprimer: Vec<String>,
    zone_mio: Option<u64>,
    taille_mio: u64,
    compte: Compte,
    clavier: Disposition,
    fuseau: String,
}

/// /dev/sda, /dev/nvme0n1p3 : rien d'autre qu'un nom de périphérique.
pub fn peripherique_valide(d: &str) -> bool {
    d.strip_prefix("/dev/")
        .is_some_and(|nom| (2..=24).contains(&nom.len()) && nom.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
}

#[derive(Debug, PartialEq)]
pub enum Dit {
    Avance(u32, String),
    Erreur(String),
    Fini,
    Autre,
}

/// Une ligne du moteur.
pub fn dit(ligne: &str) -> Dit {
    let ligne = ligne.trim();
    if ligne == "FINI" {
        return Dit::Fini;
    }
    if let Some(texte) = ligne.strip_prefix("ERREUR ") {
        return Dit::Erreur(texte.to_string());
    }
    if let Some((part, texte)) = ligne.strip_prefix("AVANCE ").and_then(|r| r.split_once(' ')) {
        if let Ok(part) = part.parse::<u32>() {
            return Dit::Avance(part.min(100), texte.to_string());
        }
    }
    Dit::Autre
}

fn moteur(action: &str) -> Command {
    let p = privilegie();
    let mut commande = Command::new(&p[0]);
    commande.args(&p[1..]).arg(MOTEUR).arg(action);
    commande
}

/// Pourquoi le moteur n'a pas répondu, en une phrase.
fn raison(code: Option<i32>, erreurs: &str) -> String {
    // 126 : fenêtre du mot de passe fermée ; 127 : autorisation refusée (pkexec).
    if matches!(code, Some(126) | Some(127)) {
        return "L'autorisation n'a pas été donnée : rien n'a été touché.".into();
    }
    match erreurs.lines().rev().find(|l| !l.trim().is_empty()) {
        Some(l) => l.trim().to_string(),
        None => "Le moteur de l'installation n'a pas répondu.".into(),
    }
}

#[derive(Serialize)]
pub struct Infos {
    cle: bool,
    efi: bool,
    secure_boot: &'static str,
    secure_boot_phrase: &'static str,
    claviers: Vec<premier::Clavier>,
    fuseaux: Vec<String>,
    /// Ce que l'assistant de la clé sait déjà : on ne repart pas de zéro.
    nom: String,
    clavier: String,
    variante: String,
    fuseau: String,
    en_cours: Option<Avance>,
}

/// Tout ce que les pages montrent avant de toucher aux disques : sans root,
/// donc tout de suite.
#[tauri::command]
pub async fn installation_infos() -> Infos {
    let connu = identite();
    let v = |c: &str| connu.get(c).cloned().unwrap_or_default();
    let secure_boot = premier::secure_boot("/sys/firmware/efi/efivars");
    let claviers = premier::donnees().claviers;
    let mut fuseaux: Vec<String> = sortie("timedatectl", &["list-timezones", "--no-pager"])
        .lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    for c in &claviers {
        if !fuseaux.contains(&c.fuseau) {
            fuseaux.push(c.fuseau.clone());
        }
    }
    fuseaux.sort();
    Infos {
        cle: std::path::Path::new("/run/live/medium").is_dir(),
        efi: std::path::Path::new("/sys/firmware/efi").is_dir(),
        secure_boot,
        secure_boot_phrase: premier::mot_sur_secure_boot(secure_boot),
        claviers,
        fuseaux,
        nom: v("nom"),
        clavier: v("clavier"),
        variante: v("variante"),
        fuseau: v("fuseau"),
        en_cours: en_cours(),
    }
}

/// Un compte ou un groupe du système porte-t-il ce nom ? (useradd refuserait)
pub fn pris_par_le_systeme(tables: &str, login: &str) -> bool {
    tables.lines().any(|l| {
        let c: Vec<&str> = l.split(':').collect();
        c.len() > 2 && c[0] == login && c[2].parse::<u32>().is_ok_and(|id| !(1000..60000).contains(&id))
    })
}

#[derive(Serialize)]
pub struct Nom {
    login: String,
    probleme: Option<String>,
}

#[tauri::command]
pub async fn installation_nom(texte: String) -> Nom {
    let texte = texte.trim();
    let login = premier::en_login(texte);
    let probleme = if texte.chars().count() > 60 {
        Some("Ce nom est trop long (60 signes au plus).")
    } else if texte.chars().any(|c| c == ':' || c == ',' || c.is_control()) {
        Some("Le nom ne peut contenir ni « : » ni « , ».")
    } else if let Some(souci) = premier::probleme_de_forme(&login) {
        Some(souci)
    } else if pris_par_le_systeme(&format!("{}\n{}", lire("/etc/passwd"), lire("/etc/group")), &login) {
        Some("Ce nom est déjà pris par le système, choisis-en un autre.")
    } else {
        None
    };
    Nom { login, probleme: probleme.map(String::from) }
}

#[tauri::command]
pub async fn installation_lire() -> Result<Value, String> {
    if let Some(a) = en_cours() {
        return Ok(serde_json::json!({ "en_cours": a }));
    }
    let o = moteur("lister")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("L'installateur n'a pas pu démarrer : {e}"))?;
    if !o.status.success() {
        return Err(raison(o.status.code(), &String::from_utf8_lossy(&o.stderr)));
    }
    let lu: Value = serde_json::from_slice(&o.stdout).map_err(|e| format!("La liste des disques est illisible : {e}"))?;
    let n = lu["disques"].as_array().map(|d| d.len()).unwrap_or(0);
    dire(&format!("installation : ecran ouvert, {n} disque(s) proposes"));
    Ok(lu)
}

#[tauri::command]
pub async fn installation_lancer(fenetre: tauri::WebviewWindow, plan: Plan) -> Result<(), String> {
    if !peripherique_valide(&plan.disque) || !plan.supprimer.iter().all(|p| peripherique_valide(p)) {
        return Err("plan refusé : rien n'a été touché".into());
    }
    let corps = serde_json::to_vec(&plan).map_err(|e| e.to_string())?;
    {
        let mut verrou = EN_COURS.lock().unwrap_or_else(|e| e.into_inner());
        if verrou.is_some() {
            return Err("Une installation est déjà en cours.".into());
        }
        *verrou = Some(Avance { part: 0, texte: "Vérification du plan".into() });
    }
    let mut enfant = match moteur("installer").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(e) => e,
        Err(e) => {
            noter(None);
            return Err(format!("L'installateur n'a pas pu démarrer : {e}"));
        }
    };
    if let Some(mut entree) = enfant.stdin.take() {
        let _ = entree.write_all(&corps);
    }
    dire(&format!("installation : lancee sur {}", plan.disque));
    let erreurs = enfant.stderr.take().map(|mut e| {
        std::thread::spawn(move || {
            let mut t = Vec::new();
            let _ = e.read_to_end(&mut t);
            String::from_utf8_lossy(&t).into_owned()
        })
    });
    std::thread::spawn(move || {
        let (mut fini, mut erreur) = (false, None);
        if let Some(sortie) = enfant.stdout.take() {
            for ligne in BufReader::new(sortie).split(b'\n') {
                let Ok(ligne) = ligne else { break };
                match dit(&String::from_utf8_lossy(&ligne)) {
                    Dit::Avance(part, texte) => {
                        let a = Avance { part, texte };
                        noter(Some(a.clone()));
                        let _ = fenetre.emit("installation-avance", a);
                    }
                    Dit::Erreur(texte) => erreur = Some(texte),
                    Dit::Fini => fini = true,
                    Dit::Autre => {}
                }
            }
        }
        let statut = enfant.wait().ok();
        let dites = erreurs.and_then(|f| f.join().ok()).unwrap_or_default();
        // « Installé » seulement si le moteur l'a dit ET a bien fini.
        let bien = fini && statut.is_some_and(|s| s.success());
        let texte = if bien {
            String::new()
        } else {
            erreur.unwrap_or_else(|| raison(statut.and_then(|s| s.code()), &dites))
        };
        noter(None);
        dire(&if bien { "installation : terminee".to_string() } else { format!("installation : echec — {texte}") });
        let _ = fenetre.emit("installation-fin", Fin { bien, texte });
    });
    Ok(())
}

#[tauri::command]
pub async fn installation_redemarrer() -> Result<(), String> {
    if reussit("systemctl", &["reboot"]) {
        Ok(())
    } else {
        Err("Le redémarrage a été refusé : éteins le PC depuis le menu de grenOS.".into())
    }
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn les_lignes_du_moteur() {
        assert_eq!(dit("AVANCE 42 Copie de grenOS — 51 %"), Dit::Avance(42, "Copie de grenOS — 51 %".into()));
        assert_eq!(dit("AVANCE 400 trop"), Dit::Avance(100, "trop".into()));
        assert_eq!(dit("ERREUR le disque a changé"), Dit::Erreur("le disque a changé".into()));
        assert_eq!(dit("FINI"), Dit::Fini);
        assert_eq!(dit("AVANCE x y"), Dit::Autre);
        assert_eq!(dit("presque FINI"), Dit::Autre);
    }

    #[test]
    fn les_peripheriques() {
        for bon in ["/dev/sda", "/dev/sda2", "/dev/nvme0n1p3", "/dev/mmcblk0"] {
            assert!(peripherique_valide(bon), "{bon}");
        }
        for mauvais in ["sda", "/dev/", "/dev/../etc", "/dev/sda;reboot", "/dev/mapper/x", "--wipe"] {
            assert!(!peripherique_valide(mauvais), "{mauvais}");
        }
    }

    #[test]
    fn le_plan_de_la_page_se_lit() {
        // Le plan relevé dans l'aperçu, mot de passe remplacé : un champ que
        // Rust ignorerait n'arriverait jamais au moteur.
        let envoye = serde_json::json!({
            "disque": "/dev/sda", "empreinte": "s", "effacer": true, "supprimer": [], "zone_mio": null, "taille_mio": 0,
            "compte": { "login": "camille-durand", "nom": "Camille Durand", "mot_de_passe": "un-essai" },
            "clavier": { "disposition": "ca", "variante": "" }, "fuseau": "America/Montreal",
        });
        let plan: Plan = serde_json::from_value(envoye.clone()).unwrap();
        assert!(serde_json::to_value(&plan).unwrap() == envoye, "un champ du plan s'est perdu");
    }

    #[test]
    fn les_noms_du_systeme() {
        let tables = "audio:x:29:\ncamille:x:1000:1000::/home/camille:/bin/bash\nnobody:x:65534:65534::/:/bin/false\n";
        assert!(pris_par_le_systeme(tables, "audio"));
        assert!(pris_par_le_systeme(tables, "nobody"));
        assert!(!pris_par_le_systeme(tables, "camille"));
    }

    #[test]
    fn la_fenetre_attend_la_fin() {
        assert!(retient("installation", true));
        assert!(!retient("installation", false));
        assert!(!retient("reglages", true));
    }

    #[test]
    fn un_refus_de_mot_de_passe_se_dit() {
        assert!(raison(Some(126), "").contains("rien n'a été touché"));
    }
}
