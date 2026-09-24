#!/usr/bin/env python3
"""Les icônes de grenOS, calculées comme les fonds d'écran.

Ce qui est à nous doit porter notre marque. GrenPlace et l'explorateur
portaient l'icône générique d'un thème, ce qui les faisait ressembler à
n'importe quoi ; et l'icône `grenos`, que **toutes** nos fenêtres demandent,
n'existait nulle part — la barre des tâches montrait donc, pour « Bienvenue
dans grenOS », un dossier avec une maison. Celles-ci sont dessinées par ce script,
en numpy et zlib, sans bibliothèque d'images — comme les fonds d'écran, elles
sont produites à la construction de l'ISO et jamais sur la machine de
quelqu'un.

Le dessin est volontairement simple : à seize pixels de côté, dans une barre
des tâches, seules les grandes formes se voient. Un sac pour la boutique, un
dossier pour les fichiers, aux couleurs du système.

    python3 icones.py <dossier> [taille]
"""
import os
import struct
import sys
import zlib

import numpy as np

ACCENT = (0x3B, 0x82, 0xF6)
ACCENT_CLAIR = (0x60, 0xA5, 0xFA)
SECOND = (0x7C, 0x4D, 0xF0)
CLAIR = (0xEE, 0xF2, 0xFA)


def png(chemin, pixels):
    """Écrit un tableau (hauteur, largeur, 4) d'octets en PNG avec transparence."""
    hauteur, largeur, _ = pixels.shape
    brut = np.concatenate(
        [np.zeros((hauteur, 1), dtype=np.uint8), pixels.reshape(hauteur, largeur * 4)],
        axis=1)

    def bloc(genre, donnees):
        return (struct.pack('>I', len(donnees)) + genre + donnees
                + struct.pack('>I', zlib.crc32(genre + donnees) & 0xFFFFFFFF))

    entete = struct.pack('>IIBBBBB', largeur, hauteur, 8, 6, 0, 0, 0)
    corps = zlib.compress(brut.tobytes(), 9)
    with open(chemin, 'wb') as fichier:
        fichier.write(b'\x89PNG\r\n\x1a\n' + bloc(b'IHDR', entete)
                      + bloc(b'IDAT', corps) + bloc(b'IEND', b''))
    print(f'{chemin}: {largeur}x{largeur}')


def toile(taille):
    """Une toile transparente, et les coordonnées de 0 à 1."""
    image = np.zeros((taille, taille, 4), dtype=float)
    ys, xs = np.mgrid[0:taille, 0:taille].astype(float)
    return image, xs / (taille - 1), ys / (taille - 1)


def poser(image, masque, couleur, force=1.0):
    """Pose une couleur là où le masque vaut 1, en gardant les bords doux."""
    alpha = np.clip(masque, 0.0, 1.0)[..., None] * force
    teinte = np.array(couleur + (255,), dtype=float)
    image *= 1.0 - alpha
    image += teinte * alpha


def rectangle(u, v, x0, y0, x1, y1, rayon=0.0, flou=0.01):
    """Un rectangle aux coins arrondis, en coordonnées de 0 à 1."""
    cx = np.clip(np.minimum(u - x0, x1 - u), -1, 1)
    cy = np.clip(np.minimum(v - y0, y1 - v), -1, 1)
    if rayon <= 0:
        return np.clip(np.minimum(cx, cy) / flou, 0.0, 1.0)
    # Distance au rectangle réduit, puis arrondi par le rayon.
    dx = np.maximum(np.abs(u - (x0 + x1) / 2) - ((x1 - x0) / 2 - rayon), 0.0)
    dy = np.maximum(np.abs(v - (y0 + y1) / 2) - ((y1 - y0) / 2 - rayon), 0.0)
    return np.clip((rayon - np.hypot(dx, dy)) / flou, 0.0, 1.0)


def grenplace(chemin, taille):
    """Le sac de GrenPlace : un cabas, anse comprise."""
    image, u, v = toile(taille)

    corps = rectangle(u, v, 0.16, 0.34, 0.84, 0.90, rayon=0.10)
    # Un dégradé du bleu vers le violet, de gauche à droite.
    melange = np.clip((u - 0.16) / 0.68, 0.0, 1.0)
    couleur = (np.array(ACCENT, dtype=float)[None, None, :] * (1 - melange[..., None])
               + np.array(SECOND, dtype=float)[None, None, :] * melange[..., None])
    alpha = corps[..., None]
    image[..., :3] = image[..., :3] * (1 - alpha) + couleur * alpha
    image[..., 3:] = image[..., 3:] * (1 - alpha) + 255.0 * alpha

    # L'anse : un arc épais, ouvert vers le bas.
    # Un anneau, pas un disque : c'est l'epaisseur du trait qui compte, et ma
    # premiere version remplissait tout le dome.
    rayon = np.hypot(u - 0.5, v - 0.40)
    anse = (np.clip((0.042 - np.abs(rayon - 0.21)) / 0.014, 0.0, 1.0)
            * (v < 0.40))
    poser(image, anse, CLAIR, 0.95)

    # Une pastille claire au centre du sac : un repère qui se voit petit.
    point = np.clip((0.055 - np.hypot(u - 0.5, v - 0.62)) / 0.012, 0.0, 1.0)
    poser(image, point, CLAIR, 0.92)

    png(chemin, np.clip(image, 0, 255).astype(np.uint8))


