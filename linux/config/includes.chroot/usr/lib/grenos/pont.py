#!/usr/bin/env python3
"""Le pont entre grenos-ui (Rust) et nos bibliothèques Python, en JSON.

    pont.py <fonction> [arguments en JSON]

Le 18 novembre, les fenêtres GTK en Python passent dans grenos-ui. Leurs
BIBLIOTHÈQUES restent : `sante.py`, `jeux.py`, `magasin.py`… sont exercées
par nos essais (`tools/essai-*`), et les réécrire en Rust ferait deux vérités
qui finiraient par diverger — la leçon de `materiel.genre_du_disque()`. La
fenêtre (ce qu'on voit, ce qui reste ouvert) est en Rust ; le calcul ponctuel
reste là où il est éprouvé.

RÈGLES DU PONT :
- seules les fonctions de `FONCTIONS` répondent : la page ne peut pas faire
  exécuter n'importe quel code Python ;
- une réponse = UNE ligne JSON sur la sortie standard ; une fonction qui
  rapporte en continu (le diagnostic) écrit une ligne par étape, et la
  dernière porte `"fin": true` ;
- une erreur rend `{"erreur": "..."}` et le code 1, jamais une trace Python
  à l'écran.
"""
import json
import sys

sys.path.insert(0, "/usr/lib/grenos")


def ecrire(objet):
    sys.stdout.write(json.dumps(objet, ensure_ascii=False) + "\n")
    sys.stdout.flush()


# ---- grenHub ------------------------------------------------------------------

def hub_etat():
    """Les pavés de l'état, dans l'ordre où on veut les lire (grenos-hub)."""
    import materiel
    import sante

    def nombre(valeur, decimales=1):
        return ("%.*f" % (decimales, valeur)).replace(".", ",")

    machine = sante.demarrage()
    matos = materiel.resume()
    paves = [("La machine", [
        ("Nom", machine["nom_machine"]),
        ("Version", machine["version"]),
        ("Noyau Linux", machine["noyau"]),
        ("Allumée depuis", sante.duree_lisible(machine["debout"])),
        ("Genre", "machine virtuelle" if matos["virtuelle"] else "machine réelle"),
    ]), ("Processeur et mémoire", [
        ("Processeur", matos["processeur"]),
        ("Cœurs", matos["coeurs"]),
        ("Mémoire vive", "%s Go, dont %s Go libres"
         % (nombre(matos["memoire"]), nombre(matos["memoire_libre"]))),
    ]), ("Affichage", [
        ("Carte", matos["carte"] or "carte inconnue"),
        ("Pilote", matos["pilote"] or "aucun pilote nommé"),
    ])]

    disques = matos.get("disques") or []
    lignes = [(d["nom"], "%s%s" % (materiel.genre_du_disque(d),
                                   ", " + d["modele"] if d["modele"] else ""))
              for d in disques]
    paves.append(("Disques", lignes or [("Aucun", "aucun disque vu")]))

    cartes = sante.cartes_reseau()
    lignes = [(c["nom"], "%s, %s%s" % (c["genre"], c["etat"],
                                       ", " + c["adresse"] if c["adresse"] else ""))
              for c in cartes]
    paves.append(("Réseau", lignes or [("Aucune", "cette machine n'a pas de carte réseau")]))

    usb = sante.peripheriques_usb()
    lignes = [(a["nom"], ", ".join(a["pilotes"]) or "aucun pilote") for a in usb[:8]]
    if len(usb) > 8:
        lignes.append(("…", "et %d autre(s)" % (len(usb) - 8)))
    paves.append(("USB", lignes or [("Aucun", "rien de branché")]))

    chaleurs = sante.temperatures()
    souffle = sante.ventilateurs()
    lignes = [(nom, "%s °C" % nombre(degres, 0)) for nom, degres in chaleurs[:6]]
    # Le ventilateur sous la température : c'est ensemble qu'ils disent
    # quelque chose (un ventilateur à l'arrêt sur une machine chaude).
    lignes.extend((nom, "%s tr/min" % nombre(tours, 0)) for nom, tours in souffle[:4])
    pile = sante.batterie()
    if pile:
        lignes.append(("Batterie", "%s %%, %s" % (
            pile["charge"] if pile["charge"] is not None else "?", pile["etat"])))
    paves.append(("Température et énergie", lignes or
                  [("Aucune sonde", "cette machine n'en déclare pas")]))

    echecs = sante.unites_en_echec()
    paves.append(("Services", [("En échec", e) for e in echecs] or
                  [("Tout va bien", "aucun service en échec")]))
    return {
        "paves": [{"titre": t, "lignes": [[n, str(v)] for n, v in l]} for t, l in paves],
        "sondes": len(chaleurs),
        "ventilateurs": len(souffle),
    }


def hub_diagnostic():
    """Les essais, un par ligne au fil de l'eau, puis le verdict."""
    import sante
    mot = {sante.BON: "bon", sante.DOUTE: "a surveiller", sante.RATE: "en echec"}
    resultats = sante.diagnostic(rapporter=lambda r: ecrire(
        {"nom": r[0], "etat": mot[r[1]], "detail": r[2]}))
    etat, phrase = sante.verdict(resultats)
    ecrire({"fin": True, "etat": mot[etat], "phrase": phrase, "points": len(resultats)})


# ---- GrenPlace -----------------------------------------------------------------
#
# Les mêmes trois sources que l'ancienne fenêtre, dans le même ordre, pour que
# le magasin s'ouvre toujours : en ligne, la copie gardée, celle de l'image.

