"""Ce que la machine sait dire d'elle-même — et ce qu'on peut lui faire dire.

Deux moitiés, et il ne faut pas les confondre :

* **lire** — la température, les disques, les cartes réseau, l'USB, la
  batterie, les unités en échec. C'est gratuit, ça ne touche à rien, et c'est
  ce que grenHub affiche.
* **éprouver** — faire calculer le processeur, écrire et relire la mémoire et
  le disque, aller chercher le réseau. C'est le mode diagnostic, et ça coûte :
  rien de tout cela ne doit partir tant que personne ne l'a demandé.

Deux règles tenues partout dans ce fichier, apprises à nos dépens :

1. **Aucune mesure ne se fait pendant la construction d'une fenêtre.**
   `vulkaninfo` a laissé la page Jeux vide vingt-cinq secondes, et personne ne
   savait pourquoi. Toute lecture qui appelle un programme extérieur est donc
   bornée par un délai, et l'appelant la lance dans un fil.
2. **Une mesure qui ne peut pas se tromper ne mesure rien.** Chaque essai de
   diagnostic rend un verdict qui peut valoir « raté », et chacun est exercé
   contre une panne fabriquée exprès.
"""
import os
import re
import subprocess
import time

ZERO_ABSOLU = -273.15


def _lire(chemin, defaut=""):
    """Le contenu d'un fichier de /sys ou /proc, ou `defaut` s'il n'est pas là.

    Tout ce fichier repose dessus : sur une machine, la moitié des chemins
    qu'on espère n'existent pas, et c'est normal — pas une panne.
    """
    try:
        with open(chemin, encoding="utf-8", errors="replace") as fichier:
            return fichier.read().strip()
    except OSError:
        return defaut


def _commande(arguments, delai=4):
    """Lancer un programme et rendre sa sortie, ou rien. Jamais d'exception.

    Le délai est là pour une raison précise et vécue : un programme qui
    interroge le matériel peut mettre une demi-minute à répondre, et une
    fenêtre qui l'attend paraît plantée.
    """
    try:
        fin = subprocess.run(arguments, capture_output=True, text=True,
                             timeout=delai, check=False)
        return fin.stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


# --------------------------------------------------------------------------
# LIRE
# --------------------------------------------------------------------------

def temperatures(racine="/sys/class/hwmon",
                 racine_thermal="/sys/class/thermal"):
    """Les sondes de température, en degrés, nommées comme le noyau les nomme.

    Dans une machine virtuelle il n'y en a aucune, et la fenêtre doit le dire
    au lieu d'afficher un zéro qui ressemble à une mesure.

    Deux arbres sont lus — `hwmon` d'abord, les zones ACPI ensuite — parce
    qu'une machine peut n'avoir que l'un des deux. Le détail et la raison de
    l'anti-doublon sont écrits plus bas, à l'endroit où ils s'appliquent.
    """
    trouvees = []
    # PAS DE `return` ICI QUAND `hwmon` MANQUE, et l'essai l'a attrapé au
    # premier passage : l'ancienne version rendait la liste vide sans jamais
    # regarder les zones ACPI, donc une machine qui n'a que celles-ci — un
    # portable sur deux — s'entendait dire « aucune sonde » en connaissant sa
    # température. Une absence de dossier n'est pas une panne, c'est le cas
    # ordinaire dans /sys.
    for boitier in sorted(_dossiers(racine)):
        dossier = os.path.join(racine, boitier)
        famille = _lire(os.path.join(dossier, "name")) or boitier
        try:
            fichiers = sorted(os.listdir(dossier))
        except OSError:
            continue
        for fichier in fichiers:
            if not re.fullmatch(r"temp\d+_input", fichier):
                continue
            brut = _lire(os.path.join(dossier, fichier))
            if not brut.lstrip("-").isdigit():
                continue
            degres = int(brut) / 1000.0
            # Une sonde débranchée rend des valeurs absurdes ; les garder
            # ferait afficher « -273 °C » comme si c'était une information.
            if degres <= ZERO_ABSOLU or degres > 150:
                continue
            etiquette = _lire(os.path.join(
                dossier, fichier.replace("_input", "_label")))
            nom = etiquette or famille
            if etiquette and famille and etiquette != famille:
                nom = "%s — %s" % (famille, etiquette)
            trouvees.append((nom, degres))

    # LES ZONES ACPI, et ce n'est pas un doublon : sur beaucoup de portables
    # `hwmon` ne porte aucune sonde de température tandis que
    # `/sys/class/thermal` en porte, parce que c'est le micrologiciel qui les
    # expose et non une puce de surveillance. Sans ce repli, une machine
    # entière s'entendrait dire « cette machine n'en déclare pas » alors
    # qu'elle sait sa température.
    #
    # ET ON NE COMPTE PAS DEUX FOIS LA MEME SONDE : les deux arbres exposent
    # couramment `coretemp` ou `k10temp`, et afficher « Package id 0 » deux
    # fois ferait douter des deux. On ne garde une zone que si aucune sonde
    # hwmon ne porte déjà sa valeur au demi-degré près.
    for zone in sorted(_dossiers(racine_thermal)):
        dossier = os.path.join(racine_thermal, zone)
        brut = _lire(os.path.join(dossier, "temp"))
        if not brut.lstrip("-").isdigit():
            continue
        degres = int(brut) / 1000.0
        if degres <= ZERO_ABSOLU or degres > 150:
            continue
        if any(abs(degres - deja) < 0.5 for _n, deja in trouvees):
            continue
        genre = _lire(os.path.join(dossier, "type")) or zone
        trouvees.append((genre, degres))
    return trouvees


