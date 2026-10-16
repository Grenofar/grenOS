#!/bin/sh
# Fabrique le paquet grenos-desktop : le bureau de grenOS, celui qui est écrit
# dans ce dépôt — la barre, le bureau, les réglages, GrenPlace, l'accueil, le
# thème des fenêtres et les fonds d'écran.
#
# C'est ce paquet que `grenos-maj` met à jour depuis l'OS : les correctifs de
# sécurité viennent de Debian, notre visage vient de là. Il contient exactement
# les mêmes fichiers que l'image, pris au même endroit — sans quoi une machine
# mise à jour ne ressemblerait plus à une machine fraîchement installée.
#
#   linux/tools/make-deb.sh <version> <dossier de sortie>

set -eu

VERSION="${1:-1.0.0}"
OUT="${2:-dist}"
HERE=$(cd "$(dirname "$0")/.." && pwd)
DEDANS="$HERE/config/includes.chroot"
BUILD=$(mktemp -d)

mkdir -p "$BUILD/DEBIAN" \
         "$BUILD/usr/bin" \
         "$BUILD/usr/lib/grenos" \
         "$BUILD/usr/share/grenos" \
         "$BUILD/usr/share/applications" \
         "$BUILD/usr/share/icons/hicolor/256x256/apps" \
         "$BUILD/usr/share/xsessions" \
         "$BUILD/usr/share/themes/grenOS/openbox-3" \
         "$BUILD/usr/share/calamares/branding/grenos" \
         "$BUILD/usr/share/grenos/modeles" \
         "$BUILD/etc/polkit-1/rules.d" \
         "$BUILD/etc/systemd/system"

