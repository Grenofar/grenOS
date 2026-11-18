//! Mode sans échec : ce que le dernier démarrage a fait, et ce qu'on peut faire.
//!
//! Repris de `grenos-secours` et `secours.py` le 18 novembre (C15–C17), règle
//! pour règle :
//! - la cause vient du journal du démarrage précédent (`derniere-cause`),
//!   jamais d'une supposition ;
//! - une action impossible est MONTRÉE éteinte avec sa raison, jamais cachée ;
//! - « Désactiver le pilote » ne s'allume que si l'on sait QUEL module nommer,
//!   et jamais pour un module du cœur (`ahci`, `nvme`, `ext4`…) : les mettre
//!   en liste noire empêcherait la machine de voir son disque.
//!
//! `secours.py` reste : le service de démarrage l'utilise. Les deux lisent les
//! mêmes fichiers, et les essais ci-dessous reprennent ses cas.

use crate::systeme::{detacher, dire, lire};
use serde::Serialize;

const ETAT: &str = "/var/lib/grenos";

const INTOUCHABLES: &[&str] = &[
    "ahci", "nvme", "sd_mod", "ext4", "btrfs", "xfs", "usbhid", "hid_generic",
    "xhci_hcd", "ehci_hcd", "atkbd", "i8042", "vfat", "overlay", "squashfs",
];