def fichiers(chemin, taille):
    """Le dossier de l'explorateur : une chemise, onglet compris."""
    image, u, v = toile(taille)

    onglet = rectangle(u, v, 0.12, 0.24, 0.48, 0.36, rayon=0.05)
    poser(image, onglet, ACCENT, 1.0)

    corps = rectangle(u, v, 0.10, 0.32, 0.90, 0.82, rayon=0.09)
    melange = np.clip((v - 0.32) / 0.50, 0.0, 1.0)
    couleur = (np.array(ACCENT_CLAIR, dtype=float)[None, None, :] * (1 - melange[..., None])
               + np.array(ACCENT, dtype=float)[None, None, :] * melange[..., None])
    alpha = corps[..., None]
    image[..., :3] = image[..., :3] * (1 - alpha) + couleur * alpha
    image[..., 3:] = image[..., 3:] * (1 - alpha) + 255.0 * alpha

    # Une feuille qui dépasse : sans elle, ce n'est qu'un rectangle bleu.
    feuille = rectangle(u, v, 0.30, 0.40, 0.70, 0.62, rayon=0.03)
    poser(image, feuille, CLAIR, 0.90)

    png(chemin, np.clip(image, 0, 255).astype(np.uint8))


def son(chemin, taille, niveau=2):
    """Le haut-parleur de la barre : un trapeze et des ondes.

    Dessine plutot qu'emprunte a un emoji : un emoji depend de la police
    installee, change de style d'une machine a l'autre, et ne se teinte pas.
    Celui-ci est clair, plat, et lisible a seize pixels de cote.

    `niveau` donne le nombre d'ondes : 0 pour le silence (une croix), 1 ou 2
    pour un son faible ou fort.
    """
    image, u, v = toile(taille)

    # Le corps du haut-parleur : un carre, puis le pavillon en triangle.
    corps = rectangle(u, v, 0.18, 0.40, 0.34, 0.60, rayon=0.03)
    poser(image, corps, CLAIR, 1.0)

    # Le pavillon : tout ce qui est a droite de 0.30 et dans le triangle.
    largeur = np.clip((u - 0.30) / 0.22, 0.0, 1.0)
    dans = (np.abs(v - 0.50) < 0.06 + largeur * 0.26) & (u > 0.30) & (u < 0.53)
    poser(image, dans.astype(float), CLAIR, 1.0)

    if niveau <= 0:
        # Le silence : une croix, plutot qu'un haut-parleur barre qu'on ne
        # distingue pas d'un haut-parleur normal en petit.
        for pente in (1.0, -1.0):
            trait = np.clip(
                (0.035 - np.abs((v - 0.50) - pente * (u - 0.72))) / 0.014, 0.0, 1.0)
            trait *= ((u > 0.60) & (u < 0.86)).astype(float)
            poser(image, trait, CLAIR, 0.95)
    else:
        rayon = np.hypot((u - 0.40) * 1.0, (v - 0.50) * 1.0)
        for index, (distance, epaisseur) in enumerate(((0.23, 0.022), (0.37, 0.022))):
            if index >= niveau:
                break
            onde = np.clip((epaisseur - np.abs(rayon - distance)) / 0.013, 0.0, 1.0)
            # Seulement la partie droite : une onde complete ferait un anneau.
            onde *= ((u > 0.52) & (np.abs(v - 0.50) < distance * 0.85)).astype(float)
            poser(image, onde, CLAIR, 0.95 - index * 0.12)

    png(chemin, np.clip(image, 0, 255).astype(np.uint8))


def nuage(chemin, taille):
    """Le jeu en nuage : un nuage, et le triangle qu'on connait."""
    image, u, v = toile(taille)

    # Trois cercles et une base plate : c'est ainsi qu'on dessine un nuage.
    forme = np.zeros_like(u)
    for cx, cy, rayon in ((0.36, 0.48, 0.15), (0.54, 0.42, 0.19), (0.68, 0.50, 0.14)):
        forme = np.maximum(forme, np.clip((rayon - np.hypot(u - cx, v - cy)) / 0.012,
                                          0.0, 1.0))
    forme = np.maximum(forme, rectangle(u, v, 0.28, 0.48, 0.74, 0.64, rayon=0.07))

    melange = np.clip((u - 0.28) / 0.46, 0.0, 1.0)
    couleur = (np.array(ACCENT, dtype=float)[None, None, :] * (1 - melange[..., None])
               + np.array(SECOND, dtype=float)[None, None, :] * melange[..., None])
    alpha = forme[..., None]
    image[..., :3] = image[..., :3] * (1 - alpha) + couleur * alpha
    image[..., 3:] = image[..., 3:] * (1 - alpha) + 255.0 * alpha

    # Le triangle de lecture, au centre du nuage.
    dans = ((u > 0.46) & (u < 0.60)
            & (np.abs(v - 0.50) < (0.60 - u) * 0.72))
    poser(image, dans.astype(float), CLAIR, 0.95)

    png(chemin, np.clip(image, 0, 255).astype(np.uint8))


