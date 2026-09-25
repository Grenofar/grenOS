#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Chaque commande que nos programmes lancent existe-t-elle dans l'image ?

Le 25 septembre, `ca-certificates` manquait : `flatpak` ne faisait que le
recommander, et on construit sans les recommandés. Aucune connexion TLS
vérifiée ne marchait — ni Flathub, ni notre propre dépôt de mise à jour.
Grenofar l'a trouvé en essayant d'installer une application.

En cherchant d'autres cas, deux sont apparus aussitôt :

  - `libgtk-3-bin` absent, donc pas de `gtk-update-icon-cache` ni de
    `gtk-launch`. `appliquer-systeme` rafraîchit le cache d'icônes après chaque
    mise à jour — en vain, et en silence (`|| true`). Le bouton « Ouvrir
    Fortnite » ne faisait rien non plus ;
  - `desktop-file-utils` absent, donc pas de `update-desktop-database` : la
    base du menu n'était jamais reconstruite après une mise à jour, et une
    entrée neuve pouvait ne pas apparaître.

Trois fois la même faute, et toujours la même forme : un programme appelle une
commande, l'échec est avalé, et personne ne le sait avant d'appuyer sur le
bouton. Ce contrôle tourne **dans l'image**, où `which` répond pour de vrai.

    python3 verif-commandes.py [dossier...]

Sans argument, il lit /usr/bin/grenos-*, /usr/bin/grenplace et
/usr/lib/grenos/*.py — les fichiers installés.
"""
import ast
import glob
import io
import os
import re
import shutil
import sys

# Ce qui est dans tout système POSIX, ou fourni par le shell : inutile de le
# vérifier, et le vérifier ferait du bruit.
DE_BASE = {
    "sh", "bash", "echo", "printf", "cat", "grep", "sed", "awk", "true", "false",
    "test", "cd", "mkdir", "rm", "cp", "mv", "chmod", "chown", "ls", "head",
    "tail", "sort", "uniq", "wc", "tr", "cut", "dirname", "basename", "date",
    "sleep", "kill", "touch", "install", "ln", "od", "exec", "command", "which",
    "getent", "exit", "elif", "then", "else", "fi", "do", "done", "local",
    "export", "set", "read", "return", "case", "esac", "for", "while", "if",
}

# Ce qui a le droit de manquer, parce que le code le vérifie avant d'appeler et
# se comporte correctement sans. Chaque ligne dit pourquoi.
FACULTATIVES = {
    # Les agents invités : absents sur une vraie machine, et `grenos-session`
    # les lance derrière `command -v`.
    "spice-vdagent", "vmware-user-suid-wrapper", "qemu-ga",
    # Le pilote NVIDIA : la page Jeux s'en sert pour conseiller, pas pour agir.
    "nvidia-detect",
    # Ce que la personne installe elle-même depuis le magasin.
    "steam", "chromium", "chromium-browser", "google-chrome",
    # Nos propres programmes : ils sont dans le même paquet, et le paquet ne
    # peut pas dépendre de lui-même.
    "grenos-dire", "grenos-fond", "grenos-maj", "grenos-menu", "grenos-shell",
    "grenos-theme", "grenos-veilleur", "grenos-parametres", "grenos-connexions",
    "grenos-bureau", "grenos-taches", "grenos-son", "grenos-jeux", "grenplace",
    "grenos-bienvenue", "grenos-arret", "grenos-compte",
    # Des mots qui ressemblent à des commandes dans nos scripts shell.
    "dire", "poser", "debian-installer-launcher.desktop", "xfce4-terminal-settings",
}

NOM = re.compile(r"^[a-z][a-z0-9._+-]*$")


def commandes_de(chemin):
    """Les commandes extérieures qu'un fichier lance."""
    try:
        contenu = io.open(chemin, encoding="utf-8", errors="replace").read()
    except OSError:
        return set()
    tete = contenu.split("\n", 1)[0]
    trouvees = set()

    def noter(nom):
        if nom and NOM.match(nom):
            trouvees.add(nom)

    if "python3" in tete or chemin.endswith(".py"):
        try:
            arbre = ast.parse(contenu)
        except SyntaxError:
            return set()
        for noeud in ast.walk(arbre):
            if not isinstance(noeud, ast.Call):
                continue
            cible = noeud.func
            if not isinstance(cible, ast.Attribute):
                continue
            if cible.attr in ("run", "Popen", "check_output", "call", "check_call"):
                if noeud.args and isinstance(noeud.args[0], ast.List) and noeud.args[0].elts:
                    premier = noeud.args[0].elts[0]
                    if isinstance(premier, ast.Constant) and isinstance(premier.value, str):
                        noter(premier.value)
            elif cible.attr == "lancer" and noeud.args:
                premier = noeud.args[0]
                if isinstance(premier, ast.Constant) and isinstance(premier.value, str):
                    noter(premier.value.split()[0] if premier.value.split() else "")
    else:
        for ligne in contenu.split("\n"):
            nue = ligne.strip()
            if not nue or nue.startswith("#"):
                continue
            depart = re.match(r"(?:command -v |exec )?([a-z][a-z0-9._+-]{2,})\s", nue)
            if depart:
                noter(depart.group(1))
    return trouvees


def main():
    dossiers = sys.argv[1:]
    if not dossiers:
        chemins = (sorted(glob.glob("/usr/bin/grenos-*"))
                   + ["/usr/bin/grenplace"]
                   + sorted(glob.glob("/usr/lib/grenos/*.py"))
                   + sorted(glob.glob("/usr/lib/grenos/grenos-*")))
    else:
        chemins = []
        for dossier in dossiers:
            chemins += ([dossier] if os.path.isfile(dossier)
                        else sorted(glob.glob(os.path.join(dossier, "*"))))

    toutes = {}
    lus = 0
    for chemin in chemins:
        if not os.path.isfile(chemin):
            continue
        lus += 1
        for commande in commandes_de(chemin):
            toutes.setdefault(commande, set()).add(os.path.basename(chemin))

    a_verifier = {c: q for c, q in toutes.items()
                  if c not in DE_BASE and c not in FACULTATIVES}
    manquantes = {c: q for c, q in a_verifier.items() if shutil.which(c) is None}

    print("commandes : %d fichiers lus, %d commandes exterieures, %d verifiees"
          % (lus, len(toutes), len(a_verifier)))
    if manquantes:
        for commande in sorted(manquantes):
            qui = ", ".join(sorted(manquantes[commande]))
            print("grenos: « %s » n'est pas dans l'image, et %s l'appelle"
                  % (commande, qui), file=sys.stderr)
        print("grenos: un appel qui echoue en silence ne se voit qu'en appuyant "
              "sur le bouton. Ajoute le paquet, ou mets la commande dans "
              "FACULTATIVES en disant pourquoi.", file=sys.stderr)
        return 1
    print("grenos: chaque commande appelee existe dans l'image")
    return 0


if __name__ == "__main__":
    sys.exit(main())
