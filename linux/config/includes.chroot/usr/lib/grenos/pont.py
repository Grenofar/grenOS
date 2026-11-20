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
    """Le paquet exact que Debian conseille pour cette carte (nvidia-detect).

    Le dépôt de NVIDIA branché par grenOS (20 novembre) passe d'abord : son
    `nvidia-driver` gère toutes les cartes, RTX 50 comprises, quand celui de
    Debian (550) s'arrête avant.
    """
    import os
    import re
    import subprocess
    if os.path.isfile("/usr/share/grenos/depots/nvidia-driver.list"):
        return {"paquet": "nvidia-driver"}
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


# ---- Réglages ---------------------------------------------------------------------
#
# Chaque page des Réglages lit et agit par les modules d'avant (ecran.py,
# son.py, heure.py, composants.py, securite.py, maj.py, materiel.py). Les
# phrases sont celles de l'ancienne fenêtre, au mot près, et les lignes série
# aussi : « reglages son : melangeur », « reglages securite : », « reglages
# composants : » sont des preuves EXIGÉES par la construction.

def _nombre(valeur, decimales=1):
    return ("%.*f" % (decimales, valeur)).replace(".", ",")


def reglages_ecran():
    import ecran
    sorties = ecran.sorties()
    if not sorties:
        return {"ecran": None, "modes": []}
    premier = sorties[0]
    modes = sorted(premier["modes"], key=lambda m: ecran.surface(m["taille"]), reverse=True)
    # EN MACHINE VIRTUELLE, RIEN « À ESSAYER » (19 novembre). Grenofar :
    # « j'ai l'impression que ça change rien que je passe en 360 (j'ai un
    # écran 300 Hz) ». C'est vrai : l'écran de la VM est virtuel, et c'est
    # VirtualBox qui le recopie sur le vrai écran, à SA cadence. Proposer
    # 360 Hz y était une promesse sans rien derrière. On n'offre plus que ce
    # que l'écran virtuel annonce, et la page dit pourquoi.
    virtuelle = _virtuelle()
    rendu = []
    for mode in modes:
        rendu.append({"taille": mode["taille"], "taux": [
            {"arrondi": a, "brut": b, "annoncee": bool(n)}
            for a, b, n in ecran.frequences_proposees(mode["taux"], () if virtuelle else ecran.FREQUENCES_USUELLES)]})
    return {"ecran": {"nom": premier["nom"], "actuelle": premier["actuelle"] or "",
                      "taux": premier["taux"] or ""}, "modes": rendu, "virtuelle": virtuelle}


def _virtuelle():
    """Le nom de l'hyperviseur (« oracle », « kvm »…), ou "" sur un vrai PC."""
    import subprocess
    try:
        r = subprocess.run(["systemd-detect-virt"], capture_output=True, text=True, timeout=3)
    except (OSError, subprocess.SubprocessError):
        return ""
    nom = r.stdout.strip()
    return "" if r.returncode != 0 or nom in ("", "none") else nom


def reglages_ecran_appliquer(nom, taille, brut, annoncee):
    """Essaie une définition SANS la noter, et dit comment revenir.

    La page compte 15 secondes : « Garder » note le choix
    (reglages_ecran_garder) ; sans réponse — écran noir, plus de signal —
    elle revient d'elle-même à l'ancien (reglages_ecran_revenir). Comme
    Windows. Sur le PC de Grenofar, 360 Hz coupait le signal et le choix
    était déjà noté pour la session suivante (20 novembre).
    """
    import ecran
    avant = next((s for s in ecran.sorties() if s["nom"] == nom), None)
    precedent = {"taille": avant["actuelle"], "brut": avant["taux"]} if avant else None
    if annoncee:
        souci, mode = ecran.appliquer(nom, taille, brut, garder=False), taille
        taux = brut
    else:
        largeur, hauteur = taille.split("x")
        souci, mode = ecran.essayer_mode(nom, largeur, hauteur, float(brut), garder=False)
        taux = ""
    return {"souci": souci or "", "precedent": precedent, "mode": mode, "taux": taux}


def reglages_ecran_garder(nom, mode, taux):
    import ecran
    ecran.noter(nom, mode, taux)
    return {"souci": ""}


def reglages_ecran_revenir(nom, taille, brut):
    import ecran
    return {"souci": ecran.appliquer(nom, taille, brut, garder=False) or ""}


