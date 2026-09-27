#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Un son d'essai CONTINU, pour que le moindre trou soit un vrai trou.

Pourquoi ce fichier existe, alors qu'`alsa-utils` en livre un tout fait.

Grenofar : « on entend des bom bom bom quand y'a la vidéo ». Un hachement, ce
sont des trous : le tampon de PipeWire se vide et la carte joue du vide pendant
quelques millisecondes. Pour les compter, la machine d'intégration enregistre
ce que la carte émet et cherche les silences.

Le premier essai jouait `Front_Center.wav`, celui d'alsa-utils. Résultat :

    son : 6 trou(s) dans la partie sonore, le plus long 366 ms

Chiffre impressionnant, et **entièrement faux** : ce fichier est une voix qui
dit « Front Centre », et une voix se tait entre les syllabes. Vérifié en
ouvrant le `.deb` et en comptant dans la source — 6 trous, le plus long
345 ms. Exactement les mêmes. Le détecteur mesurait la diction, pas la panne.

D'où ce fichier-ci : une sinusoïde **sans un seul silence**. Tout trou qu'on y
trouvera après lecture aura été creusé par la machine, et par rien d'autre.

    python3 linux/tools/son-essai.py <dossier> [secondes]

Ni numpy ni aucune bibliothèque : du `struct` et une boucle. Le fichier fait
environ 190 Kio pour deux secondes, et il est le même à chaque construction.
"""
import math
import os
import struct
import sys

TAUX = 48000       # ce que la carte attend le plus souvent
HERTZ = 440.0      # un la, ni aigu ni grave : audible sans être désagréable
AMPLITUDE = 9000   # ~27 % du maximum, assez fort pour être mesuré sans saturer


def ecrire(chemin, secondes):
    total = int(TAUX * secondes)

    # Une montée et une descente de 20 ms aux extrémités : sans elles, le son
    # commence et finit sur une marche, ce qui claque dans les enceintes et
    # ajoute un transitoire que le détecteur pourrait prendre pour autre chose.
    fondu = int(TAUX * 0.02)
    corps = bytearray()
    for n in range(total):
        valeur = math.sin(2.0 * math.pi * HERTZ * n / TAUX)
        if n < fondu:
            valeur *= n / fondu
        elif n > total - fondu:
            valeur *= (total - n) / fondu
        corps += struct.pack("<h", int(AMPLITUDE * valeur))

    entete = b"RIFF" + struct.pack("<I", 36 + len(corps)) + b"WAVEfmt "
    entete += struct.pack("<IHHIIHH", 16, 1, 1, TAUX, TAUX * 2, 2, 16)
    entete += b"data" + struct.pack("<I", len(corps))

    with open(chemin, "wb") as fichier:
        fichier.write(entete + bytes(corps))
    print("son d'essai : %s, %.1f s, %d Hz, %d Kio"
          % (chemin, secondes, HERTZ, (len(corps) + 44) // 1024))


def main():
    dossier = sys.argv[1] if len(sys.argv) > 1 else "."
    secondes = float(sys.argv[2]) if len(sys.argv) > 2 else 2.0
    os.makedirs(dossier, exist_ok=True)
    ecrire(os.path.join(dossier, "essai-son.wav"), secondes)
    return 0


if __name__ == "__main__":
    sys.exit(main())