fn nom_module(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Le premier mot fait de [A-Za-z0-9_] qui commence au début de `texte`.
fn mot(texte: &str) -> &str {
    let fin = texte.find(|c: char| !nom_module(c)).unwrap_or(texte.len());
    &texte[..fin]
}

/// Les trois formes sous lesquelles le noyau nomme un module en échec (les
/// mêmes que MOTIFS dans secours.py, dans le même ordre).
fn nomme(texte: &str) -> Option<&str> {
    // « Direct firmware load for iwlwifi-cc-a0-72.ucode failed »
    if let Some(i) = texte.find("Direct firmware load for ") {
        let m = mot(&texte[i + 25..]);
        if !m.is_empty() {
            return Some(m);
        }
    }
    // « probe with driver iwlwifi failed »
    let mut reste = texte;
    while let Some(i) = reste.find("probe with driver ") {
        let apres = &reste[i + 18..];
        let m = mot(apres);
        if !m.is_empty() && apres[m.len()..].starts_with(" failed") {
            return Some(m);
        }
        reste = apres;
    }
    // « nouveau 0000:01:00.0: … » — au tout début du texte seulement.
    let m = mot(texte);
    let apres = &texte[m.len()..];
    let b = apres.as_bytes();
    let hexa = |c: u8| c.is_ascii_digit() || (b'a'..=b'f').contains(&c);
    if !m.is_empty() && b.len() >= 9 && b[0] == b' ' && b[1..5].iter().all(|&c| hexa(c)) && b[5] == b':'
        && b[6..8].iter().all(|&c| hexa(c)) && b[8] == b':'
    {
        return Some(m);
    }
    None
}

/// Le module que le journal accuse, ou None (« je ne sais pas » est une
/// réponse : le bouton reste éteint). Le nom du micrologiciel est coupé au
/// premier tiret : `iwlwifi-cc-a0` → `iwlwifi`.
pub fn module_fautif(texte: &str) -> Option<String> {
    let nom = nomme(texte)?;
    let nom = nom.split('-').next().unwrap_or("").split('/').next().unwrap_or("");
    if nom.is_empty() || INTOUCHABLES.contains(&nom) {
        return None;
    }
    Some(nom.to_string())
}

fn echecs_de(brut: &str) -> u32 {
    // Un compteur illisible n'est pas un échec : c'est un compteur illisible.
    brut.trim().parse::<i64>().map(|n| n.max(0) as u32).unwrap_or(0)
}

#[derive(Serialize, Debug)]
pub struct Proposition {
    cle: &'static str,
    intitule: String,
    possible: bool,
    pourquoi: &'static str,
}

fn propositions(module: Option<&str>, instantanes: bool) -> Vec<Proposition> {
    let p = |cle, intitule: String, possible, pourquoi| Proposition { cle, intitule, possible, pourquoi };
    vec![
        p("analyser", "Analyser la machine".into(), true, ""),
        p("journal", "Voir ce que le dernier démarrage a dit".into(), true, ""),
        p("instantane", "Restaurer un instantané".into(), instantanes,
          if instantanes { "" } else { "aucun disque d'instantanés n'est encore choisi" }),
        p("pilote",
          module.map(|m| format!("Désactiver « {m} »")).unwrap_or_else(|| "Désactiver le pilote fautif".into()),
          module.is_some(),
          if module.is_some() { "" } else { "le journal ne nomme aucun pilote, et on n'en invente pas" }),
    ]
}

fn etat(nom: &str) -> String {
    lire(&format!("{ETAT}/{nom}")).trim().to_string()
}

fn instantanes() -> bool {
    std::path::Path::new("/etc/timeshift/timeshift.json").exists()
}

#[derive(Serialize)]
pub struct Vue {
    echecs: u32,
    cause: String,
    propositions: Vec<Proposition>,
}

#[tauri::command]
pub fn secours_vue() -> Vue {
    let echecs = echecs_de(&etat("demarrages-echoues"));
    let cause = etat("derniere-cause");
    let module = module_fautif(&cause);
    let propositions = propositions(module.as_deref(), instantanes());
    let offerts = propositions.iter().filter(|p| p.possible).count();
    dire(&format!("secours : fenetre ouverte, {echecs} echec(s), {offerts} action(s) offerte(s)"));
    if !cause.is_empty() {
        let court: String = cause.chars().take(70).collect();
        dire(&format!("secours : cause affichee — {court}"));
    }
    Vue { echecs, cause, propositions }
}

/// Une action, par sa clé. Renvoie la phrase à afficher sous les boutons.
#[tauri::command]
pub fn secours_faire(cle: String) -> Result<String, String> {
    match cle.as_str() {
        "analyser" => detacher("grenos-hub", &[]).map(|_| String::new()),
        // `-b -1` : le démarrage d'AVANT, celui qui a échoué.
        "journal" => detacher("x-terminal-emulator", &["-e", "sh", "-c", "journalctl -b -1 -p err --no-pager | less"])
            .map(|_| String::new())
            .map_err(|_| "Aucun terminal pour afficher le journal.".into()),
        "instantane" if instantanes() => detacher("pkexec", &["timeshift-gtk"]).map(|_| String::new()),
        "pilote" => {
            let Some(module) = module_fautif(&etat("derniere-cause")) else {
                return Ok("Aucun pilote nommé : rien n'a été touché.".into());
            };
            // Un fichier à nous sous /etc/modprobe.d/ : aucun paquet ne le
            // possède, dpkg ne refusera rien. Le nom est fait de [A-Za-z0-9_]
            // (module_fautif) : rien à échapper dans la ligne du shell.
            let ligne = format!("printf 'blacklist {module}\\n' > /etc/modprobe.d/grenos-secours.conf");
            if crate::systeme::reussit("pkexec", &["sh", "-c", &ligne]) {
                Ok(format!("« {module} » sera désactivé au prochain démarrage. Le fichier est /etc/modprobe.d/grenos-secours.conf — l'effacer remet le pilote."))
            } else {
                Err("La demande d'autorisation n'a pas abouti.".into())
            }
        }
        _ => Err("action non proposée ici".into()),
    }
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn les_trois_formes_du_noyau() {
        assert_eq!(module_fautif("Direct firmware load for iwlwifi-cc-a0-72.ucode failed with error -2").as_deref(), Some("iwlwifi"));
        assert_eq!(module_fautif("iwlwifi 0000:00:14.3: probe with driver iwlwifi failed with error -110").as_deref(), Some("iwlwifi"));
        assert_eq!(module_fautif("nouveau 0000:01:00.0: Direct firmware load for nouveau/nv137 failed").as_deref(), Some("nouveau"));
        assert_eq!(module_fautif("r8169 0000:02:00.0: link down").as_deref(), Some("r8169"));
    }

    #[test]
    fn rien_quand_on_ne_sait_pas() {
        assert_eq!(module_fautif(""), None);
        assert_eq!(module_fautif("le disque a mis longtemps"), None);
        assert_eq!(module_fautif("plus tard : nouveau 0000:01:00.0: x"), None);
    }

    #[test]
    fn jamais_un_module_du_coeur() {
        assert_eq!(module_fautif("ahci 0000:00:17.0: probe with driver ahci failed"), None);
        assert_eq!(module_fautif("probe with driver nvme failed with error -5"), None);
    }

    #[test]
    fn compteur() {
        assert_eq!(echecs_de("3\n"), 3);
        assert_eq!(echecs_de("-2"), 0);
        assert_eq!(echecs_de("abc"), 0);
        assert_eq!(echecs_de(""), 0);
    }

    #[test]
    fn le_bouton_pilote_suit_le_module() {
        let p = propositions(None, false);
        assert!(!p[3].possible && !p[3].pourquoi.is_empty());
        assert!(!p[2].possible);
        let p = propositions(Some("iwlwifi"), true);
        assert!(p[3].possible && p[3].intitule.contains("iwlwifi"));
        assert!(p[2].possible);
    }
}
