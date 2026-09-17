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
    top = np.array((0x08, 0x0C, 0x18) if night else (0xEC, 0xF1, 0xFA), dtype=float)
    bottom = np.array((0x03, 0x05, 0x0B) if night else (0xCF, 0xDC, 0xF2), dtype=float)
    strength = 0.55 if night else 0.30

    ys, xs = np.mgrid[0:height, 0:width].astype(float)
    # Le fond : un dégradé vertical, du plus clair en haut au plus sombre en bas.
    image = top + (bottom - top) * (ys / max(height - 1, 1))[..., None]

    # Trois halos, posés en proportions de l'image pour tenir à toute taille.
    for fx, fy, radius, tint, weight in [
        (0.24, 0.22, 0.75, ACCENT, 1.00),
        (0.82, 0.78, 0.85, VIOLET, 0.80),
        (0.66, 0.12, 0.45, TEAL, 0.35),
    ]:
        distance = np.hypot(xs - fx * width, ys - fy * height) / (radius * height)
        amount = (np.clip(1.0 - distance, 0.0, 1.0) ** 2 * weight * strength)[..., None]
        image += (np.array(tint, dtype=float) - image) * amount

    # L'écharpe de lumière : une diagonale large, si douce qu'on la voit à
    # peine — une arête nette sur un fond d'écran saute aux yeux.
    band = np.abs((xs * 0.55 + ys) / (width * 0.55 + height) - 0.42)
    veil = (np.clip(0.30 - band, 0.0, None) / 0.30) ** 2 * (0.055 if night else 0.10)
    image += (255.0 - image) * veil[..., None]

    # Un vignettage léger, pour que les icônes du bureau se détachent.
    edge = np.hypot((xs / width - 0.5) * 1.1, (ys / height - 0.5) * 1.1)
    dark = np.array((0, 0, 0) if night else (0x9A, 0xA8, 0xC0), dtype=float)
    shade = np.clip(edge - 0.45, 0.0, None)[..., None] * 0.55
    image += (dark - image) * shade

    png(path, np.clip(image, 0, 255).astype(np.uint8))


if __name__ == '__main__':
    folder = sys.argv[1] if len(sys.argv) > 1 else '.'
    size = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (2560, 1440)
    make(f'{folder}/grenos-nuit.png', size[0], size[1], night=True)
    make(f'{folder}/grenos-jour.png', size[0], size[1], night=False)
