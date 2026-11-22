#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Essai du moteur de l'installation sur des machines factices (22 novembre).

    python3 linux/tools/essai-installation-moteur.py

Ce moteur efface des disques. L'essai ne touche à AUCUN disque : chaque
commande est notée au lieu d'être lancée. Il exige deux choses :
  - un refus, quelle qu'en soit la raison, n'a RIEN écrit ;
  - un plan accepté n'écrit que sur le disque choisi, aux bornes choisies.
La machine de départ est celle de Grenofar le 22 novembre : Samsung vide,
Windows sur le Kingston, 2 To en MBR, clé grenOS à partition « active ».
"""
import copy
import importlib.machinery
import importlib.util
import json
import os
import subprocess
import sys
import tempfile

ICI = os.path.dirname(os.path.abspath(__file__))
CHEMIN = os.path.join(ICI, "..", "config", "includes.chroot", "usr", "lib", "grenos", "grenos-installation")
chargeur = importlib.machinery.SourceFileLoader("moteur", os.path.abspath(CHEMIN))
sys.dont_write_bytecode = True
M = importlib.util.module_from_spec(importlib.util.spec_from_loader("moteur", chargeur))
chargeur.exec_module(M)

COURIR_REEL = M.courir
MIO = 1024 * 1024
BASE_MS = "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7"
CAS = []


def cas(titre):
    def poser(f):
        CAS.append((titre, f))
        return f
    return poser


def disque(chemin, octets, modele, pttype=None, fs=None, enfants=(), tran="sata", rota=False, serie=None):
    return {"path": chemin, "type": "disk", "size": octets, "model": modele, "serial": serie or modele,
            "tran": tran, "rota": rota, "ro": False, "fstype": fs, "pttype": pttype,
            "mountpoints": [None], "children": list(enfants)}


def part(chemin, fs=None, monte=None, etiquette=None, sorte="part", enfants=()):
    return {"path": chemin, "type": sorte, "fstype": fs, "label": etiquette,
            "mountpoints": [monte], "children": list(enfants)}


def table(label, parts, secteur=512, octets=None):
    t = {"label": label, "sectorsize": secteur, "partitions": [
        {"node": n, "start": d * MIO // secteur, "size": s * MIO // secteur, "type": sorte}
        for n, d, s, sorte in parts]}
    if label == "gpt":
        t["firstlba"] = 34 if secteur == 512 else 6
        t["lastlba"] = octets // secteur - (34 if secteur == 512 else 6)
    return t


SAMSUNG = 250059350016
KINGSTON = 1000204886016
HDD = 2000398934016
CLE = 7747928064


def machine():
    blocs = [
        disque("/dev/sda", SAMSUNG, "Samsung SSD 860 EVO 250GB"),
        disque("/dev/sdb", HDD, "ST2000DL003-9VT166", "dos",
               enfants=[part("/dev/sdb1", "ntfs", etiquette="Disque hdd 2to")], rota=True),
        disque("/dev/sdd", CLE, "USB DISK 2.0", "dos",
               enfants=[part("/dev/sdd1", "vfat", "/run/live/medium", "GRENOS")], tran="usb"),
        disque("/dev/nvme0n1", KINGSTON, "KINGSTON SNV3S1000G", "gpt", enfants=[
            part("/dev/nvme0n1p1", "vfat"), part("/dev/nvme0n1p2"),
            part("/dev/nvme0n1p3", "ntfs"), part("/dev/nvme0n1p4", "ntfs")], tran="nvme"),
    ]
    tables = {
        "/dev/sdb": table("dos", [("/dev/sdb1", 1, 1907727, "7")]),
        "/dev/sdd": table("dos", [("/dev/sdd1", 1, 7387, "c")]),
        "/dev/nvme0n1": table("gpt", [
            ("/dev/nvme0n1p1", 1, 200, M.TYPE_EFI),
            ("/dev/nvme0n1p2", 201, 16, M.TYPE_MSR),
            ("/dev/nvme0n1p3", 217, 952700, BASE_MS),
            ("/dev/nvme0n1p4", 952917, 944, M.TYPE_SECOURS_WINDOWS)], octets=KINGSTON),
    }
    return blocs, tables


def avec_partitions(blocs, tables, efi=False):
    """Le Samsung avec les restes d'une installation : 3 partitions Linux."""
    d = blocs[0]
    d["pttype"] = "gpt"
    d["children"] = [part("/dev/sda1", "vfat" if efi else "ext4"), part("/dev/sda2", "ext4"),
                     part("/dev/sda3", "ext4", etiquette="grenOS")]
    tables["/dev/sda"] = table("gpt", [
        ("/dev/sda1", 1, 5325, M.TYPE_EFI if efi else M.TYPE_LINUX),
        ("/dev/sda2", 5326, 9728, M.TYPE_LINUX),
        ("/dev/sda3", 15054, 223400, M.TYPE_LINUX)], octets=SAMSUNG)


