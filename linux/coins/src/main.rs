//! grenos-coins : les fenêtres de grenOS ont des coins arrondis.
//!
//! POURQUOI UN DÉMON
//!
//! X11 tourne ici sans composition (c'est ce qui garde la souris fluide sans
//! carte 3D). Sans composition, pas de transparence : la seule façon d'avoir
//! un vrai coin arrondi est de RETIRER les pixels du coin à la fenêtre, par un
//! masque de forme (extension SHAPE). `grenosui.region_arrondie` le fait déjà
//! pour nos fenêtres Python ; mais la barre de titre appartient à openbox, et
//! openbox ne sait pas arrondir. Ce petit programme pose donc le même masque
//! sur chaque CADRE d'openbox (la fenêtre qui porte la barre et la fenêtre).
//!
//! LE PIÈGE D'OPENBOX (lu dans frame.c, 17 novembre)
//!
//! À chaque redimension, openbox remet le masque du cadre à « aucun » si la
//! fenêtre cliente n'a pas de forme à elle. On écoute donc ShapeNotify : quand
//! un cadre qu'on suit redevient rectangulaire, on le reprend. Notre propre
//! masque produit aussi un ShapeNotify, mais « avec forme » : on l'ignore, et
//! il n'y a pas de boucle.
//!
//! CE QU'ON NE TOUCHE PAS
//!
//! - les fenêtres agrandies ou en plein écran : un coin arrondi contre le bord
//!   de l'écran laisserait voir le bureau dans l'angle d'un jeu ;
//! - la barre, le bureau, les bulles (types DOCK, DESKTOP, NOTIFICATION…) ;
//! - les fenêtres qui ont déjà leur propre forme (menus, panneau du son…).

use std::collections::HashMap;

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ClipOrdering, ConnectionExt as _, EventMask, Rectangle,
    Window,
};
use x11rb::protocol::Event;

/// Le rayon des coins. Un peu moins que les 14 px des panneaux de grenosui :
/// une barre de titre de 30 px avec des coins de 14 px ressemble à une pilule.
const RAYON: u16 = 10;

/// Les rectangles qui dessinent un rectangle aux coins arrondis.
///
/// Même calcul que `grenosui.region_arrondie`, ligne par ligne ; les lignes
/// identiques du milieu sont fusionnées en un seul rectangle, sinon une
/// fenêtre de 1000 px de haut enverrait 1000 rectangles au serveur à chaque
/// redimension.
pub fn rectangles_arrondis(largeur: u16, hauteur: u16, rayon: u16) -> Vec<Rectangle> {
    if largeur == 0 || hauteur == 0 {
        return Vec::new();
    }
    // Trop petit pour être arrondi proprement : on garde le rectangle.
    if largeur < 2 * rayon + 2 || hauteur < 2 * rayon + 2 || rayon == 0 {
        return vec![Rectangle { x: 0, y: 0, width: largeur, height: hauteur }];
    }
    let creux = |ecart: u16| -> u16 {
        if ecart == 0 {
            return 0;
        }
        let r = rayon as f64;
        let e = ecart as f64;
        rayon - (r * r - e * e).sqrt() as u16
    };
    let mut rects: Vec<Rectangle> = Vec::new();
    let mut pousser = |y: u16, retrait: u16| {
        let largeur_ligne = largeur.saturating_sub(2 * retrait).max(1);
        if let Some(dernier) = rects.last_mut() {
            if dernier.x == retrait as i16 && dernier.y as u16 + dernier.height == y {
                dernier.height += 1;
                return;
            }
        }
        rects.push(Rectangle { x: retrait as i16, y: y as i16, width: largeur_ligne, height: 1 });
    };
    for y in 0..hauteur {
        let ecart = if y < rayon {
            rayon - 1 - y
        } else if y >= hauteur - rayon {
            y - (hauteur - rayon)
        } else {
            0
        };
        pousser(y, creux(ecart));
    }
    rects
}

struct Atomes {
    liste: u32,
    etat: u32,
    plein_ecran: u32,
    max_v: u32,
    max_h: u32,
    type_fenetre: u32,
    exclus: Vec<u32>,
}

#[derive(Default)]
struct Suivi {
    client: Window,
    taille: (u16, u16),
    arrondi: bool,
}

fn atome<C: Connection>(conn: &C, nom: &str) -> u32 {
    conn.intern_atom(false, nom.as_bytes())
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.atom)
        .unwrap_or(0)
}

fn propriete32<C: Connection>(conn: &C, fenetre: Window, nom: u32, genre: AtomEnum) -> Vec<u32> {
    conn.get_property(false, fenetre, nom, genre, 0, 1024)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| r.value32().map(|v| v.collect()))
        .unwrap_or_default()
}

