#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Nos textes affichés ont-ils leurs accents ?

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
    return [mot for mot in SANS_ACCENT.findall(texte)
            if not re.search(r"[éèêëàâäîïôöûùüç]", mot)]


def textes_affiches(arbre):
    """Chaque chaîne littérale qui finit sous les yeux de quelqu'un."""
    for noeud in ast.walk(arbre):
        if not isinstance(noeud, ast.Call):
            continue

        nom = ""
        if isinstance(noeud.func, ast.Attribute):
            nom = noeud.func.attr
        elif isinstance(noeud.func, ast.Name):
            nom = noeud.func.id

        if nom in AFFICHEURS or nom in FABRIQUES_1:
            if noeud.args and isinstance(noeud.args[0], ast.Constant) \
                    and isinstance(noeud.args[0].value, str):
                yield noeud.lineno, noeud.args[0].value

        for mot_cle in noeud.keywords:
            if mot_cle.arg in CLES and isinstance(mot_cle.value, ast.Constant) \
                    and isinstance(mot_cle.value.value, str):
                yield noeud.lineno, mot_cle.value.value


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
    for ligne, texte in textes_affiches(arbre):
        for mot in fautes_du_texte(texte):
            trouves.append((ligne, mot, texte.strip()[:70]))
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
        for ligne, mot, texte in lire(chemin):
            print(f"{chemin}:{ligne} : « {mot} » sans accent, dans « {texte} »")
            total += 1

    if total:
        print(f"\n{total} texte(s) affiche(s) sans accent.")
        print("Un texte que quelqu'un lit porte ses accents. Un commentaire "
              "fait ce qu'il veut.")
        return 1
    print(f"accents : {lus} fichiers lus, tous les textes affiches sont accentues")
    return 0


if __name__ == "__main__":
    sys.exit(main())
