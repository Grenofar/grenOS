#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Vérifie que chaque modèle sous linux/config/includes.chroot/usr/share/grenos/modeles/
est bien utilisé par appliquer-systeme et que chaque ligne poser de ce script
fait référence à un modèle existant.

POURQUOI CE FICHIER EXISTE — grenOS partage des réglages sous forme de modèles
dans linux/config/includes.chroot/usr/share/grenos/modeles/, car deux paquets
Debian ne peuvent pas posséder le même chemin. Le script
linux/config/includes.chroot/usr/lib/grenos/appliquer-systeme copie chaque
modèle à sa destination après une mise à jour, avec des lignes de la forme
'poser <nom-du-modèle> <chemin-absolu-cible>'. Un modèle jamais copié est un
fichier qui ne sert à rien, et le défaut est invisible : l'image contient le
fichier, mais la machine ne voit jamais le réglage. Ce garde lit les deux
scripts en texte, ne les exécute jamais, et signale les incohérences.
"""
import io
import os
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main(argv):
    # Chemins des deux fichiers à lire en texte
    appliquer = os.path.join(
        RACINE,
        "linux/config/includes.chroot/usr/lib/grenos/appliquer-systeme",
    )
    make_deb = os.path.join(RACINE, "linux/tools/make-deb.sh")

    # 1. Lire appliquer-systeme et extraire les modèles utilisés
    try:
        appliquer_contenu = io.open(appliquer, encoding="utf-8").read()
    except (OSError, UnicodeDecodeError) as souci:
        print(f"::error file=linux/config/includes.chroot/usr/lib/grenos/appliquer-systeme::illisible : {souci}")
        return 1

    modeles_utilises = set()
    for ligne in appliquer_contenu.splitlines():
        if ligne.startswith("poser "):
            parties = ligne.split()
            if len(parties) >= 2:
                modele = parties[1]
                modeles_utilises.add(modele)

    # 2. Lire make-deb.sh et extraire les modèles installés (créés)
    try:
        make_deb_contenu = io.open(make_deb, encoding="utf-8").read()
    except (OSError, UnicodeDecodeError) as souci:
        print(f"::error file=linux/tools/make-deb.sh::illisible : {souci}")
        return 1

    modeles_crees = set()
    prefixe = '$BUILD/usr/share/grenos/modeles/'
    for ligne in make_deb_contenu.splitlines():
        if 'install -m 0644' in ligne and '"$DEDANS/"' in ligne and prefixe in ligne:
            # Exemple de ligne :
            # install -m 0644 "$DEDANS/etc/gtk-3.0/settings.ini" "$BUILD/usr/share/grenos/modeles/gtk-settings.ini"
            parties = ligne.split()
            if len(parties) >= 4:
                cible = parties[-1].strip('"')
                if cible.startswith(prefixe):
                    modele = cible[len(prefixe):]
                    modeles_crees.add(modele)

    # 3. Vérifications et rapport d'erreurs
    fautes = 0

    # (a) Modèles créés mais jamais utilisés → faute
    for modele in sorted(modeles_crees - modeles_utilises):
        chemin = f"linux/config/includes.chroot/usr/share/grenos/modeles/{modele}"
        print(f"::error file={chemin}::le modèle n'est jamais posé par appliquer-systeme")
        fautes += 1

    # (b) Modèles utilisés mais jamais créés → faute
    for modele in sorted(modeles_utilises - modeles_crees):
        chemin = f"linux/config/includes.chroot/usr/share/grenos/modeles/{modele}"
        print(f"::error file={chemin}::le modèle est posé par appliquer-systeme mais aucun install ne le crée dans make-deb.sh")
        fautes += 1

    if fautes:
        return 1

    # 4. Tout est correct → ligne de résumé en français avec accents
    nb = len(modeles_crees)
    print(f"modeles : {nb} modele(s), chacun est posé par appliquer-systeme")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
