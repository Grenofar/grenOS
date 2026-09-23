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


def ajouter_mode(nom, largeur, hauteur, taux):
    """Crée une définition que le pilote ne proposait pas, et l'applique.

    C'est la réponse à « je veux dépasser 60 images par seconde ». Un écran
    peut accepter davantage sans que le pilote ne l'annonce — c'est fréquent
    dans une machine virtuelle, où la liste est courte et figée. On calcule
    alors une définition aux normes (`cvt` fait ce calcul depuis toujours), on
    l'ajoute à la sortie, et on l'essaie.

    Rendu : une phrase expliquant le refus, ou une chaîne vide si c'est fait.
    """
    try:
        calcul = subprocess.run(["cvt", str(largeur), str(hauteur), str(int(taux))],
                                capture_output=True, text=True, check=True).stdout
    except (subprocess.CalledProcessError, FileNotFoundError):
        return "L'outil de calcul des définitions (cvt) est absent."

    ligne = ""
    for brut in calcul.splitlines():
        if brut.strip().startswith("Modeline"):
            ligne = brut.strip()[len("Modeline"):].strip()
    if not ligne:
        return "Cette définition n'a pas pu être calculée."

    morceaux = ligne.replace('"', " ").split()
    if not morceaux:
        return "Cette définition n'a pas pu être lue."
    etiquette = morceaux[0]

    # `--newmode` échoue si la définition existe déjà : ce n'est pas une erreur.
    subprocess.run(["xrandr", "--newmode", etiquette, *morceaux[1:]],
                   capture_output=True, text=True)
    ajout = subprocess.run(["xrandr", "--addmode", nom, etiquette],
                           capture_output=True, text=True)
    if ajout.returncode != 0:
        return (ajout.stderr.strip()
                or "L'écran a refusé cette définition. Son pilote ne la permet pas.")

    essai = subprocess.run(["xrandr", "--output", nom, "--mode", etiquette],
                           capture_output=True, text=True)
    if essai.returncode != 0:
        return (essai.stderr.strip()
                or "L'écran a refusé de passer à cette définition.")

    os.makedirs(os.path.dirname(CHOIX), exist_ok=True)
    with open(CHOIX, "w", encoding="utf-8") as fichier:
        fichier.write(f"{nom}\n{etiquette}\n\n")
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
