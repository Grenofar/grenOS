//! grenos-son-natif : le panneau du son, au-dessus de la barre.
//!
//! Repris de `grenos-son` (Python) le 18 novembre, avec ses trois blocs —
//! le volume de la machine, la sortie, le volume de chaque application — et
//! surtout ses règles de fermeture, apprises une à une :
//! - il attrape le pointeur (natif::Attrape), et un clic à côté le ferme ;
//! - perdre le focus le ferme aussi (second filet), SAUF pendant que la liste
//!   des sorties est déroulée : elle est elle-même une fenêtre ;
//! - il n'y a qu'UN chemin de sortie (`partir`), et il dit toujours « son
//!   ferme » : le 25 septembre, le panneau se fermait sans le dire, et la CI
//!   concluait qu'il restait ouvert. Le défaut était dans la preuve.
//!
//! Les lignes série sont celles du Python, au mot près.

use std::cell::{Cell, RefCell};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};

use natif::{classe, dire};

const LARGEUR: i32 = 380;
const HAUTEUR_BARRE: i32 = 52;

const STYLE: &str = r#"
#panneau-son {
    background-color: @fond;
    border: 1px solid @bord-clair;
    border-radius: 14px;
}
.bloc-son { padding: 14px 16px; }
.separateur-son { background-color: @bord; min-height: 1px; }
.nom-app { font-size: 13px; }
"#;

// ---- pactl ----------------------------------------------------------------

fn pactl(arguments: &[&str]) -> String {
    Command::new("timeout")
        .arg("8")
        .arg("pactl")
        .args(arguments)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

const PROFILS: &[(&str, &str)] = &[
    ("Digital Surround 7.1 (HDMI)", "Surround numérique 7.1 (HDMI)"),
    ("Digital Surround 5.1 (HDMI)", "Surround numérique 5.1 (HDMI)"),
    ("Digital Stereo (IEC958)", "Stéréo numérique (S/PDIF)"),
    ("Digital Stereo (HDMI)", "Stéréo numérique (HDMI)"),
    ("Analog Surround 7.1", "Surround analogique 7.1"),
    ("Analog Surround 5.1", "Surround analogique 5.1"),
    ("Analog Stereo Duplex", "Stéréo analogique bidirectionnelle"),
    ("Analog Stereo Output", "Sortie stéréo analogique"),
    ("Analog Stereo Input", "Entrée stéréo analogique"),
    ("Analog Stereo", "Stéréo analogique"),
    ("Analog Mono", "Mono analogique"),
    ("Digital Stereo", "Stéréo numérique"),
    ("Line Out", "Sortie ligne"),
    ("Headphones", "Casque"),
    ("Headset", "Micro-casque"),
    ("Speakers", "Haut-parleurs"),
    ("Internal Microphone", "Micro interne"),
    ("Microphone", "Micro"),
];

/// Le nom d'une sortie, en français. On ne touche QUE ce qui s'affiche.
fn en_francais(description: &str) -> String {
    let mut d = description.to_string();
    for (anglais, francais) in PROFILS {
        if d.contains(anglais) {
            d = d.replace(anglais, francais);
        }
    }
    d
}

fn sorties_de(brut: &str) -> Vec<(String, String)> {
    let mut trouvees = Vec::new();
    let mut nom = String::new();
    for ligne in brut.lines().map(str::trim) {
        if let Some(v) = ligne.strip_prefix("Name:") {
            nom = v.trim().to_string();
        } else if let Some(v) = ligne.strip_prefix("Description:") {
            if !nom.is_empty() {
                let lisible = en_francais(v.trim());
                trouvees.push((nom.clone(), if lisible.is_empty() { nom.clone() } else { lisible }));
                nom.clear();
            }
        }
    }
    trouvees
}

fn sorties() -> Vec<(String, String)> {
    sorties_de(&pactl(&["list", "sinks"]))
}

fn sortie_actuelle() -> String {
    pactl(&["info"])
        .lines()
        .find_map(|l| l.strip_prefix("Default Sink:").map(|v| v.trim().to_string()))
        .unwrap_or_default()
}

/// Le premier « NN% » d'un texte.
fn pour_cent(texte: &str) -> Option<u32> {
    let i = texte.find('%')?;
    let debut = texte[..i].rfind(|c: char| !c.is_ascii_digit()).map(|j| j + 1).unwrap_or(0);
    texte[debut..i].parse().ok()
}

fn volume() -> u32 {
    pour_cent(&pactl(&["get-sink-volume", "@DEFAULT_SINK@"])).unwrap_or(0)
}

fn muet() -> bool {
    pactl(&["get-sink-mute", "@DEFAULT_SINK@"]).contains("yes")
}

struct Application {
    index: String,
    nom: String,
    volume: u32,
}

fn applications_de(brut: &str) -> Vec<Application> {
    let mut trouvees = Vec::new();
    let mut courante: Option<Application> = None;
    for ligne in brut.lines().map(str::trim) {
        if let Some(i) = ligne.strip_prefix("Sink Input #") {
            trouvees.extend(courante.take());
            courante = Some(Application { index: i.trim().to_string(), nom: String::new(), volume: 0 });
        } else if let Some(a) = courante.as_mut() {
            if let Some(v) = ligne.strip_prefix("application.name =") {
                a.nom = v.trim().trim_matches('"').to_string();
            } else if ligne.starts_with("Volume:") && a.volume == 0 {
                a.volume = pour_cent(ligne).unwrap_or(0);
            }
        }
    }
    trouvees.extend(courante);
    for a in &mut trouvees {
        if a.nom.is_empty() {
            a.nom = "Application".into();
        }
    }
    trouvees
}

// ---- Le panneau ------------------------------------------------------------

struct Panneau {
    fenetre: gtk::Window,
    image: gtk::Image,
    valeur: gtk::Label,
    bouton_muet: gtk::Button,
    curseur: gtk::Scale,
    choix: gtk::ComboBoxText,
    liste: gtk::Box,
    sorties: RefCell<Vec<(String, String)>>,
    /// Un réglage en cours (curseur tenu) : la relecture n'écrase rien.
    en_train: Cell<bool>,
    /// On pose nous-mêmes une valeur : ce n'est pas un geste de la personne.
    pose: Cell<bool>,
    deroule: Cell<bool>,
    parti: Cell<bool>,
    attrape: RefCell<Option<Rc<natif::Attrape>>>,
}

fn trait_() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    classe(&b, "separateur-son");
    b
}

