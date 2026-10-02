"""Les composants de la machine, et leurs mises à jour.

Demandé le 1er octobre : « QUE MAJ INTEL AMD PROC ET GPU NVIDIA INTEL AMD SE
FASSE DANS MISE A JOUR COMPOSANT ».

CE QUI EXISTE DÉJÀ, ET QU'ON N'ÉCRIT PAS

`fwupd` est l'outil des mises à jour de micrologiciels sous Linux, et il n'y en
a pas d'autre : c'est lui que Fedora, Ubuntu, Dell, Lenovo et System76
emploient. Il parle au LVFS — le dépôt de micrologiciels des constructeurs — et
met à jour BIOS, contrôleurs, micrologiciels de GPU, de disque, de clavier.

Sa page de manuel (fwupd 2.0.20, lue dans le paquet) le dit en une phrase :

    Most users who want to just update all devices to the latest versions can
    do `fwupdmgr refresh` and then `fwupdmgr update`.

Et pour lire ses réponses sans dépendre de la mise en page :

    The terminal output between versions of fwupd is not guaranteed to be
    stable, but if you plan on parsing the results then adding `json` might be
    just what you need.

D'où `--json` partout ici. Lire la sortie d'affichage d'un programme qui
prévient lui-même qu'elle change est la façon la plus sûre de casser dans six
mois sans comprendre pourquoi.

CE QUE fwupd NE FAIT PAS, ET QU'IL FAUT DIRE

Le **microcode** du processeur Intel et AMD n'arrive pas par là : il vient des
paquets `intel-microcode` et `amd64-microcode`, qui sont dans l'image depuis le
24 septembre et que « Mettre à jour grenOS » installe comme le reste. La page
le dit, au lieu de laisser croire que fwupd s'en occupe.

Le **pilote** NVIDIA n'arrive pas par là non plus : c'est un paquet, proposé
par la page Jeux quand la machine a une carte NVIDIA.

RIEN NE TOURNE PENDANT QUE LA FENÊTRE SE CONSTRUIT

`vulkaninfo` a déjà laissé la page Jeux vide pendant vingt-cinq secondes. Toute
lecture ici est faite depuis un fil, et la page affiche « Lecture… » en
attendant.
"""
import json
import subprocess


def _fwupdmgr(*arguments, delai=25):
    """Appelle fwupdmgr et rend sa sortie, ou "" si quelque chose cloche."""
    try:
        fin = subprocess.run(["fwupdmgr", *arguments],
                             capture_output=True, text=True, timeout=delai)
    except (OSError, subprocess.SubprocessError):
        return ""
    return fin.stdout or ""


def present():
    """fwupd est-il installé sur cette machine ?"""
    try:
        return subprocess.run(["fwupdmgr", "--version"],
                              capture_output=True, timeout=10).returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def _nom_lisible(appareil):
    """Le nom d'un composant, tel qu'on le dirait."""
    nom = appareil.get("Name") or "Composant"
    vendeur = appareil.get("Vendor") or ""
    if vendeur and not nom.lower().startswith(vendeur.lower()):
        return "%s %s" % (vendeur, nom)
    return nom


def composants():
    """Ce que la machine contient, vu par fwupd.

    Rendu : [{'nom', 'version', 'peut_etre_mis_a_jour'}]

    On ne lève aucune exception : une machine virtuelle n'a souvent aucun
    composant que fwupd sache lire, et c'est un cas normal, pas une panne.
    """
    brut = _fwupdmgr("get-devices", "--json")
    if not brut:
        return []
    try:
        lu = json.loads(brut)
    except ValueError:
        return []

    trouves = []
    for appareil in lu.get("Devices", []):
        version = appareil.get("Version")
        if not version:
            # Sans version affichable, la ligne n'apprendrait rien.
            continue
        drapeaux = appareil.get("Flags", []) or []
        trouves.append({
            "nom": _nom_lisible(appareil),
            "version": version,
            "peut_etre_mis_a_jour": "updatable" in drapeaux,
        })
    return trouves


def mises_a_jour():
    """Les mises à jour disponibles. Rendu : [{'nom', 'de', 'vers'}].

    `get-updates` rend 2 quand il n'y a rien à faire — ce n'est pas une erreur,
    c'est une réponse. On ne regarde donc que la sortie.
    """
    brut = _fwupdmgr("get-updates", "--json")
    if not brut:
        return []
    try:
        lu = json.loads(brut)
    except ValueError:
        return []

    trouvees = []
    for appareil in lu.get("Devices", []):
        versions = appareil.get("Releases") or []
        if not versions:
            continue
        trouvees.append({
            "nom": _nom_lisible(appareil),
            "de": appareil.get("Version") or "?",
            "vers": versions[0].get("Version") or "?",
        })
    return trouvees


def rafraichir():
    """Va chercher la liste des micrologiciels chez les constructeurs.

    Rendu : (réussi, ce qu'il a dit). `--force` parce que fwupd refuse de
    reprendre la liste s'il l'a déjà prise récemment, et que quelqu'un qui
    clique sur un bouton veut qu'il se passe quelque chose.
    """
    try:
        fin = subprocess.run(["fwupdmgr", "refresh", "--force"],
                             capture_output=True, text=True, timeout=120)
    except (OSError, subprocess.SubprocessError) as souci:
        return False, str(souci)
    sortie = (fin.stdout or "") + (fin.stderr or "")
    return fin.returncode == 0, sortie.strip()
