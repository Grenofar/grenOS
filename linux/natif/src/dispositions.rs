//! Les DISPOSITIONS, comme Windows 11 (20 novembre).
//!
//! Grenofar, capture de son Windows à l'appui : « quand on glisse une fenêtre,
//! en haut il y a un petit rectangle à bords arrondis, quasiment tout l'écran
//! de large, et il propose des dispositions dans l'écran, avec une ou
//! plusieurs pages ; désactivable dans Paramètres ».
//!
//! Ce module ne dessine rien : il dit OÙ sont les cases (dans la barre et sur
//! l'écran), et laquelle est sous le pointeur. La barre (`barre.rs`) fait le
//! reste. Tout est en fonctions pures, essayées plus bas.

/// Une case : (x, y, largeur, hauteur), en fractions de l'écran.
pub type Case = (f64, f64, f64, f64);

/// Les six dispositions de Windows 11 sur un écran large, dans son ordre.
pub fn dispositions() -> Vec<Vec<Case>> {
    let t = 1.0 / 3.0;
    vec![
        // Deux moitiés.
        vec![(0.0, 0.0, 0.5, 1.0), (0.5, 0.0, 0.5, 1.0)],
        // Deux tiers, un tiers.
        vec![(0.0, 0.0, 2.0 * t, 1.0), (2.0 * t, 0.0, t, 1.0)],
        // Trois colonnes.
        vec![(0.0, 0.0, t, 1.0), (t, 0.0, t, 1.0), (2.0 * t, 0.0, t, 1.0)],
        // Une moitié, et deux quarts empilés.
        vec![(0.0, 0.0, 0.5, 1.0), (0.5, 0.0, 0.5, 0.5), (0.5, 0.5, 0.5, 0.5)],
        // Quatre quarts.
        vec![(0.0, 0.0, 0.5, 0.5), (0.5, 0.0, 0.5, 0.5), (0.0, 0.5, 0.5, 0.5), (0.5, 0.5, 0.5, 0.5)],
        // Un quart, une moitié au centre, un quart.
        vec![(0.0, 0.0, 0.25, 1.0), (0.25, 0.0, 0.5, 1.0), (0.75, 0.0, 0.25, 1.0)],
    ]
}

/// Une case en pixels, dans la zone de travail (x, y, l, h) de l'écran, avec
/// `air` pixels entre deux fenêtres et contre les bords — le même vide
/// partout, comme les marges du dock.
pub fn en_pixels(c: Case, travail: (i32, i32, i32, i32), air: i32) -> (i32, i32, i32, i32) {
    let (tx, ty, tl, th) = travail;
    let bord = |f: f64, total: i32| (f * total as f64).round() as i32;
    let (x0, x1) = (bord(c.0, tl), bord(c.0 + c.2, tl));
    let (y0, y1) = (bord(c.1, th), bord(c.1 + c.3, th));
    let demi = air / 2;
    // Contre le bord de l'écran : tout l'air ; entre deux cases : la moitié de
    // chaque côté, donc `air` entre elles.
    let g = if x0 == 0 { air } else { demi };
    let d = if x1 >= tl { air } else { demi };
    let h = if y0 == 0 { air } else { demi };
    let b = if y1 >= th { air } else { demi };
    (tx + x0 + g, ty + y0 + h, (x1 - x0 - g - d).max(1), (y1 - y0 - h - b).max(1))
}

/// Les rectangles de la barre déployée : pour chaque disposition, la
/// miniature d'écran, et dans chacune ses cases (coordonnées de la barre).
pub struct Plan {
    pub miniatures: Vec<(f64, f64, f64, f64)>,
    pub cases: Vec<Vec<(f64, f64, f64, f64)>>,
}

/// L'espace autour et entre les miniatures, et entre les cases.
pub const MARGE_BARRE: f64 = 12.0;
pub const ENTRE_MINIATURES: f64 = 14.0;
pub const ENTRE_CASES: f64 = 3.0;

