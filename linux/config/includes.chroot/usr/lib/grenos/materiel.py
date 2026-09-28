"""Ce que la machine a sous le capot, et ce que grenOS en fait.

Il n'y a rien à « débloquer » : Linux utilise d'office tous les cœurs et toute
la mémoire qu'on lui donne. La question utile n'est donc pas « comment tout
prendre », c'est « qu'est-ce que la machine voit vraiment ? » — surtout dans
une machine virtuelle, où c'est l'hôte qui décide, et où un écran de réglages
honnête vaut mieux qu'une promesse.
"""
import glob
import os
import re
import subprocess


def coeurs():
    """Le nombre de cœurs utilisables, et le modèle du processeur."""
    nombre = os.cpu_count() or 1
    modele = ""
    try:
        with open("/proc/cpuinfo", encoding="utf-8") as fichier:
            for ligne in fichier:
                if ligne.startswith("model name"):
                    modele = ligne.split(":", 1)[1].strip()
                    break
    except OSError:
        pass
    return nombre, modele


def memoire():
    """La mémoire vive totale et celle qui reste, en gigaoctets."""
    valeurs = {}
    try:
        with open("/proc/meminfo", encoding="utf-8") as fichier:
            for ligne in fichier:
                nom, _, reste = ligne.partition(":")
                nombre = re.search(r"(\d+)", reste)
                if nombre:
                    valeurs[nom] = int(nombre.group(1)) / 1024 / 1024
    except OSError:
        return 0.0, 0.0
    return valeurs.get("MemTotal", 0.0), valeurs.get("MemAvailable", 0.0)


def graphique():
    """La carte graphique et le pilote qui la fait marcher."""
    carte, pilote = "", ""
    try:
        sortie = subprocess.run(["lspci", "-nnk"], capture_output=True, text=True,
                                timeout=5).stdout
        bloc = ""
        for ligne in sortie.splitlines():
            if re.search(r"VGA compatible controller|3D controller|Display controller", ligne):
                bloc = ligne
                carte = ligne.split(":", 2)[-1].strip()
            elif bloc and "Kernel driver in use" in ligne:
                pilote = ligne.split(":", 1)[1].strip()
                break
    except (OSError, subprocess.SubprocessError):
        pass
    if not pilote:
        try:
            rendu = subprocess.run(["glxinfo", "-B"], capture_output=True, text=True,
                                   timeout=5).stdout
            trouve = re.search(r"OpenGL renderer string: (.*)", rendu)
            if trouve:
                pilote = trouve.group(1).strip()
        except (OSError, subprocess.SubprocessError):
            pass
    return _carte_lisible(carte), pilote or "pilote générique"


# Ce que `lspci` rend quand sa base ne connaît pas la puce : le mot « Device »
# et deux nombres hexadécimaux. Sur la capture du 28 septembre, la page
# Matériel affichait « Device [1234:1111] (rev 02) » en face d'une étiquette
# française — un mot anglais et deux nombres qui ne disent rien à personne.
#
# Ces deux-là sont les cartes des machines virtuelles, et ce sont précisément
# celles que pci.ids ne nomme pas. Les nommer nous-mêmes rend la ligne utile :
# « carte virtuelle » explique aussi pourquoi il n'y a pas de 3D.
CARTES_CONNUES = {
    "1234:1111": "Carte graphique virtuelle (QEMU/Bochs)",
    "1af4:1050": "Carte graphique virtuelle (VirtIO)",
    "80ee:beef": "Carte graphique virtuelle (VirtualBox)",
    "15ad:0405": "Carte graphique virtuelle (VMware)",
}


def _carte_lisible(carte):
    """Le nom de la carte, en français, même quand lspci ne la connaît pas."""
    if not carte:
        return "carte graphique inconnue"
    identifiant = re.search(r"\[([0-9a-f]{4}:[0-9a-f]{4})\]", carte)
    if identifiant and identifiant.group(1) in CARTES_CONNUES:
        return CARTES_CONNUES[identifiant.group(1)]
    if carte.startswith("Device "):
        # Inconnue de pci.ids, et pas des nôtres : on dit au moins ce qu'on
        # sait, plutôt que de recopier un mot anglais et deux nombres.
        return "Carte non reconnue " + (identifiant.group(0) if identifiant else "")
    return carte


def disque(chemin="/"):
    """La place totale et la place libre, en gigaoctets."""
    try:
        stat = os.statvfs(chemin)
    except OSError:
        return 0.0, 0.0
    total = stat.f_blocks * stat.f_frsize / 1024 ** 3
    libre = stat.f_bavail * stat.f_frsize / 1024 ** 3
    return total, libre