fn section(texte: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(texte));
    l.set_xalign(0.0);
    classe(&l, "section");
    l
}

fn curseur() -> gtk::Scale {
    let c = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    c.set_draw_value(false);
    c
}

impl Panneau {
    fn nouveau() -> Rc<Panneau> {
        let fenetre = gtk::Window::new(gtk::WindowType::Toplevel);
        fenetre.set_widget_name("panneau-son");
        fenetre.set_title("Son");
        fenetre.set_decorated(false);
        fenetre.set_skip_taskbar_hint(true);
        fenetre.set_skip_pager_hint(true);
        fenetre.set_type_hint(gdk::WindowTypeHint::Utility);
        fenetre.set_keep_above(true);
        fenetre.set_size_request(LARGEUR, -1);

        let colonne = gtk::Box::new(gtk::Orientation::Vertical, 0);
        fenetre.add(&colonne);

        let haut = gtk::Box::new(gtk::Orientation::Vertical, 10);
        classe(&haut, "bloc-son");
        let titre = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let image = gtk::Image::from_icon_name(Some("grenos-son"), gtk::IconSize::LargeToolbar);
        titre.pack_start(&image, false, false, 0);
        let valeur = gtk::Label::new(None);
        valeur.set_xalign(0.0);
        classe(&valeur, "entree-titre");
        titre.pack_start(&valeur, true, true, 0);
        let bouton_muet = gtk::Button::with_label("Couper");
        classe(&bouton_muet, "discret");
        titre.pack_end(&bouton_muet, false, false, 0);
        haut.pack_start(&titre, false, false, 0);
        let principal = curseur();
        haut.pack_start(&principal, false, false, 0);
        colonne.pack_start(&haut, false, false, 0);
        colonne.pack_start(&trait_(), false, false, 0);

        let milieu = gtk::Box::new(gtk::Orientation::Vertical, 8);
        classe(&milieu, "bloc-son");
        milieu.pack_start(&section("SORTIE"), false, false, 0);
        let choix = gtk::ComboBoxText::new();
        milieu.pack_start(&choix, false, false, 0);
        colonne.pack_start(&milieu, false, false, 0);
        colonne.pack_start(&trait_(), false, false, 0);

        let bas = gtk::Box::new(gtk::Orientation::Vertical, 8);
        classe(&bas, "bloc-son");
        bas.pack_start(&section("APPLICATIONS"), false, false, 0);
        let liste = gtk::Box::new(gtk::Orientation::Vertical, 8);
        bas.pack_start(&liste, false, false, 0);
        colonne.pack_start(&bas, false, false, 0);

        let p = Rc::new(Panneau {
            fenetre,
            image,
            valeur,
            bouton_muet,
            curseur: principal,
            choix,
            liste,
            sorties: RefCell::new(Vec::new()),
            en_train: Cell::new(false),
            pose: Cell::new(false),
            deroule: Cell::new(false),
            parti: Cell::new(false),
            attrape: RefCell::new(None),
        });
        p.brancher();
        p
    }

