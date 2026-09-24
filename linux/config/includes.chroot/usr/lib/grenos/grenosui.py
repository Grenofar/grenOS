"""Ce qui donne son visage à grenOS : les couleurs, le texte, les fenêtres.

Toutes les fenêtres du système — l'accueil, les Réglages, GrenPlace, la barre,
le bureau — passent par ici. Un seul endroit décide de la teinte du fond, du
rayon des coins, du rythme des espacements et de la façon dont un bouton réagit
au survol ; changer le style, c'est changer ce fichier, et tout le système suit.

Deux principes tiennent tout le reste :

  - **Le jour et la nuit sont deux palettes, pas deux thèmes.** Le code ne
    connaît que des rôles — fond, texte, accent —, jamais des codes couleur.
  - **Rien de ce qui est beau ici ne coûte cher.** Pas de flou, pas de grandes
    ombres portées, pas de transparence entre fenêtres : uniquement des
    dégradés courts, des bordures et des transitions de couleur, que le
    processeur dessine sans effort. C'est ce qui permet d'avoir une interface
    soignée *et* fluide sur une machine sans accélération 3D.
"""
import os
import shlex
import subprocess
import sys

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GLib, Gtk  # noqa: E402

REGLAGES = os.path.expanduser("~/.config/grenos")
IDENTITE = "/etc/grenos/identite"

NUIT = {
    "fond": "#0b101c",
    "fond-haut": "#141b2d",
    "fond-clair": "#1a2338",
    "fond-creux": "#080c16",
    "bord": "#22304d",
    "bord-clair": "#2c3d60",
    "texte": "#eef2fa",
    "texte-faible": "#94a1ba",
    "accent": "#3b82f6",
    "accent-clair": "#60a5fa",
    "accent-sombre": "#1d4ed8",
    "second": "#7c4df0",
    "survol": "#1c2740",
    "danger": "#ef4d5e",
    "reussi": "#2bc48a",
    "ombre": "rgba(0, 0, 0, 0.45)",
}

JOUR = {
    "fond": "#f3f6fc",
    "fond-haut": "#ffffff",
    "fond-clair": "#ffffff",
    "fond-creux": "#e9eef8",
    "bord": "#d7dee9",
    "bord-clair": "#c5cfdf",
    "texte": "#0f1729",
    "texte-faible": "#5a6880",
    "accent": "#2563eb",
    "accent-clair": "#3b82f6",
    "accent-sombre": "#1e40af",
    "second": "#7c4df0",
    "survol": "#e6ecf8",
    "danger": "#d02a3a",
    "reussi": "#10916a",
    "ombre": "rgba(15, 23, 41, 0.14)",
}