def disques():
    """Tous les disques de la machine, pas seulement celui qui porte la racine.

    `disque()` ci-dessus lit la place d'un système de fichiers monté : sur une
    machine à deux disques, elle en décrit un et ignore l'autre. La page
    Matériel affichait donc « Disque : 500 Go » à quelqu'un qui en a deux, et
    l'installateur, lui, en proposait deux — de quoi douter de l'un ou de
    l'autre.

    On lit `/sys/block`, que le noyau tient à jour : pas de programme à lancer,
    donc rien qui puisse traîner dans une fenêtre en train de se dessiner.

    Rendu : [{'nom', 'taille', 'modele', 'amovible', 'rotatif'}], du plus grand
    au plus petit. Les disques de boucle, de RAM et les lecteurs de disquette
    sont écartés : ce ne sont pas des disques pour qui regarde son matériel.
    """
    racine = "/sys/block"
    trouves = []
    try:
        noms = sorted(os.listdir(racine))
    except OSError:
        return []

    for nom in noms:
        if nom.startswith(("loop", "ram", "zram", "fd", "sr", "dm-", "md")):
            continue
        base = os.path.join(racine, nom)

        # `size` est en secteurs de 512 octets, quelle que soit la taille de
        # secteur physique du disque. C'est la convention du noyau, et s'en
        # écarter donnerait des tailles fausses sur les disques 4K.
        try:
            with open(os.path.join(base, "size"), encoding="utf-8") as fichier:
                secteurs = int(fichier.read().strip())
        except (OSError, ValueError):
            continue
        if secteurs <= 0:
            continue

        def lire(quoi, defaut=""):
            try:
                with open(os.path.join(base, quoi), encoding="utf-8",
                          errors="replace") as fichier:
                    return fichier.read().strip()
            except OSError:
                return defaut

        trouves.append({
            "nom": nom,
            "taille": secteurs * 512 / 1000 ** 3,      # en Go, comme le vendeur
            "modele": lire("device/model") or lire("device/name") or "",
            "amovible": lire("removable") == "1",
            "rotatif": lire("queue/rotational") == "1",
        })

    trouves.sort(key=lambda d: d["taille"], reverse=True)
    return trouves


def genre_du_disque(disque_vu):
    """Ce qu'on en dit à quelqu'un : « SSD 500 Go », « clé USB 32 Go »."""
    if disque_vu["amovible"]:
        sorte = "amovible"
    elif disque_vu["rotatif"]:
        sorte = "disque dur"
    else:
        sorte = "SSD"
    return f"{sorte} de {disque_vu['taille']:.0f} Go"


def machine_virtuelle():
    """Le nom de l'hyperviseur si l'on tourne dans une machine virtuelle."""
    try:
        sortie = subprocess.run(["systemd-detect-virt"], capture_output=True, text=True,
                                timeout=5).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""
    return "" if sortie in ("none", "") else sortie


def son_et_video():
    """Ce que le systeme lit vraiment pour le son et la video.

    Quatre lectures, et aucune ne lance de programme : `pactl`, `vainfo` ou
    `lspci` interrogent des services qui peuvent mettre deux minutes a
    repondre — l'un d'eux a deja fige une fenetre tout ce temps. On lit des
    fichiers, qui repondent toujours tout de suite, et on dit ce qu'on a lu,
    jamais ce qu'on espere.
    """
    # Les profils qui disent a ALSA comment brancher les haut-parleurs : sans
    # eux, une carte pourtant reconnue reste silencieuse.
    profils = len(glob.glob("/usr/share/alsa/ucm2/**/*.conf", recursive=True))

    # La carte son et son pilote, lus dans ce que le noyau annonce.
    carte = ""
    try:
        with open("/proc/asound/cards", encoding="utf-8", errors="replace") as fichier:
            for ligne in fichier:
                if re.match(r"^\s*\d+\s*\[", ligne):
                    carte = ligne.split("[", 1)[1].split("]")[0].strip()
                    break
    except OSError:
        pass

    module = ""
    try:
        with open("/proc/asound/modules", encoding="utf-8", errors="replace") as fichier:
            for ligne in fichier:
                champs = ligne.split()
                if len(champs) >= 2:
                    module = champs[1]
                    break
    except OSError:
        pass

    # Le decodeur que le navigateur appelle pour chaque video, et les pilotes
    # qui peuvent decoder sur la carte graphique au lieu du processeur.
    decodeur = bool(glob.glob("/usr/lib/x86_64-linux-gnu/libavcodec.so.*"))
    pilotes_video = len(glob.glob("/usr/lib/x86_64-linux-gnu/dri/*_drv_video.so"))

    return {
        "profils_alsa": profils,
        "carte": carte,
        "module": module,
        "decodeur": decodeur,
        "pilotes_video": pilotes_video,
        "melangeur": melangeur(),
        "temps_reel": pipewire_temps_reel(),
    }


