#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Exercer ce que l'assistant de première configuration FAIT, pas ce qu'il dit.

I7 est le seul écran de sa liste dont le choix installe quelque chose : « s'il
choisit *développement*, un bouton installe VS Code, Git, un terminal
amélioré, les outils de compilation ». Une question dont la réponse ne change
rien est une question posée pour faire joli, et neuf écrans sur dix de cet
assistant étaient dans ce cas.

CE QUE CET ESSAI ATTRAPE, et aucune relecture ne l'aurait vu :

1. **Un profil qui nomme une fiche absente du catalogue.** C'est la faute du
   25 septembre à l'identique — `partition.conf` offrait btrfs sans
   `mkfs.btrfs`. Le profil « développement » promet quatre applications ;
   si l'une sort du catalogue un jour, ce contrôle le dit le jour même.
2. **Une source inconnue ne doit pas donner une commande devinée** : lancer
   apt sur un identifiant Flatpak rendrait une erreur illisible au prochain
   démarrage de quelqu'un d'autre.
3. **Le programme rend toujours 0.** Il tourne dans la file d'avant-bureau,
   dont le propre commentaire dit qu'une erreur rejouée transforme la machine
   en boucle. Une application manquante est un ennui, une machine qui ne
   démarre plus est une machine perdue.

