//! grenos-bureau-natif : le fond d'écran et les icônes du dossier Bureau.
//!
//! Remplace `/usr/bin/grenos-bureau` (Python) geste pour geste : le fond est
//! dessiné par la même fenêtre (personne ne se dispute la racine de l'écran),
//! chaque fichier du dossier Bureau a son icône, calée sur la grille de
//! Windows (« quand on déplace des raccourcis, il faut que ça soit gradué »),
//! une case ne prend qu'une icône, et la position reste au redémarrage
//! (`~/.config/grenos/bureau.json`, même format que la version Python).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, gdk_pixbuf, gio, glib, pango};

use natif::applications::{champs_desktop, nettoyer_exec};
use natif::{classe, dire, lancer};

const TAILLE_ICONE: i32 = 56;
const LARGEUR_CASE: i32 = 118;
const HAUTEUR_CASE: i32 = 116;
const MARGE: i32 = 24;
const BARRE: i32 = 52;

const STYLE: &str = r#"
#bureau { background-color: @fond; }
.icone-bureau {
    border-radius: 14px;
    border: 1px solid transparent;
    padding: 10px 6px;
    transition: background-color 130ms ease, border-color 130ms ease;
}
.icone-bureau:hover { background-color: alpha(@accent, 0.20); border-color: alpha(@accent, 0.40); }
.icone-choisie { background-color: alpha(@accent, 0.34); border-color: @accent; }
.nom-icone {
    color: #ffffff;
    font-size: 12.5px;
    font-weight: 600;
    text-shadow: 0 1px 4px rgba(0, 0, 0, 0.95), 0 0 2px rgba(0, 0, 0, 0.8);
}
"#;

// ---- La grille : une seule fonction pour poser ET pour caler ---------------

fn derniere_case(l: i32, h: i32) -> (i32, i32) {
    (((l - MARGE - LARGEUR_CASE) / LARGEUR_CASE).max(0), ((h - BARRE - MARGE - HAUTEUR_CASE) / HAUTEUR_CASE).max(0))
}

fn coin_de(c: i32, l: i32) -> (i32, i32) {
    (MARGE + c * LARGEUR_CASE, MARGE + l * HAUTEUR_CASE)
}

/// La case la plus PROCHE (arrondi, pas division entière), bornée à l'écran.
fn case_de(x: i32, y: i32, l: i32, h: i32) -> (i32, i32) {
    let (cmax, lmax) = derniere_case(l, h);
    let c = ((x - MARGE) as f64 / LARGEUR_CASE as f64).round() as i32;
    let li = ((y - MARGE) as f64 / HAUTEUR_CASE as f64).round() as i32;
    (c.clamp(0, cmax), li.clamp(0, lmax))
}

/// Caler un point sur une case LIBRE : la visée, sinon la plus proche.
/// Même ordre de recherche que la version Python (anneaux, puis distance).
fn caler(x: i32, y: i32, l: i32, h: i32, occupees: &HashSet<(i32, i32)>) -> (i32, i32) {
    let (cmax, lmax) = derniere_case(l, h);
    let (vc, vl) = case_de(x, y, l, h);
    for rayon in 0..(cmax + lmax + 2) {
        let mut voisines = Vec::new();
        for dc in -rayon..=rayon {
            for dl in -rayon..=rayon {
                if dc.abs().max(dl.abs()) != rayon {
                    continue;
                }
                let (c, li) = (vc + dc, vl + dl);
                if (0..=cmax).contains(&c) && (0..=lmax).contains(&li) {
                    voisines.push((dc.abs() + dl.abs(), dc.abs(), c, li));
                }
            }
        }
        voisines.sort();
        for (_, _, c, li) in voisines {
            if !occupees.contains(&coin_de(c, li)) {
                return coin_de(c, li);
            }
        }
    }
    coin_de(vc, vl)
}

/// Le dossier Bureau : `~/Bureau` s'il existe (les réglages XDG ne sont pas
/// encore écrits à la toute première session), sinon celui de XDG.
fn dossier_bureau() -> PathBuf {
    let francais = natif::maison().join("Bureau");
    if francais.is_dir() {
        return francais;
    }
    let chemin = glib::user_special_dir(glib::UserDirectory::Desktop).unwrap_or(francais);
    let _ = std::fs::create_dir_all(&chemin);
    chemin
}

