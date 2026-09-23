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
    """Une composition en courbes, calculée : ni photo, ni motif répété.

    Refaite le 23 septembre 2026 : l'ancienne version empilait trois halos et
    une écharpe de lumière, ce qui donnait une brume uniforme. Celle-ci a une
    ligne d'horizon — des ondes larges qui traversent l'image en biais, plus
    claires là où elles se croisent. Un fond d'écran doit tenir derrière des
    icônes : il garde donc ses valeurs sombres en bas à gauche, là où les
    icônes se posent, et respire en haut à droite.
    """
    haut = np.array((0x0B, 0x12, 0x24) if night else (0xF2, 0xF6, 0xFD), dtype=float)
    bas = np.array((0x04, 0x06, 0x0E) if night else (0xD6, 0xE0, 0xF3), dtype=float)

    ys, xs = np.mgrid[0:height, 0:width].astype(float)
    u = xs / max(width - 1, 1)
    v = ys / max(height - 1, 1)

    # Le fond : un dégradé en diagonale plutôt qu'à la verticale. Le regard
    # suit la diagonale, et l'image paraît moins plate.
    diagonale = np.clip(v * 0.78 + u * 0.22, 0.0, 1.0)
    image = haut + (bas - haut) * diagonale[..., None]

    # Trois ondes larges. Leur amplitude diminue vers le bas de l'image, pour
    # que la zone des icônes reste calme.
    calme = np.clip(1.0 - v * 1.15, 0.0, 1.0)
    for frequence, phase, hauteur_onde, tint, force in (
            (1.6, 0.0, 0.30, ACCENT, 1.00),
            (2.3, 1.7, 0.22, VIOLET, 0.75),
            (3.1, 3.4, 0.14, TEAL, 0.45)):
        ligne = 0.34 + hauteur_onde * np.sin(u * frequence * np.pi * 2 + phase) * 0.5
        distance = np.abs(v - ligne)
        lueur = np.clip(1.0 - distance / 0.34, 0.0, 1.0) ** 3
        quantite = (lueur * calme * force * (0.55 if night else 0.32))[..., None]
        image += (np.array(tint, dtype=float) - image) * quantite

    # Un point de lumière en haut à droite : une source, et l'image cesse
    # d'être un aplat.
    distance = np.hypot((u - 0.78) * (width / height), v - 0.16)
    halo = np.clip(1.0 - distance / 0.55, 0.0, 1.0) ** 2
    blanc = np.array((255, 255, 255), dtype=float)
    image += (blanc - image) * (halo * (0.16 if night else 0.42))[..., None]

    # Un grain très fin : sans lui, un dégradé calculé montre des bandes sur
    # les écrans qui n'affichent pas toutes les nuances.
    grain = np.random.default_rng(7).normal(0.0, 1.4, size=(height, width))[..., None]
    image += grain

    # Le coin des icônes reste sombre : c'est là qu'on lit des noms de fichier.
    coin = np.clip(1.0 - np.hypot(u / 0.55, (1.0 - v) / 0.75), 0.0, 1.0) ** 2
    sombre = np.array((0, 0, 0) if night else (0x8F, 0x9F, 0xBA), dtype=float)
    image += (sombre - image) * (coin * (0.35 if night else 0.18))[..., None]

    png(path, np.clip(image, 0, 255).astype(np.uint8))


if __name__ == '__main__':
    folder = sys.argv[1] if len(sys.argv) > 1 else '.'
    size = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (2560, 1440)
    make(f'{folder}/grenos-nuit.png', size[0], size[1], night=True)
    make(f'{folder}/grenos-jour.png', size[0], size[1], night=False)
