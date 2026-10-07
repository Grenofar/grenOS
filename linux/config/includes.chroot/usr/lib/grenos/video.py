# Ce que la carte graphique sait décoder, et ce qu'on en dit à Firefox.
#
# Grenofar, le 7 octobre : « quand je lance une video youtube et ou short la
# video bug alors que sur l'os je fais un speedtest ookla avec 300mbits entrant
# et sortant ». Il a lui-même écarté le réseau, et il a eu raison de le dire :
# c'est le décodage.
#
# ---------------------------------------------------------------------------
# LA PRÉFÉRENCE QUE J'ALLAIS POSER N'EXISTE PLUS
#
# J'écrivais depuis le 26 septembre que la chaîne vidéo était complète :
# `libavcodec-extra`, sept pilotes VA-API, `vainfo` pour les interroger. Et
# c'est vrai. Ce qui manquait, c'est que **rien ne configurait le navigateur**
# — troisième fois que le même défaut revient, après `va-driver-all` posé sans
# libavcodec et après la règle polkit qui nommait une action inexistante.
#
# J'allais donc poser `media.ffmpeg.vaapi.enabled = true`. Lu dans la table de
# préférences de la version exacte que Debian livre
# (firefox-esr 140.15.0esr-1~deb13u1, `application.ini` donne la révision,
# `modules/libpref/init/StaticPrefList.yaml` au tag FIREFOX_140_15_0esr_RELEASE) :
#
#     media.ffmpeg.vaapi.enabled          -> N'EXISTE PLUS dans ESR 140
#     media.hardware-video-decoding.enabled        value: true   (déjà)
#     media.hardware-video-decoding.force-enabled  value: false
#     media.av1.enabled                            value: true   (déjà)
#
# La préférence que j'allais poser aurait été **morte** : vraie, stable, posée,
# et sans effet — exactement `logo.png`, qui passe son garde et ne bouge pas un
# pixel. Et le décodage matériel que je croyais devoir allumer était déjà
# allumé.
#
# ---------------------------------------------------------------------------
# CE QUE LA MESURE CHANGE, ET POURQUOI ELLE EST ICI
#
# Les deux vraies préférences sont donc ailleurs, et **aucune des deux ne se
# règle sans savoir ce que la machine a sous le capot** :
#
#   * `media.av1.enabled` — YouTube sert de l'AV1 dès que le navigateur
#     l'accepte. L'AV1 est le codec le plus coûteux à décoder au processeur, et
#     très peu de cartes le décodent en matériel (côté Intel, à partir d'Arc ;
#     côté NVIDIA, à partir des RTX 30 ; côté AMD, à partir des RX 6000).
#     Sur tout le reste, accepter l'AV1 c'est demander au processeur de faire
#     le travail d'un circuit dédié — et c'est précisément « la vidéo bug ».
#     Le refuser fait servir du VP9 ou du H.264, que la carte décode.
#
#   * `media.hardware-video-decoding.force-enabled` — Firefox a sa propre
#     liste de blocage, par carte et par version de Mesa. Une carte qui sait
#     décoder peut s'y faire interdire, et rien ne le dit à l'écran.
#
# D'où ce module : on ne devine ni la carte ni ses codecs, **on les demande**.
# Et on ne pose une préférence que quand la mesure l'a justifiée.
#
# La leçon du 25 septembre s'applique mot pour mot : « mesurer une fois, au
# démarrage, et écrire le résultat quelque part que tout le monde lit ». Jamais
# pendant la construction d'une fenêtre — `vulkaninfo` a déjà figé la page Jeux
# pendant vingt-cinq secondes.

import re
import subprocess

# Le dossier que Firefox lit au démarrage, prouvé par le lien symbolique du
# paquet et non par une documentation :
#
#     ./usr/share/firefox-esr/browser/defaults/syspref -> /etc/firefox-esr
#
# `syspref` est le dossier de préférences système de Firefox. Debian l'écrit
# aussi dans le fichier qu'il livre : « You can, with this file and all files
# present in the /etc/firefox-esr directory, override any preference ».
#
# Notre nom commence par `zz-` pour une raison : les fichiers d'un dossier de
# préférences sont lus dans l'ordre alphabétique et le dernier gagne. `zz-`
# passe après le `firefox-esr.js` de Debian.
#
# Et c'est un fichier À NOUS, dans un dossier qui appartient au paquet. dpkg
# accepte qu'un dossier soit partagé, jamais qu'un fichier le soit — la règle
# du 24 septembre, apprise en voyant `dpkg` refuser
# `lightdm-gtk-greeter.conf`. On n'écrit donc pas dans `firefox-esr.js`.
FICHIER_PREFS = "/etc/firefox-esr/zz-grenos-video.js"

# Ce que `vainfo` appelle « décoder » : VLD, pour Variable Length Decoding.
# Une carte peut annoncer un profil en encodage seul (VAEntrypointEncSlice) :
# le compter serait dire qu'elle décode alors qu'elle sait seulement encoder.
ENTREE_DECODAGE = "VAEntrypointVLD"

# Les familles de codecs qui comptent pour une vidéo web, dans l'ordre où
# YouTube les propose.
FAMILLES = (
    ("av1", r"AV1"),
    ("vp9", r"VP9"),
    ("vp8", r"VP8"),
    ("hevc", r"HEVC|H265"),
    ("h264", r"H264"),
)