MODELE = """
* {
    font-family: "Inter", "DejaVu Sans", sans-serif;
    font-size: 14px;
}

window, .fenetre { background-color: @fond; color: @texte; }

/* ---- Le texte : une échelle, et on s'y tient --------------------------- */
.titre {
    font-size: 27px;
    font-weight: 800;
    letter-spacing: -0.5px;
    color: @texte;
}
.sous { font-size: 13.5px; color: @texte-faible; }
.section {
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 1.2px;
    color: @texte-faible;
}
.entree-titre { font-weight: 600; }
.grand-chiffre { font-size: 22px; font-weight: 700; color: @accent-clair; }

/* ---- La carte : l'unité de mise en page de tout le système ------------- */
.carte {
    background-image: linear-gradient(to bottom, @fond-clair, @fond-haut);
    border: 1px solid @bord;
    border-radius: 16px;
    padding: 18px;
}
.carte:hover { border-color: @bord-clair; }

.carte-accent {
    background-image: linear-gradient(135deg, alpha(@accent, 0.18), alpha(@second, 0.14));
    border: 1px solid alpha(@accent, 0.45);
    border-radius: 18px;
    padding: 20px;
}

/* ---- Les boutons ------------------------------------------------------- */
/* `background-image: none` n'est pas une coquetterie : le thème GTK du
   système peint ses boutons avec une image de dégradé, et une image couvre la
   couleur de fond. Sans cette ligne, tous nos boutons restent gris. */
button, .bouton {
    color: @texte;
    background-color: @fond-clair;
    background-image: none;
    border: 1px solid @bord;
    border-radius: 12px;
    padding: 10px 18px;
    font-weight: 600;
    box-shadow: none;
    text-shadow: none;
    transition: background-color 140ms ease, border-color 140ms ease, color 140ms ease;
}
button:hover, .bouton:hover {
    background-color: @survol;
    background-image: none;
    border-color: @accent;
}
button:active {
    background-color: @accent-sombre;
    background-image: none;
    color: #ffffff;
    border-color: @accent-sombre;
}
button:disabled {
    color: @texte-faible;
    background-color: @fond-creux;
    background-image: none;
    border-color: @bord;
}

.principal {
    background-image: linear-gradient(to bottom, @accent-clair, @accent);
    color: #ffffff;
    border: 1px solid @accent-sombre;
}
.principal:hover {
    background-image: linear-gradient(to bottom, @accent-clair, @accent-clair);
    border-color: @accent;
}
.principal:disabled {
    background-image: none;
    background-color: @fond-creux;
    color: @texte-faible;
    border-color: @bord;
}

.discret {
    background-color: transparent;
    background-image: none;
    border-color: transparent;
    font-weight: 500;
}
.discret:hover { background-color: @survol; border-color: @bord; }

.danger { color: @danger; border-color: alpha(@danger, 0.4); }
.danger:hover { background-color: @danger; background-image: none; color: #ffffff; }

/* ---- Les champs -------------------------------------------------------- */
entry {
    background-color: @fond-creux;
    background-image: none;
    color: @texte;
    border: 1px solid @bord;
    border-radius: 12px;
    padding: 11px 14px;
    caret-color: @accent;
    transition: border-color 140ms ease;
}
entry:focus { border-color: @accent; }
entry image { color: @texte-faible; }

.apercu {
    font-family: "Hack", monospace;
    color: @accent-clair;
    font-size: 16px;
    letter-spacing: 0.3px;
}
.erreur { color: @danger; font-size: 13px; }
.reussi { color: @reussi; font-size: 13px; }

/* ---- Les listes et les onglets latéraux -------------------------------- */
stacksidebar {
    background-color: @fond-creux;
    border-right: 1px solid @bord;
}
stacksidebar list { background-color: transparent; }
stacksidebar row {
    border-radius: 10px;
    margin: 3px 8px;
    padding: 9px 12px;
    color: @texte-faible;
    transition: background-color 130ms ease, color 130ms ease;
}
stacksidebar row:hover { background-color: @survol; color: @texte; }
stacksidebar row:selected {
    background-image: linear-gradient(to right, alpha(@accent, 0.30), alpha(@accent, 0.14));
    color: @texte;
    box-shadow: inset 3px 0 0 @accent;
}
stacksidebar row label { font-weight: 600; }

/* ---- Les barres de défilement, fines et discrètes ---------------------- */
scrollbar { background-color: transparent; border: none; }
scrollbar slider {
    background-color: @bord-clair;
    border-radius: 10px;
    min-width: 8px;
    min-height: 34px;
    border: 3px solid transparent;
    background-clip: padding-box;
}
scrollbar slider:hover { background-color: @accent; }

/* ---- Le reste ---------------------------------------------------------- */
separator { background-color: @bord; min-height: 1px; min-width: 1px; }

progressbar trough {
    background-color: @fond-creux;
    border: 1px solid @bord;
    border-radius: 999px;
    min-height: 10px;
}
progressbar progress {
    background-image: linear-gradient(to right, @accent, @second);
    border-radius: 999px;
    min-height: 10px;
}

switch {
    background-color: @fond-creux;
    border: 1px solid @bord;
    border-radius: 999px;
}
switch:checked { background-color: @accent; border-color: @accent; }
switch slider { border-radius: 999px; }

scale trough {
    background-color: @fond-creux;
    border: 1px solid @bord;
    border-radius: 999px;
    min-height: 8px;
}
scale highlight {
    background-image: linear-gradient(to right, @accent, @accent-clair);
    border-radius: 999px;
}
scale slider {
    background-color: #ffffff;
    border: 1px solid @bord-clair;
    border-radius: 999px;
    min-width: 18px;
    min-height: 18px;
}

expander title { color: @texte-faible; font-weight: 600; }
expander title:hover { color: @texte; }

textview, textview text {
    background-color: @fond-creux;
    color: @texte-faible;
    font-family: "Hack", monospace;
    font-size: 12.5px;
}

tooltip {
    background-color: @fond-haut;
    border: 1px solid @bord;
    border-radius: 10px;
    color: @texte;
}

menu, .menu {
    background-color: @fond-haut;
    border: 1px solid @bord;
    border-radius: 12px;
    padding: 6px;
}
menuitem {
    border-radius: 8px;
    padding: 8px 12px;
    color: @texte;
}
menuitem:hover { background-color: @accent; color: #ffffff; }

/* L'anneau qui marque la personne : une pastille avec son initiale. */
.avatar {
    background-image: linear-gradient(135deg, @accent, @second);
    color: #ffffff;
    font-size: 19px;
    font-weight: 800;
    border-radius: 999px;
    padding: 10px 16px;
}
"""


