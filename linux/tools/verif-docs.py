#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Un document qui nomme un fichier qui n'existe pas.

POURQUOI CE FICHIER EXISTE — et c'est un document livré, pas une précaution.

Le 15 octobre, un agent Codeur a écrit `docs/DEVELOPPEUR.md`. La CI était
verte : le fichier se lisait, le Markdown était valide, la tâche était close
et la branche fusionnée toute seule. Le document décrivait **quatre gardes sur
cinq de travers** — `verif-flux.py` y « refusait les rebases », `verif-shell.py`
y « attendait set -euo pipefail », `verif-recommandes.py` y « refusait les
paquets non recommandés » alors que sa propre conclusion dit « Ce contrôle ne
refuse rien ». Et il se terminait par : « Aucune commande ou chemin n'est
inventé dans ce document ».

C'est mot pour mot la leçon du 26 septembre : *une CI verte dit que le fichier
se lit, elle ne dit pas qu'il dit vrai.*

CE QUE CE CONTRÔLE PEUT ET NE PEUT PAS FAIRE, et il faut le dire franchement :

- il **peut** vérifier qu'un chemin cité existe. C'est mécanique, et cela aurait
  attrapé `grenos-desktop` décrit comme un binaire de `includes.chroot` ;
- il **ne peut pas** vérifier qu'une description est juste. Aucune machine ne
  sait lire « refuse les rebases » et répondre « non, il lit du YAML ». Cela
  reste à la relecture, et c'est pour cela que la règle écrite en tête du
  document compte plus que ce garde : *on ne décrit pas un outil sans l'ouvrir.*

Prétendre le contraire serait exactement le défaut qu'il poursuit.
"""
import io
import os
import re
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

MOTIF = re.compile(r"`([^`\n]+)`")

# ON NE PREND QUE CE QUI EST VRAIMENT UNE AFFIRMATION DE CHEMIN : une citation
# qui commence par un dossier de premier niveau du depot. Le reste —
# `verify.yml`, `index.json`, `make-iso.sh` — est une abreviation de
# conversation, pas une indication pour aller chercher un fichier.
#
# La premiere version de ce controle prenait toute citation contenant une barre
# oblique ou une extension connue. Elle a rendu cinquante fautes sur des
# documents d'archive, dont `.md`, `@supabase/supabase-js` et un nom de
# branche. *Sinon cent faux noieraient les vrais* — c'est la raison ecrite
# dans verif-accents.py, et elle vaut ici mot pour mot. Un garde bruyant finit
# ignore, et un garde ignore ne garde rien.
RACINES = ("linux/", "scripts/", "docs/", "worker/", "apps/", "packages/",
           "kernel/", "agents/", ".github/")


def chemins_cites(texte):
    """Les chemins du depot qu'un document nomme, sans les faux amis."""
    vus = []
    for brut in MOTIF.findall(texte):
        candidat = brut.strip()
        if not candidat.startswith(RACINES):
            continue
        # Un glob (`usr/bin/grenos-*`) ou une commande avec des arguments ne
        # designe pas un fichier unique : on ne peut pas le verifier, donc on
        # ne pretend pas le faire.
        if "*" in candidat or " " in candidat or "<" in candidat:
            continue
        vus.append(candidat)
    return vus


def verifier(chemin, racine=RACINE):
    texte = io.open(chemin, encoding="utf-8").read()
    fautes = []
    for cite in chemins_cites(texte):
        cible = os.path.join(racine, cite.rstrip("/"))
        if not os.path.exists(cible):
            fautes.append(cite)
    return fautes


def main(argv):
    docs = argv[1:]
    if not docs:
        dossier = os.path.join(RACINE, "docs")
        docs = [os.path.join(dossier, n) for n in sorted(os.listdir(dossier))
                if n.endswith(".md")]
        todo = os.path.join(RACINE, "todo.md")
        if os.path.exists(todo):
            docs.append(todo)
    total = 0
    mauvais = 0
    for doc in docs:
        fautes = verifier(doc)
        total += 1
        for cite in fautes:
            relatif = os.path.relpath(doc, RACINE).replace("\\", "/")
            print("::error file=%s::%s nomme « %s », qui n'existe pas dans le depot"
                  % (relatif, relatif, cite))
            mauvais += 1
    if mauvais:
        print("Un document qui nomme un fichier absent envoie quelqu'un le "
              "chercher. Et s'il se trompe sur un chemin, rien ne garantit "
              "qu'il dise vrai sur le reste.", file=sys.stderr)
        return 1
    print("docs : %d fichier(s) lus, chaque chemin cite existe" % total)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
