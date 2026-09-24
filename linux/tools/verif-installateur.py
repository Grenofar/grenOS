#!/usr/bin/env python3
"""L'installateur a-t-il tout ce qu'il lui faut pour démarrer ?

Calamares lit `settings.conf`, y trouve une suite de modules, et va chercher
chacun d'eux sur le disque. Si l'un manque, il s'arrête au lancement avec un
message que personne ne comprend — et on ne l'apprendrait que devant l'écran,
au moment d'installer grenOS sur une vraie machine.

Personne ne l'avait jamais vérifié. On ne peut pas lancer Calamares en
intégration — cela demanderait un écran, un disque et une suite de clics —
mais on peut lire sa configuration et demander au disque si chaque morceau
qu'elle nomme existe. C'est moins qu'une preuve, et c'est beaucoup mieux que
rien.

Ce qu'on vérifie, et pourquoi :
  - chaque module de la suite a bien son `module.desc` ;
  - l'habillage désigné existe (sinon Calamares refuse de démarrer) ;
  - le partitionnement propose tout le disque et laisse le choix manuel,
    ce que Grenofar a demandé en toutes lettres.

Ce contrôle empêche une publication, donc il ne doit pas se tromper. Vérifié
avant de le brancher, en ouvrant les deux paquets : les 30 modules distincts
de la suite sont tous fournis — 24 par `calamares` 3.3.14-1, 6 par
`calamares-settings-debian` 13.0.13-1. Aucun faux positif possible sur une
installation correcte.

    python3 linux/tools/verif-installateur.py [racine]

`racine` sert aux essais : par défaut, c'est le système lui-même.
"""
import os
import re
import sys


def modules_de_la_suite(texte):
    """Les noms de modules que `settings.conf` enchaîne, dans l'ordre.

    On lit les lignes plutôt que le YAML : le format est simple et stable, et
    le conteneur d'essai n'a pas forcément de bibliothèque YAML. Une
    dépendance de plus serait une raison de plus d'échouer pour rien.
    """
    noms, dans_la_suite = [], False
    for ligne in texte.splitlines():
        nu = ligne.strip()
        if not nu or nu.startswith("#"):
            continue
        if re.match(r"^sequence:\s*$", ligne):
            dans_la_suite = True
            continue
        if dans_la_suite:
            # Une clé sans indentation ferme la suite.
            if re.match(r"^\S", ligne) and not ligne.startswith("-"):
                break
            trouve = re.match(r"^\s+-\s+([A-Za-z0-9._-]+)\s*$", ligne)
            if trouve:
                noms.append(trouve.group(1))
    return noms


def verifier(racine=""):
    """Les reproches à faire à cette installation de Calamares."""
    fautes = []

    def chemin(*morceaux):
        return os.path.join(racine, *morceaux) if racine else os.path.join("/", *morceaux)

    reglages = chemin("etc", "calamares", "settings.conf")
    if not os.path.exists(reglages):
        return ["settings.conf est absent : l'installateur ne demarrerait pas"]

    with open(reglages, encoding="utf-8") as fichier:
        texte = fichier.read()

    modules = modules_de_la_suite(texte)
    if not modules:
        fautes.append("aucun module dans la suite : settings.conf est illisible")
    manquants = [nom for nom in modules
                 if not os.path.exists(chemin("usr", "lib", "calamares", "modules",
                                              nom, "module.desc"))]
    print(f"modules de la suite : {len(modules) - len(manquants)}/{len(modules)} presents")
    if manquants:
        fautes.append("ces modules manquent, Calamares s'arreterait au lancement : "
                      + ", ".join(sorted(set(manquants))))

    habillage = re.search(r"^branding:\s*(\S+)\s*$", texte, re.M)
    if not habillage:
        fautes.append("settings.conf ne designe aucun habillage")
    else:
        nom = habillage.group(1)
        desc = chemin("usr", "share", "calamares", "branding", nom, "branding.desc")
        print(f"habillage : {nom}")
        if not os.path.exists(desc):
            fautes.append(f"l'habillage « {nom} » n'existe pas : {desc}")
        elif nom != "grenos":
            fautes.append(f"l'habillage est « {nom} » et non « grenos »")

    partition = chemin("etc", "calamares", "modules", "partition.conf")
    if not os.path.exists(partition):
        fautes.append("partition.conf est absent : le disque entier ne serait pas propose")
    else:
        with open(partition, encoding="utf-8") as fichier:
            reglage = fichier.read()
        promesses = 0
        for cle, pourquoi in (("initialPartitioningChoice: erase", "tout le disque par defaut"),
                              ("allowManualPartitioning: true", "le choix manuel")):
            if re.search("^" + re.escape(cle) + r"\s*$", reglage, re.M):
                promesses += 1
            else:
                fautes.append(f"partition.conf ne promet pas {pourquoi} ({cle})")
        if promesses == 2:
            print("partitionnement : tout le disque par defaut, choix manuel possible")

    return fautes


def main(argv):
    racine = argv[1] if len(argv) > 1 else ""
    fautes = verifier(racine)
    for faute in fautes:
        print("installateur : " + faute, file=sys.stderr)
    if fautes:
        return 1
    print("l'installateur a tout ce qu'il lui faut pour demarrer")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
