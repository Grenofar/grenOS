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


# Le profil d'une sortie, en français.
#
# POURQUOI NOUS, ALORS QUE LA TRADUCTION EXISTE DÉJÀ
#
# Le sélecteur de sortie affichait « Audio interne Analog Stereo » — la moitié
# de la phrase traduite, l'autre non. Trouvé le 28 septembre en agrandissant la
# capture du panneau de son, et la cause a été suivie jusqu'au bout :
#
#   - `wireplumber/scripts/monitors/alsa.lua` passe le nom de l'appareil par
#     `I18n.gettext()`, d'où « Built-in Audio » → « Audio interne » ;
#   - puis il colle le profil derrière, SANS le traduire — `libspa-alsa.so` ne
#     contient pas un seul symbole gettext, vérifié.
#
# Et le détail qui décide : **la traduction existe sur la machine**.
# `libpipewire-0.3-common` livre `/usr/share/locale/fr/LC_MESSAGES/pipewire.mo`,
# qui contient « Analog Stereo » → « Stéréo analogique ». Elle est là, personne
# ne l'appelle. C'est un défaut de WirePlumber, pas de grenOS — mais c'est
# notre écran, et c'est celui qu'on ouvre le jour où il n'y a pas de son.
#
# Les plus longs d'abord : « Digital Stereo (HDMI) » doit être reconnu avant
# « Stereo », sans quoi on traduirait la moitié d'une moitié.
PROFILS = (
    ("Digital Surround 7.1 (HDMI)", "Surround numérique 7.1 (HDMI)"),
    ("Digital Surround 5.1 (HDMI)", "Surround numérique 5.1 (HDMI)"),
    ("Digital Stereo (IEC958)", "Stéréo numérique (S/PDIF)"),
    ("Digital Stereo (HDMI)", "Stéréo numérique (HDMI)"),
    ("Analog Surround 7.1", "Surround analogique 7.1"),
    ("Analog Surround 5.1", "Surround analogique 5.1"),
    ("Analog Stereo Duplex", "Stéréo analogique bidirectionnelle"),
    ("Analog Stereo Output", "Sortie stéréo analogique"),
    ("Analog Stereo Input", "Entrée stéréo analogique"),
    ("Analog Stereo", "Stéréo analogique"),
    ("Analog Mono", "Mono analogique"),
    ("Digital Stereo", "Stéréo numérique"),
    ("Line Out", "Sortie ligne"),
    ("Headphones", "Casque"),
    ("Headset", "Micro-casque"),
    ("Speakers", "Haut-parleurs"),
    ("Internal Microphone", "Micro interne"),
    ("Microphone", "Micro"),
)


def profil_en_francais(description):
    """Le nom d'une sortie, entièrement en français.

    On ne touche QUE ce qui s'affiche : l'identifiant de la sortie, lui, reste
    celui de PipeWire, sans quoi on ne saurait plus la sélectionner.
    """
    if not description:
        return description
    for anglais, francais in PROFILS:
        if anglais in description:
            description = description.replace(anglais, francais)
    return description


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
                trouvees.append((nom, profil_en_francais(description) or nom))
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


def applications_qui_jouent():
    """Ce qui fait du bruit en ce moment, et à quel volume.

    Rendu : [{'index', 'nom', 'volume'}]. C'est le mélangeur : régler le
    volume de la machine ne suffit pas quand une seule application crie.
    """
    trouvees = []
    brut = _pactl("list", "sink-inputs")
    index, nom, volume = "", "", 0
    for ligne in brut.splitlines():
        depouille = ligne.strip()
        if depouille.startswith("Sink Input #"):
            if index:
                trouvees.append({"index": index, "nom": nom or "Application",
                                 "volume": volume})
            index = depouille.split("#", 1)[1].strip()
            nom, volume = "", 0
        elif depouille.startswith("application.name ="):
            nom = depouille.split("=", 1)[1].strip().strip('"')
        elif depouille.startswith("Volume:") and not volume:
            trouve = re.search(r"(\d+)%", depouille)
            if trouve:
                volume = int(trouve.group(1))
    if index:
        trouvees.append({"index": index, "nom": nom or "Application", "volume": volume})
    return trouvees


def regler_application(index, pourcentage):
    """Le volume d'une seule application."""
    _pactl("set-sink-input-volume", str(index), f"{int(pourcentage)}%")


def nom_lisible(identifiant):
    """Le nom d'une sortie, tel qu'on le dirait : « Haut-parleurs », pas une
    suite de chiffres et de points."""
    for nom, description in sorties():
        if nom == identifiant:
            return description
    return identifiant


