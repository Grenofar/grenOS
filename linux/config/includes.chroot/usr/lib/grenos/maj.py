"""Mettre à jour la machine, sans terminal et sans bloquer la fenêtre.

Le même travail sert à deux endroits — la fenêtre « Mettre à jour grenOS » et
la page Mises à jour des Réglages —, et il n'a aucune raison d'être écrit deux
fois. Ce module ne dessine rien : il fait le travail et rend compte, ligne par
ligne, à qui l'appelle.

Tout tourne dans un fil séparé. Une fenêtre figée pendant les minutes d'un
téléchargement passe pour un plantage, et c'est ce que les gens font alors :
ils la ferment au milieu d'une installation.
"""
import os
import re
import subprocess
import threading


def lanceur():
    """sudo quand il ne demande rien, pkexec sinon."""
    if subprocess.run(["sudo", "-n", "true"], capture_output=True).returncode == 0:
        return ["sudo", "-n"]
    return ["pkexec"]


# La commande exacte que lance « Mettre à jour grenOS ».
#
# Elle est ici, au niveau du module, pour une raison précise : la machine
# d'intégration l'exécute telle quelle. Tant qu'elle était écrite à l'intérieur
# de la fenêtre, l'essai de la CI lançait un `apt-get install` de son cru — et
# une option refusée par apt (`--with-new-pkgs`) est passée jusque chez
# Grenofar sans qu'une seule étape ne rougisse.
COMMANDE_MAJ = ["apt-get", "-y",
                "-o", "Dpkg::Options::=--force-confdef",
                "-o", "Dpkg::Options::=--force-confold",
                "full-upgrade"]


class Travail:
    """Une mise à jour en cours, et ce qu'elle raconte.

    `sur_etat(texte)`, `sur_avance(part)` et `sur_ligne(texte)` sont appelés
    depuis le fil de travail : à l'appelant de les renvoyer vers son interface
    par le bon chemin (GLib.idle_add pour GTK).
    """

    def __init__(self, sur_etat, sur_avance, sur_ligne, sur_fin):
        self.sur_etat = sur_etat
        self.sur_avance = sur_avance
        self.sur_ligne = sur_ligne
        self.sur_fin = sur_fin
        self.fil = None

    def en_cours(self):
        return self.fil is not None and self.fil.is_alive()

    def demarrer(self):
        if self.en_cours():
            return False
        self.fil = threading.Thread(target=self._tout_faire, daemon=True)
        self.fil.start()
        return True

    # dpkg demande quoi faire quand un fichier de configuration a ete modifie
    # a la main. Sans personne devant l'ecran, cette question bloque
    # l'installation indefiniment. « confdef » puis « confold » repondent pour
    # nous : le fichier qu'on a modifie soi-meme est garde, les autres sont
    # remplaces par la nouvelle version.
    SANS_QUESTION = ["-o", "Dpkg::Options::=--force-confdef",
                     "-o", "Dpkg::Options::=--force-confold"]

    def _courir(self, commande, sur_sortie=None):
        processus = subprocess.Popen(
            lanceur() + commande, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            text=True, bufsize=1,
            env={**os.environ, "DEBIAN_FRONTEND": "noninteractive"})
        for ligne in processus.stdout:
            ligne = ligne.rstrip()
            if ligne:
                self.sur_ligne(ligne)
                if sur_sortie is not None:
                    sur_sortie(ligne)
        processus.wait()
        return processus.returncode

    def _tout_faire(self):
        self.sur_etat("Lecture des dépôts…")
        self.sur_avance(0.05)
        if self._courir(["apt-get", "update"]) != 0:
            return self.sur_fin("Les dépôts n'ont pas pu être lus. Vérifie le réseau.", False)

        self.sur_etat("Ce qui va changer…")
        self.sur_avance(0.2)
        simulation = subprocess.run(
            lanceur() + ["apt-get", "-s", "full-upgrade"],
            capture_output=True, text=True).stdout
        a_faire = [l for l in simulation.splitlines() if l.startswith("Inst ")]
        total = len(a_faire)
        for ligne in a_faire:
            self.sur_ligne(ligne)

        if total == 0:
            self.sur_avance(1.0)
            return self.sur_fin("Tout est déjà à jour.", True)

        self.sur_etat(f"{total} paquet{'s' if total > 1 else ''} à installer…")
        faits = [0]

        def suivre(ligne):
            # apt annonce chaque paquet qu'il déballe et qu'il installe : c'est
            # la seule mesure honnête de l'avancement.
            if re.match(r"^(Unpacking|Dépaquetage|Setting up|Paramétrage)", ligne):
                faits[0] += 1
                self.sur_avance(0.25 + 0.7 * min(faits[0] / (total * 2), 1.0))

        # `full-upgrade` et non `upgrade` : c'est la seule forme qui accepte
        # d'installer un paquet nouveau. Sans elle, une nouvelle dépendance de
        # grenos-desktop — un pilote, une bibliothèque de son — est annoncée
        # puis jamais posée, et la mise à jour ne change rien.
        #
        # Et surtout **sans `--with-new-pkgs`**, qui faisait tout échouer.
        #
        # Grenofar : « quand j'essaie de mettre à jour il dit error command
        # line --with-new-pkgs is not understood ». apt n'accepte cette option
        # que pour `upgrade` ; avec `full-upgrade` il refuse la ligne ENTIÈRE,
        # donc la mise à jour ne démarrait même pas. Elle était de toute façon
        # inutile ici : `full-upgrade` installe les paquets nouveaux par
        # définition — c'est exactement ce que dit le commentaire ci-dessus, et
        # je l'avais quand même ajoutée par précaution.
        #
        # Personne ne l'avait vu parce que l'essai de la CI appelle
        # `apt-get install`, jamais la commande que cette fenêtre lance
        # vraiment. Une CI verte prouvait que le dépôt marche, pas que le
        # bouton marche.
        code = self._courir(COMMANDE_MAJ, suivre)

        # Le repli : si notre commande échoue, on réessaie la plus nue.
        #
        # C'est la réponse au défaut de conception que le 26 septembre a mis à
        # nu : **un défaut dans la mise à jour ne peut être réparé que par une
        # mise à jour**. `--with-new-pkgs` a bloqué le bouton, et le correctif
        # voyageait dans le paquet que seul ce bouton pouvait installer. Il a
        # fallu une commande au terminal — exactement ce que Grenofar ne veut
        # plus jamais avoir à faire.
        #
        # `apt-get -y full-upgrade` sans une seule option est la forme la plus
        # pauvre qui fasse le travail. Si un jour une option que nous ajoutons
        # est refusée, mal orthographiée ou retirée d'apt, la machine se met à
        # jour quand même et le dit. Une erreur de notre part doit DÉGRADER, pas
        # BLOQUER.
        if code != 0:
            self.sur_ligne("--- la commande habituelle a échoué, essai de la forme simple ---")
            code = self._courir(["apt-get", "-y", "full-upgrade"], suivre)

        self.sur_avance(1.0)
        if code != 0:
            return self.sur_fin("L'installation s'est arrêtée. Le détail est ci-dessous.", False)
        # Une nouvelle barre ou un nouveau bureau viennent d'arriver sur le
        # disque, mais ceux qui tournent sont les anciens. On les arrête : le
        # veilleur les relance aussitôt, dans leur nouvelle version. Sans cela,
        # il faudrait fermer la session pour voir le changement — et personne
        # ne le ferait, donc personne ne verrait la mise à jour.
        self._rafraichir_le_bureau()

        return self.sur_fin(
            f"{total} paquet{'s' if total > 1 else ''} installé"
            f"{'s' if total > 1 else ''}. C'est à jour.", True)

    def _rafraichir_le_bureau(self):
        """Relance la barre et le bureau si leur programme a change."""
        for programme in ("grenos-shell", "grenos-bureau"):
            try:
                subprocess.run(["pkill", "-f", programme], capture_output=True,
                               timeout=20)
            except (OSError, subprocess.SubprocessError):
                pass


