# -*- coding: utf-8 -*-
"""Exercer `au_clavier` DE grenos-fichiers, la vraie fonction, pas une copie.

La fonction est EXTRAITE DU FICHIER LIVRE par l arbre syntaxique, puis
compilee telle quelle. Recopier son corps ici ferait deux claviers d une meme
fenetre, et c est alors la copie qu on croirait — le piege des deux mesures,
paye quatre fois dans ce depot.

CE QUE CET ESSAI ATTRAPE : apres une recherche, Echap effacait le texte et
LAISSAIT le clavier dans le champ. La barriere « on est en train d ecrire »
avalait ensuite F2, Suppr, Retour arriere et Ctrl+H, pour toujours, sans qu un
seul pixel ne le dise. La machine d integration l a prouve par l autre bout :
les quatre lettres du renommage sont arrivees dans le champ de recherche.
"""
import ast
import io
import os
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__))))
DEFAUT = os.path.join(RACINE, "linux", "config", "includes.chroot",
                      "usr", "bin", "grenos-fichiers")
FICHIER = sys.argv[1] if len(sys.argv) > 1 else DEFAUT

source = io.open(FICHIER, encoding="utf-8").read()
arbre = ast.parse(source)
trouvee = None
for noeud in ast.walk(arbre):
    if isinstance(noeud, ast.FunctionDef) and noeud.name == "au_clavier":
        trouvee = noeud
if trouvee is None:
    print("au_clavier n est pas dans %s — l essai ne juge rien" % FICHIER)
    sys.exit(1)
print("  au_clavier extraite du fichier livre : lignes %d a %d"
      % (trouvee.lineno, trouvee.end_lineno))


# --- de quoi la faire tourner, et rien de plus ---------------------------
class Masque(object):
    CONTROL_MASK = 1
    MOD1_MASK = 2
    SHIFT_MASK = 4


class FauxGdk(object):
    ModifierType = Masque

    @staticmethod
    def keyval_name(valeur):
        return valeur


class Champ(object):
    def __init__(self, focus=False, texte=""):
        self._focus = focus
        self._texte = texte

    def has_focus(self):
        return self._focus

    def get_text(self):
        return self._texte

    def set_text(self, texte):
        self._texte = texte

    def grab_focus(self):
        self._focus = True


class Vue(object):
    def __init__(self, journal):
        self.focus = False
        self._journal = journal

    def grab_focus(self):
        self.focus = True

    def get_selection(self):
        return self

    def select_all(self):
        self._journal.append("tout-selectionner")


class FausseRangee(object):
    """La VRAIE mécanique des onglets serait un module de plus à charger ;
    ici on ne juge que ce que le clavier lui demande, et dans quel ordre.
    `onglets.py` est exercé a part, en 30 cas."""

    def __init__(self, journal, combien=3, actif=0):
        self._journal = journal
        self._combien = combien
        self.actif = actif

    def combien(self):
        return self._combien

    def suivant(self):
        self._journal.append("onglet-suivant")

    def precedent(self):
        self._journal.append("onglet-precedent")

    def aller_a(self, rang):
        self._journal.append("onglet-%d" % rang)
        return 0 <= rang < self._combien


class Fenetre(object):
    """Un `self` qui note ce qu on lui demande, sans rien faire."""

    def __init__(self, cherche_focus=False, cherche_texte="",
                 chemin_focus=False, adresse=False):
        self.cherche = Champ(cherche_focus, cherche_texte)
        self.champ_chemin = Champ(chemin_focus)
        self._adresse = adresse
        self.faits = []
        self.vue = Vue(self.faits)
        self.rangee = FausseRangee(self.faits)

    def _note(self, quoi):
        self.faits.append(quoi)

    def barre_d_adresse_ouverte(self):
        return self._adresse

    def fermer_la_barre_d_adresse(self):
        self._adresse = False
        self._note("fermer-adresse")

    def ouvrir_la_barre_d_adresse(self):
        self._adresse = True
        self._note("ouvrir-adresse")

    def au_precedent(self):
        self._note("precedent")

    def au_suivant(self):
        self._note("suivant")

    def au_parent(self):
        self._note("parent")

    def peupler(self):
        self._note("recharger")

    def action_nouveau_dossier(self):
        self._note("nouveau-dossier")

    def action_nouvelle_fenetre(self):
        self._note("nouvelle-fenetre")

    def action_basculer_caches(self):
        self._note("caches")

    def action_renommer(self):
        self._note("renommer")

    def action_corbeille(self):
        self._note("corbeille")

    def action_copier(self):
        self._note("copier")

    def action_couper(self):
        self._note("couper")

    def action_coller(self):
        self._note("coller")

    def action_nouvel_onglet(self):
        self._note("nouvel-onglet")

    def action_fermer_onglet(self):
        self._note("fermer-onglet")

    def changer_d_onglet(self):
        self._note("changer-onglet")