/// Le cadre d'openbox d'une fenêtre cliente : l'ancêtre qui est enfant direct
/// de la racine.
fn cadre_de<C: Connection>(conn: &C, client: Window, racine: Window) -> Option<Window> {
    let mut courant = client;
    for _ in 0..8 {
        let arbre = conn.query_tree(courant).ok()?.reply().ok()?;
        if arbre.parent == racine || arbre.parent == 0 {
            return Some(courant);
        }
        courant = arbre.parent;
    }
    None
}

struct Demon<C: Connection> {
    conn: C,
    racine: Window,
    a: Atomes,
    suivis: HashMap<Window, Suivi>,
}

impl<C: Connection> Demon<C> {
    /// Faut-il arrondir ce client ?
    fn admis(&self, client: Window) -> bool {
        let types = propriete32(&self.conn, client, self.a.type_fenetre, AtomEnum::ATOM);
        if types.iter().any(|t| self.a.exclus.contains(t)) {
            return false;
        }
        let etat = propriete32(&self.conn, client, self.a.etat, AtomEnum::ATOM);
        if etat.contains(&self.a.plein_ecran)
            || (etat.contains(&self.a.max_v) && etat.contains(&self.a.max_h))
        {
            return false;
        }
        // Une fenêtre qui a déjà sa forme (nos panneaux arrondis) : openbox
        // recopie la sienne sur le cadre, on ne s'en mêle pas.
        let deja_formee = self
            .conn
            .shape_query_extents(client)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.bounding_shaped)
            .unwrap_or(false);
        !deja_formee
    }

    fn appliquer(&mut self, cadre: Window) {
        let Some(client) = self.suivis.get(&cadre).map(|s| s.client) else { return };
        let Some(geo) = self.conn.get_geometry(cadre).ok().and_then(|c| c.reply().ok()) else {
            return;
        };
        let admis = self.admis(client);
        let suivi = self.suivis.get_mut(&cadre).unwrap();
        if admis {
            let rects = rectangles_arrondis(geo.width, geo.height, RAYON);
            let _ = self.conn.shape_rectangles(
                shape::SO::SET,
                shape::SK::BOUNDING,
                ClipOrdering::UNSORTED,
                cadre,
                0,
                0,
                &rects,
            );
            suivi.arrondi = true;
            suivi.taille = (geo.width, geo.height);
        } else if suivi.arrondi {
            // Agrandie ou plein écran : on rend les coins, une seule fois (sinon
            // chaque remise à zéro produirait un ShapeNotify, donc une autre).
            let _ = self.conn.shape_mask(shape::SO::SET, shape::SK::BOUNDING, cadre, 0, 0, x11rb::NONE);
            suivi.arrondi = false;
            suivi.taille = (0, 0);
        }
        let _ = self.conn.flush();
    }

    /// Relit la liste des fenêtres d'openbox : suit les nouvelles, oublie les
    /// disparues.
    fn resynchroniser(&mut self) {
        let clients = propriete32(&self.conn, self.racine, self.a.liste, AtomEnum::WINDOW);
        let mut vus = Vec::new();
        for client in clients {
            let Some(cadre) = cadre_de(&self.conn, client, self.racine) else { continue };
            vus.push(cadre);
            if self.suivis.contains_key(&cadre) {
                continue;
            }
            let _ = self.conn.change_window_attributes(
                client,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
            );
            let _ = self.conn.change_window_attributes(
                cadre,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY),
            );
            let _ = self.conn.shape_select_input(cadre, true);
            self.suivis.insert(cadre, Suivi { client, ..Default::default() });
            self.appliquer(cadre);
        }
        self.suivis.retain(|cadre, _| vus.contains(cadre));
    }

    fn cadre_du_client(&self, client: Window) -> Option<Window> {
        self.suivis.iter().find(|(_, s)| s.client == client).map(|(c, _)| *c)
    }

    fn tourner(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.resynchroniser();
        loop {
            match self.conn.wait_for_event()? {
                Event::PropertyNotify(e) if e.window == self.racine && e.atom == self.a.liste => {
                    self.resynchroniser();
                }
                Event::PropertyNotify(e) if e.atom == self.a.etat => {
                    if let Some(cadre) = self.cadre_du_client(e.window) {
                        self.appliquer(cadre);
                    }
                }
                Event::ConfigureNotify(e) => {
                    let change = self
                        .suivis
                        .get(&e.window)
                        .map(|s| s.arrondi && s.taille != (e.width, e.height))
                        .unwrap_or(false);
                    if change {
                        self.appliquer(e.window);
                    }
                }
                Event::ShapeNotify(e)
                    if e.shape_kind == shape::SK::BOUNDING && !e.shaped =>
                {
                    if self.suivis.contains_key(&e.affected_window) {
                        self.appliquer(e.affected_window);
                    }
                }
                Event::DestroyNotify(e) => {
                    self.suivis.remove(&e.window);
                }
                // Une fenêtre fermée entre deux requêtes : BadWindow, sans gravité.
                _ => {}
            }
        }
    }
}