def manette(chemin, taille):
    """La manette de la page Jeux : un corps large et deux poignees."""
    image, u, v = toile(taille)

    corps = rectangle(u, v, 0.16, 0.38, 0.84, 0.66, rayon=0.14)
    for cx in (0.26, 0.74):
        corps = np.maximum(corps, np.clip(
            (0.13 - np.hypot((u - cx) * 1.0, (v - 0.60) * 0.85)) / 0.012, 0.0, 1.0))

    melange = np.clip((u - 0.16) / 0.68, 0.0, 1.0)
    couleur = (np.array(ACCENT, dtype=float)[None, None, :] * (1 - melange[..., None])
               + np.array(SECOND, dtype=float)[None, None, :] * melange[..., None])
    alpha = corps[..., None]
    image[..., :3] = image[..., :3] * (1 - alpha) + couleur * alpha
    image[..., 3:] = image[..., 3:] * (1 - alpha) + 255.0 * alpha

    # La croix a gauche, deux boutons a droite.
    croix = np.maximum(rectangle(u, v, 0.26, 0.475, 0.40, 0.515, rayon=0.012),
                       rectangle(u, v, 0.31, 0.425, 0.35, 0.565, rayon=0.012))
    poser(image, croix, CLAIR, 0.95)
    for cx, cy in ((0.64, 0.46), (0.72, 0.53)):
        point = np.clip((0.035 - np.hypot(u - cx, v - cy)) / 0.010, 0.0, 1.0)
        poser(image, point, CLAIR, 0.95)

    png(chemin, np.clip(image, 0, 255).astype(np.uint8))


def marque(chemin, taille):
    """La marque de grenOS : un G clair sur un ecusson bleu-violet.

    Toutes nos fenetres demandent l'icone nommee `grenos` — et personne ne la
    dessinait, alors la barre des taches montrait ce que le fichier .desktop
    declarait a la place. Un anneau ouvert a droite et une barre : c'est ce
    qui reste lisible a seize pixels, une fois que les details ont disparu.
    """
    image, u, v = toile(taille)

    # L'ecusson, au degrade du sac de GrenPlace : la famille doit se voir.
    fond = rectangle(u, v, 0.06, 0.06, 0.94, 0.94, rayon=0.22)
    melange = np.clip((u + v - 0.12) / 1.64, 0.0, 1.0)
    couleur = (np.array(ACCENT, dtype=float)[None, None, :] * (1 - melange[..., None])
               + np.array(SECOND, dtype=float)[None, None, :] * melange[..., None])
    alpha = fond[..., None]
    image[..., :3] = image[..., :3] * (1 - alpha) + couleur * alpha
    image[..., 3:] = image[..., 3:] * (1 - alpha) + 255.0 * alpha

    # Le G : un anneau epais, ouvert vers la droite.
    rayon = np.hypot(u - 0.5, v - 0.5)
    anneau = np.clip((0.085 - np.abs(rayon - 0.255)) / 0.02, 0.0, 1.0)
    ouverture = (u > 0.52) & (v < 0.52) & (v > 0.30)
    poser(image, anneau * (~ouverture), CLAIR, 1.0)

    # La barre du G, qui rentre vers le centre : sans elle, c'est un C.
    barre = rectangle(u, v, 0.50, 0.46, 0.80, 0.585, rayon=0.03)
    poser(image, barre, CLAIR, 1.0)

    png(chemin, np.clip(image, 0, 255).astype(np.uint8))


def main():
    dossier = sys.argv[1] if len(sys.argv) > 1 else '.'
    taille = int(sys.argv[2]) if len(sys.argv) > 2 else 256
    os.makedirs(dossier, exist_ok=True)
    marque(os.path.join(dossier, 'grenos.png'), taille)
    grenplace(os.path.join(dossier, 'grenplace.png'), taille)
    fichiers(os.path.join(dossier, 'grenos-fichiers.png'), taille)
    son(os.path.join(dossier, 'grenos-son.png'), taille, niveau=2)
    son(os.path.join(dossier, 'grenos-son-faible.png'), taille, niveau=1)
    son(os.path.join(dossier, 'grenos-son-muet.png'), taille, niveau=0)
    nuage(os.path.join(dossier, 'grenos-nuage.png'), taille)
    manette(os.path.join(dossier, 'grenos-jeux.png'), taille)


if __name__ == '__main__':
    main()
