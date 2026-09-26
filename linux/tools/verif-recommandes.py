#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Ce que nos paquets recommandent et que l'image n'aura pas.

grenOS se construit avec `--apt-recommends false`. C'est ce qui fait passer
l'image de 2,3 Go à 1,4 : sans cela on emporte 695 Mio de polices CJK et
238 de QtWebEngine que personne n'ouvrira. Le choix est bon, et il reste bon.

Mais il a un prix, et ce prix s'est payé quatre fois **le même jour** :

  - `ca-certificates`, recommandé par flatpak → aucune connexion TLS vérifiée.
    Ni Flathub, ni notre propre dépôt de mise à jour. Grenofar l'a trouvé en
    essayant d'installer une application.
  - `libgtk-3-bin` → pas de `gtk-update-icon-cache` ni de `gtk-launch` : le
    cache d'icônes jamais rafraîchi après une mise à jour, et le bouton
    Fortnite inerte.
  - `desktop-file-utils` → pas de `update-desktop-database` : la base du menu
    jamais reconstruite, alors qu'apporter une entrée de menu neuve est
    précisément ce qu'une mise à jour doit savoir faire.
  - `alsa-ucm-conf`, recommandé par libasound2-data → ALSA ne sait pas par où
    sortir le son sur une carte Intel SOF ou AMD ACP, c'est-à-dire presque tous
    les portables depuis 2019. Muet, avec une pile logicielle impeccable.

Chaque fois la même forme : absent sans bruit, découvert par quelqu'un devant
l'écran. Les trouver un par un au rythme des pannes est le pire des rythmes.

Ce contrôle les montre **tous en une fois**, pour qu'on les trie une bonne
fois. Il ne décide rien : la plupart des recommandations méritent d'être
ignorées, et c'est tout l'intérêt de `--apt-recommends false`. Il donne une
liste à lire, classée par le nombre de nos paquets qui réclament chacune.

    python3 linux/tools/verif-recommandes.py [--tout]