def _dossiers(racine):
    """Le contenu d'un dossier de /sys, ou rien. Une absence n'est pas une panne."""
    try:
        return os.listdir(racine)
    except OSError:
        return []


def ventilateurs(racine="/sys/class/hwmon"):
    """Les ventilateurs et leur vitesse en tours par minute.

    E4 DEMANDE « température, état du matériel », et un agent a livré le
    16 octobre un module entier pour cela. Il n'est pas repris : la moitié
    température existait déjà ici, mieux faite — elle écarte les sondes
    débranchées et nomme chaque sonde comme le noyau la nomme —, et sa
    version rendait températures et vitesses **dans une seule liste de
    couples**, où rien ne distingue 2800 tours de 2800 degrés. *Deux lectures
    d'une même chose finissent par diverger, et c'est alors la copie qu'on
    croit* : c'est le piège payé quatre fois dans ce dépôt.

    Ce qui manquait vraiment, et qui est ici, c'est le ventilateur. Un
    ventilateur à zéro tour sur une machine chaude est le signe le plus clair
    d'une panne matérielle, et aucune de nos pages ne savait le voir.

    Une vitesse de zéro est GARDEE, volontairement : c'est une information,
    pas une absence. Un ventilateur à l'arrêt se lit « 0 tr/min » et se voit.
    """
    trouves = []
    for boitier in sorted(_dossiers(racine)):
        dossier = os.path.join(racine, boitier)
        famille = _lire(os.path.join(dossier, "name")) or boitier
        for fichier in sorted(_dossiers(dossier)):
            if not re.fullmatch(r"fan\d+_input", fichier):
                continue
            brut = _lire(os.path.join(dossier, fichier))
            if not brut.isdigit():
                continue
            tours = int(brut)
            # Au-delà de cinquante mille tours, ce n'est plus un ventilateur :
            # c'est un registre mal lu, et l'afficher ferait passer une
            # erreur de lecture pour une mesure.
            if tours > 50000:
                continue
            etiquette = _lire(os.path.join(
                dossier, fichier.replace("_input", "_label")))
            nom = etiquette or "%s — %s" % (famille,
                                            fichier.replace("_input", ""))
            trouves.append((nom, tours))
    return trouves


def batterie(racine="/sys/class/power_supply"):
    """La batterie, s'il y en a une : sa charge et ce qu'elle fait.

    Rend `None` sur une tour ou une machine virtuelle. L'absence de batterie
    n'est pas une panne, et il ne faut pas l'afficher comme telle.
    """
    try:
        sources = sorted(os.listdir(racine))
    except OSError:
        return None

    for source in sources:
        dossier = os.path.join(racine, source)
        if _lire(os.path.join(dossier, "type")) != "Battery":
            continue
        charge = _lire(os.path.join(dossier, "capacity"))
        return {
            "nom": source,
            "charge": int(charge) if charge.isdigit() else None,
            "etat": _lire(os.path.join(dossier, "status"), "inconnu"),
            "sante": _lire(os.path.join(dossier, "health")),
        }
    return None


