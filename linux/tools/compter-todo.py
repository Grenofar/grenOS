#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Compter où en est la liste de Grenofar, sans se faire avoir par la prose.

POURQUOI CE PROGRAMME EXISTE : j'ai compté à la main, trois fois, et la
troisième fois le chiffre était faux. Je cherchais le mot « prouvé » n'importe
où dans la ligne — et une ligne marquée **[écrit]** qui expliquait « la file
d'avant-bureau, **prouvée** depuis le 27 septembre » était comptée comme
prouvée. Trois demandes ont changé d'état parce que j'avais écrit une phrase.

*Une mesure fausse ne se contredit jamais* : elle m'aurait donné 37 % au lieu
de 34 %, calmement, aussi longtemps que je l'aurais interrogée.

LE MARQUEUR EST EN TETE DE LA COLONNE D'ETAT, et nulle part ailleurs. On ne
lit donc que les premiers caractères de cette colonne, et tout ce qui vient
après — l'explication, les chiffres, les dates — est ignoré. Un état qu'on ne
reconnait pas est compte a part et NOMME, plutot que range dans « a faire » :
se tromper en silence est ce qu'on essaie d'eviter ici.
"""
import io
import os
import re
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))

SECTIONS = {
    "A": "materiel", "B": "fichiers et demarrage", "C": "recuperation",
    "D": "securite", "E": "observer", "F": "energie et ecrans",
    "G": "applications", "H": "GrenAI", "I": "premiere configuration",
    "J": "la promesse", "K": "documentation",
}

LIGNE = re.compile(r"^\|\s*([A-Z])(\d+)\s*\|(.*)\|(.*)\|\s*$")


def etat_de(colonne):
    """L'état que porte la colonne, lu sur son début seulement."""
    debut = colonne.strip().lstrip("*").strip()
    for marque, nom in (("[prouvé]", "prouve"),
                        ("[prouvé en partie]", "prouve en partie"),
                        ("[écrit]", "ecrit"),
                        ("[à faire]", "a faire"),
                        ("[à vérifier]", "a verifier"),
                        ("[à mesurer]", "a mesurer"),
                        ("[partiel]", "partiel"),
                        ("[sa main]", "sa main")):
        if debut.startswith(marque):
            return nom
    # Les formes composées, toujours en tête : « [prouvé] en partie », etc.
    for marque, nom in (("[prouvé", "prouve"), ("[écrit", "ecrit"),
                        ("[à faire", "a faire"), ("[partiel", "partiel"),
                        ("[mesuré", "mesure"), ("[à ", "a faire")):
        if debut.startswith(marque):
            return nom
    return "?"


def compter(chemin=None):
    chemin = chemin or os.path.join(RACINE, "todo.md")
    texte = io.open(chemin, encoding="utf-8").read()
    demandes = []
    for ligne in texte.split("\n"):
        trouve = LIGNE.match(ligne)
        if trouve:
            demandes.append((trouve.group(1), int(trouve.group(2)),
                             etat_de(trouve.group(4))))
    return demandes


def main(argv):
    demandes = compter(argv[1] if len(argv) > 1 else None)
    if not demandes:
        print("aucune demande lue — le tableau a change de forme")
        return 1
    faits = [d for d in demandes if d[2] in ("prouve", "ecrit")]
    inconnus = [d for d in demandes if d[2] == "?"]

    print("%d demandes | %.1f %% faites (%d prouvees + %d ecrites)"
          % (len(demandes), 100.0 * len(faits) / len(demandes),
             sum(1 for d in demandes if d[2] == "prouve"),
             sum(1 for d in demandes if d[2] == "ecrit")))
    for lettre in sorted(SECTIONS):
        dedans = [d for d in demandes if d[0] == lettre]
        if not dedans:
            continue
        bons = [d for d in dedans if d[2] in ("prouve", "ecrit")]
        print("  %s %-24s %2d/%2d  %3.0f %%"
              % (lettre, SECTIONS[lettre], len(bons), len(dedans),
                 100.0 * len(bons) / len(dedans)))
    if inconnus:
        print("  états non reconnus (comptés comme non faits) : %s"
              % ", ".join("%s%d" % (d[0], d[1]) for d in inconnus))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
