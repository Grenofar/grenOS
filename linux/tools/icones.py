#!/usr/bin/env python3
"""Les icônes de grenOS, calculées comme les fonds d'écran.

Deux applications sont à nous et n'avaient pas de logo : GrenPlace et
l'explorateur de fichiers portaient l'icône générique d'un thème, ce qui les
faisait ressembler à n'importe quoi. Celles-ci sont dessinées par ce script,
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


def main():
    dossier = sys.argv[1] if len(sys.argv) > 1 else '.'
    taille = int(sys.argv[2]) if len(sys.argv) > 2 else 256
    os.makedirs(dossier, exist_ok=True)
    grenplace(os.path.join(dossier, 'grenplace.png'), taille)
    fichiers(os.path.join(dossier, 'grenos-fichiers.png'), taille)
    son(os.path.join(dossier, 'grenos-son.png'), taille, niveau=2)
    son(os.path.join(dossier, 'grenos-son-faible.png'), taille, niveau=1)
    son(os.path.join(dossier, 'grenos-son-muet.png'), taille, niveau=0)


if __name__ == '__main__':
    main()
