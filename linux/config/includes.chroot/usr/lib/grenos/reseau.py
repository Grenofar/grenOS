"""Le réseau et le Bluetooth : voir ce qui existe, s'y connecter, en sortir.

Tout passe par `nmcli` et `bluetoothctl`, les interfaces que le système expose
déjà. Rien n'est réinventé : ce module traduit leurs réponses en quelque chose
qu'une fenêtre peut afficher, et rend les erreurs telles quelles plutôt que de
les masquer — « le mot de passe est refusé » est une information utile.
"""
import os
import re
import subprocess


def _nmcli(*arguments, delai=20):
    try:
        return subprocess.run(["nmcli", *arguments], capture_output=True,
                              text=True, timeout=delai)
    except (OSError, subprocess.SubprocessError):
        return None


# ---- Le Wi-Fi ---------------------------------------------------------------
def wifi_disponible():
    resultat = _nmcli("-t", "-f", "TYPE", "device")
    return resultat is not None and "wifi" in (resultat.stdout or "")


def wifi_allume():
    resultat = _nmcli("radio", "wifi")
    return resultat is not None and "enabled" in (resultat.stdout or "")


def allumer_wifi(actif):
    _nmcli("radio", "wifi", "on" if actif else "off")


def reseaux():
    """Les réseaux Wi-Fi vus, du plus fort au plus faible.

    Rendu : [{'nom', 'force', 'protege', 'connecte'}]
    """
    resultat = _nmcli("-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list",
                      delai=25)
    if resultat is None or resultat.returncode != 0:
        return []
    vus, trouves = set(), []
    for ligne in resultat.stdout.splitlines():
        # nmcli sépare par « : » et échappe les « : » des valeurs par « \: ».
        champs = re.split(r"(?<!\\):", ligne)
        if len(champs) < 4:
            continue
        en_cours, nom, force, securite = champs[0], champs[1].replace("\\:", ":"), champs[2], champs[3]
        if not nom or nom in vus:
            continue
        vus.add(nom)
        trouves.append({
            "nom": nom,
            "force": int(force) if force.isdigit() else 0,
            "protege": bool(securite.strip()) and securite.strip() != "--",
            "connecte": en_cours.strip() == "*",
        })
    return sorted(trouves, key=lambda r: (-r["connecte"], -r["force"]))


def connu(nom):
    """Ce réseau a-t-il déjà un mot de passe enregistré ?"""
    resultat = _nmcli("-t", "-f", "NAME", "connection", "show")
    if resultat is None:
        return False
    return nom in resultat.stdout.splitlines()


def connecter(nom, mot_de_passe=""):
    """Se connecte. Rend une phrase expliquant le refus, ou une chaîne vide."""
    if mot_de_passe:
        resultat = _nmcli("device", "wifi", "connect", nom, "password", mot_de_passe, delai=60)
    elif connu(nom):
        resultat = _nmcli("connection", "up", nom, delai=60)
    else:
        resultat = _nmcli("device", "wifi", "connect", nom, delai=60)
    if resultat is None:
        return "Le gestionnaire de réseau n'a pas répondu."
    if resultat.returncode != 0:
        message = (resultat.stderr or "").strip()
        if "Secrets were required" in message or "no secrets" in message:
            return "Ce réseau demande un mot de passe."
        if "invalid" in message.lower() or "802-11-wireless-security" in message:
            return "Mot de passe refusé."
        return message or "La connexion a échoué."
    return ""


def deconnecter(nom):
    resultat = _nmcli("connection", "down", nom, delai=30)
    if resultat is None or resultat.returncode != 0:
        return "La déconnexion a échoué."
    return ""


def etat():
    """Où en est la machine : connectée, et à quoi."""
    resultat = _nmcli("-t", "-f", "TYPE,STATE,CONNECTION", "device")
    if resultat is None:
        return {"connecte": False, "par": "", "nom": ""}
    for ligne in resultat.stdout.splitlines():
        champs = ligne.split(":")
        if len(champs) >= 3 and champs[1] == "connected" and champs[0] in ("wifi", "ethernet"):
            return {"connecte": True,
                    "par": "Wi-Fi" if champs[0] == "wifi" else "câble",
                    "nom": champs[2]}
    return {"connecte": False, "par": "", "nom": ""}


# ---- Ce que la barre affiche ------------------------------------------------
#
# Tout ce qui precede passe par `nmcli`, et c'est bien pour une fenetre qu'on
# ouvre. Pas pour la barre : elle se relit toutes les trois secondes dans la
# boucle GTK, et un `nmcli` qui met deux secondes a repondre fige la barre
# pendant deux secondes. Une barre qui saccade est exactement le reproche qui a
# fait retirer Plasma.
#
# Ce qui suit ne lance donc aucun programme : le noyau publie deja tout dans
# /sys et /proc, et le lire coute quelques microsecondes.

CARTES = "/sys/class/net"


def _lignes(chemin):
    try:
        with open(chemin, encoding="utf-8", errors="replace") as fichier:
            return fichier.read()
    except OSError:
        return ""


