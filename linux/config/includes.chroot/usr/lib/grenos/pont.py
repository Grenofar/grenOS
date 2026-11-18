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


FONCTIONS = {
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