def reglages_composants():
    import composants
    if not composants.present():
        return {"present": False, "composants": []}
    return {"present": True, "composants": composants.composants()}


def reglages_composants_chercher():
    import composants
    reussi, _dit = composants.rafraichir()
    if not reussi:
        return {"texte": "La liste des constructeurs n'a pas pu être relue. Vérifie le réseau."}
    attendues = composants.mises_a_jour()
    if not attendues:
        return {"texte": "Tous les composants sont à jour."}
    return {"texte": "\n".join("%s : %s vers %s" % (m["nom"], m["de"], m["vers"]) for m in attendues)}


def reglages_securite():
    import securite
    return {"resume": securite.resume(), "present": bool(securite.present()),
            "signatures": bool(securite.signatures()["presentes"])}


def reglages_son():
    """L'état du son et le diagnostic, en phrases, comme la page d'avant."""
    import materiel
    import son
    lu = materiel.son_et_video()
    if lu["profils_alsa"]:
        profils = f"{lu['profils_alsa']} profils ALSA trouvés."
    else:
        profils = "Aucun profil ALSA trouvé : sur un portable récent, le son peut rester muet sans eux."
    if lu["carte"]:
        carte = f"Carte son : {lu['carte']}" + (f", pilotée par {lu['module']}" if lu["module"] else "") + "."
    else:
        carte = "Aucune carte son détectée."
    decodeur = "Le décodeur vidéo est installé." if lu["decodeur"] else "Aucun décodeur vidéo installé."
    if lu["pilotes_video"]:
        pilotes = (f"{lu['pilotes_video']} pilote{'s' if lu['pilotes_video'] > 1 else ''} "
                   "de décodage vidéo sur la carte graphique.")
    else:
        pilotes = ("Aucun pilote de décodage vidéo sur la carte graphique : "
                   "les vidéos seront décodées par le processeur.")
    canaux = lu.get("melangeur", [])
    coupes = [c for c in canaux if c["coupe"]]
    bas = [c for c in canaux if not c["coupe"] and c["niveau"] is not None and c["niveau"] < 10]
    if coupes:
        melangeur = ("LE SON EST COUPÉ dans le mélangeur de la carte : "
                     + ", ".join(c["nom"] for c in coupes)
                     + ". C'est la cause la plus probable d'un silence complet. "
                       "Ouvre le mélangeur ci-dessus et remonte-les.")
    elif bas:
        melangeur = ("Le mélangeur de la carte est très bas : "
                     + ", ".join("%s à %d %%" % (c["nom"], c["niveau"]) for c in bas) + ".")
    elif canaux:
        melangeur = ("Le mélangeur de la carte est ouvert ("
                     + ", ".join("%s %d %%" % (c["nom"], c["niveau"]) for c in canaux
                                 if c["niveau"] is not None) + ").")
    else:
        melangeur = "Aucun canal de mélangeur lisible sur cette carte."
    temps_reel = lu.get("temps_reel")
    if temps_reel is True:
        priorite = "PipeWire tourne en temps réel : le son tiendra quand la machine peine."
    elif temps_reel is False:
        priorite = "PipeWire ne tourne PAS en temps réel : le son hachera dès que la machine peine."
    else:
        priorite = "PipeWire ne tourne pas."
    try:
        verdict_sortie, _soucis = son.diagnostic()
    except Exception:  # noqa: BLE001
        verdict_sortie = "Impossible de lire la sortie audio."
    lignes = [
        "reglages son : %d profils ALSA, carte %s, decodeur %s, %d pilotes video"
        % (lu["profils_alsa"], lu["carte"] or "aucune",
           "present" if lu["decodeur"] else "absent", lu["pilotes_video"]),
        "reglages son : melangeur %s" % (", ".join(
            "%s %s %s" % (c["nom"], "?" if c["niveau"] is None else "%d%%" % c["niveau"],
                          "COUPE" if c["coupe"] else "ouvert") for c in canaux) or "illisible"),
        "reglages son : sortie — %s" % verdict_sortie,
    ]
    return {
        "sorties": [[n, d] for n, d in son.sorties()],
        "courante": son.sortie_actuelle(),
        "volume": son.volume(),
        "muet": son.muet(),
        "diagnostic": "\n".join([verdict_sortie, melangeur, priorite, carte, profils, decodeur, pilotes]),
        "lignes_serie": lignes,
    }