Aucun apt ni flatpak n'est lancé : le lanceur est remplacé par une fonction
qui note ce qu'on lui a demandé. On juge la commande construite, pas le
système de celui qui exécute l'essai.
"""
import io
import json
import os
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))
GRENOS = os.path.join(RACINE, "linux", "config", "includes.chroot",
                      "usr", "lib", "grenos")
sys.path.insert(0, GRENOS)
import premier  # noqa: E402

# `grenos-installer-choix` n'a pas d'extension .py — c'est un programme livré,
# pas un module. On le charge par son chemin exact, jamais par une copie.
import importlib.machinery  # noqa: E402
import importlib.util  # noqa: E402

chemin = os.path.join(GRENOS, "grenos-installer-choix")
specification = importlib.util.spec_from_loader(
    "installeur", importlib.machinery.SourceFileLoader("installeur", chemin))
installeur = importlib.util.module_from_spec(specification)
specification.loader.exec_module(installeur)

echecs = []


def dire_cas(nom, souci=None, detail=""):
    if souci:
        echecs.append(nom)
        print("  [KO] %-56s %s" % (nom, souci))
    else:
        print("  [ok] %-56s %s" % (nom, detail))


# ---------------------------------------------------------------------------
# 1. CHAQUE PROFIL NE NOMME QUE DES FICHES QUI EXISTENT.
# ---------------------------------------------------------------------------
catalogue = json.load(io.open(
    os.path.join(RACINE, "linux", "data", "catalogue.json"), encoding="utf-8"))
liste = catalogue.get("applications", catalogue)
connus = set(f.get("slug") for f in liste)

for cle, intitule, _note, paquets in premier.USAGES:
    absents = [p for p in paquets if p not in connus]
    if absents:
        dire_cas("profil « %s »" % cle,
                 "nomme %s, absent(s) du catalogue" % ", ".join(absents))
    else:
        dire_cas("profil « %s »" % cle, None,
                 "%d fiche(s), toutes au catalogue" % len(paquets))

# Et le cas qui rend l'assertion vivante : un profil fabriqué qui mentirait.
faux = [p for p in ("vscode", "grenos-nexistepas") if p not in connus]
dire_cas("un profil qui nommerait une fiche absente serait vu",
         None if faux == ["grenos-nexistepas"] else "le controle ne detecte rien",
         "detecte : %s" % ", ".join(faux))

# ---------------------------------------------------------------------------
# 2. LA COMMANDE CONSTRUITE POUR CHAQUE SOURCE.
# ---------------------------------------------------------------------------
cas_commande = (
    ({"identifiant": "git", "source": "apt"}, "apt-get", "git"),
    ({"identifiant": "org.kde.kdenlive", "source": "flatpak"},
     "flatpak", "org.kde.kdenlive"),
)
for fiche, programme, identifiant in cas_commande:
    commande = installeur.commande_pour(fiche)
    if not commande or commande[0] != programme or identifiant not in commande:
        dire_cas("source %s" % fiche["source"], "commande %s" % commande)
    else:
        dire_cas("source %s" % fiche["source"], None, " ".join(commande))

for fiche in ({"identifiant": "truc", "source": "snap"},
              {"identifiant": "", "source": "apt"},
              {"source": "apt"}):
    if installeur.commande_pour(fiche) is None:
        dire_cas("fiche bancale (%s) : aucune commande devinee"
                 % (fiche.get("source") or "sans source"), None, "None")
    else:
        dire_cas("fiche bancale (%s)" % fiche.get("source"),
                 "une commande a ete devinee")

# ---------------------------------------------------------------------------
# 3. CE QUE `installer()` FAIT, avec un lanceur qui ne lance rien.
# ---------------------------------------------------------------------------
faux_catalogue = {
    "git": {"slug": "git", "identifiant": "git", "source": "apt"},
    "kdenlive": {"slug": "kdenlive", "identifiant": "org.kde.kdenlive",
                 "source": "flatpak"},
    "bizarre": {"slug": "bizarre", "identifiant": "x", "source": "snap"},
}


def lanceur_qui_reussit(commande, delai=0):
    lanceur_qui_reussit.vues.append((commande, delai))
    return 0, ""


def lanceur_qui_echoue(commande, delai=0):
    lanceur_qui_echoue.vues.append((commande, delai))
    return 100, "le depot ne repond pas"


lanceur_qui_reussit.vues = []
lanceur_qui_echoue.vues = []

poses, manquees = installeur.installer(
    ["git", "kdenlive"], faux_catalogue, lanceur_qui_reussit)
dire_cas("deux fiches bonnes : 2 posees, 0 manquee",
         None if (poses, manquees) == (2, 0) else "%d/%d" % (poses, manquees),
         "%d commande(s) lancee(s)" % len(lanceur_qui_reussit.vues))

poses, manquees = installeur.installer(
    ["git", "grenos-nexistepas"], faux_catalogue, lanceur_qui_reussit)
dire_cas("une fiche absente du catalogue est COMPTEE MANQUEE",
         None if (poses, manquees) == (1, 1) else "%d/%d" % (poses, manquees))

poses, manquees = installeur.installer(
    ["bizarre"], faux_catalogue, lanceur_qui_reussit)
dire_cas("une source inconnue ne lance rien du tout",
         None if (poses, manquees) == (0, 1) else "%d/%d" % (poses, manquees))

avant = len(lanceur_qui_echoue.vues)
poses, manquees = installeur.installer(
    ["git", "kdenlive"], faux_catalogue, lanceur_qui_echoue)
dire_cas("un depot muet : 0 posee, 2 manquees, et les DEUX sont essayees",
         None if (poses, manquees, len(lanceur_qui_echoue.vues) - avant)
         == (0, 2, 2) else "%d/%d/%d"
         % (poses, manquees, len(lanceur_qui_echoue.vues) - avant))

# ---------------------------------------------------------------------------
# 3bis. LE BUDGET. La file coupe un travail a 600 s ; se faire couper au milieu
# d'un `dpkg` est la façon classique de casser une machine. On verifie donc
# qu'on s'arrete AVANT, proprement, et qu'on le DIT.
# ---------------------------------------------------------------------------
pendule = {"t": 0.0}


def horloge_qui_saute():
    pendule["t"] += 300.0
    return pendule["t"]


poses, manquees = installeur.installer(
    ["git", "kdenlive", "git"], faux_catalogue, lanceur_qui_reussit,
    horloge=horloge_qui_saute, budget=480)
dire_cas("budget ecoule : la premiere passe, les suivantes sont dites",
         None if (poses, manquees) == (1, 2) else "%d posee(s), %d manquee(s)"
         % (poses, manquees))

pendule["t"] = 0.0
avant = len(lanceur_qui_reussit.vues)
poses, manquees = installeur.installer(
    ["git"], faux_catalogue, lanceur_qui_reussit,
    horloge=horloge_qui_saute, budget=0)
dire_cas("budget nul : la PREMIERE est quand meme essayee",
         None if (poses, len(lanceur_qui_reussit.vues) - avant) == (1, 1)
         else "%d posee(s), %d lancee(s)"
         % (poses, len(lanceur_qui_reussit.vues) - avant),
         "sinon un budget mal regle n installerait jamais rien")

derniere = lanceur_qui_reussit.vues[-1]
dire_cas("le delai passe au lanceur ne depasse jamais le budget",
         None if derniere[1] <= installeur.DELAI
         else "delai %s" % derniere[1], "delai %s s" % derniere[1])

# ---------------------------------------------------------------------------
# 4. ET LE PROGRAMME REND TOUJOURS 0.
# ---------------------------------------------------------------------------
code = installeur.main(["grenos-installer-choix"])
dire_cas("sans argument : rend 0", None if code == 0 else "rend %s" % code)

# Un catalogue absent : la lecture doit rendre un dictionnaire vide, pas lever.
vide = installeur.fiches("/chemin/qui/n/existe/pas.json")
dire_cas("catalogue absent : rend un dictionnaire vide",
         None if vide == {} else "rend %r" % vide)

print("")
if echecs:
    print("%d cas ont echoue." % len(echecs))
    sys.exit(1)
print("tous les cas passent.")