# ---- Ce que la vérification quotidienne a trouvé ----------------------------
COMPTE = "/run/grenos/maj"
DATE = "/run/grenos/maj-date"
LISTE = "/run/grenos/maj-liste"
REGLAGE = "/usr/lib/grenos/grenos-maj-reglage"


def disponibles():
    """Combien de paquets attendent, d'après la dernière vérification.

    Rend un entier, ou None quand on ne sait pas — sans réseau, « rien » et
    « zéro » ne sont pas la même chose, et les confondre ferait dire au bureau
    que tout est à jour alors qu'il n'en sait rien.
    """
    try:
        with open(COMPTE, encoding="utf-8") as fichier:
            texte = fichier.read().strip()
        return int(texte)
    except (OSError, ValueError):
        return None


def derniere_verification():
    try:
        with open(DATE, encoding="utf-8") as fichier:
            return fichier.read().strip()
    except OSError:
        return ""


def paquets_en_attente():
    try:
        with open(LISTE, encoding="utf-8") as fichier:
            return [l.strip() for l in fichier if l.strip()]
    except OSError:
        return []


def _reglage(action):
    sans_mot_de_passe = subprocess.run(["sudo", "-n", "true"],
                                       capture_output=True).returncode == 0
    lanceur_ = ["sudo", "-n"] if sans_mot_de_passe else ["pkexec"]
    try:
        return subprocess.run(lanceur_ + [REGLAGE, action],
                              capture_output=True, text=True, timeout=300)
    except (OSError, subprocess.SubprocessError):
        return None


def automatique():
    """Les mises à jour s'installent-elles toutes seules ?"""
    return os.path.exists("/etc/grenos/maj-automatique")


def regler_automatique(actif):
    """Allume ou coupe l'installation automatique. Rend l'erreur, ou rien."""
    resultat = _reglage("activer" if actif else "couper")
    if resultat is None or resultat.returncode != 0:
        return "Le système a refusé ce réglage."
    return ""


def verifier_maintenant():
    """Relance la vérification sans attendre le minuteur."""
    resultat = _reglage("maintenant")
    if resultat is None or resultat.returncode != 0:
        return "La vérification n'a pas pu être lancée."
    return ""
