#!/usr/bin/env python3
"""grenos-steam-bibliotheques, sans Steam (27 novembre).

    python3 linux/tools/essai-steam-bibliotheques.py

« Add Drive » de Steam Flatpak ne faisait rien : la partition donnée à
grenOS n'entrait pas dans Steam. Le veilleur l'inscrit lui-même dans
libraryfolders.vdf, Steam fermé. Ici : la lecture et l'ajout dans le format
de Steam (avec le vrai fichier de Grenofar), les SteamLibrary trouvées, et
une passe complète — rien tant que Steam tourne, une fois seulement, jamais
remise si la personne l'a retirée, rien sur un fichier illisible.
"""
import importlib.machinery
import importlib.util
import os
import sys
import tempfile

ICI = os.path.dirname(os.path.abspath(__file__))
CHEMIN = os.path.join(ICI, "../config/includes.chroot/usr/lib/grenos/grenos-steam-bibliotheques")
chargeur = importlib.machinery.SourceFileLoader("grenos_steam_bibliotheques", CHEMIN)
spec = importlib.util.spec_from_loader("grenos_steam_bibliotheques", chargeur)
m = importlib.util.module_from_spec(spec)
chargeur.exec_module(m)

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


# Le libraryfolders.vdf de Grenofar, le 27 novembre, tel que Steam l'écrit.
VRAI = '''"libraryfolders"
{
\t"0"
\t{
\t\t"path"\t\t"/home/grnfr/.var/app/com.valvesoftware.Steam/.local/share/Steam"
\t\t"label"\t\t""
\t\t"contentid"\t\t"7706822057761421463"
\t\t"totalsize"\t\t"0"
\t\t"update_clean_bytes_tally"\t\t"2147650754"
\t\t"time_last_update_verified"\t\t"1795539667"
\t\t"apps"
\t\t{
\t\t\t"228980"\t\t"50450161"
\t\t\t"767560"\t\t"6148358267"
\t\t}
\t}
}'''
MARQUE = '"libraryfolder"\n{\n\t"contentid"\t\t"1044388530973101259"\n\t"label"\t\t""\n}\n'

print("steam bibliothèques :")
verifier(m.chemins(VRAI) == ["/home/grnfr/.var/app/com.valvesoftware.Steam/.local/share/Steam"],
         "les bibliothèques d'un libraryfolders.vdf (pas les appids de \"apps\")")
verifier(m.contentid(MARQUE) == "1044388530973101259", "le contentid de la marque libraryfolder.vdf")
verifier(m.contentid('"libraryfolder"\n{\n}\n') is None, "une marque sans contentid : rien")

nouveau = m.ajouter(VRAI, "/mnt/Grenos-supdata/SteamLibrary", "1044388530973101259")
verifier(m.chemins(nouveau) == m.chemins(VRAI) + ["/mnt/Grenos-supdata/SteamLibrary"], "ajoutée après la bibliothèque de Steam")
verifier('\t"1"\n\t{\n\t\t"path"\t\t"/mnt/Grenos-supdata/SteamLibrary"' in nouveau, "au numéro suivant (1), à la tabulation de Steam")
verifier(nouveau.startswith(VRAI[:VRAI.rstrip().rfind("}")].rstrip("\n")) and nouveau.endswith("\t}\n}\n"),
         "rien d'autre ne bouge, l'accolade finale est remise")
verifier('"contentid"\t\t"1044388530973101259"' in nouveau and '"apps"\n\t\t{\n\t\t}' in nouveau,
         "le contentid de la marque, et aucun jeu")
encore = m.ajouter(nouveau, "/mnt/Jeux/SteamLibrary", "1")
verifier('\t"2"\n' in encore and len(m.chemins(encore)) == 3, "une troisième : numéro 2")
for mauvais, quoi in (("", "vide"), ('"autre"\n{\n}\n', "un autre fichier"), ('"libraryfolders"\n{\n\t"0"\n', "coupé")):
    try:
        m.ajouter(mauvais, "/mnt/X/SteamLibrary", "1")
        verifier(False, f"illisible ({quoi}) : refusé")
    except ValueError:
        verifier(True, f"illisible ({quoi}) : refusé")

