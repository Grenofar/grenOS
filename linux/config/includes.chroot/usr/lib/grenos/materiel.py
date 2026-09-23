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
        "virtuelle": machine_virtuelle(),
    }