class Monde:
    """Le moteur, branché sur la machine factice : il note, il ne fait rien."""

    def __init__(self, blocs, tables):
        self.blocs, self.tables = blocs, tables
        self.commandes = []
        self.collant = set()   # les montages qui refusent de partir
        self.images = ["/run/live/medium/live/filesystem.squashfs"]
        self.efi_pleine = False
        self.secteur = 512

    def courir(self, *c, entree=None, tolerer=False):
        self.commandes.append((list(c), entree))
        if c[0] == "umount" and c[-1] not in self.collant:
            self._demonter(self.blocs, c[-1])
        code = 1 if c[0] == "pgrep" else 0
        sortie = f"{self.secteur}\n" if c[0] == "blockdev" else ""
        return subprocess.CompletedProcess(c, code, sortie, "")

    def _demonter(self, blocs, ou):
        for b in blocs:
            b["mountpoints"] = [m for m in b.get("mountpoints") or [] if m != ou]
            self._demonter(b.get("children") or [], ou)

    def brancher(self):
        def images():
            if not self.images:
                raise M.Arret("image illisible")
            return self.images

        def place(_):
            if self.efi_pleine:
                raise M.Arret("pleine")

        M.courir = self.courir
        M.blocs = lambda: copy.deepcopy(self.blocs)
        M.lire_table = lambda chemin: copy.deepcopy(self.tables.get(chemin))
        M.images = images
        M.place_dans_efi = place
        M.montages_a_part = lambda: None
        M.aide = lambda cible, nom: "exit 0"
        M.en_direct = M.en_uefi = lambda: True
        M.en_essai = lambda: False
        M.un_seul = lambda: open(os.devnull)
        M.noter = lambda texte: None
        M.annoncer = lambda ligne: None
        M.partition_au_debut = lambda d, debut: f"{d}@{debut}"
        M.os.geteuid = lambda: 0
        M.noms_systeme = lambda: {"root", "audio", "sudo", "staff"}
        M.fuseau_existe = lambda f: f in ("Europe/Paris", "America/Montreal")
        M.lier = lambda vers, chemin: self.commandes.append((["lien", vers], None))
        return self

    def disque(self, chemin):
        return next(d for d in M.disques() if d["dev"] == chemin)

    def ecritures(self):
        return [(c, e) for c, e in self.commandes
                if c[0] in ("wipefs",) or c[0].startswith("mkfs")
                or (c[0] == "sfdisk" and "-J" not in c)]

    def refuse(self, plan):
        try:
            M.installer(plan)
        except M.Arret as e:
            assert not self.ecritures(), f"refus ({e}) APRÈS avoir écrit : {self.ecritures()}"
            return str(e)
        raise AssertionError("le plan aurait dû être refusé")

    def ecrit(self, plan):
        op = M.calculer(self.disque(plan["disque"]), plan)
        M.partitionner(op)
        return op


def monde(preparer=None):
    blocs, tables = machine()
    if preparer:
        preparer(blocs, tables)
    return Monde(blocs, tables).brancher()


def plan(d, **champs):
    return dict({"disque": d["dev"], "empreinte": d["empreinte"], "effacer": False,
                 "supprimer": [], "zone_mio": None, "taille_mio": 0,
                 "compte": {"login": "camille", "nom": "Camille", "mot_de_passe": None},
                 "clavier": {"disposition": "fr", "variante": ""},
                 "fuseau": "Europe/Paris"}, **champs)


# ------------------------------------------------------------- ce qui se voit

@cas("la clé de grenOS n'est jamais proposée")
def _():
    monde()
    vus = [d["dev"] for d in M.disques()]
    assert "/dev/sdd" not in vus and "/dev/sda" in vus, vus


@cas("le disque du système qui tourne n'est jamais proposé, même sous LVM")
def _():
    def p(b, t):
        b[3]["children"][2]["children"] = [part("/dev/mapper/v-racine", "ext4", "/", sorte="lvm")]
    monde(p)
    assert "/dev/nvme0n1" not in [d["dev"] for d in M.disques()]


