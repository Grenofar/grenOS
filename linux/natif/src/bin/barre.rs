//! grenos-shell-natif : la barre des tâches de grenOS et son menu, en Rust.
//!
//! Remplace `/usr/bin/grenos-shell` (Python) geste pour geste : une icône par
//! application, le trait sous celle qui est ouverte (vif au premier plan,
//! éteint réduite), le menu avec ses épinglées et sa recherche, le réseau, le
//! son, l'horloge, la pastille des mises à jour. Les phrases du port série
//! sont les MÊMES mot pour mot : la CI clique sur « bouton grenos en x,y » et
//! compte les « menu ouvert ».
//!
//! POURQUOI : la barre tourne toute la session. En Python + PyGObject elle
//! pesait ~60 Mo et réveillait l'interpréteur chaque seconde ; ici, un
//! binaire GTK de quelques Mo, et le son se lit dans un fil à part pour que
//! `pactl` ne fige jamais l'horloge.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk::gdk::prelude::GdkContextExt;
use gtk::prelude::*;
use gtk::{gdk, gdk_pixbuf, glib, pango};

use natif::applications::{self as apps, Application, Epingle};
use natif::fenetres::{Ecran, Fenetre};
use natif::processus;
use natif::{classe, dire, image_de, lancer, poser_infobulle};

const HAUTEUR: i32 = 52;
// Le dock (G12) : plus haut pour des icônes plus grandes, et décollé du bord.
const HAUTEUR_DOCK: i32 = 64;
const MARGE_DOCK: i32 = 8;
const ICONE: i32 = 26;
const ICONE_DOCK: i32 = 38;
const MENU_LARGEUR: i32 = 640;
const MENU_HAUTEUR: i32 = 640;

const JOURS: [&str; 7] = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."];
const MOIS: [&str; 12] = ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."];

// La feuille de la barre : celle de grenos-shell, inchangée.
const STYLE: &str = r#"
#panneau {
    background-image: linear-gradient(to bottom, @fond-haut, @fond);
    border-top: 1px solid @bord;
}
/* LA BARRE DÉCOLLÉE (19 novembre) : « fais que la barre des tâches ne se
   colle pas aux extrémités ». 8 px d'air tout autour, les coins de la découpe
   (natif::RAYON_COINS), un liseré clair : elle flotte au-dessus du bureau. */
#panneau.flottante {
    border: 1px solid @bord-clair;
    border-radius: 14px;
}
#panneau.flottante button { border-radius: 9px; }
#panneau button {
    color: @texte;
    background-color: transparent;
    background-image: none;
    border: 1px solid transparent;
    border-radius: 10px;
    padding: 2px 10px;
    margin: 3px 2px;
    box-shadow: none;
    text-shadow: none;
    transition: background-color 130ms ease, border-color 130ms ease;
}
#panneau button:hover { background-color: @survol; border-color: @bord; }
#bouton-menu { font-weight: 700; padding-left: 14px; padding-right: 14px; }
#bouton-menu:checked {
    background-image: linear-gradient(135deg, @accent, @second);
    color: #ffffff;
    border-color: transparent;
}
/* La bulle d'aperçus du dock (19 novembre). Découpée en forme par
   natif::arrondir comme le menu : ses coins montrent le bureau, pas du noir. */
#apercu { background-color: @fond-haut; border: 1px solid @bord-clair; border-radius: 14px; }
.apercu-carte { padding: 8px; border-radius: 12px; }
.apercu-titre { color: @texte; font-size: 12px; font-weight: 600; }
.apercu-fermer {
    min-width: 22px; min-height: 22px; padding: 0; margin: 0;
    border-radius: 7px; border: none; background-image: none; background-color: transparent;
    color: @texte-faible; box-shadow: none;
}
.apercu-fermer:hover { background-color: #e81123; color: #ffffff; }
#zone-etat {
    background-color: alpha(@texte, 0.045);
    border: 1px solid alpha(@texte, 0.07);
    border-radius: 10px;
    padding: 0 2px;
}
#zone-etat button { padding: 2px 7px; margin: 3px 0; border-radius: 8px; }
#bouton-coulisses { padding: 2px 8px; margin: 3px 2px; border-radius: 8px; font-size: 13px; }
.trait {
    background-color: transparent;
    border-radius: 999px;
    min-height: 3px;
    margin: 0 14px 3px 14px;
    transition: background-color 220ms ease, opacity 220ms ease, margin 220ms ease;
    opacity: 0;
}
.trait-ouverte { background-color: alpha(@accent, 0.55); opacity: 1; margin: 0 16px 3px 16px; }
.trait-active { background-color: @accent-clair; opacity: 1; margin: 0 9px 3px 9px; }
.trait-reduite { background-color: alpha(@accent, 0.30); opacity: 1; margin: 0 19px 3px 19px; }
/* L'horloge sur deux lignes, l'heure au-dessus de la date, comme Windows. */
#horloge { color: @texte; font-size: 12px; padding: 0 14px; }
#menu { background-color: @fond; border: 1px solid @bord-clair; border-radius: 14px; }
#menu entry { border-radius: 10px; }
#recherche-menu { margin: 16px 16px 6px; }
#pied-menu { background-color: @fond-haut; border-top: 1px solid @bord; padding: 12px 16px; }
/* L'étiquette de section de la deuxième édition : petite, espacée, en accent. */
.titre-section { font-size: 10.5px; font-weight: 700; letter-spacing: 0.16em; color: @accent-clair; padding: 12px 18px 4px; }
.epingle {
    background-color: transparent; background-image: none;
    border: 1px solid transparent; border-radius: 12px; padding: 12px 6px; margin: 3px;
}
.epingle:hover { background-color: @survol; border-color: @bord-clair; }
.epingle label { font-size: 12px; }
.entree {
    background-color: transparent; background-image: none;
    border: 1px solid transparent; border-radius: 10px; padding: 7px 12px; margin: 1px 10px;
}
.entree:hover { background-color: @survol; border-color: @bord; }
/* Le dock : la même barre, flottante. Le rayon est celui de la découpe
   (natif::RAYON_COINS) : un coin dessiné plus rond que la fenêtre laisserait
   un liseré dans l'angle. */
/* LE DOCK, DEUXIÈME DESSIN (19 novembre) : « améliore et redessine le dock ».
   Un verre sombre aux reflets du haut, des icônes sans cadre qui grossissent
   au survol, un POINT sous l'application ouverte (une pastille sous celle au
   premier plan) au lieu du trait de la barre, et un séparateur fin entre les
   applications et la zone d'état. */
#panneau.dock {
    background-image: linear-gradient(to bottom, alpha(@texte, 0.07), alpha(@texte, 0.02));
    background-color: alpha(@fond, 0.96);
    border: 1px solid alpha(@texte, 0.12);
    border-radius: 14px;
}
#panneau.dock button {
    padding: 5px 7px; margin: 6px 1px 1px 1px; border-radius: 12px;
    border-color: transparent; background-color: transparent;
}
#panneau.dock button:hover { background-color: alpha(@texte, 0.07); border-color: transparent; }
#panneau.dock button image { transition: -gtk-icon-transform 160ms ease; }
#panneau.dock button:hover image { -gtk-icon-transform: scale(1.14) translateY(-2px); }
#panneau.dock #bouton-menu { padding: 5px 9px; }
#panneau.dock #bouton-menu:checked { background-image: linear-gradient(135deg, @accent, @second); }
#panneau.dock .trait { min-height: 4px; margin: 0 24px 3px 24px; }
#panneau.dock .trait-ouverte { background-color: alpha(@texte, 0.55); margin: 0 25px 3px 25px; }
#panneau.dock .trait-reduite { background-color: alpha(@texte, 0.28); margin: 0 25px 3px 25px; }
#panneau.dock .trait-active { background-color: @accent-clair; margin: 0 18px 3px 18px; }
#panneau.dock separator { background-color: alpha(@texte, 0.12); min-width: 1px; margin: 12px 6px; }
#panneau.dock #zone-etat { background-color: transparent; border-color: transparent; }
#panneau.dock #horloge { padding: 0 12px 0 6px; }
"#;

/// Une case de la barre : l'icône d'une application, et son trait.
struct Tuile {
    nom: String,
    commande: String,
    icone: String,
    epinglee: bool,
    fenetres: Vec<Fenetre>,
}

struct Barre {
    panneau: gtk::Window,
    largeur: i32,
    hauteur_ecran: i32,
    /// Le coin de l'écran principal dans la racine (0,0 avec un seul écran).
    ox: i32,
    oy: i32,
    /// G12 : la disposition dock, lue au démarrage (Réglages relance la barre).
    dock: bool,
    bouton_menu: gtk::ToggleButton,
    poignee: RefCell<Option<glib::SignalHandlerId>>,
    tuiles: gtk::Box,
    horloge: gtk::Label,
    bouton_son: gtk::Button,
    image_son: gtk::Image,
    bouton_reseau: gtk::Button,
    image_reseau: gtk::Image,
    /// La batterie (F3) : cachée sur une machine qui n'en a pas.
    bouton_batterie: gtk::Button,
    image_batterie: gtk::Image,
    texte_batterie: gtk::Label,
    batterie_prevenue: std::cell::Cell<bool>,
    /// F4 : l'alerte de chaleur a été donnée, en attendant que ça retombe.
    chaleur_prevenue: std::cell::Cell<bool>,
    bouton_maj: gtk::Button,
    bouton_coulisses: gtk::Button,
    menu_coulisses: RefCell<Option<gtk::Menu>>,
    /// Le bouton gauche est enfoncé sur une tuile (depuis quand) : on ne la
    /// refait pas. Au-delà de 3 s (un glisser, un relâchement perdu), on
    /// n'attend plus — la barre ne doit jamais se figer.
    appui: std::cell::Cell<Option<std::time::Instant>>,
    /// Ce que montraient les tuiles la dernière fois (voir `refaire`).
    signature_tuiles: RefCell<String>,
    epingles: RefCell<Vec<Epingle>>,
    applications: RefCell<Vec<Application>>,
    ecran: Option<Ecran>,
    etat_tuiles: RefCell<String>,
    // Le menu
    menu: gtk::Window,
    recherche: gtk::SearchEntry,
    contenu: gtk::Box,
    attrape: RefCell<Option<Rc<natif::Attrape>>>,
    menu_contextuel: RefCell<Option<gtk::Menu>>,
    /// Le bouton « Éteindre » du pied du menu : la CI le vise (18 novembre).
    bouton_eteindre: RefCell<Option<gtk::Button>>,
    /// La bulle d'aperçus ouverte au survol d'une tuile, s'il y en a une.
    apercu: RefCell<Option<gtk::Window>>,
    /// Le minuteur qui l'ouvrira ou la fermera ; un seul à la fois.
    minuteur_apercu: RefCell<Option<glib::SourceId>>,
    /// Le pointeur est-il sur la bulle ? Alors elle reste.
    survol_apercu: std::cell::Cell<bool>,
}

fn taille_menu() -> (i32, i32) {
    let (_, _, l, h) = natif::ecran_de_la_barre();
    let bande = natif::bande_reservee(if natif::disposition_dock() { HAUTEUR_DOCK } else { HAUTEUR }, MARGE_DOCK, true);
    ((l - 24).min(MENU_LARGEUR).max(420), (h - bande - 24).min(MENU_HAUTEUR).max(360))
}

fn temps() -> u32 {
    gtk::current_event_time()
}

/// Ouvre un panneau, ou le referme s'il est déjà là (un bouton de barre
/// bascule). `pgrep -f` lit la ligne de commande : nos panneaux Python
/// s'appellent tous « python3 » pour le noyau.
fn basculer(programme: &str) {
    // La version Python (/usr/bin/…) ou native (/usr/lib/grenos/…-natif) :
    // le panneau du son passe la main à la seconde dès sa première ligne, et
    // un second clic doit la trouver pour la refermer (18 novembre).
    let chemin = format!("(/usr/bin/{programme}|/usr/lib/grenos/{programme}-natif)( |$)");
    let moi = unsafe_uid();
    let deja = std::process::Command::new("pgrep").args(["-f", "-u", &moi, &chemin]).output();
    if let Ok(sortie) = deja {
        if sortie.status.success() && !sortie.stdout.is_empty() {
            let _ = std::process::Command::new("pkill").args(["-f", "-u", &moi, &chemin]).status();
            return;
        }
    }
    lancer(programme, false);
}

