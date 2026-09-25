#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""La machine VirtualBox tient-elle ce que la page promet ?

`/download` annonce, noir sur blanc : « La machine grenOS apparaît, déjà
réglée : 64 bits, 8 Go de mémoire, quatre cœurs, 256 Mo de mémoire vidéo, un
disque de 25 Go. » Quatre nombres, écrits pour quelqu'un qui va les vérifier
d'un coup d'œil dans VirtualBox.

Rien ne vérifiait la correspondance. La CI contrôlait que le zip contient ses
trois fichiers et que le `.vbox` est du XML valide — pas qu'il déclare ce que
la page raconte. Changer `make-vbox.py` sans toucher à la page, ou l'inverse,
passait donc sans un mot, et la première personne à ouvrir la machine aurait
trouvé autre chose que ce qu'on lui avait dit.

Ce contrôle lit le `.vbox` produit et le compare à la promesse. Il refuse dès
qu'un nombre diffère — parce qu'une promesse chiffrée est soit tenue, soit
fausse.

    python3 scripts/verif-vbox-promesse.py <machine.vbox> [disque.vdi]
"""
import os
import sys
import xml.etree.ElementTree as ET

# Ce que `/download` promet, dans les deux langues.
PROMESSE = {
    "coeurs": 4,
    "memoire_mo": 8192,
    "video_mo": 256,
    "disque_go": 25,
}

ESPACE = "{http://www.virtualbox.org/}"


def texte_de(racine, chemin, attribut):
    """La valeur d'un attribut, ou None si le chemin n'existe pas."""
    noeud = racine.find(chemin.replace("{}", ESPACE))
    if noeud is None:
        return None
    return noeud.get(attribut)


def main():
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 64

    chemin = sys.argv[1]
    racine = ET.parse(chemin).getroot()
    materiel = "./{}Machine/{}Hardware"

    lu = {
        "coeurs": texte_de(racine, materiel + "/{}CPU", "count"),
        "memoire_mo": texte_de(racine, materiel + "/{}Memory", "RAMSize"),
        "video_mo": texte_de(racine, materiel + "/{}Display", "VRAMSize"),
    }

    fautes = []
    for nom, attendu in (("coeurs", PROMESSE["coeurs"]),
                         ("memoire_mo", PROMESSE["memoire_mo"]),
                         ("video_mo", PROMESSE["video_mo"])):
        valeur = lu[nom]
        if valeur is None:
            fautes.append(f"{nom} : le .vbox ne le declare pas du tout")
        elif int(valeur) != attendu:
            fautes.append(f"{nom} : le .vbox dit {valeur}, la page promet {attendu}")
        else:
            print(f"vbox: {nom} = {valeur}, comme promis")

    # Le disque : sa taille reelle, pas ce que le .vbox en dit. Un .vdi est
    # creux — il ne pese pas 25 Go sur le disque —, mais son en-tete porte la
    # taille annoncee a la machine, et c'est celle-la que VirtualBox montre.
    if len(sys.argv) > 2 and os.path.exists(sys.argv[2]):
        with open(sys.argv[2], "rb") as fichier:
            entete = fichier.read(512)
        # Format VDI : la taille du disque est un entier 64 bits a l'offset
        # 0x170 de l'en-tete. Lu dans la specification du format, pas devine.
        octets = int.from_bytes(entete[0x170:0x178], "little")
        go = octets / (1000 ** 3)
        gio = octets / (1024 ** 3)
        if abs(gio - PROMESSE["disque_go"]) < 0.6 or abs(go - PROMESSE["disque_go"]) < 0.6:
            print(f"vbox: disque = {gio:.1f} Gio, comme promis")
        else:
            fautes.append(f"disque : le .vdi annonce {gio:.1f} Gio, "
                          f"la page promet {PROMESSE['disque_go']} Go")

    if fautes:
        for faute in fautes:
            print("::error::La machine ne tient pas la promesse de /download — " + faute)
        print("::error::Corrige scripts/make-vbox.py, ou la page, mais pas l'un sans l'autre.")
        return 1

    print("vbox: la machine tient ce que /download promet")
    return 0


if __name__ == "__main__":
    sys.exit(main())
