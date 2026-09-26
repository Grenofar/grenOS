"""Ce que la machine a sous le capot, et ce que grenOS en fait.

Il n'y a rien à « débloquer » : Linux utilise d'office tous les cœurs et toute
la mémoire qu'on lui donne. La question utile n'est donc pas « comment tout
prendre », c'est « qu'est-ce que la machine voit vraiment ? » — surtout dans
une machine virtuelle, où c'est l'hôte qui décide, et où un écran de réglages
honnête vaut mieux qu'une promesse.
"""
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
    return carte or "carte graphique inconnue", pilote or "pilote générique"


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
