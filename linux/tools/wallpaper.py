#!/usr/bin/env python3
"""Les fonds d'écran de grenOS, calculés, pas dessinés à la main.

Deux images : « Nuit », bleu profond avec trois halos et une écharpe de lumière
à peine visible, et « Jour », la même composition en clair. Écrit en numpy et
zlib, sans bibliothèque d'images : cela tourne à la construction de l'ISO, dans
le conteneur Debian, jamais sur la machine de l'utilisateur.

    python3 wallpaper.py <dossier> [largeur] [hauteur]
"""
import struct
import sys
import zlib

import numpy as np

# La palette de grenOS : le bleu du bureau, un violet qui le réchauffe, et un
# vert d'eau qui empêche l'ensemble d'être froid.
ACCENT = (0x2F, 0x7D, 0xF6)
VIOLET = (0x7C, 0x4D, 0xF0)
TEAL = (0x2B, 0xC4, 0xB0)
INDIGO = (0x3B, 0x4F, 0xC8)


def png(path, pixels):
    """Écrit un tableau (hauteur, largeur, 3) d'octets en PNG."""
    height, width, _ = pixels.shape
    raw = np.concatenate([np.zeros((height, 1), dtype=np.uint8), pixels.reshape(height, width * 3)], axis=1)

    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xFFFFFFFF)

    header = struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)
    body = zlib.compress(raw.tobytes(), 9)
    open(path, 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', header) + chunk(b'IDAT', body) + chunk(b'IEND', b''))
    print(f'{path}: {width}x{height}, {len(body) // 1024} Kio')


def make(path, width, height, night):
    """L'aurore de grenos-ui, en fond d'écran : calculée, ni photo ni motif.

    Refaite le 17 novembre pour que le bureau ressemble à ses fenêtres :
    l'assistant, GrenLaunch et grenPerf dessinent tous ce même fond — trois
    grandes masses de couleur douces (bleu en haut à gauche, violet en bas à
    droite, un reflet cyan au centre) sur un bleu nuit. La pousse du logo y est
    tracée en très grand et très discrète, comme un filigrane : on la devine,
    on ne la lit pas. Le coin bas gauche, où se posent les icônes, reste calme.
    """
    # Le jour n'est pas un blanc : un lavande pâle, sinon le filigrane et les
    # masses de couleur disparaissent (« on voit rien », 17 novembre).
    fond = np.array((0x07, 0x0B, 0x14) if night else (0xE4, 0xE6, 0xF4), dtype=float)
    ys, xs = np.mgrid[0:height, 0:width].astype(float)
    ratio = width / height
    u = xs / max(width - 1, 1)
    v = ys / max(height - 1, 1)
    image = np.broadcast_to(fond, (height, width, 3)).copy()

    def tache(cx, cy, rayon, couleur, force):
        d = np.hypot((u - cx) * ratio, v - cy) / rayon
        lueur = np.clip(1.0 - d, 0.0, 1.0) ** 2.2
        image[...] += (np.array(couleur, dtype=float) - image) * (lueur * force)[..., None]

    # Le bleu du coin haut gauche « gâchait tout » : plus petit, plus doux,
    # et tiré vers l'indigo pour répondre au violet au lieu de le combattre.
    if night:
        tache(0.06, 0.02, 0.62, INDIGO, 0.34)
        tache(0.95, 0.98, 1.10, VIOLET, 0.55)
    else:
        tache(0.04, 0.00, 0.80, (0x9A, 0xB4, 0xF8), 0.55)
        tache(0.96, 1.00, 1.15, (0xA5, 0x8C, 0xF5), 0.70)
        tache(0.70, 0.30, 0.60, (0xF0, 0xC8, 0xF4), 0.35)

    # Le filigrane : une feuille de la pousse, très grande, en haut à droite.
    angle = np.radians(-118)
    c, s = np.cos(angle), np.sin(angle)
    x0, y0, longueur, largeur = 0.98 * ratio, 1.02, 1.35, 0.30
    px, py = u * ratio - x0, v - y0
    x = px * c + py * s
    y = -px * s + py * c
    bord = largeur * np.sin(np.pi * np.clip(x / longueur, 0, 1))
    feuille = np.where((x > 0) & (x < longueur), np.clip((bord - np.abs(y)) / 0.004, 0, 1), 0.0)
    contour = feuille * (1 - np.clip((bord - np.abs(y) - 0.006) / 0.004, 0, 1))
    # La nuit, un trait clair ; le jour, un trait violet : du blanc sur du
    # blanc ne se voit pas.
    trait = np.array((255, 255, 255) if night else (0x6D, 0x4B, 0xD8), dtype=float)
    image += (trait - image) * (feuille * (0.035 if night else 0.07) + contour * (0.10 if night else 0.38))[..., None]

    # Un grain très fin : sans lui, un dégradé calculé montre des bandes.
    image += np.random.default_rng(7).normal(0.0, 1.3, size=(height, width))[..., None]

    # Le coin des icônes reste sombre : c'est là qu'on lit des noms de fichier.
    coin = np.clip(1.0 - np.hypot(u / 0.50, (1.0 - v) / 0.70), 0.0, 1.0) ** 2
    sombre = np.array((0, 0, 0) if night else (0xA8, 0xB4, 0xCA), dtype=float)
    image += (sombre - image) * (coin * (0.30 if night else 0.15))[..., None]

    png(path, np.clip(image, 0, 255).astype(np.uint8))


if __name__ == '__main__':
    folder = sys.argv[1] if len(sys.argv) > 1 else '.'
    size = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (2560, 1440)
    make(f'{folder}/grenos-nuit.png', size[0], size[1], night=True)
    make(f'{folder}/grenos-jour.png', size[0], size[1], night=False)
