#!/usr/bin/env python3
"""Un fichier de flux qu'une réécriture a abîmé sans que rien ne le dise.

Trois images ont été perdues le 23 septembre parce que mes propres
modifications laissaient des traces : un « n » précédé d'une barre oblique
inverse au milieu d'une commande, une continuation de ligne mangée, un retour
chariot isolé. Chaque fois, le fichier avait l'air correct, et la construction
mourait vingt minutes plus tard sur `cp: cannot stat 'n'` ou
`-device: command not found`.

Ce contrôle est un **fichier à part**, et non un script écrit dans le YAML.
La raison est directe : les séquences d'échappement ne survivent pas à une
réécriture — c'est l'accident même qu'il sert à attraper, et il a été cassé
deux fois en essayant de l'écrire là-bas.

    python3 scripts/verif-flux.py .github/workflows/linux.yml

Sort en 0 si le fichier est sain, en 1 sinon, en nommant chaque ligne.
"""
import io
import re
import sys

# Un « n » isolé précédé d'une barre oblique inverse : ce que laisse une
# réécriture qui a pris un saut de ligne pour deux caractères.
#
# Écrit avec `chr(92)` et cherché tel quel, sans expression régulière : une
# barre oblique inverse doit être échappée dans une classe de caractères, puis
# de nouveau dans la chaîne qui la porte, et c'est là qu'on se trompe. Le
# fichier qui attrape cette faute ne doit pas la contenir.
BARRE_N = " " + chr(92) + "n "

def rafale_hors_chaine(ligne):
    """Une rafale d'espaces au milieu d'une commande, hors de toute chaîne.

    Une continuation de ligne mangée laisse exactement cela : la suite de la
    commande revient se coller sur la même ligne, à l'endroit où elle était
    indentée. Mais des espaces alignés dans un `sed 's/^   //'` sont
    parfaitement légitimes — d'où le suivi des guillemets, qui sépare les deux
    sans se tromper.
    """
    simple = double = False
    espaces = 0
    vu_du_texte = False
    for position, caractere in enumerate(ligne):
        if caractere == "'" and not double:
            simple = not simple
        elif caractere == '"' and not simple:
            double = not double

        if caractere == " " and not simple and not double:
            espaces += 1
            continue
        if espaces >= 3 and vu_du_texte and caractere != "#":
            return position
        espaces = 0
        if caractere not in " \t":
            vu_du_texte = True
    return None


def verifier(chemin):
    """Les reproches à faire à ce fichier, un par ligne."""
    # newline="" : aucune traduction. Un retour chariot isolé doit se voir tel
    # qu'il est, et non se déguiser en saut de ligne.
    texte = io.open(chemin, encoding="utf-8", newline="").read()
    fautes = []

    for numero, ligne in enumerate(texte.split("\n"), 1):
        nu = ligne.strip()
        if BARRE_N in ligne:
            fautes.append((numero, "un « n » precede d'une barre oblique inverse"))
        if "\r" in ligne:
            fautes.append((numero, "un retour chariot isole"))
        if nu.startswith("#") or not nu:
            continue
        if rafale_hors_chaine(ligne) is not None:
            fautes.append((numero, "une rafale d'espaces au milieu d'une commande, "
                                   "continuation mangee ? " + nu[:70]))

    # Le YAML se lit-il encore ? PyYAML n'est pas garanti partout : son absence
    # ne doit pas faire échouer le contrôle pour une mauvaise raison.
    try:
        import yaml
    except ImportError:
        print("PyYAML absent : la lecture du YAML n'est pas verifiee")
    else:
        try:
            yaml.safe_load(texte)
        except Exception as souci:
            fautes.append((0, "le YAML ne se lit pas : " + str(souci).split("\n")[0]))

    return fautes


def main(argv):
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 64
    fautes = verifier(argv[1])
    for numero, raison in fautes:
        ou = f"{argv[1]}:{numero}" if numero else argv[1]
        print(f"::error file={argv[1]},line={numero}::{ou} {raison}")
    if fautes:
        return 1
    print(f"{argv[1]} : aucune continuation perdue, aucun retour chariot, le YAML se lit")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
