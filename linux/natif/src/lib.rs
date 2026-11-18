//! Ce que la barre et le bureau natifs partagent : le style, le journal série,
//! les lanceurs, les fichiers .desktop, et les petits gestes de fenêtre que
//! `grenosui.py` donnait aux programmes Python.

pub mod applications;
pub mod fenetres;
pub mod theme;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use gtk::prelude::*;
use gtk::{gdk, gdk_pixbuf, glib};

/// `~/.config/grenos`
pub fn reglages() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into())).join(".config"));
    base.join("grenos")
}

pub fn maison() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into()))
}

/// Une phrase au port série, comme `grenos-dire` — mais sans lancer de shell.
///
/// `grenos-dire` dépose un « billet » dans /tmp/grenos-billets, que le
/// service du port série relaie (lui seul a le droit d'écrire sur ttyS0). On
/// dépose le même billet directement : les lignes que la CI attend (« barre
/// prete », « menu ouvert », « bouton grenos en … ») arrivent mot pour mot
/// comme avant, et sans un processus de plus par phrase.
pub fn dire(texte: &str) {
    use std::os::unix::fs::FileTypeExt;
    let serie = std::fs::metadata("/dev/ttyS0").map(|m| m.file_type().is_char_device()).unwrap_or(false);
    if !serie {
        eprintln!("grenos: {texte}");
        return;
    }
    let boite = Path::new("/tmp/grenos-billets");
    if std::fs::create_dir_all(boite).is_err() {
        return;
    }
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(boite, std::fs::Permissions::from_mode(0o1777));
    }
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let nom = format!("{}-{}-{}", t.as_secs(), std::process::id(), t.subsec_nanos() % 65536);
    let _ = std::fs::write(boite.join(nom), format!("{texte}\n"));
}

/// Lance une commande détachée : si elle meurt, la barre survit — et si la
/// barre meurt, elle survit aussi. `setsid -f` fait le double fork : aucun
/// processus zombie ne s'accumule sous nous, ce qu'un simple `spawn()` jamais
/// attendu aurait fait à chaque clic.
pub fn lancer(commande: &str, terminal: bool) {
    let commande = if terminal { format!("x-terminal-emulator -e {commande}") } else { commande.to_string() };
    let _ = Command::new("setsid").args(["-f", "/bin/sh", "-c", &commande]).status();
}

/// Quote un chemin pour le shell, comme `GLib.shell_quote`.
pub fn guillemets(texte: &str) -> String {
    format!("'{}'", texte.replace('\'', "'\\''"))
}

/// Le nom choisi au premier démarrage : compte, nom affiché, machine.
pub fn identite() -> std::collections::HashMap<String, String> {
    let mut v = std::collections::HashMap::new();
    v.insert("login".to_string(), std::env::var("USER").unwrap_or_else(|_| "grenos".into()));
    v.insert("nom".to_string(), String::new());
    v.insert("machine".to_string(), "grenos".to_string());
    if let Ok(texte) = std::fs::read_to_string("/etc/grenos/identite") {
        for ligne in texte.lines() {
            if let Some((c, val)) = ligne.trim().split_once('=') {
                v.insert(c.trim().to_string(), val.trim().to_string());
            }
        }
    }
    v
}

pub fn adresse() -> String {
    let v = identite();
    format!("{}@{}", v["login"], v["machine"])
}

// ---- Le style -------------------------------------------------------------

/// Deux fournisseurs : la feuille du système, et celle du programme. Gardés
/// pour être rechargés quand le thème change (SIGUSR1).
pub struct Style {
    systeme: gtk::CssProvider,
    propre: gtk::CssProvider,
    feuille: &'static str,
}