class Evenement(object):
    def __init__(self, touche, etat=0):
        self.keyval = touche
        self.state = etat


espace = {"Gdk": FauxGdk}
exec(compile(ast.Module(body=[trouvee], type_ignores=[]), FICHIER, "exec"),
     espace)
au_clavier = espace["au_clavier"]

CTRL = Masque.CONTROL_MASK
ALT = Masque.MOD1_MASK
MAJ = Masque.SHIFT_MASK
echecs = []
total = 0


def cas(nom, fenetre, touche, etat, attendus, pris=None, focus_liste=None):
    global total
    total += 1
    reponse = au_clavier(fenetre, None, Evenement(touche, etat))
    souci = []
    if fenetre.faits != attendus:
        souci.append("faits %s au lieu de %s" % (fenetre.faits, attendus))
    if pris is not None and bool(reponse) != pris:
        souci.append("la touche est %s" % ("prise" if reponse else "rendue"))
    if focus_liste is not None and fenetre.vue.focus != focus_liste:
        souci.append("clavier a la liste = %s" % fenetre.vue.focus)
    if souci:
        echecs.append(nom)
        print("  [KO] %-58s %s" % (nom, " ; ".join(souci)))
    else:
        print("  [ok] %-58s %s" % (nom, attendus or "rien"))


# ---- LE CAS QUI COMPTE, et ceux qui l entourent -------------------------
cas("Echap apres une recherche rend le clavier a la liste",
    Fenetre(cherche_focus=True, cherche_texte="doc"), "Escape", 0,
    [], pris=True, focus_liste=True)
cas("Echap dans un champ VIDE rend quand meme le clavier",
    Fenetre(cherche_focus=True, cherche_texte=""), "Escape", 0,
    [], pris=True, focus_liste=True)
cas("F2 juste apres cet Echap renomme",
    Fenetre(), "F2", 0, ["renommer"], pris=True)
cas("F2 pendant qu on ecrit dans la recherche ne renomme PAS",
    Fenetre(cherche_focus=True, cherche_texte="doc"), "F2", 0, [], pris=False)
cas("F2 pendant qu on ecrit un chemin ne renomme PAS",
    Fenetre(chemin_focus=True), "F2", 0, [], pris=False)
cas("Echap ferme d abord la barre d adresse",
    Fenetre(cherche_texte="doc", adresse=True), "Escape", 0,
    ["fermer-adresse"], pris=True, focus_liste=False)
cas("Echap sans rien d ouvert ni de tape ne prend pas la touche",
    Fenetre(), "Escape", 0, [], pris=False, focus_liste=False)

# ---- et rien de ce qui marchait ne doit avoir bouge --------------------
cas("Alt+Gauche : precedent", Fenetre(), "Left", ALT, ["precedent"], pris=True)
cas("Alt+Droite : suivant", Fenetre(), "Right", ALT, ["suivant"], pris=True)
cas("Alt+Haut : parent", Fenetre(), "Up", ALT, ["parent"], pris=True)
cas("Retour arriere : precedent, PAS parent",
    Fenetre(), "BackSpace", 0, ["precedent"], pris=True)
cas("Retour arriere dans la recherche efface une lettre",
    Fenetre(cherche_focus=True, cherche_texte="doc"), "BackSpace", 0,
    [], pris=False)
