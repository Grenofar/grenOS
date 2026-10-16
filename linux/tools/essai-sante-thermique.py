#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Exercer `sante.temperatures()` et `sante.ventilateurs()` sur de faux /sys.

POURQUOI CET ESSAI EXISTE — et il a une histoire courte et instructive.

Le 16 octobre, une tâche d'agent a livré `usr/lib/grenos/thermique.py`, « un
module qui lit la température et la vitesse des ventilateurs ». Il n'a pas été
reprise : la moitié température existait déjà dans `sante.py`, mieux faite, et
sa version rendait températures et vitesses **dans une seule liste de
couples** — où rien ne distingue 2800 tours de 2800 degrés. *Deux lectures
d'une même chose finissent par diverger, et c'est alors la copie qu'on croit.*

Ce qui manquait vraiment a été pris : les ventilateurs, et les zones ACPI pour
les machines dont `hwmon` ne porte aucune sonde.

CE QUE CET ESSAI ATTRAPE, et qu'une relecture ne verrait pas :

* une machine qui n'a QUE des zones ACPI — beaucoup de portables — doit
  quand même voir sa température, et pas « aucune sonde » ;
* la même sonde exposée par les deux arbres ne doit pas s'afficher deux fois ;
* un ventilateur à l'arrêt doit se lire « 0 », parce que c'est une
  information, et pas disparaître comme une absence ;
* une sonde débranchée (-273 °C) et un registre mal lu (60000 tours) ne
  doivent jamais passer pour des mesures.

Chaque cas est joué sur un arbre de fichiers fabriqué, jamais sur /sys de
cette machine : une mesure qui dépend du matériel de celui qui l'exécute ne
peut rien prouver à personne d'autre.
"""
import io
import os
import shutil
import sys
import tempfile

RACINE = os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(RACINE, "linux", "config", "includes.chroot",
                                "usr", "lib", "grenos"))
import sante  # noqa: E402  (le chemin doit être posé avant)

echecs = []


def poser(dossier, chemin, contenu):
    cible = os.path.join(dossier, chemin)
    os.makedirs(os.path.dirname(cible), exist_ok=True)
    io.open(cible, "w", encoding="utf-8").write(contenu + "\n")


def cas(nom, fichiers, temperatures_attendues, ventilateurs_attendus):
    bac = tempfile.mkdtemp(prefix="grenos-sante-")
    try:
        for chemin, contenu in fichiers.items():
            poser(bac, chemin, contenu)
        chaleurs = sante.temperatures(os.path.join(bac, "hwmon"),
                                      os.path.join(bac, "thermal"))
        vents = sante.ventilateurs(os.path.join(bac, "hwmon"))
    finally:
        shutil.rmtree(bac, ignore_errors=True)

    souci = []
    vus = sorted((n, round(v, 1)) for n, v in chaleurs)
    if vus != sorted(temperatures_attendues):
        souci.append("temperatures %s au lieu de %s"
                     % (vus, sorted(temperatures_attendues)))
    vus_v = sorted(vents)
    if vus_v != sorted(ventilateurs_attendus):
        souci.append("ventilateurs %s au lieu de %s"
                     % (vus_v, sorted(ventilateurs_attendus)))
    if souci:
        echecs.append(nom)
        print("  [KO] %-54s %s" % (nom, " ; ".join(souci)))
    else:
        print("  [ok] %-54s %d sonde(s), %d ventilateur(s)"
              % (nom, len(chaleurs), len(vents)))


# 1. RIEN : une machine virtuelle. Le hub doit pouvoir dire « aucune sonde ».
cas("aucun arbre : aucune sonde, aucun ventilateur", {}, [], [])

# 2. hwmon seul, avec etiquette.
cas("hwmon : une sonde etiquetee",
    {"hwmon/hwmon0/name": "coretemp",
     "hwmon/hwmon0/temp1_input": "47000",
     "hwmon/hwmon0/temp1_label": "Package id 0"},
    [("coretemp — Package id 0", 47.0)], [])

# 3. LE CAS QUI COMPTE : rien dans hwmon, tout dans les zones ACPI.
cas("ACPI seul : la machine voit quand meme sa temperature",
    {"thermal/thermal_zone0/type": "acpitz",
     "thermal/thermal_zone0/temp": "52000"},
    [("acpitz", 52.0)], [])

# 4. LA MEME SONDE DES DEUX COTES : une seule ligne.
cas("la meme valeur dans les deux arbres ne compte qu une fois",
    {"hwmon/hwmon0/name": "k10temp",
     "hwmon/hwmon0/temp1_input": "41000",
     "thermal/thermal_zone0/type": "k10temp",
     "thermal/thermal_zone0/temp": "41000"},
    [("k10temp", 41.0)], [])

# 5. Deux valeurs differentes : les deux comptent.
cas("deux valeurs differentes : les deux comptent",
    {"hwmon/hwmon0/name": "k10temp",
     "hwmon/hwmon0/temp1_input": "41000",
     "thermal/thermal_zone0/type": "pch",
     "thermal/thermal_zone0/temp": "63000"},
    [("k10temp", 41.0), ("pch", 63.0)], [])

# 6. Les sondes absurdes ne passent pas.
cas("une sonde debranchee et une sonde a 200 degres sont ecartees",
    {"hwmon/hwmon0/name": "nct6775",
     "hwmon/hwmon0/temp1_input": "-273150",
     "hwmon/hwmon0/temp2_input": "200000",
     "hwmon/hwmon0/temp3_input": "38000"},
    [("nct6775", 38.0)], [])

# 7. LE VENTILATEUR, et le zero qu il faut garder.
cas("deux ventilateurs, dont un a l arret",
    {"hwmon/hwmon1/name": "thinkpad",
     "hwmon/hwmon1/fan1_input": "2800",
     "hwmon/hwmon1/fan1_label": "Ventilateur processeur",
     "hwmon/hwmon1/fan2_input": "0"},
    [], [("Ventilateur processeur", 2800), ("thinkpad — fan2", 0)])

# 8. Un registre mal lu n est pas une vitesse.
cas("60000 tours : ecarte",
    {"hwmon/hwmon1/name": "it87",
     "hwmon/hwmon1/fan1_input": "60000",
     "hwmon/hwmon1/fan2_input": "1200"},
    [], [("it87 — fan2", 1200)])

# 9. Les deux ensemble, et ils ne se melangent pas.
cas("temperature et ventilateur ne se melangent jamais",
    {"hwmon/hwmon0/name": "coretemp",
     "hwmon/hwmon0/temp1_input": "55000",
     "hwmon/hwmon1/name": "dell_smm",
     "hwmon/hwmon1/fan1_input": "2800"},
    [("coretemp", 55.0)], [("dell_smm — fan1", 2800)])

# 10. Un fichier illisible ne fait pas tomber la lecture.
cas("une valeur qui n est pas un nombre est ignoree",
    {"hwmon/hwmon0/name": "fantome",
     "hwmon/hwmon0/temp1_input": "n/a",
     "hwmon/hwmon0/fan1_input": "err"},
    [], [])

print("")
if echecs:
    print("%d cas sur 10 ont echoue." % len(echecs))
    sys.exit(1)
print("les 10 cas passent.")
