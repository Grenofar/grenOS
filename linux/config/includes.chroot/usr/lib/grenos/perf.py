"""Ce que grenOS fait tourner en arrière-plan, et ce que ça coûte.

Demandé mot pour mot : « un système grenPerf, pour analyser les tâches
arrière-plan pas nécessaires au bon fonctionnement de l'OS, et gestion de la
RAM », et, deux lignes plus haut, « que grenOS et ses fonctions ne doivent pas
beaucoup consommer en arrière-plan ».

La partie difficile n'est pas de compter les services : c'est de dire lequel
est **de trop**. Un compte ne désigne personne — « 17 services » ne dit pas
lequel retirer. D'où le registre ci-dessous, où chaque verdict porte sa
raison, exactement comme `verif-recommandes.py` pour les paquets.

Et la case la plus importante du registre est la dernière : **inconnu**. Un
registre qui prétend connaître tout ce qui tourne ment la première fois qu'un
paquet neuf arrive. Celui-ci dit « je ne sais pas » et le montre.
"""
import os
import re
import subprocess

INDISPENSABLE = "indispensable"
CONFORT = "confort"
EN_TROP = "en trop"
INCONNU = "inconnu"

#: Pour chaque service : sa classe, et **pourquoi**. La raison compte autant
#: que la classe — sans elle, personne ne peut contredire le verdict.
AVIS = {
    "dbus": (INDISPENSABLE,
             "tout le bureau se parle par lui ; sans lui, rien ne s'ouvre"),
    "systemd-journald": (INDISPENSABLE,
                         "sans lui, aucun journal, donc aucune panne lisible"),
    "systemd-udevd": (INDISPENSABLE,
                      "c'est lui qui voit les périphériques apparaître"),
    "systemd-logind": (INDISPENSABLE,
                       "les sessions, la veille et l'extinction passent par lui"),
    "polkit": (INDISPENSABLE,
               "sans lui, ni montage de disque, ni installateur, ni mise à "
               "jour sans mot de passe"),
    "lightdm": (INDISPENSABLE, "c'est lui qui ouvre le bureau"),
    "NetworkManager": (INDISPENSABLE,
                       "le réseau sans passer par le terminal, c'est lui"),
    "udisks2": (INDISPENSABLE,
                "c'est lui qui monte les disques dans l'explorateur"),
    "earlyoom": (INDISPENSABLE,
                 "il tue une application avant que la machine ne gèle ; "
                 "ajouté le 2 octobre"),

    "irqbalance": (CONFORT,
                   "il répartit les interruptions sur les cœurs : utile à "
                   "partir de deux cœurs, inutile sur un seul"),
    "getty@tty1": (CONFORT,
                   "une console de secours sur la première touche ; elle ne "
                   "coûte presque rien et sauve une machine sans bureau"),
    "cron": (CONFORT,
             "rien de grenOS ne s'en sert ; des paquets Debian, oui"),
    "accounts-daemon": (CONFORT,
                        "l'écran de connexion y lit la liste des comptes"),
    "NetworkManager-dispatcher": (CONFORT,
                                  "il lance des scripts quand le réseau "
                                  "change ; nous n'en avons aucun"),
    "systemd-hostnamed": (CONFORT,
                          "démarré à la demande, il s'arrête tout seul"),
    "systemd-timedated": (CONFORT,
                          "démarré à la demande, il s'arrête tout seul"),

    "grenos-preuve": (EN_TROP,
                      "il n'existe que pour écrire sur le port série de la "
                      "machine d'essai ; sur une vraie machine il ne sert à "
                      "rien et s'arrête de lui-même faute de port"),
}


def avis(nom):
    """La classe d'un service et sa raison, sans jamais inventer.

    Le défaut par défaut est `inconnu`, et c'est voulu : prétendre savoir ce
    qu'est un service qu'on n'a jamais vu serait exactement la promesse à
    l'écran sans rien derrière.
    """
    return AVIS.get(nom, (INCONNU,
                          "je ne sais pas ce que c'est, et je ne vais pas le "
                          "deviner"))


