#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Y a-t-il des lignes de service de systemd sur cette photo d'extinction ?

POURQUOI CE CONTRÔLE EXISTE

Le 1er octobre, j'ai fait revenir le journal d'arrêt — quarante lignes — en
déplaçant une unité, et **rien dans la CI ne l'a dit**. Elle a imprimé
« extinction 3 : couleur dominante 92 % » au lieu de 100, et c'est tout ; aucune
étape n'a rougi. Je l'ai découvert en ouvrant la photo.

« La couleur dominante » n'est pas la bonne mesure, et le jour où la splash
dessinera notre fond elle le dira encore moins : un fond d'écran bleu n'est pas
uniforme, donc il ferait chuter ce pourcentage exactement comme un journal.
Le garde refuserait alors ce qu'on cherche à obtenir.

CE QU'ON MESURE À LA PLACE

Les lignes de systemd portent une marque que rien d'autre ne porte : un
`[  OK  ]` **vert vif** et un `[FAILED]` **rouge vif**, en couleurs pures de
console. Notre fond d'écran est bleu nuit, notre logo blanc et bleu, notre
barre de progression bleue : aucun de ces pixels n'est un vert ni un rouge
saturés.

On compte donc ces deux couleurs. C'est spécifique au défaut, et ça laisse
passer n'importe quel habillage — y compris celui qu'on n'a pas encore.

    python3 ci-lignes-arret.py ecran-extinction-1.ppm [...]

Rend 0 si aucune photo ne porte de marque, 1 sinon. Sans argument lisible, il
rend 0 et le dit : une photo absente n'est pas une preuve d'absence de journal,
et le prétendre serait la faute du `grep -c` de la semaine dernière.
"""
import sys


def lire_ppm(chemin):
    """Rend (largeur, hauteur, octets RVB), ou None si ce n'est pas lisible."""
    try:
        with open(chemin, "rb") as f:
            donnees = f.read()
    except OSError:
        return None
    if not donnees.startswith(b"P6"):
        return None
    # En-tête P6 : trois entiers, les commentaires commencent par #.
    champs, i = [], 2
    while len(champs) < 3 and i < len(donnees):
        while i < len(donnees) and donnees[i:i + 1].isspace():
            i += 1
        if donnees[i:i + 1] == b"#":
            while i < len(donnees) and donnees[i:i + 1] != b"\n":
                i += 1
            continue
        j = i
        while j < len(donnees) and not donnees[j:j + 1].isspace():
            j += 1
        try:
            champs.append(int(donnees[i:j]))
        except ValueError:
            return None
        i = j
    if len(champs) < 3:
        return None
    largeur, hauteur, _maxi = champs
    return largeur, hauteur, donnees[i + 1:i + 1 + largeur * hauteur * 3]


def marques(pixels):
    """Combien de pixels d'un vert et d'un rouge de console.

    SEUL LE VERT COMPTE, et c'est une correction faite avant de poser ce garde.

    La première version jugeait sur le vert ET le rouge. Exercée sur de vraies
    captures, elle a refusé **nos propres écrans** : 316 pixels rouges sur le
    bureau (le logo de Firefox) et 1654 sur la page Jeux. Un garde qui refuse
    une construction pour la mauvaise raison coûte trente minutes et fait
    douter de tous les autres — c'est arrivé quatre fois à `verif-commandes.py`.

    Le `[  OK  ]` vert est la vraie signature : il apparaît une fois par ligne,
    donc des milliers de pixels (3024 mesurés sur la photo de la régression du
    1er octobre), et il vaut **zéro** sur chacun de nos écrans — bureau, page
    Jeux, et l'écran de mise à jour avec notre fond, notre logo et notre barre.
    Le `[FAILED]` rouge est rapporté pour la lecture, jamais pour le verdict.
    """
    verts = rouges = 0
    for d in range(0, len(pixels) - 2, 3):
        r, v, b = pixels[d], pixels[d + 1], pixels[d + 2]
        if v > 120 and r < 100 and b < 100:
            verts += 1
        elif r > 120 and v < 100 and b < 100:
            rouges += 1
    return verts, rouges


def main():
    chemins = sys.argv[1:]
    if not chemins:
        print("lignes d arret : aucune photo donnee, rien a dire")
        return 0

    vues = 0
    coupable = None
    for chemin in chemins:
        image = lire_ppm(chemin)
        if image is None:
            continue
        vues += 1
        _l, _h, pixels = image
        verts, rouges = marques(pixels)
        nom = chemin.rsplit("/", 1)[-1]
        if verts > 300:
            print("lignes d arret : %s porte %d pixels de [ OK ] vert"
                  " (%d rouges, pour information)" % (nom, verts, rouges))
            coupable = coupable or nom
        else:
            print("lignes d arret : %s, aucune marque de service"
                  " (%d verts, %d rouges)" % (nom, verts, rouges))

    if vues == 0:
        print("lignes d arret : aucune photo lisible — rien n est prouve")
        return 0
    if coupable:
        print("lignes d arret : le journal de systemd est a l ecran pendant"
              " l extinction")
        return 1

    # UN ECRAN PROPRE SUR DEUX PHOTOS NE PROUVE RIEN, ET CELA M A FAIT ANNONCER
    # UNE VICTOIRE QUI N EN ETAIT PAS UNE.
    #
    # La machine d essai meurt a une vitesse qui varie du simple au decuple.
    # Quatre tours de suite, le meme jour :
    #
    #     3 captures  -> 3024 pixels verts
    #     1 capture   ->    0          <- « le journal est parti », ai-je ecrit
    #     10 captures ->  420
    #     3 captures  -> 3276
    #
    # Le tour ou j ai annonce la reparation n avait pris QU UNE photo : l arret
    # s est termine avant que le journal n ait eu le temps d apparaitre. Zero
    # vert n y voulait pas dire « rien ne s affiche », mais « on n a pas
    # regarde assez longtemps ».
    #
    # C est exactement la faute que ce depot documente depuis le 26 septembre :
    # absence de preuve n est pas preuve d absence. On refuse donc de conclure
    # quand la fenetre d observation a ete trop courte, au lieu de rendre un
    # vert qui ne veut rien dire.
    if vues < 3:
        print("lignes d arret : %d photo(s) seulement — la machine est partie"
              " trop vite, on ne peut RIEN conclure" % vues)
        return 0
    print("lignes d arret : %d photo(s), aucune ligne de service a l ecran" % vues)
    return 0


if __name__ == "__main__":
    sys.exit(main())