CATALOGUE_EN_LIGNE = (
    "https://tpqzhzuoyqpfairatdrw.supabase.co/storage/v1/object/public/catalogue/magasin.json"
)
CATALOGUE_LIVRE = "/usr/share/grenos/catalogue.json"


def _fiche_valable(application):
    import re
    if application.get("source") not in ("flatpak", "apt"):
        return False
    if not re.match(r"^[A-Za-z0-9][A-Za-z0-9._+-]*$", application.get("identifiant", "")):
        return False
    return bool(application.get("slug") and application.get("nom"))


def magasin_catalogue():
    import os
    import urllib.error
    import urllib.request
    cache = os.path.expanduser("~/.cache/grenos")
    garde = os.path.join(cache, "catalogue.json")
    try:
        requete = urllib.request.Request(CATALOGUE_EN_LIGNE, headers={"User-Agent": "GrenPlace"})
        with urllib.request.urlopen(requete, timeout=8) as reponse:
            donnees = json.loads(reponse.read().decode("utf-8"))
        applications = [a for a in donnees.get("applications", []) if _fiche_valable(a)]
        if applications:
            os.makedirs(cache, exist_ok=True)
            with open(garde, "w", encoding="utf-8") as fichier:
                json.dump(donnees, fichier, ensure_ascii=False)
            return {"applications": applications, "origine": "en ligne"}
    except (urllib.error.URLError, OSError, ValueError, TimeoutError):
        pass
    for chemin, origine in ((garde, "dernier connu"), (CATALOGUE_LIVRE, "livré avec l'image")):
        try:
            with open(chemin, encoding="utf-8") as fichier:
                donnees = json.load(fichier)
        except (OSError, ValueError):
            continue
        applications = [a for a in donnees.get("applications", []) if _fiche_valable(a)]
        if applications:
            return {"applications": applications, "origine": origine}
    return {"applications": [], "origine": "introuvable"}


def magasin_retards(applications):
    """Parmi ces fiches, celles qui ont une mise à jour (magasin.py)."""
    import magasin
    return {"retards": [a.get("slug") for a in magasin.a_mettre_a_jour(applications)]}


# ---- Jeux ------------------------------------------------------------------------

def jeux_vue():
    """Tout ce que la page Jeux affiche, lu en une fois (jeux.py, materiel.py).

    `vulkaninfo` peut prendre six secondes : la page s'affiche AVANT, et ceci
    arrive ensuite — la règle payée par l'écran vide de vingt-cinq secondes.
    """
    import jeux
    import materiel
    infos = materiel.resume()
    marche, detail = jeux.vulkan()
    paquet, conseil = jeux.pilote_propose()
    pose = jeux.gamemode_pose()
    return {
        "coeurs": infos["coeurs"],
        "memoire": infos["memoire"],
        "carte": (infos["carte"] or "")[:56],
        "pilote": (infos["pilote"] or "")[:56],
        "virtuelle": bool(jeux.machine_virtuelle()),
        "vulkan": marche,
        "vulkan_detail": detail,
        "pilote_paquet": paquet,
        "pilote_conseil": conseil,
        "gamemode_pose": pose,
        "gamemode_arme": jeux.gamemode_arme(),
        "option_steam": jeux.OPTION_STEAM,
        "outils": [dict(o, installe=jeux.installe(o["source"], o["identifiant"])) for o in jeux.OUTILS],
        "jeux": jeux.JEUX,
        "heroic": jeux.HEROIC,
        "epic_rocket": jeux.EPIC_ROCKET,
        "heroic_pose": jeux.heroic_pose(),
        "rocket_prete": jeux.rocket_league_prete(),
        "roblox_pose": jeux.roblox_pose(),
        "fortnite_pose": jeux.fortnite_pose(),
    }


def jeux_paquet_nvidia():
    """Le paquet exact que Debian conseille pour cette carte (nvidia-detect)."""
    import re
    import subprocess
    try:
        sortie = subprocess.run(["nvidia-detect"], capture_output=True, text=True, timeout=30).stdout
    except (OSError, subprocess.SubprocessError):
        return {"paquet": "nvidia-driver"}
    trouve = re.search(r"(nvidia-\S*driver\S*)", sortie)
    return {"paquet": trouve.group(1) if trouve else "nvidia-driver"}


def jeux_preparer_rocket():
    import jeux
    return {"manque": jeux.preparer_rocket_league() or ""}


def jeux_poser_fortnite():
    import jeux
    return {"manque": jeux.poser_fortnite() or ""}


FONCTIONS = {
    "jeux_vue": jeux_vue,
    "jeux_paquet_nvidia": jeux_paquet_nvidia,
    "jeux_preparer_rocket": jeux_preparer_rocket,
    "jeux_poser_fortnite": jeux_poser_fortnite,
    "magasin_catalogue": magasin_catalogue,
    "magasin_retards": magasin_retards,
    "hub_etat": hub_etat,
    "hub_diagnostic": hub_diagnostic,
}


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in FONCTIONS:
        ecrire({"erreur": "fonction inconnue"})
        return 1
    arguments = json.loads(sys.argv[2]) if len(sys.argv) > 2 else {}
    try:
        resultat = FONCTIONS[sys.argv[1]](**arguments)
    except Exception as souci:  # noqa: BLE001 — le pont ne laisse rien filer
        ecrire({"erreur": "%s : %s" % (type(souci).__name__, souci)})
        return 1
    if resultat is not None:
        ecrire(resultat)
    return 0


if __name__ == "__main__":
    sys.exit(main())