def _commande(arguments, delai=6):
    try:
        fin = subprocess.run(arguments, capture_output=True, text=True,
                             timeout=delai, check=False)
        return fin.stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def services_en_marche():
    """Les services effectivement en marche, nommés sans leur suffixe.

    **La même commande que la sonde du port série**, mot pour mot. Deux
    mesures d'une même chose finissent par diverger ; la machine d'intégration
    vérifie d'ailleurs que les deux comptes s'accordent.
    """
    sortie = _commande(["systemctl", "list-units", "--type=service",
                        "--state=running", "--no-legend", "--no-pager"])
    noms = []
    for ligne in sortie.splitlines():
        morceaux = ligne.split()
        if not morceaux:
            continue
        nom = morceaux[0]
        if nom.endswith(".service"):
            noms.append(nom[:-len(".service")])
    return sorted(noms)


def memoire():
    """La mémoire, décomposée — et surtout ce qui est *récupérable*.

    « 3,1 Go sur 4 utilisés » fait peur et ne veut rien dire : le cache
    compte dedans, et il se rend à la première demande. La seule ligne qui
    renseigne est celle que le noyau appelle disponible.
    """
    valeurs = {}
    try:
        with open("/proc/meminfo", encoding="utf-8") as fichier:
            for ligne in fichier:
                nom, _, reste = ligne.partition(":")
                nombre = re.search(r"(\d+)", reste)
                if nombre:
                    valeurs[nom] = int(nombre.group(1)) / 1024 / 1024
    except OSError:
        return {}

    total = valeurs.get("MemTotal", 0.0)
    dispo = valeurs.get("MemAvailable", 0.0)
    cache = valeurs.get("Cached", 0.0) + valeurs.get("Buffers", 0.0)
    swap_total = valeurs.get("SwapTotal", 0.0)
    return {
        "total": total,
        "disponible": dispo,
        "utilisee": max(0.0, total - dispo),
        "cache": cache,
        "swap_total": swap_total,
        "swap_utilise": max(0.0, swap_total - valeurs.get("SwapFree", 0.0)),
    }


def gourmands(combien=8):
    """Les processus qui prennent le plus de mémoire résidente.

    Lu dans /proc, sans `ps` : un programme de moins à justifier, et un délai
    de moins à borner.
    """
    trouves = []
    try:
        entrees = os.listdir("/proc")
    except OSError:
        return trouves

    for entree in entrees:
        if not entree.isdigit():
            continue
        try:
            with open("/proc/%s/statm" % entree, encoding="utf-8") as fichier:
                champs = fichier.read().split()
            with open("/proc/%s/comm" % entree, encoding="utf-8") as fichier:
                nom = fichier.read().strip()
        except OSError:
            continue
        if len(champs) < 2:
            continue
        try:
            # La deuxième colonne est en pages ; une page fait 4 Kio.
            mega = int(champs[1]) * 4 / 1024
        except ValueError:
            continue
        trouves.append((nom, int(entree), mega))

    trouves.sort(key=lambda t: t[2], reverse=True)
    return trouves[:combien]


def demarrage_le_plus_cher(combien=6):
    """Ce qui a le plus retardé le démarrage, si systemd sait le dire."""
    sortie = _commande(["systemd-analyze", "blame", "--no-pager"], delai=8)
    lignes = []
    for ligne in sortie.splitlines()[:combien]:
        morceaux = ligne.split(None, 1)
        if len(morceaux) == 2:
            lignes.append((morceaux[1].strip(), morceaux[0].strip()))
    return lignes


def bilan():
    """Tout d'un coup, rangé par classe — ce que grenPerf affiche."""
    services = services_en_marche()
    par_classe = {INDISPENSABLE: [], CONFORT: [], EN_TROP: [], INCONNU: []}
    for nom in services:
        classe, raison = avis(nom)
        par_classe[classe].append((nom, raison))
    return {
        "services": services,
        "par_classe": par_classe,
        "memoire": memoire(),
        "gourmands": gourmands(),
        "demarrage": demarrage_le_plus_cher(),
    }