def reglages_son_regler(geste, valeur=""):
    import son
    if geste == "sortie":
        son.choisir_sortie(valeur)
    elif geste == "volume":
        son.regler_volume(float(valeur))
    elif geste == "muet":
        son.basculer_muet()
    return {}


def reglages_heure():
    import heure
    etat = heure.etat()
    return {"fuseau": etat["fuseau"], "automatique": bool(etat["automatique"]),
            "fuseaux": heure.fuseaux()}


def reglages_heure_regler(geste, valeur):
    import heure
    if geste == "fuseau":
        souci = heure.choisir_fuseau(valeur)
        return {"texte": souci or f"Fuseau : {valeur}."}
    if geste == "automatique":
        souci = heure.horloge_automatique(bool(valeur))
        return {"texte": souci or ("L'heure se règle toute seule." if valeur
                                   else "L'heure ne se règle plus toute seule.")}
    texte = str(valeur).strip()
    if not texte:
        return {"texte": "Écris une date et une heure."}
    souci = heure.regler(texte)
    return {"texte": souci or f"Heure réglée sur {texte}."}


def reglages_materiel():
    import materiel
    infos = materiel.resume()
    virtuelle = infos["virtuelle"]
    lignes = [
        ["Processeur", f"{infos['coeurs']} cœurs"],
        ["Modèle", infos["processeur"][:52]],
        ["Mémoire vive", f"{_nombre(infos['memoire'])} Go ({_nombre(infos['memoire_libre'])} Go libres)"],
        ["Carte graphique", infos["carte"][:52]],
        ["Pilote", infos["pilote"][:52]],
        ["Système sur", f"{infos['disque']:.0f} Go ({infos['disque_libre']:.0f} Go libres)"],
    ]
    vus = infos.get("disques") or []
    for index, d in enumerate(vus):
        nom = d["modele"] or d["nom"]
        lignes.append(["Disque" if index == 0 and len(vus) == 1 else f"Disque {index + 1}",
                       f"{nom[:34]} — {materiel.genre_du_disque(d)} ({d['nom']})"])
    if not vus:
        lignes.append(["Disques", "aucun disque vu : rien à installer dessus"])
    explication = ("grenOS utilise d'office tout ce que la machine lui donne : tous les "
                   "cœurs, toute la mémoire, toute la mémoire vidéo. Il n'y a rien à débloquer.")
    if len(vus) > 1:
        explication += ("\n\nCette machine a plusieurs disques. L'installateur les propose "
                        "tous et demande lequel prendre : il n'efface jamais avant que tu "
                        "aies choisi.")
    if virtuelle:
        explication += (f"\n\nCette machine est virtuelle ({virtuelle}) : c'est l'hôte qui "
                        "décide de ce qu'elle voit. Pour lui en donner plus, éteins-la et "
                        "augmente la mémoire, les processeurs et la mémoire vidéo dans les "
                        "réglages de la machine.")
    return {"lignes": lignes, "explication": explication}


def reglages_maj():
    import maj
    attendus = maj.disponibles()
    quand = maj.derniere_verification()
    if attendus is None:
        resume = "Aucune vérification depuis le démarrage, ou pas de réseau."
    elif attendus == 0:
        resume = f"Rien à installer. Dernière vérification : {quand} UTC."
    else:
        resume = f"{attendus} paquet{'s' if attendus > 1 else ''} à installer. Vérifié le {quand} UTC."
    version = "grenOS"
    try:
        with open("/etc/os-release", encoding="utf-8") as fichier:
            for ligne in fichier:
                if ligne.startswith("PRETTY_NAME="):
                    version = ligne.split("=", 1)[1].strip().strip('"')
    except OSError:
        pass
    return {"resume": resume, "version": version}


FONCTIONS = {
    "reglages_ecran": reglages_ecran,
    "reglages_ecran_appliquer": reglages_ecran_appliquer,
    "reglages_ecran_garder": reglages_ecran_garder,
    "reglages_ecran_revenir": reglages_ecran_revenir,
    "reglages_composants": reglages_composants,
    "reglages_composants_chercher": reglages_composants_chercher,
    "reglages_securite": reglages_securite,
    "reglages_son": reglages_son,
    "reglages_son_regler": reglages_son_regler,
    "reglages_heure": reglages_heure,
    "reglages_heure_regler": reglages_heure_regler,
    "reglages_materiel": reglages_materiel,
    "reglages_maj": reglages_maj,
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
