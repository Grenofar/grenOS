"""L'heure de la machine : son fuseau, et qui la règle.

Deux réglages, et rien de plus : le fuseau horaire, et le choix entre une
horloge mise à l'heure toute seule par le réseau ou réglée à la main. Tout
passe par `timedatectl`, qui est l'interface du système pour cela.

Changer l'heure demande les droits d'administration : `timedatectl` les
demande lui-même par polkit, et notre règle les accorde sans mot de passe tant
que la personne n'en a pas choisi un.
"""
import subprocess


def _dire(*arguments):
    try:
        return subprocess.run(["timedatectl", *arguments], capture_output=True,
                              text=True, timeout=15)
    except (OSError, subprocess.SubprocessError):
        return None


def etat():
    """Le fuseau actuel, et si l'heure se règle toute seule."""
    resultat = _dire("show", "--property=Timezone",
                     "--property=NTP", "--property=NTPSynchronized")
    valeurs = {}
    if resultat is not None and resultat.returncode == 0:
        for ligne in resultat.stdout.splitlines():
            if "=" in ligne:
                cle, valeur = ligne.split("=", 1)
                valeurs[cle] = valeur
    return {
        "fuseau": valeurs.get("Timezone", "Europe/Paris"),
        "automatique": valeurs.get("NTP", "no") == "yes",
        "synchronisee": valeurs.get("NTPSynchronized", "no") == "yes",
    }


def fuseaux():
    """Tous les fuseaux que le système connaît."""
    resultat = _dire("list-timezones")
    if resultat is None or resultat.returncode != 0:
        # Une liste courte vaut mieux qu'une liste vide : ce sont les fuseaux
        # des endroits d'où l'on parle français.
        return ["Europe/Paris", "Europe/Brussels", "Europe/Zurich",
                "Europe/Luxembourg", "America/Montreal", "Africa/Casablanca",
                "Africa/Dakar", "Indian/Reunion", "UTC"]
    return [f for f in resultat.stdout.splitlines() if f.strip()]


def choisir_fuseau(nom):
    """Change le fuseau. Rend une phrase expliquant le refus, ou rien."""
    resultat = _dire("set-timezone", nom)
    if resultat is None:
        return "L'outil de date du système n'a pas répondu."
    if resultat.returncode != 0:
        return resultat.stderr.strip() or "Le système a refusé ce fuseau."
    return ""


def horloge_automatique(actif):
    """Met l'heure à l'heure par le réseau, ou laisse la main."""
    resultat = _dire("set-ntp", "true" if actif else "false")
    if resultat is None:
        return "L'outil de date du système n'a pas répondu."
    if resultat.returncode != 0:
        return resultat.stderr.strip() or "Le système a refusé ce réglage."
    return ""


def regler(date_heure):
    """Règle l'heure à la main, au format « 2026-09-23 18:30:00 »."""
    resultat = _dire("set-time", date_heure)
    if resultat is None:
        return "L'outil de date du système n'a pas répondu."
    if resultat.returncode != 0:
        message = resultat.stderr.strip()
        if "NTP" in message or "ntp" in message:
            return ("Impossible tant que l'heure se règle toute seule : "
                    "désactive la mise à l'heure automatique d'abord.")
        return message or "Le système a refusé cette heure."
    return ""
