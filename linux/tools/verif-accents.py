#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Nos textes affichés sont-ils écrits en français ?

Deux règles, et la même cause : une phrase qui se lit encore passe toutes les
relectures. Les accents d'abord, puis, depuis le 28 septembre, la virgule
décimale — « 2.9 Go de mémoire » était à l'écran du mode Jeux depuis des
semaines, et c'est en agrandissant une capture qu'on l'a vu.

Une phrase sans accents se lit encore, et c'est exactement le problème : elle
passe les relectures. Le 24 septembre, le catalogue du magasin en portait deux
(« Telecharger », « reseau ») que personne n'avait vues pendant une semaine ;
le 25, l'infobulle du réseau en portait trois, écrites une heure plus tôt, dans
un texte que je venais moi-même de relire.

La cause est toujours la même : j'écris du français sans accents dans les
commentaires pour survivre aux heredocs du shell, et la main continue quand
elle passe à une chaîne qui, elle, s'affiche.

Ce contrôle ne regarde que ce qui **s'affiche vraiment** : le texte donné à une
étiquette, une infobulle, un bouton, un titre, un champ de saisie. Les
commentaires, les noms de classes CSS, les noms d'icônes et les signaux GObject
sont laissés tranquilles — s'y intéresser noierait les vrais défauts sous cent
faux.

    python3 linux/tools/verif-accents.py [dossier...]

Sort en 0 si tout va bien, en 1 en nommant chaque texte fautif.
"""
import ast
import glob
import io
import os
import re
import sys

# Les mots français courants qu'on écrit le plus souvent sans accent. La liste
# est volontairement courte : un mot n'y entre qu'après être vraiment passé.
SANS_ACCENT = re.compile(
    r"\b(?:"
    r"telecharg\w*|reseau\w*|systeme\w*|parametre\w*|peripherique\w*|"
    r"prefere\w*|deja|apres|tres|ecran\w*|verifi\w*|securite\w*|demarr\w*|"
    r"arrete\w*|reglage\w*|acces|fenetre\w*|numero\w*|precedent\w*|"
    r"derniere|premiere|entree\w*|creer|repertoire\w*|donnee\w*|"
    # « connecte » est retire de la liste : « se connecter », « connecte-toi »
    # et « il se connecte » sont du francais juste, et seul « connecte » au sens
    # de « connecte » (participe) est fautif. Un controle qui crie a tort finit
    # par etre ignore, et c'est pire que pas de controle du tout.
    r"controle\w*|selectionn\w*|desactiv\w*|cable\w*|"
    r"reussi\w*|echoue\w*|termine\w*|annule\w*|installe\w*e|"
    r"disponible\w*s?|necessaire\w*|energie|memoire|duree|etat|elements?"
    r")\b", re.I)

# Le « à » écrit « a ». C'est la faute d'accent la plus courante du français, et
# la liste de mots ci-dessus ne pouvait pas l'attraper : elle cherche des mots
# entiers, or ici le mot fautif fait une seule lettre.
#
# On ne peut pas interdire « a » tout court — « il a », « on a », « elle a »
# sont justes, et un contrôle qui crie à tort finit par être ignoré. Mais dans
# ces locutions-là, « a » est TOUJOURS la préposition : aucune phrase française
# n'écrit « mise a jour » ni « a partir de ».
#
# Trouvé le 26 septembre dans la pastille de la barre — « 3 mises a jour »,
# affiché à tout le monde, passé sous tous les contrôles depuis des jours.
LOCUTIONS = re.compile(
    r"\b(?:mises? a jour|est a jour|a jour|a partir|a cote|a nouveau|"
    r"a distance|a droite|a gauche|a propos|a venir|a suivre|jusqu a)\b", re.I)

# Le point décimal anglais. En français, 2,9 Go — jamais 2.9 Go.
#
# Trouvé le 28 septembre en agrandissant les captures du mode Jeux et du
# gestionnaire de tâches : « 2.9 Go de mémoire », « 1.5 % », « 0.5 Go sur
# 2.9 Go », affichés depuis des semaines. C'est la faute sans accent
# exactement : le texte se lit encore, donc personne ne le voit.
#
# On ne regarde que la façon dont un nombre est MIS EN FORME — « :.1f »,
# « %.1f », « :, » —, jamais le texte autour : une version « 1.0.202609 » ou
# une adresse « 127.0.0.1 » sont justes, et un contrôle qui crie à tort finit
# par être ignoré. Une précision de zéro décimale ne montre aucun point.
#
# Dans une consigne de format, la virgule ne peut être que le séparateur de
# milliers anglais (« 1,234 »), qui en français veut dire 1,234 — l'inverse.
POINT_DECIMAL = re.compile(r"\.[1-9][0-9]*[fFeEgG%]|,")
# Les deux autres façons d'écrire un nombre, dans un texte tout fait :
# « "%.1f Go" % reste » et « "{:.1f} Go".format(reste) ».
DANS_LE_TEXTE = re.compile(r"%\.[1-9][0-9]*[fFeEgG]"
                           r"|\{[^{}]*:[^{}]*\.[1-9][0-9]*[fFeEgG%][^{}]*\}")

# Ce par quoi un texte arrive sous les yeux de quelqu'un.
AFFICHEURS = {
    "set_tooltip_text", "set_text", "set_label", "set_title",
    "set_placeholder_text", "set_markup", "set_subtitle",
}
# Les fabriques dont le PREMIER argument est un texte affiché.
FABRIQUES_1 = {"bouton", "titre"}
# Les mots-clés qui portent un texte affiché.
CLES = {"label", "title", "text", "tooltip_text", "placeholder_text"}


def fautes_du_texte(texte):
    """Les mots sans accent de ce texte, s'il en a."""
    if not texte or not texte.strip():
        return []
    # Un identifiant technique ne contient pas d'espace : « ligne-reseau »,
    # « grenos-parametres », « notify::active ». Ce n'est pas une phrase.
    if " " not in texte.strip():
        return []
    fautes = [mot for mot in SANS_ACCENT.findall(texte)
              if not re.search(r"[éèêëàâäîïôöûùüç]", mot)]
    return fautes + LOCUTIONS.findall(texte)