fn main() {
    let (conn, ecran) = match x11rb::connect(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("grenos-coins : pas d'affichage X ({e})");
            std::process::exit(1);
        }
    };
    if conn.extension_information(shape::X11_EXTENSION_NAME).ok().flatten().is_none() {
        eprintln!("grenos-coins : le serveur X n'a pas l'extension SHAPE, coins carrés");
        return;
    }
    let racine = conn.setup().roots[ecran].root;
    let _ = conn.change_window_attributes(
        racine,
        &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    );
    let exclus = [
        "_NET_WM_WINDOW_TYPE_DOCK",
        "_NET_WM_WINDOW_TYPE_DESKTOP",
        "_NET_WM_WINDOW_TYPE_NOTIFICATION",
        "_NET_WM_WINDOW_TYPE_SPLASH",
        "_NET_WM_WINDOW_TYPE_TOOLTIP",
        "_NET_WM_WINDOW_TYPE_DND",
    ]
    .iter()
    .map(|n| atome(&conn, n))
    .collect();
    let a = Atomes {
        liste: atome(&conn, "_NET_CLIENT_LIST"),
        etat: atome(&conn, "_NET_WM_STATE"),
        plein_ecran: atome(&conn, "_NET_WM_STATE_FULLSCREEN"),
        max_v: atome(&conn, "_NET_WM_STATE_MAXIMIZED_VERT"),
        max_h: atome(&conn, "_NET_WM_STATE_MAXIMIZED_HORZ"),
        type_fenetre: atome(&conn, "_NET_WM_WINDOW_TYPE"),
        exclus,
    };
    let mut demon = Demon { conn, racine, a, suivis: HashMap::new() };
    if let Err(e) = demon.tourner() {
        eprintln!("grenos-coins : connexion X perdue ({e})");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod essais {
    use super::*;

    fn contient(rects: &[Rectangle], x: i16, y: i16) -> bool {
        rects.iter().any(|r| {
            x >= r.x && x < r.x + r.width as i16 && y >= r.y && y < r.y + r.height as i16
        })
    }

    #[test]
    fn les_coins_sont_retires_et_le_centre_garde() {
        let r = rectangles_arrondis(400, 300, 10);
        for (x, y) in [(0, 0), (399, 0), (0, 299), (399, 299), (1, 1), (398, 298)] {
            assert!(!contient(&r, x, y), "coin ({x},{y}) encore là");
        }
        for (x, y) in [(200, 150), (0, 150), (399, 150), (200, 0), (200, 299), (10, 10)] {
            assert!(contient(&r, x, y), "({x},{y}) retiré à tort");
        }
    }

    #[test]
    fn le_milieu_tient_en_un_seul_rectangle() {
        // 10 lignes en haut, 10 en bas au plus, et un seul bloc entre les deux.
        let r = rectangles_arrondis(800, 1000, 10);
        assert!(r.len() <= 21, "{} rectangles", r.len());
        assert!(r.iter().any(|x| x.height >= 980));
    }

    #[test]
    fn meme_dessin_que_grenosui() {
        // Le retrait de chaque ligne, recalculé comme `region_arrondie` (Python).
        let (l, h, rayon) = (120u16, 60u16, 10u16);
        let r = rectangles_arrondis(l, h, rayon);
        for y in 0..h {
            let ecart: i32 = if y < rayon {
                (rayon - 1 - y) as i32
            } else if y >= h - rayon {
                (y - (h - rayon)) as i32
            } else {
                0
            };
            let creux = if ecart != 0 {
                rayon as i32 - ((rayon as i32).pow(2) as f64 - (ecart * ecart) as f64).sqrt() as i32
            } else {
                0
            };
            assert!(!contient(&r, creux as i16 - 1, y as i16) || creux == 0);
            assert!(contient(&r, creux as i16, y as i16), "ligne {y}");
            assert!(contient(&r, (l as i32 - 1 - creux) as i16, y as i16), "ligne {y} droite");
        }
    }

    #[test]
    fn trop_petit_reste_rectangle() {
        assert_eq!(rectangles_arrondis(15, 15, 10).len(), 1);
        assert!(rectangles_arrondis(0, 10, 10).is_empty());
    }
}
