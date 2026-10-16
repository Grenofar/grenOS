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
    fautes.extend(outil_fantome(chemin, racine))
    fautes.extend(fonctions_fantomes(texte, racine))
    return fautes


MODULES = os.path.join("linux", "config", "includes.chroot", "usr", "lib",
                       "grenos")

APPEL = re.compile(r"`([a-z_][a-z_0-9]*)\.([a-z_][a-z_0-9]*)\(")


def fonctions_fantomes(texte, racine=RACINE):
    """Une fonction citée comme `module.fonction()` doit exister.

    C'est la deuxième chose mécanique qu'on puisse vérifier dans un document,
    après les chemins — et c'est celle qui compte le plus pour un document
    d'outil, puisqu'elle est la seule façon de rattacher une phrase à du code.
    Le document de grenHub livré le 16 octobre en cite dix-huit : les
    dix-huit existent, et c'est ce qui permet de le croire sur le reste.

    On ne juge QUE les modules qu'on livre. Une citation de `json.load()` ou
    de `os.path.exists()` n'est pas une affirmation sur notre code, et la
    refuser remplirait le journal de fautes qu'on ne veut pas corriger.
    """
    fautes = []
    for module, fonction in sorted(set(APPEL.findall(texte))):
        source = os.path.join(racine, MODULES, module + ".py")
        if not os.path.exists(source):
            continue
        try:
            code = io.open(source, encoding="utf-8").read()
        except OSError:
            continue
        if ("def %s(" % fonction) not in code:
            fautes.append("%s.%s() — ce module ne porte pas cette fonction"
                          % (module, fonction))
    return fautes


# OU VIVENT LES PROGRAMMES QU UN DOCUMENT D OUTIL PEUT DECRIRE.
LIVRES = (os.path.join("linux", "config", "includes.chroot", "usr", "bin"),
          os.path.join("linux", "config", "includes.chroot", "usr", "lib",
                       "grenos"),
          os.path.join("linux", "tools"),
          "scripts")


def outil_fantome(chemin, racine=RACINE):
    """Un document d'outil doit porter le nom d'un outil qui existe.

    LE 15 OCTOBRE, DEUX TACHES D AGENT ONT LIVRE
    `docs/linux/outils/grenos-outil-de-base.md` — « l'outil de base du
    système grenOS ». **Ce programme n'existe pas**, sous aucun nom : le
    document pointait vers `grenos-shell` tout en se faisant appeler
    autrement, et il lui attribuait un verrouillage d'écran, une zone de
    notification et une roue dentée dont aucune n'apparait dans le code
    (`verrouill`, `notification`, `roue` : zero occurrence, comptees).

    Les deux taches ont ete marquees **done** et fusionnees toutes seules.
    Ce garde-ci ne juge pas la description — c'est dit plus haut, et ca ne
    change pas. Il juge la seule chose qui soit mecanique : le NOM. Un
    document rangé sous `docs/.../outils/` et nomme d'apres rien est un
    renvoi vers le vide, et il est pire qu'un document absent, parce qu'on
    le croit.
    """
    dossier = os.path.basename(os.path.dirname(os.path.abspath(chemin)))
    if dossier != "outils":
        return []
    nom = os.path.basename(chemin)[:-3]
    for coin in LIVRES:
        for suffixe in ("", ".py", ".sh"):
            if os.path.exists(os.path.join(racine, coin, nom + suffixe)):
                return []
    return ["(aucun outil ne s'appelle « %s »)" % nom]


def main(argv):
    docs = argv[1:]
    if not docs:
        # ON DESCEND DANS LES SOUS-DOSSIERS, et c'est ce qui manquait : le
        # document fantome du 15 octobre vivait dans `docs/linux/outils/`,
        # ou ce controle ne regardait pas. Un garde qui ne lit que le premier
        # etage ne garde que le premier etage.
        docs = []
        for ou, _dossiers, fichiers in os.walk(os.path.join(RACINE, "docs")):
            # `docs/specs/` EST UNE ARCHIVE, pas un guide : chaque spec est
            # une affirmation a une date, et un chemin qu'elle nomme a pu
            # etre retire depuis — `bienvenue.html` l'a ete avec KDE,
            # `grenos-disk.img` avec le noyau Rust. Les corriger reecrirait
            # ce qui a ete demande ce jour-la ; les refuser remplirait le
            # journal de fautes qu'on ne veut pas corriger, et un garde
            # bruyant finit ignore.
            if os.path.basename(ou) == "specs":
                continue
            for nom in sorted(fichiers):
                if nom.endswith(".md"):
                    docs.append(os.path.join(ou, nom))
        docs.sort()
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
