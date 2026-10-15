#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Nos propres outils compilent-ils ?

POURQUOI CE FICHIER EXISTE — et c'est un trou, pas une precaution.

Le 15 octobre, un agent Codeur a livre `linux/tools/verif-catalogue.py`. Sa
tache a ete marquee **done**, et le fichier ne compile pas : son en-tete porte
`\\"\\"\\"` au lieu de `\"\"\"`, et Python s'arrete a la deuxieme ligne.

Rien ne l'a vu, et c'est normal : la construction compile ce qui part dans
l'image (`/usr/lib/grenos`, `/usr/bin`), pas `linux/tools/`, qui reste sur la
machine qui construit. Un outil casse y dormait donc jusqu'au jour ou
quelqu'un l'appelle — c'est-a-dire le jour ou il devait servir.

C'est la meme forme que le `| tail` du 25 septembre : un garde dont personne
n'entend l'echec. Et c'est la deuxieme fois en une journee qu'un travail
d'agent passe au vert sans faire ce qu'il annonce.

CE CONTROLE NE DIT PAS QUE L'OUTIL EST JUSTE. Il dit qu'il demarre. C'est peu,
et c'est exactement ce qui manquait : un fichier qui ne compile pas ne peut
rien refuser, donc il rassure sans rien garder.
"""
import ast
import io
import os
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOSSIERS = ("tools",)


def fichiers(racine=RACINE):
    for dossier in DOSSIERS:
        chemin = os.path.join(racine, dossier)
        if not os.path.isdir(chemin):
            continue
        for nom in sorted(os.listdir(chemin)):
            if nom.endswith(".py"):
                yield os.path.join(chemin, nom)


def verifier(chemin):
    """None si le fichier compile, sinon la raison en une ligne."""
    try:
        contenu = io.open(chemin, encoding="utf-8").read()
    except (OSError, UnicodeDecodeError) as souci:
        return "illisible : %s" % souci
    try:
        ast.parse(contenu, chemin)
    except SyntaxError as souci:
        return "ligne %s : %s" % (souci.lineno, souci.msg)
    return None


def main(argv):
    cibles = argv[1:] or list(fichiers())
    fautes = 0
    for chemin in cibles:
        souci = verifier(chemin)
        if souci:
            relatif = os.path.relpath(chemin, os.path.dirname(RACINE))
            relatif = relatif.replace("\\", "/")
            print("::error file=%s::%s ne compile pas — %s"
                  % (relatif, relatif, souci))
            fautes += 1
    if fautes:
        print("Un outil qui ne compile pas ne peut rien refuser : il rassure "
              "sans rien garder.", file=sys.stderr)
        return 1
    print("outils : %d fichier(s) lus, tous compilent" % len(cibles))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
