"""Ce qui donne son visage à grenOS : les couleurs, le texte, les fenêtres.

Toutes les fenêtres du système — l'accueil, les réglages, le magasin, la barre,
l'extinction — passent par ici. Un seul endroit décide de la teinte du fond, du
rayon des coins et de la façon dont un bouton réagit au survol ; changer le
thème, c'est changer ce fichier, et tout le système suit.

Le jour et la nuit sont deux palettes, pas deux thèmes : le reste du code ne
connaît que des noms de rôles (fond, texte, accent), jamais des codes couleur.
"""
import os
import subprocess

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, Gtk  # noqa: E402

REGLAGES = os.path.expanduser("~/.config/grenos")
IDENTITE = "/etc/grenos/identite"

NUIT = {
    "fond": "#0d1220",
    "fond-haut": "#131a2b",
    "fond-creux": "#0a0e1a",
    "bord": "#1e2942",
    "texte": "#e6ebf5",
    "texte-faible": "#8e9ab0",
    "accent": "#2f7df6",
    "accent-clair": "#5b9bff",
    "survol": "#1b2740",
    "danger": "#e05260",
}

JOUR = {
    "fond": "#f4f6fb",
    "fond-haut": "#ffffff",
    "fond-creux": "#e8ecf4",
    "bord": "#d3dae7",
    "texte": "#121826",
    "texte-faible": "#5b6678",
    "accent": "#1f6fe5",
    "accent-clair": "#4b8ef0",
    "survol": "#e3e9f5",
    "danger": "#c4353f",
}

MODELE = """
* {
    font-family: "Inter", "DejaVu Sans", sans-serif;
    font-size: 14px;
}

window, .fenetre { background-color: @fond; color: @texte; }

.titre   { font-size: 26px; font-weight: 700; color: @texte; }
.sous    { font-size: 14px; color: @texte-faible; }
.section { font-size: 12px; font-weight: 700; color: @texte-faible; }

.carte {
    background-color: @fond-haut;
    border: 1px solid @bord;
    border-radius: 14px;
    padding: 16px;
}

/* `background-image: none` n'est pas une coquetterie : le thème GTK du
   système peint ses boutons avec une image de dégradé, et une image couvre la
   couleur de fond. Sans cette ligne, tous nos boutons restent gris. */
button, .bouton {
    color: @texte;
    background-color: @fond-haut;
    background-image: none;
    border: 1px solid @bord;
    border-radius: 10px;
    padding: 9px 16px;
    box-shadow: none;
    text-shadow: none;
    transition: background-color 130ms ease, border-color 130ms ease;
}
button:hover, .bouton:hover {
    background-color: @survol;
    background-image: none;
    border-color: @accent;
}
button:active { background-color: @accent; background-image: none; color: #ffffff; }
button:disabled {
    color: @texte-faible;
    background-color: @fond-creux;
    background-image: none;
}

.principal {
    background-color: @accent;
    background-image: none;
    color: #ffffff;
    border: 1px solid @accent;
    font-weight: 600;
}
.principal:hover {
    background-color: @accent-clair;
    background-image: none;
    border-color: @accent-clair;
}
.principal:disabled { background-color: @fond-creux; color: @texte-faible; }

.discret { background: transparent; border-color: transparent; }
.discret:hover { background-color: @survol; border-color: @bord; }

.danger { color: @danger; }
.danger:hover { background-color: @danger; color: #ffffff; border-color: @danger; }

entry {
    background-color: @fond-creux;
    background-image: none;
    color: @texte;
    border: 1px solid @bord;
    border-radius: 10px;
    padding: 10px 12px;
    caret-color: @accent;
}
entry:focus { border-color: @accent; }

.apercu {
    font-family: "Hack", monospace;
    color: @accent-clair;
    font-size: 15px;
}

.erreur { color: @danger; font-size: 13px; }

scrollbar { background-color: transparent; }
scrollbar slider {
    background-color: @bord;
    border-radius: 8px;
    min-width: 8px;
    min-height: 30px;
}
scrollbar slider:hover { background-color: @accent; }

separator { background-color: @bord; min-height: 1px; min-width: 1px; }

switch { background-color: @fond-creux; border: 1px solid @bord; }
switch:checked { background-color: @accent; border-color: @accent; }
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


def habiller(couleurs=None):
    """Applique le style à tout l'écran : toute fenêtre ouverte ensuite le suit."""
    fournisseur = Gtk.CssProvider()
    fournisseur.load_from_data(feuille(couleurs))
    Gtk.StyleContext.add_provider_for_screen(
        Gdk.Screen.get_default(), fournisseur, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
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


def carte(*enfants, espace=10):
    """Un bloc encadré : c'est l'unité de mise en page de tout le système."""
    boite = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=espace)
    boite.get_style_context().add_class("carte")
    for enfant in enfants:
        boite.pack_start(enfant, False, False, 0)
    return boite


def bouton(texte, action=None, genre=""):
    """Un bouton, son style et ce qu'il fait."""
    widget = Gtk.Button(label=texte)
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
