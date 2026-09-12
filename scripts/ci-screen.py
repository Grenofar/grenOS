#!/usr/bin/env python3
"""L'écran du kernel, tel que la CI le voit (D-033).

QEMU tourne sans fenêtre (-display none) mais garde sa carte graphique : son
moniteur en fait une capture PPM. `grab` la prend, `judge` la juge et la
décrit en texte, parce que les agents ne voient aucune image : ils lisent le
log du verdict. La description donne la taille, les couleurs dominantes, celle
du bas, du haut et du centre de l'écran, et une carte en lettres, une par case
pour sa couleur dominante : de quoi savoir où sont dessinées la barre des
tâches et la fenêtre.

Un écran est jugé dessiné quand il a au moins trois couleurs et qu'aucune n'en
couvre plus de 90 %. Un kernel qui ne dessine rien laisse du noir, ou le menu
de Limine sur du noir : une couleur dépasse alors 90 %.

Le même moniteur sert à *entrer* quelque chose (D-035) : `send` lui passe des
commandes HMP, dont `mouse_move`, `mouse_button` et `sendkey`, qui arrivent au
kernel comme une vraie souris PS/2 et un vrai clavier. C'est ainsi que la CI
vérifie que la souris marche, et pas seulement que l'écran est dessiné.

    python3 scripts/ci-screen.py grab <socket du moniteur> <sortie.ppm>
    python3 scripts/ci-screen.py judge <capture.ppm> [<apercu.png>]
    python3 scripts/ci-screen.py send <socket du moniteur> <commande HMP>...

`judge` sort en 0 si l'écran est dessiné, 2 s'il ne l'est pas, 1 sans capture.
"""

import collections
import os
import socket
import struct
import sys
import time
import zlib

MAX_SHARE = 0.90
MIN_COLOURS = 3
MAP_COLS, MAP_ROWS = 48, 16
LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"


def read_ppm(data: bytes):
    """(largeur, hauteur, octets RGB) d'un PPM binaire P6, tel que QEMU l'écrit."""
    if not data.startswith(b"P6"):
        raise ValueError("not a binary PPM (P6)")
    fields = []
    i = 2
    while len(fields) < 3:
        while i < len(data) and data[i : i + 1].isspace():
            i += 1
        if data[i : i + 1] == b"#":
            while i < len(data) and data[i : i + 1] not in (b"\n", b"\r"):
                i += 1
            continue
        start = i
        while i < len(data) and not data[i : i + 1].isspace():
            i += 1
        fields.append(int(data[start:i]))
    i += 1  # un seul blanc sépare l'en-tête des pixels
    width, height, maxval = fields
    if maxval != 255:
        raise ValueError(f"unsupported depth {maxval}")
    rgb = data[i : i + width * height * 3]
    if len(rgb) != width * height * 3:
        raise ValueError("truncated image")
    return width, height, rgb


def hexc(colour: bytes) -> str:
    return "#%02x%02x%02x" % (colour[0], colour[1], colour[2])


def region(width, rgb, x0, y0, x1, y1, step=1):
    """Les couleurs d'un rectangle, un pixel sur `step` dans chaque sens."""
    counts = collections.Counter()
    for y in range(y0, y1, step):
        row = y * width * 3
        for x in range(x0, x1, step):
            k = row + x * 3
            counts[rgb[k : k + 3]] += 1
    return counts


