#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Exercer la mécanique des onglets AVANT qu'elle ne touche à la fenêtre.

L'explorateur est l'explorateur par défaut depuis le 7 octobre, et la semaine
a montré ce qu'une modification de sa fenêtre peut coûter : la recherche
ajoutée le 15 a tué le renommage, et il a fallu une construction de quarante
minutes pour l'apprendre. La mécanique des onglets est donc écrite sans GTK et
jugée ici, comme celle de l'assistant de première configuration.

LES CAS QUI COMPTENT, et chacun est une façon de perdre son travail :

* **fermer le dernier onglet** laisserait une fenêtre sans liste ;
* **une histoire partagée** ferait que Précédent dans un onglet renvoie au
  dossier d'un autre — un bouton qui fait autre chose que ce qu'il dit ;
* **un rang hors bornes** (Ctrl+7 dans une rangée de deux) ne doit rien
  changer plutôt que de choisir au hasard ;
* **la racine** n'a pas de nom court : `basename("/")` rend une chaîne vide,
  et un onglet sans intitulé est un onglet où l'on ne peut plus revenir.
"""
import os
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(RACINE, "linux", "config", "includes.chroot",
                                "usr", "lib", "grenos"))
import onglets  # noqa: E402

echecs = []


def verifier(nom, obtenu, attendu):
    if obtenu == attendu:
        print("  [ok] %-56s %s" % (nom, obtenu))
    else:
        echecs.append(nom)
        print("  [KO] %-56s %r au lieu de %r" % (nom, obtenu, attendu))


# -- une rangée commence avec un seul onglet -----------------------------
r = onglets.Rangee("/home/ci")
verifier("une rangee neuve a un onglet", r.combien(), 1)
verifier("et il est actif", r.actif, 0)
verifier("son intitule est le nom du dossier", r.intitules(), ["ci"])

# -- LE DERNIER ONGLET NE SE FERME PAS -----------------------------------
verifier("fermer le dernier : refuse", r.fermer(), False)
verifier("il est toujours la", r.combien(), 1)

# -- ouvrir -------------------------------------------------------------
r.ouvrir("/home/ci/Documents")
verifier("un onglet de plus", r.combien(), 2)
verifier("le neuf est actif", r.actif, 1)
verifier("les intitules", r.intitules(), ["ci", "Documents"])

# Ouvrir depuis le PREMIER d'une rangee de trois doit poser le neuf JUSTE
# apres lui, pas a la fin.
r.aller_a(0)
r.ouvrir("/media/cle")
verifier("ouvert depuis le premier : pose en deuxieme",
         r.intitules(), ["ci", "cle", "Documents"])
verifier("et c est lui qui est actif", r.actif, 1)

# -- CHAQUE ONGLET GARDE SON HISTOIRE ------------------------------------
r = onglets.Rangee("/home/ci")
r.courant.aller("/home/ci/Documents")
r.courant.aller("/home/ci/Documents/travail")
r.ouvrir("/etc")
r.courant.aller("/etc/systemd")
verifier("l onglet 2 est dans /etc/systemd", r.courant.dossier, "/etc/systemd")
verifier("son precedent rend /etc", r.courant.precedent(), "/etc")
verifier("et il n a pas de precedent au-dela", r.courant.precedent(), None)
r.aller_a(0)
verifier("l onglet 1 est reste dans travail",
         r.courant.dossier, "/home/ci/Documents/travail")
verifier("son precedent rend Documents",
         r.courant.precedent(), "/home/ci/Documents")
verifier("puis la maison", r.courant.precedent(), "/home/ci")
verifier("et plus rien", r.courant.precedent(), None)
verifier("son suivant rend Documents",
         r.courant.suivant(), "/home/ci/Documents")

# Partir ailleurs depuis le milieu de l histoire efface le futur.
r.courant.aller("/tmp")
verifier("partir du milieu efface le futur", r.courant.suivant(), None)
verifier("et l histoire fait trois pas",
         r.courant.histoire, ["/home/ci", "/home/ci/Documents", "/tmp"])

# -- fermer ------------------------------------------------------------
r = onglets.Rangee("/a")
r.ouvrir("/b")
r.ouvrir("/c")
verifier("trois onglets, le troisieme actif", (r.combien(), r.actif), (3, 2))
verifier("fermer : accepte", r.fermer(), True)
verifier("c est le voisin de GAUCHE qui prend la main",
         (r.intitules(), r.actif), (["a", "b"], 1))
r.aller_a(0)
verifier("fermer le premier : l actif reste 0",
         (r.fermer(), r.actif, r.intitules()), (True, 0, ["b"]))

# -- aller_a, et les bornes -------------------------------------------
r = onglets.Rangee("/a")
r.ouvrir("/b")
verifier("Ctrl+7 dans une rangee de deux : refuse", r.aller_a(6), False)
verifier("et l actif n a pas bouge", r.actif, 1)
verifier("un rang negatif : refuse", r.aller_a(-1), False)
verifier("Ctrl+1 : accepte", (r.aller_a(0), r.actif), (True, 0))

# -- Ctrl+Tab tourne, et ne bloque jamais ------------------------------
r = onglets.Rangee("/a")
r.ouvrir("/b")
r.aller_a(1)
verifier("suivant depuis le dernier revient au premier",
         r.suivant().dossier, "/a")
verifier("precedent depuis le premier va au dernier",
         r.precedent().dossier, "/b")
seul = onglets.Rangee("/a")
verifier("sur un seul onglet, suivant ne bloque pas",
         seul.suivant().dossier, "/a")

# -- la racine a un nom -------------------------------------------------
verifier("la racine s intitule /", onglets.Rangee("/").intitules(), ["/"])
verifier("un chemin avec barre finale garde son nom",
         onglets.Rangee("/home/ci/Images/").intitules(), ["Images"])

print("")
if echecs:
    print("%d cas ont echoue." % len(echecs))
    sys.exit(1)
print("les 30 cas passent.")
