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

# Ce que lance le BOUTON, qui ne pose plus rien lui-meme.
#
# `-d` : telecharger sans installer. Le bouton remplit le cache apt et arme le
# service d'avant-bureau ; c'est lui, et lui seul, qui pose les paquets —
# pendant que rien de graphique ne tourne.
COMMANDE_TELECHARGER = ["apt-get", "-d", "-y", "full-upgrade"]


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

        # ---- On TELECHARGE, on n'installe pas ------------------------------
        #
        # POURQUOI CE N'EST PLUS LA MEME CHOSE
        #
        # Grenofar, le 1er octobre : « quand je mets a jour l'ecran devient
        # noir, donc on applique la maj apres un redemarrage, pendant le boot ».
        #
        # Il a raison, et la cause etait juste en dessous de ces lignes :
        #
        #     def _rafraichir_le_bureau(self):
        #         for programme in ("grenos-shell", "grenos-bureau"):
        #             subprocess.run(["pkill", "-f", programme], ...)
        #
        # On remplacait la barre et le bureau PENDANT qu'ils tournaient, puis on
        # les tuait pour que le veilleur les relance en version neuve. Entre les
        # deux, l'ecran est noir. C'etait ecrit comme un service rendu — « sans
        # cela personne ne verrait la mise a jour » — et c'etait un ecran noir
        # au milieu d'une session.
        #
        # Pire : une session interrompue en plein `dpkg` est la facon classique
        # de casser une machine. Windows applique hors session pour cette raison
        # exacte, et c'est ce que Grenofar demandait deja le 26 septembre.
        #
        # Donc : on telecharge ici, et c'est `grenos-maj-demarrage` qui pose,
        # avant le bureau, sur un ecran a nous. `-d` ne touche a rien ; le pire
        # qui puisse arriver est un cache apt rempli pour rien.
        self.sur_etat(f"{total} paquet{'s' if total > 1 else ''} a telecharger...")
        faits = [0]

        def suivre(ligne):
            # apt annonce chaque fichier qu'il recupere : c'est la seule mesure
            # honnete de l'avancement d'un telechargement.
            if re.match(r"^(Get:|Réception de|Téléchargement)", ligne):
                faits[0] += 1
                self.sur_avance(0.25 + 0.6 * min(faits[0] / total, 1.0))

        if self._courir(COMMANDE_TELECHARGER, suivre) != 0:
            return self.sur_fin(
                "Le téléchargement s'est arrêté. Le détail est ci-dessous.", False)

        # ---- Et on arme le prochain demarrage -------------------------------
        #
        # Exactement le geste du menu d'arret (`grenos-arret`), au mot pres :
        # `systemctl enable grenos-maj-demarrage.service`. Deux chemins vers le
        # meme service, jamais deux mecanismes — deux copies d'une meme chose
        # finissent par diverger.
        self.sur_etat("Préparation du redémarrage…")
        self.sur_avance(0.9)
        if self._courir(["systemctl", "enable", "grenos-maj-demarrage.service"]) != 0:
            return self.sur_fin(
                "Les paquets sont là, mais le redémarrage n'a pas pu être armé.",
                False)

        self.sur_avance(1.0)
        return self.sur_fin(
            f"{total} paquet{'s' if total > 1 else ''} prêt"
            f"{'s' if total > 1 else ''}. Redémarre pour les installer.", True)


def redemarrage_arme():
    """La mise à jour attend-elle le prochain démarrage ?

    On interroge systemd plutôt que de garder un drapeau à nous : un drapeau et
    l'état réel finissent par diverger, et c'est alors le drapeau qu'on croit.
    `is-enabled` ne demande aucun droit.
    """
    try:
        sortie = subprocess.run(
            ["systemctl", "is-enabled", "grenos-maj-demarrage.service"],
            capture_output=True, text=True, timeout=8).stdout
    except (OSError, subprocess.SubprocessError):
        return False
    return sortie.strip() == "enabled"


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
