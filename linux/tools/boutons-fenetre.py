#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Les boutons des barres de titre : Réduire, Agrandir, Fermer.

POURQUOI CE FICHIER EXISTE

Notre thème openbox réglait soigneusement la couleur de chaque bouton dans
chacun de ses six états — et ne livrait **aucun dessin**. Openbox retombe alors
sur ses bitmaps internes, qui font 6×6 pixels. À cette taille, sa croix n'est
plus une croix : mesuré sur la capture du 28 septembre, c'est un carré plein de
6×6 avec une encoche au milieu de chacun de ses quatre côtés. Aucune diagonale.
À côté, Agrandir est un carré évidé propre — donc les deux boutons ont la même
taille, la même couleur et à peu près la même silhouette.

C'est le glyphe le plus répété de tout le système, et Fermer est le seul bouton
qu'on ne doit jamais atteindre par erreur.

Trouvé en agrandissant une capture, pas en relisant du code : le themerc est
juste, complet et bien écrit. Il ne lui manquait que les images.

LE FORMAT

Openbox lit des XBM — un bitmap d'un bit par pixel, rangé en octets de poids
faible en premier, chaque ligne alignée sur un octet. On dessine à 12×12 au
lieu de 6×6 : la barre de titre fait 24 pixels de haut, il y a la place, et un
trait de deux pixels reste net quand il est calculé pour deux pixels.

    python3 linux/tools/boutons-fenetre.py <dossier du thème openbox-3>
"""
import os
import sys

TAILLE = 10


def xbm(nom, points):
    """Un XBM : un bit par pixel, octets de poids faible en premier."""
    octets_par_ligne = (TAILLE + 7) // 8
    donnees = bytearray(octets_par_ligne * TAILLE)
    for x, y in points:
        if 0 <= x < TAILLE and 0 <= y < TAILLE:
            donnees[y * octets_par_ligne + x // 8] |= 1 << (x % 8)
    lignes = ", ".join("0x%02x" % o for o in donnees)
    return ("#define %s_width %d\n#define %s_height %d\n"
            "static unsigned char %s_bits[] = {\n   %s };\n"
            % (nom, TAILLE, nom, TAILLE, nom, lignes))


def croix():
    """Une vraie croix, deux traits de 2 px, de coin à coin avec une marge.

    La marge compte : un X qui touche les bords se confond avec le carré
    d'Agrandir. Les diagonales, elles, sont ce qui distingue les deux
    d'un seul coup d'œil — c'est tout ce qu'on demande à une icône de barre.
    """
    points = set()
    for i in range(1, TAILLE - 1):
        for e in (0, 1):
            points.add((min(i + e, TAILLE - 1), i))
            points.add((min(TAILLE - 1 - i + e, TAILLE - 1), i))
    return points


def carre():
    """Agrandir : un cadre creux de 2 px, comme une fenêtre vue de loin."""
    points = set()
    for i in range(1, TAILLE - 1):
        for e in (0, 1):
            points.add((i, 1 + e))                 # haut
            points.add((i, TAILLE - 2 - e))        # bas
            points.add((1 + e, i))                 # gauche
            points.add((TAILLE - 2 - e, i))        # droite
    return points


def deux_carres():
    """Restaurer : deux cadres décalés, le geste inverse d'Agrandir."""
    points = set()
    for i in range(0, TAILLE - 4):
        for e in (0, 1):
            points.add((i, 4 + e))
            points.add((i, TAILLE - 1 - e))
            points.add((0 + e, 4 + i))
            points.add((TAILLE - 5 - e, 4 + i))
    for i in range(4, TAILLE):
        points.add((i, 0))
        points.add((i, 1))
        points.add((TAILLE - 1, i - 4))
        points.add((TAILLE - 2, i - 4))
    return points


def trait():
    """Réduire : un trait de 2 px, posé bas, centré à l'œil comme à la règle."""
    points = set()
    for x in range(2, TAILLE - 2):
        points.add((x, TAILLE - 4))
        points.add((x, TAILLE - 3))
    return points


def point():
    """La pastille des menus, et le bouton « sur tous les bureaux »."""
    points = set()
    centre = (TAILLE - 1) / 2
    for y in range(TAILLE):
        for x in range(TAILLE):
            if (x - centre) ** 2 + (y - centre) ** 2 <= 8:
                points.add((x, y))
    return points


def ombre():
    """Enrouler : un trait en haut, la fenêtre repliée sous sa barre."""
    points = set()
    for x in range(2, TAILLE - 2):
        points.add((x, 2))
        points.add((x, 3))
    return points


# Chaque bouton, et tous ses états. Openbox lit un fichier par état ; livrer
# les variantes évite de dépendre de son comportement de repli, que personne
# ici n'a vérifié à la source — et une supposition non vérifiée est exactement
# ce qui a produit le bouton d'aujourd'hui.
BASES = {
    "close": croix,
    "max": carre,
    "max_toggled": deux_carres,
    "iconify": trait,
    "desk": point,
    "desk_toggled": point,
    "shade": ombre,
    "shade_toggled": ombre,
    "bullet": point,
}
ETATS = ("", "_hover", "_pressed", "_disabled")


def main():
    dossier = sys.argv[1] if len(sys.argv) > 1 else "."
    os.makedirs(dossier, exist_ok=True)
    ecrits = 0
    for base, dessin in BASES.items():
        points = dessin()
        for etat in ETATS:
            nom = base + etat
            chemin = os.path.join(dossier, nom + ".xbm")
            with open(chemin, "w", encoding="ascii") as fichier:
                fichier.write(xbm(nom, points))
            ecrits += 1
    print("boutons de fenetre : %d fichiers XBM en %dx%d dans %s"
          % (ecrits, TAILLE, TAILLE, dossier))
    return 0


if __name__ == "__main__":
    sys.exit(main())