# TOUS les programmes du bureau, jamais une liste ecrite a la main.
#
# CETTE BOUCLE NOMMAIT DIX-SEPT PROGRAMMES, ET IL Y EN A VINGT ET UN.
#
# Les quatre absents : `grenos-premier` (ecarte expres), `grenos-fichiers`
# (ecrit le 7 octobre), `grenos-premier-x`, et surtout `grenos-preuve`. Ce
# dernier est un vrai defaut, et il datait : make-deb livre bien
# `grenos-preuve.service`, `appliquer-systeme` l active, et son
# `ExecStart=/usr/bin/grenos-preuve` designait un fichier que le paquet ne
# contenait pas. Sur l ISO cela marche — includes.chroot est copie en entier.
# Sur une machine MISE A JOUR par le depot, le service echoue a chaque
# demarrage, et rien ne le dit.
#
# C est exactement le defaut du 2 octobre sur `composants.py` et `securite.py`,
# a l etage des programmes au lieu de celui des modules. Meme lecon : *une
# liste qu il faut rallonger a chaque fichier ecrit n est pas une liste, c est
# une dette*.
#
# ET L EXCEPTION D HIER ETAIT ELLE-MEME LE BOGUE : ecarter `grenos-premier`
# pendant qu on livre `grenos-premier.service` fabrique precisement l unite
# sans son programme. On prend donc tout. Livrer `grenos-premier` sur une
# machine deja nommee ne coute rien : son service est conditionne a l absence
# de /etc/grenos/identite, qui existe sur une machine installee.
for source in "$DEDANS"/usr/bin/*; do
    install -m 0755 "$source" "$BUILD/usr/bin/"
done

# TOUS les modules, jamais une liste ecrite a la main.
#
# Cette boucle nommait neuf modules un par un. Le 2 octobre, `composants.py` et
# `securite.py` ont ete ecrits et branches dans les Reglages, et personne ne les
# a ajoutes ici : l ISO les aurait eus (includes.chroot est copie en entier) et
# une machine mise a jour par le depot ne les aurait PAS eus. Les deux pages
# neuves auraient plante a l import, chez Grenofar seulement, et aucune etape
# n aurait rougi.
#
# C est mot pour mot la dette que ce depot a deja payee sur la liste
# FACULTATIVES de verif-commandes.py : *une liste qu il faut rallonger a chaque
# fichier ecrit n est pas une liste, c est une dette*. On prend donc tout, et
# on ecarte seulement les gardes de construction, qui n ont rien a faire sur la
# machine de quelqu un.
for source in "$DEDANS"/usr/lib/grenos/*.py; do
    nom=$(basename "$source")
    case "$nom" in
        verif-*) continue ;;
    esac
    install -m 0644 "$source" "$BUILD/usr/lib/grenos/"
done
install -m 0755 "$DEDANS/usr/lib/grenos/grenos-compte" "$BUILD/usr/lib/grenos/"

# ET LA MEME DETTE, DANS LE MEME FICHIER, POUR LES AUXILIAIRES QUI NE SONT PAS
# EN PYTHON. Le commentaire ci-dessus la nomme pour les modules `.py` et la
# ferme par un glob ; les executables, eux, etaient encore installes un par un,
# plus bas, a la main. Dix lignes, et il suffisait d en oublier une.
#
# Mesure, pas supposition : le 15 octobre, `grenos-instantane` a ete ecrit et
# appele par `grenos-maj-demarrage`, et il n aurait PAS voyage dans le paquet.
# L ISO l aurait eu, une machine mise a jour non — et comme l appelant le garde
# derriere un `[ -x ]`, l instantane n aurait simplement jamais eu lieu chez
# elle, sans une ligne d erreur. C est le defaut du 1er octobre (`composants.py`
# et `securite.py` absents de la liste) a l identique.
#
# Le glob est desormais la source : il reprend exactement les onze fichiers que
# la liste a la main portait, plus ceux qu on ecrira. Les `install` nommes plus
# bas deviennent redondants et sont gardes pour leurs raisons — ils ne peuvent
# pas diverger dangereusement, le glob etant un sur-ensemble.
for source in "$DEDANS"/usr/lib/grenos/*; do
    nom=$(basename "$source")
    case "$nom" in
        *.py|verif-*|__pycache__) continue ;;
    esac
    [ -f "$source" ] || continue
    install -m 0755 "$source" "$BUILD/usr/lib/grenos/"
done

install -m 0644 "$DEDANS/usr/share/xsessions/grenos.desktop" "$BUILD/usr/share/xsessions/"

# Le tampon de PipeWire, sans lequel le son hoquette sous charge. Fichier a
# nous dans un dossier de complements : aucun autre paquet ne le possede, donc
# dpkg ne refusera pas notre paquet — c'est la lecon du 24 septembre avec
# lightdm-gtk-greeter.
# UN GLOB, PLUS JAMAIS UNE LISTE ECRITE A LA MAIN.
#
# Les reglages de /etc partaient dans le paquet par SEPT copies nommees une
# par une. Elles etaient toutes justes — verifie avant d'ecrire cette ligne,
# et c'est pour cela qu'elle ne dit pas qu'elles avaient derive. Ce qui est
# sur, en revanche, c'est que le fichier journald ajoute aujourd'hui aurait
# manque a la liste : je l'ai pose dans l'image et je ne l'aurais su qu'en
# voyant, chez lui, un journal qui ne survit pas a l'extinction. C'est
# exactement le defaut du 1er octobre — `composants.py` et `securite.py`
# absents de la liste des modules, l'ISO les avait, une machine mise a jour
# non, et les deux pages neuves plantaient chez lui seul.
#
# *Une liste qu'il faut rallonger a chaque fichier ecrit n'est pas une liste,
# c'est une dette.*
#
# Deux exclusions, et chacune a sa raison :
#   * `etc/systemd/system/` — les unites sont posees plus bas, avec leur
#     fragment et leur activation ; les recopier ici les poserait deux fois ;
#   * `etc/calamares/` — c'est le reglage de l'installateur, qui ne sert que
#     dans l'image vive ; le hook le verifie la, et une machine deja installee
#     n'en a plus l'usage.
( cd "$DEDANS" && find etc -type f -name '*.conf' \
    ! -path 'etc/systemd/system/*' ! -path 'etc/calamares/*' | sort ) \
  | while read -r relatif; do
    nom=$(basename "$relatif")
    case "$nom" in
        *grenos*)
            install -d "$BUILD/$(dirname "$relatif")"
            install -m 0644 "$DEDANS/$relatif" "$BUILD/$relatif"
            echo "  reglage : $relatif"
            ;;
        *)
            # CE FICHIER APPARTIENT A UN AUTRE PAQUET, et le livrer a coute
            # une construction : `find etc -name '*.conf'` a ramasse
            # `etc/lightdm/lightdm-gtk-greeter.conf`, et dpkg a repondu
            # « trying to overwrite […] which is also in package
            # lightdm-gtk-greeter ». C'est la panne du 24 septembre au mot
            # pres, dans le fichier meme qui porte la lecon.
            #
            # Ces fichiers-la partent en MODELES (`usr/share/grenos/modeles`)
            # et `appliquer-systeme` les pose. Le critere est le nom : chaque
            # complement que nous avons invente s'appelle `*grenos*`, et aucun
            # chemin partage ne le fait.
            #
            # ET LE TROISIEME CAS EST REFUSE. Un reglage a nous dont le nom ne
            # nous designe pas n'irait ni dans le paquet ni dans les modeles :
            # l'image l'aurait, une machine mise a jour non, et personne ne le
            # saurait. Plutot que de le laisser tomber en silence, on arrete
            # la construction et on le nomme.
            # ON DEMANDE AU MECANISME, PAS A UN TEXTE. Ma premiere version
            # cherchait le chemin dans ce script-ci : le fichier de lightdm y
            # passait parce qu'un COMMENTAIRE le nommait. Un controle qu'un
            # commentaire satisfait ne controle rien — c'est le garde du
            # 7 octobre qui refusait le service que son propre commentaire
            # documentait, par l'autre bout.
            #
            # La seule chose qui compte est : `appliquer-systeme` POSE-T-IL ce
            # chemin ? La reponse est dans ses lignes `poser`, qui portent la
            # cible reelle. Etre livre comme modele sans etre pose ne servirait
            # a rien, et ce controle-la le verrait aussi.
            poseur="$DEDANS/usr/lib/grenos/appliquer-systeme"
            if [ ! -r "$poseur" ]; then
                # SANS LUI, ON NE SAIT RIEN, ET ON NE FAIT PAS SEMBLANT.
                # L'essai a trouve ce trou : un `grep` sur un fichier absent
                # rendait une chaine vide, `[ "" -eq 0 ]` sortait en erreur,
                # et le fichier partait en « modele » comme si tout allait
                # bien. Le controle disait oui faute de pouvoir dire non.
                echo "  appliquer-systeme est introuvable : impossible de" >&2
                echo "  savoir si $relatif est pose. On s'arrete." >&2
                exit 1
            fi
            pose=$(grep -c "^poser .*[[:space:]]/${relatif}\$" "$poseur" || true)
            if [ "${pose:-0}" -eq 0 ]; then
                echo "  REGLAGE ORPHELIN : $relatif" >&2
                echo "  Il n'est ni a nous (son nom ne porte pas « grenos »)" >&2
                echo "  ni pose par appliquer-systeme. L'image l'aurait, une machine" >&2
                echo "  mise a jour non. Nomme-le *grenos* ou ajoute-le aux" >&2
                echo "  modeles avec son « poser » dans appliquer-systeme." >&2
                exit 1
            fi
            echo "  modele  : $relatif (pose par appliquer-systeme)"
            ;;
    esac
done || exit 1

# Ce que grenOS demande a GameMode pendant qu'un jeu tourne. Le paquet livre
# `renice=0`, donc sans ce fichier GameMode ne change PAS la priorite du jeu —
# tout en accordant le droit de le faire dans limits.d. /etc est le chemin que
# gamemoded(8) designe, et aucun paquet ne le possede.
install -m 0644 "$DEDANS/etc/gamemode.ini" "$BUILD/etc/"

# Les six descriptions de types de fichiers que shared-mime-info ne traduit pas
# en francais — dont celle d un dossier, qui se repete a chaque ligne de
# l explorateur. `appliquer-systeme` rejoue `update-mime-database` ensuite,
# sans quoi le fichier arriverait sans effet : le texte est lu dans un cache
# compile, pas dans le XML.
mkdir -p "$BUILD/usr/share/mime/packages"
install -m 0644 "$DEDANS/usr/share/mime/packages/grenos-francais.xml" \
    "$BUILD/usr/share/mime/packages/"

install -m 0644 "$DEDANS/usr/share/themes/grenOS/openbox-3/themerc" \
    "$BUILD/usr/share/themes/grenOS/openbox-3/"

# Les fonds d'écran, calculés ici comme à la construction de l'image.
python3 "$HERE/tools/wallpaper.py" "$BUILD/usr/share/grenos" 2560 1440

# Le catalogue de secours de GrenPlace : celui du réseau l'emporte toujours.
install -m 0644 "$HERE/data/catalogue.json" "$BUILD/usr/share/grenos/catalogue.json"
# Le son d'essai : c'est lui qui bipe a l'ouverture de session et sur lequel la
# machine d'integration compte les trous. Sans cette ligne il n'arriverait que
# par une ISO.
install -m 0644 "$DEDANS/usr/share/grenos/essai-son.wav" "$BUILD/usr/share/grenos/" 2>/dev/null || true

# Nos icônes, calculées comme les fonds d'écran.
# Nos icones, dans TOUTES les tailles que le theme sait chercher.
#
# Elles n'existaient qu'en 256x256. GTK cherche alors un 24x24, ne trouve que
# le 256, et le rend **tel quel** : dans la barre, le logo grenOS, le
# haut-parleur et l'icone du reseau faisaient quarante-huit pixels au lieu de
# vingt-quatre, et passaient sous le bas de l'ecran. Vu sur la capture de la
# construction en 720p, agrandie quatre fois — la ligne serie ne pouvait pas
# le dire.
#
# On dessine chaque taille plutot que de laisser GTK reduire la grande : nos
# icones sont calculees, pas photographiees, et un trait de deux pixels reste
# net quand il est calcule pour deux pixels.
for taille in 16 22 24 32 48 64 128 256; do
    dossier="$BUILD/usr/share/icons/hicolor/${taille}x${taille}/apps"
    mkdir -p "$dossier"
    python3 "$HERE/tools/icones.py" "$dossier" "$taille" > /dev/null
done

# ---- Ce qui manquait, et qui rend une vraie mise à jour possible -----------
#
# Les services, les entrées de menu, les réglages du système et l'habillage de
# l'installateur étaient écrits au moment de construire l'image, donc jamais
# livrés à une machine déjà installée. Ajouter un service ou une application
# demandait de regraver. Ils font partie du paquet désormais.

# Les services et les minuteries. LA TROISIEME LISTE A LA MAIN DE CE FICHIER,
# et la troisieme dette de la meme famille : elle portait douze noms, l arbre
# en porte treize, et le treizieme est celui qui venait d etre ecrit.
#
# Une unite oubliee ici ne fait AUCUN bruit : l ISO l a, une machine mise a
# jour non, et le service qu elle declenche ne tourne simplement jamais chez
# elle. C est ce qui serait arrive a `grenos-surveiller-demarrage` — donc au
# detecteur d echec de demarrage, c est-a-dire a la piece censee rattraper les
# machines en difficulte, absente precisement des machines en difficulte.
#
# Le glob prend tout ce que l arbre porte, et l arbre ne porte que nos unites.
for service in "$DEDANS"/etc/systemd/system/*.service \
               "$DEDANS"/etc/systemd/system/*.timer; do
    [ -f "$service" ] || continue
    install -m 0644 "$service" "$BUILD/etc/systemd/system/"
done

# Les fragments qui laissent passer un plymouthd deja la. Ils voyagent avec le
# paquet, donc une machine deja posee recoit par mise a jour l arret silencieux
# — c est tout l objet du chantier « plus jamais d ISO ».
for unite in plymouth-poweroff plymouth-reboot plymouth-halt plymouth-kexec; do
    install -d "$BUILD/etc/systemd/system/${unite}.service.d"
    install -m 0644         "$DEDANS/etc/systemd/system/${unite}.service.d/10-grenos-tolerant.conf"         "$BUILD/etc/systemd/system/${unite}.service.d/"
done

# Le silence de systemd et l'absence de lecteur de disquette partaient ici,
# nommes un par un ; ils partent maintenant par le glob de /etc plus haut,
# comme tous les autres reglages. Ce qu'ils corrigent — ce qu'on voit EN
# ETEIGNANT — n'a pas change, ni la raison de les livrer : une machine deja
# posee doit les recevoir par mise a jour.

# Les entrées de menu, et le raccourci d'installation.
install -m 0644 "$DEDANS"/usr/share/applications/*.desktop \
    "$BUILD/usr/share/applications/"

# Les réglages du système, livrés comme **modèles**. Deux paquets ne peuvent
# pas posséder le même fichier, et ceux-ci appartiennent déjà a openbox, a
# l'explorateur, a la bibliotheque de fichiers et a l'ecran de connexion :
# dpkg refuse le notre s'il les reclame. `appliquer-systeme` les pose apres
# l'installation — une mise a jour peut donc toujours les changer, ce qui
# etait tout l'objet.
install -m 0644 "$DEDANS/etc/gtk-3.0/settings.ini" "$BUILD/usr/share/grenos/modeles/gtk-settings.ini"
install -m 0644 "$DEDANS/etc/fonts/local.conf" "$BUILD/usr/share/grenos/modeles/fonts-local.conf"
install -m 0644 "$DEDANS/etc/xdg/pcmanfm/default/pcmanfm.conf" "$BUILD/usr/share/grenos/modeles/pcmanfm.conf"
install -m 0644 "$DEDANS/etc/xdg/libfm/libfm.conf" "$BUILD/usr/share/grenos/modeles/libfm.conf"
install -m 0644 "$DEDANS/etc/lightdm/lightdm-gtk-greeter.conf" "$BUILD/usr/share/grenos/modeles/lightdm-gtk-greeter.conf"
install -m 0644 "$DEDANS/etc/xdg/openbox/rc.xml" "$BUILD/usr/share/grenos/modeles/openbox-rc.xml"

# Celui-ci est bien le notre : personne d'autre ne le reclame.
install -m 0644 "$DEDANS/etc/polkit-1/rules.d/49-grenos-administration.rules" \
    "$BUILD/etc/polkit-1/rules.d/"

# L'habillage de l'installateur, et le partitionnement qu'il propose. Le
# fichier de partitionnement va dans /usr/share : celui de /etc appartient à
# calamares-settings-debian, et deux paquets ne peuvent pas posséder le même.
install -m 0644 "$DEDANS"/usr/share/calamares/branding/grenos/* \
    "$BUILD/usr/share/calamares/branding/grenos/"
install -m 0644 "$DEDANS/usr/share/grenos/partition.conf" "$BUILD/usr/share/grenos/"

# Le script qui branche tout cela dans le système, appelé aussi bien à la
# construction de l'image qu'après chaque mise à jour.
install -m 0755 "$DEDANS/usr/lib/grenos/appliquer-systeme" "$BUILD/usr/lib/grenos/"
install -m 0755 "$DEDANS/usr/lib/grenos/grenos-maj-verif" "$BUILD/usr/lib/grenos/"
# Celui qui installe AVANT l'ouverture du bureau, comme Windows. Sans cette
# ligne il n'arriverait que par une ISO — or tout l'objet de ce chantier est de
# ne plus jamais avoir a en telecharger une.
install -m 0755 "$DEDANS/usr/lib/grenos/grenos-maj-demarrage" "$BUILD/usr/lib/grenos/"
# La sonde qui dit, au moment de l arret, ce que PID 1 croit de son
# affichage. Elle ne corrige rien : elle tranche entre deux causes, et
# une machine deja posee doit pouvoir repondre sans reinstaller.
install -m 0755 "$DEDANS/usr/lib/grenos/grenos-sonde-arret" "$BUILD/usr/lib/grenos/"
# Celui qui reteint l affichage des messages d etat des le debut de l arret :
# systemd le rallume lui-meme, et c est la seule facon de le contredire.
install -m 0755 "$DEDANS/usr/lib/grenos/grenos-taire-larret" "$BUILD/usr/lib/grenos/"
# Le gouverneur du processeur : un noyau neuf repart sur celui par defaut,
# donc ce reglage doit voyager avec le paquet et etre rejoue.
install -m 0755 "$DEDANS/usr/lib/grenos/grenos-performances" "$BUILD/usr/lib/grenos/"
# Ce que la carte graphique sait decoder, dit a Firefox. Il voyage avec le
# paquet parce que c est la seule chose qui puisse reparer une video qui
# saccade sur une machine deja installee — exactement ce que Grenofar
# demande depuis le 7 octobre, et sans retelecharger une image.
install -m 0755 "$DEDANS/usr/lib/grenos/grenos-video" "$BUILD/usr/lib/grenos/"
# Le depot d'un travail a appliquer au prochain demarrage. Il voyage avec le
# paquet, donc une machine deja installee le recoit par mise a jour — ce qui
# est la moindre des choses pour un outil dont tout l'objet est d'appliquer des
# changements sans reinstaller.
install -m 0755 "$DEDANS/usr/lib/grenos/grenos-au-prochain-demarrage" "$BUILD/usr/lib/grenos/"
install -m 0755 "$DEDANS/usr/lib/grenos/grenos-maj-reglage" "$BUILD/usr/lib/grenos/"
# L'habillage de Plymouth. Il doit voyager avec le paquet parce que
# `appliquer-systeme` le rejoue : `spinner.plymouth` appartient a Debian, et
# une remise a jour de `plymouth-themes` remettrait l'anglais sur l'ecran de la
# mise a jour au redemarrage — le seul ecran que Grenofar verra vraiment.
install -m 0755 "$DEDANS/usr/lib/grenos/habiller-plymouth" "$BUILD/usr/lib/grenos/"

# Ce qui s'exécute après l'installation du paquet. Sans lui, un service
# nouvellement livré resterait éteint et une icône nouvelle invisible : le
# fichier serait là, mais rien ne l'aurait branché.
cat > "$BUILD/DEBIAN/postinst" <<'FIN'
#!/bin/sh
set -e
if [ "$1" = configure ] && [ -x /usr/lib/grenos/appliquer-systeme ]; then
    /usr/lib/grenos/appliquer-systeme || true
fi
exit 0
FIN
chmod 0755 "$BUILD/DEBIAN/postinst"

SIZE=$(du -ks "$BUILD" | cut -f1)
cat > "$BUILD/DEBIAN/control" <<EOF
Package: grenos-desktop
Version: $VERSION
Section: x11
Priority: optional
Architecture: all
Maintainer: grenOS <grenos@grenos-dev.vercel.app>
Depends: grenos-systeme (= $VERSION), python3, python3-gi, gir1.2-gtk-3.0, gir1.2-wnck-3.0, openbox, feh, x11-utils, x11-xserver-utils, xdg-utils, xcvt, adwaita-icon-theme-legacy, pulseaudio-utils, pciutils, dbus-user-session, pipewire-audio, wireplumber, xdg-user-dirs
Recommends: flatpak, lightdm, pcmanfm, xfce4-terminal, xfce4-taskmanager, papirus-icon-theme
Installed-Size: $SIZE
Homepage: https://grenos-dev.vercel.app
Description: Le bureau de grenOS
 La barre, le bureau et ses icônes, le menu, les Réglages, GrenPlace, la
 bascule jour et nuit, le thème des fenêtres et les fonds d'écran. Tout ce qui
 se voit dans grenOS est ici ; Debian fournit le noyau, les pilotes et les
 applications. Ce paquet se met à jour depuis le système comme un autre.
EOF

# ---- Le metapaquet : tout ce que l'image contient --------------------------
#
# C'est la piece qui evite de reinstaller le systeme a chaque nouveaute. Quand
# l'image gagne un paquet — un pilote, une bibliotheque de son, un theme
# d'icones —, ce metapaquet le declare, et « Mettre a jour grenOS » l'installe
# sur une machine deja posee. Sans lui, la mise a jour n'apporterait que nos
# propres fichiers, et il faudrait regraver une image pour le reste.
META=$(mktemp -d)
trap 'rm -rf "$BUILD" "$META"' EXIT
mkdir -p "$META/DEBIAN"

DEPENDANCES=$(grep -v '^#' "$HERE/config/package-lists/grenos.list.chroot"     | grep -v '^[[:space:]]*$' | paste -sd, - | sed 's/,/, /g')

cat > "$META/DEBIAN/control" <<EOF
Package: grenos-systeme
Version: $VERSION
Section: metapackages
Priority: optional
Architecture: all
Maintainer: grenOS <grenos@grenos-dev.vercel.app>
Depends: $DEPENDANCES
Installed-Size: 16
Homepage: https://grenos-dev.vercel.app
Description: Tout ce que grenOS installe
 Ce paquet ne contient aucun fichier : il nomme les paquets dont grenOS est
 fait. Sa raison d'etre est la mise a jour — quand l'image gagne un paquet,
 une machine deja installee le recoit en mettant a jour, sans avoir a
 reinstaller le systeme.
EOF

mkdir -p "$OUT"
DEB_META="$OUT/grenos-systeme_${VERSION}_all.deb"
dpkg-deb --build --root-owner-group "$META" "$DEB_META"
echo "$DEB_META"

DEB="$OUT/grenos-desktop_${VERSION}_all.deb"
# TOUT MODULE QU UN PROGRAMME LIVRE IMPORTE DOIT ETRE DANS LE PAQUET.
#
# La boucle ci-dessus prend maintenant tous les .py, donc ce garde ne devrait
# jamais parler. Il est la pour le jour ou quelqu un la restreindra, ou posera
# un module ailleurs que dans /usr/lib/grenos : l oubli ne se verrait alors
# QUE chez quelqu un qui met a jour, jamais dans l image, et jamais en CI.
#
# Il lit les `import X` de chaque programme livre, ecarte ce que Python et le
# systeme fournissent, et exige le fichier dans le paquet.
python3 - "$BUILD" <<'VERIF'
import os
import re
import sys

racine = sys.argv[1]
fournis = set(sys.builtin_module_names) | {
    "os", "sys", "re", "json", "time", "glob", "math", "shutil", "socket",
    "subprocess", "threading", "datetime", "traceback", "urllib", "html",
    "collections", "functools", "itertools", "tempfile", "signal", "errno",
    "random", "hashlib", "textwrap", "unicodedata", "locale", "configparser",
    "gi", "cairo", "numpy",
}

modules = {n[:-3] for n in os.listdir(os.path.join(racine, "usr/lib/grenos"))
           if n.endswith(".py")}

programmes = []
for dossier in ("usr/bin", "usr/lib/grenos"):
    chemin = os.path.join(racine, dossier)
    for nom in sorted(os.listdir(chemin)):
        plein = os.path.join(chemin, nom)
        if os.path.isfile(plein):
            programmes.append(plein)

motif = re.compile(r"^\s*(?:import|from)\s+([a-zA-Z_][a-zA-Z0-9_]*)")
manquants = []
for plein in programmes:
    try:
        with open(plein, encoding="utf-8") as f:
            tete = f.readline()
            if "python" not in tete:
                continue
            f.seek(0)
            lignes = f.read().splitlines()
    except (OSError, UnicodeDecodeError):
        continue
    for ligne in lignes:
        trouve = motif.match(ligne)
        if not trouve:
            continue
        nom = trouve.group(1)
        if nom in fournis or nom in modules:
            continue
        manquants.append((os.path.basename(plein), nom))

if manquants:
    for programme, nom in manquants:
        print("grenos: %s importe %s, qui n est PAS dans le paquet"
              % (programme, nom), file=sys.stderr)
    sys.exit(1)

print("grenos: %d module(s) dans le paquet, et tout ce qu on importe y est"
      % len(modules))

# ---------------------------------------------------------------------------
# CHAQUE SERVICE LIVRE DOIT TROUVER SON PROGRAMME
#
# Le controle du dessus ferme la classe « un programme importe un module
# absent ». Celui-ci ferme sa jumelle, qui a vraiment mordu : « une unite
# systemd est livree et activee, et son ExecStart designe un fichier que le
# paquet ne contient pas ».
#
# C etait le cas de `grenos-preuve.service` jusqu au 7 octobre. Sur l ISO on
# ne voit rien, parce que includes.chroot est copie en entier ; c est seulement
# sur une machine mise a jour par le depot que l unite echoue, a chaque
# demarrage, sans qu une seule etape rougisse. Exactement la forme que ce
# depot attrape depuis des semaines : *une promesse a l ecran sans rien
# derriere*, ici une unite sans son programme.
#
# On ne regarde que nos propres chemins. Un ExecStart vers /bin/sh ou vers un
# binaire de Debian est legitime : ces paquets-la sont des dependances, pas
# notre contenu.
import glob as _glob                                        # noqa: E402

absents = []
for unite in sorted(_glob.glob(os.path.join(racine,
                                            "etc/systemd/system/*.service"))):
    with open(unite, encoding="utf-8", errors="replace") as f:
        for ligne in f:
            nue = ligne.strip()
            if not nue.startswith(("ExecStart=", "ExecStartPre=",
                                   "ExecStartPost=", "ExecStop=")):
                continue
            commande = nue.split("=", 1)[1].strip()
            # systemd accepte les prefixes `-`, `@`, `+`, `!` devant le chemin.
            commande = commande.lstrip("-@+!").strip()
            if not commande:
                continue
            programme = commande.split()[0]
            if not programme.startswith(("/usr/bin/grenos", "/usr/lib/grenos",
                                         "/usr/bin/grenplace")):
                continue
            if not os.path.exists(os.path.join(racine, programme.lstrip("/"))):
                absents.append((os.path.basename(unite), programme))

if absents:
    for unite, programme in absents:
        print("grenos: %s lance %s, qui n est PAS dans le paquet"
              % (unite, programme), file=sys.stderr)
    print("grenos: l ISO marcherait, une machine mise a jour non — "
          "le service echouerait a chaque demarrage", file=sys.stderr)
    sys.exit(1)

print("grenos: chaque service livre trouve son programme dans le paquet")
VERIF

dpkg-deb --build --root-owner-group "$BUILD" "$DEB"
dpkg-deb --info "$DEB" | head -12
dpkg-deb --contents "$DEB" | awk '{print $6}' | sort
echo "$DEB"
