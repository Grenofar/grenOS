//! Les fenêtres ouvertes, lues par EWMH — ce que libwnck faisait pour la
//! barre Python.
//!
//! POURQUOI PAS LIBWNCK
//!
//! gtk-rs ne lie plus libwnck, et une liaison maison coûterait plus que ce
//! qu'on en utilise : la liste des fenêtres, leur classe, celle qui a le
//! focus, si elle est réduite, son icône, et trois gestes (afficher, réduire,
//! fermer). Tout cela est une poignée de propriétés de la fenêtre racine que
//! openbox tient à jour (`_NET_CLIENT_LIST`, `_NET_ACTIVE_WINDOW`…), et trois
//! messages à lui envoyer. x11rb les parle en pur Rust, comme grenos-coins.
//!
//! On reproduit ce que wnck appelait « skip tasklist » : une fenêtre de type
//! bureau, barre ou écran d'accueil, ou marquée SKIP_TASKBAR, n'a pas de tuile.

use std::collections::HashMap;
use std::os::unix::io::AsRawFd;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt as _, EventMask, ImageFormat,
    PropMode, Window,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

/// Ce qu'on sait d'une fenêtre ouverte.
#[derive(Clone, Debug, PartialEq)]
pub struct Fenetre {
    pub xid: u32,
    /// WM_CLASS, deuxième moitié (« Firefox-esr ») : wnck l'appelait le
    /// nom du groupe de classe.
    pub classe: String,
    /// Le titre (wnck : `get_name`).
    pub titre: String,
    pub reduite: bool,
}

pub struct Ecran {
    conn: RustConnection,
    racine: Window,
    atomes: HashMap<&'static str, u32>,
}

const NOMS: &[&str] = &[
    "_NET_CLIENT_LIST", "_NET_ACTIVE_WINDOW", "_NET_CLIENT_LIST_STACKING", "_NET_WM_STATE",
    "_NET_WM_STATE_HIDDEN", "_NET_WM_STATE_SKIP_TASKBAR", "_NET_WM_WINDOW_TYPE",
    "_NET_WM_WINDOW_TYPE_DESKTOP", "_NET_WM_WINDOW_TYPE_DOCK", "_NET_WM_WINDOW_TYPE_SPLASH",
    "_NET_WM_NAME", "UTF8_STRING", "_NET_WM_ICON", "_NET_CLOSE_WINDOW", "WM_CHANGE_STATE",
    "_NET_WM_STRUT", "_NET_WM_STRUT_PARTIAL",
    "_NET_CURRENT_DESKTOP", "_NET_WM_DESKTOP",
    "_NET_FRAME_EXTENTS", "_NET_MOVERESIZE_WINDOW", "_NET_WM_STATE_MAXIMIZED_VERT",
    "_NET_WM_STATE_MAXIMIZED_HORZ",
];

/// Une fenêtre a-t-elle sa tuile sur ce bureau ? Celles « de tous les
/// bureaux » (0xFFFFFFFF) partout, celles sans bureau déclaré aussi : mieux
/// vaut une tuile de trop qu'une fenêtre qu'on ne retrouve plus.
pub fn sur_ce_bureau(sien: Option<u32>, courant: Option<u32>) -> bool {
    match (sien, courant) {
        (Some(0xFFFF_FFFF), _) | (None, _) | (_, None) => true,
        (Some(s), Some(c)) => s == c,
    }
}

