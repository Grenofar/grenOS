# -*- coding: utf-8 -*-
"""Ce que le mode sans échec sait de la machine, et ce qu'il peut proposer.

C17 : « désactiver le pilote, restaurer la version précédente, redémarrer en
mode minimal, analyser ».

**Aucun GTK ici, et aucune action.** Ce module ne fait que lire et décider ;
c'est la fenêtre qui agit. C'est ce qui permet de l'exercer sans écran, et
c'est la leçon de `sante.py` : un module qui dessine ne se mesure pas.

LE BOUTON QUI COMPTE EST « DÉSACTIVER LE PILOTE », et il est le seul à pouvoir
casser quelque chose. Il ne doit donc s'allumer que si l'on sait **quel**
module nommer. Proposer « désactiver le pilote » sans savoir lequel serait une
promesse à l'écran sans rien derrière — et pire : on écrirait une liste noire
au hasard sur la machine de quelqu'un.
"""
import os
import re

ETAT = "/var/lib/grenos"

# Les formes sous lesquelles le noyau nomme un module en échec. Chacune vient
# d'un message réel, pas d'une supposition sur ce qu'il pourrait dire.
#
#   Direct firmware load for iwlwifi-cc-a0-72.ucode failed with error -2
#   iwlwifi 0000:00:14.3: probe with driver iwlwifi failed with error -110
#   nouveau 0000:01:00.0: Direct firmware load for nouveau/nv137 failed
#
# On garde la PREMIÈRE partie du nom, avant le tiret ou la barre oblique :
# `iwlwifi-cc-a0-72.ucode` est un micrologiciel, le module s'appelle
# `iwlwifi`. Charger la liste noire avec le nom du fichier n'aurait aucun
# effet, et c'est exactement le genre de correctif qui passe son garde sans
# rien corriger.
MOTIFS = (
    re.compile(r"Direct firmware load for ([A-Za-z0-9_]+)"),
    re.compile(r"probe with driver ([A-Za-z0-9_]+) failed"),
    re.compile(r"^([A-Za-z0-9_]+) [0-9a-f]{4}:[0-9a-f]{2}:"),
)

# Des modules qu'on ne mettra JAMAIS en liste noire, même si le journal les
# nomme : les retirer empêcherait la machine de démarrer ou de voir son
# disque, c'est-à-dire exactement ce que le mode sans échec est censé éviter.
INTOUCHABLES = {
    "ahci", "nvme", "sd_mod", "ext4", "btrfs", "xfs", "usbhid", "hid_generic",
    "xhci_hcd", "ehci_hcd", "atkbd", "i8042", "vfat", "overlay", "squashfs",
}


def lire(nom, defaut=""):
    """Un fichier d'état, ou le défaut. Ne lève jamais."""
    try:
        with open(os.path.join(ETAT, nom), encoding="utf-8") as fichier:
            return fichier.read().strip()
    except (OSError, UnicodeDecodeError):
        return defaut


def echecs(racine=None):
    """Combien de démarrages d'affilée n'ont pas ouvert de bureau."""
    global ETAT
    if racine is not None:
        ancien, ETAT = ETAT, racine
        try:
            brut = lire("demarrages-echoues", "0")
        finally:
            ETAT = ancien
    else:
        brut = lire("demarrages-echoues", "0")
    try:
        return max(0, int(brut))
    except ValueError:
        # Un compteur illisible n'est pas un échec : c'est un compteur
        # illisible. Le compter pour un échec déclencherait le mode sans
        # échec sur une machine qui va très bien.
        return 0


def cause(racine=None):
    """La cause probable du dernier échec, telle que le journal l'a dite."""
    global ETAT
    if racine is not None:
        ancien, ETAT = ETAT, racine
        try:
            return lire("derniere-cause", "")
        finally:
            ETAT = ancien
    return lire("derniere-cause", "")


def module_fautif(texte):
    """Le module que le journal accuse, ou None.

    None veut dire « je ne sais pas », et c'est une réponse. Le bouton qui
    désactive un pilote reste éteint tant qu'on ne sait pas lequel.
    """
    if not texte:
        return None
    for motif in MOTIFS:
        trouve = motif.search(texte)
        if not trouve:
            continue
        nom = trouve.group(1).split("-")[0].split("/")[0]
        if not nom or nom in INTOUCHABLES:
            # On a trouvé un nom, et on refuse de le proposer. Ce n'est pas la
            # même chose que ne rien trouver, mais pour le bouton c'est la
            # même conséquence : il reste éteint.
            return None
        return nom
    return None


def propositions(racine=None):
    """Ce que la fenêtre peut offrir à cette machine-ci, et rien de plus.

    Chaque entrée est (clé, intitulé, possible, pourquoi pas). Une action
    impossible est **montrée éteinte avec sa raison**, jamais cachée : quelqu'un
    qui cherche « restaurer » doit voir pourquoi il ne peut pas, sinon il croit
    que grenOS ne sait pas le faire.
    """
    texte = cause(racine)
    module = module_fautif(texte)
    instantanes = os.path.exists("/etc/timeshift/timeshift.json")
    return [
        ("analyser", "Analyser la machine", True, ""),
        ("journal", "Voir ce que le dernier démarrage a dit", True, ""),
        ("instantane", "Restaurer un instantané", instantanes,
         "" if instantanes else "aucun disque d'instantanés n'est encore choisi"),
        ("pilote", "Désactiver « %s »" % module if module else "Désactiver le pilote fautif",
         module is not None,
         "" if module else "le journal ne nomme aucun pilote, et on n'en invente pas"),
    ]
