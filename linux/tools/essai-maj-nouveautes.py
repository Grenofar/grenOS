#!/usr/bin/env python3
"""Les nouveautés des mises à jour et leur notification, sans réseau ni
session (28 novembre).

    python3 linux/tools/essai-maj-nouveautes.py

Grenofar : des descriptions pour chaque mise à jour, dans Mises à jour et
Paramètres › Mises à jour ; à chaque redémarrage, vérifier sans installer et
notifier s'il y a une mise à jour. Ici :
  - nouveautes.json du dépôt (ce que le paquet et le dépôt APT emportent) :
    lisible, complet, ids uniques, le plus récent en premier ;
  - grenos-maj-nouveautes : l'adresse du dépôt dans grenos.list, « à venir »
    = en ligne et pas sur la machine, une passe complète (réseau en panne,
    machine d'avant les nouveautés, rien de neuf) ;
  - grenos-maj-notifier : le compte, le mode, le texte selon le mode, une
    seule notification par démarrage, rien en « Manuelles ».
"""
import importlib.machinery
import importlib.util
import json
import os
import re
import stat
import sys
import tempfile

ICI = os.path.dirname(os.path.abspath(__file__))
DEDANS = os.path.join(ICI, "../config/includes.chroot")


def charger(nom, chemin):
    chargeur = importlib.machinery.SourceFileLoader(nom, os.path.join(DEDANS, chemin))
    spec = importlib.util.spec_from_loader(nom, chargeur)
    module = importlib.util.module_from_spec(spec)
    chargeur.exec_module(module)
    return module


nv = charger("grenos_maj_nouveautes", "usr/lib/grenos/grenos-maj-nouveautes")
nt = charger("grenos_maj_notifier", "usr/lib/grenos/grenos-maj-notifier")

FAUTES = 0
CAS = 0


def verifier(vrai, texte):
    global FAUTES, CAS
    CAS += 1
    if vrai:
        print(f"  [ok] {texte}")
    else:
        print(f"  [FAUTE] {texte}")
        FAUTES += 1


print("nouveautes.json du dépôt :")
TEXTE = open(os.path.join(DEDANS, "usr/share/grenos/nouveautes.json"), encoding="utf-8").read()
BRUT = json.loads(TEXTE)["nouveautes"]
LUES = nv.notes(TEXTE)
verifier(len(LUES) == len(BRUT) and LUES, "chaque nouveauté a son id, son titre et son texte (aucune écartée)")
ids = [n["id"] for n in BRUT]
verifier(len(ids) == len(set(ids)), "les ids sont uniques")
verifier(all(n.get("genre") in nv.GENRES for n in BRUT), "genre : « nouveaute » ou « correctif »")
dates = [n.get("date", "") for n in BRUT]
verifier(all(re.fullmatch(r"\d{4}-\d{2}-\d{2}", d) for d in dates), "des dates AAAA-MM-JJ")
verifier(dates == sorted(dates, reverse=True), "la plus récente en premier")
verifier(all(len(n["titre"]) <= 90 for n in BRUT), "des titres courts (une ligne de notification)")

print("grenos-maj-nouveautes :")
LISTE = ("deb [signed-by=/usr/share/keyrings/grenos-apt.gpg] "
         "https://tpqzhzuoyqpfairatdrw.supabase.co/storage/v1/object/public/apt stable main\n")
verifier(nv.adresse_depot(LISTE) == "https://tpqzhzuoyqpfairatdrw.supabase.co/storage/v1/object/public/apt",
         "l'adresse du dépôt dans le vrai grenos.list")
verifier(nv.adresse_depot("# deb https://ancien/ stable main\ndeb https://x/apt/ stable main\n") == "https://x/apt",
         "une ligne commentée ignorée, le « / » final retiré")
verifier(nv.adresse_depot("") is None and nv.adresse_depot("deb-src https://x stable main") is None,
         "pas de ligne deb : aucune adresse")

A = {"id": "a", "date": "2026-11-28", "genre": "correctif", "titre": "A", "texte": "a"}
B = {"id": "b", "date": "2026-11-27", "genre": "nouveaute", "titre": "B", "texte": "b"}
C = {"id": "c", "date": "2026-11-26", "genre": "nouveaute", "titre": "C", "texte": "c"}
verifier(nv.a_venir([A, B, C], [B, C]) == [A], "à venir : ce qui est en ligne et pas sur la machine")
verifier(nv.a_venir([A, B], []) == [A, B], "une machine d'avant les nouveautés : tout est à venir, dans l'ordre")
verifier(nv.a_venir([A], [A, B]) == [], "rien de neuf : rien à venir")
abime = json.dumps({"nouveautes": [A, {"id": "x"}, "texte", {"id": "y", "titre": " ", "texte": "t"}]})
verifier([n["id"] for n in nv.notes(abime)] == ["a"], "une entrée incomplète est écartée, pas le fichier")
for mauvais, quoi in (("[]", "une liste"), ('{"autre": 1}', "sans « nouveautes »"), ("{", "JSON coupé")):
    try:
        nv.notes(mauvais)
        verifier(False, f"illisible ({quoi}) : refusé")
    except ValueError:
        verifier(True, f"illisible ({quoi}) : refusé")