pub fn plan(largeur: f64, hauteur: f64) -> Plan {
    let liste = dispositions();
    let n = liste.len() as f64;
    let l_mini = ((largeur - 2.0 * MARGE_BARRE - (n - 1.0) * ENTRE_MINIATURES) / n).max(10.0);
    let h_mini = (hauteur - 2.0 * MARGE_BARRE).max(10.0);
    let mut miniatures = Vec::new();
    let mut cases = Vec::new();
    for (i, d) in liste.iter().enumerate() {
        let x = MARGE_BARRE + i as f64 * (l_mini + ENTRE_MINIATURES);
        let y = MARGE_BARRE;
        miniatures.push((x, y, l_mini, h_mini));
        cases.push(
            d.iter()
                .map(|c| {
                    let e = ENTRE_CASES / 2.0;
                    (x + c.0 * l_mini + e, y + c.1 * h_mini + e, c.2 * l_mini - 2.0 * e, c.3 * h_mini - 2.0 * e)
                })
                .collect(),
        );
    }
    Plan { miniatures, cases }
}

/// La case sous le point (px, py) de la barre : (disposition, case).
pub fn case_sous(p: &Plan, px: f64, py: f64) -> Option<(usize, usize)> {
    p.cases.iter().enumerate().find_map(|(d, cs)| {
        cs.iter()
            .position(|&(x, y, l, h)| px >= x && px < x + l && py >= y && py < y + h)
            .map(|c| (d, c))
    })
}

/// Le réglage : « non » dans ~/.config/grenos/dispositions les coupe.
pub fn coupees(texte: &str) -> bool {
    texte.trim() == "non"
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn six_dispositions_qui_couvrent_l_ecran() {
        for d in dispositions() {
            let surface: f64 = d.iter().map(|c| c.2 * c.3).sum();
            assert!((surface - 1.0).abs() < 1e-9, "{d:?} ne couvre pas l'écran");
        }
        assert_eq!(dispositions().len(), 6);
    }

    #[test]
    fn deux_moities_avec_le_meme_air_partout() {
        // Écran 1920×1080, dock réservé en bas (72) : zone de 1920×1008.
        let t = (0, 0, 1920, 1008);
        let g = en_pixels((0.0, 0.0, 0.5, 1.0), t, 8);
        let d = en_pixels((0.5, 0.0, 0.5, 1.0), t, 8);
        assert_eq!(g, (8, 8, 948, 992));
        assert_eq!(d, (964, 8, 948, 992));
        // 8 px entre les deux, 8 px contre chaque bord.
        assert_eq!(d.0 - (g.0 + g.2), 8);
        assert_eq!(1920 - (d.0 + d.2), 8);
    }

    #[test]
    fn un_second_ecran_decale() {
        let r = en_pixels((0.0, 0.0, 0.5, 0.5), (1920, 0, 2560, 1368), 8);
        assert_eq!(r.0, 1928);
        assert_eq!(r.1, 8);
    }

    #[test]
    fn la_case_sous_le_pointeur() {
        let p = plan(1600.0, 96.0);
        assert_eq!(p.miniatures.len(), 6);
        // Le milieu de la première case de la première disposition.
        let (x, y, l, h) = p.cases[0][0];
        assert_eq!(case_sous(&p, x + l / 2.0, y + h / 2.0), Some((0, 0)));
        // Le quart en bas à droite de la cinquième.
        let (x, y, l, h) = p.cases[4][3];
        assert_eq!(case_sous(&p, x + l / 2.0, y + h / 2.0), Some((4, 3)));
        // Entre deux miniatures : rien.
        let (x, _, l, _) = p.miniatures[0];
        assert_eq!(case_sous(&p, x + l + ENTRE_MINIATURES / 2.0, 40.0), None);
    }

    #[test]
    fn le_reglage() {
        assert!(coupees("non\n"));
        assert!(!coupees(""));
        assert!(!coupees("oui"));
    }
}
