# -*- coding: utf-8 -*-
"""Les onglets de l'explorateur : lesquels, dans quel ordre, lequel est actif.

**Aucun GTK ici, et aucun accès au disque.** Ce module ne tient que la liste
et l'histoire de chaque onglet. C'est ce qui permet de l'exercer sans écran,
et c'est la même raison qu'avait `premier.py` d'exister avant la fenêtre :
l'explorateur est devenu l'explorateur par défaut le 7 octobre, et une
refonte de sa fenêtre est le genre de travail qui casse ce qui marchait.

CE QUE LES ONGLETS NE CHANGENT PAS, et c'est délibéré : il n'y a qu'UNE liste
de fichiers à l'écran. Un onglet n'est pas une seconde vue, c'est un dossier
mémorisé avec son histoire. De l'extérieur cela se comporte comme les onglets
de Windows ; de l'intérieur, cela ne touche ni au modèle, ni au filtre, ni au
tri — donc rien de ce qui est déjà prouvé ne peut tomber.

TROIS RÈGLES, et chacune répond à une façon de perdre son travail :

1. **Le dernier onglet ne se ferme pas.** Fermer le dernier laisserait une
   fenêtre sans liste, c'est-à-dire un explorateur vide qui a l'air cassé.
   Windows ferme la fenêtre ; nous gardons l'onglet, parce que Ctrl+W frappé
   deux fois de suite ne doit pas faire disparaître la fenêtre par surprise.
2. **Chaque onglet garde SON histoire.** Sans cela, Précédent dans un onglet
   renverrait au dossier d'un autre — un bouton qui fait autre chose que ce
   qu'il dit.
3. **L'onglet actif existe toujours.** Après une fermeture, c'est le voisin
   de gauche : c'est ce que fait Windows, et c'est le seul choix qui ne
   déplace pas le regard d'un bout à l'autre de la barre.
"""
import os


class Onglet(object):
    """Un dossier, et le chemin qu'on a pris pour y arriver."""

    def __init__(self, dossier):
        self.dossier = dossier
        self.histoire = [dossier]
        self.rang = 0

    def aller(self, chemin, memoriser=True):
        self.dossier = chemin
        if not memoriser:
            return
        # On coupe ce qui était « en avant » : partir ailleurs depuis le
        # milieu de l'histoire efface le futur, comme dans un navigateur.
        del self.histoire[self.rang + 1:]
        self.histoire.append(chemin)
        self.rang = len(self.histoire) - 1

    def precedent(self):
        if self.rang <= 0:
            return None
        self.rang -= 1
        self.dossier = self.histoire[self.rang]
        return self.dossier

    def suivant(self):
        if self.rang + 1 >= len(self.histoire):
            return None
        self.rang += 1
        self.dossier = self.histoire[self.rang]
        return self.dossier

    def intitule(self):
        """Le nom court du dossier, ou « / » à la racine.

        `os.path.basename("/")` rend une chaîne vide, et un onglet sans nom
        serait un onglet invisible : on ne pourrait plus y revenir.
        """
        nom = os.path.basename(self.dossier.rstrip("/"))
        return nom or "/"


class Rangee(object):
    """La rangée d'onglets, et celui qui est actif."""

    def __init__(self, depart):
        self.onglets = [Onglet(depart)]
        self.actif = 0

    # -- ce que la fenêtre demande le plus souvent -------------------------
    @property
    def courant(self):
        return self.onglets[self.actif]

    def combien(self):
        return len(self.onglets)

    def intitules(self):
        return [o.intitule() for o in self.onglets]

    # -- les gestes --------------------------------------------------------
    def ouvrir(self, dossier):
        """Un onglet de plus, juste APRÈS l'actif, et il devient l'actif.

        Après l'actif, et non à la fin : ouvrir un dossier depuis l'onglet 1
        d'une rangée de six doit le poser à côté de celui dont il vient.
        C'est ce que font les navigateurs, et c'est ce qu'on attend.
        """
        self.onglets.insert(self.actif + 1, Onglet(dossier))
        self.actif += 1
        return self.courant

    def fermer(self):
        """Fermer l'actif, sauf s'il est le dernier. Rend True s'il est parti."""
        if len(self.onglets) <= 1:
            return False
        del self.onglets[self.actif]
        if self.actif > 0:
            self.actif -= 1
        return True

    def aller_a(self, rang):
        """Le rang est 0 pour le premier. Hors bornes : on ne bouge pas."""
        if 0 <= rang < len(self.onglets):
            self.actif = rang
            return True
        return False

    def suivant(self):
        """L'onglet d'après, en tournant. Ctrl+Tab ne doit jamais rien bloquer."""
        self.actif = (self.actif + 1) % len(self.onglets)
        return self.courant

    def precedent(self):
        self.actif = (self.actif - 1) % len(self.onglets)
        return self.courant
