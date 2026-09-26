#!/bin/sh
# La mise à jour depuis grenOS fonctionne-t-elle vraiment ?
#
# On part d'une Debian nue, on ajoute notre dépôt exactement comme l'image le
# fait, et on installe `grenos-desktop`. Si cela échoue ici, aucune machine ne
# pourra se mettre à jour — et il vaut mieux le savoir en deux minutes que de
# le découvrir sur la machine de quelqu'un.
#
# Ce que cet essai prouve, et c'est précisément ce qui manquait :
#   - le dépôt est joignable, et sa signature est acceptée ;
#   - le paquet s'installe avec toutes ses dépendances, y compris les nouvelles ;
#   - les fichiers du bureau arrivent bien à leur place.
#
# Lancé dans un conteneur par la CI, avec DEPOT et le dépôt local monté.

set -eu

echo "--- une Debian nue, avec les memes sections que grenOS ---"
# Le conteneur n'a que « main ». Une machine grenOS a aussi contrib, non-free
# et les micrologiciels — c'est la que vivent le microcode du processeur et les
# pilotes des cartes Wi-Fi. Sans ces sections, l'essai echouerait sur des
# paquets qui, eux, s'installent tres bien sur une vraie machine : il serait
# alors moins fidele que ce qu'il pretend verifier.
cat > /etc/apt/sources.list <<'FIN'
deb http://deb.debian.org/debian trixie main contrib non-free non-free-firmware
deb http://deb.debian.org/debian trixie-updates main contrib non-free non-free-firmware
deb http://security.debian.org/debian-security trixie-security main contrib non-free non-free-firmware
FIN
rm -f /etc/apt/sources.list.d/debian.sources
apt-get update -qq
apt-get install -y -qq ca-certificates curl gnupg >/dev/null

echo "--- notre depot, comme dans l'image ---"
install -D -m 0644 linux/grenos-apt.gpg /usr/share/keyrings/grenos-apt.gpg
printf 'deb [signed-by=/usr/share/keyrings/grenos-apt.gpg] %s stable main\n' "$DEPOT" \
    > /etc/apt/sources.list.d/grenos.list
cat /etc/apt/sources.list.d/grenos.list

echo "--- lecture du depot ---"
# Sans `-o Acquire::Retries`, une propagation de cache un peu lente ferait
# echouer l'essai pour une mauvaise raison.
apt-get -o Acquire::Retries=5 update

echo "--- ce que le depot propose ---"
apt-cache policy grenos-desktop grenos-systeme

echo "--- installation ---"
# Les memes options que la mise a jour automatique : une question posee par
# dpkg bloquerait une installation sans personne devant l'ecran.
apt-get install -y --no-install-recommends \
    -o Dpkg::Options::=--force-confdef \
    -o Dpkg::Options::=--force-confold \
    grenos-desktop

echo "--- ce qui est arrive ---"
# Ce que l'on vérifie n'est pas seulement « des fichiers sont arrivés » : c'est
# qu'une mise à jour peut apporter une VRAIE nouveauté — un service qui
# n'existait pas, une entrée de menu, une icône. Sans eux, il faudrait regraver
# une image pour changer autre chose qu'un programme.
for fichier in /usr/bin/grenos-shell /usr/bin/grenos-maj /usr/bin/grenos-jeux \
               /usr/lib/grenos/grenosui.py /usr/share/grenos/catalogue.json \
               /usr/lib/grenos/appliquer-systeme \
               /etc/systemd/system/grenos-maj-verif.timer \
               /etc/systemd/system/grenos-maj-auto.service \
               /usr/share/applications/grenos-parametres.desktop \
               /usr/share/applications/grenos-jeux.desktop \
               /usr/share/icons/hicolor/256x256/apps/grenplace.png \
               /usr/share/icons/hicolor/24x24/apps/grenplace.png \
               /usr/share/icons/hicolor/24x24/apps/grenos.png \
               /etc/xdg/openbox/rc.xml \
               /etc/polkit-1/rules.d/49-grenos-administration.rules; do
    if [ ! -e "$fichier" ]; then
        echo "manquant : $fichier" >&2
        exit 1
    fi
done
dpkg-query -W -f='${Package} ${Version} ${Status}\n' grenos-desktop grenos-systeme