with tempfile.TemporaryDirectory() as d:
    sources, installees, sortie = f"{d}/grenos.list", f"{d}/share/nouveautes.json", f"{d}/run/maj-nouveautes.json"
    open(sources, "w").write(LISTE)
    demandes = []

    def en_ligne(adresse):
        demandes.append(adresse)
        return json.dumps({"nouveautes": [A, B, C]})

    def passe(telecharger=en_ligne):
        return nv.passe(sources=sources, installees=installees, sortie=sortie, telecharger=telecharger)

    verifier(passe() == 3 and demandes[-1].endswith("/object/public/apt/nouveautes.json"),
             "le fichier est demandé à la racine du dépôt")
    ecrit = json.load(open(sortie, encoding="utf-8"))
    verifier([n["id"] for n in ecrit["a_venir"]] == ["a", "b", "c"] and ecrit.get("date"),
             "machine sans nouveautes.json : les trois à venir, datées")
    verifier(stat.S_IMODE(os.stat(sortie).st_mode) == 0o644, "lisible par la session (0644) : il est écrit en root")
    os.makedirs(os.path.dirname(installees))
    open(installees, "w").write(json.dumps({"nouveautes": [B, C]}))
    verifier(passe() == 1 and [n["id"] for n in json.load(open(sortie))["a_venir"]] == ["a"],
             "la machine a b et c : seule a vient")

    def panne(adresse):
        raise OSError("réseau injoignable")
    avant = open(sortie).read()
    verifier(passe(panne) is None and open(sortie).read() == avant, "réseau en panne : on ne sait rien, rien n'est réécrit")
    verifier(passe(lambda a: "<html>404</html>") is None, "un dépôt d'avant les nouveautés (404, page HTML) : rien")
    os.remove(sources)
    verifier(passe() is None, "pas de grenos.list : rien")

print("grenos-maj-notifier :")
verifier(nt.lire_compte("12\n") == 12 and nt.lire_compte("0") == 0, "le compte de /run/grenos/maj")
verifier(nt.lire_compte("inconnu\n") is None and nt.lire_compte("") is None and nt.lire_compte(None) is None,
         "« inconnu », vide ou absent : on ne sait pas")
verifier(nt.lire_mode("manuelles\n", False) == "manuelles" and nt.lire_mode(None, True) == "automatiques"
         and nt.lire_mode(None, False) == "demander", "le mode, comme grenos-ui le lit")
A_VENIR = json.dumps({"a_venir": [A, B, C, dict(A, id="d", titre="D")]})
verifier(nt.titres(A_VENIR) == ["A", "B", "C"], "les trois premières nouveautés")
verifier(nt.titres("{abîmé") == [] and nt.titres("") == [], "nouveautés illisibles : la notification part sans elles")

titre, corps = nt.message(37, "demander", ["A", "B"])
verifier(titre == "Mise à jour de grenOS disponible" and corps.startswith("37 paquets à installer. Rien ne s'installe sans toi."),
         "« Me demander » : combien, et rien sans toi")
verifier(corps.endswith("\nNouveau : A · B"), "les nouveautés en phrases, sous le compte")
verifier(nt.message(1, "demander", [])[1] == "1 paquet à installer. Rien ne s'installe sans toi.", "un seul paquet : au singulier, sans « Nouveau »")
verifier("se poseront d'eux-mêmes" in nt.message(5, "automatiques", [])[1], "« Automatiques » : ils se poseront seuls")
verifier("installés au prochain redémarrage" in nt.message(5, "demander", [], arme=True)[1], "déjà armée : au prochain redémarrage")
verifier(nt.message(5, "manuelles", ["A"]) is None, "« Manuelles » : on se tait")
verifier(nt.message(0, "demander", ["A"]) is None and nt.message(None, "demander", []) is None, "rien, ou on ne sait pas : on se tait")

with tempfile.TemporaryDirectory() as d:
    fichiers = {nt.COMPTE: "4\n", nt.MODE: "demander\n", nt.NOUVEAUTES: A_VENIR}
    envoyees = []
    m = f"{d}/{nt.MARQUE}"

    def passe(arme=False):
        return nt.passe(lire=lambda c: fichiers.get(c), marque=lambda: m,
                        notifier=lambda t, c: envoyees.append((t, c)), arme=lambda: arme)

    verifier(passe() == "annoncée" and len(envoyees) == 1 and os.path.exists(m), "une mise à jour : annoncée, marquée")
    verifier(passe() == "déjà" and len(envoyees) == 1, "une seconde lecture : pas deux notifications par démarrage")
    os.remove(m)
    fichiers[nt.COMPTE] = "0\n"
    verifier(passe() is None and not os.path.exists(m), "rien à installer : pas de notification, pas de marque")
    fichiers[nt.COMPTE] = "4\n"
    fichiers[nt.MODE] = "manuelles\n"
    verifier(passe() is None and len(envoyees) == 1, "« Manuelles » : pas de notification")
    del fichiers[nt.COMPTE]
    fichiers[nt.MODE] = "demander\n"
    verifier(passe() is None, "pas encore de vérification depuis le démarrage : on attend")

if FAUTES:
    print(f"maj nouveautés : {FAUTES} faute(s) sur {CAS} cas")
    sys.exit(1)
print(f"maj nouveautés : {CAS} cas, 0 faute")
