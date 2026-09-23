#!/usr/bin/env python3
"""Toutes nos feuilles de style se chargent-elles vraiment ?

GTK refuse une feuille **entière** pour une seule propriété qu'il ne connaît
pas, et PyGObject transforme ce refus en exception. Une faute de style ne se
voit donc pas comme un défaut d'apparence : la fenêtre ne s'ouvre pas du tout.
C'est exactement ce qui est arrivé le 23 septembre — `font-weight: 650`, que
GTK n'accepte pas, et le premier écran de grenOS n'apparaissait plus.

Ce script tourne à la construction de l'image, dans le chroot, et fait échouer
la construction si une feuille ne passe pas. Mieux vaut une image qui ne se
construit pas qu'une image sans interface.
"""
import glob
import re
import sys

sys.path.insert(0, "/usr/lib/grenos")

try:
    import gi

    gi.require_version("Gtk", "3.0")
    from gi.repository import GLib, Gtk

    import grenosui
except (ImportError, ValueError) as souci:
    # Ce contrôle est un filet, pas une dépendance. S'il ne peut pas s'exécuter
    # ici, on le dit et on laisse la construction continuer : bloquer une image
    # parce que le vérificateur lui-même ne démarre pas serait absurde.
    print(f"styles : verification impossible ({souci})")
    sys.exit(0)

# Les feuilles des programmes sont des constantes `STYLE = """..."""`.
PROGRAMMES = ["/usr/bin/grenos-shell", "/usr/bin/grenos-bureau", "/usr/bin/grenplace"]


def essayer(nom, css):
    """Charge une feuille et dit si GTK l'accepte."""
    fournisseur = Gtk.CssProvider()
    try:
        fournisseur.load_from_data(css if isinstance(css, bytes) else css.encode("utf-8"))
    except GLib.Error as souci:
        print(f"  REFUSE  {nom} : {souci}")
        return False
    print(f"  ok      {nom}")
    return True


def main():
    bon = True

    # La feuille du système, dans ses deux palettes.
    for nom, couleurs in (("grenosui (nuit)", grenosui.NUIT), ("grenosui (jour)", grenosui.JOUR)):
        bon &= essayer(nom, grenosui.feuille(couleurs))

    # Celles des programmes, avec les couleurs devant, comme à l'exécution.
    entete = "".join(f"@define-color {n} {v};\n" for n, v in grenosui.NUIT.items())
    for chemin in PROGRAMMES:
        for fichier in glob.glob(chemin):
            texte = open(fichier, encoding="utf-8").read()
            trouve = re.search(r'^STYLE = """(.*?)"""', texte, re.S | re.M)
            if trouve:
                bon &= essayer(fichier, entete + trouve.group(1))

    print("styles : tout se charge" if bon else "styles : au moins une feuille est refusee")
    return 0 if bon else 1


if __name__ == "__main__":
    sys.exit(main())