# ---------------------------------------------------------------------------
# POURQUOI N'ENTEND-ON RIEN ? LA QUESTION POSÉE, PLUS DEVINÉE.
#
# Grenofar l'a dit trois fois : le 24 septembre, le 27 septembre (« le son
# fonctionne toujours pas »), et le 7 octobre (« le son focntionne pas »).
# Trois fois, j'ai cherché une cause et annoncé une correction, et trois fois
# il est revenu. Le compte exact des hypothèses que j'ai émises sans mesure :
#
#   dbus-user-session absent · alsa-ucm-conf absent · rtkit absent · le tampon
#   trop serré · la priorité temps réel · le mélangeur coupé
#
# Six. Les six étaient de vrais manques, aucun n'était SA panne. Et la case B4
# du plan du 26 septembre disait déjà, en toutes lettres, ce qu'il fallait
# faire à la place : « c'est le chiffre qui transforme "ça grésille" en
# diagnostic. Sans lui, la prochaine fois je devinerai encore. » Je n'ai posé
# que la moitié de la case — le mélangeur et le temps réel — et la prochaine
# fois est arrivée.
#
# Ce que la CI ne peut PAS voir, et qui explique pourquoi le défaut survit :
# elle a UNE carte, UNE sortie, et le mélangeur d'`intel-hda` sous QEMU sort
# ouvert. Elle entend un son propre et elle dit vrai. Toutes les pannes
# ci-dessous demandent une machine qui a plusieurs sorties — c'est-à-dire la
# sienne, et jamais la nôtre.
#
# LA PISTE QUE CE CODE EXISTE POUR TRANCHER : sur une machine avec une carte
# graphique, PipeWire choisit couramment la sortie HDMI de cette carte comme
# sortie par défaut. Le son part alors vers l'écran — qui n'a pas forcément de
# haut-parleurs — pendant que les enceintes analogiques restent muettes. Pile
# logicielle impeccable, volume à 80 %, mélangeur ouvert, et zéro son.
# Grenofar a une vraie carte graphique : il a demandé le même jour la mise à
# jour des pilotes GPU.
#
# On ne CORRIGE rien ici, et c'est volontaire. Changer sa sortie par défaut
# serait décider à sa place laquelle est la bonne — et si ses enceintes sont
# branchées en HDMI, nous casserions le son de quelqu'un chez qui il marche.
# C'est la faute exacte du mélangeur posé à 80 % le 27 septembre, qui a fait
# tomber la crête de 10 584 à 2 215. On nomme, il choisit : le sélecteur de
# sortie est déjà dans la page.
# ---------------------------------------------------------------------------

# Ce qui, dans le nom d'une sortie, désigne un écran et non des enceintes.
# `pactl` nomme ses sorties d'après le profil ALSA : `...hdmi-stereo`,
# `...hdmi-surround`, et DisplayPort chez les cartes récentes.
VERS_UN_ECRAN = ("hdmi", "displayport", "display-port")


def _vers_un_ecran(nom):
    nom = (nom or "").lower()
    return any(marque in nom for marque in VERS_UN_ECRAN)


def diagnostic():
    """Ce qui empêche d'entendre, nommé. Rend (verdict, [soucis]).

    Les soucis sont dans l'ordre où ils tuent le son : pas de carte, puis
    coupé, puis à zéro, puis envoyé au mauvais endroit. Le premier de la liste
    est celui qu'il faut regarder — les suivants peuvent n'être que des
    conséquences.
    """
    liste = sorties()
    soucis = []

    if not liste:
        # Aucune sortie. Et il faut le dire franchement : aucune mise à jour
        # ne peut ajouter une carte son à une machine qui n'en déclare pas.
        return ("Aucune sortie audio : PipeWire ne voit aucune carte son.",
                ["aucune carte son"])

    courante = sortie_actuelle()
    lisible = ""
    for nom, texte in liste:
        if nom == courante:
            lisible = texte
            break

    if muet():
        soucis.append("la sortie est coupée")
    niveau = volume()
    if niveau < 5:
        soucis.append("le volume de la sortie est à %d %%" % niveau)

    # Le son part-il vers l'écran alors qu'une autre sortie existe ?
    autres = [texte for nom, texte in liste if not _vers_un_ecran(nom)]
    if _vers_un_ecran(courante) and autres:
        soucis.append(
            "le son part vers l'écran (HDMI) alors qu'une autre sortie existe : "
            + ", ".join(autres[:2]))

    if not soucis:
        verdict = "Le son devrait s'entendre sur « %s »." % (lisible or courante
                                                             or "la sortie par défaut")
    else:
        verdict = "Le son ne sortira pas : " + soucis[0] + "."
    return verdict, soucis