impl Ecran {
    pub fn ouvrir() -> Option<Ecran> {
        let (conn, numero) = x11rb::connect(None).ok()?;
        let racine = conn.setup().roots.get(numero)?.root;
        let mut atomes = HashMap::new();
        let cookies: Vec<_> = NOMS.iter().map(|n| conn.intern_atom(false, n.as_bytes())).collect();
        for (nom, cookie) in NOMS.iter().zip(cookies) {
            atomes.insert(*nom, cookie.ok()?.reply().ok()?.atom);
        }
        // On écoute les propriétés de la racine : liste, focus, empilement.
        conn.change_window_attributes(racine, &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE))
            .ok()?;
        conn.flush().ok()?;
        Some(Ecran { conn, racine, atomes })
    }

    fn a(&self, nom: &str) -> u32 {
        self.atomes.get(nom).copied().unwrap_or(0)
    }

    pub fn fd(&self) -> i32 {
        self.conn.stream().as_raw_fd()
    }

    fn propriete32(&self, fenetre: Window, nom: u32, genre: u32, longueur: u32) -> Vec<u32> {
        self.conn
            .get_property(false, fenetre, nom, genre, 0, longueur)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|v| v.collect()))
            .unwrap_or_default()
    }

    fn texte(&self, fenetre: Window, nom: u32, genre: u32) -> Option<String> {
        let r = self.conn.get_property(false, fenetre, nom, genre, 0, 1024).ok()?.reply().ok()?;
        if r.value.is_empty() {
            return None;
        }
        Some(String::from_utf8_lossy(&r.value).into_owned())
    }

    pub fn active(&self) -> Option<u32> {
        self.propriete32(self.racine, self.a("_NET_ACTIVE_WINDOW"), AtomEnum::WINDOW.into(), 1)
            .first()
            .copied()
            .filter(|&x| x != 0)
    }

    /// Les fenêtres qui ont droit à une tuile, dans l'ordre d'ouverture.
    pub fn fenetres(&self) -> Vec<Fenetre> {
        let liste = self.propriete32(self.racine, self.a("_NET_CLIENT_LIST"), AtomEnum::WINDOW.into(), 4096);
        let types_caches = [
            self.a("_NET_WM_WINDOW_TYPE_DESKTOP"),
            self.a("_NET_WM_WINDOW_TYPE_DOCK"),
            self.a("_NET_WM_WINDOW_TYPE_SPLASH"),
        ];
        // G11 — les bureaux virtuels (18 novembre) : comme Windows, la barre ne
        // montre que les fenêtres du bureau où l'on est.
        let courant = self.propriete32(self.racine, self.a("_NET_CURRENT_DESKTOP"), AtomEnum::CARDINAL.into(), 1).first().copied();
        let mut sortie = Vec::new();
        for xid in liste {
            // On suit aussi chaque fenêtre : réduite ou rendue, sa tuile change.
            let _ = self.conn.change_window_attributes(
                xid,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
            );
            let sien = self.propriete32(xid, self.a("_NET_WM_DESKTOP"), AtomEnum::CARDINAL.into(), 1).first().copied();
            if !sur_ce_bureau(sien, courant) {
                continue;
            }
            let etats = self.propriete32(xid, self.a("_NET_WM_STATE"), AtomEnum::ATOM.into(), 64);
            if etats.contains(&self.a("_NET_WM_STATE_SKIP_TASKBAR")) {
                continue;
            }
            let types = self.propriete32(xid, self.a("_NET_WM_WINDOW_TYPE"), AtomEnum::ATOM.into(), 16);
            if types.iter().any(|t| types_caches.contains(t)) {
                continue;
            }
            let classe = self
                .texte(xid, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())
                .map(|t| t.split('\0').nth(1).unwrap_or("").to_string())
                .unwrap_or_default();
            let titre = self
                .texte(xid, self.a("_NET_WM_NAME"), self.a("UTF8_STRING"))
                .or_else(|| self.texte(xid, AtomEnum::WM_NAME.into(), AtomEnum::STRING.into()))
                .unwrap_or_default();
            sortie.push(Fenetre { xid, classe, titre, reduite: etats.contains(&self.a("_NET_WM_STATE_HIDDEN")) });
        }
        let _ = self.conn.flush();
        sortie
    }

    /// L'icône que la fenêtre déclare (_NET_WM_ICON), à la taille voulue.
    /// Rendue en octets RGBA, largeur, hauteur.
    pub fn icone(&self, xid: u32, taille: u32) -> Option<(Vec<u8>, u32, u32)> {
        let donnees = self.propriete32(xid, self.a("_NET_WM_ICON"), AtomEnum::CARDINAL.into(), 1 << 20);
        choisir_icone(&donnees, taille)
    }

    /// L'APERÇU D'UNE FENÊTRE (19 novembre) : « quand on reste sur l'app
    /// ouverte dans le dock, il faut qu'elle affiche la fenêtre avec un
    /// aperçu ». Sans compositeur, on lit ce que l'écran montre de la fenêtre
    /// (GetImage) : une fenêtre réduite n'a rien à montrer — `None`, et la
    /// bulle met son icône — et une partie couverte par une autre montre
    /// celle du dessus. C'est la limite honnête d'un bureau sans compositeur.
    /// Rendu en RGBA, réduit à `largeur` pixels de large au plus.
    pub fn apercu(&self, xid: u32, largeur: u32) -> Option<(Vec<u8>, u32, u32)> {
        let geo = self.conn.get_geometry(xid).ok()?.reply().ok()?;
        let (l, h) = (geo.width as u32, geo.height as u32);
        if l == 0 || h == 0 {
            return None;
        }
        let image = self.conn.get_image(ImageFormat::Z_PIXMAP, xid, 0, 0, geo.width, geo.height, !0).ok()?.reply().ok()?;
        // 24 ou 32 bits de profondeur : 4 octets par pixel, B G R X.
        if image.depth < 24 || image.data.len() < (l * h * 4) as usize {
            return None;
        }
        Some(reduire_bgrx(&image.data, l, h, largeur))
    }

    fn message(&self, fenetre: Window, genre: u32, donnees: [u32; 5]) {
        let ev = ClientMessageEvent::new(32, fenetre, genre, donnees);
        let _ = self.conn.send_event(
            false,
            self.racine,
            EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT,
            ev,
        );
        let _ = self.conn.flush();
    }

    /// Afficher et donner le focus (openbox la rend aussi si elle est réduite).
    /// Source 2 : « un pagineur », la demande qu'openbox honore toujours.
    pub fn activer(&self, xid: u32, temps: u32) {
        self.message(xid, self.a("_NET_ACTIVE_WINDOW"), [2, temps, 0, 0, 0]);
    }

    /// Réduire : WM_CHANGE_STATE IconicState (ICCCM 4.1.4).
    pub fn reduire(&self, xid: u32) {
        self.message(xid, self.a("WM_CHANGE_STATE"), [3, 0, 0, 0, 0]);
    }

    pub fn fermer(&self, xid: u32, temps: u32) {
        self.message(xid, self.a("_NET_CLOSE_WINDOW"), [temps, 2, 0, 0, 0]);
    }

    /// Réserve la bande du bas : une fenêtre agrandie s'arrête au-dessus.
    /// `debut`..`fin` : les colonnes de la racine que la bande couvre — celles
    /// de l'écran principal seulement ; les autres écrans gardent tout leur bas.
    pub fn reserver(&self, xid: u32, hauteur: u32, debut: u32, fin: u32) {
        let _ = self.conn.change_property32(
            PropMode::REPLACE, xid, self.a("_NET_WM_STRUT"), AtomEnum::CARDINAL, &[0, 0, 0, hauteur],
        );
        let _ = self.conn.change_property32(
            PropMode::REPLACE,
            xid,
            self.a("_NET_WM_STRUT_PARTIAL"),
            AtomEnum::CARDINAL,
            &[0, 0, 0, hauteur, 0, 0, 0, 0, 0, 0, debut, fin],
        );
        let _ = self.conn.flush();
    }

    /// Où est la fenêtre cliente dans la racine : (x, y) de son coin.
    /// Sert aux dispositions (20 novembre) : voir qu'une fenêtre BOUGE pendant
    /// que le bouton est tenu, c'est savoir qu'on la fait glisser.
    pub fn position(&self, xid: u32) -> Option<(i32, i32)> {
        let t = self.conn.translate_coordinates(xid, self.racine, 0, 0).ok()?.reply().ok()?;
        Some((t.dst_x as i32, t.dst_y as i32))
    }

    /// La taille de la fenêtre cliente : (largeur, hauteur). Une fenêtre qui
    /// change de place SANS changer de taille est déplacée, pas redimensionnée
    /// par son bord gauche ou haut.
    pub fn taille(&self, xid: u32) -> Option<(u32, u32)> {
        let g = self.conn.get_geometry(xid).ok()?.reply().ok()?;
        Some((g.width as u32, g.height as u32))
    }

    /// Les épaisseurs du cadre d'openbox : (gauche, droite, haut, bas).
    pub fn cadre(&self, xid: u32) -> (u32, u32, u32, u32) {
        let v = self.propriete32(xid, self.a("_NET_FRAME_EXTENTS"), AtomEnum::CARDINAL.into(), 4);
        if v.len() == 4 { (v[0], v[1], v[2], v[3]) } else { (0, 0, 0, 0) }
    }

    /// Ranger la fenêtre dans le rectangle (x, y, l, h) de la racine, CADRE
    /// COMPRIS : on la désagrandit, puis _NET_MOVERESIZE_WINDOW (EWMH), que
    /// openbox honore. Gravité nord-ouest : (x, y) est le coin du cadre ; la
    /// taille demandée est celle du client, donc le rectangle moins le cadre.
    pub fn ranger(&self, xid: u32, x: i32, y: i32, l: i32, h: i32) {
        let (cg, cd, ch, cb) = self.cadre(xid);
        self.message(
            xid,
            self.a("_NET_WM_STATE"),
            [0, self.a("_NET_WM_STATE_MAXIMIZED_VERT"), self.a("_NET_WM_STATE_MAXIMIZED_HORZ"), 2, 0],
        );
        let largeur = (l - (cg + cd) as i32).max(80) as u32;
        let hauteur = (h - (ch + cb) as i32).max(60) as u32;
        // 1 = nord-ouest ; 0xF00 = x, y, largeur et hauteur fournis ; 2 << 12 =
        // demande d'un « pagineur », qu'openbox honore toujours.
        let drapeaux = 1 | 0xF00 | (2 << 12);
        self.message(xid, self.a("_NET_MOVERESIZE_WINDOW"), [drapeaux, x as u32, y as u32, largeur, hauteur]);
    }

    /// Vide la file d'événements ; vrai si l'un d'eux change la barre.
    pub fn evenements(&self) -> bool {
        let interessants = [
            self.a("_NET_CLIENT_LIST"),
            self.a("_NET_ACTIVE_WINDOW"),
            self.a("_NET_CLIENT_LIST_STACKING"),
            self.a("_NET_WM_STATE"),
            self.a("_NET_CURRENT_DESKTOP"),
            self.a("_NET_WM_DESKTOP"),
            u32::from(AtomEnum::WM_CLASS),
        ];
        let mut change = false;
        while let Ok(Some(ev)) = self.conn.poll_for_event() {
            if let Event::PropertyNotify(p) = ev {
                if interessants.contains(&p.atom) {
                    change = true;
                }
            }
        }
        change
    }
}

