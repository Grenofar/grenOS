#!/usr/bin/env python3
"""Ce que la machine d'intégration ouvre vraiment quand elle tape dans le menu.

POURQUOI CE FICHIER EXISTE.

La machine d'essai ouvre nos applications comme quelqu'un le ferait : elle
ouvre le menu, tape quelques lettres, appuie sur Entrée. Le bloc de GrenPlace
portait ce commentaire, écrit le jour où il a été posé :

    « grenp » ne designe que lui

C'était vrai. Le 14 octobre, deux applications neuves sont arrivées —
**grenHub** et **grenPerf** — et « grenp » a cessé de ne désigner que lui :
`grenperf` passe avant `grenplace` dans l'ordre alphabétique. La machine a
donc ouvert grenPerf en croyant ouvrir GrenPlace, la preuve bloquante
`grenplace ouvert` n'est jamais venue, et la construction a été refusée.

Aucune étape n'était fautive. Le défaut tenait tout entier dans une hypothèse
vraie au moment où on l'écrit, et fausse dès qu'une application s'ajoute — et
cette hypothèse était **écrite en toutes lettres dans un commentaire**, où
aucune machine ne la relisait.

CE QUE LE GARDE FAIT

Il applique la **règle exacte du menu** (`grenos-shell` : le filtre est
cherché dans le nom *et* dans la note, et la liste est triée par nom), et il
dit quelle entrée l'Entrée choisirait. Une séquence qui en désigne plusieurs
est refusée : ce n'est pas une erreur aujourd'hui, c'est une erreur en
puissance, et elle se paiera à la prochaine application.

Les séquences sont déclarées dans `ci-demarrage.sh` par un commentaire :

    # recherche-menu: GrenPlace
    for touche in g r e n p l ret; do

Pourquoi une déclaration plutôt qu'une devinette : toutes les suites de
touches ne sont pas des recherches. La machine tape aussi un nom de compte et
un renommage de dossier. Deviner lesquelles sont des recherches nous ramènerait
à une hypothèse — exactement ce qu'on vient de payer.

    python3 linux/tools/verif-recherches.py
"""
import glob
import io
import os
import re
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PARCOURS = os.path.join(RACINE, "scripts", "ci-demarrage.sh")
ENTREES = os.path.join(RACINE, "linux", "config", "includes.chroot",
                       "usr", "share", "applications", "*.desktop")

#: `sendkey` nomme la touche PHYSIQUE, en QWERTY. La machine d'essai est en
#: AZERTY : la touche physique « q » y écrit un « a ». Sans cette table, le
#: garde jugerait une recherche que personne ne tape — et le parcours porte
#: déjà la trace de ce piège : « install » s'y écrit `i n s t q l l`.
AZERTY = {"q": "a", "a": "q", "w": "z", "z": "w", "m": ",", "semicolon": "m"}

MARQUEUR = "# recherche-menu:"


def applications():
    """Ce que le menu montre : son nom et sa note, comme `grenos-shell` les lit."""
    vues = []
    for chemin in sorted(glob.glob(ENTREES)):
        champs = {}
        for ligne in io.open(chemin, encoding="utf-8"):
            if "=" in ligne and not ligne.startswith("["):
                cle, _, valeur = ligne.partition("=")
                champs.setdefault(cle.strip(), valeur.strip())
        # Le menu saute ce qui se cache — même règle que `grenos-shell`.
        if champs.get("NoDisplay", "").lower() == "true":
            continue
        if champs.get("Hidden", "").lower() == "true":
            continue
        vues.append((champs.get("Name", ""), champs.get("Comment", "")))
    return vues


def texte_tape(touches):
    """Les lettres réellement écrites, une fois passées au clavier français."""
    lettres = []
    for touche in touches:
        if touche == "ret":
            break
        lettres.append(AZERTY.get(touche, touche))
    return "".join(lettres)


def choisies(filtre, vues):
    """Les entrées que ce filtre retient, dans l'ordre où le menu les range.

    C'est la règle de `grenos-shell` recopiee telle quelle : le filtre est
    cherché dans le nom ET dans la note, et la liste est triée par nom. Une
    règle differente ici ferait un garde qui juge un autre menu que le nôtre.
    """
    filtre = filtre.lower()
    gardees = [(nom, note) for nom, note in vues
               if filtre in nom.lower() or filtre in note.lower()]
    return sorted(gardees, key=lambda e: e[0].lower())


def recherches():
    """Les séquences déclarées dans le parcours, avec l'application visée."""
    trouvees = []
    lignes = io.open(PARCOURS, encoding="utf-8").read().splitlines()
    attendu = None
    for numero, ligne in enumerate(lignes, 1):
        nu = ligne.strip()
        if nu.startswith(MARQUEUR):
            attendu = nu[len(MARQUEUR):].strip()
            continue
        trouve = re.match(r"for touche in (.+); do", nu)
        if trouve and attendu is not None:
            trouvees.append((numero, attendu, trouve.group(1).split()))
            attendu = None
    return trouvees


def juger():
    vues = applications()
    fautes = []
    lignes = []
    for numero, attendu, touches in recherches():
        filtre = texte_tape(touches)
        retenues = choisies(filtre, vues)
        noms = [nom for nom, _ in retenues]
        if not retenues:
            fautes.append((numero, filtre, attendu,
                           "ne designe AUCUNE application"))
        elif len(retenues) > 1:
            fautes.append((numero, filtre, attendu,
                           "en designe %d : %s — l Entree prendrait « %s »"
                           % (len(noms), ", ".join(noms), noms[0])))
        elif noms[0] != attendu:
            fautes.append((numero, filtre, attendu,
                           "ouvrirait « %s »" % noms[0]))
        else:
            lignes.append("  « %s » -> %s" % (filtre, noms[0]))
    return lignes, fautes


def main():
    lignes, fautes = juger()
    for numero, filtre, attendu, reproche in fautes:
        print("ci-demarrage.sh:%d: « %s » devait ouvrir « %s » et %s"
              % (numero, filtre, attendu, reproche), file=sys.stderr)
    if fautes:
        print("recherches : %d sequence(s) ambigues. Une application de plus "
              "et la machine ouvre la mauvaise fenetre en silence."
              % len(fautes), file=sys.stderr)
        return 1
    for ligne in lignes:
        print(ligne)
    print("recherches : %d sequence(s), chacune ne designe qu une application"
          % len(lignes))
    return 0


if __name__ == "__main__":
    sys.exit(main())
