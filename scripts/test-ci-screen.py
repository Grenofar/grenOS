#!/usr/bin/env python3
"""Vérifie le jugement de l'écran (ci-screen.py) et le .vbox (make-vbox.py)."""

import importlib.util
import os
import sys
import xml.etree.ElementTree as ET


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, os.path.join(os.path.dirname(__file__), file))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


screen = load("ci_screen", "ci-screen.py")
makevbox = load("make_vbox", "make-vbox.py")

failures = []


def check(label, condition, detail=""):
    if condition:
        print(f"  OK   {label}")
    else:
        failures.append(label)
        print(f"  ECHEC {label} {detail}")


def ppm(width, height, paint):
    """Un PPM P6 dont chaque pixel vient de paint(x, y) -> (r, g, b)."""
    pixels = bytearray()
    for y in range(height):
        for x in range(width):
            pixels += bytes(paint(x, y))
    return f"P6\n# QEMU\n{width} {height}\n255\n".encode() + bytes(pixels)


TEAL, GREY, WHITE, NAVY, BLACK = (0, 128, 128), (192, 192, 192), (255, 255, 255), (0, 0, 128), (0, 0, 0)


def desktop(x, y):
    # 96x64 : fond sarcelle, barre des tâches grise en bas, une fenêtre blanche
    # au centre avec sa barre de titre bleu marine.
    if y >= 58:
        return GREY
    if 24 <= x < 72 and 16 <= y < 20:
        return NAVY
    if 24 <= x < 72 and 20 <= y < 44:
        return WHITE
    return TEAL


print("lecture du PPM :")
w, h, rgb = screen.read_ppm(ppm(4, 3, lambda x, y: (x, y, 7)))
check("taille lue malgré le commentaire", (w, h) == (4, 3))
check("pixels intacts", rgb[:6] == bytes([0, 0, 7, 1, 0, 7]))

print("\njugement :")
drawn, lines = screen.describe(*screen.read_ppm(ppm(96, 64, lambda x, y: BLACK)))
check("un écran noir n'est pas dessiné", not drawn)
check("il dit pourquoi", "screen: FAIL" in lines[-1] and "nothing was drawn" in lines[-1], lines[-1])

limine = lambda x, y: WHITE if (y == 5 and x < 20) else BLACK  # une ligne de texte sur du noir
drawn, lines = screen.describe(*screen.read_ppm(ppm(96, 64, limine)))
check("du texte sur du noir n'est pas un bureau", not drawn, lines[-1])

drawn, lines = screen.describe(*screen.read_ppm(ppm(96, 64, desktop)))
text = "\n".join(lines)
check("un bureau est dessiné", drawn, text)
check("la couleur dominante et sa part", "#008080" in lines[0], lines[0])
check("le bas de l'écran est gris", "bottom strip: mostly #c0c0c0" in text, text)
check("le centre est blanc", "centre: mostly #ffffff" in text, text)
grid = [l.strip() for l in lines if l.startswith("  ")]
check("une carte de 16 lignes de 48 cases", len(grid) == 16 and all(len(r) == 48 for r in grid), str(grid[:2]))
check("la barre des tâches occupe la dernière ligne de la carte", len(set(grid[-1])) == 1 and grid[-1] != grid[0], grid[-1])
check("la légende nomme chaque lettre", "legend: A=#008080" in text, text)

print("\naperçu PNG :")
image = screen.png(*screen.read_ppm(ppm(8, 4, desktop)))
check("signature PNG", image.startswith(b"\x89PNG\r\n\x1a\n"))
check("se termine par le bloc IEND", image[-12:] == b"\x00\x00\x00\x00IEND\xaeB`\x82")

print("\nmachine VirtualBox :")
text = makevbox.vbox("grenos-20260911-1918-843bbd2.iso", "11111111-2222-3333-4444-555555555555", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
root = ET.fromstring(text.encode())
ns = {"v": "http://www.virtualbox.org/"}
machine = root.find("v:Machine", ns)
check("une machine 64 bits nommée grenOS", machine is not None and machine.get("name") == "grenOS" and machine.get("OSType") == "Other_64")
image = root.find(".//v:DVDImages/v:Image", ns)
check("l'ISO est désignée par son seul nom", image is not None and image.get("location") == "grenos-20260911-1918-843bbd2.iso")
attached = root.find(".//v:AttachedDevice/v:Image", ns)
check("le lecteur optique porte cette ISO", attached is not None and attached.get("uuid") == image.get("uuid") == "{aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee}")
port = root.find(".//v:UART/v:Port", ns)
check("COM1 écrit dans un fichier au chemin absolu", port is not None and port.get("IOBase") == "0x3f8" and port.get("hostMode") == "RawFile" and port.get("path", "").startswith("C:\\"))
check("256 Mo", root.find(".//v:Memory", ns).get("RAMSize") == "256")
try:
    makevbox.vbox('grenos"><x.iso')
    check("un nom d'ISO piégé est refusé", False)
except ValueError:
    check("un nom d'ISO piégé est refusé", True)

print()
if failures:
    print(f"{len(failures)} echec(s)")
    sys.exit(1)
print("tout passe")