@cas("le Samsung vide : un seul espace libre, tout le disque")
def _():
    d = monde().disque("/dev/sda")
    total = SAMSUNG // MIO
    assert d["etat"] == "vide" and d["libres"] == [{"debut_mio": 1, "taille_mio": total - 2}], d


@cas("un disque formaté sans table n'est pas de l'espace libre")
def _():
    d = monde(lambda b, t: b[0].update(fstype="exfat")).disque("/dev/sda")
    assert d["etat"] == "effacer" and not d["libres"], d


@cas("une table illisible n'est pas de l'espace libre")
def _():
    d = monde(lambda b, t: b[0].update(pttype="gpt")).disque("/dev/sda")
    assert d["etat"] == "effacer" and not d["libres"], d


@cas("le 2 To en MBR ne se prend qu'en entier")
def _():
    d = monde().disque("/dev/sdb")
    assert d["etat"] == "effacer" and not d["libres"] and d["partitions"][0]["genre"] == "windows", d


@cas("Windows est reconnu partition par partition")
def _():
    g = [p["genre"] for p in monde().disque("/dev/nvme0n1")["partitions"]]
    assert g == ["efi", "microsoft", "windows", "microsoft"], g


@cas("un volume chiffré ouvert rend le disque intouchable")
def _():
    def p(b, t):
        avec_partitions(b, t)
        b[0]["children"][1]["children"] = [part("/dev/mapper/secret", "ext4", sorte="crypt")]
    w = monde(p)
    d = w.disque("/dev/sda")
    assert d["etat"] == "occupe"
    w.refuse(plan(d, effacer=True))


@cas("secteurs de 4096 octets : les mêmes Mio")
def _():
    def p(b, t):
        b[0]["pttype"] = "gpt"
        b[0]["children"] = [part("/dev/sda1", "ext4")]
        t["/dev/sda"] = table("gpt", [("/dev/sda1", 1, 30000, M.TYPE_LINUX)], 4096, SAMSUNG)
    d = monde(p).disque("/dev/sda")
    assert d["partitions"][0]["fin_mio"] == 30001 and d["libres"][0]["debut_mio"] == 30001, d


# ------------------------------------------------------- un refus n'écrit rien

@cas("trop petit : refusé avant toute suppression")
def _():
    w = monde(avec_partitions)
    w.refuse(plan(w.disque("/dev/sda"), supprimer=["/dev/sda2"], zone_mio=5326, taille_mio=0))


@cas("taille plus grande que l'espace : refusée avant toute suppression")
def _():
    w = monde(avec_partitions)
    w.refuse(plan(w.disque("/dev/sda"), supprimer=["/dev/sda3"], zone_mio=15054, taille_mio=999999))


@cas("taille illisible (texte, négative, booléen)")
def _():
    for t in ("60", -1, True, 60.5):
        w = monde()
        w.refuse(plan(w.disque("/dev/sda"), zone_mio=1, taille_mio=t))


@cas("espace libre qui n'existe pas")
def _():
    w = monde(avec_partitions)
    w.refuse(plan(w.disque("/dev/sda"), supprimer=["/dev/sda3"], zone_mio=1))


@cas("une partition d'un autre disque dans les suppressions")
def _():
    w = monde(avec_partitions)
    w.refuse(plan(w.disque("/dev/sda"), supprimer=["/dev/nvme0n1p3"], zone_mio=15054))


@cas("la clé donnée de force dans le plan")
def _():
    w = monde()
    w.refuse({"disque": "/dev/sdd", "empreinte": "x", "effacer": True})


@cas("sda devenu un autre disque depuis l'écran")
def _():
    w = monde()
    p = plan(w.disque("/dev/sda"), effacer=True)
    w.blocs[0]["model"] = w.blocs[0]["serial"] = "Un autre disque de 250 Go"
    w.refuse(p)


@cas("une partition apparue depuis l'écran")
def _():
    w = monde()
    p = plan(w.disque("/dev/sda"), effacer=True)
    avec_partitions(w.blocs, w.tables)
    w.refuse(p)


@cas("plan sans empreinte")
def _():
    w = monde()
    w.refuse({"disque": "/dev/sda", "effacer": True})


@cas("MBR : pas de suppression à l'unité")
def _():
    w = monde()
    w.refuse(plan(w.disque("/dev/sdb"), supprimer=["/dev/sdb1"], zone_mio=1))


