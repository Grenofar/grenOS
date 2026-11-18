#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Régénérer la partie Steam du catalogue de GrenLaunch, à la source.

Un catalogue de jeux écrit de mémoire ment à la première mise à jour d'un
éditeur. Celui-ci est CALCULÉ :

* l'API publique du Steam Store donne le vrai nom, la gratuité, le genre et
  surtout si le jeu a une version Linux native (`platforms.linux`) ;
* la liste d'AreWeAntiCheatYet dit, pour les jeux à anti-triche, si l'éditeur
  l'a activé sous Linux (Supported/Running) ou le refuse (Denied/Broken).

Les entrées qui ne viennent pas de Steam (Fortnite, Roblox, les jeux de
Flathub…) sont gardées telles quelles : elles sont écrites à la main, avec
leur raison.

    python3 linux/tools/catalogue-jeux.py          # réécrit jeux.json
    python3 linux/tools/catalogue-jeux.py --essai  # montre sans écrire

Un appid refusé par Steam est signalé et sauté, jamais deviné.
"""
import colorsys
import hashlib
import io
import json
import os
import re
import sys
import time
import urllib.request

RACINE = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CHEMIN = os.path.join(RACINE, "linux", "config", "includes.chroot", "usr", "share", "grenos", "jeux.json")
AWACY = "https://raw.githubusercontent.com/AreWeAntiCheatYet/AreWeAntiCheatYet/HEAD/games.json"
STORE = "https://store.steampowered.com/api/appdetails?appids=%d&l=french&cc=fr"

# Les jeux proposés, par appid Steam. L'ordre est celui de l'affichage.
APPIDS = [
    730, 570, 440, 252950, 230410, 2357570, 2767030, 553850, 1172470, 578080,
    1085660, 1938090, 359550, 2073850, 236390, 1203220, 1599340, 238960, 2694490,
    1665460, 1097150, 1172620, 381210, 945360, 1966720, 739630, 548430, 252490,
    892970, 1623730, 322330, 108600, 294100, 427520, 526870, 255710, 949230,
    289070, 1158310, 394360, 281990, 813780, 1466860, 1142710, 457140, 1062090,
    264710, 848450, 242760, 1326470, 251570, 346110, 1245620, 1086940, 1091500,
    271590, 1174180, 292030, 489830, 377160, 1716740, 990080, 1593500, 2322010,
    1817070, 1888930, 2050650, 883710, 1196590, 374320, 814380, 2246340, 582010,
    1446780, 1151640, 2420110, 1771300, 379430, 2183900, 1361210, 552500,
    413150, 105600, 367520, 1145360, 1145350, 646570, 2379780, 1794680, 1868140,
    268910, 504230, 391540, 1150690, 1332010, 620, 400, 220, 4000, 550, 218620,
    1426210, 1551360, 1293830, 244210, 805550, 1250410, 39210, 1063730, 2139460,
    1449850, 976730, 1240440, 1517290, 1238810,
    # 18 novembre : « ajoute un max d'apps au launcher ». Les sorties récentes
    # et les classiques qui manquaient ; le calcul dit pour chacun s'il marche.
    2358720, 2399830, 3241660, 2622380, 1903340, 3489700, 1808500, 2215430,
    1817230, 2054970, 1030300, 588650, 753640, 250900, 632360, 1942280,
    1092790, 975370, 1295660, 236850, 2001120, 2440510, 1203620, 1366540,
    1328670, 524220, 1222140, 1659040, 1237970, 2208920, 3164500, 2344520,
    1817190, 2651280, 1238840, 1888160, 2161700, 1687950, 1113560, 1454400,
    1677740,
]

# Ce que les sources ne disent pas, et qu'il faut dire quand même.
NOTES_A_LA_MAIN = {
    271590: ("proton", "Le mode histoire marche par Proton. GTA Online, non : Rockstar a bloqué Linux en 2024."),
    252950: None,  # Rocket League : entrée écrite à la main (Heroic), gardée telle quelle
}


def lire(url, essais=3):
    for n in range(essais):
        try:
            requete = urllib.request.Request(url, headers={"User-Agent": "grenos-catalogue/1.0"})
            with urllib.request.urlopen(requete, timeout=30) as reponse:
                return json.loads(reponse.read().decode("utf-8"))
        except Exception as souci:  # noqa: BLE001 — réseau : on réessaie
            if n + 1 == essais:
                raise
            print("  nouvel essai (%s)" % souci, file=sys.stderr)
            time.sleep(4 * (n + 1))


def couleurs(nom):
    """Deux couleurs stables tirées du nom : le repli quand l'image manque."""
    teinte = int(hashlib.sha1(nom.encode("utf-8")).hexdigest()[:4], 16) / 65535.0
    fonce = colorsys.hls_to_rgb(teinte, 0.13, 0.45)
    vif = colorsys.hls_to_rgb((teinte + 0.12) % 1, 0.52, 0.75)
    return ["#%02x%02x%02x" % tuple(int(c * 255) for c in fonce),
            "#%02x%02x%02x" % tuple(int(c * 255) for c in vif)]


