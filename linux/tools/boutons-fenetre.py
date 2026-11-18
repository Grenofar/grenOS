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

TAILLE = 12


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


def disque():
    """Le rond plein des trois boutons « feux » (17 novembre).

    Grenofar voulait des barres de titre « arrondies et propres ». Trois
    ronds de couleur se lisent d'un coup d'œil, de loin, sans avoir à
    déchiffrer un glyphe de 12 pixels : rouge pour fermer, ambre pour
    réduire, vert pour agrandir. Le glyphe n'apparaît qu'au survol, creusé
    dans le rond — le geste se confirme au moment où on va le faire.
    """
    points = set()
    centre = (TAILLE - 1) / 2
    rayon2 = (TAILLE / 2) ** 2 - 0.5
    for y in range(TAILLE):
        for x in range(TAILLE):
            if (x - centre) ** 2 + (y - centre) ** 2 <= rayon2:
                points.add((x, y))
    return points


def petite_croix():
    """La croix creusée dans le rond rouge : deux diagonales de 6 px."""
    points = set()
    for i in range(3, TAILLE - 3):
        points.add((i, i))
        points.add((TAILLE - 1 - i, i))
    return points


def petit_trait():
    """Le trait creusé dans le rond ambre."""
    milieu = TAILLE // 2
    return {(x, y) for x in range(3, TAILLE - 3) for y in (milieu - 1, milieu)}


def petit_plus():
    """Le plus creusé dans le rond vert : agrandir, ou rendre sa taille."""
    milieu = TAILLE // 2
    points = {(x, y) for x in range(3, TAILLE - 3) for y in (milieu - 1, milieu)}
    points |= {(x, y) for y in range(3, TAILLE - 3) for x in (milieu - 1, milieu)}
    return points


def petit_moins_plus():
    """Agrandie : le rond vert porte deux pointes qui rentrent (restaurer)."""
    milieu = TAILLE // 2
    return {(x, y) for x in range(3, TAILLE - 3) for y in (milieu - 1, milieu)} | {
        (milieu - 1, 3), (milieu, 3), (milieu - 1, TAILLE - 4), (milieu, TAILLE - 4)}


def fin_trait():
    """Réduire, à la Windows : un trait d'UN pixel, au milieu, sur 10 px."""
    milieu = TAILLE // 2
    return {(x, milieu) for x in range(1, TAILLE - 1)}


def fin_carre():
    """Agrandir : un carré d'un pixel de trait, coins à peine adoucis."""
    points = set()
    for i in range(2, TAILLE - 2):
        points |= {(i, 1), (i, TAILLE - 2), (1, i), (TAILLE - 2, i)}
    return points


def fin_restaurer():
    """Restaurer : deux carrés fins décalés, celui de devant entier."""
    points = set()
    # Devant : de (1,3) à (8,10).
    for i in range(1, 9):
        points |= {(i, 3), (i, 10)}
    for j in range(3, 11):
        points |= {(1, j), (8, j)}
    # Derrière : seulement ce qui dépasse, en haut et à droite.
    for i in range(3, 11):
        points.add((i, 1))
    for j in range(1, 9):
        points.add((10, j))
    points |= {(3, 2), (10, 1)}
    return points


def fine_croix():
    """Fermer : deux diagonales d'un pixel, de coin à coin, sur 10 px."""
    points = set()
    for i in range(1, TAILLE - 1):
        points.add((i, i))
        points.add((TAILLE - 1 - i, i))
    return points


# LES BOUTONS DE WINDOWS (19 novembre). Grenofar : « fais comme sur Windows,
# fais trop cheap ». Les trois ronds de couleur du 17 novembre faisaient
# jouet ; on revient à ce que tout le monde connaît : trois glyphes fins
# (—, □, ✕), de la couleur du texte, et au survol un fond — gris clair pour
# Réduire et Agrandir, ROUGE pour Fermer (themerc). Le même dessin dans tous
# les états : c'est le fond qui change, pas le glyphe.
FEUX = {
    "close": fine_croix,
    "max": fin_carre,
    "max_toggled": fin_restaurer,
    "iconify": fin_trait,
}

# Les autres boutons gardent leur glyphe, dans tous leurs états. Openbox lit
# un fichier par état ; livrer les variantes évite de dépendre de son
# comportement de repli, que personne ici n'a vérifié à la source.
BASES = {
    "desk": point,
    "desk_toggled": point,
    "shade": ombre,
    "shade_toggled": ombre,
    "bullet": point,
}
ETATS = ("", "_hover", "_pressed", "_disabled")


def dessins():
    """Chaque fichier à écrire, avec ses points : {nom: points}."""
    sortie = {}
    for base, glyphe in FEUX.items():
        for etat in ETATS:
            sortie[base + etat] = glyphe()
    for base, dessin in BASES.items():
        for etat in ETATS:
            sortie[base + etat] = dessin()
    # Les anciens glyphes restent appelables (croix, carre…) : l'aperçu les
    # montre encore, et un retour en arrière ne coûte qu'une ligne.
    return sortie


def apercu(points):
    """Le dessin en caractères, pour le relire sans serveur X."""
    return "\n".join("".join("#" if (x, y) in points else "."
                             for x in range(TAILLE)) for y in range(TAILLE))


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--apercu":
        for nom, points in dessins().items():
            if not nom.endswith(("_pressed", "_disabled")):
                print(nom + "\n" + apercu(points) + "\n")
        return 0
    dossier = sys.argv[1] if len(sys.argv) > 1 else "."
    os.makedirs(dossier, exist_ok=True)
    ecrits = 0
    for nom, points in dessins().items():
        chemin = os.path.join(dossier, nom + ".xbm")
        with open(chemin, "w", encoding="ascii") as fichier:
            fichier.write(xbm(nom, points))
        ecrits += 1
    print("boutons de fenetre : %d fichiers XBM en %dx%d dans %s"
          % (ecrits, TAILLE, TAILLE, dossier))
    return 0


if __name__ == "__main__":
    sys.exit(main())
