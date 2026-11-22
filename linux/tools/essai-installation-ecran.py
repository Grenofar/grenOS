#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""L'écran et le moteur de l'installation font-ils le MÊME calcul ? (22 novembre)

    python3 linux/tools/essai-installation-ecran.py      (demande node)

L'écran (installation.js) montre ce qui sera supprimé et créé ; le moteur
(grenos-installation) le refait avant d'écrire. Un Mio d'écart et la personne
confirme autre chose que ce qui sera fait. On tire des milliers de disques
et de clics au hasard (graine fixe) : l'écran calcule le plan, le moteur doit
l'accepter et tomber sur les mêmes bornes ; là où l'écran ne propose rien,
il n'y a pas de plan.
"""
import importlib.machinery
import importlib.util
import json
import os
import random
import subprocess
import sys
import tempfile

ICI = os.path.dirname(os.path.abspath(__file__))
LINUX = os.path.dirname(ICI)
MOTEUR = os.path.join(LINUX, "config", "includes.chroot", "usr", "lib", "grenos", "grenos-installation")
ECRAN = os.path.join(LINUX, "ui", "web", "installation.js")
TOURS = 20000

chargeur = importlib.machinery.SourceFileLoader("moteur", MOTEUR)
sys.dont_write_bytecode = True
M = importlib.util.module_from_spec(importlib.util.spec_from_loader("moteur", chargeur))
chargeur.exec_module(M)

REGLES = {"minimum_mio": M.MINIMUM_RACINE_MIO, "efi_mio": M.EFI_MIO, "trou_minimum_mio": M.TROU_MINIMUM_MIO}

NODE = """
const fs = require("fs");
const { calculer, parDefaut } = require(process.argv[2]);
const W = { dev: "/dev/w", etat: "gpt", partitions: [{ genre: "efi" }, { genre: "windows" }] };
const F = { dev: "/dev/f", etat: "effacer", partitions: [] };
const L = { dev: "/dev/l", etat: "gpt", partitions: [{ genre: "linux" }] };
const V = { dev: "/dev/v", etat: "vide", partitions: [] };
for (const [disques, attendu] of [[[W, F], null], [[W, L], "/dev/l"], [[W, L, V], "/dev/v"], [[], null]]) {
  if (parDefaut(disques) !== attendu) { console.error("le disque de Windows serait choisi d'office"); process.exit(1); }
}
const entree = JSON.parse(fs.readFileSync(0, "utf8"));
process.stdout.write(JSON.stringify(entree.cas.map((c) => calculer(c.disque, c.choix, entree.regles))));
"""


def tirer(h):
    taille = h.choice([9000, 21000, 64000, 238475, 953869, 1907729])
    etat = h.choice(["gpt", "gpt", "gpt", "gpt", "vide", "effacer", "occupe"])
    partitions = []
    if etat != "vide":
        position = h.choice([1, 1, 1, 2, 300])
        for i in range(h.randint(0, 6)):
            position += h.choice([0, 0, 0, 1, 900, 1024, 25000, 80000])
            longueur = h.choice([16, 100, 512, 944, 5325, 20480, 60000, 200000])
            if position + longueur > taille - 1:
                break
            efi = h.random() < .2
            partitions.append({"dev": f"/dev/sdz{i + 1}", "debut_mio": position,
                               "fin_mio": position + longueur, "taille_mio": longueur,
                               "fs": "vfat" if efi else "ext4", "genre": "efi" if efi else "linux",
                               "nom": "Partition", "efi_utilisable": efi and longueur >= M.EFI_MINIMUM_MIO})
            position += longueur
    disque = {"dev": "/dev/sdz", "modele": "Disque d'essai", "taille_mio": taille, "etat": etat,
              "raison": "pas ici", "premier_mio": 1, "fin_mio": taille - 1,
              "partitions": partitions, "empreinte": f"e{h.random()}"}
    effacer = h.random() < .2
    choix = {"effacer": effacer,
             "supprimer": [p["dev"] for p in partitions if h.random() < .4],
             "point": 1 if effacer else h.choice([None] + [1] + [p["debut_mio"] for p in partitions]
                                                + [p["fin_mio"] for p in partitions]),
             "taille": h.choice([0, 0, 20480, 40960, 61440, 102400, 4000000, 5])}
    return {"disque": disque, "choix": choix}


def main():
    h = random.Random(22112026)
    cas = [tirer(h) for _ in range(TOURS)]
    with tempfile.TemporaryDirectory() as d:
        lanceur = os.path.join(d, "calcul.js")
        with open(lanceur, "w", encoding="utf-8") as f:
            f.write(NODE)
        r = subprocess.run(["node", lanceur, ECRAN], capture_output=True, text=True, encoding="utf-8",
                           input=json.dumps({"regles": REGLES, "cas": cas}))
    if r.returncode != 0:
        print(f"installation-ecran : node a echoue : {r.stderr[:800]}")
        return 1
    fautes, plans = [], 0
    for c, vu in zip(cas, json.loads(r.stdout)):
        plan = vu["plan"]
        if not plan:
            continue
        plans += 1
        try:
            op = M.calculer(c["disque"], plan)
        except M.Arret as e:
            fautes.append(f"le moteur refuse le plan de l'écran ({e}) : {c}")
            continue
        k = vu["creation"]
        montre = (k["debut_mio"], k["taille"], k["efi"]["dev"] if k["efi"] else None,
                  sorted(p["dev"] for p in vu["partent"]))
        fera = (op["racine_debut_mio"], op["racine_mio"], op["efi"], sorted(op["partent"]))
        if montre != fera:
            fautes.append(f"montré {montre}, ferait {fera} : {c}")
            continue
        # Rien de ce qui reste n'est recouvert par ce qui se crée.
        a = op["efi_debut_mio"] if op["efi"] is None else op["racine_debut_mio"]
        b = op["racine_debut_mio"] + op["racine_mio"]
        for p in c["disque"]["partitions"]:
            if p["dev"] not in op["partent"] and p["debut_mio"] < b and p["fin_mio"] > a:
                fautes.append(f"recouvre {p['dev']}, qui reste : {c}")
        if b > c["disque"]["taille_mio"] - 1:
            fautes.append(f"dépasse le disque : {c}")
    for f in fautes[:8]:
        print("FAUX", f)
    if plans < TOURS // 10:
        print(f"FAUX trop peu de plans tirés ({plans}) : l'essai ne prouve rien")
        return 1
    print(f"installation-ecran : {TOURS} tirages, {plans} plans, {len(fautes)} faute(s)")
    return 1 if fautes else 0


if __name__ == "__main__":
    sys.exit(main())