@cas("image illisible sur la clé : rien n'est effacé")
def _():
    w = monde()
    w.images = []
    w.refuse(plan(w.disque("/dev/sda"), effacer=True))


@cas("partition encore utilisée : pas de démontage forcé, rien n'est effacé")
def _():
    def p(b, t):
        avec_partitions(b, t)
        b[0]["children"][1]["mountpoints"] = ["/media/reste"]
    w = monde(p)
    w.collant.add("/media/reste")
    w.refuse(plan(w.disque("/dev/sda"), effacer=True))
    assert not [c for c, _ in w.commandes if c[0] == "umount" and "-l" in c]


@cas("partition EFI gardée mais pleine : rien n'est supprimé")
def _():
    w = monde(lambda b, t: avec_partitions(b, t, efi=True))
    w.efi_pleine = True
    w.refuse(plan(w.disque("/dev/sda"), supprimer=["/dev/sda3"], zone_mio=15054))


@cas("compte refusé (système, réservé, injection) : rien n'est effacé")
def _():
    for login, nom in (("root", "A"), ("audio", "A"), ("grenos", "A"), ("-x", "A"), ("a;reboot", "A"),
                       ("camille", "Cam:0:0"), ("camille", "Cam\nroot"), ("camille", "A,B"), ("", "A")):
        w = monde()
        w.refuse(plan(w.disque("/dev/sda"), effacer=True,
                      compte={"login": login, "nom": nom, "mot_de_passe": None}))


@cas("mot de passe qui glisserait une seconde ligne : rien n'est effacé")
def _():
    for mot in ("bon\nroot:pirate", "a\rb", "", 12):
        w = monde()
        w.refuse(plan(w.disque("/dev/sda"), effacer=True,
                      compte={"login": "camille", "nom": "Camille", "mot_de_passe": mot}))


@cas("clavier ou fuseau inventé : rien n'est effacé")
def _():
    for champs in ({"fuseau": "../../etc/shadow"}, {"fuseau": "Mars/Olympe"}, {"fuseau": None},
                   {"clavier": {"disposition": 'fr"; reboot', "variante": ""}}, {"clavier": None},
                   {"compte": None}):
        w = monde()
        w.refuse(plan(w.disque("/dev/sda"), effacer=True, **champs))


# ------------------------------------------------- un plan accepté écrit juste

@cas("Samsung vide, tout le disque : GPT, EFI 512 Mio, racine jusqu'au bout")
def _():
    w = monde()
    op = w.ecrit(plan(w.disque("/dev/sda"), effacer=True))
    total = SAMSUNG // MIO
    assert op["racine_debut_mio"] == 513 and op["racine_mio"] == total - 1 - 513, op
    texte = next(e for c, e in w.commandes if c[0] == "sfdisk")
    assert "label: gpt" in texte and "first-lba: 2048" in texte, texte
    assert "start=2048, size=1048576" in texte and "start=1050624" in texte, texte
    assert "MiB" not in texte, "avec une unité, sfdisk réaligne sur ce que le disque annonce"


@cas("créer pour grenOS, 60 Gio dans l'espace libre")
def _():
    w = monde()
    op = w.ecrit(plan(w.disque("/dev/sda"), zone_mio=1, taille_mio=60 * 1024))
    assert op["racine_mio"] == 61440 and op["efi_debut_mio"] == 1 and op["table_neuve"], op


@cas("supprimer deux voisines : leur place ne fait qu'un espace")
def _():
    w = monde(avec_partitions)
    op = w.ecrit(plan(w.disque("/dev/sda"), supprimer=["/dev/sda2", "/dev/sda3"], zone_mio=5326))
    enleves = [c[-1] for c, _ in w.commandes if c[:2] == ["sfdisk", "--delete"]]
    assert enleves == ["3", "2"] and op["racine_debut_mio"] == 5326 + 512 and not op["table_neuve"], op
    assert not [c for c, _ in w.commandes if c[0] == "wipefs"], "wipefs hors de « effacer »"


@cas("la partition EFI du même disque est gardée, jamais formatée")
def _():
    w = monde(lambda b, t: avec_partitions(b, t, efi=True))
    op = w.ecrit(plan(w.disque("/dev/sda"), supprimer=["/dev/sda3"], zone_mio=15054))
    assert op["efi"] == "/dev/sda1"
    assert not [c for c, _ in w.commandes if c[0] == "mkfs.vfat"], w.commandes


