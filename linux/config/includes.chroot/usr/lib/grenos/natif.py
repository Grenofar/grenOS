"""Passer la main à la version Rust d'un programme, s'il y en a une qui tient.

La barre et le bureau existent en deux versions : Rust natif (léger, ce qui
tourne toute la session doit l'être) et Python (celle d'avant, éprouvée par
des mois de CI). Le programme Python reste le point d'entrée — tout ce qui le
lance ou le vise (`grenos-veilleur`, `pkill -f grenos-shell`) n'a pas changé
d'une ligne — et il se remplace par la version Rust dès sa première ligne.

Le repli : si la version Rust tombe trois fois en deux minutes, on n'insiste
pas. Python reprend, et on le DIT sur le port série. Un bureau sans barre est
un bureau perdu ; une barre Python vaut infiniment mieux que pas de barre.

Ce module n'importe PAS GTK : il tourne avant, et son coût doit être nul
quand on passe la main.
"""
import os
import sys
import time

FENETRE = 120   # secondes : trois chutes dans cet intervalle, et on renonce
CHUTES = 3


def _dire(texte):
    try:
        os.spawnlp(os.P_NOWAIT, "grenos-dire", "grenos-dire", texte)
    except OSError:
        pass


def passer_la_main(nom):
    """Remplace ce processus par `/usr/lib/grenos/<nom>-natif`, si possible.

    Ne revient que si l'on doit rester en Python : `--python` demandé,
    binaire absent, ou trop de chutes récentes.
    """
    if "--python" in sys.argv:
        sys.argv.remove("--python")
        return
    binaire = f"/usr/lib/grenos/{nom}-natif"
    if not os.access(binaire, os.X_OK):
        return

    dossier = os.environ.get("XDG_RUNTIME_DIR") or "/tmp"
    carnet = os.path.join(dossier, f"grenos-natif-{nom}")
    maintenant = time.time()
    try:
        with open(carnet, encoding="utf-8") as fichier:
            departs = [float(l) for l in fichier.read().split() if l.strip()]
    except (OSError, ValueError):
        departs = []
    # Chaque départ récent est une chute : un programme qui tient ne repart
    # pas, `grenos-veilleur` ne le relance que s'il s'est arrêté.
    departs = [d for d in departs if maintenant - d < FENETRE]
    if len(departs) >= CHUTES:
        _dire(f"{nom} : la version native est tombee {len(departs)} fois, retour a Python")
        return
    departs.append(maintenant)
    try:
        with open(carnet, "w", encoding="utf-8") as fichier:
            fichier.write("\n".join(f"{d:.0f}" for d in departs))
    except OSError:
        pass
    try:
        os.execv(binaire, [binaire] + sys.argv[1:])
    except OSError as souci:
        _dire(f"{nom} : version native impossible a lancer ({souci}), Python garde la main")