Sans `--tout`, il ne montre que ce qui n'est pas déjà jugé dans `AVIS`.
Il sort toujours en 0 : ce n'est pas un juge, c'est une lampe.
"""
import gzip
import io
import re
import sys
import urllib.request

BASE = "http://deb.debian.org/debian"
SECTIONS = ("main", "contrib", "non-free", "non-free-firmware")
LISTE = "linux/config/package-lists/grenos.list.chroot"

# Ce qu'on a déjà regardé, et ce qu'on en a décidé. Une ligne par verdict, pour
# qu'un lecteur voie la raison au lieu de refaire l'enquête.
AVIS = {
    # Pris, et pourquoi.
    "ca-certificates": "PRIS — sans lui, aucun TLS verifie : ni Flathub ni notre depot",
    "libgtk-3-bin": "PRIS — gtk-update-icon-cache et gtk-launch",
    "desktop-file-utils": "PRIS — update-desktop-database, la base du menu",
    "alsa-ucm-conf": "PRIS — sans lui, muet sur toute carte SOF ou ACP",
    "alsa-topology-conf": "PRIS — va avec alsa-ucm-conf, memes cartes",
    "dosfstools": "PRIS — sans lui, pas de partition EFI, donc pas d'install UEFI",
    "btrfs-progs": "PRIS — partition.conf offre btrfs ; le proposer sans l'outil est un piege",
    "xfsprogs": "PRIS — idem pour xfs",
    "librsvg2-common": "PRIS — Papirus est en SVG ; sans lui ses icones ne s'affichent pas",
    "chromium-sandbox": "PRIS — le bac a sable de Chromium",
    "qt5-gtk-platformtheme": "PRIS — Calamares est en Qt, le reste en GTK",
    "gnome-icon-theme": "LAISSE — 15 Mio d'icones d'un bureau que nous n'avons pas",
    "iso-codes": "LAISSE — 23 Mio ; Calamares nomme ses langues sans",
    "modemmanager": "LAISSE — le reseau mobile n'est pas un sujet ici",
    "7zip": "LAISSE — xarchiver ouvre deja zip, tar et rar",
    "busybox": "LAISSE — initramfs-tools s'en passe, l'image a un vrai shell",
    "console-data": "LAISSE — 2 Mio de dispositions console ; grenOS demarre en graphique",
    # Laissés, et pourquoi.
    "fonts-noto-cjk": "LAISSE — 695 Mio de polices que personne n'ouvrira ici",
    "qtwebengine5-data": "LAISSE — 238 Mio, aucun de nos programmes ne s'en sert",
    "ibus": "LAISSE — methodes de saisie asiatiques, 131 Mio",
    "fcitx5": "LAISSE — idem",
    "printer-driver-all": "LAISSE — l'impression n'est pas encore un sujet",
    "cups": "LAISSE — idem",
    "xdg-desktop-portal": "LAISSE — utile a Flatpak, mais rien ne l'a reclame a l'usage",
}


def index():
    """Tous les paquets de trixie, toutes sections, par nom."""
    paquets = {}
    for section in SECTIONS:
        for arch in ("amd64", "all"):
            adresse = f"{BASE}/dists/trixie/{section}/binary-{arch}/Packages.gz"
            try:
                brut = gzip.decompress(urllib.request.urlopen(adresse, timeout=240).read())
            except Exception:
                continue
            for bloc in brut.decode("utf-8", "replace").split("\n\n"):
                if bloc.startswith("Package: "):
                    paquets.setdefault(bloc.split("\n", 1)[0][9:], bloc)
    return paquets


def champ(bloc, cle):
    trouve = re.search("^" + cle + r": (.*)$", bloc, re.M)
    return trouve.group(1) if trouve else ""


def noms(expression):
    """Les paquets nommés dans un champ Depends ou Recommends."""
    sortie = []
    for groupe in expression.split(","):
        for choix in groupe.split("|"):
            choix = choix.strip()
            if choix:
                sortie.append(choix.split()[0].split(":")[0])
    return sortie


def ferme(depart, paquets):
    """Tout ce qui sera vraiment installe : les Depends, en cascade."""
    dedans, a_voir = set(), list(depart)
    while a_voir:
        nom = a_voir.pop()
        if nom in dedans or nom not in paquets:
            dedans.add(nom)
            continue
        dedans.add(nom)
        bloc = paquets[nom]
        a_voir += noms(champ(bloc, "Depends") + "," + champ(bloc, "Pre-Depends"))
    return dedans


def main():
    tout = "--tout" in sys.argv
    nos = [l.strip() for l in io.open(LISTE, encoding="utf-8")
           if l.strip() and not l.startswith("#")]

    print("lecture de l'index de trixie…")
    paquets = index()
    print("%d paquets connus, %d demandes par grenOS" % (len(paquets), len(nos)))

    installes = ferme(nos, paquets)
    print("%d paquets seront installes une fois les Depends suivis" % len(installes))

    manquants = {}
    for nom in sorted(installes):
        bloc = paquets.get(nom)
        if not bloc:
            continue
        for recommande in noms(champ(bloc, "Recommends")):
            if recommande in installes:
                continue
            manquants.setdefault(recommande, []).append(nom)

    a_lire = {k: v for k, v in manquants.items() if tout or k not in AVIS}
    print()
    print("%d recommandations absentes, dont %d pas encore jugees"
          % (len(manquants), len(a_lire)))
    print()

    for nom, par in sorted(a_lire.items(), key=lambda x: (-len(x[1]), x[0])):
        bloc = paquets.get(nom, "")
        taille = champ(bloc, "Installed-Size")
        avis = AVIS.get(nom, "")
        print("  %-34s %6s Kio  reclame par %d : %s%s"
              % (nom[:34], taille or "?", len(par), ", ".join(sorted(par)[:3]),
                 ("  [" + avis + "]") if avis else ""))

    print()
    print("Ce controle ne refuse rien : la plupart de ces recommandations doivent")
    print("rester absentes, c'est tout l'interet de --apt-recommends false. Il est")
    print("la pour qu'on les trie en les lisant, plutot qu'une par une au rythme")
    print("des pannes. Ce qui est juge va dans AVIS, avec sa raison.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