@cas("sans EFI sur le Samsung : ni celle de Windows, ni celle de la clé (21 novembre)")
def _():
    w = monde(avec_partitions)
    op = w.ecrit(plan(w.disque("/dev/sda"), supprimer=["/dev/sda3"], zone_mio=15054))
    assert op["efi"] is None and op["efi_debut_mio"] == 15054, op


@cas("un plan accepté n'écrit que sur le disque choisi")
def _():
    for champs in ({"effacer": True}, {"supprimer": ["/dev/sda3"], "zone_mio": 15054}):
        w = monde(avec_partitions)
        w.ecrit(plan(w.disque("/dev/sda"), **champs))
        for c, _ in w.ecritures():
            vises = [a for a in c if a.startswith("/dev/")]
            assert vises and all(a.startswith("/dev/sda") for a in vises), c


@cas("les choix vont sur le disque ; le mot de passe, seulement par l'entrée")
def _():
    w = monde()
    reel = w.courir

    def courir(*c, entree=None, tolerer=False):
        r = reel(*c, entree=entree, tolerer=tolerer)
        sortie = "uuid-1" if c[0] == "blkid" else "fr_FR.utf8" if c[-2:] == ("locale", "-a") else ""
        return subprocess.CompletedProcess(c, r.returncode, sortie, "")

    M.courir = courir
    secret = "Essai-pas-un-vrai"
    choisi = M.verifier_reglages(plan(w.disque("/dev/sda"), clavier={"disposition": "ca", "variante": "eng"},
                                      fuseau="America/Montreal",
                                      compte={"login": "camille", "nom": "Camille D", "mot_de_passe": secret}),
                                 set(), M.fuseau_existe)
    M.en_essai = lambda: True  # pas le Wi-Fi de la machine d'essai
    with tempfile.TemporaryDirectory() as cible:
        M.configurer(cible, "/dev/sda2", "/dev/sda1", choisi)
        ecrits = ""
        for racine, _d, fichiers in os.walk(cible):
            for f in fichiers:
                with open(os.path.join(racine, f), encoding="utf-8") as lu:
                    ecrits += lu.read()
    arguments = " ".join(" ".join(c) for c, _e in w.commandes)
    assert secret not in arguments and secret not in ecrits, "le mot de passe a fui"
    definir = [e for c, e in w.commandes if "definir" in c]
    assert definir == [secret + "\n"], definir
    for attendu in ("login=camille", "nom=Camille D", 'XKBLAYOUT="ca"', 'XKBVARIANT="eng"', "America/Montreal"):
        assert attendu in ecrits, attendu


@cas("le journal ne note jamais ce qui passe par l'entrée")
def _():
    monde()
    notes, vrai = [], M.subprocess.run
    M.noter = notes.append
    M.subprocess.run = lambda *a, **k: subprocess.CompletedProcess(a[0], 0, "", "")
    try:
        COURIR_REEL("chroot", "/mnt", "grenos-compte", "definir", "camille", entree="Essai-pas-un-vrai\n")
    finally:
        M.subprocess.run = vrai
    assert notes and not any("Essai-pas-un-vrai" in n for n in notes), notes


@cas("secteurs de 4096 octets : les mêmes bornes, écrites dans leurs secteurs")
def _():
    w = monde()
    w.secteur = 4096
    w.ecrit(plan(w.disque("/dev/sda"), effacer=True))
    texte = next(e for c, e in w.commandes if c[0] == "sfdisk")
    assert "first-lba: 256" in texte and "start=256, size=131072" in texte and "start=131328" in texte, texte


@cas("udev a fini avant chaque écriture de table (sinon « disque occupé », déjà effacé)")
def _():
    for champs in ({"effacer": True}, {"supprimer": ["/dev/sda2", "/dev/sda3"], "zone_mio": 5326}):
        w = monde(avec_partitions)
        w.ecrit(plan(w.disque("/dev/sda"), **champs))
        occupe = False
        for c, _e in w.commandes:
            if c[0] == "udevadm":
                occupe = False
            if c[0] == "sfdisk":
                assert not occupe, (champs, c)
            if c[0] in ("sfdisk", "wipefs"):
                occupe = True


