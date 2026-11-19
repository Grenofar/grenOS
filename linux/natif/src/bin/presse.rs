//! grenos-presse-natif : l'historique du presse-papier, Win+V (todo G31 —
//! 18 novembre).
//!
//! Il tourne toute la session (lancé par grenos-session, relevé par
//! grenos-veilleur) : il écoute le presse-papier et garde les 25 derniers
//! textes copiés. Win+V lui envoie SIGUSR1 (`grenos-presse`) ; il montre la
//! liste au-dessus de la barre.
//!
//! RIEN N'EST ÉCRIT SUR LE DISQUE. Un mot de passe copié depuis un
//! gestionnaire ne doit pas survivre à la session dans un fichier : la liste
//! vit en mémoire, et disparaît à la déconnexion.
//!
//! CE QU'IL NE FAIT PAS, et la fenêtre le dit : coller tout seul. Windows
//! colle l'élément choisi dans l'application d'avant ; il faudrait ici simuler
//! Ctrl+V, et l'image n'a pas d'outil pour ça. Choisir un élément le met dans
//! le presse-papier, et on colle avec Ctrl+V.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use natif::historique::{apercu, Historique};
use natif::{classe, dire};

const LARGEUR: i32 = 420;
const HAUTEUR: i32 = 460;
// La place de la barre (ou du dock), air compris : natif::bande_basse.
fn hauteur_barre() -> i32 {
    natif::bande_basse()
}

const STYLE: &str = r#"
#presse { background-color: @fond; border: 1px solid @bord-clair; border-radius: 14px; }
#presse .entete { padding: 12px 14px 6px 14px; }
#presse list { background-color: transparent; }
#presse row { padding: 8px 12px; border-radius: 10px; margin: 2px 8px; }
#presse row:hover, #presse row:selected { background-color: @survol; }
#presse .pied { padding: 8px 14px 12px 14px; }
"#;

struct Presse {
    historique: RefCell<Historique>,
    fenetre: gtk::Window,
    liste: gtk::ListBox,
    vide: gtk::Label,
    presse_papier: gtk::Clipboard,
    /// Nous venons de mettre ce texte dans le presse-papier : l'écoute ne doit
    /// pas le prendre pour une nouvelle copie.
    pose: RefCell<Option<String>>,
}

impl Presse {
    fn nouveau() -> Rc<Presse> {
        let fenetre = gtk::Window::new(gtk::WindowType::Toplevel);
        fenetre.set_widget_name("presse");
        fenetre.set_title("Presse-papier");
        fenetre.set_decorated(false);
        fenetre.set_skip_taskbar_hint(true);
        fenetre.set_skip_pager_hint(true);
        fenetre.set_type_hint(gdk::WindowTypeHint::Utility);
        fenetre.set_keep_above(true);
        fenetre.set_size_request(LARGEUR, HAUTEUR);

        let colonne = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let entete = gtk::Label::new(Some("Presse-papier"));
        entete.set_xalign(0.0);
        classe(&entete, "entree-titre");
        classe(&entete, "entete");
        colonne.pack_start(&entete, false, false, 0);

        let defile = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        defile.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        let liste = gtk::ListBox::new();
        liste.set_selection_mode(gtk::SelectionMode::Single);
        defile.add(&liste);
        colonne.pack_start(&defile, true, true, 0);

        let vide = gtk::Label::new(Some("Rien n'a encore été copié pendant cette session."));
        vide.set_line_wrap(true);
        classe(&vide, "sous");
        colonne.pack_start(&vide, false, false, 12);

        let pied = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        classe(&pied, "pied");
        let note = gtk::Label::new(Some("Entrée copie l'élément ; colle-le avec Ctrl+V."));
        note.set_xalign(0.0);
        note.set_line_wrap(true);
        classe(&note, "sous");
        pied.pack_start(&note, true, true, 0);
        let vider = gtk::Button::with_label("Tout effacer");
        classe(&vider, "discret");
        pied.pack_end(&vider, false, false, 0);
        colonne.pack_start(&pied, false, false, 0);
        fenetre.add(&colonne);

        let p = Rc::new(Presse {
            historique: RefCell::new(Historique::default()),
            fenetre,
            liste,
            vide,
            presse_papier: gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD),
            pose: RefCell::new(None),
        });

