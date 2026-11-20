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
            # La place de l'écran dans le bureau (« 2560x1440+2560+0 ») : pour
            # garder la disposition de plusieurs écrans (21 novembre).
            place = re.search(r" (\d+)x(\d+)\+(\d+)\+(\d+)", ligne)
            courante = {"nom": entete.group(1), "actuelle": "", "taux": "", "modes": [],
                        "principal": " connected primary" in ligne,
                        "x": int(place.group(3)) if place else 0,
                        "y": int(place.group(4)) if place else 0,
                        "allume": bool(place)}
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


# LA RÉFÉRENCE DE L'ÉCRAN ET SES LIMITES (21 novembre). Grenofar : « qu'il
# donne la référence de l'écran, son nombre d'images et sa résolution max,
# donc pas plus, pas possible ». La référence est lue dans l'EDID que l'écran
# envoie (son nom en clair, sa marque) ; les maximums sont ceux que le pilote
# a VALIDÉS (la liste de xrandr), pas une promesse calculée.
MARQUES = {"AOC": "AOC", "GSM": "LG", "SAM": "Samsung", "DEL": "Dell", "ACR": "Acer",
           "AUS": "ASUS", "BNQ": "BenQ", "MSI": "MSI", "GBT": "Gigabyte", "HPN": "HP",
           "HWP": "HP", "LEN": "Lenovo", "PHL": "Philips", "VSC": "ViewSonic",
           "IVM": "iiyama", "SNY": "Sony", "XMI": "Xiaomi", "AOU": "AOC", "CMN": "Innolux",
           "SHP": "Sharp", "BOE": "BOE", "MEI": "Panasonic", "NEC": "NEC", "EIZ": "EIZO",
           "PNP": "Pixio", "KTC": "KTC", "HKC": "HKC", "MAG": "MAG", "SKY": "Skyworth"}


def _edids():
    """L'EDID de chaque sortie branchée, lu dans `xrandr --current --verbose`."""
    try:
        texte = subprocess.run(["xrandr", "--current", "--verbose"], capture_output=True,
                               text=True, timeout=10).stdout
    except (OSError, subprocess.SubprocessError):
        return {}
    edids, nom, hexa, dedans = {}, None, [], False
    for ligne in texte.splitlines():
        entete = re.match(r"^(\S+) (connected|disconnected)", ligne)
        if entete:
            if nom and hexa:
                edids[nom] = bytes.fromhex("".join(hexa))
            nom, hexa, dedans = entete.group(1), [], False
            continue
        if re.match(r"^\s+EDID:\s*$", ligne):
            dedans = True
            continue
        if dedans:
            morceau = ligne.strip()
            if re.fullmatch(r"[0-9a-fA-F]{32}", morceau):
                hexa.append(morceau)
                continue
            dedans = False
    if nom and hexa:
        edids[nom] = bytes.fromhex("".join(hexa))
    return edids


def decrire_edid(edid):
    """{'reference': 'AOC Q27G3XMN', 'vmax': 300} — ou {} si illisible."""
    if len(edid) < 128 or edid[:8] != bytes([0, 255, 255, 255, 255, 255, 255, 0]):
        return {}
    code = (edid[8] << 8) | edid[9]
    sigle = "".join(chr(((code >> d) & 0x1F) + 64) for d in (10, 5, 0))
    nom, vmax = "", 0
    for i in range(54, 126, 18):
        bloc = edid[i:i + 18]
        if bloc[0] or bloc[1]:
            continue
        if bloc[3] == 0xFC:
            nom = bloc[5:18].split(b"\n")[0].decode("ascii", "replace").strip()
        elif bloc[3] == 0xFD:
            # Fréquence verticale maximale ; au-delà de 255 Hz, l'EDID 1.4
            # ajoute 255 (bit 1 de l'octet 4) — un écran 300 Hz s'écrit 45.
            vmax = bloc[6] + (255 if bloc[4] & 0x02 else 0)
    marque = MARQUES.get(sigle, sigle)
    if nom and nom.lower().startswith(marque.lower()):
        reference = nom
    else:
        reference = ("%s %s" % (marque, nom)).strip()
    return {"reference": reference, "vmax": vmax}


def limites(sortie):
    """Ce que le pilote a validé pour cet écran : la plus grande définition et
    la plus haute fréquence (toutes définitions confondues)."""
    plus_grande = max(sortie["modes"], key=lambda m: surface(m["taille"]), default=None)
    taux = [float(t) for m in sortie["modes"] for t in m["taux"]]
    return {"definition": plus_grande["taille"] if plus_grande else "",
            "frequence": int(round(max(taux))) if taux else 0}


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


