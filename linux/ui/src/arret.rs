//! Fin de session : fermer la session, mettre en veille, redémarrer, éteindre.
//!
//! Repris de `grenos-arret` (Python) le 18 novembre, règle pour règle :
//! - la veille n'est offerte que si `/sys/power/state` la permet (F1) — un
//!   bouton qui ne fait rien est la faute que ce dépôt attrape depuis des
//!   semaines ;
//! - « Mettre à jour et redémarrer » n'apparaît que s'il y a vraiment quelque
//!   chose qui attend, et dit QUOI (paquets ou changements de système) ;
//! - la page n'envoie qu'une CLÉ (« eteindre »…), jamais une commande : c'est
//!   Rust qui sait ce que chaque clé lance. Une page compromise ne peut rien
//!   faire de plus qu'un clic sur un des boutons.
//!
//! La ligne série est la même, au mot près : « arret : N geste(s), veille … ».

use crate::systeme::{dire, lire};
use serde::Serialize;

const ETATS_VEILLE: &str = "/sys/power/state";
const ATTENTE: &str = "/run/grenos/maj";
const TRAVAUX: &str = "/var/lib/grenos/au-prochain-demarrage";

#[derive(Serialize, Debug, PartialEq)]
pub struct Geste {
    cle: &'static str,
    titre: String,
    note: String,
    /// "" (normal), "danger" (rouge, Éteindre seul) ou "accent" (la mise à jour).
    genre: &'static str,
    icone: &'static str,
}

/// Ce que le noyau sait faire ici : `mem` (vraie veille), `freeze` (les
/// programmes s'arrêtent, la machine reste allumée), ou rien.
fn veille_de(etats: &str) -> Option<&'static str> {
    let etats: Vec<&str> = etats.split_whitespace().collect();
    if etats.contains(&"mem") {
        Some("mem")
    } else if etats.contains(&"freeze") {
        Some("freeze")
    } else {
        None
    }
}

fn veille() -> Option<&'static str> {
    veille_de(&lire(ETATS_VEILLE))
}

fn combien_attendent() -> u32 {
    lire(ATTENTE).trim().parse().unwrap_or(0)
}

/// Le sous-dossier `faits/` garde ce qui a déjà été appliqué : seuls les
/// fichiers qui commencent par un chiffre attendent.
fn combien_de_travaux() -> u32 {
    std::fs::read_dir(TRAVAUX)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with(|c: char| c.is_ascii_digit()))
                .count() as u32
        })
        .unwrap_or(0)
}

fn pluriel(n: u32, un: &str, plusieurs: &str) -> String {
    format!("{n} {}", if n == 1 { un } else { plusieurs })
}

fn composer(veille: Option<&str>, paquets: u32, travaux: u32) -> Vec<Geste> {
    let g = |cle, titre: &str, note: &str, genre, icone| Geste { cle, titre: titre.into(), note: note.into(), genre, icone };
    let mut liste = vec![g("session", "Fermer la session", "Revenir à l'écran de connexion.", "", "compte")];
    if let Some(etat) = veille {
        let note = if etat == "mem" {
            "La machine s'éteint presque entièrement, et retrouve tout au réveil."
        } else {
            "Les programmes s'arrêtent, la machine reste allumée."
        };
        liste.push(g("veille", "Mettre en veille", note, "", "nuit"));
    }
    liste.push(g("redemarrer", "Redémarrer", "Éteindre puis rallumer la machine.", "", "redemarrer"));
    liste.push(g("eteindre", "Éteindre", "Arrêter complètement la machine.", "danger", "eteindre"));

    if paquets > 0 || travaux > 0 {
        // « Mettre à jour » pour trois réglages système serait un mensonge.
        let mut morceaux = Vec::new();
        if paquets > 0 {
            morceaux.push(pluriel(paquets, "mise à jour", "mises à jour"));
        }
        if travaux > 0 {
            morceaux.push(pluriel(travaux, "changement", "changements"));
        }
        let titre = if paquets > 0 { "Mettre à jour et redémarrer" } else { "Appliquer et redémarrer" };
        let note = format!("Poser {} avant d'ouvrir le bureau, puis redémarrer.", morceaux.join(" et "));
        liste.insert(0, Geste { cle: "maj", titre: titre.into(), note, genre: "accent", icone: "maj" });
    }
    liste
}

