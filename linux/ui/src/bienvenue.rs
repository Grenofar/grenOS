//! Bienvenue : ce qu'on peut faire, tout de suite, en un clic.
//!
//! Repris de `grenos-bienvenue` (Python) le 18 novembre. Les mêmes gestes, les
//! mêmes astuces, et la même règle : « Installer grenOS » n'apparaît que sur
//! une vraie machine démarrée par la clé — dans une machine virtuelle, le
//! disque garde déjà tout, et proposer d'installer par-dessus ne ferait
//! qu'inquiéter. Sur un grenOS DÉJÀ installé, la carte ne menait à rien : le
//! moteur refuse hors de la clé (22 novembre).
//!
//! La clé n'a plus d'assistant (22 novembre) : cet accueil est le premier
//! écran qu'on y voit, il dit donc qu'on essaie, et non « ta machine ».

use crate::{materiel, systeme};
use serde::Serialize;

#[derive(Serialize)]
pub struct Geste {
    nom: &'static str,
    note: &'static str,
    programme: &'static str,
    icone: &'static str,
}

const fn g(nom: &'static str, note: &'static str, programme: &'static str, icone: &'static str) -> Geste {
    Geste { nom, note, programme, icone }
}

const GESTES: [Geste; 8] = [
    g("GrenPlace", "Ajouter des applications en un clic — Steam, un traitement de texte, un éditeur de code.", "grenplace", "paquet"),
    g("Jeux", "Ce que cette machine sait faire tourner, et ce qu'il lui manque. La vérité sur chaque jeu, y compris ceux qui ne marchent pas.", "grenos-jeux", "manette"),
    g("Connexions", "Le Wi-Fi et le Bluetooth, dans une fenêtre.", "grenos-connexions", "wifi"),
    g("Réglages", "Le thème jour et nuit, l'écran, le son, l'heure, le mot de passe, les mises à jour.", "grenos-parametres", "reglages"),
    g("Mettre à jour", "Les nouveautés de grenOS et les correctifs de sécurité, sans retélécharger l'image.", "grenos-maj", "maj"),
    g("Naviguer", "Firefox est là, avec ses onglets et ses favoris.", "firefox-esr", "langue"),
    g("Tes fichiers", "Ouvrir, renommer, copier, supprimer — et tout ce que tu poses sur le bureau reste là.", "grenos-fichiers", "dossier"),
    g("Terminal", "Pour qui veut taper des commandes. Les autres n'en ont pas besoin.", "x-terminal-emulator", "terminal"),
];

const INSTALLER: Geste = g(
    "Installer grenOS",
    "Poser le système sur le disque de ce PC, dans un espace libre ou à la place de ce qui s'y trouve. Rien n'est touché avant que tu ne valides.",
    "installer",
    "disque",
);

const ASTUCES: [&str; 6] = [
    "Clic droit sur le bureau : créer un dossier, un fichier, changer le fond.",
    "Glisser une application du menu vers le bureau en fait un raccourci ; clic droit sur une icône de la barre pour l'y épingler ou l'en retirer.",
    "Alt+Tab passe d'une fenêtre à l'autre, Alt+F4 ferme celle du dessus.",
    "Impr. écran capture tout l'écran, Maj+Impr. écran une zone choisie ; les images vont dans le dossier Images.",
    "Copier seulement, sans fichier : Meta+Maj+S une zone, Ctrl+Impr. écran tout l'écran — puis Ctrl+V pour coller.",
    "Ctrl+Maj+Échap ouvre le gestionnaire de tâches, Ctrl+Alt+Échap le mode jeu, Meta+I les Réglages, Meta+E tes fichiers, Meta+T bascule jour et nuit.",
];

const ASTUCE_VIRTUELLE: &str = "Cette machine est virtuelle : tout ce que tu fais est déjà gardé sur son disque. Pour lui donner plus de mémoire ou de cœurs, éteins-la et change ses réglages côté hôte.";

#[derive(Serialize)]
pub struct Vue {
    salut: String,
    adresse: String,
    gestes: Vec<&'static Geste>,
    astuces: Vec<&'static str>,
    /// Sur la clé, d'un vrai PC : on essaie avant d'installer.
    essai: bool,
}

type Compose = (String, Vec<&'static Geste>, Vec<&'static str>);

fn composer(nom: &str, virtuelle: bool, cle: bool) -> Compose {
    let salut = if nom.is_empty() { "Bienvenue".to_string() } else { format!("Bonjour {nom}") };
    let mut gestes: Vec<&'static Geste> = GESTES.iter().collect();
    let mut astuces = ASTUCES.to_vec();
    if virtuelle {
        astuces.push(ASTUCE_VIRTUELLE);
    } else if cle {
        // Sur la clé d'un vrai PC, installer est le geste qui compte : devant.
        gestes.insert(0, &INSTALLER);
    }
    (salut, gestes, astuces)
}

#[tauri::command]
pub async fn bienvenue_vue() -> Vue {
    let id = systeme::identite();
    let virtuelle = !materiel::virtuelle().is_empty();
    let cle = systeme::sur_cle();
    let (salut, gestes, astuces) = composer(&id["nom"], virtuelle, cle);
    Vue { salut, adresse: systeme::adresse(), gestes, astuces, essai: cle && !virtuelle }
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn installer_seulement_depuis_la_cle_d_un_vrai_pc() {
        let (_, g, a) = composer("", true, true);
        assert!(g.iter().all(|g| g.programme != "installer"));
        assert_eq!(a.last(), Some(&ASTUCE_VIRTUELLE));
        let (_, g, a) = composer("", false, true);
        assert_eq!(g[0].programme, "installer");
        assert!(!a.contains(&ASTUCE_VIRTUELLE));
        let (_, g, _) = composer("Léa", false, false);
        assert!(g.iter().all(|g| g.programme != "installer"), "deja installe : rien a installer");
    }

    #[test]
    fn chaque_geste_est_permis() {
        for g in GESTES.iter().chain([&INSTALLER]) {
            assert!(systeme::PROGRAMMES.contains(&g.programme), "{}", g.programme);
        }
    }

    #[test]
    fn le_salut() {
        assert_eq!(composer("", true, false).0, "Bienvenue");
        assert_eq!(composer("Léa", true, false).0, "Bonjour Léa");
    }
}