cas("Suppr au milieu d un chemin n envoie RIEN a la corbeille",
    Fenetre(chemin_focus=True), "Delete", 0, [], pris=False)
cas("Suppr sur la liste : corbeille",
    Fenetre(), "Delete", 0, ["corbeille"], pris=True)
cas("Ctrl+Maj+N : nouveau dossier",
    Fenetre(), "n", CTRL | MAJ, ["nouveau-dossier"], pris=True)
cas("Ctrl+Maj+N marche MEME en ecrivant",
    Fenetre(cherche_focus=True, cherche_texte="doc"), "n", CTRL | MAJ,
    ["nouveau-dossier"], pris=True)
cas("Ctrl+N : nouvelle fenetre",
    Fenetre(), "n", CTRL, ["nouvelle-fenetre"], pris=True)
cas("Ctrl+F donne le clavier au champ",
    Fenetre(), "f", CTRL, [], pris=True)
cas("Ctrl+L ouvre la barre d adresse",
    Fenetre(), "l", CTRL, ["ouvrir-adresse"], pris=True)
cas("F4 ouvre la barre d adresse",
    Fenetre(), "F4", 0, ["ouvrir-adresse"], pris=True)
cas("F5 recharge", Fenetre(), "F5", 0, ["recharger"], pris=True)
cas("Ctrl+H bascule les caches", Fenetre(), "h", CTRL, ["caches"], pris=True)
cas("Ctrl+H en ecrivant ne bascule rien",
    Fenetre(cherche_focus=True, cherche_texte="d"), "h", CTRL, [], pris=False)
cas("Ctrl+C copie", Fenetre(), "c", CTRL, ["copier"], pris=True)
cas("Ctrl+X coupe", Fenetre(), "x", CTRL, ["couper"], pris=True)
cas("Ctrl+V colle", Fenetre(), "v", CTRL, ["coller"], pris=True)
cas("Ctrl+A selectionne tout",
    Fenetre(), "a", CTRL, ["tout-selectionner"], pris=True)
cas("une lettre seule ne declenche rien", Fenetre(), "z", 0, [], pris=False)

# ---- LES ONGLETS, ajoutes le 16 octobre --------------------------------
cas("Ctrl+T ouvre un onglet",
    Fenetre(), "t", CTRL, ["nouvel-onglet"], pris=True)
cas("Ctrl+T marche MEME en ecrivant dans la recherche",
    Fenetre(cherche_focus=True, cherche_texte="doc"), "t", CTRL,
    ["nouvel-onglet"], pris=True)
cas("Ctrl+W ferme l onglet",
    Fenetre(), "w", CTRL, ["fermer-onglet"], pris=True)
cas("Ctrl+Tab passe au suivant",
    Fenetre(), "Tab", CTRL, ["onglet-suivant", "changer-onglet"], pris=True)
cas("Ctrl+Maj+Tab passe au precedent",
    Fenetre(), "Tab", CTRL | MAJ, ["onglet-precedent", "changer-onglet"],
    pris=True)
cas("Ctrl+Maj+Tab sous sa forme X11 (ISO_Left_Tab) aussi",
    Fenetre(), "ISO_Left_Tab", CTRL, ["onglet-precedent", "changer-onglet"],
    pris=True)
cas("Ctrl+1 va au premier",
    Fenetre(), "1", CTRL, ["onglet-0", "changer-onglet"], pris=True)
cas("Ctrl+9 va au DERNIER, pas au neuvieme",
    Fenetre(), "9", CTRL, ["onglet-2", "changer-onglet"], pris=True)
cas("Ctrl+5 dans une rangee de trois ne change pas d onglet",
    Fenetre(), "5", CTRL, ["onglet-4"], pris=True)
cas("Tab SANS Ctrl ne touche pas aux onglets",
    Fenetre(), "Tab", 0, [], pris=False)

print("")
if echecs:
    print("%d cas sur %d ont echoue." % (len(echecs), total))
    sys.exit(1)
print("les %d cas passent." % total)
