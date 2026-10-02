"""Ce que GrenPlace sait des applications déjà posées : à jour, ou non.

Grenofar, le 1er octobre : « ajoute un max de trucs à GrenPlace (d'ailleurs
redesign-le pour que ça ressemble aussi à Windows, avec maj de l'app aussi) ».

POURQUOI UN MODULE, ET PAS DU CODE DANS LA FENÊTRE

Parce que les deux morceaux difficiles sont des **analyseurs de texte**, et
qu'un analyseur ne se vérifie qu'en l'exerçant sur des sorties réelles. Dans
une fenêtre GTK, il ne se vérifie jamais : il faudrait une machine, un réseau,
et des applications périmées au bon moment. Ici, il s'exerce en une seconde.

CE QUI EST LU À LA SOURCE, ET NON DE MÉMOIRE

`flatpak-remote-ls(1)`, version 1.16.6-1~deb13u2, lue dans le paquet :

    --updates     Show only those which have updates available
    --app         Show only applications, omit runtimes
    --columns=FIELD,...
        application   Show the application or runtime ID
    --cached      Prefer to use locally cached information if possible, even
                  though it may be out of date. This is faster, but risks
                  returning stale information.

On n'emploie donc PAS `--cached` : une liste de mises à jour périmée est pire
qu'une absence de liste, parce qu'elle a l'air d'une réponse.

Pour apt, on passe par `apt-get -s` et ses lignes `Inst `, et non par
`apt list --upgradable` : apt imprime lui-même, en tête de cette commande,
« WARNING: apt does not have a stable CLI interface ». La leçon est la même que
pour fwupd — lire la mise en page d'un programme qui prévient qu'elle change
est la façon la plus sûre de casser plus tard sans comprendre pourquoi.

RIEN ICI N'EST APPELÉ PENDANT QU'UNE FENÊTRE SE CONSTRUIT

Les deux commandes vont sur le réseau. `vulkaninfo` a déjà laissé la page Jeux
vide pendant vingt-cinq secondes ; la règle qu'on en a tirée vaut partout.
"""
import re
import subprocess

# Un identifiant Flatpak : des morceaux séparés par des points. On ne cherche
# pas à être strict — on cherche à ne pas prendre l'en-tête « Application » ni
# une ligne vide pour un identifiant.
_ID_FLATPAK = re.compile(r"^[A-Za-z0-9_-]+(\.[A-Za-z0-9_-]+)+$")

# `apt-get -s` écrit une ligne par paquet qui serait installé ou remplacé :
#
#     Inst libc6 [2.41-7] (2.41-8 Debian:trixie [amd64])
#
# Le premier mot après `Inst ` est le nom. Ce format est celui que lisent tous
# les outils qui parlent à apt sans passer par sa bibliothèque.
_INST_APT = re.compile(r"^Inst\s+(\S+)\s")


def ids_flatpak(sortie):
    """Les identifiants Flatpak d'une sortie de `remote-ls`. Rendu : un set.

    Exercé sur les quatre formes réelles : la liste nue, la liste avec en-tête,
    la sortie vide, et le message « Nothing to do » que flatpak écrit parfois.
    """
    trouves = set()
    for ligne in (sortie or "").splitlines():
        ligne = ligne.strip()
        if not ligne or ligne.lower() in ("application", "ref", "id"):
            continue
        # Avec --columns=application il n'y a qu'une colonne, mais une sortie
        # d'une autre version pourrait en porter plusieurs, séparées par des
        # tabulations. On prend la première qui ressemble à un identifiant.
        for morceau in re.split(r"[\t ]+", ligne):
            if _ID_FLATPAK.match(morceau):
                trouves.add(morceau)
                break
    return trouves


def paquets_apt(sortie):
    """Les paquets qu'apt remplacerait. Rendu : un set."""
    trouves = set()
    for ligne in (sortie or "").splitlines():
        trouve = _INST_APT.match(ligne.strip())
        if trouve:
            trouves.add(trouve.group(1))
    return trouves


def _courir(commande, delai):
    try:
        fin = subprocess.run(commande, capture_output=True, text=True, timeout=delai)
    except (OSError, subprocess.SubprocessError):
        return ""
    return fin.stdout or ""


def flatpaks_a_mettre_a_jour(delai=90):
    """Les Flatpak installés qui ont une version plus récente en ligne."""
    return ids_flatpak(_courir(
        ["flatpak", "remote-ls", "--updates", "--app", "--columns=application"],
        delai))


def paquets_a_mettre_a_jour(delai=90):
    """Les paquets Debian qu'une mise à jour remplacerait.

    `-s` simule : rien n'est installé, et la commande ne demande pas root.
    """
    return paquets_apt(_courir(["apt-get", "-s", "-q", "upgrade"], delai))


def a_mettre_a_jour(applications, delai=90):
    """Parmi ces fiches du catalogue, celles qui ont une mise à jour.

    Rendu : la liste des fiches, dans l'ordre du catalogue.

    On ne pose qu'UNE question par source, puis on croise — plutôt qu'une
    question par application. Quarante-trois `flatpak info` seraient quarante-
    trois allers-retours, et la fenêtre attendrait.
    """
    veut_flatpak = any(a.get("source") == "flatpak" for a in applications)
    veut_apt = any(a.get("source") == "apt" for a in applications)
    flatpaks = flatpaks_a_mettre_a_jour(delai) if veut_flatpak else set()
    paquets = paquets_a_mettre_a_jour(delai) if veut_apt else set()

    retard = []
    for application in applications:
        identifiant = application.get("identifiant", "")
        if application.get("source") == "flatpak" and identifiant in flatpaks:
            retard.append(application)
        elif application.get("source") == "apt" and identifiant in paquets:
            retard.append(application)
    return retard
