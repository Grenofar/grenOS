"""L'écran : quelles définitions la machine accepte, et laquelle on garde.

C'est la réponse à « c'est saccadé, et je ne peux pas changer la résolution ».
Tout passe par RandR, l'interface que le serveur X expose déjà : on lit ce que
le pilote propose (définitions et fréquences réelles), on applique, et on note
le choix pour le réappliquer à l'ouverture suivante.
"""
import os
import re
import subprocess

CHOIX = os.path.expanduser("~/.config/grenos/ecran")


def sorties():
    """Les écrans branchés, leurs définitions et les fréquences de chacune.

    Rendu : [{'nom', 'actuelle', 'taux', 'modes': [{'taille', 'taux': [...]}]}]
    """
    try:
        texte = subprocess.run(["xrandr", "--query"], capture_output=True, text=True,
                               check=True).stdout
    except (subprocess.CalledProcessError, FileNotFoundError):
        return []

    trouvees, courante = [], None
    for ligne in texte.splitlines():
        entete = re.match(r"^(\S+) connected", ligne)
        if entete:
            courante = {"nom": entete.group(1), "actuelle": "", "taux": "", "modes": []}
            trouvees.append(courante)
            continue
        mode = re.match(r"^\s+(\d+x\d+)\s+(.*)$", ligne)
        if mode and courante is not None:
            taux = []
            for brut in mode.group(2).split():
                valeur = re.match(r"^(\d+\.\d+)([*+]*)$", brut)
                if not valeur:
                    continue
                taux.append(valeur.group(1))
                if "*" in valeur.group(2):
                    courante["actuelle"] = mode.group(1)
                    courante["taux"] = valeur.group(1)
            if taux:
                courante["modes"].append({"taille": mode.group(1), "taux": taux})
    return trouvees


def surface(taille):
    """Le nombre de pixels d'une définition, pour trier de la plus grande."""
    largeur, hauteur = taille.split("x")
    return int(largeur) * int(hauteur)


def appliquer(nom, taille, taux=""):
    """Change la définition, et garde le choix pour la prochaine ouverture."""
    commande = ["xrandr", "--output", nom, "--mode", taille]
    if taux:
        commande += ["--rate", taux]
    resultat = subprocess.run(commande, capture_output=True, text=True)
    if resultat.returncode != 0:
        return resultat.stderr.strip() or "Le pilote a refusé cette définition."
    os.makedirs(os.path.dirname(CHOIX), exist_ok=True)
    with open(CHOIX, "w", encoding="utf-8") as fichier:
        fichier.write(f"{nom}\n{taille}\n{taux}\n")
    return ""


def restaurer():
    """Réapplique le choix noté, à l'ouverture de la session."""
    try:
        with open(CHOIX, encoding="utf-8") as fichier:
            lignes = [l.strip() for l in fichier.readlines()]
    except OSError:
        return ""
    if len(lignes) < 2 or not lignes[0] or not lignes[1]:
        return ""
    return appliquer(lignes[0], lignes[1], lignes[2] if len(lignes) > 2 else "")
