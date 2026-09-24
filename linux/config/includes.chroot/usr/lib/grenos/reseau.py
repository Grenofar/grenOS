"""Le réseau et le Bluetooth : voir ce qui existe, s'y connecter, en sortir.

Tout passe par `nmcli` et `bluetoothctl`, les interfaces que le système expose
déjà. Rien n'est réinventé : ce module traduit leurs réponses en quelque chose
qu'une fenêtre peut afficher, et rend les erreurs telles quelles plutôt que de
les masquer — « le mot de passe est refusé » est une information utile.
"""
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
