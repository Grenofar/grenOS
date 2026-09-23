"""Le son : par où il sort, et à quel volume.

Tout passe par PipeWire, que `pactl` sait piloter comme il pilotait PulseAudio.
Deux besoins seulement, et ce sont ceux qu'on cherche dans des réglages :
choisir la sortie (les enceintes, le casque, l'écran par HDMI) et régler le
volume.
"""
import re
import subprocess


def _pactl(*arguments):
    try:
        return subprocess.run(["pactl", *arguments], capture_output=True, text=True,
                              timeout=8).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def sorties():
    """Les sorties audio disponibles : [(identifiant, nom lisible)]."""
    trouvees = []
    brut = _pactl("list", "sinks")
    nom, description = "", ""
    for ligne in brut.splitlines():
        ligne = ligne.strip()
        if ligne.startswith("Name:"):
            nom = ligne.split(":", 1)[1].strip()
        elif ligne.startswith("Description:"):
            description = ligne.split(":", 1)[1].strip()
            if nom:
                trouvees.append((nom, description or nom))
                nom, description = "", ""
    return trouvees


def sortie_actuelle():
    """La sortie utilisée en ce moment."""
    for ligne in _pactl("info").splitlines():
        if ligne.startswith("Default Sink:"):
            return ligne.split(":", 1)[1].strip()
    return ""


def choisir_sortie(nom):
    """Change la sortie, et y déplace ce qui joue déjà."""
    _pactl("set-default-sink", nom)
    for ligne in _pactl("list", "short", "sink-inputs").splitlines():
        champs = ligne.split()
        if champs:
            _pactl("move-sink-input", champs[0], nom)


def volume():
    """Le volume de la sortie courante, de 0 à 100."""
    brut = _pactl("get-sink-volume", "@DEFAULT_SINK@")
    trouve = re.search(r"(\d+)%", brut)
    return int(trouve.group(1)) if trouve else 0


def regler_volume(pourcentage):
    _pactl("set-sink-volume", "@DEFAULT_SINK@", f"{int(pourcentage)}%")


def muet():
    return "yes" in _pactl("get-sink-mute", "@DEFAULT_SINK@")


def basculer_muet():
    _pactl("set-sink-mute", "@DEFAULT_SINK@", "toggle")