impl Style {
    pub fn poser(feuille: &'static str) -> Rc<Style> {
        let style = Rc::new(Style { systeme: gtk::CssProvider::new(), propre: gtk::CssProvider::new(), feuille });
        if let Some(ecran) = gdk::Screen::default() {
            gtk::StyleContext::add_provider_for_screen(&ecran, &style.systeme, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
            gtk::StyleContext::add_provider_for_screen(&ecran, &style.propre, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
        }
        style.repeindre();
        style
    }

    /// Relit la palette et recharge les deux feuilles. Une feuille refusée
    /// par GTK ne fait pas tomber le programme : on le DIT sur le port série,
    /// où la CI le lira, et on continue sans elle.
    pub fn repeindre(&self) {
        let couleurs = theme::palette();
        let entete = theme::entete(&couleurs);
        for (nom, fournisseur, css) in [
            ("systeme", &self.systeme, format!("{entete}{}", theme::modele())),
            ("programme", &self.propre, format!("{entete}{}", self.feuille)),
        ] {
            if let Err(souci) = fournisseur.load_from_data(css.as_bytes()) {
                dire(&format!("style {nom} refuse par GTK : {souci}"));
            }
        }
    }
}

/// Nomme le programme : la classe de ses fenêtres et son icône par défaut.
pub fn nommer(nom: &str) {
    glib::set_prgname(Some(nom));
    gdk::set_program_class(nom);
    gtk::Window::set_default_icon_name("grenos");
}

// ---- Les images ---------------------------------------------------------------

/// La première icône trouvée parmi plusieurs noms, notre marque en dernier :
/// une ligne de menu sans icône est un trou que l'œil voit tout de suite.
pub fn pixbuf_de(noms: &[&str], taille: i32) -> Option<gdk_pixbuf::Pixbuf> {
    let theme = gtk::IconTheme::default()?;
    let replis = ["application-x-executable", "grenos"];
    for nom in noms.iter().chain(replis.iter()) {
        if nom.is_empty() {
            continue;
        }
        if nom.starts_with('/') {
            if let Ok(p) = gdk_pixbuf::Pixbuf::from_file_at_scale(nom, taille, taille, true) {
                return Some(p);
            }
            continue;
        }
        if let Ok(Some(p)) = theme.load_icon(nom, taille, gtk::IconLookupFlags::FORCE_SIZE) {
            return Some(p);
        }
    }
    None
}

pub fn image_de(noms: &[&str], taille: i32) -> Option<gtk::Image> {
    pixbuf_de(noms, taille).map(|p| gtk::Image::from_pixbuf(Some(&p)))
}

/// Change l'infobulle SEULEMENT si elle a changé. Voir la longue note de
/// `poser_infobulle` dans grenos-shell : reposer la même infobulle chaque
/// seconde la ressuscitait juste après le clic, et elle restait collée.
pub fn poser_infobulle(widget: &impl IsA<gtk::Widget>, texte: &str) {
    if widget.tooltip_text().as_deref() != Some(texte) {
        widget.set_tooltip_text(Some(texte));
    }
}

pub fn classe(widget: &impl IsA<gtk::Widget>, nom: &str) {
    widget.style_context().add_class(nom);
}

// ---- Les coins et l'attrape ----------------------------------------------------

pub const RAYON_COINS: i32 = 14;

/// Un rectangle aux coins arrondis, découpé ligne par ligne — le même calcul
/// que `grenosui.region_arrondie`. Sans compositeur, un coin ne s'arrondit
/// qu'en retirant ses pixels à la fenêtre.
pub fn rectangles_arrondis(largeur: i32, hauteur: i32, rayon: i32) -> Vec<(i32, i32, i32, i32)> {
    let mut sortie: Vec<(i32, i32, i32, i32)> = Vec::new();
    for y in 0..hauteur {
        let ecart = if y < rayon {
            rayon - 1 - y
        } else if y >= hauteur - rayon {
            y - (hauteur - rayon)
        } else {
            0
        };
        let creux = if ecart > 0 { rayon - (((rayon * rayon - ecart * ecart) as f64).sqrt() as i32) } else { 0 };
        let largeur_ligne = (largeur - 2 * creux).max(1);
        // Les lignes identiques qui se suivent ne font qu'un rectangle.
        if let Some(der) = sortie.last_mut() {
            if der.0 == creux && der.2 == largeur_ligne && der.1 + der.3 == y {
                der.3 += 1;
                continue;
            }
        }
        sortie.push((creux, y, largeur_ligne, 1));
    }
    sortie
}

/// Retire les coins d'une fenêtre, et les reretire à chaque taille neuve.
pub fn arrondir(fenetre: &gtk::Window) {
    let derniere = Rc::new(Cell::new((0, 0)));
    let decouper = {
        let derniere = derniere.clone();
        move |f: &gtk::Window| {
            let Some(cadre) = f.window() else { return };
            let (l, h) = f.size();
            if derniere.get() == (l, h) {
                return;
            }
            derniere.set((l, h));
            let region = gtk::cairo::Region::create();
            for (x, y, w, hh) in rectangles_arrondis(l, h, RAYON_COINS) {
                let _ = region.union_rectangle(&gtk::cairo::RectangleInt::new(x, y, w, hh));
            }
            cadre.shape_combine_region(Some(&region), 0, 0);
        }
    };
    let d1 = decouper.clone();
    fenetre.connect_map_event(move |f, _| {
        d1(f);
        glib::Propagation::Proceed
    });
    fenetre.connect_size_allocate(move |w, _| {
        if let Some(f) = w.downcast_ref::<gtk::Window>() {
            decouper(f);
        }
    });
}

/// Attrape le pointeur pour une fenêtre, dès qu'elle est VRAIMENT affichée.
///
/// `present()` rend la main avant que X ait affiché la fenêtre ; une attrape
/// demandée dans la foulée échoue en silence (GDK_GRAB_NOT_VIEWABLE), et
/// cliquer à côté ne fermait plus rien. On réessaie toutes les 30 ms.
pub struct Attrape {
    siege: RefCell<Option<gdk::Seat>>,
    fini: Cell<bool>,
}

impl Attrape {
    pub fn prendre(cadre: &gtk::Window) -> Rc<Attrape> {
        let attrape = Rc::new(Attrape { siege: RefCell::new(None), fini: Cell::new(false) });
        let restants = Cell::new(40);
        let a = attrape.clone();
        let cadre = cadre.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(30), move || {
            if a.fini.get() {
                return glib::ControlFlow::Break;
            }
            restants.set(restants.get() - 1);
            let Some(fenetre) = cadre.window().filter(|f| f.is_viewable()) else {
                return if restants.get() > 0 { glib::ControlFlow::Continue } else { glib::ControlFlow::Break };
            };
            let Some(siege) = gdk::Display::default().and_then(|d| d.default_seat()) else {
                return glib::ControlFlow::Break;
            };
            let pris = siege.grab(&fenetre, gdk::SeatCapabilities::ALL_POINTING, true, None, None, None);
            if pris == gdk::GrabStatus::Success {
                *a.siege.borrow_mut() = Some(siege);
                return glib::ControlFlow::Break;
            }
            if restants.get() > 0 { glib::ControlFlow::Continue } else { glib::ControlFlow::Break }
        });
        attrape
    }

    pub fn lacher(&self) {
        self.fini.set(true);
        if let Some(siege) = self.siege.borrow_mut().take() {
            siege.ungrab();
        }
    }
}

/// Un signal Unix (SIGUSR1, SIGUSR2) dans la boucle GTK.
pub fn sur_signal(numero: i32, mut action: impl FnMut() + 'static) {
    glib::source::unix_signal_add_local(numero, move || {
        action();
        glib::ControlFlow::Continue
    });
}

pub const SIGUSR1: i32 = 10;
pub const SIGUSR2: i32 = 12;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_coins_sont_creuses_et_le_milieu_plein() {
        let r = rectangles_arrondis(100, 60, 14);
        // La première ligne est la plus creusée, et le milieu fait toute la largeur.
        assert!(r[0].0 > 5);
        assert!(r.iter().any(|&(x, _, w, h)| x == 0 && w == 100 && h > 20));
        // Toutes les lignes sont couvertes, une seule fois.
        let total: i32 = r.iter().map(|x| x.3).sum();
        assert_eq!(total, 60);
    }

    #[test]
    fn les_guillemets_tiennent() {
        assert_eq!(guillemets("l'image"), "'l'\\''image'");
    }
}
