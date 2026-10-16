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

ET LA SEULE LECTURE DU SYSTÈME QU'ON S'AUTORISE ICI est celle de l'état de
Secure Boot, et elle prend sa racine en paramètre — c'est ce qui permet de
l'exercer sur une variable fabriquée, sur une machine qui n'a pas d'UEFI, au
lieu de dépendre du matériel de qui lance l'essai.
"""
import os

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


# --------------------------------------------------------------------------
# I7 — « À QUOI VA SERVIR CET ORDINATEUR ? »
#
# Sa demande, mot pour mot : « s'il choisit *développement*, un bouton installe
# VS Code, Git, un terminal amélioré, les outils de compilation et GrenAI ».
#
# CE QUI EST TENU, ET CE QUI NE L'EST PAS, parce que la différence est tout :
#
# * les applications sont **installées pour de vrai**, par la file de travaux
#   d'avant-bureau (`grenos-au-prochain-demarrage`), qui est prouvée depuis le
#   27 septembre : elle rend toujours 0, se désarme, et survit à un dépôt
#   injoignable. Un choix d'usage n'est donc pas une préférence rangée dans un
#   fichier, c'est un travail qui s'exécute ;
# * **GrenAI n'est pas proposée**, parce qu'elle n'existe pas : zéro ligne
#   écrite sur onze demandes. La nommer ici serait la sixième promesse à
#   l'écran sans rien derrière, et celle-là se verrait au premier démarrage ;
# * **Ctrl+Alt+Échap n'est pas annoncé** non plus — le mode jeu qui ne laisse
#   tourner que la sécurité n'est pas écrit.
#
# CHAQUE APPLICATION EST DÉSIGNÉE PAR SON NOM COURT DANS LE CATALOGUE, jamais
# par un nom de paquet écrit ici. Le catalogue sait déjà si une fiche
# s'installe par apt ou par Flatpak, et son adresse est vérifiée à la source à
# chaque construction. Deux listes de paquets finiraient par diverger, et
# c'est alors celle qui ne sert à rien qu'on croirait.
USAGES = (
    ("bureautique", "Bureautique et Internet",
     "Écrire, naviguer, regarder des vidéos. Rien de plus n'est installé.",
     ()),
    ("developpement", "Développement",
     "Visual Studio Code, Git, Python et Node. GrenAI n'est pas encore "
     "écrite, donc elle n'est pas proposée.",
     ("vscode", "git", "python", "nodejs")),
    ("jeu", "Jeux",
     "Steam, et Heroic pour les jeux Epic. GameMode est déjà là et donne la "
     "priorité au jeu qui tourne.",
     ("steam", "heroic")),
    ("montage", "Montage photo, vidéo et son",
     "Kdenlive, GIMP et Audacity.",
     ("kdenlive", "gimp", "audacity")),
)


def paquets_pour(usage):
    """Les noms courts de catalogue qu'un usage entraîne, ou rien.

    Un usage inconnu ne rend rien plutôt que le premier de la liste : installer
    quatre applications parce qu'un nom a été mal écrit serait pire que de
    n'en installer aucune.
    """
    for cle, _titre, _note, paquets in USAGES:
        if cle == usage:
            return paquets
    return ()


# --------------------------------------------------------------------------
# I9 — « PROTÉGEONS VOTRE ORDINATEUR »
#
# Et le garde-fou qu'il nomme lui-même : « il ne faut pas que ça se retourne
# contre nous ». Donc cet écran **n'offre aucun interrupteur** : il dit l'état,
# et rien de plus.
#
# Le chiffrement du disque ne se décide pas ici, et ce n'est pas un oubli :
# cette machine tourne sur une image vive. Chiffrer un disque se fait au moment
# où on l'installe, par l'installateur, et nulle part ailleurs. Offrir la case
# ici reviendrait à promettre un chiffrement qui n'aurait rien à chiffrer.
def secure_boot(racine="/sys/firmware/efi/efivars"):
    """« actif », « inactif », ou « pas en UEFI » — lu dans la variable.

    La variable `SecureBoot` fait cinq octets : quatre d'attributs, puis un
    seul qui vaut 1 quand Secure Boot est actif. On la lit directement plutôt
    que d'appeler `mokutil`, qui n'est pas dans l'image — un programme absent
    rendrait « inconnu » à tous les coups, c'est-à-dire une mesure morte.
    """
    if not os.path.isdir(racine):
        return "pas en UEFI"
    for nom in sorted(os.listdir(racine)):
        if not nom.startswith("SecureBoot-"):
            continue
        try:
            with open(os.path.join(racine, nom), "rb") as fichier:
                octets = fichier.read(5)
        except OSError:
            return "inconnu"
        if len(octets) < 5:
            return "inconnu"
        return "actif" if octets[4] else "inactif"
    return "inconnu"


def mot_sur_secure_boot(etat):
    """Ce qu'on en dit à quelqu'un qui installe grenOS pour la première fois.

    AUCUNE DE CES PHRASES NE PROMET SECURE BOOT. grenOS livre
    `grub-efi-amd64-bin` sans `shim-signed` ni `grub-efi-amd64-signed` : un
    micrologiciel qui vérifie les signatures refuse donc de démarrer cette
    image. C'est la case D8 de la liste, et elle est à faire. Le dire en face
    vaut mieux que le laisser découvrir devant un écran noir.
    """
    if etat == "actif":
        return ("Secure Boot est actif. grenOS n'est pas encore signé : "
                "il faut le désactiver dans le BIOS pour démarrer cette "
                "machine sur grenOS.")
    if etat == "inactif":
        return ("Secure Boot est désactivé, ce qu'il faut aujourd'hui pour "
                "démarrer grenOS. Le noyau signé est en chantier.")
    if etat == "pas en UEFI":
        return ("Cette machine a démarré sans UEFI, donc Secure Boot ne "
                "s'applique pas.")
    return ("L'état de Secure Boot n'a pas pu être lu sur cette machine.")


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