struct Raccourci {
    nom: String,
    icone: String,
    commande: String,
}

fn lire_raccourci(chemin: &Path) -> Option<Raccourci> {
    let texte = std::fs::read(chemin).ok()?;
    let c = champs_desktop(&String::from_utf8_lossy(&texte));
    Some(Raccourci {
        nom: c.get("Name[fr]").or_else(|| c.get("Name")).cloned()
            .unwrap_or_else(|| chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
        icone: c.get("Icon").cloned().unwrap_or_default(),
        commande: nettoyer_exec(c.get("Exec").map(String::as_str).unwrap_or("")),
    })
}

fn est_raccourci(chemin: &Path) -> bool {
    chemin.extension().map(|e| e == "desktop").unwrap_or(false)
}

/// L'image qui représente ce fichier, demandée au système.
fn icone_de(chemin: &Path) -> Option<gdk_pixbuf::Pixbuf> {
    let theme = gtk::IconTheme::default()?;
    let mut nom = "text-x-generic".to_string();
    if chemin.is_dir() {
        nom = "folder".into();
    } else if est_raccourci(chemin) {
        nom = lire_raccourci(chemin).map(|r| r.icone).filter(|i| !i.is_empty()).unwrap_or_else(|| "application-x-executable".into());
        if nom.starts_with('/') {
            if let Ok(p) = gdk_pixbuf::Pixbuf::from_file_at_scale(&nom, TAILLE_ICONE, TAILLE_ICONE, true) {
                return Some(p);
            }
        }
    } else {
        let (devine, _) = gio::content_type_guess(Some(chemin), &[]);
        if let Ok(themee) = gio::content_type_get_icon(&devine).downcast::<gio::ThemedIcon>() {
            if let Some(n) = themee.names().iter().find(|n| theme.has_icon(n)) {
                nom = n.to_string();
            }
        }
    }
    for n in [nom.as_str(), "text-x-generic"] {
        if let Ok(Some(p)) = theme.load_icon(n, TAILLE_ICONE, gtk::IconLookupFlags::FORCE_SIZE) {
            return Some(p);
        }
    }
    None
}

struct Bureau {
    fenetre: gtk::Window,
    fond: gtk::DrawingArea,
    plan: gtk::Fixed,
    largeur: i32,
    hauteur: i32,
    dossier: PathBuf,
    positions: RefCell<BTreeMap<String, (i32, i32)>>,
    image: RefCell<Option<gdk_pixbuf::Pixbuf>>,
    choisie: RefCell<Option<gtk::EventBox>>,
    /// Le glissement en cours : départ du pointeur, coin de départ, icône.
    glisse: RefCell<Option<(f64, f64, i32, i32, gtk::EventBox)>>,
    recalees: Cell<usize>,
    nombre: Cell<usize>,
    moniteur: RefCell<Option<gio::FileMonitor>>,
    a_refaire: Cell<bool>,
    menu: RefCell<Option<gtk::Menu>>,
}

fn fichier_positions() -> PathBuf {
    natif::reglages().join("bureau.json")
}

impl Bureau {
    fn nouveau() -> Rc<Bureau> {
        let (largeur, hauteur) = natif::taille_ecran();
        let fenetre = gtk::Window::new(gtk::WindowType::Toplevel);
        fenetre.set_widget_name("bureau");
        fenetre.set_type_hint(gdk::WindowTypeHint::Desktop);
        fenetre.set_decorated(false);
        fenetre.set_skip_taskbar_hint(true);
        fenetre.set_skip_pager_hint(true);
        fenetre.set_keep_below(true);
        fenetre.set_default_size(largeur, hauteur);
        fenetre.move_(0, 0);

        let fond = gtk::DrawingArea::new();
        let plan = gtk::Fixed::new();
        let pile = gtk::Overlay::new();
        pile.add(&fond);
        pile.add_overlay(&plan);
        fenetre.add(&pile);

        // Le format de la version Python : { "nom": [x, y] }.
        let positions: BTreeMap<String, (i32, i32)> = std::fs::read_to_string(fichier_positions())
            .ok()
            .and_then(|t| serde_json::from_str::<BTreeMap<String, serde_json::Value>>(&t).ok())
            .map(|m| {
                m.into_iter()
                    .filter_map(|(k, v)| Some((k, (v.get(0)?.as_i64()? as i32, v.get(1)?.as_i64()? as i32))))
                    .collect()
            })
            .unwrap_or_default();

        let bureau = Rc::new(Bureau {
            fenetre,
            fond,
            plan,
            largeur,
            hauteur,
            dossier: dossier_bureau(),
            positions: RefCell::new(positions),
            image: RefCell::new(None),
            choisie: RefCell::new(None),
            glisse: RefCell::new(None),
            recalees: Cell::new(0),
            nombre: Cell::new(0),
            moniteur: RefCell::new(None),
            a_refaire: Cell::new(false),
            menu: RefCell::new(None),
        });
        bureau.brancher();
        bureau.repeindre();
        bureau.remplir();
        bureau.surveiller();
        bureau
    }

    fn brancher(self: &Rc<Self>) {
        let b = self.clone();
        self.fond.connect_draw(move |_, cr| {
            if let Some(image) = b.image.borrow().as_ref() {
                cr.set_source_pixbuf(image, 0.0, 0.0);
            } else {
                let (r, g, bl) = natif::theme::palette().get("fond").and_then(|c| natif::theme::rgb(c)).unwrap_or((0.04, 0.06, 0.11));
                cr.set_source_rgb(r, g, bl);
            }
            let _ = cr.paint();
            glib::Propagation::Proceed
        });

        self.fenetre.add_events(gdk::EventMask::BUTTON_PRESS_MASK);
        let b = self.clone();
        self.fenetre.connect_button_press_event(move |_, ev| b.clic_sur_le_vide(ev));

        // Glisser une application du menu jusqu'ici la pose sur le bureau.
        self.fenetre.drag_dest_set(gtk::DestDefaults::ALL, &[], gdk::DragAction::COPY);
        self.fenetre.drag_dest_add_uri_targets();
        let b = self.clone();
        self.fenetre.connect_drag_data_received(move |_, _, x, y, donnees, _, _| b.recevoir(x, y, donnees));
    }

    // ---- Le fond d'écran -----------------------------------------------------
    fn charger_fond(&self) -> Option<gdk_pixbuf::Pixbuf> {
        let moment = natif::theme::reglage("theme");
        let moment = if moment.is_empty() { "nuit".to_string() } else { moment };
        let mut chemin = format!("/usr/share/grenos/grenos-{moment}.png");
        // L'image choisie dans les Réglages l'emporte sur la nôtre.
        let choisie = natif::theme::reglage("fond");
        let choisie = choisie.lines().next().unwrap_or("").trim();
        if !choisie.is_empty() && Path::new(choisie).exists() {
            chemin = choisie.to_string();
        }
        gdk_pixbuf::Pixbuf::from_file_at_scale(&chemin, self.largeur, self.hauteur, false).ok()
    }

    /// Relit le thème et le fond : c'est ce que déclenche le jour/nuit.
    fn repeindre(&self) {
        *self.image.borrow_mut() = self.charger_fond();
        self.fond.queue_draw();
    }

    // ---- Les icônes ----------------------------------------------------------
    fn ecrire_positions(&self) {
        let _ = std::fs::create_dir_all(natif::reglages());
        let positions = self.positions.borrow();
        let carte: BTreeMap<&String, [i32; 2]> = positions.iter().map(|(k, &(x, y))| (k, [x, y])).collect();
        if let Ok(t) = serde_json::to_string_pretty(&carte) {
            let _ = std::fs::write(fichier_positions(), t);
        }
    }

    /// La première case vide, en colonnes, de haut en bas — comme Windows.
    fn place_libre(&self, occupees: &HashSet<(i32, i32)>) -> (i32, i32) {
        let (cmax, lmax) = derniere_case(self.largeur, self.hauteur);
        for c in 0..=cmax {
            for l in 0..=lmax {
                if !occupees.contains(&coin_de(c, l)) {
                    return coin_de(c, l);
                }
            }
        }
        coin_de(0, 0)
    }

    fn remplir(self: &Rc<Self>) {
        for enfant in self.plan.children() {
            self.plan.remove(&enfant);
        }
        *self.choisie.borrow_mut() = None;
        let mut noms: Vec<String> = std::fs::read_dir(&self.dossier)
            .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        noms.sort_by_key(|n| n.to_lowercase());

        let mut occupees = HashSet::new();
        let mut recalees = 0;
        let mut change = false;
        let mut nombre = 0;
        for nom in noms.into_iter().filter(|n| !n.starts_with('.')) {
            let chemin = self.dossier.join(&nom);
            let gardee = self.positions.borrow().get(&nom).copied();
            let (x, y) = match gardee {
                // Une position d'avant la grille est recalée une fois.
                Some((gx, gy)) => {
                    let p = caler(gx, gy, self.largeur, self.hauteur, &occupees);
                    if p != (gx, gy) {
                        recalees += 1;
                    }
                    p
                }
                None => self.place_libre(&occupees),
            };
            if gardee != Some((x, y)) {
                self.positions.borrow_mut().insert(nom.clone(), (x, y));
                change = true;
            }
            occupees.insert((x, y));
            let icone = self.faire_icone(&nom, &chemin);
            self.plan.put(&icone, x, y);
            nombre += 1;
        }
        self.recalees.set(recalees);
        self.nombre.set(nombre);
        if change {
            self.ecrire_positions();
        }
        self.plan.show_all();
    }

    fn faire_icone(self: &Rc<Self>, nom: &str, chemin: &Path) -> gtk::EventBox {
        let boite = gtk::EventBox::new();
        boite.set_visible_window(true);
        classe(&boite, "icone-bureau");
        boite.set_size_request(LARGEUR_CASE - 12, HAUTEUR_CASE - 12);
        let colonne = gtk::Box::new(gtk::Orientation::Vertical, 4);
        if let Some(p) = icone_de(chemin) {
            colonne.pack_start(&gtk::Image::from_pixbuf(Some(&p)), false, false, 0);
        }
        let titre = if est_raccourci(chemin) { lire_raccourci(chemin).map(|r| r.nom).unwrap_or_else(|| nom.into()) } else { nom.to_string() };
        let etiquette = gtk::Label::new(Some(&titre));
        classe(&etiquette, "nom-icone");
        etiquette.set_max_width_chars(12);
        etiquette.set_ellipsize(pango::EllipsizeMode::End);
        etiquette.set_justify(gtk::Justification::Center);
        colonne.pack_start(&etiquette, false, false, 0);
        boite.add(&colonne);

        boite.add_events(gdk::EventMask::BUTTON_PRESS_MASK | gdk::EventMask::BUTTON_RELEASE_MASK | gdk::EventMask::POINTER_MOTION_MASK);
        let (b, n, c) = (self.clone(), nom.to_string(), chemin.to_path_buf());
        boite.connect_button_press_event(move |w, ev| b.clic_icone(w, ev, &n, &c));
        let b = self.clone();
        boite.connect_motion_notify_event(move |w, ev| b.glisser(w, ev));
        let (b, n) = (self.clone(), nom.to_string());
        boite.connect_button_release_event(move |w, _| b.poser(w, &n));
        boite
    }

    fn clic_icone(self: &Rc<Self>, w: &gtk::EventBox, ev: &gdk::EventButton, nom: &str, chemin: &Path) -> glib::Propagation {
        self.selectionner(Some(w));
        if ev.event_type() == gdk::EventType::DoubleButtonPress && ev.button() == 1 {
            *self.glisse.borrow_mut() = None;
            self.ouvrir(chemin);
            return glib::Propagation::Stop;
        }
        if ev.button() == 1 {
            let (x0, y0) = self.positions.borrow().get(nom).copied().unwrap_or((MARGE, MARGE));
            let (rx, ry) = ev.root();
            *self.glisse.borrow_mut() = Some((rx, ry, x0, y0, w.clone()));
            return glib::Propagation::Stop;
        }
        if ev.button() == 3 {
            self.menu_icone(nom, chemin, ev);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    }

    fn glisser(&self, w: &gtk::EventBox, ev: &gdk::EventMotion) -> glib::Propagation {
        let g = self.glisse.borrow();
        let Some((dx, dy, x0, y0, icone)) = g.as_ref() else { return glib::Propagation::Proceed };
        if icone != w {
            return glib::Propagation::Proceed;
        }
        let (rx, ry) = ev.root();
        let x = ((*x0 as f64 + rx - dx) as i32).clamp(0, self.largeur - LARGEUR_CASE);
        let y = ((*y0 as f64 + ry - dy) as i32).clamp(0, self.hauteur - HAUTEUR_CASE - BARRE);
        self.plan.move_(w, x, y);
        glib::Propagation::Stop
    }

    /// Une icône lâchée se CALE sur la grille, et y reste au redémarrage.
    fn poser(&self, w: &gtk::EventBox, nom: &str) -> glib::Propagation {
        let pris = self.glisse.borrow_mut().take();
        let Some((_, _, x0, y0, icone)) = pris else { return glib::Propagation::Stop };
        if &icone != w {
            return glib::Propagation::Stop;
        }
        // La position actuelle de l'icône, lue sur le pointeur.
        let (px, py) = w.translate_coordinates(&self.plan, 0, 0).unwrap_or((x0, y0));
        let prises: HashSet<(i32, i32)> = self.positions.borrow().iter().filter(|(k, _)| k.as_str() != nom).map(|(_, &p)| p).collect();
        let (x, y) = caler(px, py, self.largeur, self.hauteur, &prises);
        self.plan.move_(w, x, y);
        self.positions.borrow_mut().insert(nom.to_string(), (x, y));
        self.ecrire_positions();
        glib::Propagation::Stop
    }

    fn selectionner(&self, w: Option<&gtk::EventBox>) {
        if let Some(ancienne) = self.choisie.borrow_mut().take() {
            ancienne.style_context().remove_class("icone-choisie");
        }
        if let Some(w) = w {
            classe(w, "icone-choisie");
            *self.choisie.borrow_mut() = Some(w.clone());
        }
    }

    fn ouvrir(&self, chemin: &Path) {
        if est_raccourci(chemin) {
            if let Some(r) = lire_raccourci(chemin).filter(|r| !r.commande.is_empty()) {
                lancer(&r.commande, false);
                return;
            }
        }
        lancer(&format!("xdg-open {}", natif::guillemets(&chemin.to_string_lossy())), false);
    }

    /// Ce qu'on lâche sur le bureau : posé là où on l'a lâché, calé.
    fn recevoir(self: &Rc<Self>, x: i32, y: i32, donnees: &gtk::SelectionData) {
        for adresse in donnees.uris() {
            let Ok((source, _)) = glib::filename_from_uri(&adresse) else { continue };
            if !source.exists() {
                continue;
            }
            let Some(nom_cible) = source.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
            let cible = self.dossier.join(&nom_cible);
            if est_raccourci(&source) {
                if std::fs::copy(&source, &cible).is_err() {
                    continue;
                }
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&cible, std::fs::Permissions::from_mode(0o755));
            } else if !cible.exists() {
                let _ = std::process::Command::new("cp").arg("-a").arg(&source).arg(&cible).status();
            }
            let prises: HashSet<(i32, i32)> =
                self.positions.borrow().iter().filter(|(k, _)| **k != nom_cible).map(|(_, &p)| p).collect();
            let p = caler(x - LARGEUR_CASE / 2, y - HAUTEUR_CASE / 2, self.largeur, self.hauteur, &prises);
            self.positions.borrow_mut().insert(nom_cible, p);
            self.ecrire_positions();
        }
        self.remplir();
    }

    // ---- Les menus -------------------------------------------------------------
    fn montrer_menu(&self, gestes: Vec<(Option<&'static str>, Option<Box<dyn Fn()>>)>, ev: &gdk::EventButton) {
        let menu = gtk::Menu::new();
        for (texte, action) in gestes {
            match (texte, action) {
                (Some(t), Some(a)) => {
                    let e = gtk::MenuItem::with_label(t);
                    e.connect_activate(move |_| a());
                    menu.append(&e);
                }
                _ => menu.append(&gtk::SeparatorMenuItem::new()),
            }
        }
        menu.show_all();
        menu.popup_at_pointer(Some(&**ev));
        *self.menu.borrow_mut() = Some(menu);
    }

    fn menu_icone(self: &Rc<Self>, nom: &str, chemin: &Path, ev: &gdk::EventButton) {
        let (b1, c1) = (self.clone(), chemin.to_path_buf());
        let (b2, n2, c2) = (self.clone(), nom.to_string(), chemin.to_path_buf());
        let (b3, n3, c3) = (self.clone(), nom.to_string(), chemin.to_path_buf());
        self.montrer_menu(
            vec![
                (Some("Ouvrir"), Some(Box::new(move || b1.ouvrir(&c1)))),
                (Some("Renommer"), Some(Box::new(move || b2.renommer(&n2, &c2)))),
                (Some("Supprimer"), Some(Box::new(move || b3.supprimer(&n3, &c3)))),
            ],
            ev,
        );
    }

    fn clic_sur_le_vide(self: &Rc<Self>, ev: &gdk::EventButton) -> glib::Propagation {
        if ev.button() != 3 {
            self.selectionner(None);
            return glib::Propagation::Proceed;
        }
        let (b1, b2) = (self.clone(), self.clone());
        let dossier = self.dossier.clone();
        self.montrer_menu(
            vec![
                (Some("Nouveau dossier"), Some(Box::new(move || b1.nouveau_dossier()))),
                (Some("Nouveau fichier texte"), Some(Box::new(move || b2.nouveau_fichier()))),
                (Some("Ouvrir le dossier Bureau"), Some(Box::new(move || {
                    lancer(&format!("xdg-open {}", natif::guillemets(&dossier.to_string_lossy())), false)
                }))),
                (None, None),
                (Some("Changer le fond et les couleurs"), Some(Box::new(|| lancer("grenos-parametres apparence", false)))),
                (Some("Réglages"), Some(Box::new(|| lancer("grenos-parametres", false)))),
            ],
            ev,
        );
        glib::Propagation::Stop
    }

    /// Une petite fenêtre qui demande un nom. Entrée valide, Échap annule.
    fn demander(&self, titre: &str, valeur: &str) -> Option<String> {
        let boite = gtk::Dialog::with_buttons(
            Some(titre),
            Some(&self.fenetre),
            gtk::DialogFlags::MODAL,
            &[("Annuler", gtk::ResponseType::Cancel), ("Valider", gtk::ResponseType::Ok)],
        );
        let champ = gtk::Entry::new();
        champ.set_text(valeur);
        champ.set_activates_default(true);
        boite.set_default_response(gtk::ResponseType::Ok);
        let contenu = boite.content_area();
        contenu.set_border_width(16);
        contenu.set_spacing(10);
        let l = gtk::Label::new(Some(titre));
        l.set_xalign(0.0);
        contenu.pack_start(&l, false, false, 0);
        contenu.pack_start(&champ, false, false, 0);
        boite.show_all();
        let reponse = boite.run();
        let texte = champ.text().trim().to_string();
        boite.close();
        (reponse == gtk::ResponseType::Ok && !texte.is_empty() && !texte.contains('/')).then_some(texte)
    }

    fn nouveau_dossier(&self) {
        if let Some(n) = self.demander("Nom du nouveau dossier", "Nouveau dossier") {
            let _ = std::fs::create_dir_all(self.dossier.join(n));
        }
    }

    fn nouveau_fichier(&self) {
        if let Some(n) = self.demander("Nom du nouveau fichier", "Nouveau fichier.txt") {
            let chemin = self.dossier.join(n);
            if !chemin.exists() {
                let _ = std::fs::write(chemin, "");
            }
        }
    }

    fn renommer(&self, nom: &str, chemin: &Path) {
        if let Some(nouveau) = self.demander("Nouveau nom", nom).filter(|n| n != nom) {
            if std::fs::rename(chemin, self.dossier.join(&nouveau)).is_ok() {
                let p = self.positions.borrow_mut().remove(nom).unwrap_or((MARGE, MARGE));
                self.positions.borrow_mut().insert(nouveau, p);
                self.ecrire_positions();
            }
        }
    }

    fn supprimer(&self, nom: &str, chemin: &Path) {
        let question = gtk::MessageDialog::new(
            Some(&self.fenetre),
            gtk::DialogFlags::MODAL,
            gtk::MessageType::Question,
            gtk::ButtonsType::OkCancel,
            &format!("Supprimer « {nom} » ?"),
        );
        question.set_secondary_text(Some("Cette suppression est définitive."));
        let reponse = question.run();
        question.close();
        if reponse != gtk::ResponseType::Ok {
            return;
        }
        let fait = if chemin.is_dir() { std::fs::remove_dir_all(chemin) } else { std::fs::remove_file(chemin) };
        if fait.is_ok() {
            self.positions.borrow_mut().remove(nom);
            self.ecrire_positions();
        }
    }

    // ---- Rester à jour ---------------------------------------------------------
    /// Le dossier est surveillé : ce qu'on y dépose apparaît tout seul. Une
    /// rafale (copie d'un dossier entier) ne refait la grille qu'une fois.
    fn surveiller(self: &Rc<Self>) {
        let fichier = gio::File::for_path(&self.dossier);
        let Ok(moniteur) = fichier.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) else { return };
        let b = self.clone();
        moniteur.connect_changed(move |_, _, _, _| {
            if b.a_refaire.replace(true) {
                return;
            }
            let b2 = b.clone();
            glib::timeout_add_local_once(Duration::from_millis(150), move || {
                b2.a_refaire.set(false);
                b2.remplir();
            });
        });
        *self.moniteur.borrow_mut() = Some(moniteur);
    }
}

fn main() {
    if gtk::init().is_err() {
        eprintln!("grenos-bureau-natif : pas d'écran X");
        std::process::exit(1);
    }
    natif::nommer("grenos-bureau");
    let style = natif::Style::poser(STYLE);
    let bureau = Bureau::nouveau();
    bureau.fenetre.show_all();
    let dossier = bureau.dossier.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    dire(&format!("bureau pret, {} icones dans {} (natif)", bureau.nombre.get(), dossier));
    // Ce que la machine vérifie sans souris : que CHAQUE icône tombe sur une case.
    let positions = bureau.positions.borrow().clone();
    let sur_grille = positions
        .values()
        .filter(|&&(x, y)| {
            let (c, l) = case_de(x, y, bureau.largeur, bureau.hauteur);
            coin_de(c, l) == (x, y)
        })
        .count();
    dire(&format!(
        "bureau : grille {LARGEUR_CASE}x{HAUTEUR_CASE}, {sur_grille}/{} icone(s) calee(s), {} recalee(s)",
        positions.len(),
        bureau.recalees.get()
    ));

    // `grenos-fond` prévient d'un changement de thème ou de fond d'écran.
    let b = bureau.clone();
    natif::sur_signal(natif::SIGUSR1, move || {
        style.repeindre();
        b.repeindre();
    });
    gtk::main();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_grille_cale_au_plus_proche() {
        let (l, h) = (1280, 720);
        assert_eq!(caler(30, 30, l, h, &HashSet::new()), coin_de(0, 0));
        // Aux neuf dixièmes d'une case, on descend.
        assert_eq!(caler(MARGE, MARGE + HAUTEUR_CASE * 9 / 10, l, h, &HashSet::new()), coin_de(0, 1));
    }

    #[test]
    fn une_case_ne_prend_qu_une_icone() {
        let (l, h) = (1280, 720);
        let mut prises = HashSet::new();
        prises.insert(coin_de(0, 0));
        let p = caler(MARGE, MARGE, l, h, &prises);
        assert_ne!(p, coin_de(0, 0));
        assert!(p == coin_de(0, 1) || p == coin_de(1, 0));
    }

    #[test]
    fn la_grille_reste_dans_l_ecran() {
        let (l, h) = (800, 600);
        let (cmax, lmax) = derniere_case(l, h);
        assert_eq!(case_de(10_000, 10_000, l, h), (cmax, lmax));
        let (x, y) = coin_de(cmax, lmax);
        assert!(x + LARGEUR_CASE <= l && y + HAUTEUR_CASE <= h - BARRE);
    }
}