/// Réduire une image B G R X à `cible` pixels de large (proportions
/// gardées), en moyennant chaque bloc — au plus 4×4 points par bloc, pour
/// qu'une fenêtre 4K se réduise en quelques millisecondes. Rendue en RGBA.
pub fn reduire_bgrx(src: &[u8], l: u32, h: u32, cible: u32) -> (Vec<u8>, u32, u32) {
    let cl = cible.min(l).max(1);
    let ch = ((h as u64 * cl as u64) / l.max(1) as u64).max(1) as u32;
    let mut sortie = Vec::with_capacity((cl * ch * 4) as usize);
    for y in 0..ch {
        let y0 = y * h / ch;
        let y1 = ((y + 1) * h / ch).max(y0 + 1).min(h);
        let pas_y = ((y1 - y0) / 4).max(1);
        for x in 0..cl {
            let x0 = x * l / cl;
            let x1 = ((x + 1) * l / cl).max(x0 + 1).min(l);
            let pas_x = ((x1 - x0) / 4).max(1);
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            let mut yy = y0;
            while yy < y1 {
                let mut xx = x0;
                while xx < x1 {
                    let i = ((yy * l + xx) * 4) as usize;
                    b += src[i] as u32;
                    g += src[i + 1] as u32;
                    r += src[i + 2] as u32;
                    n += 1;
                    xx += pas_x;
                }
                yy += pas_y;
            }
            let n = n.max(1);
            sortie.extend_from_slice(&[(r / n) as u8, (g / n) as u8, (b / n) as u8, 255]);
        }
    }
    (sortie, cl, ch)
}