def _force_wifi(interface):
    """La qualite du lien Wi-Fi, de 0 a 3 barres.

    /proc/net/wireless donne « link », que le pilote exprime le plus souvent
    sur 70. On ne divise pas par un maximum suppose : on compare a des seuils,
    parce qu'un pilote qui rendrait /100 donnerait alors plus de barres, jamais
    moins — se tromper vers l'optimisme est moins grave que d'afficher « pas de
    signal » a quelqu'un qui navigue.
    """
    for ligne in _lignes("/proc/net/wireless").splitlines():
        if not ligne.strip().startswith(interface + ":"):
            continue
        champs = ligne.split(":", 1)[1].split()
        if len(champs) < 2:
            break
        try:
            lien = float(champs[1].rstrip("."))
        except ValueError:
            break
        if lien >= 55:
            return 3
        if lien >= 35:
            return 2
        if lien >= 12:
            return 1
        return 0
    # Pas de ligne : connecte, mais le pilote ne dit pas la force. Plein
    # signal est le moindre mal — l'inverse ferait clignoter « aucun reseau »
    # sur une machine qui marche.
    return 3


def etat_barre():
    """Par quoi la machine est reliee, et a quelle force.

    Rendu : {'lien': 'cable' | 'wifi' | 'aucun', 'barres': 0-3, 'interface'}
    """
    cable, sans_fil = None, None
    try:
        interfaces = sorted(os.listdir(CARTES))
    except OSError:
        return {"lien": "aucun", "barres": 0, "interface": ""}

    for interface in interfaces:
        if interface == "lo":
            continue
        # « up » ne suffit pas : une carte peut etre allumee sans rien au bout.
        # `carrier` dit s'il y a vraiment un lien physique.
        if _lignes(f"{CARTES}/{interface}/operstate").strip() != "up":
            continue
        if _lignes(f"{CARTES}/{interface}/carrier").strip() != "1":
            continue
        if os.path.isdir(f"{CARTES}/{interface}/wireless"):
            sans_fil = sans_fil or interface
        else:
            cable = cable or interface

    # Le cable l'emporte : quand les deux sont branches, c'est lui qui porte
    # le trafic, et c'est lui qu'il faut montrer.
    if cable:
        return {"lien": "cable", "barres": 3, "interface": cable}
    if sans_fil:
        return {"lien": "wifi", "barres": _force_wifi(sans_fil),
                "interface": sans_fil}
    return {"lien": "aucun", "barres": 0, "interface": ""}


# Ce que dit l'infobulle, par nombre de barres. Écrit ici plutôt que dans une
# expression conditionnelle : la version d'avant tenait sur une seule ligne de
# cent trente caractères, et deux de ces phrases avaient perdu leurs accents
# sans que personne ne les relise.
FORCE = {
    0: "connecté, signal très faible",
    1: "signal faible",
    2: "signal moyen",
    3: "bon signal",
}


def icone_barre(etat_lu=None):
    """Le nom de l'icône qui dit cet état-là, et la phrase de l'infobulle."""
    etat_lu = etat_lu or etat_barre()
    interface = etat_lu["interface"]

    if etat_lu["lien"] == "cable":
        return "grenos-reseau-cable", f"Connecté par câble ({interface})"

    if etat_lu["lien"] == "wifi":
        barres = etat_lu["barres"]
        phrase = FORCE.get(barres, FORCE[3])
        return f"grenos-reseau-wifi-{barres}", f"Wi-Fi : {phrase} ({interface})"

    return "grenos-reseau-aucun", "Aucune connexion"


# ---- Le Bluetooth -----------------------------------------------------------
def _bluetoothctl(*arguments, delai=20):
    try:
        return subprocess.run(["bluetoothctl", *arguments], capture_output=True,
                              text=True, timeout=delai)
    except (OSError, subprocess.SubprocessError):
        return None


def bluetooth_disponible():
    resultat = _bluetoothctl("list")
    return resultat is not None and resultat.returncode == 0 and bool(resultat.stdout.strip())


def bluetooth_allume():
    resultat = _bluetoothctl("show")
    if resultat is None:
        return False
    return "Powered: yes" in resultat.stdout


def allumer_bluetooth(actif):
    _bluetoothctl("power", "on" if actif else "off")


def chercher_appareils(secondes=6):
    """Lance une recherche d'appareils pendant quelques secondes."""
    try:
        subprocess.run(["bluetoothctl", "--timeout", str(secondes), "scan", "on"],
                       capture_output=True, text=True, timeout=secondes + 10)
    except (OSError, subprocess.SubprocessError):
        pass


def appareils():
    """Les appareils Bluetooth connus ou visibles."""
    trouves = []
    resultat = _bluetoothctl("devices")
    if resultat is None or resultat.returncode != 0:
        return trouves
    connectes = set()
    lies = _bluetoothctl("devices", "Connected")
    if lies is not None and lies.returncode == 0:
        for ligne in lies.stdout.splitlines():
            champs = ligne.split(None, 2)
            if len(champs) >= 2:
                connectes.add(champs[1])
    for ligne in resultat.stdout.splitlines():
        champs = ligne.split(None, 2)
        if len(champs) < 3 or champs[0] != "Device":
            continue
        trouves.append({"adresse": champs[1], "nom": champs[2],
                        "connecte": champs[1] in connectes})
    return sorted(trouves, key=lambda a: (-a["connecte"], a["nom"].lower()))


def lier(adresse):
    """Associe puis connecte un appareil. Rend l'erreur, ou une chaîne vide."""
    _bluetoothctl("trust", adresse)
    appairage = _bluetoothctl("pair", adresse, delai=40)
    connexion = _bluetoothctl("connect", adresse, delai=40)
    if connexion is not None and "Connection successful" in (connexion.stdout or ""):
        return ""
    if appairage is not None and "Failed to pair" in (appairage.stdout or ""):
        return "L'association a échoué. Mets l'appareil en mode association."
    return "La connexion a échoué."


def delier(adresse):
    resultat = _bluetoothctl("disconnect", adresse, delai=30)
    if resultat is None:
        return "Le Bluetooth n'a pas répondu."
    return ""