def palette():
    """La palette du moment : nuit par défaut, jour si la personne l'a choisi."""
    try:
        with open(os.path.join(REGLAGES, "theme"), encoding="utf-8") as fichier:
            if fichier.read().strip() == "jour":
                return JOUR
    except OSError:
        pass
    return NUIT


def feuille(couleurs=None):
    """Le CSS complet, les noms de rôles remplacés par les couleurs choisies."""
    couleurs = couleurs or palette()
    entete = "".join(f"@define-color {nom} {valeur};\n" for nom, valeur in couleurs.items())
    return (entete + MODELE).encode("utf-8")


def charger(fournisseur, css):
    """Charge une feuille de style sans jamais faire tomber le programme.

    GTK refuse une feuille entière pour une seule propriété qu'il ne connaît
    pas, et PyGObject transforme ce refus en exception. Une faute de style ne
    doit pas coûter un bureau : on la signale sur la sortie d'erreur, et on
    continue sans elle. Le vrai garde-fou est ailleurs — la construction de
    l'image vérifie toutes nos feuilles et refuse de produire une image dont
    le style ne se charge pas.
    """
    try:
        fournisseur.load_from_data(css if isinstance(css, bytes) else css.encode("utf-8"))
        return True
    except GLib.Error as souci:
        print(f"grenos: style refuse par GTK : {souci}", file=sys.stderr)
        return False


def nommer():
    """Chaque programme dit son nom aux fenêtres qu'il ouvre.

    Nos programmes commencent par `#!/usr/bin/env python3`, et GTK annonçait
    donc **toutes** nos fenêtres sous la même classe : « python3 ». La barre
    des tâches, qui reconnaît une application à cette classe, les rangeait
    toutes ensemble — Bienvenue, le gestionnaire de tâches et GrenPlace dans
    une seule tuile — et aucune ne rejoignait jamais son icône épinglée.

    C'est la cause commune de deux choses qu'il avait signalées : « les apps
    ouvertes ont leur logo en bas qui n'est pas le leur », et le petit trait
    sous une application déjà épinglée qui n'apparaissait pas. Vu en comptant
    les tuiles sur une capture : quatre épinglées, et une seule pour trois
    fenêtres.

    À appeler avant d'ouvrir quoi que ce soit — la classe est lue quand la
    fenêtre est créée, pas après.
    """
    nom = os.path.basename(sys.argv[0]) or "grenos"
    GLib.set_prgname(nom)
    Gdk.set_program_class(nom)
    return nom


def habiller(couleurs=None):
    """Applique le style à tout l'écran : toute fenêtre ouverte ensuite le suit.

    Et la marque avec. `fenetre()` posait l'icône, mais une seule application
    s'en servait : les autres construisent leur fenêtre elles-mêmes, et
    portaient donc l'icône générique de GTK — dans leur barre de titre comme
    dans la barre des tâches. Le défaut se voyait sur chaque capture sans que
    personne ne le nomme. Une icône par défaut vaut pour toutes les fenêtres
    ouvertes ensuite, et chaque programme passe par ici.
    """
    nommer()
    Gtk.Window.set_default_icon_name("grenos")
    fournisseur = Gtk.CssProvider()
    charger(fournisseur, feuille(couleurs))
    Gtk.StyleContext.add_provider_for_screen(
        Gdk.Screen.get_default(), fournisseur, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
    )
    return fournisseur


def ajouter_style(css, couleurs=None):
    """Ajoute un style propre à une fenêtre, par-dessus celui du système."""
    couleurs = couleurs or palette()
    entete = "".join(f"@define-color {nom} {valeur};\n" for nom, valeur in couleurs.items())
    fournisseur = Gtk.CssProvider()
    charger(fournisseur, entete + css)
    Gtk.StyleContext.add_provider_for_screen(
        Gdk.Screen.get_default(), fournisseur, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1
    )
    return fournisseur