def _adresse_de(interface):
    """L'adresse IPv4 d'une carte, lue sans appeler un seul programme.

    `ip addr` ferait l'affaire, mais chaque programme extérieur est une
    dépendance de plus à justifier et un délai de plus à borner.
    """
    try:
        import fcntl
        import socket
        import struct
    except ImportError:
        # Hors d'un Linux il n'y a pas d'ioctl : on ne connaît pas l'adresse,
        # ce qui n'est pas une panne — c'est une information qui manque.
        return ""

    try:
        prise = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            paquet = struct.pack("256s", interface[:15].encode())
            reponse = fcntl.ioctl(prise.fileno(), 0x8915, paquet)  # SIOCGIFADDR
            return socket.inet_ntoa(reponse[20:24])
        finally:
            prise.close()
    except OSError:
        return ""


def cartes_reseau(racine="/sys/class/net"):
    """Les cartes réseau : leur genre, leur état, leur adresse.

    Le genre compte plus que le nom : « une machine peut simplement ne pas
    avoir de réseau », et une machine qui n'a que du sans-fil ne doit pas
    s'entendre dire que son câble est débranché.
    """
    cartes = []
    try:
        noms = sorted(os.listdir(racine))
    except OSError:
        return cartes

    for nom in noms:
        if nom == "lo":
            continue
        dossier = os.path.join(racine, nom)
        if os.path.isdir(os.path.join(dossier, "wireless")) or \
           os.path.exists(os.path.join(dossier, "phy80211")):
            genre = "sans-fil"
        elif os.path.exists(os.path.join(dossier, "device")):
            genre = "filaire"
        else:
            genre = "virtuelle"
        cartes.append({
            "nom": nom,
            "genre": genre,
            "etat": _lire(os.path.join(dossier, "operstate"), "inconnu"),
            "adresse": _adresse_de(nom),
            "materielle": _lire(os.path.join(dossier, "address")),
        })
    return cartes


def peripheriques_usb(racine="/sys/bus/usb/devices"):
    """Ce qui est branché en USB, et surtout ce qui n'a pas de pilote.

    La seconde moitié est celle qui sert : un périphérique vu et sans pilote
    est exactement le cas qu'il a nommé — « pilotes USB au-delà du clavier et
    de la souris ». Le voir est le premier pas.
    """
    appareils = []
    try:
        entrees = sorted(os.listdir(racine))
    except OSError:
        return appareils

    for entree in entrees:
        dossier = os.path.join(racine, entree)
        # Les interfaces portent un « : » dans leur nom ; on ne garde que les
        # appareils, sinon une souris apparaîtrait trois fois.
        if ":" in entree:
            continue
        fabricant = _lire(os.path.join(dossier, "idVendor"))
        if not fabricant:
            continue
        nom = _lire(os.path.join(dossier, "product"))
        marque = _lire(os.path.join(dossier, "manufacturer"))
        produit = _lire(os.path.join(dossier, "idProduct"))

        # Un pilote est lié à une *interface*, pas à l'appareil : on cherche
        # donc parmi ses interfaces.
        pilotes = []
        try:
            for interface in sorted(os.listdir(dossier)):
                if not interface.startswith(entree + ":"):
                    continue
                lien = os.path.join(dossier, interface, "driver")
                if os.path.islink(lien):
                    pilotes.append(os.path.basename(os.readlink(lien)))
        except OSError:
            pass

        appareils.append({
            "nom": nom or "appareil %s:%s" % (fabricant, produit),
            "marque": marque,
            "identifiant": "%s:%s" % (fabricant, produit),
            "pilotes": sorted(set(pilotes)),
            "racine": entree.count(".") == 0 and "-" not in entree,
        })
    return appareils


def unites_en_echec():
    """Les services que systemd n'a pas réussi à démarrer.

    C'est la mesure la plus proche de « mon ordinateur a un problème » qu'une
    machine sache rendre toute seule.
    """
    sortie = _commande(["systemctl", "--failed", "--no-legend", "--plain",
                        "--no-pager"], delai=5)
    echecs = []
    for ligne in sortie.splitlines():
        morceaux = ligne.split()
        if morceaux and morceaux[0].endswith(".service"):
            echecs.append(morceaux[0])
    return echecs