def melangeur():
    """Les canaux du mélangeur ALSA : leur niveau, et s'ils sont coupés.

    C'est la lecture qui manquait, et elle vaut toutes les autres. Le
    27 septembre, après cinq corrections sur la priorité temps réel et le
    tampon de PipeWire, Grenofar a répondu « le son fonctionne toujours pas ».
    Aucune de ces corrections ne peut faire sortir un son d'une carte dont le
    canal Master est coupé — et rien, dans grenOS, ne le disait ni ne le
    démontait.

    **Pourquoi un programme ici, alors que le reste de ce fichier n'en lance
    aucun** : `amixer` lit /dev/snd directement et répond tout de suite, là où
    `pactl`, `vainfo` et `vulkaninfo` interrogent des services qui peuvent
    mettre deux minutes — l'un d'eux a déjà figé une fenêtre vingt-cinq
    secondes. Il est de plus appelé depuis un fil, jamais pendant qu'une
    fenêtre se dessine, et borné à deux secondes. La règle n'est pas oubliée :
    elle vise les appels lents, et celui-ci ne l'est pas.
    """
    canaux = []
    for carte in re.findall(r"^\s*(\d+)\s*\[", _lire("/proc/asound/cards"), re.M):
        for canal in ("Master", "PCM", "Speaker", "Headphone"):
            try:
                sortie = subprocess.run(
                    ["amixer", "-c", carte, "sget", canal],
                    capture_output=True, text=True, timeout=2)
            except (OSError, subprocess.SubprocessError):
                continue
            if sortie.returncode != 0:
                continue
            # « [75%] [on] » ou « [0%] [off] » — on prend le premier canal
            # physique, les deux oreilles portent le même réglage.
            niveau = re.search(r"\[(\d+)%\]", sortie.stdout)
            coupe = "[off]" in sortie.stdout
            canaux.append({
                "carte": carte,
                "nom": canal,
                "niveau": int(niveau.group(1)) if niveau else None,
                "coupe": coupe,
            })
    return canaux


def pipewire_temps_reel():
    """PipeWire a-t-il un fil en temps réel ? Lu dans /proc, sans rien lancer.

    Le champ 41 de `/proc/<pid>/task/<tid>/stat` est la politique
    d'ordonnancement : 1 = FIFO, 2 = RR, 0 = ordinaire. C'est le fil
    `data-loop` qui porte le son et qui bascule — jamais le processus. Compter
    le processus au lieu de ses fils est l'erreur qui a coûté sept tentatives
    dans la nuit du 26 au 27.
    """
    for chemin in glob.glob("/proc/[0-9]*"):
        if _lire(chemin + "/comm").strip() != "pipewire":
            continue
        for fil in glob.glob(chemin + "/task/[0-9]*"):
            stat = _lire(fil + "/stat")
            # Le nom du fil est entre parenthèses et peut contenir des
            # espaces : on coupe après la dernière.
            champs = stat[stat.rfind(")") + 1:].split()
            # stat[0] est le 3e champ, donc le 41e est à l'indice 38.
            if len(champs) > 38 and champs[38] in ("1", "2"):
                return True
        return False
    return None


def _lire(chemin):
    try:
        with open(chemin, encoding="utf-8", errors="replace") as fichier:
            return fichier.read()
    except OSError:
        return ""


def resume():
    """Tout d'un coup, prêt à être affiché."""
    nombre, modele = coeurs()
    total, libre = memoire()
    carte, pilote = graphique()
    place, reste = disque()
    return {
        "coeurs": nombre,
        "processeur": modele or "processeur inconnu",
        "memoire": total,
        "memoire_libre": libre,
        "carte": carte,
        "pilote": pilote,
        "disque": place,
        "disque_libre": reste,
        # Tous les disques, pas seulement celui de la racine. Sans cela, une
        # machine a deux disques n'en montrait qu'un, et l'installateur en
        # proposait deux : de quoi douter de l'un ou de l'autre.
        "disques": disques(),
        "virtuelle": machine_virtuelle(),
    }