def profils_materiels(delai=12):
    """Les familles de codecs que cette carte décode VRAIMENT, en matériel.

    Rend (familles, detail). `familles` est un ensemble ; vide veut dire « rien
    en matériel », et `None` veut dire « on n'a pas pu savoir » — les deux ne
    sont pas la même chose et ne mènent pas à la même décision.
    """
    sortie = None
    # `--display drm` d'abord : ce script tourne dans un service, donc sans
    # DISPLAY. Un `vainfo` nu chercherait X et répondrait « can't connect ».
    for commande in (["vainfo", "--display", "drm"], ["vainfo"]):
        try:
            rendu = subprocess.run(commande, capture_output=True, text=True,
                                   timeout=delai)
        except (OSError, subprocess.SubprocessError):
            continue
        texte = (rendu.stdout or "") + (rendu.stderr or "")
        if "VAProfile" in texte:
            sortie = texte
            break
        if sortie is None and texte.strip():
            sortie = texte
    if sortie is None:
        return None, "vainfo introuvable"
    return familles_du_texte(sortie)


def familles_du_texte(sortie):
    """L'analyse, séparée du lancement — pour pouvoir l'exercer.

    C'est ici que vit tout ce qui peut se tromper, et c'est la seule raison
    pour laquelle cette fonction est à part : une analyse qu'on ne peut jouer
    que sur la machine d'intégration est une analyse qu'on ne joue jamais.
    """
    if "VAProfile" not in sortie:
        premiere = ""
        for ligne in sortie.splitlines():
            if ligne.strip():
                premiere = ligne.strip()[:70]
                break
        return set(), premiere or "aucun profil annonce"

    familles = set()
    for ligne in sortie.splitlines():
        # LE PIÈGE DE CETTE LIGNE : une carte annonce des profils en ENCODAGE
        # aussi (`VAEntrypointEncSlice`). Compter ceux-là, c'est conclure
        # qu'elle décode l'AV1 parce qu'elle sait le fabriquer — et laisser
        # YouTube servir de l'AV1 à un processeur qui va s'étouffer dessus.
        if ENTREE_DECODAGE not in ligne:
            continue
        trouve = re.search(r"VAProfile(\S+)", ligne)
        if not trouve:
            continue
        nom = trouve.group(1)
        for famille, motif in FAMILLES:
            if re.search(motif, nom, re.I):
                familles.add(famille)
                break
    pilote = ""
    trouve = re.search(r"Driver version: (.*)", sortie)
    if trouve:
        pilote = trouve.group(1).strip()[:60]
    return familles, pilote or "pilote non nomme"


def decision(familles):
    """Les préférences à poser, et la phrase qui dit pourquoi.

    Trois états, trois décisions. Chacune se déduit de la mesure, et l'état
    « on ne sait pas » ne pose RIEN : poser une préférence sans mesure, c'est
    la cinquième fois de ce fichier qu'on règle à l'aveugle.
    """
    if familles is None:
        return {}, "carte non interrogeable, aucune preference posee"

    if not familles:
        # Aucun décodage matériel du tout. Forcer la liste de blocage ne
        # créerait pas un circuit qui n'existe pas : on ne force rien. Mais le
        # processeur va tout faire, donc autant lui épargner le codec le plus
        # cher — et c'est exactement le cas d'une machine virtuelle.
        return (
            {"media.av1.enabled": False},
            "aucun decodage materiel : AV1 coupe, le processeur fera du VP9",
        )

    prefs = {
        # La carte a prouvé qu'elle décode. La liste de blocage de Mozilla
        # juge sur un modèle et une version de pilote, pas sur une mesure ;
        # celle-ci est faite sur CETTE machine.
        "media.hardware-video-decoding.force-enabled": True,
    }
    if "av1" in familles:
        return prefs, "AV1 decode en materiel : rien a couper"
    prefs["media.av1.enabled"] = False
    return (
        prefs,
        "AV1 non decode par cette carte : coupe, YouTube servira du "
        + ("VP9" if "vp9" in familles else "H.264"),
    )


def texte_du_fichier(familles, detail, prefs, verdict):
    """Le contenu du fichier de préférences, avec de quoi le relire plus tard.

    Un fichier généré qui ne dit pas ce qui l'a généré est un fichier que
    personne n'ose toucher. Celui-ci porte la mesure qui l'a décidé.
    """
    lignes = [
        "// Écrit par grenOS à chaque démarrage. Ne pas modifier à la main :",
        "// `grenos-video` le réécrit au démarrage suivant.",
        "//",
        "// Mesure : " + ("vainfo n'a pas répondu" if familles is None else (
            "décodage matériel " + (", ".join(sorted(familles)) if familles
                                    else "aucun"))),
        # « Ce que vainfo a dit » et non « Pilote » : quand l'interrogation
        # échoue, ce champ porte le message d'erreur, et l'étiqueter « Pilote »
        # ferait passer une panne pour un nom de pilote.
        "// Vu de vainfo : " + detail,
        "// Décision : " + verdict,
        "",
    ]
    for nom in sorted(prefs):
        valeur = "true" if prefs[nom] else "false"
        lignes.append('pref("%s", %s);' % (nom, valeur))
    if not prefs:
        lignes.append("// Aucune préférence : la mesure ne justifiait rien.")
    return "\n".join(lignes) + "\n"