# ---- Et maintenant, une vraie mise a jour -----------------------------------
#
# Ce qui precede prouve une installation neuve. Ce qu'on nous demande est
# autre chose : qu'une version en remplace une autre, en apportant une
# nouveaute — « un truc sur l'interface, les processus, etc. » — sans regraver
# l'image et sans rien perdre. Alors on le fait pour de vrai.
if ls dist/grenos-desktop_*.deb >/dev/null 2>&1; then
    echo "--- une version plus recente arrive, comme sur une vraie machine ---"
    ANCIENNE=$(dpkg-query -W -f='${Version}' grenos-desktop)

    rm -rf /tmp/suite
    dpkg-deb -R "$(ls dist/grenos-desktop_*.deb | head -1)" /tmp/suite
    sed -i "s/^Version: .*/Version: ${ANCIENNE}+suite/" /tmp/suite/DEBIAN/control
    # Les sommes de controle d'origine ne couvriraient pas les fichiers qu'on
    # ajoute : mieux vaut aucune somme qu'une somme fausse.
    rm -f /tmp/suite/DEBIAN/md5sums

    # Deux nouveautes que la version installee n'a pas : un service et une
    # entree de menu. C'est exactement ce qu'une mise a jour « majeure » doit
    # pouvoir apporter, et ce qui etait impossible tant que le paquet ne
    # portait que des programmes.
    mkdir -p /tmp/suite/etc/systemd/system /tmp/suite/usr/share/applications
    cat > /tmp/suite/etc/systemd/system/grenos-nouveaute.service <<'FIN'
[Unit]
Description=Preuve qu une mise a jour peut apporter un service neuf
[Service]
Type=oneshot
ExecStart=/bin/true
[Install]
WantedBy=multi-user.target
FIN
    cat > /tmp/suite/usr/share/applications/grenos-nouveaute.desktop <<'FIN'
