"""Ce qu'il faut pour jouer, et où en est cette machine.

Trois questions, et rien d'autre : la carte graphique sait-elle dessiner en 3D,
ce qu'il faut pour jouer est-il installé, et ce jeu-là tourne-t-il vraiment sur
Linux ?

La troisième mérite d'être dite franchement, parce qu'elle décide de tout :
certains jeux ne tournent pas, et ce n'est pas la faute du système. Quand
l'éditeur refuse d'activer son anti-triche sur Linux, aucun réglage n'y change
rien. Mieux vaut le dire et proposer le chemin qui marche que promettre et
décevoir.
"""
import os
import subprocess

import materiel

# Ce qui s'installe depuis la page Jeux. Les Flatpak passent par Flathub, les
# paquets par les dépôts Debian — les mêmes chemins que GrenPlace, pour que
# tout se mette à jour ensemble.
OUTILS = [
    {
        "cle": "steam",
        "nom": "Steam",
        "note": "La boutique et la couche Proton, qui fait tourner les jeux Windows.",
        "source": "flatpak",
        "identifiant": "com.valvesoftware.Steam",
    },
    {
        "cle": "protonup",
        "nom": "ProtonUp-Qt",
        "note": "Installe les versions GE de Proton, qui font tourner ce que Proton "
                "officiel ne fait pas encore.",
        "source": "flatpak",
        "identifiant": "net.davidotek.pupgui2",
    },
    {
        "cle": "gamemode",
        "nom": "GameMode",
        "note": "Donne la priorité au jeu pendant qu'il tourne, et la rend au reste "
                "quand il se ferme.",
        "source": "apt",
        "identifiant": "gamemode",
    },
    {
        "cle": "mangohud",
        "nom": "MangoHud",
        "note": "Affiche les images par seconde et la charge du processeur par-dessus "
                "le jeu.",
        "source": "apt",
        "identifiant": "mangohud",
    },
]

# Les jeux que Grenofar a nommés, avec ce qui est vrai de chacun au
# 24 septembre 2026. Vérifié à la source, pas de mémoire.
JEUX = [
    {
        "cle": "rocket-league",
        "nom": "Rocket League",
        "etat": "marche",
        "note": "Fonctionne sur Linux depuis le 28 avril 2026 : Psyonix a activé la "
                "version Linux d'Easy Anti-Cheat, parties classées comprises. Il faut "
                "Proton Experimental ou un GE-Proton récent, à choisir dans Steam.",
        "action": "steam",
        "adresse": "steam://store/252950",
    },
    {
        "cle": "fortnite",
        "nom": "Fortnite",
        "etat": "en nuage",
        "note": "Ne tourne pas sur Linux, et ce n'est pas un défaut du système : "
                "Easy Anti-Cheat sait fonctionner sous Linux, mais Epic refuse de "
                "l'activer pour Fortnite. Le chemin qui marche est le jeu en nuage — "
                "gratuit, dans le navigateur, avec un compte Microsoft.",
        "action": "nuage",
        "adresse": "https://www.xbox.com/play/games/fortnite/BT5P2X999VH2",
    },
]

NUAGE_DESKTOP = os.path.expanduser("~/.local/share/applications/grenos-fortnite.desktop")


def installe(source, identifiant):
    """Cette chose est-elle déjà là ?"""
    if source == "flatpak":
        return subprocess.run(["flatpak", "info", identifiant],
                              capture_output=True).returncode == 0
    return subprocess.run(["dpkg-query", "-W", "-f=${Status}", identifiant],
                          capture_output=True, text=True).stdout.startswith("install ok")