def describe(width, height, rgb):
    """(dessiné ?, lignes de description) d'une capture."""
    total = width * height
    counts = collections.Counter(rgb[k : k + 3] for k in range(0, len(rgb), 3))
    top = counts.most_common(6)
    share = top[0][1] / total
    drawn = share <= MAX_SHARE and len(counts) >= MIN_COLOURS

    lines = [
        f"{width}x{height}, {len(counts)} colours; the most common, {hexc(top[0][0])}, covers {share:.0%} of the screen",
        "top colours: " + ", ".join(f"{hexc(c)} {n / total:.0%}" for c, n in top),
    ]
    strip = max(1, height // 20)
    for name, (x0, y0, x1, y1) in (
        ("bottom strip", (0, height - strip, width, height)),
        ("top strip", (0, 0, width, strip)),
        ("centre", (width // 4, height // 4, 3 * width // 4, 3 * height // 4)),
    ):
        part = region(width, rgb, x0, y0, x1, y1, step=2)
        colour, n = part.most_common(1)[0]
        lines.append(f"{name}: mostly {hexc(colour)} ({n / sum(part.values()):.0%})")

    legend = {}
    lines.append(f"map, {MAP_COLS}x{MAP_ROWS}, one letter per cell for its dominant colour:")
    for r in range(MAP_ROWS):
        y0 = r * height // MAP_ROWS
        y1 = max(y0 + 1, (r + 1) * height // MAP_ROWS)
        row = ""
        for c in range(MAP_COLS):
            x0 = c * width // MAP_COLS
            x1 = max(x0 + 1, (c + 1) * width // MAP_COLS)
            colour = region(width, rgb, x0, y0, x1, y1, step=3).most_common(1)[0][0]
            if colour not in legend:
                legend[colour] = LETTERS[len(legend)] if len(legend) < len(LETTERS) else "?"
            row += legend[colour]
        lines.append("  " + row)
    lines.append("legend: " + ", ".join(f"{letter}={hexc(c)}" for c, letter in legend.items()))

    if drawn:
        lines.append("screen: PASS, the kernel drew on the screen")
    elif len(counts) < MIN_COLOURS:
        lines.append(f"screen: FAIL, only {len(counts)} colour(s): nothing was drawn")
    else:
        lines.append(
            f"screen: FAIL, {hexc(top[0][0])} covers {share:.0%} of the screen (at most {MAX_SHARE:.0%} allowed): nothing, or almost nothing, was drawn"
        )
    return drawn, lines


def png(width, height, rgb) -> bytes:
    """Un PNG RGB 8 bits, sans dépendance : l'aperçu publié sur /download."""
    stride = width * 3
    raw = b"".join(b"\x00" + rgb[y * stride : (y + 1) * stride] for y in range(height))

    def chunk(tag: bytes, body: bytes) -> bytes:
        return struct.pack(">I", len(body)) + tag + body + struct.pack(">I", zlib.crc32(tag + body) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def grab(monitor: str, out: str, wait: float = 10.0) -> bool:
    """Demande une capture au moniteur de QEMU et attend qu'elle soit écrite."""
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
        s.settimeout(5)
        s.connect(monitor)
        try:
            s.recv(4096)  # la bannière et l'invite « (qemu) »
        except socket.timeout:
            pass
        s.sendall(f"screendump {out}\n".encode())
        deadline = time.time() + wait
        size = -1
        while time.time() < deadline:
            time.sleep(0.5)
            now = os.path.getsize(out) if os.path.exists(out) else -1
            if now > 0 and now == size:
                return True
            size = now
    return os.path.exists(out) and os.path.getsize(out) > 0


def send(monitor: str, commands: list) -> bool:
    """Passe des commandes au moniteur de QEMU, une par une.

    `mouse_move dx dy`, `mouse_button 1|0` et `sendkey <touche>` entrent par la
    couche d'entrée de QEMU : le kernel reçoit des octets PS/2 comme d'une vraie
    souris et d'un vrai clavier, IRQ comprises. Une pause entre deux commandes,
    sinon QEMU les avale pendant que le kernel dort encore.
    """
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
        s.settimeout(5)
        s.connect(monitor)
        try:
            s.recv(4096)  # la bannière et l'invite « (qemu) »
        except socket.timeout:
            pass
        for command in commands:
            s.sendall(f"{command}\n".encode())
            print(f"input: {command}")
            time.sleep(0.4)
    return True


def main(argv) -> int:
    if len(argv) >= 4 and argv[1] == "send":
        try:
            send(argv[2], argv[3:])
        except OSError as exc:
            print(f"input: the QEMU monitor did not answer ({exc})")
            return 1
        return 0

    if len(argv) == 4 and argv[1] == "grab":
        try:
            ok = grab(argv[2], argv[3])
        except OSError as exc:
            print(f"screenshot: the QEMU monitor did not answer ({exc})")
            return 1
        print("screenshot: taken" if ok else "screenshot: none written")
        return 0 if ok else 1

    if len(argv) in (3, 4) and argv[1] == "judge":
        try:
            with open(argv[2], "rb") as f:
                width, height, rgb = read_ppm(f.read())
        except (OSError, ValueError) as exc:
            print(f"screen: FAIL, no usable screenshot ({exc}): QEMU never answered the capture at 25 s")
            return 1
        drawn, lines = describe(width, height, rgb)
        print("\n".join(lines))
        if len(argv) == 4:
            with open(argv[3], "wb") as f:
                f.write(png(width, height, rgb))
        return 0 if drawn else 2

    print(__doc__, file=sys.stderr)
    return 64


if __name__ == "__main__":
    sys.exit(main(sys.argv))
