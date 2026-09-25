#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""On n'interroge jamais le matériel pendant qu'on construit une fenêtre.

Trois fois la même faute, et chaque fois découverte par quelqu'un devant
l'écran plutôt que par un contrôle :

  - la page **Jeux** appelait `vulkaninfo` avant de s'afficher : vingt-cinq
    secondes d'écran vide sur une machine sans pilote 3D ;
  - la page **Connexions** appelait `nmcli` et `bluetoothctl` cinq fois dans
    son constructeur, vingt secondes de patience chacun. Grenofar a mesuré
    **deux minutes** avant le premier pixel, le 25 septembre ;
  - et à chaque fois la fenêtre finissait par s'ouvrir, donc rien ne rougissait.

Le remède est toujours le même : montrer la fenêtre, puis la remplir depuis un
fil. Ce contrôle refuse le retour de la maladie — un appel connu pour être lent
dans `__init__` ou dans une méthode qui construit une page.

Il ne juge que ce qu'il sait lent. Une fonction qui le devient devra être
ajoutée ici ; c'est le prix d'un contrôle qui ne crie pas à tort.

    python3 linux/tools/verif-fenetres.py [fichier...]
"""
import ast
import glob
import io
import os
import sys

# Ce qui lance un programme extérieur et peut attendre des secondes.
LENTS = {
    "reseau": {"etat", "wifi_allume", "wifi_disponible", "reseaux", "connu",
               "bluetooth_allume", "bluetooth_disponible", "chercher_appareils",
               "appareils", "etat_barre", "icone_barre"},
    "jeux": {"vulkan", "pilote_propose", "installe", "steam_tourne",
             "preparer_rocket_league", "rocket_league_prete"},
    "son": {"sorties", "sortie_actuelle", "applications_qui_jouent"},
    "maj": {"disponibles", "verifier"},
    "materiel": {"graphique", "resume"},
    "subprocess": {"run", "check_output", "call", "check_call"},
}

# Ce qu'on tolère, nommément, et pourquoi. Trois pages des Réglages lisent leur
# état pendant qu'elles se construisent, mais chacune est BORNÉE : `nmcli` y est
# appelé avec cinq secondes de délai, et les deux autres lisent /proc et pactl,
# qui répondent tout de suite. Les réécrire maintenant serait toucher à des
# fenêtres qui marchent, pendant qu'on répare celles qui ne marchent pas.
#
# Cette liste est une dette, pas une permission : elle est écrite ici pour être
# vue, et tout ce qui n'y figure pas est refusé.
TOLERES = {
    ("Reglages", "page_son", "module_son.sorties()"),
    ("Reglages", "page_son", "module_son.sortie_actuelle()"),
    ("Reglages", "page_reseau", "subprocess.run()"),
    ("Reglages", "page_materiel", "materiel.resume()"),
}

# Les méthodes qui dessinent : tout ce qui s'y passe retarde le premier pixel.
BATISSEURS = ("__init__", "page_", "construire", "remplir_page", "onglet_")


def batisseur(nom):
    return nom == "__init__" or any(nom.startswith(p) for p in BATISSEURS[1:])


def fautes(chemin):
    try:
        with io.open(chemin, encoding="utf-8") as fichier:
            tete = fichier.readline()
            fichier.seek(0)
            contenu = fichier.read()
    except OSError:
        return []
    if "python3" not in tete and not chemin.endswith(".py"):
        return []
    try:
        arbre = ast.parse(contenu, chemin)
    except SyntaxError:
        return []

    trouves = []
    for classe in ast.walk(arbre):
        if not isinstance(classe, ast.ClassDef):
            continue
        for methode in classe.body:
            if not isinstance(methode, ast.FunctionDef) or not batisseur(methode.name):
                continue
            for noeud in ast.walk(methode):
                # Un appel lancé dans un fil ou différé ne bloque pas : on ne
                # regarde que les appels directs.
                if not isinstance(noeud, ast.Call):
                    continue
                cible = noeud.func
                if not isinstance(cible, ast.Attribute) or not isinstance(cible.value, ast.Name):
                    continue
                module = cible.value.id
                if module == "module_son":
                    module = "son"
                if module in LENTS and cible.attr in LENTS[module]:
                    appel = f"{cible.value.id}.{cible.attr}()"
                    if (classe.name, methode.name, appel) in TOLERES:
                        continue
                    trouves.append((noeud.lineno, classe.name, methode.name, appel))
    return trouves


def main():
    chemins = sys.argv[1:]
    if not chemins:
        chemins = sorted(glob.glob("linux/config/includes.chroot/usr/bin/*"))

    total, lus = 0, 0
    for chemin in chemins:
        if not os.path.isfile(chemin):
            continue
        lus += 1
        for ligne, classe, methode, appel in fautes(chemin):
            print(f"{chemin}:{ligne} : {classe}.{methode} appelle {appel} "
                  f"pendant qu'il construit la fenetre")
            total += 1

    if total:
        print(f"\n{total} appel(s) lents pendant la construction d'une fenetre.")
        print("Montre la fenetre, puis remplis-la depuis un fil : c'est ce qui a "
              "coute vingt-cinq secondes a la page Jeux et deux minutes a "
              "Connexions.")
        return 1
    print(f"fenetres : {lus} programmes lus, aucun NOUVEL appel lent pendant "
          f"qu'une fenetre se dessine ({len(TOLERES)} cas connus et bornes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
