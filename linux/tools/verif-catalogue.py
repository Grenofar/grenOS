#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Le catalogue de GrenPlace tient-il debout ?

POURQUOI CE FICHIER EXISTE — et c'est une image perdue, pas une precaution.

Le 24 septembre, une branche d'agent a REMPLACE le catalogue entier au lieu
d'y ajouter des fiches. Sa tache disait pourtant « no existing entry is
changed or removed ». Elle a ressuscite `grenos-magasin.desktop`, c'est-a-dire
le « Magasin » que Grenofar avait demande de remplacer par GrenPlace, et six
de ses fiches etaient des outils en ligne de commande (`tree`, `rsync`,
`ncdu`) dans une boutique a boutons « Installer ».

La CI etait verte. Elle disait que le fichier se lit et que les paquets
existent ; elle ne disait pas que l'agent avait fait ce qu'on demandait.

CE QUE CE CONTROLE VERIFIE, et chaque regle vient d'un defaut reel :

* le fichier se lit, et c'est une liste de fiches ;
* deux fiches ne partagent ni `slug` ni `identifiant` — un doublon donne deux
  boutons qui installent la meme chose, et la page des mises a jour compte
  faux ;
* chaque fiche porte exactement les neuf champs, ni plus ni moins. Un champ en
  trop est du bruit que personne ne lit ; un champ manquant fait planter la
  tuile a l'affichage, donc la page entiere ;
* le logo d'une fiche Flatpak designe SON identifiant. Une URL recopiee d'une
  fiche voisine montre le mauvais logo, et c'est exactement le genre de defaut
  qu'une relecture ne voit pas ;
* le `rayon` est l'un de ceux qui existent deja. Un rayon neuf cree une
  etagere vide a cote des autres.

CE QU'IL NE VERIFIE PAS, et il faut le dire : que l'application existe sur
Flathub. Cela demande le reseau, et un controle de construction qui depend
d'un service exterieur refuse l'image le jour ou ce service tousse. La
verification des identifiants se fait a la main, avant d'ecrire la fiche.
"""
import io
import json
import os
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CATALOGUE = os.path.join(RACINE, "data", "catalogue.json")

CHAMPS = {"slug", "nom", "rayon", "editeur", "description", "logo", "source",
          "identifiant", "site"}

# Le prefixe des logos Flathub, tel qu'il est ecrit dans les fiches existantes.
PREFIXE_LOGO = "https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/"

# LES RAYONS ET LES SOURCES SONT ECRITS ICI, PAS DEDUITS DU FICHIER.
#
# Et c'est tout l'enjeu : la premiere version les calculait depuis le
# catalogue qu'elle controlait. Un rayon invente devenait donc valide par sa
# seule presence, et le controle ne pouvait RIEN refuser — vrai, stable, et
# incapable de se tromper comme de trouver quoi que ce soit. L'essai l'a dit
# au premier tour, en refusant deux cas que le garde laissait passer.
#
# C'est l'assertion morte du 1er octobre, dans un fichier ecrit pour eviter ce
# genre de chose. Ajouter un rayon demande desormais de modifier cette ligne,
# ce qui est exactement le point : un rayon neuf est une decision, pas un
# effet de bord d'une fiche.
RAYONS = ("Bureautique", "Création", "Développement",
          "Intelligence artificielle", "Internet", "Jeux", "Système")
SOURCES = ("apt", "flatpak")


def charger(chemin=CATALOGUE):
    donnees = json.loads(io.open(chemin, encoding="utf-8").read())
    if isinstance(donnees, dict) and "applications" in donnees:
        return donnees["applications"]
    return donnees


def verifier(fiches):
    """La liste des fautes, chacune en une ligne lisible."""
    fautes = []
    if not isinstance(fiches, list) or not fiches:
        return ["le catalogue n'est pas une liste de fiches, ou il est vide"]

    vus_slug = {}
    vus_id = {}
    for rang, fiche in enumerate(fiches, 1):
        if not isinstance(fiche, dict):
            fautes.append("fiche %d : ce n'est pas un bloc" % rang)
            continue
        slug = fiche.get("slug") or "(sans slug)"

        manquants = CHAMPS - set(fiche)
        surplus = set(fiche) - CHAMPS
        if manquants:
            fautes.append("%s : champ(s) manquant(s) : %s"
                          % (slug, ", ".join(sorted(manquants))))
        if surplus:
            fautes.append("%s : champ(s) inconnu(s) : %s"
                          % (slug, ", ".join(sorted(surplus))))

        if slug in vus_slug:
            fautes.append("%s : slug deja utilise par la fiche %d"
                          % (slug, vus_slug[slug]))
        vus_slug[slug] = rang

        identifiant = fiche.get("identifiant")
        if identifiant:
            if identifiant in vus_id:
                fautes.append("%s : identifiant « %s » deja utilise par la fiche %d"
                              % (slug, identifiant, vus_id[identifiant]))
            vus_id[identifiant] = rang

        if fiche.get("source") not in SOURCES:
            fautes.append("%s : source « %s » inconnue du catalogue"
                          % (slug, fiche.get("source")))

        if fiche.get("rayon") not in RAYONS:
            fautes.append("%s : rayon « %s » inconnu du catalogue"
                          % (slug, fiche.get("rayon")))

        # Le logo d'une fiche FLATPAK doit designer SON identifiant.
        #
        # ET SEULEMENT FLATPAK, ce que la premiere version de ce controle
        # ignorait : elle a refuse six fiches correctes. `chromium`, `baobab`,
        # `simple-scan`, `transmission`, `filezilla` et `virt-manager` sont
        # des paquets APT — leur identifiant est un nom de paquet Debian — et
        # leur logo est EMPRUNTE a Flathub, faute d'en avoir un ailleurs.
        # C'est deliberé et ce n'est pas une faute.
        #
        # Un garde trop strict ne protege pas mieux : il refuse du travail
        # juste, donc on finit par le desactiver, et il ne garde plus rien.
        logo = fiche.get("logo") or ""
        if identifiant and fiche.get("source") == "flatpak" \
                and logo.startswith(PREFIXE_LOGO):
            attendu = PREFIXE_LOGO + identifiant + ".png"
            if logo != attendu:
                fautes.append("%s : le logo ne designe pas son identifiant"
                              % slug)
    return fautes


def main(argv):
    chemin = argv[1] if len(argv) > 1 else CATALOGUE
    try:
        fiches = charger(chemin)
    except (OSError, ValueError) as souci:
        print("::error::le catalogue ne se lit pas : %s" % souci)
        return 1
    fautes = verifier(fiches)
    for faute in fautes:
        print("::error file=linux/data/catalogue.json::%s" % faute)
    if fautes:
        print("Un catalogue abime donne des boutons qui installent la mauvaise "
              "chose, et une CI verte ne le dirait pas.", file=sys.stderr)
        return 1
    print("catalogue : %d fiche(s), aucun doublon, chaque fiche a ses neuf champs"
          % len(fiches))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
