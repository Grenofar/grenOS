//! « Préparer le matériel » — le bouton du premier bureau (23 novembre).
//!
//! Grenofar : « quand l'installation est terminée, sur le SSD, il redémarre,
//! arrive au bureau, il y a un bouton "Préparer le matériel", et ça installe
//! et met en place les drivers ». Sur sa RTX 5060, le pilote libre a donné
//! des écrans verts ; replié sur l'image du firmware, son DEUXIÈME ÉCRAN
//! restait noir (une seule sortie pilotée). Le pilote officiel de NVIDIA,
//! pris dans son dépôt, règle les deux.
//!
//! Cette page dit ce qu'elle a trouvé, ce qu'elle va faire, puis le fait par
//! l'installateur commun (installer.rs : dépôt de l'éditeur, module compilé
//! et signé pour le Secure Boot, vérifié après coup). Avec le Secure Boot,
//! elle affiche en GRAND le code à taper sur l'écran bleu du redémarrage —
//! perdu dans un journal, il ne servirait à personne.
//!
//! Le raccourci du bureau est posé à l'ouverture de session par
//! `grenos-preparer-materiel` (même règle que `etat()` ici), et retiré dès
//! que le pilote tourne.

use crate::systeme::{dire, sortie, sur_cle};
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
pub struct Etat {
    /// Le nom de la carte graphique, tel que lspci le dit.
    carte: String,
    /// Le pilote qui la tient ("nvidia", "nouveau", "" si aucun).
    pilote: String,
    nvidia: bool,
    /// Le paquet qui connaît la carte (26 novembre) : `nvidia-driver-580`
    /// pour une Maxwell, Pascal ou Volta (GT 1030…), `-580-open` après.
    paquet: String,
    /// Une NVIDIA qu'aucun pilote signé ne connaît plus (Kepler et avant) :
    /// le pilote libre est ce qui la fait marcher, rien à installer.
    trop_ancienne: bool,
    /// Le pilote officiel manque et peut être installé ici.
    a_installer: bool,
    /// Il est installé mais ne tourne pas encore : il faut redémarrer.
    redemarrer: bool,
    /// Une clé de signature attend l'écran bleu (MokManager).
    cle_en_attente: bool,
    sur_cle: bool,
    secure_boot: String,
    materiel: Vec<crate::materiel::Ligne>,
}

/// Les cartes NVIDIA de la machine, et le pilote de chacune.
fn cartes_nvidia(sysfs: &Path) -> Vec<String> {
    crate::premier::cartes_nvidia(sysfs).into_iter().map(|(_, pilote)| pilote).collect()
}

fn paquet_installe(nom: &str) -> bool {
    sortie("dpkg-query", &["-W", "-f=${Status}", nom]).starts_with("install ok installed")
}

#[tauri::command]
pub async fn preparer_etat() -> Etat {
    let materiel = crate::materiel::resume();
    let carte = materiel.iter().find(|l| l.icone == "gpu").map(|l| l.valeur.clone()).unwrap_or_default();
    let pilotes = cartes_nvidia(Path::new("/sys"));
    let nvidia = !pilotes.is_empty();
    let tourne = pilotes.iter().any(|p| p == "nvidia");
    let paquet = crate::premier::paquet_nvidia(Path::new("/sys")).unwrap_or_default();
    // Le pilote signé de Canonical (23 novembre) DE LA BONNE VARIANTE, ou
    // l'ancien, compilé par DKMS. Le « open » posé sur une Pascal (26
    // novembre, la GT 1030) ne la prend pas : le bouton revient, et la
    // variante fermée remplace l'autre (grenos-pilote-signe).
    let pose = (!paquet.is_empty() && paquet_installe(&paquet))
        || paquet_installe("nvidia-kernel-open-dkms")
        || paquet_installe("nvidia-kernel-dkms");
    let ligne = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    let a_installer = crate::premier::pilote_nvidia_a_proposer(Path::new("/sys"), &ligne) && !pose;
    let cle_en_attente = !sortie("mokutil", &["--list-new"]).trim().is_empty();
    let etat = Etat {
        carte,
        pilote: pilotes.first().cloned().unwrap_or_default(),
        nvidia,
        trop_ancienne: nvidia && paquet.is_empty(),
        paquet,
        a_installer,
        redemarrer: nvidia && pose && !tourne,
        cle_en_attente,
        sur_cle: sur_cle(),
        secure_boot: crate::premier::secure_boot("/sys/firmware/efi/efivars").into(),
        materiel,
    };
    dire(&format!(
        "preparer : nvidia {} pilote « {} » paquet « {} » a installer {} redemarrer {} cle en attente {}",
        etat.nvidia, etat.pilote, etat.paquet, etat.a_installer, etat.redemarrer, etat.cle_en_attente
    ));
    etat
}

/// Le matériel est prêt (ou la personne a dit « plus tard ») : le raccourci
/// du bureau s'en va et ne revient pas.
#[tauri::command]
pub fn preparer_fini() -> Result<(), String> {
    let maison = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into()));
    let _ = fs::remove_file(maison.join("Bureau/grenos-preparer-materiel.desktop"));
    let reglages = maison.join(".config/grenos");
    fs::create_dir_all(&reglages).map_err(|e| e.to_string())?;
    fs::write(reglages.join("materiel-prepare"), "oui\n").map_err(|e| e.to_string())?;
    dire("preparer : raccourci du bureau retire");
    Ok(())
}

/// Redémarrer pour charger le pilote (et passer l'écran bleu de la clé).
#[tauri::command]
pub fn preparer_redemarrer() -> Result<(), String> {
    dire("preparer : redemarrage demande");
    crate::systeme::detacher("systemctl", &["reboot"])
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn trouve_les_cartes_nvidia_et_leur_pilote() {
        let r = std::env::temp_dir().join(format!("grenos-preparer-{}", std::process::id()));
        let carte = |nom: &str, vendeur: &str, classe: &str, pilote: Option<&str>| {
            let d = r.join("bus/pci/devices").join(nom);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("vendor"), vendeur).unwrap();
            fs::write(d.join("class"), classe).unwrap();
            if let Some(p) = pilote {
                let cible = r.join("pilotes").join(p);
                fs::create_dir_all(&cible).unwrap();
                let _ = std::os::unix::fs::symlink(&cible, d.join("driver"));
            }
        };
        carte("0000:00:02.0", "0x8086", "0x030000", Some("i915"));
        assert!(cartes_nvidia(&r).is_empty(), "Intel seule : aucune NVIDIA");
        carte("0000:01:00.0", "0x10de", "0x030000", None);
        carte("0000:01:00.1", "0x10de", "0x040300", Some("snd_hda_intel"));
        assert_eq!(cartes_nvidia(&r), vec![String::new()], "la carte, sans pilote ; pas sa sortie son");
        let _ = fs::remove_dir_all(&r);
    }
}