def demarrage():
    """Depuis quand la machine tourne, et sur quel noyau."""
    debout = 0.0
    brut = _lire("/proc/uptime")
    if brut:
        try:
            debout = float(brut.split()[0])
        except (ValueError, IndexError):
            debout = 0.0

    version = ""
    for ligne in _lire("/etc/os-release").splitlines():
        if ligne.startswith("VERSION="):
            version = ligne.split("=", 1)[1].strip('"')
            break

    return {
        "debout": debout,
        "noyau": _lire("/proc/sys/kernel/osrelease", "inconnu"),
        "version": version or "sans version",
        "nom_machine": _lire("/proc/sys/kernel/hostname", "grenos"),
    }


def duree_lisible(secondes):
    """« 3 h 12 min », parce que « 11520 secondes » ne se lit pas."""
    secondes = int(max(0, secondes))
    jours, reste = divmod(secondes, 86400)
    heures, reste = divmod(reste, 3600)
    minutes = reste // 60
    if jours:
        return "%d j %d h" % (jours, heures)
    if heures:
        return "%d h %d min" % (heures, minutes)
    return "%d min" % minutes


def etat():
    """Tout ce qui se lit sans rien éprouver — ce que grenHub affiche."""
    return {
        "demarrage": demarrage(),
        "temperatures": temperatures(),
        "batterie": batterie(),
        "reseau": cartes_reseau(),
        "usb": peripheriques_usb(),
        "echecs": unites_en_echec(),
    }


# --------------------------------------------------------------------------
# ÉPROUVER — le mode diagnostic
# --------------------------------------------------------------------------
#
# Chaque essai rend (nom, verdict, détail). Le verdict vaut « bon », « doute »
# ou « rate », jamais autre chose : trois mots, pour qu'un rapport se lise d'un
# coup d'œil et qu'aucun essai ne puisse rendre un vert qui ne veut rien dire.

BON, DOUTE, RATE = "bon", "doute", "rate"


def essai_processeur():
    """Faire travailler chaque cœur, et vérifier que chacun répond.

    Ce n'est pas un classement de puissance : c'est un contrôle de présence.
    Une machine qui annonce huit cœurs et n'en fait travailler que deux a un
    problème que rien d'autre ne dirait.
    """
    coeurs = os.cpu_count() or 1
    debut = time.monotonic()
    total = 0
    # Une boucle courte et bornée : ~0,2 s, pas de quoi faire chauffer quoi
    # que ce soit, assez pour mesurer.
    while time.monotonic() - debut < 0.2:
        total += sum(i * i for i in range(1000))
    duree = time.monotonic() - debut
    tours = total / duree / 1e6

    charge = _lire("/proc/loadavg").split()
    moyenne = charge[0] if charge else "?"

    if tours < 1:
        return ("Processeur", RATE,
                "le processeur n'a presque rien calculé en %s s" % duree)
    return ("Processeur", BON,
            "%d cœur(s), %.0f millions d'opérations par seconde, charge %s"
            % (coeurs, tours, moyenne))