@cas("un lien du disque installé ne fait jamais écrire sur le système qui tourne")
def _():
    w = monde()
    choisi = M.verifier_reglages(plan(w.disque("/dev/sda")), set(), M.fuseau_existe)
    reel = w.courir

    def courir(*c, entree=None, tolerer=False):
        r = reel(*c, entree=entree, tolerer=tolerer)
        sortie = "uuid-1" if c[0] == "blkid" else "fr_FR.utf8" if c[-2:] == ("locale", "-a") else ""
        return subprocess.CompletedProcess(c, r.returncode, sortie, "")

    M.courir = courir
    M.en_essai = lambda: True
    with tempfile.TemporaryDirectory() as base:
        cible = os.path.join(base, "a", "b", "cible")
        os.makedirs(cible)
        liens = {os.path.join(cible, "etc", "default", "locale"): "/etc/locale.conf",
                 os.path.join(cible, "etc", "hostname"): "../../../../dehors"}
        islink, readlink = M.os.path.islink, M.os.readlink
        M.os.path.islink = lambda p: p in liens
        M.os.readlink = lambda p: liens[p]
        try:
            M.configurer(cible, "/dev/sda2", "/dev/sda1", choisi)
        finally:
            M.os.path.islink, M.os.readlink = islink, readlink
        assert os.path.isfile(os.path.join(cible, "etc", "locale.conf")), "le lien absolu ne repart pas du disque installé"
        assert os.path.isfile(os.path.join(cible, "dehors")), "« .. » sort du disque installé"
        assert not os.path.exists(os.path.join(base, "dehors"))


@cas("un nom que la page accepte, le moteur l'accepte (espace insécable, emoji composé)")
def _():
    w = monde()
    for nom in ("Jean\u00a0Paul", "Zoé", "A\u200dB"):
        p = plan(w.disque("/dev/sda"), compte={"login": "camille", "nom": nom, "mot_de_passe": None})
        assert M.verifier_reglages(p, set(), M.fuseau_existe)["nom"] == nom
    p = plan(w.disque("/dev/sda"), compte={"login": "camille", "nom": "A\x85B", "mot_de_passe": None})
    try:
        M.verifier_reglages(p, set(), M.fuseau_existe)
    except M.Arret:
        return
    raise AssertionError("un signe de contrôle est passé")


@cas("chaque clavier que la page propose passe au moteur")
def _():
    w = monde()
    d = w.disque("/dev/sda")
    with open(os.path.join(ICI, "..", "config", "includes.chroot", "usr", "share", "grenos", "premier.json"),
              encoding="utf-8") as f:
        claviers = json.load(f)["claviers"]
    for c in claviers:
        M.verifier_reglages(plan(d, clavier={"disposition": c["disposition"], "variante": c["variante"]},
                                 fuseau=c["fuseau"]), set(), lambda _f: True)


@cas("GRUB du disque : « grenOS », et l'écran de chargement au lieu du texte")
def _():
    modele = ('GRUB_DISTRIBUTOR=`( . /etc/os-release; echo ${NAME:-Debian} ) 2>/dev/null`\n'
              '#GRUB_TERMINAL=console\nGRUB_CMDLINE_LINUX_DEFAULT="quiet"\nGRUB_CMDLINE_LINUX=""\n')
    for texte in (modele, ""):
        fait = M.habiller_grub(texte).splitlines()
        assert fait.count("GRUB_DISTRIBUTOR='grenOS'") == 1, fait
        assert fait.count('GRUB_CMDLINE_LINUX_DEFAULT="quiet splash"') == 1, fait
        assert len([l for l in fait if l.startswith("GRUB_CMDLINE_LINUX_DEFAULT=")]) == 1, fait
    assert 'GRUB_CMDLINE_LINUX=""' in M.habiller_grub(modele)


@cas("le Wi-Fi réservé au compte de la clé passe au nouveau compte")
def _():
    avant = "[connection]\nid=maison\npermissions=user:grenos:;\n[wifi]\nssid=maison\n"
    assert M.reseau_pour(avant, "camille") == avant.replace("user:grenos:", "user:camille:")
    commun = "[connection]\nid=maison\n"
    assert M.reseau_pour(commun, "camille") == commun


def main():
    fautes = 0
    for titre, f in CAS:
        try:
            f()
            print(f"ok   {titre}")
        except Exception as e:  # chaque cas rend son verdict
            fautes += 1
            print(f"FAUX {titre} : {type(e).__name__} {e}")
    print(f"installation-moteur : {len(CAS)} cas, {fautes} faute(s)")
    return 1 if fautes else 0


if __name__ == "__main__":
    sys.exit(main())