/// L'uid courant, lu dans /proc (pas de libc pour une seule valeur).
fn unsafe_uid() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|t| t.lines().find(|l| l.starts_with("Uid:")).and_then(|l| l.split_whitespace().nth(1)).map(String::from))
        .unwrap_or_else(|| "0".into())
}

/// Un raccourci sur le bureau : une copie du fichier .desktop.
fn poser_sur_le_bureau(app: &Application) {
    let bureau = natif::maison().join("Bureau");
    let _ = std::fs::create_dir_all(&bureau);
    let Some(nom) = app.fichier.file_name() else { return };
    let cible = bureau.join(nom);
    if std::fs::copy(&app.fichier, &cible).is_ok() {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&cible, std::fs::Permissions::from_mode(0o755));
    }
}

fn section(texte: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(texte));
    l.set_xalign(0.0);
    classe(&l, "titre-section");
    l
}

fn menu_de(gestes: Vec<(&'static str, Box<dyn Fn()>)>, ev: &gdk::EventButton) -> gtk::Menu {
    let menu = gtk::Menu::new();
    for (texte, action) in gestes {
        let element = gtk::MenuItem::with_label(texte);
        element.connect_activate(move |_| action());
        menu.append(&element);
    }
    menu.show_all();
    menu.popup_at_pointer(Some(&**ev));
    menu
}

/// Une entrée de menu : icône (si le thème la connaît) et texte.
fn entree(texte: &str, icone: &str, sous: Option<&gtk::Menu>) -> gtk::MenuItem {
    let element = gtk::MenuItem::new();
    let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    if let Some(i) = image_de(&[icone, "application-x-executable"], 18) {
        ligne.pack_start(&i, false, false, 0);
    }
    let l = gtk::Label::new(Some(texte));
    l.set_xalign(0.0);
    ligne.pack_start(&l, true, true, 0);
    element.add(&ligne);
    if let Some(s) = sous {
        element.set_submenu(Some(s));
    }
    element
}

fn ajouter(menu: &gtk::Menu, texte: &str, action: impl Fn() + 'static) {
    let element = gtk::MenuItem::with_label(texte);
    element.connect_activate(move |_| action());
    menu.append(&element);
}

fn couper(texte: &str, n: usize) -> String {
    if texte.chars().count() <= n { texte.to_string() } else { format!("{}…", texte.chars().take(n).collect::<String>()) }
}

/// Un nom de classe brut (« steamwebhelper ») commence par une majuscule.
fn nom_affiche(nom: &str) -> String {
    let mut c = nom.chars();
    c.next().map(|p| p.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// « com.valvesoftware.Steam » pour une application Flatpak, sinon "".
fn identifiant_flatpak(a: &Application) -> String {
    let chemin = a.fichier.to_string_lossy();
    if !chemin.contains("/flatpak/") {
        return String::new();
    }
    a.fichier.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Les applications Flatpak en cours, une fois chacune.
fn flatpaks_en_cours() -> Vec<String> {
    let Ok(sortie) = std::process::Command::new("flatpak").args(["ps", "--columns=application"]).output() else {
        return Vec::new();
    };
    let mut ids: Vec<String> = String::from_utf8_lossy(&sortie.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && l.contains('.') && !l.starts_with("org.freedesktop.") && !l.starts_with("org.gtk."))
        .collect();
    ids.dedup();
    ids
}

/// Fermer de force : `flatpak kill` pour une Flatpak, sinon le programme
/// par son nom — jamais un programme de grenOS lui-même.
fn forcer_arret(programme: &str, flatpak: &str) {
    if !flatpak.is_empty() {
        let _ = std::process::Command::new("flatpak").args(["kill", flatpak]).status();
        return;
    }
    let nom = std::path::Path::new(programme).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if nom.is_empty() || nom.starts_with("grenos") || nom == "openbox" || nom == "flatpak" {
        return;
    }
    // pkill compare les 15 premiers caractères du nom du processus.
    let court: String = nom.chars().take(15).collect();
    let _ = std::process::Command::new("pkill").args(["-x", court.as_str()]).status();
}

impl Barre {
    fn nouvelle() -> Rc<Barre> {
        // L'écran de cette barre (le principal, ou celui d'une copie
        // secondaire), et où il est dans la racine (plusieurs écrans).
        let (ox, oy, largeur, hauteur_ecran) = natif::ecran_de_la_barre();

        let panneau = gtk::Window::new(gtk::WindowType::Toplevel);
        panneau.set_widget_name("panneau");
        panneau.set_type_hint(gdk::WindowTypeHint::Dock);
        panneau.set_decorated(false);
        panneau.set_keep_above(true);
        panneau.set_skip_taskbar_hint(true);
        panneau.set_skip_pager_hint(true);
        let dock = natif::disposition_dock();
        if dock {
            // Pas de largeur imposée : le dock prend celle de ce qu'il porte,
            // et se recentre chaque fois qu'elle change (une application
            // s'ouvre, une autre se ferme).
            panneau.set_size_request(-1, HAUTEUR_DOCK);
            classe(&panneau, "dock");
            natif::arrondir(&panneau);
            panneau.connect_size_allocate(move |f, _| {
                let (x, y) = natif::place_dock((largeur, hauteur_ecran), f.size(), MARGE_DOCK);
                let (x, y) = (ox + x, oy + y);
                if f.position() != (x, y) {
                    f.move_(x, y);
                }
            });
            panneau.move_(ox + (largeur - 600).max(0) / 2, oy + hauteur_ecran - HAUTEUR_DOCK - MARGE_DOCK);
        } else {
            // Pleine largeur MOINS l'air des deux côtés, et décollée du bas.
            panneau.set_default_size(largeur - 2 * MARGE_DOCK, HAUTEUR);
            panneau.move_(ox + MARGE_DOCK, oy + hauteur_ecran - HAUTEUR - MARGE_DOCK);
            classe(&panneau, "flottante");
            natif::arrondir(&panneau);
        }

        let boite = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        panneau.add(&boite);

        let bouton_son = gtk::Button::new();
        let image_son = gtk::Image::new();
        bouton_son.add(&image_son);
        bouton_son.set_tooltip_text(Some("Son : volume, sortie, applications"));
        bouton_son.connect_clicked(|_| basculer("grenos-son"));

        boite.pack_start(&gtk::Box::new(gtk::Orientation::Horizontal, 0), true, true, 0);
        let centre = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        boite.pack_start(&centre, false, false, 0);

        let bouton_menu = gtk::ToggleButton::new();
        let etiquette = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        if gtk::IconTheme::default().map(|t| t.has_icon("grenos")).unwrap_or(false) {
            etiquette.pack_start(&gtk::Image::from_icon_name(Some("grenos"), gtk::IconSize::LargeToolbar), false, false, 0);
        }
        // Le logo seul, comme le bouton Démarrer de Windows 11 (19 novembre) :
        // le mot « grenOS » à côté faisait étiquette. Le nom reste dans
        // l'infobulle. Sans thème d'icônes, le mot revient : un bouton vide
        // serait une promesse sans rien dedans.
        if etiquette.children().is_empty() {
            etiquette.pack_start(&gtk::Label::new(Some("grenOS")), false, false, 0);
        }
        bouton_menu.add(&etiquette);
        bouton_menu.set_widget_name("bouton-menu");
        bouton_menu.set_tooltip_text(Some("Menu grenOS"));
        centre.pack_start(&bouton_menu, false, false, 0);

        let tuiles = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        centre.pack_start(&tuiles, false, false, 6);
        boite.pack_start(&gtk::Box::new(gtk::Orientation::Horizontal, 0), true, true, 0);

        let horloge = gtk::Label::new(None);
        horloge.set_widget_name("horloge");
        horloge.set_justify(gtk::Justification::Right);
        boite.pack_end(&horloge, false, false, 0);

        let bouton_reseau = gtk::Button::new();
        let image_reseau = gtk::Image::new();
        bouton_reseau.add(&image_reseau);
        bouton_reseau.connect_clicked(|_| lancer("grenos-connexions", false));
        // LA ZONE D'ÉTAT (19 novembre) : batterie, réseau et son dans UN
        // groupe, sur un fond commun, comme Windows 11 — trois boutons
        // espacés au hasard faisaient « pas fini » (capture de Grenofar).
        // Chacun garde son geste. `pack_start` : l'ordre se lit de gauche à
        // droite, batterie, réseau, son.
        let zone_etat = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        zone_etat.set_widget_name("zone-etat");
        zone_etat.set_valign(gtk::Align::Center);

        // La batterie, à gauche du réseau comme sur Windows. Cachée tant
        // qu'on n'a pas lu qu'elle existe : un fixe n'en montre jamais.
        let bouton_batterie = gtk::Button::new();
        let contenu_batterie = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let image_batterie = gtk::Image::new();
        let texte_batterie = gtk::Label::new(None);
        contenu_batterie.pack_start(&image_batterie, false, false, 0);
        contenu_batterie.pack_start(&texte_batterie, false, false, 0);
        bouton_batterie.add(&contenu_batterie);
        bouton_batterie.set_no_show_all(true);
        contenu_batterie.show_all();
        bouton_batterie.connect_clicked(|_| lancer("grenos-hub", false));
        zone_etat.pack_start(&bouton_batterie, false, false, 0);
        zone_etat.pack_start(&bouton_reseau, false, false, 0);
        zone_etat.pack_start(&bouton_son, false, false, 0);
        boite.pack_end(&zone_etat, false, false, 4);
        // « ^ » (23 novembre), comme la flèche des icônes cachées de
        // Windows : ce qui tourne — fenêtres ouvertes et applications en
        // fond (Steam, Heroic… en Flatpak) — et de quoi les fermer.
        // La flèche du thème (pan-up-symbolic, livrée avec GTK) : un « ⌃ »
        // tapé dépendrait de la police.
        let bouton_coulisses = gtk::Button::new();
        bouton_coulisses.add(&gtk::Image::from_icon_name(Some("pan-up-symbolic"), gtk::IconSize::Button));
        bouton_coulisses.set_widget_name("bouton-coulisses");
        bouton_coulisses.set_valign(gtk::Align::Center);
        poser_infobulle(&bouton_coulisses, "Applications en cours");
        boite.pack_end(&bouton_coulisses, false, false, 0);
        // Dans le dock, un trait fin sépare les applications de la zone d'état.
        if dock {
            boite.pack_end(&gtk::Separator::new(gtk::Orientation::Vertical), false, false, 0);
        }

        let bouton_maj = gtk::Button::with_label("");
        bouton_maj.set_no_show_all(true);
        bouton_maj.connect_clicked(|_| lancer("grenos-maj", false));
        boite.pack_end(&bouton_maj, false, false, 0);

        // ---- Le menu ----
        let menu = gtk::Window::new(gtk::WindowType::Toplevel);
        menu.set_widget_name("menu");
        let (ml, mh) = taille_menu();
        menu.set_size_request(ml, mh);
        menu.set_decorated(false);
        menu.set_skip_taskbar_hint(true);
        menu.set_skip_pager_hint(true);
        menu.set_type_hint(gdk::WindowTypeHint::Utility);
        menu.set_keep_above(true);
        natif::arrondir(&menu);

        let vboite = gtk::Box::new(gtk::Orientation::Vertical, 0);
        menu.add(&vboite);
        let recherche = gtk::SearchEntry::new();
        recherche.set_widget_name("recherche-menu");
        recherche.set_placeholder_text(Some("Chercher une application"));
        vboite.pack_start(&recherche, false, false, 0);
        let defilement = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        defilement.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        vboite.pack_start(&defilement, true, true, 0);
        let contenu = gtk::Box::new(gtk::Orientation::Vertical, 0);
        defilement.add(&contenu);

        let barre = Rc::new(Barre {
            panneau,
            largeur,
            hauteur_ecran,
            ox,
            oy,
            dock,
            bouton_menu,
            poignee: RefCell::new(None),
            tuiles,
            horloge,
            bouton_son,
            image_son,
            bouton_reseau,
            image_reseau,
            bouton_batterie,
            image_batterie,
            texte_batterie,
            batterie_prevenue: std::cell::Cell::new(false),
            chaleur_prevenue: std::cell::Cell::new(false),
            bouton_maj,
            bouton_coulisses,
            menu_coulisses: RefCell::new(None),
            appui: std::cell::Cell::new(None),
            signature_tuiles: RefCell::new(String::new()),
            epingles: RefCell::new(apps::lire_epingles()),
            applications: RefCell::new(apps::applications()),
            ecran: Ecran::ouvrir(),
            etat_tuiles: RefCell::new(String::new()),
            menu,
            recherche,
            contenu,
            attrape: RefCell::new(None),
            menu_contextuel: RefCell::new(None),
            bouton_eteindre: RefCell::new(None),
            apercu: RefCell::new(None),
            minuteur_apercu: RefCell::new(None),
            survol_apercu: std::cell::Cell::new(false),
        });

        vboite.pack_start(&barre.pied(), false, false, 0);
        barre.brancher();
        barre
    }

    /// La bande du bas que la barre occupe (dock : avec l'air dessous).
    fn bande(&self) -> i32 {
        natif::bande_reservee(if self.dock { HAUTEUR_DOCK } else { HAUTEUR }, MARGE_DOCK, true)
    }

    fn icone(&self) -> i32 {
        if self.dock { ICONE_DOCK } else { ICONE }
    }

    fn pied(self: &Rc<Self>) -> gtk::Box {
        let pied = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        pied.set_widget_name("pied-menu");
        let id = natif::identite();
        let nom = if id["nom"].is_empty() { id["login"].clone() } else { id["nom"].clone() };
        let lettre = nom.trim().chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into());
        let avatar = gtk::Label::new(Some(&lettre));
        classe(&avatar, "avatar");
        avatar.set_size_request(32, 32);
        pied.pack_start(&avatar, false, false, 0);
        let qui = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let l_nom = gtk::Label::new(Some(&nom));
        l_nom.set_xalign(0.0);
        classe(&l_nom, "entree-titre");
        let l_adr = gtk::Label::new(Some(&natif::adresse()));
        l_adr.set_xalign(0.0);
        classe(&l_adr, "sous");
        qui.pack_start(&l_nom, false, false, 0);
        qui.pack_start(&l_adr, false, false, 0);
        pied.pack_start(&qui, true, true, 0);
        let eteindre = self.petit("Éteindre", "grenos-arret", &["system-shutdown", "system-log-out"]);
        pied.pack_end(&eteindre, false, false, 0);
        *self.bouton_eteindre.borrow_mut() = Some(eteindre);
        pied.pack_end(
            &self.petit("Réglages", "grenos-parametres", &["grenos-parametres", "preferences-system", "applications-system"]),
            false, false, 0,
        );
        pied
    }

    fn petit(self: &Rc<Self>, texte: &str, commande: &'static str, icones: &[&str]) -> gtk::Button {
        let bouton = gtk::Button::new();
        let contenu = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        if let Some(i) = image_de(icones, 18) {
            contenu.pack_start(&i, false, false, 0);
        }
        contenu.pack_start(&gtk::Label::new(Some(texte)), false, false, 0);
        bouton.add(&contenu);
        classe(&bouton, "discret");
        let b = self.clone();
        bouton.connect_clicked(move |_| {
            b.fermer_menu();
            lancer(commande, false);
        });
        bouton
    }

    fn brancher(self: &Rc<Self>) {
        let b = self.clone();
        self.bouton_coulisses.connect_clicked(move |bouton| b.ouvrir_coulisses(bouton));
        let b = self.clone();
        self.panneau.connect_realize(move |p| {
            if let (Some(ecran), Some(f)) = (&b.ecran, p.window()) {
                if let Ok(x) = f.downcast::<gdkx11::X11Window>() {
                    let hauteur_racine = natif::taille_ecran().1;
                    let bande = natif::bande_racine(hauteur_racine, b.oy, b.hauteur_ecran, b.bande());
                    ecran.reserver(x.xid() as u32, bande, b.ox.max(0) as u32, (b.ox + b.largeur - 1).max(0) as u32);
                }
            }
        });

        // Lâcher une application sur le dock l'épingle. Pas de HIGHLIGHT : le
        // cadre que GTK dessinerait autour jurerait avec le dock arrondi.
        self.panneau.drag_dest_set(gtk::DestDefaults::MOTION | gtk::DestDefaults::DROP, &[], gdk::DragAction::COPY);
        self.panneau.drag_dest_add_uri_targets();
        let b = self.clone();
        self.panneau.connect_drag_data_received(move |_, _, _, _, donnees, _, _| b.recevoir_sur_le_dock(donnees));

        let b = self.clone();
        let id = self.bouton_menu.connect_clicked(move |bouton| {
            if bouton.is_active() { b.ouvrir_menu() } else { b.fermer_menu() }
        });
        *self.poignee.borrow_mut() = Some(id);

        let b = self.clone();
        self.recherche.connect_search_changed(move |_| b.remplir());
        let b = self.clone();
        self.recherche.connect_activate(move |_| {
            let premiere = {
                let liste = b.applications.borrow();
                apps::visibles(&liste, &b.recherche.text()).first().map(|a| (*a).clone())
            };
            if let Some(a) = premiere {
                b.choisir(&a);
            } else if let Some(r) = apps::reglages_trouves(&b.recherche.text()).first() {
                // Aucune application, mais un réglage : Entrée l'ouvre.
                b.fermer_menu();
                lancer(&r.commande(), false);
            }
        });
        for w in [self.menu.clone().upcast::<gtk::Widget>(), self.recherche.clone().upcast()] {
            let b = self.clone();
            w.connect_key_press_event(move |_, ev| {
                if ev.keyval() == gdk::keys::constants::Escape {
                    b.fermer_menu();
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
        }
        self.menu.add_events(gdk::EventMask::BUTTON_PRESS_MASK);
        let b = self.clone();
        self.menu.connect_button_press_event(move |m, ev| {
            // Avec l'attrape, un clic ailleurs arrive ICI, hors de nos bornes.
            // Les coordonnées d'ÉCRAN : `position()` est relative à la
            // sous-fenêtre cliquée (une liste défilée peut dépasser la hauteur
            // du menu et passer pour un clic à côté).
            let (rx, ry) = ev.root();
            let (_, ox, oy) = m.window().map(|f| f.origin()).unwrap_or((0, 0, 0));
            let (l, h) = m.size();
            let (x, y) = (rx - ox as f64, ry - oy as f64);
            if !(0.0..l as f64).contains(&x) || !(0.0..h as f64).contains(&y) {
                b.fermer_menu();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });

        // Les fenêtres des autres : on écoute la connexion X dans la boucle
        // GTK, et on regroupe les rafales d'événements en un seul refaire.
        if let Some(ecran) = &self.ecran {
            let b = self.clone();
            let en_attente = Rc::new(std::cell::Cell::new(false));
            glib::source::unix_fd_add_local(ecran.fd(), glib::IOCondition::IN, move |_, _| {
                if b.ecran.as_ref().map(|e| e.evenements()).unwrap_or(false) && !en_attente.get() {
                    en_attente.set(true);
                    let b2 = b.clone();
                    let attente = en_attente.clone();
                    glib::timeout_add_local_once(Duration::from_millis(60), move || {
                        attente.set(false);
                        b2.refaire();
                    });
                }
                glib::ControlFlow::Continue
            });
        } else {
            dire("barre : pas de connexion X pour la liste des fenetres");
        }
        self.refaire();

        let b = self.clone();
        glib::timeout_add_seconds_local(1, move || {
            b.tic();
            // Filet : un événement resté dans le tampon de x11rb après une
            // lecture (afficher, réduire) est rattrapé dans la seconde.
            if b.ecran.as_ref().map(|e| e.evenements()).unwrap_or(false) {
                b.refaire();
            }
            glib::ControlFlow::Continue
        });
        self.tic();
        let b = self.clone();
        glib::timeout_add_seconds_local(4, move || {
            b.relire_reseau();
            glib::ControlFlow::Continue
        });
        self.relire_reseau();
        let b = self.clone();
        glib::timeout_add_seconds_local(60, move || {
            b.relire_maj();
            glib::ControlFlow::Continue
        });
        self.relire_maj();
        let b = self.clone();
        glib::timeout_add_seconds_local(30, move || {
            b.relire_batterie();
            b.relire_chaleur();
            glib::ControlFlow::Continue
        });
        self.relire_batterie();
        self.relire_chaleur();
        self.surveiller_son();
    }

    /// La chaleur (F4) : une notification quand une zone approche de son
    /// seuil critique, une seule, réarmée quand tout est redescendu.
    fn relire_chaleur(&self) {
        use natif::thermique;
        // Une seule alerte pour toute la machine : celle de la barre principale.
        if natif::ecran_secondaire().is_some() {
            return;
        }
        let mut zones = thermique::lire_zones(std::path::Path::new("/sys/class/thermal"));
        zones.extend(thermique::lire_sondes(std::path::Path::new("/sys/class/hwmon")));
        if self.chaleur_prevenue.get() {
            if thermique::retombe(&zones) {
                self.chaleur_prevenue.set(false);
            }
            return;
        }
        if let Some((degres, critique)) = thermique::alerte(&zones) {
            self.chaleur_prevenue.set(true);
            dire(&format!("chaleur : {degres} degres, seuil critique {critique}"));
            lancer(
                &format!(
                    "notify-send --app-name=grenOS --urgency=critical --icon=dialog-warning 'La machine chauffe' '{degres} °C, à {} °C de l’arrêt de sécurité. Ferme ce qui tourne fort, et vérifie que les aérations ne sont pas bouchées.'",
                    (critique - degres).max(0)
                ),
                false,
            );
        }
    }

    /// La batterie (F3), lue dans /sys : aucun programme lancé.
    fn relire_batterie(&self) {
        use natif::batterie;
        let Some(b) = batterie::lire_batterie(std::path::Path::new("/sys/class/power_supply")) else {
            self.bouton_batterie.hide();
            return;
        };
        let (icone, phrase) = batterie::decrire(b);
        self.image_batterie.set_from_icon_name(Some(icone), gtk::IconSize::LargeToolbar);
        self.texte_batterie.set_text(&format!("{} %", b.pour_cent));
        poser_infobulle(&self.bouton_batterie, &phrase);
        self.bouton_batterie.show();
        if natif::ecran_secondaire().is_none() && batterie::prevenir(b, self.batterie_prevenue.get()) {
            self.batterie_prevenue.set(true);
            lancer(
                &format!(
                    "notify-send --app-name=grenOS --urgency=critical --icon=battery-caution 'Batterie faible' 'Plus que {} %. Branche le chargeur pour ne rien perdre.'",
                    b.pour_cent
                ),
                false,
            );
        } else if b.etat != batterie::Etat::SurBatterie {
            // Rebranché : la prochaine descente sous 10 % prévient de nouveau.
            self.batterie_prevenue.set(false);
        }
    }

    // ---- Les tuiles ----------------------------------------------------------
    fn refaire(self: &Rc<Self>) {
        // UN CLIC EN COURS N'EST JAMAIS COUPÉ (23 novembre). Grenofar :
        // « quand je clique sur le logo de l'app ouverte dans le dock, ça
        // n'ouvre pas, en tout cas pas toutes les apps ». Refaire les tuiles
        // DÉTRUIT leurs boutons ; entre l'appui et le relâchement, le bouton
        // n'existait plus et le clic se perdait. On attend le relâchement.
        if self.appui.get().map(|t| t.elapsed() < Duration::from_secs(3)).unwrap_or(false) {
            let b = self.clone();
            glib::timeout_add_local_once(Duration::from_millis(120), move || b.refaire());
            return;
        }
        let (ouvertes, active) = match &self.ecran {
            Some(e) => (e.fenetres(), e.active()),
            None => (Vec::new(), None),
        };
        // ET ON NE REFAIT QUE CE QUI A CHANGÉ À L'ÉCRAN. Steam, un navigateur,
        // Discord changent sans cesse leurs états et l'ordre des fenêtres :
        // chaque changement refaisait toute la barre, et un clic tombait
        // souvent au milieu. Les tuiles ne dépendent que de ceci.
        let signature = format!(
            "{:?}|{:?}|{}|{}",
            ouvertes.iter().map(|f| (f.xid, &f.classe, f.reduite)).collect::<Vec<_>>(),
            active.filter(|a| ouvertes.iter().any(|f| f.xid == *a)),
            self.epingles.borrow().iter().map(|e| e.commande.as_str()).collect::<Vec<_>>().join(","),
            self.applications.borrow().len()
        );
        if *self.signature_tuiles.borrow() == signature {
            return;
        }
        *self.signature_tuiles.borrow_mut() = signature;
        // Les tuiles vont être refaites : une bulle ouverte pointerait sur
        // des fenêtres qui ont peut-être changé (celle qu'on vient de fermer).
        if !self.survol_apercu.get() {
            self.fermer_apercu();
        }
        for enfant in self.tuiles.children() {
            self.tuiles.remove(&enfant);
        }
        let applis = self.applications.borrow().clone();
        let mut prises = std::collections::HashSet::new();
        let mut rejointes = Vec::new();

        for e in self.epingles.borrow().iter() {
            let miennes: Vec<Fenetre> = ouvertes
                .iter()
                .filter(|f| apps::meme_application(&f.classe, &f.titre, &e.commande, &applis))
                .cloned()
                .collect();
            prises.extend(miennes.iter().map(|f| f.xid));
            if !miennes.is_empty() {
                rejointes.push(e.nom.clone());
            }
            let t = Tuile { nom: e.nom.clone(), commande: e.commande.clone(), icone: e.icone.clone(), epinglee: true, fenetres: miennes };
            self.tuiles.pack_start(&self.faire_tuile(t, active, None), false, false, 0);
        }

        // Ce qui tourne sans être épinglé, groupé par classe, dans l'ordre.
        let mut restantes: Vec<(String, Vec<Fenetre>)> = Vec::new();
        for f in ouvertes.iter().filter(|f| !prises.contains(&f.xid)) {
            let groupe = apps::groupe_de(&f.classe, &f.titre);
            match restantes.iter_mut().find(|(g, _)| *g == groupe) {
                Some((_, v)) => v.push(f.clone()),
                None => restantes.push((groupe, vec![f.clone()])),
            }
        }

        let mut noms: Vec<&str> = restantes.iter().map(|(g, _)| g.as_str()).collect();
        noms.sort();
        let etat = format!(
            "tuiles : rejointes [{}], a part [{}]",
            if rejointes.is_empty() { "aucune".into() } else { rejointes.join(", ") },
            if noms.is_empty() { "aucune".into() } else { noms.join(", ") }
        );
        if *self.etat_tuiles.borrow() != etat {
            dire(&etat);
            *self.etat_tuiles.borrow_mut() = etat;
        }

        for (groupe, fenetres) in restantes {
            // Un jeu (« steam_app_252950 ») : son raccourci ou son titre, et
            // l'icône de sa fenêtre — plus jamais Steam (24 novembre).
            let app = apps::application_de_fenetre(&groupe, &fenetres[0].titre, &applis);
            let t = Tuile {
                nom: apps::nom_de_fenetre(&groupe, &fenetres[0].titre, app),
                commande: app.map(|a| a.commande.clone()).unwrap_or_default(),
                icone: app.map(|a| a.icone.clone()).unwrap_or_default(),
                epinglee: false,
                fenetres,
            };
            // Sans application connue, l'icône que la fenêtre déclare.
            let propre = if t.icone.is_empty() {
                self.ecran.as_ref().and_then(|e| e.icone(t.fenetres[0].xid, self.icone() as u32)).map(|(rgba, l, h)| {
                    let p = gdk_pixbuf::Pixbuf::from_bytes(
                        &glib::Bytes::from_owned(rgba), gdk_pixbuf::Colorspace::Rgb, true, 8, l as i32, h as i32, (l * 4) as i32,
                    );
                    p.scale_simple(self.icone(), self.icone(), gdk_pixbuf::InterpType::Bilinear).unwrap_or(p)
                })
            } else {
                None
            };
            self.tuiles.pack_start(&self.faire_tuile(t, active, propre), false, false, 0);
        }
        self.tuiles.show_all();
        // Les réponses lues ci-dessus ont pu faire entrer des événements dans
        // le tampon de x11rb : la prise ne redeviendra pas lisible pour eux.
        // On les vide ici, et on refait une fois de plus s'ils comptaient.
        if self.ecran.as_ref().map(|e| e.evenements()).unwrap_or(false) {
            let b = self.clone();
            glib::timeout_add_local_once(Duration::from_millis(60), move || b.refaire());
        }
    }

    fn faire_tuile(self: &Rc<Self>, t: Tuile, active: Option<u32>, propre: Option<gdk_pixbuf::Pixbuf>) -> gtk::Box {
        let boite = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let bouton = gtk::Button::new();
        match propre.map(|p| gtk::Image::from_pixbuf(Some(&p))).or_else(|| image_de(&[&t.icone, "application-x-executable"], self.icone())) {
            Some(i) => bouton.add(&i),
            None => bouton.add(&gtk::Label::new(Some(&t.nom.chars().take(8).collect::<String>()))),
        }
        bouton.set_tooltip_text(Some(&t.nom));
        boite.pack_start(&bouton, true, true, 0);
        let trait_ = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        classe(&trait_, "trait");
        boite.pack_start(&trait_, false, false, 0);

        // Le trait dit l'état : ouverte, au premier plan, réduite, fermée.
        if !t.fenetres.is_empty() {
            let au_premier_plan = active.map(|a| t.fenetres.iter().any(|f| f.xid == a)).unwrap_or(false);
            classe(
                &trait_,
                if au_premier_plan { "trait-active" } else if t.fenetres.iter().all(|f| f.reduite) { "trait-reduite" } else { "trait-ouverte" },
            );
            let n = t.fenetres.len();
            poser_infobulle(&bouton, &format!("{} — {} fenêtre{}", t.nom, n, if n > 1 { "s" } else { "" }));
        }

        let t = Rc::new(t);
        let b = self.clone();
        let tt = t.clone();
        bouton.connect_clicked(move |_| b.activer(&tt));

        // L'APERÇU AU SURVOL (19 novembre) : rester sur une application
        // ouverte montre ses fenêtres. Son infobulle se tait alors — deux
        // bulles l'une sur l'autre se gêneraient.
        if !t.fenetres.is_empty() {
            bouton.set_has_tooltip(false);
            let b = self.clone();
            let tt = t.clone();
            bouton.connect_enter_notify_event(move |w, _| {
                b.programmer_apercu(tt.clone(), w.clone());
                glib::Propagation::Proceed
            });
            // Quitter la tuile AVANT que la bulle s'ouvre annule son ouverture ;
            // une fois ouverte, c'est l'observation du pointeur qui la ferme.
            let b = self.clone();
            bouton.connect_leave_notify_event(move |_, _| {
                if b.apercu.borrow().is_none() {
                    b.annuler_minuteur_apercu();
                }
                glib::Propagation::Proceed
            });
        }
        let b = self.clone();
        bouton.connect_button_press_event(move |_, ev| {
            if ev.button() == 1 {
                b.appui.set(Some(std::time::Instant::now()));
            }
            if ev.button() != 3 {
                return glib::Propagation::Proceed;
            }
            b.menu_tuile(&t, ev);
            glib::Propagation::Stop
        });
        // Relâché (sur la tuile ou à côté) : la barre peut se refaire.
        let b = self.clone();
        bouton.connect_button_release_event(move |_, ev| {
            if ev.button() == 1 {
                b.appui.set(None);
            }
            glib::Propagation::Proceed
        });
        boite
    }

    fn annuler_minuteur_apercu(&self) {
        if let Some(id) = self.minuteur_apercu.borrow_mut().take() {
            id.remove();
        }
    }

    /// Ouvrir la bulle dans 450 ms, si le pointeur est resté sur la tuile.
    fn programmer_apercu(self: &Rc<Self>, t: Rc<Tuile>, bouton: gtk::Button) {
        self.annuler_minuteur_apercu();
        let b = self.clone();
        let id = glib::timeout_add_local_once(Duration::from_millis(450), move || {
            *b.minuteur_apercu.borrow_mut() = None;
            b.montrer_apercu(&t, &bouton);
        });
        *self.minuteur_apercu.borrow_mut() = Some(id);
    }

    /// LA BULLE BLOQUÉE (23 novembre, capture de Grenofar : l'aperçu de
    /// Claude restait au-dessus du dock, pointeur parti depuis longtemps).
    /// `close()` ne ferme pas : il envoie une demande de fermeture, et GTK
    /// l'IGNORE en silence tant qu'une autre fenêtre tient une attrape GTK —
    /// un menu contextuel ouvert (« Retirer de la barre »), un glisser en
    /// cours. La bulle était déjà oubliée ici, plus rien ne la fermait. On la
    /// cache donc tout de suite, et on la détruit.
    fn fermer_apercu(&self) {
        self.survol_apercu.set(false);
        if let Some(f) = self.apercu.borrow_mut().take() {
            f.hide();
            // SAFETY : plus aucune référence à cette bulle n'est gardée (son
            // guet compare un clone, qui voit seulement qu'elle a changé).
            unsafe { f.destroy() };
        }
    }

    /// La bulle : une carte par fenêtre — son titre, un ✕ qui la ferme, et
    /// son image aux coins arrondis. Un clic sur la carte l'affiche.
    fn montrer_apercu(self: &Rc<Self>, t: &Rc<Tuile>, bouton: &gtk::Button) {
        self.fermer_apercu();
        let Some(ecran) = &self.ecran else { return };
        const L: i32 = 220;
        const H: i32 = 132;

        let fenetre = gtk::Window::new(gtk::WindowType::Popup);
        fenetre.set_widget_name("apercu");
        fenetre.set_type_hint(gdk::WindowTypeHint::Tooltip);
        fenetre.add_events(gdk::EventMask::ENTER_NOTIFY_MASK | gdk::EventMask::LEAVE_NOTIFY_MASK);
        let rangee = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        rangee.set_margin_top(6);
        rangee.set_margin_bottom(6);
        rangee.set_margin_start(6);
        rangee.set_margin_end(6);
        fenetre.add(&rangee);

        for f in t.fenetres.iter().take(6) {
            let carte = gtk::Box::new(gtk::Orientation::Vertical, 6);
            classe(&carte, "apercu-carte");

            let tete = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let titre = gtk::Label::new(Some(if f.titre.is_empty() { &t.nom } else { &f.titre }));
            classe(&titre, "apercu-titre");
            titre.set_ellipsize(pango::EllipsizeMode::End);
            titre.set_max_width_chars(24);
            titre.set_xalign(0.0);
            tete.pack_start(&titre, true, true, 0);
            let fermer = gtk::Button::with_label("✕");
            classe(&fermer, "apercu-fermer");
            poser_infobulle(&fermer, "Fermer cette fenêtre");
            let b = self.clone();
            let xid = f.xid;
            fermer.connect_clicked(move |_| {
                if let Some(e) = &b.ecran {
                    e.fermer(xid, temps());
                }
                b.fermer_apercu();
            });
            tete.pack_end(&fermer, false, false, 0);
            carte.pack_start(&tete, false, false, 0);

            // L'image : celle de la fenêtre, ou son icône si elle est réduite
            // (une fenêtre cachée n'a rien à l'écran).
            let image = if f.reduite { None } else { ecran.apercu(f.xid, (L * 2) as u32) }.map(|(rgba, l, h)| {
                gdk_pixbuf::Pixbuf::from_bytes(
                    &glib::Bytes::from_owned(rgba), gdk_pixbuf::Colorspace::Rgb, true, 8, l as i32, h as i32, (l * 4) as i32,
                )
            });
            let icone = natif::pixbuf_de(&[&t.icone, "application-x-executable"], 48);
            let zone = gtk::DrawingArea::new();
            zone.set_size_request(L, H);
            zone.connect_draw(move |z, cr| {
                let (w, h) = (z.allocated_width() as f64, z.allocated_height() as f64);
                // LES COINS : on découpe NOUS-MÊMES l'image en arrondi, sur le
                // fond de la bulle. Rien hors du chemin n'est peint — pas de
                // carré noir aux angles (la demande du 19 novembre).
                chemin_arrondi(cr, 0.0, 0.0, w, h, 10.0);
                cr.clip();
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.05);
                let _ = cr.paint();
                if let Some(pb) = &image {
                    // « Couvrir » : l'image remplit le cadre, centrée.
                    let (pw, ph) = (pb.width() as f64, pb.height() as f64);
                    let echelle = (w / pw).max(h / ph);
                    let (sw, sh) = ((pw * echelle).ceil() as i32, (ph * echelle).ceil() as i32);
                    if let Some(petite) = pb.scale_simple(sw.max(1), sh.max(1), gdk_pixbuf::InterpType::Bilinear) {
                        cr.set_source_pixbuf(&petite, (w - sw as f64) / 2.0, (h - sh as f64) / 2.0);
                        let _ = cr.paint();
                    }
                } else if let Some(ic) = &icone {
                    cr.set_source_pixbuf(ic, (w - ic.width() as f64) / 2.0, (h - ic.height() as f64) / 2.0);
                    let _ = cr.paint();
                }
                glib::Propagation::Stop
            });
            carte.pack_start(&zone, false, false, 0);

            // Un clic sur la carte (hors du ✕) affiche cette fenêtre.
            let boite = gtk::EventBox::new();
            boite.add(&carte);
            let b = self.clone();
            boite.connect_button_release_event(move |_, ev| {
                if ev.button() == 1 {
                    if let Some(e) = &b.ecran {
                        e.activer(xid, temps());
                    }
                    b.fermer_apercu();
                }
                glib::Propagation::Stop
            });
            rangee.pack_start(&boite, false, false, 0);
        }

        // LA BULLE SE FERME D'ELLE-MÊME (19 novembre). Grenofar : « il reste
        // même quand il n'est pas sélectionné et quand il n'y a plus l'app ».
        // Les événements « entrer / sortir » se perdaient (une fenêtre qui
        // apparaît sous le pointeur trouble ces événements) : on REGARDE où
        // est le pointeur, toutes les 120 ms. Hors de la bulle ET de sa tuile
        // deux fois de suite → fermée. Tuile retirée de la barre (application
        // fermée), ou fenêtre de la bulle disparue → fermée aussi.
        let xids: Vec<u32> = t.fenetres.iter().map(|f| f.xid).collect();
        let b = self.clone();
        let bulle = fenetre.clone();
        let tuile = bouton.clone();
        let absences = std::rc::Rc::new(std::cell::Cell::new(0u8));
        glib::timeout_add_local(Duration::from_millis(120), move || {
            let ouverte = b.apercu.borrow().as_ref().map(|f| f == &bulle).unwrap_or(false);
            if !ouverte {
                return glib::ControlFlow::Break;
            }
            // L'application n'est plus là : sa tuile a quitté la barre, ou
            // aucune de ses fenêtres n'est encore ouverte.
            let encore = b.ecran.as_ref().map(|e| e.fenetres().iter().any(|f| xids.contains(&f.xid))).unwrap_or(false);
            if tuile.parent().is_none() || !encore {
                b.fermer_apercu();
                return glib::ControlFlow::Break;
            }
            let pointeur = gdk::Display::default()
                .and_then(|d| d.default_seat())
                .and_then(|s| s.pointer())
                .map(|p| { let (_, x, y) = p.position(); (x, y) });
            let Some((x, y)) = pointeur else { return glib::ControlFlow::Continue };
            let dans = |(ox, oy): (i32, i32), (l, h): (i32, i32)| x >= ox && x < ox + l && y >= oy && y < oy + h;
            let sur_bulle = dans(bulle.position(), bulle.size());
            let zone = tuile.allocation();
            let (px, py) = b.panneau.position();
            // La tuile, et le petit couloir entre elle et la bulle.
            let sur_tuile = dans((px + zone.x(), py + zone.y() - 14), (zone.width(), zone.height() + 14));
            b.survol_apercu.set(sur_bulle);
            if sur_bulle || sur_tuile {
                absences.set(0);
                return glib::ControlFlow::Continue;
            }
            absences.set(absences.get() + 1);
            if absences.get() >= 2 {
                b.fermer_apercu();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });

        // Au-dessus de la tuile, centrée sur elle, sans sortir de l'écran.
        let zone_tuile = bouton.allocation();
        let (px, py) = self.panneau.position();
        let centre = px + zone_tuile.x() + zone_tuile.width() / 2;
        let (bord_g, largeur_ecran) = (self.ox, self.largeur);
        fenetre.connect_size_allocate(move |f, _| {
            let (l, h) = f.size();
            let x = (centre - l / 2).clamp(bord_g + 8, (bord_g + largeur_ecran - l - 8).max(bord_g + 8));
            let y = (py - h - 10).max(8);
            if f.position() != (x, y) {
                f.move_(x, y);
            }
            natif::decouper_maintenant(f);
        });
        natif::arrondir(&fenetre);
        fenetre.show_all();
        // Les coins : découpés tout de suite, sans attendre un « map » qu'une
        // popup ne reçoit pas toujours.
        natif::decouper_maintenant(&fenetre);
        *self.apercu.borrow_mut() = Some(fenetre);
    }

    fn activer(&self, t: &Tuile) {
        let Some(ecran) = &self.ecran else { return };
        if t.fenetres.is_empty() {
            lancer(&t.commande, false);
            return;
        }
        // La liste vivante, pas celle du moment où la tuile a été faite :
        // une fenêtre fermée depuis ne répondrait plus.
        let vivantes: Vec<u32> = ecran.fenetres().iter().map(|f| f.xid).collect();
        let miennes: Vec<u32> = t.fenetres.iter().map(|f| f.xid).filter(|x| vivantes.contains(x)).collect();
        if miennes.is_empty() {
            lancer(&t.commande, false);
            return;
        }
        let active = ecran.active();
        if active.map(|a| miennes.contains(&a)).unwrap_or(false) {
            for x in &miennes {
                ecran.reduire(*x);
            }
            return;
        }
        // La DERNIÈRE utilisée, comme Windows — la plus haute dans l'ordre
        // d'empilement —, et pas la première ouverte : la première de Steam
        // ou d'un navigateur est souvent une fenêtre secondaire.
        let pile = ecran.empilement();
        let choisie = pile.iter().rev().find(|x| miennes.contains(x)).copied().unwrap_or(miennes[0]);
        ecran.activer(choisie, temps());
        dire(&format!("tuile : {} affichee ({} fenetre(s))", t.nom, miennes.len()));
    }

    fn menu_tuile(self: &Rc<Self>, t: &Rc<Tuile>, ev: &gdk::EventButton) {
        let mut gestes: Vec<(&'static str, Box<dyn Fn()>)> = Vec::new();
        if !t.fenetres.is_empty() {
            let (b, tt) = (self.clone(), t.clone());
            gestes.push(("Afficher", Box::new(move || {
                if let Some(e) = &b.ecran { e.activer(tt.fenetres[0].xid, temps()) }
            })));
            let (b, tt) = (self.clone(), t.clone());
            gestes.push(("Réduire", Box::new(move || {
                if let Some(e) = &b.ecran { tt.fenetres.iter().for_each(|f| e.reduire(f.xid)) }
            })));
        } else {
            let tt = t.clone();
            gestes.push(("Ouvrir", Box::new(move || lancer(&tt.commande, false))));
        }
        let (b, tt) = (self.clone(), t.clone());
        if t.epinglee {
            gestes.push(("Retirer de la barre", Box::new(move || b.retirer(&tt.commande))));
        } else {
            gestes.push(("Épingler à la barre", Box::new(move || b.epingler(&tt.nom, &tt.commande, &tt.icone))));
        }
        if let Some(g) = self.application_de_commande(&t.commande).and_then(|a| self.geste_desinstaller(&a)) {
            gestes.push(g);
        }
        if !t.fenetres.is_empty() {
            let (b, tt) = (self.clone(), t.clone());
            gestes.push(("Fermer", Box::new(move || {
                if let Some(e) = &b.ecran { tt.fenetres.iter().for_each(|f| e.fermer(f.xid, temps())) }
            })));
        }
        *self.menu_contextuel.borrow_mut() = Some(menu_de(gestes, ev));
    }

    fn epingler(self: &Rc<Self>, nom: &str, commande: &str, icone: &str) {
        if commande.is_empty() || self.epingles.borrow().iter().any(|e| e.commande == commande) {
            return;
        }
        self.epingles.borrow_mut().push(Epingle { nom: nom.into(), commande: commande.into(), icone: icone.into() });
        apps::ecrire_epingles(&self.epingles.borrow());
        self.refaire();
    }

    fn retirer(self: &Rc<Self>, commande: &str) {
        self.epingles.borrow_mut().retain(|e| e.commande != commande);
        apps::ecrire_epingles(&self.epingles.borrow());
        self.refaire();
    }

    // ---- Désinstaller (26 novembre) -------------------------------------------
    // Grenofar : « fais qu'on puisse désinstaller des apps, depuis paramètres
    // ou clic droit sur le dock quand on appuie sur Windows ». Le clic droit
    // d'une application du menu (liste ou épinglée) et du dock propose
    // « Désinstaller… », qui ouvre Réglages › Applications sur elle, sa
    // confirmation dépliée : rien ne part sans ce second clic, et ce qui fait
    // grenOS n'est même pas proposé (apps::desinstallation).

    fn application_de_commande(&self, commande: &str) -> Option<Application> {
        if commande.is_empty() {
            return None;
        }
        self.applications.borrow().iter().find(|a| a.commande == commande).cloned()
    }

    fn geste_desinstaller(self: &Rc<Self>, a: &Application) -> Option<(&'static str, Box<dyn Fn()>)> {
        let commande = apps::desinstallation(a)?;
        let (b, nom) = (self.clone(), a.nom.clone());
        let action: Box<dyn Fn()> = Box::new(move || {
            if b.menu.is_visible() {
                b.fermer_menu();
            }
            dire(&format!("desinstaller : {nom}, ouvert dans les reglages"));
            lancer(&commande, false);
        });
        Some(("Désinstaller…", action))
    }

    /// Relit les applications ; celles qui ont disparu (désinstallées, d'ici
    /// ou d'ailleurs) quittent les épinglées, comme sous Windows.
    fn relire_applications(self: &Rc<Self>) {
        let nouvelles = apps::applications();
        let parties = apps::commandes_parties(&self.applications.borrow(), &nouvelles);
        *self.applications.borrow_mut() = nouvelles;
        if parties.is_empty() {
            return;
        }
        let avant = self.epingles.borrow().len();
        self.epingles.borrow_mut().retain(|e| !parties.contains(&e.commande));
        let retirees = avant - self.epingles.borrow().len();
        if retirees > 0 {
            apps::ecrire_epingles(&self.epingles.borrow());
            dire(&format!("desinstaller : {retirees} epinglee(s) retiree(s), leur application est partie"));
        }
        // Le menu, s'il est ouvert, se refait au prochain passage ; la barre, tout de suite.
        let b = self.clone();
        glib::idle_add_local_once(move || b.refaire());
    }

    /// Une installation ou une désinstallation change l'heure d'un dossier
    /// d'applications : on relit alors, quatre secondes plus tard (le temps
    /// que Flatpak ou dpkg aient fini d'écrire).
    fn surveiller_applications(self: &Rc<Self>) {
        let derniere = RefCell::new(apps::empreinte_dossiers());
        let b = self.clone();
        glib::timeout_add_seconds_local(10, move || {
            let e = apps::empreinte_dossiers();
            if *derniere.borrow() != e {
                *derniere.borrow_mut() = e;
                let b = b.clone();
                glib::timeout_add_seconds_local_once(4, move || b.relire_applications());
            }
            glib::ControlFlow::Continue
        });
    }

    // ---- Ce qui tourne (« ^ ») ---------------------------------------------------
    /// Le menu de la flèche : une entrée par application qui tourne, avec un
    /// sous-menu — Afficher, Fermer, Forcer l'arrêt. Un gtk::Menu, pas une
    /// bulle : il prend lui-même le pointeur et se ferme d'un clic à côté.
    fn ouvrir_coulisses(self: &Rc<Self>, bouton: &gtk::Button) {
        let applis = self.applications.borrow().clone();
        let ouvertes = self.ecran.as_ref().map(|e| e.fenetres()).unwrap_or_default();
        let menu = gtk::Menu::new();
        classe(&menu, "coulisses");
        let titre = gtk::MenuItem::with_label("Applications en cours");
        titre.set_sensitive(false);
        menu.append(&titre);
        menu.append(&gtk::SeparatorMenuItem::new());

        // Les fenêtres, groupées par application.
        let mut groupes: Vec<(String, Vec<Fenetre>)> = Vec::new();
        for f in ouvertes {
            let g = apps::groupe_de(&f.classe, &f.titre);
            match groupes.iter_mut().find(|(x, _)| *x == g) {
                Some((_, v)) => v.push(f),
                None => groupes.push((g, vec![f])),
            }
        }
        let mut vues = std::collections::HashSet::new();
        let mut compte = 0;
        for (groupe, fenetres) in groupes {
            let app = apps::application_de_fenetre(&groupe, &fenetres[0].titre, &applis);
            if let Some(a) = app {
                vues.insert(identifiant_flatpak(a));
            }
            let jeu = apps::est_un_jeu(&groupe);
            let nom = apps::nom_de_fenetre(&groupe, &fenetres[0].titre, app);
            let icone = app.map(|a| a.icone.clone()).unwrap_or_default();
            let etiquette = if fenetres.len() > 1 { format!("{nom} ({} fenêtres)", fenetres.len()) } else { nom.clone() };
            let sous = gtk::Menu::new();
            for f in fenetres.iter().take(8) {
                let texte = if f.titre.is_empty() { format!("Afficher {nom}") } else { format!("Afficher : {}", couper(&f.titre, 48)) };
                let (b, xid) = (self.clone(), f.xid);
                ajouter(&sous, &texte, move || if let Some(e) = &b.ecran { e.activer(xid, temps()) });
            }
            sous.append(&gtk::SeparatorMenuItem::new());
            // « Fermer » quitte l'application entière (24 novembre) : ses
            // fenêtres d'abord, puis tout son arbre de processus — sinon
            // Steam, Discord… restaient dans leur coin (natif::processus).
            let xids = fenetres.iter().map(|f| f.xid).collect::<Vec<_>>();
            // Un jeu : ses processus à lui (SteamAppId), jamais son lanceur —
            // Heroic ou Steam restent ouverts.
            // Une fenêtre de grenOS : ses fenêtres seulement. Son processus,
            // grenos-ui, porte TOUTES les autres (Réglages, Explorateur…) :
            // le quitter les fermerait toutes (26 novembre).
            let partage = apps::de_grenos_ui(&fenetres[0].classe);
            let flatpak = if jeu || partage { String::new() } else { app.map(identifiant_flatpak).unwrap_or_default() };
            let programme = if jeu || partage { String::new() } else { app.map(|a| apps::programme_de(&a.commande)).unwrap_or_default() };
            // Une Flatpak : `flatpak kill` suffit, et le pid de ses fenêtres
            // est celui du bac à sable — on ne s'en sert pas.
            let mut pids: Vec<u32> = match (&self.ecran, flatpak.is_empty()) {
                _ if partage => Vec::new(),
                _ if jeu => processus::du_jeu(&groupe),
                (Some(e), true) => xids
                    .iter()
                    .filter_map(|x| e.pid(*x))
                    .filter(|p| processus::est_de(*p, &[groupe.as_str(), programme.as_str()]))
                    .collect(),
                _ => Vec::new(),
            };
            pids.sort();
            pids.dedup();
            let (b, p, fp, prog) = (self.clone(), pids.clone(), flatpak.clone(), programme.clone());
            ajouter(&sous, "Fermer", move || {
                if let Some(e) = &b.ecran {
                    xids.iter().for_each(|x| e.fermer(*x, temps()));
                }
                if p.is_empty() && fp.is_empty() {
                    // Aucune fenêtre ne dit son processus : par son nom.
                    let prog = prog.clone();
                    glib::timeout_add_local_once(Duration::from_millis(1500), move || forcer_arret(&prog, ""));
                } else {
                    processus::quitter(p.clone(), fp.clone(), Duration::from_millis(1500), false);
                }
            });
            if !pids.is_empty() || !flatpak.is_empty() || !programme.is_empty() {
                ajouter(&sous, "Forcer l'arrêt", move || {
                    if pids.is_empty() && flatpak.is_empty() {
                        forcer_arret(&programme, "");
                    } else {
                        processus::quitter(pids.clone(), flatpak.clone(), Duration::ZERO, true);
                    }
                });
            }
            menu.append(&entree(&nom_affiche(&etiquette), &icone, Some(&sous)));
            compte += 1;
        }

        // Les applications Flatpak qui tournent SANS fenêtre (Steam réduit
        // dans un coin, un lanceur qui télécharge) : sans ce menu, rien ne
        // les montrait, et rien ne permettait de les fermer.
        let mut en_fond = Vec::new();
        for id in flatpaks_en_cours() {
            if vues.contains(&id) || en_fond.contains(&id) {
                continue;
            }
            en_fond.push(id.clone());
            let app = applis.iter().find(|a| identifiant_flatpak(a) == id);
            let nom = app.map(|a| a.nom.clone()).unwrap_or_else(|| id.rsplit('.').next().unwrap_or(id.as_str()).to_string());
            let icone = app.map(|a| a.icone.clone()).unwrap_or_else(|| id.clone());
            let sous = gtk::Menu::new();
            if let Some(a) = app {
                let commande = a.commande.clone();
                ajouter(&sous, "Afficher", move || lancer(&commande, false));
            }
            let i = id.clone();
            ajouter(&sous, "Fermer", move || {
                let _ = std::process::Command::new("flatpak").args(["kill", i.as_str()]).status();
            });
            menu.append(&entree(&format!("{nom} — en arrière-plan"), &icone, Some(&sous)));
            compte += 1;
        }

        if compte == 0 {
            let vide = gtk::MenuItem::with_label("Rien ne tourne pour l'instant");
            vide.set_sensitive(false);
            menu.append(&vide);
        }
        menu.append(&gtk::SeparatorMenuItem::new());
        ajouter(&menu, "Gestionnaire des tâches…", || lancer("grenos-taches", false));
        menu.show_all();
        menu.popup_at_widget(bouton, gdk::Gravity::North, gdk::Gravity::South, None);
        dire(&format!("coulisses : {compte} application(s) en cours, dont {} en fond", en_fond.len()));
        *self.menu_coulisses.borrow_mut() = Some(menu);
    }

    // ---- Le menu ---------------------------------------------------------------
    fn enfoncer(&self, etat: bool) {
        if let Some(id) = self.poignee.borrow().as_ref() {
            self.bouton_menu.block_signal(id);
            self.bouton_menu.set_active(etat);
            self.bouton_menu.unblock_signal(id);
        }
    }

    fn ouvrir_menu(self: &Rc<Self>) {
        let (l, h) = taille_menu();
        self.menu.set_size_request(l, h);
        let (mx, my) = (((self.largeur - l) / 2).max(8), (self.hauteur_ecran - self.bande() - h - 8).max(8));
        self.menu.move_(self.ox + mx, self.oy + my);
        self.relire_applications();
        self.recherche.set_text("");
        self.remplir();
        self.menu.show_all();
        self.menu.present();
        self.recherche.grab_focus();
        self.enfoncer(true);
        if let Some(a) = self.attrape.borrow_mut().take() {
            a.lacher();
        }
        *self.attrape.borrow_mut() = Some(natif::Attrape::prendre(&self.menu));
        dire(&format!("menu ouvert, {} applications", self.applications.borrow().len()));
        // Où est « Éteindre » : la CI clique dessus pour prouver que la fenêtre
        // Fin de session s'ouvre (arret.rs). Mesuré une fois le menu dessiné —
        // avant, l'allocation vaut zéro et le clic tomberait à côté.
        let b = self.clone();
        glib::timeout_add_local_once(Duration::from_millis(400), move || {
            if let Some(bouton) = b.bouton_eteindre.borrow().as_ref() {
                if let Some((x, y)) = bouton.translate_coordinates(&b.menu, 0, 0) {
                    let zone = bouton.allocation();
                    dire(&format!("bouton eteindre en {},{}", mx + x + zone.width() / 2, my + y + zone.height() / 2));
                }
            }
        });
    }

    fn fermer_menu(&self) {
        if let Some(a) = self.attrape.borrow_mut().take() {
            a.lacher();
        }
        self.menu.hide();
        self.enfoncer(false);
        dire("menu ferme");
    }

    /// Le menu se ferme au début d'un glisser — sans relâcher le pointeur,
    /// que GTK tient désormais pour le glisser (voir `Attrape::ceder`).
    fn fermer_menu_pour_glisser(&self) {
        if let Some(a) = self.attrape.borrow_mut().take() {
            a.ceder();
        }
        self.menu.hide();
        self.enfoncer(false);
        dire("menu ferme pour un glisser");
    }

    /// Une application lâchée sur le dock y est épinglée (23 novembre :
    /// « déplacer un raccourci d'app vers le bureau ou dock est buggé »).
    /// Elle vient du menu, ou d'un raccourci .desktop d'ailleurs.
    fn recevoir_sur_le_dock(self: &Rc<Self>, donnees: &gtk::SelectionData) {
        for adresse in donnees.uris() {
            let Ok((chemin, _)) = glib::filename_from_uri(&adresse) else { continue };
            if chemin.extension().map(|e| e != "desktop").unwrap_or(true) {
                continue;
            }
            if let Some(a) = apps::lire_desktop(&chemin) {
                dire(&format!("epinglee par glisser : {}", a.nom));
                self.epingler(&a.nom, &a.commande, &a.icone);
            }
        }
    }

    fn remplir(self: &Rc<Self>) {
        for enfant in self.contenu.children() {
            self.contenu.remove(&enfant);
        }
        let filtre = self.recherche.text().to_string();
        let cherche = !filtre.trim().is_empty();
        if !cherche {
            self.contenu.pack_start(&section("ÉPINGLÉES"), false, false, 0);
            let grille = gtk::FlowBox::new();
            grille.set_selection_mode(gtk::SelectionMode::None);
            grille.set_max_children_per_line(6);
            grille.set_min_children_per_line(4);
            grille.set_homogeneous(true);
            grille.set_margin_start(10);
            grille.set_margin_end(10);
            for e in self.epingles.borrow().iter() {
                grille.add(&self.carreau(e));
            }
            self.contenu.pack_start(&grille, false, false, 0);
        }
        self.contenu.pack_start(&section(if cherche { "RÉSULTATS" } else { "TOUTES LES APPLICATIONS" }), false, false, 0);
        let liste = self.applications.borrow().clone();
        for a in apps::visibles(&liste, &filtre).into_iter().take(80) {
            self.contenu.pack_start(&self.ligne(a.clone()), false, false, 0);
        }
        // Les pages des Réglages qui répondent au mot tapé (23 novembre) :
        // « partition », « hz », « navigateur »… comme la recherche de Windows.
        let reglages = if cherche { apps::reglages_trouves(&filtre) } else { Vec::new() };
        if !reglages.is_empty() {
            self.contenu.pack_start(&section("RÉGLAGES"), false, false, 0);
            for r in reglages {
                self.contenu.pack_start(&self.ligne_reglage(r), false, false, 0);
            }
        }
        self.contenu.show_all();
    }

    fn carreau(self: &Rc<Self>, e: &Epingle) -> gtk::Button {
        let bouton = gtk::Button::new();
        classe(&bouton, "epingle");
        let colonne = gtk::Box::new(gtk::Orientation::Vertical, 6);
        if let Some(i) = image_de(&[&e.icone], 36) {
            colonne.pack_start(&i, false, false, 0);
        }
        let l = gtk::Label::new(Some(&e.nom));
        l.set_max_width_chars(11);
        l.set_ellipsize(pango::EllipsizeMode::End);
        colonne.pack_start(&l, false, false, 0);
        bouton.add(&colonne);
        let (b, commande) = (self.clone(), e.commande.clone());
        bouton.connect_clicked(move |_| {
            b.fermer_menu();
            lancer(&commande, false);
        });
        let (b, commande) = (self.clone(), e.commande.clone());
        bouton.connect_button_press_event(move |_, ev| {
            if ev.button() != 3 {
                return glib::Propagation::Proceed;
            }
            let (b1, c1) = (b.clone(), commande.clone());
            let mut gestes: Vec<(&'static str, Box<dyn Fn()>)> = vec![("Désépingler", Box::new(move || {
                b1.retirer(&c1);
                b1.remplir();
            }))];
            if let Some(g) = b.application_de_commande(&commande).and_then(|a| b.geste_desinstaller(&a)) {
                gestes.push(g);
            }
            *b.menu_contextuel.borrow_mut() = Some(menu_de(gestes, ev));
            glib::Propagation::Stop
        });
        bouton
    }

    fn ligne_reglage(self: &Rc<Self>, r: &'static apps::Reglage) -> gtk::Button {
        let bouton = gtk::Button::new();
        classe(&bouton, "entree");
        let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        if let Some(i) = image_de(&[r.icone, "preferences-system", "grenos-parametres"], 24) {
            ligne.pack_start(&i, false, false, 0);
        }
        let l = gtk::Label::new(Some(&format!("Réglages › {}", r.nom)));
        l.set_xalign(0.0);
        l.set_ellipsize(pango::EllipsizeMode::End);
        ligne.pack_start(&l, true, true, 0);
        bouton.add(&ligne);
        let b = self.clone();
        bouton.connect_clicked(move |_| {
            b.fermer_menu();
            dire(&format!("menu : reglage ouvert depuis la recherche ({})", r.page));
            lancer(&r.commande(), false);
        });
        bouton
    }

    fn ligne(self: &Rc<Self>, a: Application) -> gtk::Button {
        let bouton = gtk::Button::new();
        classe(&bouton, "entree");
        let ligne = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        if let Some(i) = image_de(&[&a.icone], 24) {
            ligne.pack_start(&i, false, false, 0);
        }
        let l = gtk::Label::new(Some(&a.nom));
        l.set_xalign(0.0);
        ligne.pack_start(&l, true, true, 0);
        bouton.add(&ligne);
        let a = Rc::new(a);
        let (b, aa) = (self.clone(), a.clone());
        bouton.connect_clicked(move |_| b.choisir(&aa));
        let (b, aa) = (self.clone(), a.clone());
        bouton.connect_button_press_event(move |_, ev| {
            if ev.button() != 3 {
                return glib::Propagation::Proceed;
            }
            let (b1, a1) = (b.clone(), aa.clone());
            let a2 = aa.clone();
            let mut gestes: Vec<(&'static str, Box<dyn Fn()>)> = vec![
                ("Épingler à la barre des tâches", Box::new(move || {
                    b1.epingler(&a1.nom, &a1.commande, &a1.icone);
                    b1.remplir();
                })),
                ("Ajouter au bureau", Box::new(move || poser_sur_le_bureau(&a2))),
            ];
            if let Some(g) = b.geste_desinstaller(&aa) {
                gestes.push(g);
            }
            *b.menu_contextuel.borrow_mut() = Some(menu_de(gestes, ev));
            glib::Propagation::Stop
        });
        // Glisser une application vers le bureau l'y pose ; vers le dock,
        // l'y épingle. L'icône qui suit le pointeur est celle de l'application
        // (l'icône par défaut de GTK, sans compositeur, faisait un carré).
        bouton.drag_source_set(gdk::ModifierType::BUTTON1_MASK, &[], gdk::DragAction::COPY);
        bouton.drag_source_add_uri_targets();
        if let Some(p) = natif::pixbuf_de(&[&a.icone], 48) {
            bouton.drag_source_set_icon_pixbuf(&p);
        }
        let aa = a.clone();
        bouton.connect_drag_data_get(move |_, _, donnees, _, _| {
            if let Ok(uri) = glib::filename_to_uri(&aa.fichier, None) {
                donnees.set_uris(&[uri.as_str()]);
            }
        });
        let b = self.clone();
        bouton.connect_drag_begin(move |_, _| b.fermer_menu_pour_glisser());
        bouton
    }

    fn choisir(&self, a: &Application) {
        self.fermer_menu();
        lancer(&a.commande, a.terminal);
    }

    // ---- Le reste --------------------------------------------------------------
    fn tic(&self) {
        let m = glib::DateTime::now_local();
        if let Ok(m) = m {
            let jour = JOURS[(m.day_of_week() as usize).clamp(1, 7) - 1];
            let mois = MOIS[(m.month() as usize).clamp(1, 12) - 1];
            let texte = format!("{:02}:{:02}\n{} {} {}", m.hour(), m.minute(), jour, m.day_of_month(), mois);
            if self.horloge.text() != texte {
                self.horloge.set_text(&texte);
            }
        }
    }

    /// Ce que la vérification quotidienne a trouvé, et ce qu'une mise à jour
    /// au démarrage a donné : un échec se DIT, la machine ouvre quand même.
    fn relire_maj(&self) {
        let resultat = std::fs::read_to_string("/run/grenos/maj-resultat").unwrap_or_default();
        if resultat.split_whitespace().next() == Some("echec") {
            self.bouton_maj.set_label("mise à jour échouée");
            poser_infobulle(&self.bouton_maj, "La dernière mise à jour au démarrage ne s'est pas faite. Cliquer pour réessayer et voir pourquoi.");
            self.bouton_maj.show();
            return;
        }
        let nombre = std::fs::read_to_string("/run/grenos/maj").ok().and_then(|t| t.trim().parse::<u32>().ok());
        match nombre {
            Some(n) if n > 0 => {
                self.bouton_maj.set_label(&format!("{n} mise{} à jour", if n > 1 { "s" } else { "" }));
                poser_infobulle(&self.bouton_maj, "Cliquer pour installer les mises à jour de grenOS et de Debian, ou choisir « Mettre à jour et redémarrer » dans l'écran d'arrêt.");
                self.bouton_maj.show();
            }
            _ => self.bouton_maj.hide(),
        }
    }

    /// Le réseau, lu dans /sys et /proc : jamais de `nmcli` dans une minuterie.
    fn relire_reseau(&self) {
        let (nom, phrase) = icone_reseau();
        self.image_reseau.set_from_icon_name(Some(&nom), gtk::IconSize::LargeToolbar);
        poser_infobulle(&self.bouton_reseau, &format!("{phrase} — cliquer pour les connexions"));
    }

    /// Le son : `pactl` peut mettre des secondes à répondre quand PipeWire
    /// démarre. Il tourne donc dans un fil à part, qui envoie le résultat à
    /// la boucle GTK — l'horloge ne se fige jamais.
    fn surveiller_son(self: &Rc<Self>) {
        #[allow(deprecated)]
        let (envoi, reception) = glib::MainContext::channel::<(u32, bool)>(glib::Priority::DEFAULT);
        std::thread::spawn(move || loop {
            let volume = pactl(&["get-sink-volume", "@DEFAULT_SINK@"]);
            let muet = pactl(&["get-sink-mute", "@DEFAULT_SINK@"]);
            if let (Some(v), Some(m)) = (volume, muet) {
                let pourcent = v.split('%').next().and_then(|d| d.rsplit(|c: char| !c.is_ascii_digit()).next())
                    .and_then(|n| n.parse().ok()).unwrap_or(0);
                if envoi.send((pourcent, m.contains("yes"))).is_err() {
                    return;
                }
            }
            std::thread::sleep(Duration::from_secs(3));
        });
        let b = self.clone();
        reception.attach(None, move |(volume, muet)| {
            let nom = if muet { "grenos-son-muet" } else if volume < 45 { "grenos-son-faible" } else { "grenos-son" };
            b.image_son.set_from_icon_name(Some(nom), gtk::IconSize::LargeToolbar);
            poser_infobulle(&b.bouton_son, &if muet { "Son coupé".to_string() } else { format!("Son : {volume} %") });
            glib::ControlFlow::Continue
        });
        // Une première icône tout de suite, avant la première réponse.
        self.image_son.set_from_icon_name(Some("grenos-son"), gtk::IconSize::LargeToolbar);
    }

    /// Les deux positions que la CI vise : le menu et le son.
    fn dire_les_boutons(&self) {
        for (nom, bouton) in [("grenos", self.bouton_menu.clone().upcast::<gtk::Widget>()), ("son", self.bouton_son.clone().upcast())] {
            let zone = bouton.allocation();
            let x = zone.x() + zone.width() / 2;
            // Dans le dock, la barre n'est ni à x = 0 ni collée au bord : on
            // part de là où elle est vraiment.
            let (px, py) = if self.dock {
                self.panneau.position()
            } else {
                (self.ox + MARGE_DOCK, self.oy + self.hauteur_ecran - HAUTEUR - MARGE_DOCK)
            };
            let (x, y) = (px + x, py + zone.y() + zone.height() / 2);
            dire(&format!("bouton {nom} en {x},{y}"));
        }
    }
}

/// Un rectangle aux coins arrondis, en chemin cairo (pour découper).
// ---- Les dispositions en glissant une fenêtre (20 novembre) -----------------
//
// Grenofar, avec une capture de Windows 11 : en haut, pendant qu'on fait
// glisser une fenêtre, une barre arrondie presque aussi large que l'écran
// propose des dispositions ; lâcher la fenêtre sur une case l'y range.
// Désactivable dans Réglages › Apparence (~/.config/grenos/dispositions).
//
// COMMENT ON SAIT QU'UNE FENÊTRE GLISSE. Openbox déplace la fenêtre en tenant
// le pointeur (une « attrape ») : aucune autre fenêtre ne reçoit d'événement
// pendant ce temps. On REGARDE donc, toutes les 40 ms : le bouton gauche est-il
// tenu, et la fenêtre active a-t-elle changé de place sans changer de taille ?
// C'est une question au serveur X (XQueryPointer), rien de plus. Le même
// moyen que la bulle d'aperçus, et pour la même raison.

/// Ce qu'on sait du glissement en cours.
struct Glisse {
    /// Faux : réglage coupé, ou rien à ranger — on attend juste le lâcher.
    actif: bool,
    xid: u32,
    depart: (i32, i32),
    taille: (u32, u32),
    /// La fenêtre a vraiment bougé : la barre est montrée.
    bouge: bool,
    ecran: (i32, i32, i32, i32),
    deplie: bool,
    survol: Option<(usize, usize)>,
}

/// La barre repliée (un trait arrondi), puis déployée quand on s'approche.
const DISPO_REPLIEE: i32 = 14;
const DISPO_DEPLIEE: i32 = 108;
/// À moins de tant de pixels du haut de l'écran, la barre se déploie.
const DISPO_ZONE_HAUTE: i32 = 96;
const DISPO_AIR: i32 = 8;

fn pointeur_et_boutons() -> Option<(i32, i32, gdk::ModifierType)> {
    let d = gdk::Display::default()?;
    let p = d.default_seat()?.pointer()?;
    let racine = gdk::Screen::default()?.root_window()?;
    let (_, x, y, masque) = racine.device_position(&p);
    Some((x, y, masque))
}

fn dispositions_en_glissant(b: Rc<Barre>) {
    let fenetre = gtk::Window::new(gtk::WindowType::Popup);
    fenetre.set_widget_name("dispositions");
    fenetre.set_app_paintable(true);
    let dessin = gtk::DrawingArea::new();
    fenetre.add(&dessin);
    // Coins arrondis à chaque nouvelle taille (pas de compositeur partout).
    fenetre.connect_size_allocate(|f, _| natif::decouper_maintenant(f));
    let etat: Rc<RefCell<Option<Glisse>>> = Rc::new(RefCell::new(None));
    let e = etat.clone();
    dessin.connect_draw(move |zone, cr| {
        if let Some(g) = e.borrow().as_ref() {
            dessiner_dispositions(cr, zone.allocated_width() as f64, zone.allocated_height() as f64, g);
        }
        glib::Propagation::Stop
    });
    glib::timeout_add_local(Duration::from_millis(40), move || {
        suivre_glisse(&b, &fenetre, &dessin, &etat);
        glib::ControlFlow::Continue
    });
}

fn suivre_glisse(b: &Rc<Barre>, fenetre: &gtk::Window, dessin: &gtk::DrawingArea, etat: &Rc<RefCell<Option<Glisse>>>) {
    use natif::dispositions as dispo;
    let Some(ecran) = b.ecran.as_ref() else { return };
    let Some((x, y, masque)) = pointeur_et_boutons() else { return };
    let tenu = masque.contains(gdk::ModifierType::BUTTON1_MASK);
    let mut e = etat.borrow_mut();

    if !tenu {
        // Lâché : si c'était sur une case, la fenêtre y va.
        if let Some(g) = e.take() {
            fenetre.hide();
            if let (true, Some((d, c))) = (g.bouge, g.survol) {
                let (mx, my, ml, mh) = g.ecran;
                let travail = (mx, my, ml, mh - natif::bande_basse());
                let (rx, ry, rl, rh) = dispo::en_pixels(dispo::dispositions()[d][c], travail, DISPO_AIR);
                let xid = g.xid;
                let b = b.clone();
                // Après qu'openbox a fini SON déplacement (il reçoit le lâcher aussi).
                glib::timeout_add_local_once(Duration::from_millis(60), move || {
                    if let Some(ecran) = b.ecran.as_ref() {
                        ecran.ranger(xid, rx, ry, rl, rh);
                    }
                });
                dire(&format!("dispositions : fenetre rangee (disposition {d}, case {c})"));
            }
        }
        return;
    }

    // Le bouton vient d'être enfoncé : on note la fenêtre active et sa place.
    if e.is_none() {
        let coupees = dispo::coupees(&std::fs::read_to_string(natif::reglages().join("dispositions")).unwrap_or_default());
        let active = ecran.active();
        let lieu = active.and_then(|xid| Some((xid, ecran.position(xid)?, ecran.taille(xid)?)));
        *e = Some(match (coupees, lieu) {
            (false, Some((xid, depart, taille))) => Glisse {
                actif: true, xid, depart, taille, bouge: false, ecran: natif::ecran_sous(x, y), deplie: false, survol: None,
            },
            _ => Glisse {
                actif: false, xid: 0, depart: (0, 0), taille: (0, 0), bouge: false, ecran: (0, 0, 0, 0), deplie: false, survol: None,
            },
        });
        return;
    }
    let Some(g) = e.as_mut() else { return };
    if !g.actif {
        return;
    }

    if !g.bouge {
        // Openbox donne le focus à l'appui : la fenêtre active peut changer
        // juste après. On suit la nouvelle.
        if let Some(active) = ecran.active() {
            if active != g.xid {
                if let (Some(p), Some(t)) = (ecran.position(active), ecran.taille(active)) {
                    g.xid = active;
                    g.depart = p;
                    g.taille = t;
                }
                return;
            }
        }
        let (Some(p), Some(t)) = (ecran.position(g.xid), ecran.taille(g.xid)) else { return };
        if p == g.depart || t != g.taille {
            return; // immobile, ou redimensionnée par un bord : pas un glissement
        }
        g.bouge = true;
    }

    // Elle glisse : la barre suit l'écran sous le pointeur.
    let mon = natif::ecran_sous(x, y);
    let deplie = y - mon.1 < DISPO_ZONE_HAUTE;
    let largeur = (mon.2 as f64 * 0.86) as i32;
    let hauteur = if deplie { DISPO_DEPLIEE } else { DISPO_REPLIEE };
    let (fx, fy) = (mon.0 + (mon.2 - largeur) / 2, mon.1 + 10);
    let survol = if deplie {
        dispo::case_sous(&dispo::plan(largeur as f64, hauteur as f64), (x - fx) as f64, (y - fy) as f64)
    } else {
        None
    };
    let change = !fenetre.is_visible() || mon != g.ecran || deplie != g.deplie || survol != g.survol;
    g.ecran = mon;
    g.deplie = deplie;
    g.survol = survol;
    drop(e);
    if change {
        fenetre.set_size_request(largeur, hauteur);
        fenetre.resize(largeur, hauteur);
        fenetre.move_(fx, fy);
        if !fenetre.is_visible() {
            fenetre.show_all();
        }
        natif::decouper_maintenant(fenetre);
        dessin.queue_draw();
    }
}

fn dessiner_dispositions(cr: &gtk::cairo::Context, l: f64, h: f64, g: &Glisse) {
    use natif::dispositions as dispo;
    // Le fond : verre sombre, liseré clair — la famille du dock.
    cr.set_source_rgba(0.07, 0.08, 0.11, 0.96);
    cr.paint().ok();
    chemin_arrondi(cr, 0.5, 0.5, l - 1.0, h - 1.0, 14.0);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.14);
    cr.set_line_width(1.0);
    cr.stroke().ok();
    if !g.deplie {
        // Replié : un trait, comme la poignée de Windows.
        chemin_arrondi(cr, l / 2.0 - 40.0, h / 2.0 - 2.0, 80.0, 4.0, 2.0);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.45);
        cr.fill().ok();
        return;
    }
    let p = dispo::plan(l, h);
    for (d, (mini, cases)) in p.miniatures.iter().zip(&p.cases).enumerate() {
        let (x, y, w, hh) = *mini;
        chemin_arrondi(cr, x, y, w, hh, 8.0);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.04);
        cr.fill().ok();
        for (c, &(cx, cy, cw, ch)) in cases.iter().enumerate() {
            chemin_arrondi(cr, cx, cy, cw, ch, 5.0);
            match g.survol {
                // La case visée, à la couleur de grenOS.
                Some((sd, sc)) if sd == d && sc == c => cr.set_source_rgba(0.49, 0.36, 1.0, 0.92),
                Some((sd, _)) if sd == d => cr.set_source_rgba(1.0, 1.0, 1.0, 0.20),
                _ => cr.set_source_rgba(1.0, 1.0, 1.0, 0.11),
            }
            cr.fill().ok();
        }
    }
}

fn chemin_arrondi(cr: &gtk::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    use std::f64::consts::PI;
    let r = r.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
    cr.arc(x + r, y + r, r, PI, 1.5 * PI);
    cr.close_path();
}

fn pactl(args: &[&str]) -> Option<String> {
    let sortie = std::process::Command::new("pactl").args(args).output().ok()?;
    sortie.status.success().then(|| String::from_utf8_lossy(&sortie.stdout).into_owned())
}

/// Par quoi la machine est reliée, et à quelle force — `reseau.icone_barre`.
fn icone_reseau() -> (String, String) {
    let lire = |p: String| std::fs::read_to_string(p).unwrap_or_default();
    let mut interfaces: Vec<String> = std::fs::read_dir("/sys/class/net")
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    interfaces.sort();
    let (mut cable, mut sans_fil) = (None, None);
    for i in interfaces.into_iter().filter(|i| i != "lo") {
        if lire(format!("/sys/class/net/{i}/operstate")).trim() != "up" || lire(format!("/sys/class/net/{i}/carrier")).trim() != "1" {
            continue;
        }
        if std::path::Path::new(&format!("/sys/class/net/{i}/wireless")).is_dir() {
            sans_fil = sans_fil.or(Some(i));
        } else {
            cable = cable.or(Some(i));
        }
    }
    if let Some(i) = cable {
        return ("grenos-reseau-cable".into(), format!("Connecté par câble ({i})"));
    }
    if let Some(i) = sans_fil {
        let barres = force_wifi(&lire("/proc/net/wireless".into()), &i);
        let phrase = ["connecté, signal très faible", "signal faible", "signal moyen", "bon signal"][barres as usize];
        return (format!("grenos-reseau-wifi-{barres}"), format!("Wi-Fi : {phrase} ({i})"));
    }
    ("grenos-reseau-aucun".into(), "Aucune connexion".into())
}

/// 0 à 3 barres d'après « link » dans /proc/net/wireless ; 3 sans ligne.
fn force_wifi(texte: &str, interface: &str) -> u8 {
    for ligne in texte.lines() {
        let Some(reste) = ligne.trim().strip_prefix(&format!("{interface}:")) else { continue };
        let champs: Vec<&str> = reste.split_whitespace().collect();
        let Some(lien) = champs.get(1).and_then(|c| c.trim_end_matches('.').parse::<f64>().ok()) else { return 3 };
        return if lien >= 55.0 { 3 } else if lien >= 35.0 { 2 } else if lien >= 12.0 { 1 } else { 0 };
    }
    3
}

fn main() {
    if gtk::init().is_err() {
        eprintln!("grenos-shell-natif : pas d'écran X");
        std::process::exit(1);
    }
    // Après gtk::init : gtk-rs refuse tout appel GTK avant (panique), et la
    // classe des fenêtres n'est lue qu'à leur création.
    natif::nommer("grenos-shell");
    let style = natif::Style::poser(STYLE);
    let barre = Barre::nouvelle();
    barre.panneau.show_all();
    barre.surveiller_applications();
    // grenos-theme : le thème ou l'accent a changé (toutes les barres).
    natif::sur_signal(natif::SIGUSR1, move || style.repeindre());

    if let Some(n) = natif::ecran_secondaire() {
        // UNE COPIE SECONDAIRE (dock sur chaque écran, 20 novembre).
        // `pkill -USR2 -f grenos-shell` (la touche Windows) la touche aussi :
        // sans gestionnaire, SIGUSR2 la TUERAIT. Elle l'ignore — le menu
        // s'ouvre sur l'écran principal, comme sous Windows.
        natif::sur_signal(natif::SIGUSR2, || {});
        dire(&format!("barre secondaire prete, ecran {n}"));
        // Si la barre principale disparaît (relancée par son veilleur, ou
        // tuée par les Réglages), ses copies partent avec elle : la nouvelle
        // en relancera, sans doublons.
        let parent = std::os::unix::process::parent_id();
        glib::timeout_add_seconds_local(3, move || {
            if std::os::unix::process::parent_id() != parent {
                gtk::main_quit();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        gtk::main();
        return;
    }

    dire(&format!("barre prete, {} epinglees (native{})", barre.epingles.borrow().len(), if barre.dock { ", dock" } else { "" }));
    let b = barre.clone();
    glib::timeout_add_seconds_local_once(3, move || b.dire_les_boutons());
    // grenos-menu (Ctrl+Échap, touche Windows) : ouvrir le menu.
    let b = barre.clone();
    natif::sur_signal(natif::SIGUSR2, move || b.ouvrir_menu());
    copies_secondaires();
    // Les dispositions en glissant une fenêtre : la barre principale seule.
    dispositions_en_glissant(barre.clone());
    gtk::main();
}

/// Une barre par AUTRE écran (20 novembre), relancées quand on branche ou
/// débranche un écran. Chaque copie est ce même programme avec GRENOS_ECRAN.
fn copies_secondaires() {
    use std::process::{Child, Command};
    let enfants: Rc<RefCell<Vec<Child>>> = Rc::new(RefCell::new(Vec::new()));
    let relancer = {
        let enfants = enfants.clone();
        move || {
            for mut e in enfants.borrow_mut().drain(..) {
                let _ = e.kill();
                let _ = e.wait();
            }
            let Ok(moi) = std::env::current_exe() else { return };
            for n in natif::autres_ecrans() {
                if let Ok(e) = Command::new(&moi).env("GRENOS_ECRAN", n.to_string()).spawn() {
                    enfants.borrow_mut().push(e);
                }
            }
            dire(&format!("barre : {} ecran(s) de plus, un dock sur chacun", enfants.borrow().len()));
        }
    };
    relancer();
    if let Some(d) = gdk::Display::default() {
        // Une rafale d'événements (un écran qui s'allume) : un seul relancement.
        let minuteur: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let relancer = Rc::new(relancer);
        let changer = {
            let minuteur = minuteur.clone();
            let relancer = relancer.clone();
            move || {
                if let Some(id) = minuteur.borrow_mut().take() {
                    id.remove();
                }
                let relancer = relancer.clone();
                let m = minuteur.clone();
                *minuteur.borrow_mut() = Some(glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                    m.borrow_mut().take();
                    relancer();
                }));
            }
        };
        let c = changer.clone();
        d.connect_monitor_added(move |_, _| c());
        d.connect_monitor_removed(move |_, _| changer());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_force_du_wifi() {
        let t = "Inter-| sta-|   Quality\n face | tus | link level noise\n wlan0: 0000   60.  -50.  -256\n";
        assert_eq!(force_wifi(t, "wlan0"), 3);
        assert_eq!(force_wifi(" wlan0: 0000   20.  -80.  -256", "wlan0"), 1);
        assert_eq!(force_wifi("", "wlan0"), 3);
    }
}