with tempfile.TemporaryDirectory() as d:
    mnt, steam, etat = f"{d}/mnt", f"{d}/Steam", f"{d}/etat/steam-bibliotheques"
    os.makedirs(f"{mnt}/Grenos-supdata/SteamLibrary/steamapps")
    open(f"{mnt}/Grenos-supdata/SteamLibrary/libraryfolder.vdf", "w").write(MARQUE)
    os.makedirs(f"{mnt}/Vide")                                  # une partition sans SteamLibrary
    os.makedirs(f"{mnt}/Autre/SteamLibrary")                    # un dossier sans marque
    biblio = f"{mnt}/Grenos-supdata/SteamLibrary"

    verifier(m.trouvees((mnt,)) == [biblio], "trouvée : la SteamLibrary qui porte sa marque, seulement")
    verifier(m.trouvees((mnt,), uid=os.getuid() + 1) == [], "pas celle d'une autre personne")

    def passe(tourne=False):
        return m.passe(steam=steam, racines=(mnt,), etat=etat, tourne=lambda: tourne)

    verifier(passe() == [] and not os.path.exists(etat), "Steam jamais ouvert (pas de liste) : on attend, rien n'est noté")
    os.makedirs(f"{steam}/steamapps")
    os.makedirs(f"{steam}/config")
    for l in m.LISTES:
        open(f"{steam}/{l}", "w").write(VRAI)
    verifier(passe(tourne=True) == [] and open(f"{steam}/steamapps/libraryfolders.vdf").read() == VRAI,
             "Steam ouvert : rien n'est écrit (il réécrirait sa liste)")
    verifier(passe() == [biblio], "Steam fermé : ajoutée")
    verifier(all(biblio in m.chemins(open(f"{steam}/{l}").read()) for l in m.LISTES),
             "dans steamapps/ ET config/libraryfolders.vdf")
    verifier(all(open(f"{steam}/{l}.avant-grenos").read() == VRAI for l in m.LISTES), "l'ancienne liste gardée à côté")
    verifier(open(etat).read() == biblio + "\n", "notée comme faite")
    verifier(passe() == [] and m.chemins(open(f"{steam}/steamapps/libraryfolders.vdf").read()).count(biblio) == 1,
             "une seconde passe : rien en double")

    # La personne la retire de Steam : Steam réécrit sa liste sans elle.
    for l in m.LISTES:
        open(f"{steam}/{l}", "w").write(VRAI)
    verifier(passe() == [] and biblio not in m.chemins(open(f"{steam}/steamapps/libraryfolders.vdf").read()),
             "retirée par la personne : jamais remise")

    # Déjà dans Steam (Grenofar, ajoutée à la main) : notée, rien d'écrit.
    os.remove(etat)
    deja = m.ajouter(VRAI, biblio, "1044388530973101259")
    for l in m.LISTES:
        open(f"{steam}/{l}", "w").write(deja)
        os.remove(f"{steam}/{l}.avant-grenos")
    verifier(passe() == [] and open(f"{steam}/steamapps/libraryfolders.vdf").read() == deja
             and open(etat).read() == biblio + "\n", "déjà dans Steam : notée, la liste n'est pas touchée")

    # Une liste illisible : on n'y touche pas, et on réessaiera.
    os.remove(etat)
    for l in m.LISTES:
        open(f"{steam}/{l}", "w").write('"libraryfolders"\n{\n\t"0"\n')
    verifier(passe() == [] and not os.path.exists(etat)
             and open(f"{steam}/steamapps/libraryfolders.vdf").read() == '"libraryfolders"\n{\n\t"0"\n',
             "liste illisible : rien n'est écrit ni noté")

if FAUTES:
    print(f"steam bibliothèques : {FAUTES} faute(s) sur {CAS} cas")
    sys.exit(1)
print(f"steam bibliothèques : {CAS} cas, 0 faute")
