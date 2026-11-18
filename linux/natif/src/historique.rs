//! L'historique du presse-papier (todo G31 — 18 novembre), sans GTK : ce
//! qu'on garde, dans quel ordre, et comment on l'affiche. Le programme
//! `grenos-presse-natif` n'y ajoute que la fenêtre et l'écoute.

/// Combien d'éléments on garde : ceux de Windows (25).
pub const TAILLE: usize = 25;
/// Au-delà, on ne garde pas : un texte de cent mille caractères dans
/// l'historique, c'est un fichier collé par erreur, pas une phrase.
pub const TEXTE_MAX: usize = 100_000;

#[derive(Default, Debug)]
pub struct Historique {
    elements: Vec<String>,
}

impl Historique {
    /// Ajoute en tête. Un texte déjà présent remonte au lieu d'être doublé ;
    /// un texte vide ou trop long est ignoré. Rend vrai si la liste change.
    pub fn ajouter(&mut self, texte: &str) -> bool {
        if texte.trim().is_empty() || texte.len() > TEXTE_MAX {
            return false;
        }
        if self.elements.first().map(|t| t == texte).unwrap_or(false) {
            return false;
        }
        self.elements.retain(|t| t != texte);
        self.elements.insert(0, texte.to_string());
        self.elements.truncate(TAILLE);
        true
    }

    pub fn elements(&self) -> &[String] {
        &self.elements
    }

    pub fn vider(&mut self) {
        self.elements.clear();
    }

    pub fn retirer(&mut self, rang: usize) {
        if rang < self.elements.len() {
            self.elements.remove(rang);
        }
    }
}

/// Ce qu'une ligne de la liste montre : les espaces repliés, coupé à `max`
/// caractères (pas octets : « é » ne se coupe pas en deux).
pub fn apercu(texte: &str, max: usize) -> String {
    let replie: String = texte.split_whitespace().collect::<Vec<_>>().join(" ");
    if replie.chars().count() <= max {
        replie
    } else {
        let mut s: String = replie.chars().take(max.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn en_tete_sans_doublon() {
        let mut h = Historique::default();
        assert!(h.ajouter("a"));
        assert!(h.ajouter("b"));
        assert!(!h.ajouter("b"));
        assert!(h.ajouter("a"));
        assert_eq!(h.elements(), ["a", "b"]);
    }

    #[test]
    fn ignore_le_vide_et_le_trop_long() {
        let mut h = Historique::default();
        assert!(!h.ajouter("   \n"));
        assert!(!h.ajouter(&"x".repeat(TEXTE_MAX + 1)));
        assert!(h.elements().is_empty());
    }

    #[test]
    fn vingt_cinq_au_plus() {
        let mut h = Historique::default();
        for i in 0..40 {
            h.ajouter(&i.to_string());
        }
        assert_eq!(h.elements().len(), TAILLE);
        assert_eq!(h.elements()[0], "39");
        h.retirer(0);
        assert_eq!(h.elements()[0], "38");
        h.vider();
        assert!(h.elements().is_empty());
    }

    #[test]
    fn apercus() {
        assert_eq!(apercu("  une\n  ligne  ", 40), "une ligne");
        assert_eq!(apercu("éééééé", 4), "ééé…");
    }
}
