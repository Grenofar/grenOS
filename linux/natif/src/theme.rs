//! Les couleurs et la feuille de style communes, lues dans `grenosui.py`.
//!
//! POURQUOI ON LIT UN FICHIER PYTHON
//!
//! `grenosui.py` est l'unique endroit où vivent les deux palettes, les six
//! accents et la feuille GTK de tout le système. Recopier ces valeurs ici en
//! ferait deux exemplaires, et le jour où l'un change, la barre n'aurait plus
//! la couleur des Réglages — exactement le défaut « même application, deux
//! visages » que Grenofar a déjà relevé. Le fichier est donc embarqué à la
//! compilation (`include_str!`) et découpé ici ; les tests ci-dessous cassent
//! la construction si sa forme change au point qu'on ne le lise plus.

use std::collections::BTreeMap;

const GRENOSUI: &str = include_str!("../../config/includes.chroot/usr/lib/grenos/grenosui.py");

/// Une palette : le nom de rôle (« fond », « accent »…) et sa valeur CSS.
pub type Palette = BTreeMap<String, String>;

/// Les lignes `"cle": "valeur",` d'un dictionnaire Python `NOM = { … }`.
fn dictionnaire(nom: &str) -> Palette {
    let mut sortie = Palette::new();
    let debut = format!("\n{nom} = {{");
    let Some(pos) = GRENOSUI.find(&debut) else { return sortie };
    for ligne in GRENOSUI[pos + debut.len()..].lines() {
        let ligne = ligne.trim();
        if ligne.starts_with('}') {
            break;
        }
        let morceaux: Vec<&str> = ligne.split('"').collect();
        // "fond": "#0b101c",  →  ["", fond, ": ", #0b101c, ","]
        if morceaux.len() >= 4 {
            sortie.insert(morceaux[1].to_string(), morceaux[3].to_string());
        }
    }
    sortie
}

/// `ACCENTS = { "bleu": ("#…", "#…", "#…", "#…"), … }`
fn accents() -> BTreeMap<String, [String; 4]> {
    let mut sortie = BTreeMap::new();
    let debut = "\nACCENTS = {";
    let Some(pos) = GRENOSUI.find(debut) else { return sortie };
    for ligne in GRENOSUI[pos + debut.len()..].lines() {
        let ligne = ligne.trim();
        if ligne.starts_with('}') {
            break;
        }
        let morceaux: Vec<&str> = ligne.split('"').collect();
        // "bleu": ("#a", "#b", "#c", "#d"),
        if morceaux.len() >= 10 {
            sortie.insert(
                morceaux[1].to_string(),
                [morceaux[3].into(), morceaux[5].into(), morceaux[7].into(), morceaux[9].into()],
            );
        }
    }
    sortie
}

/// La feuille de style GTK du système (`MODELE`).
pub fn modele() -> &'static str {
    let debut = "\nMODELE = \"\"\"";
    let Some(pos) = GRENOSUI.find(debut) else { return "" };
    let reste = &GRENOSUI[pos + debut.len()..];
    match reste.find("\"\"\"") {
        Some(fin) => &reste[..fin],
        None => "",
    }
}

/// Un réglage de `~/.config/grenos`, vide s'il n'existe pas.
pub fn reglage(nom: &str) -> String {
    std::fs::read_to_string(crate::reglages().join(nom)).map(|t| t.trim().to_string()).unwrap_or_default()
}

/// La palette du moment : nuit par défaut, jour si choisi, et l'accent.
pub fn palette() -> Palette {
    palette_pour(&reglage("theme"), &reglage("accent"))
}

pub fn palette_pour(theme: &str, accent: &str) -> Palette {
    let mut base = dictionnaire(if theme == "jour" { "JOUR" } else { "NUIT" });
    if let Some([a, clair, sombre, second]) = accents().get(accent) {
        base.insert("accent".into(), a.clone());
        base.insert("accent-clair".into(), clair.clone());
        base.insert("accent-sombre".into(), sombre.clone());
        base.insert("second".into(), second.clone());
    }
    base
}

/// Les `@define-color` qui précèdent toute feuille.
pub fn entete(couleurs: &Palette) -> String {
    couleurs.iter().map(|(n, v)| format!("@define-color {n} {v};\n")).collect()
}

/// Une couleur « #rrggbb » en trois flottants, pour dessiner au cairo.
pub fn rgb(hex: &str) -> Option<(f64, f64, f64)> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|x| x as f64 / 255.0);
    Some((v(0)?, v(2)?, v(4)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_palettes_se_lisent() {
        let nuit = palette_pour("nuit", "");
        assert_eq!(nuit.get("fond").map(String::as_str), Some("#07080d"));
        assert!(nuit.len() >= 15, "palette nuit incomplète : {nuit:?}");
        let jour = palette_pour("jour", "");
        assert_eq!(jour.get("fond").map(String::as_str), Some("#f3f6fc"));
        assert_eq!(jour.len(), nuit.len());
    }

    #[test]
    fn les_accents_s_appliquent() {
        assert_eq!(accents().len(), 6);
        let violet = palette_pour("nuit", "violet");
        assert_eq!(violet.get("accent").map(String::as_str), Some("#7c5cff"));
        assert_eq!(violet.get("second").map(String::as_str), Some("#22d3ee"));
    }

    #[test]
    fn la_feuille_se_lit() {
        let m = modele();
        assert!(m.contains("menuitem"), "MODELE introuvable dans grenosui.py");
        assert!(m.contains(".avatar"));
        assert!(!m.contains("\"\"\""));
    }

    #[test]
    fn rgb_lit_l_hexa() {
        assert_eq!(rgb("#ff0000"), Some((1.0, 0.0, 0.0)));
        assert_eq!(rgb("rgba(0,0,0,1)"), None);
    }
}
