"""Les partitions : ce que les disques contiennent, et ce qu'on peut en faire
(23 novembre).

Grenofar : « fais un système de réglage des partitions ». Réglages ›
Partitions montre chaque disque comme une barre (ses partitions et l'espace
libre, à l'échelle), et permet sans connaître Linux : ouvrir, monter,
démonter, renommer, formater, supprimer, créer dans l'espace libre.

Ce module LIT seulement, sans droits : `lsblk` et /sys. Monter et démonter
passent par udisks, au nom de la personne. Tout ce qui ÉCRIT sur un disque
passe par `grenos-partitions`, en administrateur, qui revérifie tout — la
page n'est jamais crue sur parole.
"""
import json
import os
import subprocess

# Ce qui fait tourner le système : on n'y touche jamais depuis la page.
POINTS_SYSTEME = ("/", "/boot", "/boot/efi", "/efi", "/home", "/usr", "/var",
                  "/run/live/medium", "/run/live/persistence", "[SWAP]")
# Un trou plus petit n'est que de l'alignement (1 Mio au début d'un disque).
LIBRE_MIN = 16 * 1024 * 1024
COLONNES = "NAME,PATH,TYPE,SIZE,FSTYPE,FSVER,LABEL,MOUNTPOINTS,FSUSED,FSAVAIL,MODEL,TRAN,RM,HOTPLUG,PTTYPE,PARTTYPENAME,RO"


def _lsblk():
    try:
        r = subprocess.run(["lsblk", "-J", "-b", "-o", COLONNES], capture_output=True,
                           text=True, timeout=10)
        return json.loads(r.stdout or "{}").get("blockdevices", [])
    except (OSError, subprocess.SubprocessError, ValueError):
        return []


def _sys(nom, champ):
    try:
        with open("/sys/class/block/%s/%s" % (nom, champ), encoding="ascii") as f:
            return int(f.read().strip())
    except (OSError, ValueError):
        return None


def _entier(v):
    try:
        return int(v)
    except (TypeError, ValueError):
        return None


def _points(p):
    return [m for m in (p.get("mountpoints") or []) if m]


def est_systeme(p):
    """Cette partition (ou ce qu'elle porte : LUKS, LVM) fait-elle tourner
    le système ?"""
    if any(m in POINTS_SYSTEME for m in _points(p)):
        return True
    return any(est_systeme(e) for e in p.get("children") or [])


def libres(disque, parties):
    """Les espaces non partitionnés d'un disque : [(début, taille)] en octets."""
    total = _entier(disque.get("size")) or 0
    occupe = []
    for p in parties:
        debut = _sys(p["name"], "start")
        taille = _entier(p.get("size"))
        if debut is not None and taille:
            occupe.append((debut * 512, taille))
    occupe.sort()
    # Les tables GPT gardent 1 Mio au début et 33 secteurs à la fin.
    curseur, trous = 1024 * 1024, []
    for debut, taille in occupe:
        if debut - curseur >= LIBRE_MIN:
            trous.append((curseur, debut - curseur))
        curseur = max(curseur, debut + taille)
    fin = total - 1024 * 1024
    if fin - curseur >= LIBRE_MIN:
        trous.append((curseur, fin - curseur))
    return trous


def _partition(p):
    points = _points(p)
    enfants = p.get("children") or []
    # Une partition chiffrée ouverte : ce qu'on voit est dans son enfant.
    porte = enfants[0] if enfants and not points else p
    points = points or _points(porte)
    return {
        "chemin": p.get("path") or "/dev/" + p["name"],
        "nom": p["name"],
        "taille": _entier(p.get("size")) or 0,
        "debut": (_sys(p["name"], "start") or 0) * 512,
        "systeme_fichiers": p.get("fstype") or "",
        "etiquette": p.get("label") or "",
        "montee": points[0] if points and points[0] != "[SWAP]" else "",
        "echange": "[SWAP]" in points,
        "utilise": _entier(porte.get("fsused")),
        "libre": _entier(porte.get("fsavail")),
        "systeme": est_systeme(p),
        "chiffree": p.get("fstype") == "crypto_LUKS",
        "lecture_seule": p.get("ro") in (True, "1", 1),
        # « EFI System », « Microsoft reserved », « Windows recovery
        # environment » : la page prévient avant qu'on y touche.
        "type": p.get("parttypename") or "",
        "demarrage": (p.get("parttypename") or "") == "EFI System",
        # « Microsoft basic data » est aussi le type de tout disque de données
        # NTFS ou exFAT : Windows n'est reconnu que par son dossier.
        "windows": (p.get("parttypename") or "") in ("Microsoft reserved", "Windows recovery environment")
                   or bool(points and os.path.isdir(os.path.join(points[0], "Windows", "System32"))),
    }


def etat():
    """{'disques': [{chemin, modele, taille, amovible, table, systeme,
    partitions: [...], libres: [{debut, taille}]}]}"""
    disques = []
    for d in _lsblk():
        if d.get("type") != "disk" or d["name"].startswith(("loop", "zram", "ram", "sr")):
            continue
        if not _entier(d.get("size")):
            continue
        parties = [c for c in d.get("children") or [] if c.get("type") == "part"]
        partitions = sorted((_partition(p) for p in parties), key=lambda x: x["debut"])
        amovible = d.get("rm") in (True, "1", 1) or d.get("hotplug") in (True, "1", 1)
        disques.append({
            "chemin": d.get("path") or "/dev/" + d["name"],
            "nom": d["name"],
            "modele": (d.get("model") or "").strip() or ("Clé ou disque USB" if amovible else "Disque"),
            "liaison": (d.get("tran") or "").upper(),
            "taille": _entier(d.get("size")) or 0,
            "amovible": amovible,
            "table": d.get("pttype") or "",
            "systeme": est_systeme(d),
            # Un disque sans table mais formaté d'un bloc (clé USB) : la
            # partition, c'est le disque lui-même.
            "entier": bool(d.get("fstype")) and not parties,
            "systeme_fichiers": d.get("fstype") or "",
            "partitions": partitions,
            "libres": [{"debut": a, "taille": t} for a, t in libres(d, parties)],
            "lecture_seule": d.get("ro") in (True, "1", 1),
        })
    # Le disque du système d'abord, puis les internes, puis les clés.
    disques.sort(key=lambda d: (not d["systeme"], d["amovible"], d["nom"]))
    return {"disques": disques}


def _udisks(geste, chemin):
    """Monter ou démonter au nom de la personne (udisks, sous /media)."""
    if not chemin.startswith("/dev/") or "/" in chemin[5:]:
        return "Ce périphérique n'existe pas."
    try:
        r = subprocess.run(["udisksctl", geste, "-b", chemin, "--no-user-interaction"],
                           capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.SubprocessError) as e:
        return str(e)
    if r.returncode == 0:
        return ""
    texte = (r.stderr or r.stdout).strip()
    if "target is busy" in texte or "busy" in texte.lower():
        return "Une application utilise encore cette partition : ferme ce qui y est ouvert, puis réessaie."
    if "NotAuthorized" in texte:
        return "Il faut un compte administrateur pour cette partition."
    return texte.splitlines()[-1] if texte else "Le système a refusé."


def monter(chemin):
    return _udisks("mount", chemin)


def demonter(chemin):
    for p in [x for d in etat()["disques"] for x in d["partitions"]]:
        if p["chemin"] == chemin and p["systeme"]:
            return "Cette partition fait tourner grenOS : on ne la démonte pas."
    return _udisks("unmount", chemin)