def identite():
    """Le nom choisi au premier démarrage : compte, nom affiché, machine."""
    valeurs = {"login": os.environ.get("USER", "grenos"), "nom": "", "machine": "grenos"}
    try:
        with open(IDENTITE, encoding="utf-8") as fichier:
            for ligne in fichier:
                if "=" in ligne:
                    cle, valeur = ligne.strip().split("=", 1)
                    valeurs[cle.strip()] = valeur.strip()
    except OSError:
        pass
    return valeurs


def adresse():
    """Ce qui s'écrit dans le terminal et sur l'écran de connexion : nom@machine."""
    valeurs = identite()
    return f"{valeurs['login']}@{valeurs['machine']}"


def lancer(commande, terminal=False):
    """Lance une commande détachée : si elle meurt, ce qui l'a lancée survit."""
    if terminal:
        commande = f"x-terminal-emulator -e {commande}"
    return subprocess.Popen(["/bin/sh", "-c", commande], start_new_session=True)


def dire(texte):
    """Dit une phrase au journal de la machine, quoi qu'elle contienne.

    Chaque programme construisait sa commande à la main, ce qui marche tant
    que le texte n'a pas d'apostrophe. Le magasin voulait dire « (livré avec
    l'image) » : cette apostrophe-là fermait la chaîne du shell au milieu de
    la phrase, et le message se serait perdu sans que rien ne le signale.
    """
    return lancer("grenos-dire " + shlex.quote(str(texte)))


def icone(nom, taille=24):
    """L'image d'un nom d'icône du système, ou rien si le thème ne l'a pas."""
    if not nom:
        return None
    try:
        pixbuf = Gtk.IconTheme.get_default().load_icon(
            nom, taille, Gtk.IconLookupFlags.FORCE_SIZE)
        return Gtk.Image.new_from_pixbuf(pixbuf)
    except GLib.Error:
        return None


def titre(texte, sous_texte=""):
    """Le bloc de tête d'une fenêtre : un titre, et une phrase qui explique."""
    boite = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
    etiquette = Gtk.Label(label=texte, xalign=0)
    etiquette.get_style_context().add_class("titre")
    boite.pack_start(etiquette, False, False, 0)
    if sous_texte:
        sous = Gtk.Label(label=sous_texte, xalign=0)
        sous.get_style_context().add_class("sous")
        sous.set_line_wrap(True)
        boite.pack_start(sous, False, False, 0)
    return boite


def avatar(nom, taille=20):
    """La pastille à l'initiale : ce qui rend une machine personnelle."""
    lettre = (nom or "?").strip()[:1].upper() or "?"
    etiquette = Gtk.Label(label=lettre)
    etiquette.get_style_context().add_class("avatar")
    etiquette.set_size_request(taille * 2, taille * 2)
    return etiquette


def carte(*enfants, espace=10, genre="carte"):
    """Un bloc encadré : l'unité de mise en page de tout le système."""
    boite = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=espace)
    boite.get_style_context().add_class(genre)
    for enfant in enfants:
        boite.pack_start(enfant, False, False, 0)
    return boite


def bouton(texte, action=None, genre="", nom_icone="", taille_icone=18):
    """Un bouton, son style, son icône, et ce qu'il fait."""
    widget = Gtk.Button()
    contenu = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=9)
    contenu.set_halign(Gtk.Align.CENTER)
    image = icone(nom_icone, taille_icone) if nom_icone else None
    if image is not None:
        contenu.pack_start(image, False, False, 0)
    if texte:
        contenu.pack_start(Gtk.Label(label=texte), False, False, 0)
    widget.add(contenu)
    for classe in genre.split():
        widget.get_style_context().add_class(classe)
    if action is not None:
        widget.connect("clicked", lambda _: action())
    return widget


def fenetre(nom, largeur=820, hauteur=600):
    """Une fenêtre ordinaire de grenOS, déjà habillée et centrée."""
    habiller()
    cadre = Gtk.Window(title=nom)
    cadre.set_default_size(largeur, hauteur)
    cadre.set_position(Gtk.WindowPosition.CENTER)
    cadre.set_icon_name("grenos")
    cadre.connect("destroy", Gtk.main_quit)
    return cadre
