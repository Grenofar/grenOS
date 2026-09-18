#!/usr/bin/env python3
"""Chaque nom de la liste existe-t-il vraiment dans trixie ?

La leçon a été payée deux fois : `spectacle` n'existe pas (c'est
`kde-spectacle`), `policykit-1` non plus (c'est `polkitd`). Un seul nom faux
fait échouer la construction vingt minutes plus tard, dans le conteneur, après
le téléchargement de tout le reste. Ce script pose la question aux index de
Debian avant de lancer quoi que ce soit.

    python3 linux/tools/verif-paquets.py
"""
import gzip
import os
import re
import sys
import urllib.request

AREAS = ['main', 'contrib', 'non-free', 'non-free-firmware']
BASE = 'http://deb.debian.org/debian/dists/trixie'
LIST = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                    '..', 'config', 'package-lists', 'grenos.list.chroot')


def index():
    """Les paquets réels de trixie, et ce que d'autres fournissent en leur nom."""
    reels, virtuels = set(), {}
    for area in AREAS:
        raw = urllib.request.urlopen(f'{BASE}/{area}/binary-amd64/Packages.gz', timeout=180).read()
        for bloc in gzip.decompress(raw).decode('utf-8', 'replace').split('\n\n'):
            nom = re.search(r'^Package: (.*)$', bloc, re.M)
            if not nom:
                continue
            reels.add(nom.group(1))
            fournit = re.search(r'^Provides: (.*)$', bloc, re.M)
            if fournit:
                for virtuel in re.split(r',\s*', fournit.group(1)):
                    virtuel = virtuel.split()[0] if virtuel.strip() else ''
                    if virtuel:
                        virtuels.setdefault(virtuel, nom.group(1))
    return reels, virtuels


def main():
    reels, virtuels = index()
    voulus = [ligne.strip() for ligne in open(LIST, encoding='utf-8')
              if ligne.strip() and not ligne.startswith('#')]
    print(f'{len(reels)} paquets dans trixie, {len(voulus)} demandés\n')

    manquants = []
    for paquet in voulus:
        if paquet in reels:
            continue
        if paquet in virtuels:
            print(f'  virtuel  {paquet}  -> {virtuels[paquet]}')
            continue
        proches = sorted(nom for nom in reels if paquet in nom or nom in paquet)[:6]
        manquants.append(paquet)
        print(f'  ABSENT   {paquet}   proches: {proches}')

    print('\n' + ('tout existe' if not manquants else f'{len(manquants)} à corriger'))
    return 1 if manquants else 0


if __name__ == '__main__':
    sys.exit(main())