def entree_steam(appid, donnees, anticheat):
    # Les marques déposées alourdissent une jaquette sans rien dire de plus :
    # la bibliothèque de Steam les retire aussi.
    nom = re.sub(r"\s*[™®©]", "", donnees["name"]).strip()
    gratuit = donnees.get("is_free", False)
    genres = [g["description"] for g in donnees.get("genres", [])][:1]
    genre = " · ".join(genres + (["Gratuit"] if gratuit else []))
    natif = bool(donnees.get("platforms", {}).get("linux"))

    etat, note = ("natif", "Version Linux native, par Steam.") if natif else \
                 ("proton", "Jeu Windows : il passe par Proton dans Steam. ProtonDB dit jeu par jeu ce qui marche.")
    if anticheat:
        statut = anticheat.get("status", "")
        systemes = ", ".join(anticheat.get("anticheats") or []) or "son anti-triche"
        if statut in ("Denied", "Broken", "Planned"):
            etat = "indisponible"
            note = ("L'éditeur n'a pas activé %s sous Linux : le jeu en ligne refuse de démarrer. "
                    "Aucun réglage n'y change rien." % systemes)
        elif statut in ("Supported", "Running"):
            note = ("%s Anti-triche (%s) activé pour Linux."
                    % ("Version Linux native." if natif else "Par Proton dans Steam.", systemes))
    if appid in NOTES_A_LA_MAIN and NOTES_A_LA_MAIN[appid]:
        etat, note = NOTES_A_LA_MAIN[appid]

    cle = "steam-%d" % appid
    entree = {"cle": cle, "nom": nom, "genre": genre or "Jeu", "etat": etat,
              "source": "steam", "steam": appid, "couleurs": couleurs(nom), "note": note}
    if etat == "indisponible":
        entree["source"] = "aucune"
    return entree


def main():
    essai = "--essai" in sys.argv
    with io.open(CHEMIN, encoding="utf-8") as fichier:
        catalogue = json.load(fichier)

    print("AreWeAntiCheatYet…")
    awacy = {}
    for jeu in lire(AWACY):
        steam = (jeu.get("storeIds") or {}).get("steam")
        if steam:
            awacy[int(steam)] = jeu

    # Les entrées écrites à la main : tout ce qui n'a pas été produit ici.
    a_la_main = [j for j in catalogue["jeux"] if not j["cle"].startswith("steam-")]
    deja = {j.get("steam") for j in a_la_main if j.get("source") in ("steam", "heroic")}
    # Les anciennes entrées Steam écrites à la main sont remplacées par le calcul,
    # sauf celles que NOTES_A_LA_MAIN garde explicitement.
    a_la_main = [j for j in a_la_main if j.get("source") != "steam"
                 and not (j.get("source") == "aucune" and j.get("steam"))]

    calcules, refuses = [], []
    for appid in APPIDS:
        if appid in deja and appid in NOTES_A_LA_MAIN and NOTES_A_LA_MAIN[appid] is None:
            continue
        reponse = lire(STORE % appid).get(str(appid), {})
        if not reponse.get("success") or reponse["data"].get("type") != "game":
            refuses.append(appid)
            continue
        calcules.append(entree_steam(appid, reponse["data"], awacy.get(appid)))
        print("  %-8d %-40s %s" % (appid, calcules[-1]["nom"][:40], calcules[-1]["etat"]))
        time.sleep(1.6)  # l'API du magasin limite à ~200 requêtes par 5 minutes

    # Les jeux écrits à la main d'abord (ils portent la une), puis le calcul.
    vus = set()
    jeux = []
    for j in a_la_main + calcules:
        if j["cle"] not in vus:
            vus.add(j["cle"])
            jeux.append(j)
    catalogue["jeux"] = jeux
    print("%d jeux au catalogue (%d calculés), %d appid(s) refusé(s) : %s"
          % (len(jeux), len(calcules), len(refuses), refuses))
    if not essai:
        with io.open(CHEMIN, "w", encoding="utf-8", newline="\n") as fichier:
            json.dump(catalogue, fichier, ensure_ascii=False, indent=1)
            fichier.write("\n")
        print("écrit :", CHEMIN)


if __name__ == "__main__":
    main()