[Desktop Entry]
Type=Application
Name=Nouveaute
Exec=/bin/true
Icon=grenplace
FIN

    # Un reglage partage, efface exprès. S'il revient, c'est que
    # `appliquer-systeme` s'est execute apres l'installation — et c'est lui qui
    # branche tout le reste. Sans cette verification, on croirait sur parole
    # qu'il a tourne.
    rm -f /etc/xdg/openbox/rc.xml

    dpkg-deb -b /tmp/suite /tmp/grenos-desktop-suite.deb >/dev/null

    # --- Et un PAQUET neuf, pas seulement un fichier neuf ---
    #
    # Ce qui precede prouve qu'une mise a jour apporte nos propres fichiers.
    # Grenofar a pose une autre question, et c'est la vraie : « je peux mettre
    # a jour depuis grenOS pour la version avec son etc ? » — autrement dit,
    # une machine deja installee recoit-elle les paquets DEBIAN que la nouvelle
    # image ajoute ? Le 26 septembre j'ai repondu oui en LISANT `make-deb.sh`.
    # Lire n'est pas prouver : c'est la faute que j'ai deja payee avec les
    # modules de Calamares.
    #
    # On simule donc sa machine : on lui retire un paquet que la nouvelle liste
    # contient, et on regarde s'il revient. `vdpauinfo` est choisi parce qu'il
    # est une feuille — rien ne depend de lui sauf notre metapaquet — et qu'il
    # pese 45 Kio. `--force-depends` laisse `grenos-systeme` installe mais
    # insatisfait, exactement l'etat d'une machine en retard d'une version.
    AMPUTE=vdpauinfo
    if dpkg-query -W -f='${Status}' "$AMPUTE" 2>/dev/null | grep -q 'ok installed'; then
        dpkg --remove --force-depends "$AMPUTE" >/dev/null 2>&1 || true
        echo "maj : $AMPUTE retire de la machine, comme s'il lui manquait"
    fi

    # Le metapaquet monte de version lui aussi : sans cela apt le voit deja a
    # jour et ne resout pas ses dependances. Une vraie nouvelle image change
    # bien la version des deux.
    SYSTEME_DEB=$(ls dist/grenos-systeme_*_all.deb 2>/dev/null | head -1)
    if [ -n "$SYSTEME_DEB" ]; then
        rm -rf /tmp/suite-systeme
        dpkg-deb -R "$SYSTEME_DEB" /tmp/suite-systeme
        sed -i "s/^Version: .*/Version: ${ANCIENNE}+suite/" \
            /tmp/suite-systeme/DEBIAN/control
        rm -f /tmp/suite-systeme/DEBIAN/md5sums
        dpkg-deb -b /tmp/suite-systeme /tmp/grenos-systeme-suite.deb >/dev/null
        # Le bureau exige le metapaquet a la version exacte : elle a change.
        sed -i "s/grenos-systeme (= [^)]*)/grenos-systeme (= ${ANCIENNE}+suite)/" \
            /tmp/suite/DEBIAN/control
        dpkg-deb -b /tmp/suite /tmp/grenos-desktop-suite.deb >/dev/null
    fi

    # Les deux paquets ensemble, et pas seulement l'un d'eux.
    #
    # `grenos-desktop` exige `grenos-systeme` a la version exacte construite en
    # meme temps que lui. Sur `main`, le depot vient d'etre publie avec cette
    # version-la, donc apt la trouve. Sur une branche, le depot porte encore
    # celle de main, et apt refusait : « Depends: grenos-systeme (= …0208) but
    # …0134 is to be installed ». **Toutes les taches d'agent paraissaient donc
    # echouer**, et personne ne voyait que c'etait l'essai qui avait tort.
    SYSTEME=/tmp/grenos-systeme-suite.deb
    [ -f "$SYSTEME" ] || SYSTEME=""
    apt-get install -y --no-install-recommends \
        -o Dpkg::Options::=--force-confdef \
        -o Dpkg::Options::=--force-confold \
        /tmp/grenos-desktop-suite.deb ${SYSTEME:+"$SYSTEME"}

    NOUVELLE=$(dpkg-query -W -f='${Version}' grenos-desktop)
    echo "version : $ANCIENNE -> $NOUVELLE"
    [ "$NOUVELLE" != "$ANCIENNE" ] || { echo "la version n'a pas change" >&2; exit 1; }

    for preuve in /etc/systemd/system/grenos-nouveaute.service \
                  /usr/share/applications/grenos-nouveaute.desktop \
                  /etc/xdg/openbox/rc.xml; do
        if [ ! -e "$preuve" ]; then
            echo "la mise a jour n'a pas apporte : $preuve" >&2
            exit 1
        fi
    done
    echo "la mise a jour a apporte un service neuf, une entree de menu neuve,"
    echo "et a repose le reglage partage : appliquer-systeme s'est bien execute"

    # Le paquet qu'on avait retire est-il revenu de lui-meme ?
    if dpkg-query -W -f='${Status}' "$AMPUTE" 2>/dev/null | grep -q 'ok installed'; then
        echo "maj : $AMPUTE est revenu — un paquet neuf arrive bien par la mise a jour"
    else
        echo "la mise a jour n a pas rapporte le paquet $AMPUTE" >&2
        echo "une machine deja installee resterait donc en retard sur l image" >&2
        exit 1
    fi

    # Et la machine a-t-elle TOUT ce que la nouvelle image declare ? C'est la
    # promesse entiere : « 0 donnees perdu, et rien a reinstaller ».
    LISTE=linux/config/package-lists/grenos.list.chroot
    manquants=""
    combien=0
    for paquet in $(grep -v '^#' "$LISTE" | grep -v '^[[:space:]]*$'); do
        combien=$((combien + 1))
        dpkg-query -W -f='${Status}' "$paquet" 2>/dev/null | grep -q 'ok installed' \
            || manquants="$manquants $paquet"
    done
    if [ -n "$manquants" ]; then
        echo "apres la mise a jour, ces paquets manquent toujours :$manquants" >&2
        exit 1
    fi
    echo "maj : les $combien paquets de la nouvelle image sont sur la machine"
else
    echo "(pas de paquet local : l'essai de remplacement est saute)"
fi

echo "--- l'installateur a-t-il tout ce qu'il lui faut ---"
# Calamares lit settings.conf, y trouve une suite de modules, et va chercher
# chacun d'eux sur le disque. Si l'un manque, il s'arrete au lancement avec un
# message que personne ne comprend — et on ne l'apprendrait que devant l'ecran,
# au moment d'installer grenOS sur une vraie machine.
#
# On ne peut pas le lancer ici : il faudrait un ecran, un disque et une suite
# de clics. Mais on peut lire sa configuration et demander au disque si chaque
# morceau qu'elle nomme existe.
python3 linux/tools/verif-installateur.py

echo "la mise a jour depuis grenOS fonctionne"
