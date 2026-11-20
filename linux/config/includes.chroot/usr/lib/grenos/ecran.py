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
    # `--current`, pas `--query` (20 novembre). `--query` fait RÉINTERROGER
    # chaque écran branché par le serveur X (lecture de l'EDID), et le serveur
    # ne répond à rien d'autre pendant ce temps : sur le PC de Grenofar (RTX
    # 5060, deux écrans), ouvrir les Réglages laissait la souris sans clic.
    # `--current` rend la même liste, telle que X la connaît déjà — un écran
    # branché ensuite y arrive de lui-même par l'événement de branchement.
    # Et une borne : un serveur qui ne répond plus ne fige pas la page.
    try:
        texte = subprocess.run(["xrandr", "--current"], capture_output=True, text=True,
                               check=True, timeout=10).stdout
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return []

    trouvees, courante = [], None
    for ligne in texte.splitlines():
        entete = re.match(r"^(\S+) connected", ligne)
        if entete:
            courante = {"nom": entete.group(1), "actuelle": "", "taux": "", "modes": [],
                        "principal": " connected primary" in ligne}
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
    # L'écran PRINCIPAL d'abord (20 novembre) : les Réglages ne règlent que le
    # premier de la liste, et avec deux écrans c'était l'ordre des prises —
    # Grenofar réglait 300 Hz sans savoir sur lequel.
    trouvees.sort(key=lambda s: not s["principal"])
    return trouvees


def surface(taille):
    """Le nombre de pixels d'une définition, pour trier de la plus grande."""
    largeur, hauteur = taille.split("x")
    return int(largeur) * int(hauteur)


def noter(nom, mode, taux=""):
    """Garde ce choix pour la prochaine ouverture de session."""
    os.makedirs(os.path.dirname(CHOIX), exist_ok=True)
    with open(CHOIX, "w", encoding="utf-8") as fichier:
        fichier.write(f"{nom}\n{mode}\n{taux}\n")


def appliquer(nom, taille, taux="", garder=True):
    """Change la définition ; `garder` la note pour la prochaine ouverture.

    Les Réglages passent `garder=False` (20 novembre) : sur le PC de Grenofar,
    360 Hz a coupé le signal de l'écran, ET le choix était déjà noté — il
    serait revenu à chaque ouverture de session. On n'écrit plus qu'après
    « Garder » (noter), comme Windows.
    """
    commande = ["xrandr", "--output", nom, "--mode", taille]
    if taux:
        commande += ["--rate", taux]
    resultat = subprocess.run(commande, capture_output=True, text=True)
    if resultat.returncode != 0:
        return resultat.stderr.strip() or "Le pilote a refusé cette définition."
    if garder:
        noter(nom, taille, taux)
    return ""


def ajouter_mode(nom, largeur, hauteur, taux, garder=True):
    """Comme `essayer_mode`, sans rendre l'étiquette (l'ancienne interface)."""
    return essayer_mode(nom, largeur, hauteur, taux, garder)[0]


def essayer_mode(nom, largeur, hauteur, taux, garder=True):
    """Crée une définition que le pilote ne proposait pas, et l'applique.

    C'est la réponse à « je veux dépasser 60 images par seconde ». Un écran
    peut accepter davantage sans que le pilote ne l'annonce — c'est fréquent
    dans une machine virtuelle, où la liste est courte et figée. On calcule
    alors une définition aux normes (`cvt` fait ce calcul depuis toujours), on
    l'ajoute à la sortie, et on l'essaie.

    Rendu : (phrase expliquant le refus ou "", étiquette du mode créé).
    """
    try:
        calcul = subprocess.run(["cvt", str(largeur), str(hauteur), str(int(taux))],
                                capture_output=True, text=True, check=True).stdout
    except (subprocess.CalledProcessError, FileNotFoundError):
        return "L'outil de calcul des définitions (cvt) est absent.", ""

    ligne = ""
    for brut in calcul.splitlines():
        if brut.strip().startswith("Modeline"):
            ligne = brut.strip()[len("Modeline"):].strip()
    if not ligne:
        return "Cette définition n'a pas pu être calculée.", ""

    morceaux = ligne.replace('"', " ").split()
    if not morceaux:
        return "Cette définition n'a pas pu être lue.", ""
    etiquette = morceaux[0]

    # `--newmode` échoue si la définition existe déjà : ce n'est pas une erreur.
    subprocess.run(["xrandr", "--newmode", etiquette, *morceaux[1:]],
                   capture_output=True, text=True)
    ajout = subprocess.run(["xrandr", "--addmode", nom, etiquette],
                           capture_output=True, text=True)
    if ajout.returncode != 0:
        return (ajout.stderr.strip()
                or "L'écran a refusé cette définition. Son pilote ne la permet pas."), ""

    essai = subprocess.run(["xrandr", "--output", nom, "--mode", etiquette],
                           capture_output=True, text=True)
    if essai.returncode != 0:
        return (essai.stderr.strip()
                or "L'écran a refusé de passer à cette définition."), ""

    if garder:
        noter(nom, etiquette)
    return "", etiquette


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


# Les fréquences qu'on propose en plus de celles que le pilote annonce.
#
# Ce sont celles des écrans qu'on trouve aujourd'hui, et celles que Grenofar a
# nommées : « 120 240 fps etc ».
FREQUENCES_USUELLES = (60, 75, 90, 100, 120, 144, 165, 180, 200, 240, 360)


def frequences_proposees(annonces, usuelles=FREQUENCES_USUELLES):
    """Ce que le menu doit offrir pour une définition donnée.

    Rendu : [(arrondi, valeur_brute, annoncée_par_le_pilote)], croissant.

    POURQUOI CETTE FONCTION EXISTE AILLEURS QUE DANS LA FENÊTRE

    Elle y était, et elle n'était donc exerçable que par une copie — or deux
    écritures d'une même chose finissent par diverger, et c'est alors la copie
    qu'on croit. C'est le piège payé sur les icônes, où la planche recopiait le
    calcul de la construction.

    Chaque entrée garde **son origine**. Une fréquence annoncée s'applique par
    `appliquer()` ; une fréquence proposée demande d'abord `ajouter_mode()`.
    Sans ce drapeau il faudrait relire le texte affiché pour décider — et une
    phrase qu'on relit pour décider d'une action est un drapeau déguisé.
    """
    trouvees, deja = [], set()
    for brut in annonces:
        try:
            valeur = float(brut)
        except (TypeError, ValueError):
            continue
        arrondi = round(valeur)
        if arrondi in deja:
            # 59.94 et 60.00 arrondissent au même nombre : les montrer toutes
            # les deux donnerait deux lignes « 60 Hz » dans le menu, et personne
            # ne saurait laquelle choisir.
            continue
        deja.add(arrondi)
        trouvees.append((arrondi, brut, True))
    # PAS AU-DESSUS DE CE QUE L'ÉCRAN ANNONCE (20 novembre). Grenofar a
    # choisi « 360 Hz — à essayer » sur un écran de 300 Hz : plus de signal,
    # « comme si j'avais débranché le HDMI ». Un écran qui annonce 300 au plus
    # ne fera pas 360. On ne propose donc, en plus, que des fréquences
    # INFÉRIEURES au maximum annoncé — celles qu'un pilote oublie parfois.
    plafond = max(deja) if deja else None
    for usuelle in usuelles:
        if usuelle not in deja and (plafond is None or usuelle < plafond):
            trouvees.append((usuelle, str(usuelle), False))
    trouvees.sort(key=lambda entree: entree[0])
    return trouvees