    fn brancher(self: &Rc<Self>) {
        let p = self.clone();
        self.curseur.connect_value_changed(move |c| {
            if !p.pose.get() {
                pactl(&["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", c.value() as u32)]);
            }
        });
        self.tenir(&self.curseur.clone().upcast());

        let p = self.clone();
        self.bouton_muet.connect_clicked(move |_| {
            pactl(&["set-sink-mute", "@DEFAULT_SINK@", "toggle"]);
            p.relire();
        });

        let p = self.clone();
        self.choix.connect_changed(move |c| {
            if p.pose.get() {
                return;
            }
            let Some(i) = c.active() else { return };
            let Some((nom, _)) = p.sorties.borrow().get(i as usize).cloned() else { return };
            // Changer la sortie, et y déplacer ce qui joue déjà.
            pactl(&["set-default-sink", &nom]);
            for ligne in pactl(&["list", "short", "sink-inputs"]).lines() {
                if let Some(index) = ligne.split_whitespace().next() {
                    pactl(&["move-sink-input", index, &nom]);
                }
            }
        });
        let p = self.clone();
        self.choix.connect_notify_local(Some("popup-shown"), move |c, _| {
            p.deroule.set(c.property::<bool>("popup-shown"));
        });

        self.fenetre.add_events(gdk::EventMask::BUTTON_PRESS_MASK);
        let p = self.clone();
        self.fenetre.connect_button_press_event(move |f, ev| {
            let (x, y) = ev.position();
            let (l, h) = f.size();
            if !(0.0..l as f64).contains(&x) || !(0.0..h as f64).contains(&y) {
                p.partir();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        let p = self.clone();
        self.fenetre.connect_key_press_event(move |_, ev| {
            if ev.keyval() == gdk::keys::constants::Escape {
                p.partir();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        let p = self.clone();
        self.fenetre.connect_destroy(move |_| p.partir());
    }

    /// Un curseur tenu par la personne : la relecture le laisse tranquille.
    fn tenir(self: &Rc<Self>, w: &gtk::Widget) {
        let p = self.clone();
        w.connect_button_press_event(move |_, _| {
            p.en_train.set(true);
            glib::Propagation::Proceed
        });
        let p = self.clone();
        w.connect_button_release_event(move |_, _| {
            p.en_train.set(false);
            glib::Propagation::Proceed
        });
    }

    fn relire(self: &Rc<Self>) {
        if self.en_train.get() || self.parti.get() {
            return;
        }
        let v = volume();
        let m = muet();
        self.pose.set(true);
        self.curseur.set_value(v as f64);
        self.valeur.set_text(&if m { "Silence".to_string() } else { format!("Volume {v} %") });
        let icone = if m { "grenos-son-muet" } else if v < 45 { "grenos-son-faible" } else { "grenos-son" };
        self.image.set_from_icon_name(Some(icone), gtk::IconSize::LargeToolbar);
        self.bouton_muet.set_label(if m { "Rétablir" } else { "Couper" });

        let nouvelles = sorties();
        let memes = nouvelles.iter().map(|s| &s.0).eq(self.sorties.borrow().iter().map(|s| &s.0));
        if !memes {
            let courante = sortie_actuelle();
            self.choix.remove_all();
            for (i, (nom, description)) in nouvelles.iter().enumerate() {
                self.choix.append_text(description);
                if *nom == courante {
                    self.choix.set_active(Some(i as u32));
                }
            }
            if !nouvelles.is_empty() && self.choix.active().is_none() {
                self.choix.set_active(Some(0));
            }
            *self.sorties.borrow_mut() = nouvelles;
        }
        self.pose.set(false);
        self.poser_applications();
    }

    fn poser_applications(self: &Rc<Self>) {
        for enfant in self.liste.children() {
            self.liste.remove(&enfant);
        }
        let jouent = applications_de(&pactl(&["list", "sink-inputs"]));
        if jouent.is_empty() {
            let vide = gtk::Label::new(Some("Rien ne joue en ce moment."));
            vide.set_xalign(0.0);
            classe(&vide, "sous");
            self.liste.pack_start(&vide, false, false, 0);
        }
        for a in jouent {
            let ligne = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let nom = gtk::Label::new(Some(&a.nom));
            nom.set_xalign(0.0);
            classe(&nom, "nom-app");
            ligne.pack_start(&nom, false, false, 0);
            let c = curseur();
            c.set_value(a.volume as f64);
            let index = a.index.clone();
            c.connect_value_changed(move |c| {
                pactl(&["set-sink-input-volume", &index, &format!("{}%", c.value() as u32)]);
            });
            self.tenir(&c.clone().upcast());
            ligne.pack_start(&c, false, false, 0);
            self.liste.pack_start(&ligne, false, false, 0);
        }
        self.liste.show_all();
    }

    fn ouvrir(self: &Rc<Self>) -> (i32, i32) {
        self.fenetre.show_all();
        let (largeur, hauteur) = self.fenetre.size();
        let (l, h) = natif::taille_ecran();
        // En bas à DROITE, juste au-dessus de la barre : là où est l'icône qui
        // l'ouvre. Un panneau loin du bouton cliqué se lit comme une fenêtre
        // apparue toute seule.
        let place = ((l - largeur - 10).max(10), h - HAUTEUR_BARRE - hauteur - 8);
        self.fenetre.move_(place.0, place.1);
        self.fenetre.present();
        natif::arrondir(&self.fenetre);

        let p = self.clone();
        self.fenetre.connect_focus_out_event(move |_, _| {
            if !p.deroule.get() {
                p.partir();
            }
            glib::Propagation::Proceed
        });
        let attrape = natif::Attrape::prendre_puis(&self.fenetre, |pris| {
            dire(&format!("son : le pointeur est {}", if pris { "attrape" } else { "refuse, on compte sur le focus" }));
        });
        *self.attrape.borrow_mut() = Some(attrape);
        place
    }

    /// Le SEUL chemin de sortie, et il dit toujours qu'il part.
    fn partir(&self) {
        if self.parti.replace(true) {
            return;
        }
        dire("son ferme");
        if let Some(a) = self.attrape.borrow_mut().take() {
            a.lacher();
        }
        gtk::main_quit();
    }
}

fn main() {
    if gtk::init().is_err() {
        eprintln!("grenos-son-natif : pas d'écran X");
        std::process::exit(1);
    }
    natif::nommer("grenos-son");
    let _style = natif::Style::poser(STYLE);
    let panneau = Panneau::nouveau();
    panneau.relire();
    let place = panneau.ouvrir();
    let (l, h) = panneau.fenetre.size();
    // Dire le volume et l'état muet prouve qu'on sait LIRE le mélangeur.
    dire(&format!(
        "son ouvert, {} sorties, volume {}%, {}, panneau {l}x{h} (natif)",
        panneau.sorties.borrow().len(),
        volume(),
        if muet() { "muet" } else { "actif" }
    ));
    // L'endroit DEMANDÉ, pas celui que X rapporte (souvent périmé) : la CI
    // mesure les quatre coins de la capture à cet endroit.
    dire(&format!("son : panneau en {},{} de {l}x{h}", place.0, place.1));

    let p = panneau.clone();
    glib::timeout_add_local(Duration::from_secs(2), move || {
        if p.parti.get() {
            return glib::ControlFlow::Break;
        }
        p.relire();
        glib::ControlFlow::Continue
    });
    gtk::main();
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn sorties_en_francais() {
        let s = sorties_de("Sink #1\n\tName: alsa_output.pci.analog-stereo\n\tDescription: Built-in Audio Analog Stereo\nSink #2\n\tName: hdmi\n\tDescription: Digital Stereo (HDMI)\n");
        assert_eq!(s, vec![
            ("alsa_output.pci.analog-stereo".to_string(), "Built-in Audio Stéréo analogique".to_string()),
            ("hdmi".to_string(), "Stéréo numérique (HDMI)".to_string()),
        ]);
    }

    #[test]
    fn pour_cent_lu() {
        assert_eq!(pour_cent("Volume: front-left: 42597 /  65% / -11.23 dB"), Some(65));
        assert_eq!(pour_cent("rien"), None);
    }

    #[test]
    fn applications() {
        let a = applications_de("Sink Input #42\n\tVolume: front-left: 1 / 80% / x\n\tProperties:\n\t\tapplication.name = \"Firefox\"\nSink Input #43\n\tVolume: mono: 1 / 30%\n");
        assert_eq!(a.len(), 2);
        assert_eq!((a[0].index.as_str(), a[0].nom.as_str(), a[0].volume), ("42", "Firefox", 80));
        assert_eq!((a[1].nom.as_str(), a[1].volume), ("Application", 30));
    }
}