PROPOSE = os.path.expanduser("~/.config/grenos/ecran-propose")


def meilleure_frequence():
    """La plus haute fréquence ANNONCÉE par l'écran principal à sa
    définition actuelle, si elle dépasse l'actuelle — sinon None.

    Grenofar (20 novembre) : « mon écran est à 300 Hz », et grenOS démarrait
    à 60. Windows propose la fréquence maximale d'un écran neuf ; grenOS la
    PROPOSE aussi, une fois par écran, avec le « Garder ? » de 15 secondes
    des Réglages : si l'écran n'affiche rien à cette fréquence (360 Hz
    coupait son signal), l'ancien réglage revient seul. Seulement ce que le
    pilote liste (validé par lui), jamais une fréquence calculée.
    """
    liste = sorties()
    if not liste:
        return None
    ecran = liste[0]
    if not ecran["actuelle"]:
        return None
    mode = next((m for m in ecran["modes"] if m["taille"] == ecran["actuelle"]), None)
    if not mode or not mode["taux"]:
        return None
    meilleur = max(mode["taux"], key=float)
    try:
        if float(meilleur) <= float(ecran["taux"] or 0) + 5:
            return None
    except ValueError:
        return None
    return {"nom": ecran["nom"], "taille": ecran["actuelle"], "brut": meilleur,
            "arrondi": int(round(float(meilleur)))}


def deja_propose():
    return os.path.exists(PROPOSE)


def noter_proposition(proposition):
    os.makedirs(os.path.dirname(PROPOSE), exist_ok=True)
    with open(PROPOSE, "w", encoding="utf-8") as fichier:
        fichier.write("%s %s %s\n" % (proposition["nom"], proposition["taille"], proposition["brut"]))


TOUS = os.path.expanduser("~/.config/grenos/ecrans")


def noter_tout():
    """Garde l'état de TOUS les écrans allumés : définition, fréquence,
    place dans le bureau, principal (21 novembre).

    Grenofar, avec deux écrans : « fais qu'on puisse choisir l'écran auquel
    changer les paramètres » — et la disposition faite avec « Disposer les
    écrans » se perdait à la session suivante. Une ligne par écran :
    nom définition fréquence x y principal(0/1).
    """
    lignes = []
    for s in sorties():
        if not s["allume"] or not s["actuelle"]:
            continue
        lignes.append("%s %s %s %d %d %d" % (s["nom"], s["actuelle"], s["taux"] or "-",
                                             s["x"], s["y"], 1 if s["principal"] else 0))
    if not lignes:
        return ""
    os.makedirs(os.path.dirname(TOUS), exist_ok=True)
    with open(TOUS + ".tmp", "w", encoding="utf-8") as fichier:
        fichier.write("\n".join(lignes) + "\n")
    os.replace(TOUS + ".tmp", TOUS)
    return ""


def _commande_disposition(lignes, branches, places=True):
    commande = ["xrandr"]
    for ligne in lignes:
        morceaux = ligne.split()
        if len(morceaux) < 6 or morceaux[0] not in branches:
            continue
        nom, mode, taux, x, y, principal = morceaux[:6]
        commande += ["--output", nom, "--mode", mode]
        if taux != "-":
            commande += ["--rate", taux]
        if places:
            commande += ["--pos", "%sx%s" % (x, y)]
        if principal == "1":
            commande += ["--primary"]
    return commande


def restaurer():
    """Réapplique ce qui a été gardé, à l'ouverture de la session.

    D'abord la disposition de tous les écrans (noter_tout) ; un écran
    débranché depuis est simplement sauté. Si le pilote refuse les places
    (écrans changés), on garde au moins définitions et fréquences. Sans
    disposition gardée : l'ancien choix d'un seul écran.
    """
    try:
        with open(TOUS, encoding="utf-8") as fichier:
            lignes = [l.strip() for l in fichier if l.strip()]
    except OSError:
        lignes = []
    if lignes:
        branches = {s["nom"] for s in sorties()}
        for places in (True, False):
            commande = _commande_disposition(lignes, branches, places)
            if len(commande) == 1:
                return ""
            r = subprocess.run(commande, capture_output=True, text=True)
            if r.returncode == 0:
                return ""
        return "La disposition gardée n'a pas pu être remise."
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