        let q = p.clone();
        vider.connect_clicked(move |_| {
            q.historique.borrow_mut().vider();
            q.remplir();
        });
        let q = p.clone();
        p.liste.connect_row_activated(move |_, rang| q.choisir(rang.index().max(0) as usize));
        let q = p.clone();
        p.fenetre.connect_key_press_event(move |_, ev| {
            if ev.keyval() == gdk::keys::constants::Escape {
                q.fenetre.hide();
                return glib::Propagation::Stop;
            }
            if ev.keyval() == gdk::keys::constants::Delete {
                if let Some(r) = q.liste.selected_row() {
                    q.historique.borrow_mut().retirer(r.index().max(0) as usize);
                    q.remplir();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        let q = p.clone();
        p.fenetre.connect_focus_out_event(move |_, _| {
            q.fenetre.hide();
            glib::Propagation::Proceed
        });
        let q = p.clone();
        p.fenetre.connect_delete_event(move |_, _| {
            q.fenetre.hide();
            glib::Propagation::Stop
        });
        natif::arrondir(&p.fenetre);

        // L'écoute : chaque nouveau propriétaire du presse-papier. gtk-rs 0.18
        // ne génère pas `connect_owner_change` pour GtkClipboard (le signal
        // porte un GdkEventOwnerChange) : on s'y branche par son nom.
        let q = p.clone();
        p.presse_papier.connect_local("owner-change", false, move |_| {
            let r = q.clone();
            q.presse_papier.request_text(move |_, texte| {
                let Some(texte) = texte else { return };
                if r.pose.borrow().as_deref() == Some(texte) {
                    *r.pose.borrow_mut() = None;
                    return;
                }
                if r.historique.borrow_mut().ajouter(texte) && r.fenetre.is_visible() {
                    r.remplir();
                }
            });
            None
        });
        p
    }

    fn remplir(&self) {
        for enfant in self.liste.children() {
            self.liste.remove(&enfant);
        }
        let h = self.historique.borrow();
        for texte in h.elements() {
            let l = gtk::Label::new(Some(&apercu(texte, 120)));
            l.set_xalign(0.0);
            l.set_line_wrap(true);
            l.set_lines(3);
            l.set_ellipsize(gtk::pango::EllipsizeMode::End);
            self.liste.add(&l);
        }
        self.vide.set_visible(h.elements().is_empty());
        self.liste.show_all();
        if let Some(premier) = self.liste.row_at_index(0) {
            self.liste.select_row(Some(&premier));
            premier.grab_focus();
        }
    }

    fn choisir(&self, rang: usize) {
        let Some(texte) = self.historique.borrow().elements().get(rang).cloned() else { return };
        *self.pose.borrow_mut() = Some(texte.clone());
        self.presse_papier.set_text(&texte);
        // L'élément choisi remonte en tête, comme une nouvelle copie.
        self.historique.borrow_mut().ajouter(&texte);
        self.fenetre.hide();
        dire("presse : element copie depuis l historique");
    }

    fn montrer(&self) {
        if self.fenetre.is_visible() {
            self.fenetre.hide();
            return;
        }
        self.remplir();
        let (l, h) = natif::taille_ecran();
        // Au-dessus de la barre, au centre : là où le regard est quand on
        // vient de taper Win+V.
        self.fenetre.move_(((l - LARGEUR) / 2).max(8), (h - hauteur_barre() - HAUTEUR - 12).max(8));
        self.fenetre.show_all();
        self.vide.set_visible(self.historique.borrow().elements().is_empty());
        self.fenetre.present();
        dire(&format!("presse : historique ouvert, {} element(s)", self.historique.borrow().elements().len()));
    }
}

fn main() {
    if gtk::init().is_err() {
        eprintln!("grenos-presse-natif : pas d'écran X");
        std::process::exit(1);
    }
    natif::nommer("grenos-presse");
    let style = natif::Style::poser(STYLE);
    let presse = Presse::nouveau();
    let p = presse.clone();
    // Repeindre à chaque ouverture : le thème a pu changer depuis (jour,
    // nuit, accent), et grenos-theme ne prévient que la barre.
    natif::sur_signal(natif::SIGUSR1, move || {
        style.repeindre();
        p.montrer();
    });
    dire("presse : historique pret (en memoire seulement)");
    gtk::main();
}