#[derive(Serialize)]
pub struct Vue {
    gestes: Vec<Geste>,
    adresse: String,
}

#[tauri::command]
pub fn arret_vue() -> Vue {
    let etat = veille();
    let gestes = composer(etat, combien_attendent(), combien_de_travaux());
    dire(&format!("arret : {} geste(s), veille {}", gestes.len(), etat.unwrap_or("impossible ici")));
    Vue { gestes, adresse: crate::systeme::adresse() }
}

fn commande(cle: &str) -> Option<(&'static str, &'static [&'static str])> {
    Some(match cle {
        "session" => ("openbox", &["--exit"]),
        "veille" => ("systemctl", &["suspend"]),
        "redemarrer" => ("systemctl", &["reboot"]),
        "eteindre" => ("systemctl", &["poweroff"]),
        "maj" => ("sh", &["-c", "systemctl enable grenos-maj-demarrage.service && systemctl reboot"]),
        _ => return None,
    })
}

/// Lance le geste, détaché (la fenêtre se ferme aussitôt après), et ferme.
#[tauri::command]
pub fn arret_faire(fenetre: tauri::WebviewWindow, cle: String) -> Result<(), String> {
    // On refuse ce que la vue n'a pas proposé : pas de veille sur une machine
    // qui ne sait pas dormir, même si la page le demande.
    let offerts = composer(veille(), combien_attendent(), combien_de_travaux());
    if !offerts.iter().any(|g| g.cle == cle) {
        return Err("geste non proposé ici".into());
    }
    let (programme, arguments) = commande(&cle).ok_or("geste inconnu")?;
    crate::systeme::detacher(programme, arguments)?;
    dire(&format!("arret : {cle}"));
    let _ = fenetre.close();
    Ok(())
}

#[cfg(test)]
mod essais {
    use super::*;

    fn cles(g: &[Geste]) -> Vec<&str> {
        g.iter().map(|g| g.cle).collect()
    }

    #[test]
    fn la_veille_suit_le_noyau() {
        assert_eq!(veille_de("freeze mem disk\n"), Some("mem"));
        assert_eq!(veille_de("freeze\n"), Some("freeze"));
        assert_eq!(veille_de("disk"), None);
        assert_eq!(veille_de(""), None);
    }

    #[test]
    fn sans_veille_pas_de_bouton() {
        assert_eq!(cles(&composer(None, 0, 0)), ["session", "redemarrer", "eteindre"]);
        assert_eq!(cles(&composer(Some("freeze"), 0, 0)), ["session", "veille", "redemarrer", "eteindre"]);
    }

    #[test]
    fn la_mise_a_jour_dit_ce_qui_attend() {
        let g = composer(None, 3, 0);
        assert_eq!(g[0].cle, "maj");
        assert_eq!(g[0].titre, "Mettre à jour et redémarrer");
        assert!(g[0].note.contains("3 mises à jour"));
        let g = composer(None, 0, 1);
        assert_eq!(g[0].titre, "Appliquer et redémarrer");
        assert!(g[0].note.contains("1 changement "));
        let g = composer(None, 1, 2);
        assert!(g[0].note.contains("1 mise à jour et 2 changements"));
    }

    #[test]
    fn seul_eteindre_est_rouge() {
        let g = composer(Some("mem"), 2, 2);
        let rouges: Vec<_> = g.iter().filter(|g| g.genre == "danger").map(|g| g.cle).collect();
        assert_eq!(rouges, ["eteindre"]);
    }

    #[test]
    fn chaque_geste_a_sa_commande() {
        for g in composer(Some("mem"), 1, 1) {
            assert!(commande(g.cle).is_some(), "{}", g.cle);
        }
        assert!(commande("rm").is_none());
    }
}