def _litteral(noeud):
    """Le texte et les consignes de format d'un argument affiché.

    Une chaîne toute faite, mais aussi une f-string : le garde n'en lisait
    aucune, et c'est précisément là que vivaient « 2.9 Go » et « 1.5 % ».
    On rend le texte visible (les morceaux littéraux recollés) et, à part,
    chaque consigne de format — « .1f », « ,.0f » — qui décide de la façon
    dont un nombre sera écrit.
    """
    # « "%.1f Go" % reste » : l'argument n'est pas une chaîne, c'est une
    # opération. Le garde ne voyait donc rien — trouvé en l'exerçant contre un
    # fichier fautif écrit exprès, pas en le relisant.
    if isinstance(noeud, ast.BinOp) and isinstance(noeud.op, ast.Mod):
        noeud = noeud.left
    # « "{:.1f} Go".format(reste) » : le texte est porté par l'appelé.
    elif isinstance(noeud, ast.Call) and isinstance(noeud.func, ast.Attribute) \
            and noeud.func.attr == "format":
        noeud = noeud.func.value

    if isinstance(noeud, ast.Constant) and isinstance(noeud.value, str):
        return noeud.value, []
    if not isinstance(noeud, ast.JoinedStr):
        return None, []

    texte, consignes = "", []
    for morceau in noeud.values:
        if isinstance(morceau, ast.Constant) and isinstance(morceau.value, str):
            texte += morceau.value
        elif isinstance(morceau, ast.FormattedValue):
            texte += "0"          # un nombre ou un mot y prendra la place
            spec = morceau.format_spec
            if isinstance(spec, ast.JoinedStr):
                consignes.append("".join(
                    part.value for part in spec.values
                    if isinstance(part, ast.Constant)
                    and isinstance(part.value, str)))
    return texte, consignes


def textes_affiches(arbre):
    """Chaque texte qui finit sous les yeux de quelqu'un, et ses formats."""
    for noeud in ast.walk(arbre):
        if not isinstance(noeud, ast.Call):
            continue

        nom = ""
        if isinstance(noeud.func, ast.Attribute):
            nom = noeud.func.attr
        elif isinstance(noeud.func, ast.Name):
            nom = noeud.func.id

        candidats = []
        if (nom in AFFICHEURS or nom in FABRIQUES_1) and noeud.args:
            candidats.append(noeud.args[0])
        candidats += [mot_cle.value for mot_cle in noeud.keywords
                      if mot_cle.arg in CLES]

        for candidat in candidats:
            texte, consignes = _litteral(candidat)
            if texte is not None:
                yield noeud.lineno, texte, consignes


def lire(chemin):
    """Les fautes d'un fichier Python, s'il en a."""
    try:
        with io.open(chemin, encoding="utf-8") as fichier:
            contenu = fichier.read()
    except OSError:
        return []
    if not chemin.endswith(".py") and "python3" not in contenu.split("\n", 1)[0]:
        return []
    try:
        arbre = ast.parse(contenu, chemin)
    except SyntaxError:
        return []          # la syntaxe est jugée ailleurs, et avant celle-ci

    trouves = []
    for ligne, texte, consignes in textes_affiches(arbre):
        for mot in fautes_du_texte(texte):
            trouves.append((ligne, f"« {mot} » sans accent", texte.strip()[:70]))
        for consigne in consignes:
            if POINT_DECIMAL.search(consigne):
                trouves.append((ligne, f"le format « {consigne} » écrit un "
                                       "nombre à l'anglaise",
                                texte.strip()[:70]))
        for consigne in DANS_LE_TEXTE.findall(texte):
            trouves.append((ligne, f"le format « {consigne} » écrit un nombre "
                                   "à l'anglaise", texte.strip()[:70]))
    return trouves


def main():
    dossiers = sys.argv[1:] or [
        "linux/config/includes.chroot/usr/bin",
        "linux/config/includes.chroot/usr/lib/grenos",
    ]
    chemins = []
    for dossier in dossiers:
        if os.path.isfile(dossier):
            chemins.append(dossier)
        else:
            chemins += sorted(glob.glob(os.path.join(dossier, "*")))

    total, lus = 0, 0
    for chemin in chemins:
        if not os.path.isfile(chemin):
            continue
        lus += 1
        for ligne, faute, texte in lire(chemin):
            print(f"{chemin}:{ligne} : {faute}, dans « {texte} »")
            total += 1

    if total:
        print(f"\n{total} texte(s) affiche(s) mal ecrit(s).")
        print("Un texte que quelqu'un lit porte ses accents, et ses nombres "
              "prennent la virgule. Un commentaire fait ce qu'il veut.")
        return 1
    print(f"accents : {lus} fichiers lus, tous les textes affiches sont "
          "accentues et leurs nombres a la francaise")
    return 0


if __name__ == "__main__":
    sys.exit(main())
