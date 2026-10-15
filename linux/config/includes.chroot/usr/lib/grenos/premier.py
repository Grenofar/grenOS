# -*- coding: utf-8 -*-
"""Le parcours de première configuration : quels écrans, dans quel ordre.

Section I de sa liste — le plus détaillé de ses souhaits, et le plus visible :
c'est le premier écran que voit quelqu'un qui installe grenOS.

**Aucun GTK ici, et aucune action.** Ce module ne décide que de l'enchaînement.
C'est ce qui permet de l'exercer sans écran, et c'est la raison pour laquelle
il existe avant la fenêtre : `grenos-premier` est le seul programme dont une
panne empêche d'entrer dans la machine. On prouve la mécanique d'abord, on
dessine ensuite.

LES TROIS PARCOURS, et ce qu'il en dit mot pour mot :

* **Rapide** — « le moins d'écrans possible » ;
* **Personnalisé (recommandé)** — toutes les questions ;
* **Dev** — tout cela **plus** partitionnement, chiffrement, Secure Boot,
  swap, ordonnanceur, services, réseau, pare-feu, télémétrie, journaux, mises
  à jour, bac à sable et paramètres du noyau.

Le choix du parcours arrive **après la langue, le clavier et le nom** : c'est
son ordre, et il est juste. Demander « quel parcours ? » à quelqu'un qui n'a
pas encore choisi sa langue, c'est lui demander de choisir dans une langue
qu'il ne lit peut-être pas.
"""

RAPIDE = "rapide"
PERSONNALISE = "personnalise"
DEV = "dev"

PARCOURS = (RAPIDE, PERSONNALISE, DEV)

# Chaque écran : identifiant, titre, et les parcours qui le montrent.
#
# L'ordre de ce tableau EST l'ordre des écrans. Un seul endroit décide, donc
# il n'y a rien qui puisse diverger — pas de liste d'ordre à côté d'une liste
# d'écrans, qui finiraient par ne plus s'accorder.
ECRANS = (
    # — Le tronc commun, avant même le choix du parcours. —
    ("bienvenue", "Welcome to grenOS", PARCOURS),
    ("langue", "Langue et région", PARCOURS),
    ("clavier", "Clavier", PARCOURS),
    ("compte", "Votre compte", PARCOURS),
    ("parcours", "Comment voulez-vous continuer ?", PARCOURS),

    # — Ce que le parcours rapide saute. —
    ("reseau", "Connexion à Internet", (PERSONNALISE, DEV)),
    ("apparence", "Apparence", (PERSONNALISE, DEV)),
    ("usage", "À quoi va servir cet ordinateur ?", (PERSONNALISE, DEV)),
    ("confidentialite", "Vos données vous appartiennent", (PERSONNALISE, DEV)),
    ("securite", "Protégeons votre ordinateur", (PERSONNALISE, DEV)),
    ("majs", "Mises à jour", (PERSONNALISE, DEV)),
    ("applications", "Applications proposées", (PERSONNALISE, DEV)),
    ("sauvegardes", "Sauvegardes", (PERSONNALISE, DEV)),
    ("optimisation", "Optimisation automatique", (PERSONNALISE, DEV)),

    # — Ce que seul le parcours Dev montre. —
    ("partitionnement", "Partitionnement et chiffrement", (DEV,)),
    ("demarrage", "Secure Boot et démarrage", (DEV,)),
    ("systeme", "Swap, ordonnanceur et services", (DEV,)),
    ("pare-feu", "Réseau et pare-feu", (DEV,)),
    ("journaux", "Télémétrie et journaux", (DEV,)),
    ("bac-a-sable", "Bac à sable et paramètres du noyau", (DEV,)),

    # — La fin, pour tout le monde. —
    ("final", "grenOS est prêt", PARCOURS),
)

# Les écrans dont on ne peut pas sortir sans réponse. Tous les autres ont un
# défaut sensé et se passent d'un clic : *un assistant qui bloque sur une
# question de confort est un assistant qu'on subit.*
#
# `compte` en fait partie parce que la machine n'a pas de nom de compte tant
# qu'on n'en a pas donné un ; `parcours` parce que c'est lui qui décide de la
# suite. `bienvenue` non : son seul bouton est « Start ».
OBLIGATOIRES = ("compte", "parcours")

# Ce que vaut chaque réglage quand personne n'a touché à rien.
#
# `telemetrie: aucune` est écrit dans sa demande en toutes lettres — « aucune
# par défaut ». Le reste suit le même principe : le défaut ne doit jamais
# être celui qui en demande le plus à la personne.
DEFAUTS = {
    "langue": "fr_FR",
    "clavier": "fr",
    "parcours": PERSONNALISE,
    "apparence": "auto",
    "usage": "bureautique",
    "telemetrie": "aucune",
    "majs": "automatiques",
    "sauvegardes": False,
    "chiffrement": False,
}


def ecrans(parcours):
    """Les identifiants d'écrans de ce parcours, dans l'ordre.

    Un parcours inconnu rend le parcours recommandé plutôt que rien : se
    tromper de nom ne doit pas donner un assistant vide, c'est-à-dire une
    machine dans laquelle personne n'entre.
    """
    if parcours not in PARCOURS:
        parcours = PERSONNALISE
    return tuple(cle for cle, _, ou in ECRANS if parcours in ou)


def titre(cle):
    for identifiant, intitule, _ in ECRANS:
        if identifiant == cle:
            return intitule
    return ""


def suivant(cle, parcours):
    """L'écran d'après, ou None si c'est le dernier."""
    liste = ecrans(parcours)
    if cle not in liste:
        return None
    rang = liste.index(cle)
    return liste[rang + 1] if rang + 1 < len(liste) else None


def precedent(cle, parcours):
    """L'écran d'avant, ou None si c'est le premier."""
    liste = ecrans(parcours)
    if cle not in liste:
        return None
    rang = liste.index(cle)
    return liste[rang - 1] if rang > 0 else None


def avancement(cle, parcours):
    """Où l'on en est, de 0.0 à 1.0.

    Sert à la barre du haut. Elle compte les écrans de CE parcours : quelqu'un
    en Rapide ne doit pas voir une barre qui stagne parce qu'elle mesure les
    vingt écrans du parcours Dev.
    """
    liste = ecrans(parcours)
    if cle not in liste or len(liste) < 2:
        return 0.0
    return liste.index(cle) / float(len(liste) - 1)


def peut_continuer(cle, reponses):
    """A-t-on de quoi quitter cet écran ?

    Rend (oui, pourquoi pas). La raison est destinée à l'écran : un bouton
    éteint sans explication est la faute qu'on a déjà corrigée dans le mode
    sans échec.
    """
    if cle not in OBLIGATOIRES:
        return True, ""
    valeur = (reponses or {}).get(cle)
    if cle == "compte":
        if not valeur:
            return False, "Il faut un nom pour créer votre compte."
        return True, ""
    if cle == "parcours":
        if valeur not in PARCOURS:
            return False, "Choisissez comment continuer."
        return True, ""
    return True, ""


def reponses_par_defaut():
    """Une copie des défauts, pour que l'appelant puisse la modifier."""
    return dict(DEFAUTS)