/// Dans un _NET_WM_ICON (plusieurs images : largeur, hauteur, pixels ARGB),
/// la plus petite qui couvre `taille`, sinon la plus grande. En RGBA.
pub fn choisir_icone(donnees: &[u32], taille: u32) -> Option<(Vec<u8>, u32, u32)> {
    let mut i = 0;
    let mut meilleure: Option<(usize, u32, u32)> = None;
    while i + 2 <= donnees.len() {
        let (l, h) = (donnees[i], donnees[i + 1]);
        let n = (l as usize).checked_mul(h as usize)?;
        if l == 0 || h == 0 || i + 2 + n > donnees.len() {
            break;
        }
        let mieux = match meilleure {
            None => true,
            Some((_, ml, _)) => (l >= taille && (ml < taille || l < ml)) || (ml < taille && l > ml),
        };
        if mieux {
            meilleure = Some((i + 2, l, h));
        }
        i += 2 + n;
    }
    let (debut, l, h) = meilleure?;
    let mut rgba = Vec::with_capacity((l * h * 4) as usize);
    for &argb in &donnees[debut..debut + (l * h) as usize] {
        rgba.extend_from_slice(&[(argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8]);
    }
    Some((rgba, l, h))
}

#[cfg(test)]
mod tests {
    #[test]
    fn l_apercu_se_reduit_en_gardant_les_proportions_et_les_couleurs() {
        // 400×200 entièrement rouge (B G R X = 0 0 255 0) → 100×50, rouge.
        let src: Vec<u8> = std::iter::repeat([0u8, 0, 255, 0]).take(400 * 200).flatten().collect();
        let (rgba, l, h) = super::reduire_bgrx(&src, 400, 200, 100);
        assert_eq!((l, h), (100, 50));
        assert_eq!(rgba.len(), 100 * 50 * 4);
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
        // Plus petite que la cible : rendue telle quelle, jamais agrandie.
        let (_, l, h) = super::reduire_bgrx(&src[..40 * 20 * 4], 40, 20, 100);
        assert_eq!((l, h), (40, 20));
    }


    #[test]
    fn une_tuile_par_bureau() {
        assert!(sur_ce_bureau(Some(1), Some(1)));
        assert!(!sur_ce_bureau(Some(0), Some(1)));
        assert!(sur_ce_bureau(Some(0xFFFF_FFFF), Some(1)));
        assert!(sur_ce_bureau(None, Some(1)));
        assert!(sur_ce_bureau(Some(2), None));
    }

    use super::*;

    #[test]
    fn on_choisit_la_bonne_icone() {
        // Une 16×16 puis une 32×32 : pour 26 px, la 32.
        let mut d = vec![16, 16];
        d.extend(std::iter::repeat(0xff00_00ff).take(256));
        d.extend([32, 32]);
        d.extend(std::iter::repeat(0x80ff_0000).take(1024));
        let (rgba, l, h) = choisir_icone(&d, 26).unwrap();
        assert_eq!((l, h), (32, 32));
        assert_eq!(&rgba[..4], &[0xff, 0, 0, 0x80]);
        // Pour 12 px, la 16.
        assert_eq!(choisir_icone(&d, 12).unwrap().1, 16);
        // Des données tronquées ne font pas tomber la barre.
        assert!(choisir_icone(&[64, 64, 1, 2], 26).is_none());
    }
}