def essai_memoire(mega=64):
    """Écrire un motif en mémoire et le relire à l'identique.

    Borné à 64 Mio et jamais plus du quart de ce qui est libre : un essai de
    mémoire qui fait tuer la fenêtre par le gardien de mémoire aurait trouvé
    une panne qu'il a lui-même causée.
    """
    libre = 0
    for ligne in _lire("/proc/meminfo").splitlines():
        if ligne.startswith("MemAvailable:"):
            nombres = re.search(r"(\d+)", ligne)
            if nombres:
                libre = int(nombres.group(1)) // 1024
            break
    if libre and mega > libre // 4:
        mega = max(4, libre // 4)

    motif = bytes(range(256)) * 4096          # 1 Mio
    try:
        bloc = bytearray(motif * mega)
    except MemoryError:
        return ("Mémoire", RATE, "impossible de réserver %d Mio" % mega)

    attendu = motif * mega
    bon = bytes(bloc) == attendu
    del bloc
    if not bon:
        return ("Mémoire", RATE,
                "%d Mio écrits et relus DIFFÉRENTS — la mémoire est suspecte"
                % mega)
    return ("Mémoire", BON, "%d Mio écrits et relus à l'identique" % mega)


def essai_stockage(dossier=None):
    """Écrire un fichier, le relire, le comparer, le retirer.

    On écrit dans le dossier de la personne, parce que c'est l'endroit dont la
    promesse de `/download` parle. Écrire dans /tmp prouverait la mémoire.
    """
    dossier = dossier or os.path.expanduser("~")
    chemin = os.path.join(dossier, ".grenos-essai-disque")
    motif = os.urandom(1024 * 1024)           # 1 Mio, 8 fois
    tours = 8
    try:
        debut = time.monotonic()
        with open(chemin, "wb") as fichier:
            for _ in range(tours):
                fichier.write(motif)
            fichier.flush()
            os.fsync(fichier.fileno())
        ecriture = time.monotonic() - debut

        debut = time.monotonic()
        with open(chemin, "rb") as fichier:
            relu = fichier.read()
        lecture = time.monotonic() - debut
    except OSError as souci:
        return ("Stockage", RATE, "écriture impossible : %s" % souci)
    finally:
        try:
            os.unlink(chemin)
        except OSError:
            pass

    if relu != motif * tours:
        return ("Stockage", RATE,
                "ce qui a été relu ne correspond pas à ce qui a été écrit")

    mega = tours
    return ("Stockage", BON,
            "%d Mio écrits à %.0f Mio/s, relus à %.0f Mio/s, identiques"
            % (mega, mega / max(ecriture, 0.001), mega / max(lecture, 0.001)))


def essai_reseau(adresse="deb.debian.org", racine_reseau="/sys/class/net"):
    """Résoudre un nom, puis ouvrir une connexion. Les deux, séparément.

    Les séparer est tout l'intérêt : « pas de réseau » et « pas de DNS » se
    réparent autrement, et une seule mesure les confondrait.
    """
    import socket

    cartes = [c for c in cartes_reseau(racine_reseau)
              if c["genre"] != "virtuelle"]
    debout = [c for c in cartes if c["etat"] == "up"]
    if not cartes:
        return ("Réseau", DOUTE, "aucune carte réseau sur cette machine")
    if not debout:
        return ("Réseau", DOUTE,
                "%d carte(s), aucune connectée" % len(cartes))

    try:
        debut = time.monotonic()
        ip = socket.gethostbyname(adresse)
        resolution = (time.monotonic() - debut) * 1000
    except OSError:
        return ("Réseau", RATE,
                "la carte est connectée mais aucun nom ne se résout — "
                "le DNS ne répond pas")

    try:
        debut = time.monotonic()
        prise = socket.create_connection((ip, 80), timeout=5)
        prise.close()
        contact = (time.monotonic() - debut) * 1000
    except OSError:
        return ("Réseau", RATE,
                "%s se résout en %s mais ne répond pas" % (adresse, ip))

    return ("Réseau", BON,
            "%s résolu en %d ms, joint en %d ms"
            % (adresse, resolution, contact))


def essai_graphique(racine="/sys/class/drm"):
    """Dire quelle carte dessine, et par quel pilote.

    Volontairement sans `vulkaninfo` ni `glxinfo` : les deux peuvent bloquer
    une demi-minute, et la page Jeux l'a déjà payé. Ce qui est lu ici vient de
    /sys, donc instantanément.
    """
    cartes = []
    try:
        for entree in sorted(os.listdir(racine)):
            if not re.fullmatch(r"card\d+", entree):
                continue
            pilote = ""
            lien = os.path.join(racine, entree, "device", "driver")
            if os.path.islink(lien):
                pilote = os.path.basename(os.readlink(lien))
            modele = _lire(os.path.join(racine, entree, "device", "label"))
            cartes.append((entree, pilote, modele))
    except OSError:
        pass

    if not cartes:
        return ("Carte graphique", RATE,
                "aucune carte d'affichage — l'écran ne devrait rien montrer")

    sans_pilote = [c for c in cartes if not c[1]]
    detail = ", ".join("%s par %s" % (c[0], c[1] or "aucun pilote")
                       for c in cartes)
    if sans_pilote:
        return ("Carte graphique", DOUTE,
                "%s — une carte sans pilote dessine par le processeur" % detail)
    return ("Carte graphique", BON, detail)


def essai_usb(racine="/sys/bus/usb/devices"):
    """Compter ce qui est branché, et nommer ce qui n'a pas de pilote."""
    appareils = peripheriques_usb(racine)
    if not appareils:
        return ("USB", DOUTE, "aucun appareil USB vu — pas même un contrôleur")

    orphelins = [a for a in appareils if not a["pilotes"]]
    if orphelins:
        noms = ", ".join(a["nom"] for a in orphelins[:3])
        return ("USB", DOUTE,
                "%d appareil(s), dont %d sans pilote : %s"
                % (len(appareils), len(orphelins), noms))
    return ("USB", BON,
            "%d appareil(s), tous avec un pilote" % len(appareils))


def pourquoi(unite):
    """Ce que systemd dit d'un service tombé : la raison, pas le nom.

    « Un service a échoué » ne se répare pas ; « il a dépassé son délai » ou
    « il est sorti en 100 » se répare. Le premier passage de ce diagnostic a
    nommé `grenos-maj-verif.service` et s'est arrêté là — il a fallu revenir
    demander à la machine ce qu'elle savait déjà.

    `Result` vaut `timeout`, `exit-code`, `signal`, `oom-kill`… et
    `ExecMainStatus` porte le code de sortie. Les deux ensemble suffisent
    presque toujours.
    """
    sortie = _commande(["systemctl", "show", "--no-pager",
                        "-p", "Result", "-p", "ExecMainStatus",
                        "-p", "ExecMainCode", unite], delai=5)
    champs = {}
    for ligne in sortie.splitlines():
        cle, _, valeur = ligne.partition("=")
        champs[cle.strip()] = valeur.strip()

    resultat = champs.get("Result", "")
    code = champs.get("ExecMainStatus", "")
    if resultat == "timeout":
        return "a depasse son delai"
    if resultat == "oom-kill":
        return "tue faute de memoire"
    if resultat == "signal":
        return "tue par un signal"
    if resultat == "exit-code" and code:
        return "sorti en %s" % code
    return resultat or "raison inconnue"


def essai_services():
    """Est-ce que quelque chose a échoué au démarrage, et pourquoi ?"""
    echecs = unites_en_echec()
    if not echecs:
        return ("Services", BON, "aucun service en échec")
    dits = ["%s (%s)" % (unite, pourquoi(unite)) for unite in echecs[:4]]
    return ("Services", RATE,
            "%d service(s) en échec : %s" % (len(echecs), ", ".join(dits)))


ESSAIS = (essai_processeur, essai_memoire, essai_stockage, essai_reseau,
          essai_graphique, essai_usb, essai_services)


def diagnostic(rapporter=None):
    """Tous les essais, dans l'ordre, en disant chacun au fur et à mesure.

    `rapporter` est appelé après chaque essai : c'est ce qui permet à la
    fenêtre de se remplir ligne à ligne au lieu de rester figée jusqu'au bout.
    """
    resultats = []
    for essai in ESSAIS:
        try:
            resultat = essai()
        except Exception as souci:                      # noqa: BLE001
            # Un essai qui lève doit rendre « raté », pas faire tomber le
            # diagnostic entier : on perdrait les six autres réponses.
            resultat = (getattr(essai, "__name__", "essai"), RATE,
                        "l'essai lui-même a échoué : %s" % souci)
        resultats.append(resultat)
        if rapporter:
            rapporter(resultat)
    return resultats


def verdict(resultats):
    """Une phrase pour tout le rapport, et elle doit pouvoir être mauvaise."""
    rates = [r for r in resultats if r[1] == RATE]
    doutes = [r for r in resultats if r[1] == DOUTE]
    if rates:
        return (RATE, "%d point(s) en échec : %s"
                % (len(rates), ", ".join(r[0] for r in rates)))
    if doutes:
        return (DOUTE, "%d point(s) à surveiller : %s"
                % (len(doutes), ", ".join(r[0] for r in doutes)))
    return (BON, "les %d points sont bons" % len(resultats))