def vulkan():
    """La machine sait-elle dessiner en 3D ? Rendu : (oui, ce qu'on a trouvé)."""
    # Six secondes, pas vingt-cinq : cette question est posée pendant la
    # construction de la fenêtre, donc **avant** qu'elle s'affiche. Sur une
    # machine sans pilote, `vulkaninfo` traîne, et la personne restait devant
    # un écran vide sans savoir si le programme avait démarré. Une machine qui
    # met plus de six secondes à répondre n'a de toute façon pas de 3D utile.
    try:
        sortie = subprocess.run(["vulkaninfo", "--summary"], capture_output=True,
                                text=True, timeout=6)
    except (OSError, subprocess.SubprocessError):
        return False, "Vulkan n'a pas pu être interrogé."
    if sortie.returncode != 0:
        return False, "Aucun pilote Vulkan : les jeux récents ne démarreront pas."
    for ligne in sortie.stdout.splitlines():
        if "deviceName" in ligne:
            return True, ligne.split("=", 1)[-1].strip()
    return True, "Vulkan répond."


def pilote_propose():
    """Le pilote qu'il faudrait installer, s'il en manque un."""
    carte, pilote = materiel.graphique()
    minuscules = (carte + " " + pilote).lower()
    if "nvidia" in minuscules and "nvidia" not in pilote.lower():
        return ("nvidia-driver",
                "Cette carte est une NVIDIA, et le pilote libre en cours ne sait pas "
                "jouer. Le pilote de NVIDIA le sait. Il faut redémarrer après.")
    if "amd" in minuscules or "radeon" in minuscules:
        return ("", "Le pilote libre d'AMD fait tourner les jeux sans rien installer.")
    if "intel" in minuscules:
        return ("", "Le pilote libre d'Intel fait tourner les jeux sans rien installer.")
    return ("", "")


def machine_virtuelle():
    """Dans une machine virtuelle, il n'y a pas de 3D — autant le dire."""
    return materiel.machine_virtuelle()


def commande_installation(source, identifiant):
    """Ce qui installe vraiment, tel qu'on pourrait le taper soi-même."""
    if source == "flatpak":
        return ("flatpak remote-add --if-not-exists flathub "
                "https://dl.flathub.org/repo/flathub.flatpakrepo && "
                f"flatpak install -y flathub {identifiant}")
    return f"apt-get update && apt-get install -y {identifiant}"


def poser_fortnite():
    """Pose Fortnite en nuage comme une application ordinaire.

    C'est une page web, mais ouverte sans barre d'adresse ni onglets : lancée
    depuis le menu ou le bureau, elle se comporte comme un jeu installé. Le
    navigateur doit être un Chromium — le service de Microsoft refuse Firefox.
    """
    navigateur = ""
    for candidat in ("chromium", "chromium-browser", "google-chrome"):
        if subprocess.run(["which", candidat], capture_output=True).returncode == 0:
            navigateur = candidat
            break
    if not navigateur:
        return "chromium"          # à installer d'abord

    os.makedirs(os.path.dirname(NUAGE_DESKTOP), exist_ok=True)
    adresse = JEUX[1]["adresse"]
    with open(NUAGE_DESKTOP, "w", encoding="utf-8") as fichier:
        fichier.write(
            "[Desktop Entry]\n"
            "Type=Application\n"
            "Name=Fortnite\n"
            "Comment=Fortnite en nuage, par Xbox Cloud Gaming\n"
            f"Exec={navigateur} --app={adresse} --start-maximized\n"
            "Icon=grenos-nuage\n"
            "Terminal=false\n"
            "Categories=Game;\n"
            "StartupWMClass=Chromium\n")
    os.chmod(NUAGE_DESKTOP, 0o755)

    # Et sur le bureau, là où l'on cherche un jeu.
    bureau = os.path.expanduser("~/Bureau")
    if os.path.isdir(bureau):
        cible = os.path.join(bureau, "grenos-fortnite.desktop")
        try:
            with open(NUAGE_DESKTOP, encoding="utf-8") as source:
                contenu = source.read()
            with open(cible, "w", encoding="utf-8") as sortie:
                sortie.write(contenu)
            os.chmod(cible, 0o755)
        except OSError:
            pass
    return ""


def fortnite_pose():
    return os.path.exists(NUAGE_DESKTOP)
