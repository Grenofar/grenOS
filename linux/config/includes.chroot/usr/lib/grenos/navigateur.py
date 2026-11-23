"""Le navigateur principal : celui qui ouvre TOUS les liens (23 novembre).

Grenofar : « qu'on puisse choisir le navigateur principal — si je veux me
connecter à Google, je choisis juste mon compte, celui que j'ai enregistré
dans mon navigateur principal ». Une connexion « avec Google » (Steam,
Heroic, une application, un lien) ouvre une page web : elle doit s'ouvrir là
où les comptes sont déjà enregistrés, pas dans un Firefox vierge.

Sur la machine de Grenofar, AUCUN navigateur n'était désigné : xdg-open
prenait au hasard de sa liste de repli. Le choix est écrit dans
~/.config/mimeapps.list (la norme freedesktop), que lisent xdg-open, GTK/GIO
et le portail des Flatpak — donc toutes les applications.
"""
import configparser
import os
import subprocess

# Ce qu'un navigateur principal ouvre : les liens web et les pages locales.
TYPES = ("x-scheme-handler/http", "x-scheme-handler/https", "x-scheme-handler/about",
         "x-scheme-handler/unknown", "text/html", "application/xhtml+xml")

MIMEAPPS = os.path.join(os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config"),
                        "mimeapps.list")
CHOIX = os.path.expanduser("~/.config/grenos/navigateur")


def _dossiers():
    """Où vivent les entrées d'applications, de la plus personnelle à la
    plus générale — Flatpak compris (Opera, Brave, Chrome… y sont souvent)."""
    maison = os.environ.get("XDG_DATA_HOME") or os.path.expanduser("~/.local/share")
    systeme = (os.environ.get("XDG_DATA_DIRS") or "/usr/local/share:/usr/share").split(":")
    dossiers = [maison, os.path.join(maison, "flatpak/exports/share"),
                "/var/lib/flatpak/exports/share", *systeme]
    vus, rendus = set(), []
    for d in dossiers:
        d = os.path.join(d, "applications")
        if d not in vus and os.path.isdir(d):
            vus.add(d)
            rendus.append(d)
    return rendus


def _entree(chemin):
    """Les champs de [Desktop Entry], ou {} si illisible."""
    champs, dedans = {}, False
    try:
        with open(chemin, encoding="utf-8", errors="replace") as f:
            for ligne in f:
                ligne = ligne.strip()
                if ligne.startswith("["):
                    dedans = ligne == "[Desktop Entry]"
                    continue
                if dedans and "=" in ligne:
                    cle, valeur = ligne.split("=", 1)
                    champs.setdefault(cle.strip(), valeur.strip())
    except OSError:
        return {}
    return champs


def est_navigateur(champs):
    """Un navigateur web : il se déclare comme tel ET sait ouvrir https."""
    if champs.get("Type", "Application") != "Application":
        return False
    if champs.get("NoDisplay", "").lower() == "true" or champs.get("Hidden", "").lower() == "true":
        return False
    categories = champs.get("Categories", "").split(";")
    types = champs.get("MimeType", "").split(";")
    return "WebBrowser" in categories and "x-scheme-handler/https" in types


def liste():
    """[{'id', 'nom', 'actuel'}] — un navigateur par identifiant, le premier
    trouvé l'emporte (celui du compte masque celui du système)."""
    actuel_id = actuel()
    trouves, vus = [], set()
    for dossier in _dossiers():
        try:
            noms = sorted(os.listdir(dossier))
        except OSError:
            continue
        for nom in noms:
            if not nom.endswith(".desktop") or nom in vus:
                continue
            vus.add(nom)
            champs = _entree(os.path.join(dossier, nom))
            if not est_navigateur(champs):
                continue
            affiche = champs.get("Name[fr]") or champs.get("Name") or nom[:-8]
            trouves.append({"id": nom, "nom": affiche, "actuel": nom == actuel_id})
    trouves.sort(key=lambda n: n["nom"].lower())
    return trouves


def _lire_mimeapps():
    c = configparser.ConfigParser(interpolation=None, strict=False, delimiters=("=",))
    c.optionxform = str  # les types MIME gardent leur casse
    try:
        c.read(MIMEAPPS, encoding="utf-8")
    except (configparser.Error, OSError):
        c = configparser.ConfigParser(interpolation=None, delimiters=("=",))
        c.optionxform = str
    return c


def actuel():
    """L'identifiant du navigateur principal (« com.opera.Opera.desktop »),
    ou "" si aucun n'est désigné."""
    c = _lire_mimeapps()
    valeur = c.get("Default Applications", "x-scheme-handler/https", fallback="")
    premier = valeur.split(";")[0].strip()
    if premier:
        return premier
    try:
        r = subprocess.run(["xdg-mime", "query", "default", "x-scheme-handler/https"],
                           capture_output=True, text=True, timeout=5)
        return r.stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


def choisir(identifiant):
    """Fait de ce navigateur celui qui ouvre tous les liens. Rendu : "" ou
    la phrase qui dit pourquoi non."""
    if not any(n["id"] == identifiant for n in liste()):
        return "Ce navigateur n'est pas installé sur cette machine."
    c = _lire_mimeapps()
    for section in ("Default Applications", "Added Associations"):
        if not c.has_section(section):
            c.add_section(section)
    for t in TYPES:
        c.set("Default Applications", t, identifiant)
        deja = [x for x in c.get("Added Associations", t, fallback="").split(";") if x and x != identifiant]
        c.set("Added Associations", t, ";".join([identifiant, *deja]) + ";")
    os.makedirs(os.path.dirname(MIMEAPPS), exist_ok=True)
    with open(MIMEAPPS + ".tmp", "w", encoding="utf-8") as f:
        c.write(f, space_around_delimiters=False)
    os.replace(MIMEAPPS + ".tmp", MIMEAPPS)
    os.makedirs(os.path.dirname(CHOIX), exist_ok=True)
    with open(CHOIX, "w", encoding="utf-8") as f:
        f.write(identifiant + "\n")
    return ""


def ouvrir(adresses):
    """Ouvre le navigateur principal (sur ces adresses, s'il y en a)."""
    identifiant = actuel()
    if identifiant and subprocess.run(["gtk-launch", identifiant[:-8] if identifiant.endswith(".desktop")
                                       else identifiant, *adresses]).returncode == 0:
        return 0
    for repli in ("x-www-browser", "firefox-esr"):
        try:
            return subprocess.run([repli, *adresses]).returncode
        except OSError:
            continue
    return 1


if __name__ == "__main__":
    import sys
    sys.exit(ouvrir(sys.argv[1:]))
