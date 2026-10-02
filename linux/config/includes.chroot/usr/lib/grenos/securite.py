"""L'antivirus, et ce qu'il peut vraiment dire.

Demandé le 1er octobre : « ajouter un antivirus linux préinstaller genre
malwarebyte ou avast ou peu importe ».

POURQUOI CE N'EST NI MALWAREBYTES NI AVAST

Aucun des deux n'a de produit Linux de bureau. Avast a retiré son antivirus
Linux en 2021 ; Malwarebytes n'en a jamais publié. Les nommer dans une
interface serait une promesse à l'écran sans rien derrière — exactement la
forme de défaut que ce dépôt passe son temps à trouver.

Sous Linux, l'antivirus qui existe est **ClamAV** : libre, dans Debian, sa base
de signatures tenue à jour par ses auteurs. C'est celui qu'emploient les
serveurs de courrier du monde entier.

CE QU'IL NE FAUT PAS LAISSER CROIRE

Un antivirus sous Linux ne sert pas d'abord à protéger Linux : les logiciels
malveillants visant un bureau Linux sont rares. Il sert surtout à **ne pas
faire suivre** un fichier infecté à une machine Windows — une clé USB, une
pièce jointe, un fichier partagé. La page le dit, parce qu'un antivirus qui
laisse croire qu'il protège de tout est pire qu'aucun.

POURQUOI LES SIGNATURES NE SONT PAS DANS L'IMAGE

Elles pèsent plusieurs centaines de mégaoctets et changent tous les jours :
dans l'ISO elles seraient périmées avant d'être téléchargées. `freshclam` n'est
donc pas armé au démarrage — il serait désagréable qu'une machine neuve tire
300 Mo sans prévenir sur la première connexion de quelqu'un. C'est la page qui
le demande, une fois, en le disant.
"""
import glob
import os
import subprocess
import time

DOSSIER_SIGNATURES = "/var/lib/clamav"


def present():
    """ClamAV est-il installé ?"""
    for outil in ("clamscan", "freshclam"):
        try:
            if subprocess.run(["which", outil],
                              capture_output=True, timeout=8).returncode != 0:
                return False
        except (OSError, subprocess.SubprocessError):
            return False
    return True


def signatures():
    """L'état de la base de signatures.

    Rendu : {'presentes', 'fichiers', 'octets', 'date'}

    On lit le disque plutôt que d'interroger un démon : `clamav-freshclam` n'est
    pas armé chez nous, donc il n'y a aucun démon à interroger, et une question
    posée à un service absent reste sans réponse pendant son délai d'attente.
    """
    fichiers = []
    for motif in ("*.cvd", "*.cld"):
        fichiers.extend(glob.glob(os.path.join(DOSSIER_SIGNATURES, motif)))

    octets, plus_recent = 0, 0
    for chemin in fichiers:
        try:
            etat = os.stat(chemin)
        except OSError:
            continue
        octets += etat.st_size
        plus_recent = max(plus_recent, int(etat.st_mtime))

    return {
        "presentes": bool(fichiers),
        "fichiers": len(fichiers),
        "octets": octets,
        "date": (time.strftime("%Y-%m-%d %H:%M", time.gmtime(plus_recent))
                 if plus_recent else ""),
    }


def resume():
    """Une phrase qui dit où on en est, et qui ne ment pas."""
    if not present():
        return "ClamAV n'est pas installé sur cette machine."
    etat = signatures()
    if not etat["presentes"]:
        return ("Les signatures ne sont pas encore là. Sans elles, une analyse "
                "ne trouverait rien — et dirait que tout va bien.")
    return ("Base de %d fichier%s, %.0f Mio, du %s."
            % (etat["fichiers"], "s" if etat["fichiers"] > 1 else "",
               etat["octets"] / 1048576.0, etat["date"] or "?"))
