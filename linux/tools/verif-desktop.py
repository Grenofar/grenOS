#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Vérifie les fichiers .desktop de grenOS.

POURQUOI CE FICHIER EXISTE — une promesse à l'écran sans rien derrière est le
défaut capital de ce dépôt : un menu entry dont Exec nomme un programme manquant
dont Icon nomme une icône inexistante trompe l'utilisateur. Ce garde refuse
les .desktop fautifs avant qu'ils ne soient intégrés à l'image.

Il lit chaque *.desktop sous linux/config/includes.chroot/usr/share/applications/
ou les chemins passés en argument) et refuse (exit 1, après avoir affiché une
ligne ::error file=<path>::<reason> par faute) lorsque :
  (a) le fichier ne possède pas de section [Desktop Entry] ;
  (b) il manque Name, Exec ou Icon ;
  (c) le premier mot d'Exec nomme un programme que grenOS ne livre pas et qui
      n'est pas un programme Debian — on considère comme livré tout fichier
      présent sous linux/config/includes.chroot/usr/bin/ ou
      linux/config/includes.chroot/usr/lib/grenos/, et tout nom dont le
      basename commence par 'grenos' ou 'gren' comme nôtre, donc un Exec qui
      commence par gren* que nous ne livrons pas est une faute ;
  (d) l'Icon nomme une de nos propres icônes (nom commençant par 'grenos')
      qui n'est pas produite par linux/tools/icones.py — on lit les noms
      d'icones depuis icones.py en le parsant comme du texte, sans l'importer.

Lorsque rien ne manque, il affiche exactement une ligne :
  desktop : N fichier(s) lus, chaque Exec et chaque icone existent
et quitte avec le code 0.

Le script accepte des chemins de fichiers en arguments et ne juge que ceux-ci,
comme linux/tools/verif-outils.py, afin de pouvoir être testé sur une arborescence
fabriquée.

Commentaires et texte affiché en français ; les commentaires sont sans accents,
le texte affiché porte ses accents.
"""
import os
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOSSIER_APPLICATIONS = os.path.join(
    RACINE, 'config', 'includes.chroot', 'usr', 'share', 'applications'
)
DOSSOIR_BIN_USR = os.path.join(RACINE, 'config', 'includes.chroot', 'usr', 'bin')
DOSSOIR_BIN_GRENOS = os.path.join(RACINE, 'config', 'includes.chroot', 'usr', 'lib', 'grenos')
FICHIER_ICONES = os.path.join(RACINE, 'tools', 'icones.py')


def _lire_icones_disponibles():
    """Lit linux/tools/icones.py en texte et renvoie l'ensemble des noms d'icones produits."""
    noms = set()
    try:
        with open(FICHIER_ICONES, encoding='utf-8') as f:
            contenu = f.read()
    except OSError:
        # Si le fichier manque, on considère qu'aucune icône n'est disponible.
        return noms
    # On cherche les appels à png(chemin, ...) où chemin se termine par /<nom>.png
    import re
    for match in re.finditer(r'png\(\s*["\']([^"\']*?)["\']\s*,', contenu):
        chemin = match.group(1)
        if chemin.endswith('.png'):
            nom = chemin[:-4]  # retire l'extension .png
            # On ne garde que les noms qui commencent par 'grenos' (nos icones)
            if nom.startswith('grenos'):
                noms.add(nom)
    return noms


def _lire_binaires_disponibles():
    """Renvoie l'ensemble des noms de programmes disponibles dans les dossiers binaires."""
    binaires = set()
    for dossier in (DOSSOIR_BIN_USR, DOSSOIR_BIN_GRENOS):
        try:
            for nom in os.listdir(dossier):
                chemin = os.path.join(dossier, nom)
                if os.path.isfile(chemin) and os.access(chemin, os.X_OK):
                    binaires.add(nom)
        except OSError:
            # Si le dossier manque, on l'ignore simplement.
            pass
    return binaires


def _parser_desktop(contenu):
    """Extrait les champs Name, Exec, Icon d'un fichier .desktop."""
    name = None
    exec_ = None
    icon = None
    in_desktop_entry = False
    for ligne in contenu.splitlines():
        ligne = ligne.strip()
        if ligne == '[Desktop Entry]':
            in_desktop_entry = True
            continue
        if ligne.startswith('[') and ligne.endswith(']'):
            # Nouvelle section, on sort de [Desktop Entry]
            in_desktop_entry = False
            continue
        if not in_desktop_entry:
            continue
        if '=' not in ligne:
            continue
        cle, valeur = ligne.split('=', 1)
        cle = cle.strip()
        valeur = valeur.strip()
        if cle == 'Name':
            name = valeur
        elif cle == 'Exec':
            exec_ = valeur
        elif cle == 'Icon':
            icon = valeur
    return name, exec_, icon, in_desktop_entry


def _verifier_fichier(chemin, binaires, icones_disponibles):
    """Vérifie un fichier .desktop, affiche les erreurs et renvoie True si OK."""
    try:
        with open(chemin, encoding='utf-8') as f:
            contenu = f.read()
    except OSError as e:
        print(f'::error file={chemin}::impossible de lire le fichier — {e}')
        return False

    name, exec_, icon, dans_section = _parser_desktop(contenu)
    ok = True

    if not dans_section:
        print(f'::error file={chemin}::section [Desktop Entry] manquante')
        ok = False
    else:
        if not name:
            print(f'::error file={chemin}::champ Name manquant')
            ok = False
        if not exec_:
            print(f'::error file={chemin}::champ Exec manquant')
            ok = False
        if not icon:
            print(f'::error file={chemin}::champ Icon manquant')
            ok = False

    if exec_:
        # On prend le premier mot d'Exec (jusqu'au premier espace ou tabulation)
        premier_mot = exec_.split()[0] if exec_.split() else ''
        # On enlève les éventuels guillemets autour
        if premier_mot.startswith('"') and premier_mot.endswith('"'):
            premier_mot = premier_mot[1:-1]
        elif premier_mot.startswith("'") and premier_mot.endswith("'"):
            premier_mot = premier_mot[1:-1]
        # On considère comme nôtre tout nom dont le basename commence par 'grenos' ou 'gren'
        basename = os.path.basename(premier_mot)
        est_nôtre = basename.startswith('grenos') or basename.startswith('gren')
        if not est_nôtre and premier_mot not in binaires:
            print(f'::error file={chemin}::le programme « {premier_mot} » n\'est pas livré par grenOS ni présent dans Debian')
            ok = False

    if icon:
        # On considère comme nôtre toute icône dont le nom commence par 'grenos'
        if icon.startswith('grenos'):
            if icon not in icones_disponibles:
                print(f'::error file={chemin}::l\'icône « {icon} » n\'est pas produite par linux/tools/icones.py')
                ok = False

    return ok


def main(argv):
    binaires = _lire_binaires_disponibles()
    icones_disponibles = _lire_icones_disponibles()
    cibles = argv[1:] or [
        os.path.join(DOSSIER_APPLICATIONS, nom)
        for nom in sorted(os.listdir(DOSSIER_APPLICATIONS))
        if nom.endswith('.desktop')
    ]
    fautes = 0
    for chemin in cibles:
        if not os.path.isfile(chemin):
            continue
        if not _verifier_fichier(chemin, binaires, icones_disponibles):
            fautes += 1
    if fautes:
        return 1
    print(f'desktop : {len(cibles)} fichier(s) lus, chaque Exec et chaque icone existent')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
